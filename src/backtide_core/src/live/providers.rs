//! WebSocket market-data providers and deterministic stream abstractions.

use crate::data::models::{Interval, Provider};
use crate::live::models::MarketUpdate;
use async_trait::async_trait;
use chrono::DateTime;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

/// Errors returned by live market-data streams.
#[derive(Debug, Error)]
pub enum LiveStreamError {
    #[error("{0}")]
    Unsupported(String),

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("invalid provider message: {0}")]
    InvalidMessage(String),
}

/// Provider-independent asynchronous market-data source.
#[async_trait]
pub trait MarketDataStream: Send {
    /// Return the next market update, or `None` after a finite stream ends.
    async fn next_update(&mut self) -> Result<Option<MarketUpdate>, LiveStreamError>;
}

/// Finite deterministic stream for unit tests, examples, and replays.
pub struct MockMarketDataStream {
    updates: VecDeque<MarketUpdate>,
}

impl MockMarketDataStream {
    /// Create a finite stream that yields `updates` in insertion order.
    pub fn new(updates: Vec<MarketUpdate>) -> Self {
        Self {
            updates: updates.into(),
        }
    }
}

#[async_trait]
impl MarketDataStream for MockMarketDataStream {
    async fn next_update(&mut self) -> Result<Option<MarketUpdate>, LiveStreamError> {
        Ok(self.updates.pop_front())
    }
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connected exchange WebSocket stream.
pub struct ExchangeMarketDataStream {
    provider: Provider,
    interval: Interval,
    socket: Socket,
    queued: VecDeque<MarketUpdate>,
    partial: HashMap<String, MarketUpdate>,
    canonical_symbols: HashMap<String, String>,
}

impl ExchangeMarketDataStream {
    /// Connect and subscribe to candles for the requested symbols.
    pub async fn connect(
        provider: Provider,
        symbols: &[String],
        interval: Interval,
    ) -> Result<Self, LiveStreamError> {
        validate_subscription(provider, symbols, interval)?;
        let endpoint = match provider {
            Provider::Binance => "wss://stream.binance.com:9443/ws",
            Provider::Coinbase => "wss://advanced-trade-ws.coinbase.com",
            Provider::Kraken => "wss://ws.kraken.com/v2",
            Provider::Yahoo => {
                return Err(LiveStreamError::Unsupported(
                    "Yahoo Finance does not expose a market-data WebSocket".to_owned(),
                ));
            },
        };

        let (mut socket, _) = connect_async(endpoint).await?;
        let subscription = subscription_message(provider, symbols, interval)?;
        socket.send(Message::Text(subscription.to_string().into())).await?;
        if provider == Provider::Coinbase {
            // Coinbase recommends subscribing to heartbeats alongside sparse
            // channels, otherwise it may close an idle candles connection.
            let heartbeat = json!({
                "type": "subscribe",
                "channel": "heartbeats",
            });
            socket.send(Message::Text(heartbeat.to_string().into())).await?;
        }

        Ok(Self {
            provider,
            interval,
            socket,
            queued: VecDeque::new(),
            partial: HashMap::new(),
            canonical_symbols: canonical_symbol_map(provider, symbols),
        })
    }

    fn queue_updates(&mut self, updates: Vec<MarketUpdate>) {
        for mut update in updates {
            let native_key = provider_symbol_key(self.provider, &update.symbol);
            let Some(canonical) = self.canonical_symbols.get(&native_key) else {
                // Ignore unsolicited symbols rather than allowing provider
                // naming conventions to leak across the engine boundary.
                continue;
            };
            update.symbol.clone_from(canonical);
            queue_chronological_update(&mut self.queued, &mut self.partial, update);
        }
    }
}

fn queue_chronological_update(
    queued: &mut VecDeque<MarketUpdate>,
    partial: &mut HashMap<String, MarketUpdate>,
    update: MarketUpdate,
) {
    let symbol = update.symbol.clone();
    let previous_timestamp = partial.get(&symbol).map(|previous| previous.open_ts);

    match previous_timestamp {
        Some(timestamp) if update.open_ts < timestamp => {},
        Some(timestamp) if update.open_ts == timestamp => {
            if update.is_final {
                partial.remove(&symbol);
            } else {
                partial.insert(symbol, update.clone());
            }
            queued.push_back(update);
        },
        Some(_) => {
            if let Some(mut previous) = partial.remove(&symbol) {
                previous.is_final = true;
                queued.push_back(previous);
            }
            if !update.is_final {
                partial.insert(symbol, update.clone());
            }
            queued.push_back(update);
        },
        None => {
            if !update.is_final {
                partial.insert(symbol, update.clone());
            }
            queued.push_back(update);
        },
    }
}

#[async_trait]
impl MarketDataStream for ExchangeMarketDataStream {
    async fn next_update(&mut self) -> Result<Option<MarketUpdate>, LiveStreamError> {
        loop {
            if let Some(update) = self.queued.pop_front() {
                return Ok(Some(update));
            }

            let Some(message) = self.socket.next().await else {
                return Ok(None);
            };

            match message? {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(text.as_ref()).map_err(|error| {
                        LiveStreamError::InvalidMessage(format!("invalid JSON: {error}"))
                    })?;
                    let updates = parse_message(self.provider, self.interval, &value)?;
                    self.queue_updates(updates);
                },
                Message::Ping(payload) => {
                    self.socket.send(Message::Pong(payload)).await?;
                },
                Message::Close(_) => return Ok(None),
                _ => {},
            }
        }
    }
}

/// Explain whether a provider can stream the requested candle interval.
pub fn support_message(provider: Provider, interval: Interval) -> Result<&'static str, String> {
    match provider {
        Provider::Yahoo => Err(
            "Yahoo Finance does not expose an official market-data WebSocket; use Binance, Coinbase, or Kraken for live trading"
                .to_owned(),
        ),
        Provider::Coinbase if interval != Interval::FiveMinutes => Err(
            "Coinbase's public candles WebSocket emits five-minute candles only; select interval='5m'"
                .to_owned(),
        ),
        Provider::Binance => Ok("Binance public kline WebSocket"),
        Provider::Coinbase => Ok("Coinbase Advanced Trade public candles WebSocket"),
        Provider::Kraken => Ok("Kraken public OHLC WebSocket v2"),
    }
}

fn validate_subscription(
    provider: Provider,
    symbols: &[String],
    interval: Interval,
) -> Result<(), LiveStreamError> {
    support_message(provider, interval).map_err(LiveStreamError::Unsupported)?;
    if symbols.is_empty() || symbols.iter().any(|symbol| symbol.trim().is_empty()) {
        return Err(LiveStreamError::Unsupported(
            "at least one non-empty symbol is required".to_owned(),
        ));
    }
    Ok(())
}

fn subscription_message(
    provider: Provider,
    symbols: &[String],
    interval: Interval,
) -> Result<Value, LiveStreamError> {
    let message = match provider {
        Provider::Binance => json!({
            "method": "SUBSCRIBE",
            "params": symbols
                .iter()
                .map(|symbol| format!("{}@kline_{}", compact_symbol(symbol), interval))
                .collect::<Vec<_>>(),
            "id": 1,
        }),
        Provider::Coinbase => json!({
            "type": "subscribe",
            "channel": "candles",
            "product_ids": symbols.iter().map(|symbol| dash_symbol(symbol)).collect::<Vec<_>>(),
        }),
        Provider::Kraken => json!({
            "method": "subscribe",
            "params": {
                "channel": "ohlc",
                "symbol": symbols.iter().map(|symbol| slash_symbol(symbol)).collect::<Vec<_>>(),
                "interval": interval.minutes(),
                "snapshot": true,
            },
        }),
        Provider::Yahoo => {
            return Err(LiveStreamError::Unsupported(
                "Yahoo Finance does not expose a market-data WebSocket".to_owned(),
            ));
        },
    };
    Ok(message)
}

fn parse_message(
    provider: Provider,
    interval: Interval,
    value: &Value,
) -> Result<Vec<MarketUpdate>, LiveStreamError> {
    match provider {
        Provider::Binance => parse_binance(interval, value),
        Provider::Coinbase => parse_coinbase(interval, value),
        Provider::Kraken => parse_kraken(interval, value),
        Provider::Yahoo => Err(LiveStreamError::Unsupported(
            "Yahoo Finance does not expose a market-data WebSocket".to_owned(),
        )),
    }
}

fn parse_binance(interval: Interval, value: &Value) -> Result<Vec<MarketUpdate>, LiveStreamError> {
    let Some(kline) = value.get("k") else {
        return Ok(Vec::new());
    };
    let symbol = value
        .get("s")
        .or_else(|| kline.get("s"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if symbol.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![MarketUpdate {
        provider: "binance".to_owned(),
        symbol,
        interval: interval.to_string(),
        open_ts: millis_to_seconds(required_u64(kline, "t")?),
        close_ts: millis_to_seconds(required_u64(kline, "T")?),
        open: required_number(kline, "o")?,
        high: required_number(kline, "h")?,
        low: required_number(kline, "l")?,
        close: required_number(kline, "c")?,
        volume: required_number(kline, "v")?,
        n_trades: kline.get("n").and_then(Value::as_i64).and_then(|n| i32::try_from(n).ok()),
        is_final: kline.get("x").and_then(Value::as_bool).unwrap_or(false),
        received_ts: received_ts(),
    }])
}

fn parse_coinbase(interval: Interval, value: &Value) -> Result<Vec<MarketUpdate>, LiveStreamError> {
    let Some(events) = value.get("events").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut updates = Vec::new();
    for candle in
        events.iter().filter_map(|event| event.get("candles").and_then(Value::as_array)).flatten()
    {
        let open_ts = required_u64(candle, "start")?;
        updates.push(MarketUpdate {
            provider: "coinbase".to_owned(),
            symbol: required_string(candle, "product_id")?,
            interval: interval.to_string(),
            open_ts,
            close_ts: open_ts + interval.minutes() * 60,
            open: required_number(candle, "open")?,
            high: required_number(candle, "high")?,
            low: required_number(candle, "low")?,
            close: required_number(candle, "close")?,
            volume: required_number(candle, "volume")?,
            n_trades: None,
            is_final: false,
            received_ts: received_ts(),
        });
    }
    updates.sort_unstable_by_key(|update| update.open_ts);
    Ok(updates)
}

fn parse_kraken(interval: Interval, value: &Value) -> Result<Vec<MarketUpdate>, LiveStreamError> {
    if value.get("channel").and_then(Value::as_str) != Some("ohlc") {
        return Ok(Vec::new());
    }
    let Some(data) = value.get("data").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut updates = Vec::with_capacity(data.len());
    for candle in data {
        let begin = required_string(candle, "interval_begin")?;
        let timestamp = DateTime::parse_from_rfc3339(&begin)
            .map_err(|error| LiveStreamError::InvalidMessage(error.to_string()))?
            .timestamp();
        let open_ts = u64::try_from(timestamp).map_err(|_| {
            LiveStreamError::InvalidMessage("Kraken timestamp predates Unix epoch".to_owned())
        })?;
        updates.push(MarketUpdate {
            provider: "kraken".to_owned(),
            symbol: required_string(candle, "symbol")?.replace('/', "-"),
            interval: interval.to_string(),
            open_ts,
            close_ts: open_ts + interval.minutes() * 60,
            open: required_number(candle, "open")?,
            high: required_number(candle, "high")?,
            low: required_number(candle, "low")?,
            close: required_number(candle, "close")?,
            volume: required_number(candle, "volume")?,
            n_trades: candle
                .get("trades")
                .and_then(Value::as_i64)
                .and_then(|n| i32::try_from(n).ok()),
            is_final: false,
            received_ts: received_ts(),
        });
    }
    updates.sort_unstable_by_key(|update| update.open_ts);
    Ok(updates)
}

fn required_number(value: &Value, key: &str) -> Result<f64, LiveStreamError> {
    let raw = value
        .get(key)
        .ok_or_else(|| LiveStreamError::InvalidMessage(format!("missing numeric field {key:?}")))?;
    raw.as_f64()
        .or_else(|| raw.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| LiveStreamError::InvalidMessage(format!("invalid numeric field {key:?}")))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, LiveStreamError> {
    let raw = value
        .get(key)
        .ok_or_else(|| LiveStreamError::InvalidMessage(format!("missing integer field {key:?}")))?;
    raw.as_u64()
        .or_else(|| raw.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| LiveStreamError::InvalidMessage(format!("invalid integer field {key:?}")))
}

fn required_string(value: &Value, key: &str) -> Result<String, LiveStreamError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| LiveStreamError::InvalidMessage(format!("missing string field {key:?}")))
}

fn millis_to_seconds(timestamp: u64) -> u64 {
    timestamp / 1_000
}

fn compact_symbol(symbol: &str) -> String {
    symbol.replace(['-', '/', '_'], "").to_ascii_lowercase()
}

fn dash_symbol(symbol: &str) -> String {
    symbol.replace(['/', '_'], "-").to_ascii_uppercase()
}

fn slash_symbol(symbol: &str) -> String {
    symbol.replace(['-', '_'], "/").to_ascii_uppercase()
}

fn provider_symbol_key(provider: Provider, symbol: &str) -> String {
    match provider {
        Provider::Binance => symbol.replace(['-', '/', '_'], "").to_ascii_uppercase(),
        Provider::Coinbase => dash_symbol(symbol),
        Provider::Kraken => slash_symbol(symbol),
        Provider::Yahoo => symbol.to_ascii_uppercase(),
    }
}

fn canonical_symbol_map(provider: Provider, symbols: &[String]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for symbol in symbols {
        let canonical = dash_symbol(symbol);
        let native = provider_symbol_key(provider, symbol);
        result.insert(native.clone(), canonical.clone());

        if provider == Provider::Kraken {
            // Kraken may return its exchange aliases even when the v2
            // subscription accepts the common asset name.
            if native.starts_with("BTC/") {
                result.insert(native.replacen("BTC/", "XBT/", 1), canonical.clone());
            } else if native.starts_with("XBT/") {
                result.insert(native.replacen("XBT/", "BTC/", 1), canonical.clone());
            }
            if native.starts_with("DOGE/") {
                result.insert(native.replacen("DOGE/", "XDG/", 1), canonical.clone());
            } else if native.starts_with("XDG/") {
                result.insert(native.replacen("XDG/", "DOGE/", 1), canonical.clone());
            }
        }
    }
    result
}

fn received_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial_update(open_ts: u64, close: f64) -> MarketUpdate {
        MarketUpdate {
            provider: "mock".to_owned(),
            symbol: "BTC-USD".to_owned(),
            interval: "5m".to_owned(),
            open_ts,
            close_ts: open_ts + 300,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
            n_trades: None,
            is_final: false,
            received_ts: open_ts as i64,
        }
    }

    #[tokio::test]
    async fn mock_stream_is_finite_and_deterministic() {
        let expected = MarketUpdate {
            provider: "mock".to_owned(),
            symbol: "BTC-USD".to_owned(),
            interval: "1m".to_owned(),
            open_ts: 1,
            close_ts: 61,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 3.0,
            n_trades: Some(2),
            is_final: true,
            received_ts: 61,
        };
        let mut stream = MockMarketDataStream::new(vec![expected.clone()]);

        assert_eq!(stream.next_update().await.unwrap().unwrap().close, expected.close);
        assert!(stream.next_update().await.unwrap().is_none());
    }

    #[test]
    fn yahoo_limitation_is_explicit() {
        let error = support_message(Provider::Yahoo, Interval::OneMinute).unwrap_err();
        assert!(error.contains("does not expose an official"));
    }

    #[test]
    fn coinbase_rejects_non_five_minute_candles() {
        assert!(support_message(Provider::Coinbase, Interval::OneMinute).is_err());
        assert!(support_message(Provider::Coinbase, Interval::FiveMinutes).is_ok());
    }

    #[test]
    fn parses_binance_closed_kline() {
        let message = json!({
            "s": "BTCUSDT",
            "k": {
                "t": 1_700_000_000_000_u64,
                "T": 1_700_000_059_999_u64,
                "o": "100.0",
                "h": "110.0",
                "l": "90.0",
                "c": "105.0",
                "v": "12.5",
                "n": 42,
                "x": true
            }
        });
        let updates = parse_binance(Interval::OneMinute, &message).unwrap();

        assert_eq!(updates.len(), 1);
        // Parsing retains the provider symbol; the connected stream maps it
        // back to the exact canonical symbol requested at subscription time.
        assert_eq!(updates[0].symbol, "BTCUSDT");
        assert_eq!(updates[0].open_ts, 1_700_000_000);
        assert!(updates[0].is_final);
    }

    #[test]
    fn parses_coinbase_candle() {
        let message = json!({
            "events": [{
                "candles": [{
                    "start": "1700000000",
                    "product_id": "BTC-USD",
                    "open": "100.0",
                    "high": "110.0",
                    "low": "90.0",
                    "close": "105.0",
                    "volume": "12.5"
                }]
            }]
        });
        let updates = parse_coinbase(Interval::FiveMinutes, &message).unwrap();

        assert_eq!(updates[0].symbol, "BTC-USD");
        assert_eq!(updates[0].close_ts, 1_700_000_300);
        assert!(!updates[0].is_final);
    }

    #[test]
    fn parses_kraken_ohlc() {
        let message = json!({
            "channel": "ohlc",
            "data": [{
                "symbol": "BTC/USD",
                "interval_begin": "2023-11-14T22:13:20Z",
                "open": 100.0,
                "high": 110.0,
                "low": 90.0,
                "close": 105.0,
                "volume": 12.5,
                "trades": 42
            }]
        });
        let updates = parse_kraken(Interval::OneMinute, &message).unwrap();

        assert_eq!(updates[0].symbol, "BTC-USD");
        assert_eq!(updates[0].open_ts, 1_700_000_000);
    }

    #[test]
    fn canonical_map_normalizes_binance_and_kraken_aliases() {
        let binance = canonical_symbol_map(Provider::Binance, &["BTC-USDT".to_owned()]);
        assert_eq!(binance["BTCUSDT"], "BTC-USDT");

        let kraken = canonical_symbol_map(Provider::Kraken, &["BTC-USD".to_owned()]);
        assert_eq!(kraken["BTC/USD"], "BTC-USD");
        assert_eq!(kraken["XBT/USD"], "BTC-USD");
    }

    #[test]
    fn older_snapshot_candles_do_not_replace_current_partial() {
        let mut queued = VecDeque::new();
        let mut partial = HashMap::new();
        queue_chronological_update(&mut queued, &mut partial, partial_update(2_000, 101.0));
        queued.clear();

        queue_chronological_update(&mut queued, &mut partial, partial_update(1_000, 99.0));

        assert!(queued.is_empty());
        assert_eq!(partial["BTC-USD"].open_ts, 2_000);
        assert_eq!(partial["BTC-USD"].close, 101.0);
    }

    #[test]
    fn equal_partial_replaces_and_newer_partial_finalizes() {
        let mut queued = VecDeque::new();
        let mut partial = HashMap::new();
        queue_chronological_update(&mut queued, &mut partial, partial_update(1_000, 100.0));
        queued.clear();

        queue_chronological_update(&mut queued, &mut partial, partial_update(1_000, 101.0));
        assert_eq!(queued.pop_front().unwrap().close, 101.0);
        assert_eq!(partial["BTC-USD"].close, 101.0);

        queue_chronological_update(&mut queued, &mut partial, partial_update(1_300, 102.0));
        let finalized = queued.pop_front().unwrap();
        assert_eq!(finalized.open_ts, 1_000);
        assert!(finalized.is_final);
        assert_eq!(queued.pop_front().unwrap().open_ts, 1_300);
        assert_eq!(partial["BTC-USD"].open_ts, 1_300);
    }
}
