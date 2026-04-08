//! DuckDB storage solution.

use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::storage::errors::StorageResult;
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
        create_dir_all(&path)?;

        Ok(Self {
            conn: Mutex::new(Connection::open(path.join("database.duckdb"))?),
        })
    }
}

impl Storage for DuckDb {
    /// Initialize all tables in the database.
    fn init(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bars (
                symbol            VARCHAR NOT NULL,
                asset_type        VARCHAR NOT NULL,
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
                PRIMARY KEY (symbol, provider, interval, open_ts)
            );
        ",
        )?;

        Ok(())
    }

    /// Store OHLC data.
    fn write_bars(
        &self,
        symbol: &str,
        asset_type: AssetType,
        interval: Interval,
        provider: Provider,
        bars: &[Bar],
    ) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO bars (
                symbol, asset_type, interval, provider, open_ts, close_ts, open_ts_exchange,
                open, high, low, close, adj_close, volume, n_trades
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;

        for bar in bars {
            stmt.execute(params![
                symbol,
                asset_type.to_string(),
                interval.to_string(),
                provider.to_string(),
                bar.open_ts,
                bar.close_ts,
                bar.open_ts_exchange,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                bar.adj_close,
                bar.volume,
                bar.n_trades,
            ])?;
        }

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
            stmt.query_row(params![symbol, provider.to_string(), interval.to_string()], |row| {
                let min_ts: Option<u64> = row.get(0)?;
                let max_ts: Option<u64> = row.get(1)?;
                Ok((min_ts, max_ts))
            })?;

        match result {
            (Some(min), Some(max)) => Ok(Some((min, max))),
            _ => Ok(None),
        }
    }

    /// Return a summary for every (symbol, provider, interval) group in the database.
    fn get_summary(&self) -> StorageResult<Vec<StorageSummary>> {
        let conn = self.conn.lock().unwrap();

        // Step 1: get groups with their stats
        let mut stmt = conn.prepare(
            "SELECT symbol, interval, asset_type, provider, MIN(open_ts), MAX(open_ts), COUNT(*)
             FROM bars
             GROUP BY symbol, interval, asset_type, provider
             ORDER BY symbol, interval, provider",
        )?;

        let groups: Vec<(String, String, String, String, u64, u64, u64)> = stmt
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

        // Step 2: for each group, fetch last 365 adj_close values
        let mut sparkline_stmt = conn.prepare(
            "SELECT adj_close FROM (
                SELECT adj_close, open_ts FROM bars
                WHERE symbol = ? AND provider = ? AND interval = ?
                ORDER BY open_ts DESC
                LIMIT 365
            ) sub ORDER BY open_ts ASC",
        )?;

        let mut summaries = Vec::with_capacity(groups.len());
        for (symbol, interval, at, provider, first_ts, last_ts, n_rows) in groups {
            let sparkline: Vec<f64> = sparkline_stmt
                .query_map(params![symbol, provider, interval], |row| row.get(0))?
                .collect::<Result<_, _>>()?;

            summaries.push(StorageSummary {
                symbol,
                provider,
                interval,
                asset_type: at,
                first_ts,
                last_ts,
                n_rows,
                sparkline,
            });
        }

        Ok(summaries)
    }

    /// Delete all bars for a given (symbol, provider, interval) group.
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
