//! DuckDB storage solution.

use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::storage_summary::StorageSummary;
use crate::storage::traits::Storage;
use duckdb::params;
use duckdb::params_from_iter;
use duckdb::Connection;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct DuckDb {
    /// Connection to the database.
    conn: Mutex<Connection>,
}

impl DuckDb {
    pub fn new(path: &PathBuf) -> StorageResult<Self> {
        create_dir_all(path)?;

        Ok(Self {
            conn: Mutex::new(Connection::open(path.join("database.duckdb"))?),
        })
    }
}

impl Storage for DuckDb {
    /// Initialize all tables in the database.
    fn init(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();

        // Use UNIQUE instead of KEYS since appender doesn't play well with keys
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bars (
                symbol            VARCHAR NOT NULL,
                instrument_type   VARCHAR NOT NULL,
                interval          VARCHAR NOT NULL,
                provider          VARCHAR NOT NULL,
                open_ts           BIGINT NOT NULL,
                close_ts          BIGINT NOT NULL,
                open_ts_exchange  BIGINT NOT NULL,
                open              DOUBLE NOT NULL,
                high              DOUBLE NOT NULL,
                low               DOUBLE NOT NULL,
                close             DOUBLE NOT NULL,
                adj_close         DOUBLE NOT NULL,
                volume            DOUBLE NOT NULL,
                n_trades          INTEGER,
                UNIQUE (symbol, provider, interval, open_ts)
            );
        ",
        )?;

        Ok(())
    }

    /// Store multiple series of OHLC data in one bulk operation.
    ///
    /// 1. Removes overlapping rows for every series in a single transaction.
    /// 2. Bulk-inserts all rows from every series via DuckDB's `Appender`.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        // Filter out empty series early.
        let non_empty: Vec<&BarSeries> = series.iter().filter(|s| !s.bars.is_empty()).collect();
        if non_empty.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Phase 1: delete all overlapping ranges in a single transaction.
        conn.execute_batch("BEGIN TRANSACTION")?;
        for s in &non_empty {
            let iv = s.interval.to_string();
            let prov = s.provider.to_string();
            let min_ts = s.bars.iter().map(|b| b.open_ts).min().unwrap();
            let max_ts = s.bars.iter().map(|b| b.open_ts).max().unwrap();
            conn.execute(
                "DELETE FROM bars
                 WHERE symbol = ? AND interval = ? AND provider = ?
                    AND open_ts >= ? AND open_ts <= ?",
                params![&s.symbol, iv, prov, min_ts as i64, max_ts as i64],
            )?;
        }
        conn.execute_batch("COMMIT")?;

        // Phase 2: bulk-insert every row via the Appender (one flush).
        let mut appender = conn.appender("bars")?;
        for s in &non_empty {
            let at = s.instrument_type.to_string();
            let iv = s.interval.to_string();
            let prov = s.provider.to_string();
            for bar in &s.bars {
                appender.append_row(params![
                    &s.symbol,
                    &at,
                    &iv,
                    &prov,
                    bar.open_ts as i64,
                    bar.close_ts as i64,
                    bar.open_ts_exchange as i64,
                    bar.open,
                    bar.high,
                    bar.low,
                    bar.close,
                    bar.adj_close,
                    bar.volume,
                    bar.n_trades,
                ])?;
            }
        }
        appender.flush()?;

        Ok(())
    }

    /// Get the (min_ts, max_ts) of stored bars for a given symbol/provider/interval.
    /// Returns `None` if no data exists.
    fn get_stored_range(
        &self,
        symbol: &str,
        interval: Interval,
        provider: Provider,
    ) -> StorageResult<Option<(u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT MIN(open_ts), MAX(open_ts) FROM bars
             WHERE symbol = ? AND interval = ? AND provider = ?",
        )?;

        let result =
            stmt.query_row(params![symbol, interval.to_string(), provider.to_string()], |row| {
                let min_ts: Option<u64> = row.get(0)?;
                let max_ts: Option<u64> = row.get(1)?;
                Ok((min_ts, max_ts))
            })?;

        match result {
            (Some(min), Some(max)) => Ok(Some((min, max))),
            _ => Ok(None),
        }
    }

    /// Return a summary for every (symbol, provider, interval) series in the database.
    fn get_summary(&self) -> StorageResult<Vec<StorageSummary>> {
        let conn = self.conn.lock().unwrap();

        // Step 1: get each series with its stats
        let mut stmt = conn.prepare(
            "SELECT symbol, interval, instrument_type, provider, MIN(open_ts), MAX(open_ts), COUNT(*)
             FROM bars
             GROUP BY symbol, interval, instrument_type, provider
             ORDER BY symbol, interval, provider",
        )?;

        let rows: Vec<(String, String, String, String, u64, u64, u64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            })?
            .collect::<Result<_, _>>()?;

        // Step 2: for each series, fetch last 365 adj_close values
        let mut sparkline_stmt = conn.prepare(
            "SELECT adj_close FROM (
                SELECT adj_close, open_ts FROM bars
                WHERE symbol = ? AND provider = ? AND interval = ?
                ORDER BY open_ts DESC
                LIMIT 365
            ) sub ORDER BY open_ts ASC",
        )?;

        let mut summaries = Vec::with_capacity(rows.len());
        for (symbol, interval, it, provider, first_ts, last_ts, n_rows) in rows {
            let sparkline: Vec<f64> = sparkline_stmt
                .query_map(params![symbol, provider, interval], |row| row.get(0))?
                .collect::<Result<_, _>>()?;

            summaries.push(StorageSummary {
                symbol,
                provider,
                interval,
                instrument_type: it,
                first_ts,
                last_ts,
                n_rows,
                sparkline,
            });
        }

        Ok(summaries)
    }

    /// Delete all bars for a given (symbol, provider, interval) series.
    fn delete_rows(
        &self,
        symbol: &str,
        interval: Option<Interval>,
        provider: Option<Provider>,
    ) -> StorageResult<u64> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("DELETE FROM bars WHERE symbol = ?");
        let mut values: Vec<String> = vec![symbol.to_string()];

        if let Some(interval) = interval {
            sql.push_str(" AND interval = ?");
            values.push(interval.to_string());
        }
        if let Some(provider) = provider {
            sql.push_str(" AND provider = ?");
            values.push(provider.to_string());
        }

        let deleted = conn.execute(&sql, params_from_iter(values.iter()))?;
        Ok(deleted as u64)
    }
}
