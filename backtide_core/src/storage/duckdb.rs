//! DuckDB storage solution.

use crate::data::models::bar::Bar;
use crate::data::models::dividend::Dividend;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::bar_summary::BarSummary;
use crate::storage::models::dividend_series::DividendSeries;
use crate::storage::models::stored_bar::StoredBar;
use crate::storage::models::stored_dividend::StoredDividend;
use crate::storage::traits::Storage;
use duckdb::params;
use duckdb::params_from_iter;
use duckdb::Connection;
use std::collections::HashMap;
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

            CREATE TABLE IF NOT EXISTS dividends (
                symbol            VARCHAR NOT NULL,
                provider          VARCHAR NOT NULL,
                ex_date           BIGINT NOT NULL,
                amount            DOUBLE NOT NULL,
                UNIQUE (symbol, provider, ex_date)
            );
        ",
        )?;

        Ok(())
    }

    /// Get all stored ranges in a single query, keyed by (symbol, interval, provider).
    fn get_bar_ranges(&self) -> StorageResult<HashMap<(String, String, String), (u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol, interval, provider, MIN(open_ts), MAX(open_ts)
             FROM bars
             GROUP BY symbol, interval, provider",
        )?;

        let rows = stmt
            .query_map([], |row| {
                let symbol: String = row.get(0)?;
                let interval: String = row.get(1)?;
                let provider: String = row.get(2)?;
                let min_ts: u64 = row.get(3)?;
                let max_ts: u64 = row.get(4)?;
                Ok(((symbol, interval, provider), (min_ts, max_ts)))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(rows)
    }

    /// Return a pre-aggregated summary of stored bars.
    fn get_bars_summary(&self) -> StorageResult<Vec<BarSummary>> {
        let conn = self.conn.lock().unwrap();

        // Phase 1: grouped summary (one row per series).
        let mut stmt = conn.prepare(
            "SELECT symbol, instrument_type, interval, provider,
                    MIN(open_ts) AS first_ts,
                    MAX(open_ts) AS last_ts,
                    COUNT(*)     AS n_rows
             FROM bars
             GROUP BY symbol, instrument_type, interval, provider
             ORDER BY symbol, interval",
        )?;

        let mut summaries: Vec<BarSummary> = stmt
            .query_map([], |row| {
                Ok(BarSummary {
                    symbol: row.get(0)?,
                    instrument_type: row.get(1)?,
                    interval: row.get(2)?,
                    provider: row.get(3)?,
                    first_ts: row.get(4)?,
                    last_ts: row.get(5)?,
                    n_rows: row.get(6)?,
                    sparkline: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Phase 2: fetch the last 365 adj_close values per group for sparklines.
        // Uses a window function to rank rows within each group, then filters.
        let mut spark_stmt = conn.prepare(
            "SELECT symbol, interval, provider, adj_close
             FROM (
                 SELECT symbol, interval, provider, adj_close,
                        ROW_NUMBER() OVER (
                            PARTITION BY symbol, interval, provider
                            ORDER BY open_ts DESC
                        ) AS rn
                 FROM bars
             )
             WHERE rn <= 365
             ORDER BY symbol, interval, provider, rn DESC",
        )?;

        // Build a map of (symbol, interval, provider) -> Vec<adj_close>.
        let mut sparkline_map: HashMap<(String, String, String), Vec<f64>> = HashMap::new();
        let mut spark_rows = spark_stmt.query([])?;
        while let Some(row) = spark_rows.next()? {
            let key: (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            let val: f64 = row.get(3)?;
            sparkline_map.entry(key).or_default().push(val);
        }

        // Attach sparklines to summaries.
        for s in &mut summaries {
            let key = (s.symbol.clone(), s.interval.clone(), s.provider.clone());
            if let Some(spark) = sparkline_map.remove(&key) {
                s.sparkline = spark;
            }
        }

        Ok(summaries)
    }

    /// Return all stored bars.
    fn get_all_bars(&self) -> StorageResult<Vec<StoredBar>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT symbol, instrument_type, interval, provider,
                    open_ts, close_ts, open_ts_exchange,
                    open, high, low, close, adj_close, volume, n_trades
             FROM bars
             ORDER BY symbol, interval, open_ts",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(StoredBar {
                    symbol: row.get(0)?,
                    instrument_type: row.get(1)?,
                    interval: row.get(2)?,
                    provider: row.get(3)?,
                    bar: Bar {
                        open_ts: row.get(4)?,
                        close_ts: row.get(5)?,
                        open_ts_exchange: row.get(6)?,
                        open: row.get(7)?,
                        high: row.get(8)?,
                        low: row.get(9)?,
                        close: row.get(10)?,
                        adj_close: row.get(11)?,
                        volume: row.get(12)?,
                        n_trades: row.get(13)?,
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Return all stored dividends.
    fn get_all_dividends(&self) -> StorageResult<Vec<StoredDividend>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT symbol, provider, ex_date, amount
             FROM dividends
             ORDER BY symbol, ex_date",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(StoredDividend {
                    symbol: row.get(0)?,
                    provider: row.get(1)?,
                    dividend: Dividend {
                        ex_date: row.get(2)?,
                        amount: row.get(3)?,
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
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

    /// Store multiple series of dividend events in one bulk operation.
    ///
    /// 1. Removes overlapping rows for every series in a single transaction.
    /// 2. Bulk-inserts all rows from every series via DuckDB's `Appender`.
    fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        let non_empty: Vec<&DividendSeries> =
            series.iter().filter(|s| !s.dividends.is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Phase 1: delete overlapping ranges.
        conn.execute_batch("BEGIN TRANSACTION")?;
        for s in &non_empty {
            let prov = s.provider.to_string();
            let min_ts = s.dividends.iter().map(|d| d.ex_date).min().unwrap();
            let max_ts = s.dividends.iter().map(|d| d.ex_date).max().unwrap();
            conn.execute(
                "DELETE FROM dividends
                 WHERE symbol = ? AND provider = ?
                    AND ex_date >= ? AND ex_date <= ?",
                params![&s.symbol, prov, min_ts as i64, max_ts as i64],
            )?;
        }
        conn.execute_batch("COMMIT")?;

        // Phase 2: bulk-insert every row via the Appender.
        let mut appender = conn.appender("dividends")?;
        for s in &non_empty {
            let prov = s.provider.to_string();
            for div in &s.dividends {
                appender.append_row(params![&s.symbol, &prov, div.ex_date as i64, div.amount,])?;
            }
        }
        appender.flush()?;

        Ok(())
    }

    /// Delete bars (and orphaned dividends) for one or more series in a single transaction.
    fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64> {
        if series.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN TRANSACTION")?;

        let mut total_deleted = 0u64;

        // Phase 1: delete bars for each series.
        for (symbol, interval, provider) in series {
            let mut sql = String::from("DELETE FROM bars WHERE symbol = ?");
            let mut values: Vec<String> = vec![symbol.clone()];

            if let Some(iv) = interval {
                sql.push_str(" AND interval = ?");
                values.push(iv.to_string());
            }
            if let Some(prov) = provider {
                sql.push_str(" AND provider = ?");
                values.push(prov.to_string());
            }

            let deleted = conn.execute(&sql, params_from_iter(values.iter()))?;
            total_deleted += deleted as u64;
        }

        // Phase 2: clean up orphaned dividends — any symbol that has no
        // remaining bars (scoped to provider when given) gets its dividends removed.
        let mut checked = std::collections::HashSet::new();
        for (symbol, _, provider) in series {
            let key = (symbol.clone(), provider.map(|p| p.to_string()));
            if !checked.insert(key) {
                continue;
            }

            let (check_sql, check_params): (&str, Vec<String>) = if let Some(prov) = provider {
                (
                    "SELECT COUNT(*) FROM bars WHERE symbol = ? AND provider = ?",
                    vec![symbol.clone(), prov.to_string()],
                )
            } else {
                ("SELECT COUNT(*) FROM bars WHERE symbol = ?", vec![symbol.clone()])
            };

            let remaining: u64 =
                conn.query_row(check_sql, params_from_iter(check_params.iter()), |row| row.get(0))?;

            if remaining == 0 {
                if let Some(prov) = provider {
                    conn.execute(
                        "DELETE FROM dividends WHERE symbol = ? AND provider = ?",
                        params![symbol, prov.to_string()],
                    )?;
                } else {
                    conn.execute("DELETE FROM dividends WHERE symbol = ?", params![symbol])?;
                }
            }
        }

        conn.execute_batch("COMMIT")?;
        Ok(total_deleted)
    }
}
