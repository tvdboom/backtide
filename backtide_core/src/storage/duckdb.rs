//! DuckDB storage solution.

use crate::data::models::bar::Bar;
use crate::data::models::dividend::Dividend;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
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
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
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

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS instruments (
                symbol            VARCHAR NOT NULL,
                provider          VARCHAR NOT NULL,
                instrument_type   VARCHAR NOT NULL,
                name              VARCHAR,
                base              VARCHAR,
                quote             VARCHAR,
                exchange          VARCHAR,
                UNIQUE (symbol, provider)
            );

            CREATE TABLE IF NOT EXISTS bars (
                symbol            VARCHAR NOT NULL,
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
    fn query_bar_ranges(&self) -> StorageResult<HashMap<(String, String, String), (u64, u64)>> {
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

    /// Return a pre-aggregated summary of stored bars, enriched with instrument metadata.
    fn query_bars_summary(&self) -> StorageResult<Vec<BarSummary>> {
        let conn = self.conn.lock().unwrap();

        // Phase 1: Grouped summary with a LEFT JOIN to instruments for metadata.
        let mut stmt = conn.prepare(
            "SELECT b.symbol,
                    COALESCE(i.instrument_type, '') AS instrument_type,
                    b.interval,
                    b.provider,
                    i.name,
                    i.base,
                    i.quote,
                    i.exchange,
                    MIN(b.open_ts) AS first_ts,
                    MAX(b.open_ts) AS last_ts,
                    COUNT(*)       AS n_rows
             FROM bars b
             LEFT JOIN instruments i
                    ON b.symbol = i.symbol AND b.provider = i.provider
             GROUP BY b.symbol, i.instrument_type, b.interval, b.provider,
                      i.name, i.base, i.quote, i.exchange
             ORDER BY b.symbol, b.interval",
        )?;

        let mut summaries: Vec<BarSummary> = stmt
            .query_map([], |row| {
                Ok(BarSummary {
                    symbol: row.get(0)?,
                    instrument_type: row.get(1)?,
                    interval: row.get(2)?,
                    provider: row.get(3)?,
                    name: row.get(4)?,
                    base: row.get(5)?,
                    quote: row.get(6)?,
                    exchange: row.get(7)?,
                    first_ts: row.get(8)?,
                    last_ts: row.get(9)?,
                    n_rows: row.get(10)?,
                    sparkline: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Phase 2: Fetch the last 365 adj_close values per group for sparklines.
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

        let mut sparkline_map: HashMap<(String, String, String), Vec<f64>> = HashMap::new();
        let mut spark_rows = spark_stmt.query([])?;
        while let Some(row) = spark_rows.next()? {
            let key: (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            let val: f64 = row.get(3)?;
            sparkline_map.entry(key).or_default().push(val);
        }

        for s in &mut summaries {
            let key = (s.symbol.clone(), s.interval.clone(), s.provider.clone());
            if let Some(spark) = sparkline_map.remove(&key) {
                s.sparkline = spark;
            }
        }

        Ok(summaries)
    }

    /// Return stored bars, optionally filtered by symbol/interval/provider with a limit.
    fn query_bars(
        &self,
        symbol: Option<&str>,
        interval: Option<Interval>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>> {
        let conn = self.conn.lock().unwrap();

        let mut sql =
            "SELECT symbol, interval, provider,
                    open_ts, close_ts, open_ts_exchange,
                    open, high, low, close, adj_close, volume, n_trades
             FROM bars"
                .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<&str> = Vec::new();

        if let Some(symbol) = symbol {
            clauses.push("symbol = ?");
            params.push(symbol.to_owned());
        }
        if let Some(interval) = interval {
            clauses.push("interval = ?");
            params.push(interval.to_string());
        }
        if let Some(provider) = provider {
            clauses.push("provider = ?");
            params.push(provider.to_string());
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY symbol, interval, open_ts");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(StoredBar {
                    symbol: row.get(0)?,
                    interval: row.get(1)?,
                    provider: row.get(2)?,
                    bar: Bar {
                        open_ts: row.get(3)?,
                        close_ts: row.get(4)?,
                        open_ts_exchange: row.get(5)?,
                        open: row.get(6)?,
                        high: row.get(7)?,
                        low: row.get(8)?,
                        close: row.get(9)?,
                        adj_close: row.get(10)?,
                        volume: row.get(11)?,
                        n_trades: row.get(12)?,
                    },
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Return stored dividends, optionally filtered by symbol/provider with a limit.
    fn query_dividends(
        &self,
        symbol: Option<&str>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>> {
        let conn = self.conn.lock().unwrap();

        let mut sql =
            "SELECT symbol, provider, ex_date, amount
             FROM dividends"
                .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<&str> = Vec::new();

        if let Some(s) = symbol {
            clauses.push("symbol = ?");
            params.push(s.to_owned());
        }
        if let Some(prov) = provider {
            clauses.push("provider = ?");
            params.push(prov.to_string());
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY symbol, ex_date");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
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

    /// Return stored instrument metadata, optionally filtered by type/provider/exchanges with a limit.
    fn query_instruments(
        &self,
        instrument_type: Option<InstrumentType>,
        provider: Option<Provider>,
        exchanges: Option<&[Exchange]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Instrument>> {
        let conn = self.conn.lock().unwrap();

        let mut sql =
            "SELECT symbol, provider, instrument_type, name, base, quote, exchange
             FROM instruments"
                .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<String> = Vec::new();

        if let Some(instrument_type) = instrument_type {
            clauses.push("instrument_type = ?".to_owned());
            params.push(instrument_type.to_string());
        }
        if let Some(provider) = provider {
            clauses.push("provider = ?".to_owned());
            params.push(provider.to_string());
        }
        if let Some(exs) = exchanges {
            if !exs.is_empty() {
                let placeholders: Vec<&str> = exs.iter().map(|_| "?").collect();
                clauses.push(format!("exchange IN ({})", placeholders.join(", ")));
                for ex in exs {
                    params.push(ex.to_string());
                }
            }
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY symbol");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                let it_str: String = row.get(2)?;
                let it = instrument_type.unwrap_or_else(|| {
                    it_str.parse::<InstrumentType>().unwrap()
                });
                let prov = provider.unwrap_or_else(|| {
                    let s: String = row.get(1).unwrap();
                    s.parse::<Provider>().unwrap()
                });
                Ok(Instrument {
                    symbol: row.get(0)?,
                    name: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    base: row.get(4)?,
                    quote: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    instrument_type: it,
                    exchange: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    provider: prov,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Upsert instrument metadata rows.
    fn write_instruments(&self, instruments: &[Instrument]) -> StorageResult<()> {
        if instruments.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Phase 1: Bulk-delete existing rows for the incoming pairs.
        let pairs: Vec<String> = instruments
            .iter()
            .map(|i| format!("('{}', '{}')", i.symbol.replace('\'', "''"), i.provider))
            .collect();

        conn.execute_batch(&format!(
            "DELETE FROM instruments WHERE (symbol, provider) IN ({})",
            pairs.join(", "),
        ))?;

        // Phase 2: bulk-insert via the Appender.
        let mut appender = conn.appender("instruments")?;
        for inst in instruments {
            appender.append_row(params![
                &inst.symbol,
                &inst.provider.to_string(),
                &inst.instrument_type.to_string(),
                &Some(&inst.name),
                &inst.base,
                &Some(&inst.quote),
                &Some(&inst.exchange),
            ])?;
        }
        appender.flush()?;

        Ok(())
    }

    /// Store multiple series of OHLC data in one bulk operation.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        let non_empty: Vec<&BarSeries> = series.iter().filter(|s| !s.bars.is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Phase 1: Delete all overlapping ranges in a single transaction.
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

        // Phase 2: Bulk-insert every row via the Appender (one flush).
        let mut appender = conn.appender("bars")?;
        for s in &non_empty {
            let iv = s.interval.to_string();
            let prov = s.provider.to_string();
            for bar in &s.bars {
                appender.append_row(params![
                    &s.symbol,
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
    fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        let non_empty: Vec<&DividendSeries> =
            series.iter().filter(|s| !s.dividends.is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Phase 1: Delete overlapping ranges.
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

        // Phase 2: Bulk-insert every row via the Appender.
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

    /// Delete bars (and orphaned dividends/instruments) for one or more series.
    fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64> {
        if series.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN TRANSACTION")?;

        // Phase 1: Bulk-delete bars, grouped by filter signature.
        let mut groups: [Vec<String>; 4] = Default::default();
        for (symbol, interval, provider) in series {
            let s = symbol.replace('\'', "''");
            match (interval, provider) {
                (None, None) => groups[0].push(format!("'{s}'")),
                (Some(iv), None) => groups[1].push(format!("('{s}', '{iv}')")),
                (None, Some(p)) => groups[2].push(format!("('{s}', '{p}')")),
                (Some(iv), Some(p)) => groups[3].push(format!("('{s}', '{iv}', '{p}')")),
            }
        }

        let mut total_deleted = 0u64;
        let columns = ["symbol", "(symbol, interval)", "(symbol, provider)", "(symbol, interval, provider)"];
        for (col, vals) in columns.iter().zip(&groups) {
            if !vals.is_empty() {
                let list = vals.iter().join(", ");
                total_deleted += conn.execute(
                    &format!("DELETE FROM bars WHERE {col} IN ({list})"),
                    [],
                )? as u64;
            }
        }

        // Phase 2: bulk-cleanup orphaned dividends and instruments.
        // Group by filter: symbol-only vs (symbol, provider).
        let mut orphans: [HashSet<String>; 2] = Default::default();
        for (symbol, _, provider) in series {
            let s = symbol.replace('\'', "''");
            match provider {
                None => orphans[0].insert(format!("'{s}'")),
                Some(p) => orphans[1].insert(format!("('{s}', '{p}')")),
            };
        }

        let orphan_cols = ["symbol", "(symbol, provider)"];
        let orphan_excl = [
            "symbol NOT IN (SELECT DISTINCT symbol FROM bars)",
            "(symbol, provider) NOT IN (SELECT DISTINCT symbol, provider FROM bars)",
        ];
        for ((col, excl), vals) in orphan_cols.iter().zip(&orphan_excl).zip(&orphans) {
            if !vals.is_empty() {
                let list = vals.iter().join(", ");
                conn.execute_batch(&format!(
                    "DELETE FROM dividends WHERE {col} IN ({list}) AND {excl};
                     DELETE FROM instruments WHERE {col} IN ({list}) AND {excl};"
                ))?;
            }
        }

        conn.execute_batch("COMMIT")?;
        Ok(total_deleted)
    }
}
