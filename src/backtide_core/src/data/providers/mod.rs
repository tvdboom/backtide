mod binance;
mod coinbase;
mod kraken;
mod traits;
mod yahoo;

use crate::constants::{Symbol, TASK_TIMEOUT};
use crate::data::errors::{DataError, DataResult};
use crate::data::models::{Bar, Instrument, InstrumentType, Interval};
use std::time::{SystemTime, UNIX_EPOCH};

pub use binance::Binance;
pub use coinbase::Coinbase;
pub use kraken::Kraken;
pub use traits::DataProvider;
pub use yahoo::YahooFinance;

/// Fetch a bounded daily preview directly from a provider without touching storage.
pub(crate) async fn load_bar_preview(
    provider: &dyn DataProvider,
    symbol: &Symbol,
    instrument_type: InstrumentType,
    limit: usize,
) -> DataResult<(Instrument, Vec<Bar>)> {
    let interval = Interval::OneDay;
    let task = async {
        let instrument = provider.fetch_instrument(symbol, instrument_type).await?;
        // Request extra calendar days so equity previews still contain `limit`
        // trading sessions after weekends and exchange holidays are removed.
        let preview_seconds = interval.minutes() * 60 * (limit as u64).saturating_mul(3);
        let end = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let start = end.saturating_sub(preview_seconds);
        let mut bars =
            provider.download_bars(symbol, instrument_type, interval, start, end).await?.bars;
        bars.retain(|bar| bar.adj_close.is_finite());
        bars.sort_unstable_by_key(|bar| bar.open_ts);
        bars.dedup_by_key(|bar| bar.open_ts);
        if bars.len() > limit {
            bars.drain(..bars.len() - limit);
        }
        Ok((instrument, bars))
    };

    tokio::time::timeout(TASK_TIMEOUT, task).await.map_err(|_| DataError::Timeout {
        symbol: symbol.clone(),
        interval,
    })?
}
