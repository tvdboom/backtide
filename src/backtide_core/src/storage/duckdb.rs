//! DuckDB storage solution.

use crate::backtest::models::experiment_result::{EquitySample, OrderRecord, RunResult, Trade};
use crate::backtest::models::order::Order;
use crate::backtest::models::order_type::OrderType;
use crate::data::models::bar::Bar;
use crate::data::models::currency::Currency;
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
use crate::storage::models::stored_experiment::StoredExperiment;
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

            CREATE TABLE IF NOT EXISTS experiments (
                id                VARCHAR PRIMARY KEY,
                name              VARCHAR NOT NULL,
                tags              VARCHAR NOT NULL,
                description       VARCHAR NOT NULL,
                config_toml       VARCHAR NOT NULL,
                started_at        BIGINT NOT NULL,
                finished_at       BIGINT NOT NULL,
                status            VARCHAR NOT NULL
            );

            CREATE TABLE IF NOT EXISTS experiment_strategies (
                id                VARCHAR NOT NULL,
                experiment_id     VARCHAR NOT NULL,
                strategy_id       VARCHAR NOT NULL,
                strategy_name     VARCHAR NOT NULL,
                metrics           VARCHAR NOT NULL,
                base_currency     VARCHAR,
                error             VARCHAR,
                UNIQUE (experiment_id, strategy_id)
            );

            CREATE TABLE IF NOT EXISTS experiment_equity (
                run_id            VARCHAR NOT NULL,
                ts                BIGINT NOT NULL,
                equity            DOUBLE NOT NULL,
                cash              VARCHAR NOT NULL,
                drawdown          DOUBLE NOT NULL,
                UNIQUE (run_id, ts)
            );

            CREATE TABLE IF NOT EXISTS experiment_orders (
                run_id            VARCHAR NOT NULL,
                order_id          VARCHAR NOT NULL,
                ts                BIGINT NOT NULL,
                symbol            VARCHAR NOT NULL,
                order_type        VARCHAR NOT NULL,
                quantity          BIGINT NOT NULL,
                price             DOUBLE,
                limit_price       DOUBLE,
                status            VARCHAR NOT NULL,
                fill_price        DOUBLE,
                reason            VARCHAR NOT NULL,
                commission        DOUBLE NOT NULL DEFAULT 0,
                pnl               DOUBLE,
                UNIQUE (run_id, order_id)
            );

            CREATE TABLE IF NOT EXISTS experiment_trades (
                run_id            VARCHAR NOT NULL,
                symbol            VARCHAR NOT NULL,
                quantity          BIGINT NOT NULL,
                entry_ts          BIGINT NOT NULL,
                exit_ts           BIGINT NOT NULL,
                entry_price       DOUBLE NOT NULL,
                exit_price        DOUBLE NOT NULL,
                pnl               DOUBLE NOT NULL,
                UNIQUE (run_id, symbol, entry_ts, exit_ts)
            );
        ",
        )?;

        // Migrate older databases where experiment_equity.cash was DOUBLE.
        let _ = conn.execute_batch("ALTER TABLE experiment_equity ALTER COLUMN cash TYPE VARCHAR");

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
        symbols: Option<&[&str]>,
        intervals: Option<&[Interval]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = "SELECT symbol, interval, provider,
                    open_ts, close_ts, open_ts_exchange,
                    open, high, low, close, adj_close, volume, n_trades
             FROM bars"
            .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<String> = Vec::new();

        if let Some(syms) = symbols {
            if !syms.is_empty() {
                let ph: Vec<&str> = syms.iter().map(|_| "?").collect();
                clauses.push(format!("symbol IN ({})", ph.join(", ")));
                params.extend(syms.iter().map(|s| s.to_string()));
            }
        }
        if let Some(ivs) = intervals {
            if !ivs.is_empty() {
                let ph: Vec<&str> = ivs.iter().map(|_| "?").collect();
                clauses.push(format!("interval IN ({})", ph.join(", ")));
                params.extend(ivs.iter().map(|i| i.to_string()));
            }
        }
        if let Some(provs) = providers {
            if !provs.is_empty() {
                let ph: Vec<&str> = provs.iter().map(|_| "?").collect();
                clauses.push(format!("provider IN ({})", ph.join(", ")));
                params.extend(provs.iter().map(|p| p.to_string()));
            }
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
        symbols: Option<&[&str]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = "SELECT symbol, provider, ex_date, amount
             FROM dividends"
            .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<String> = Vec::new();

        if let Some(syms) = symbols {
            if !syms.is_empty() {
                let ph: Vec<&str> = syms.iter().map(|_| "?").collect();
                clauses.push(format!("symbol IN ({})", ph.join(", ")));
                params.extend(syms.iter().map(|s| s.to_string()));
            }
        }
        if let Some(provs) = providers {
            if !provs.is_empty() {
                let ph: Vec<&str> = provs.iter().map(|_| "?").collect();
                clauses.push(format!("provider IN ({})", ph.join(", ")));
                params.extend(provs.iter().map(|p| p.to_string()));
            }
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
        instrument_types: Option<&[InstrumentType]>,
        providers: Option<&[Provider]>,
        exchanges: Option<&[Exchange]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Instrument>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = "SELECT symbol, provider, instrument_type, name, base, quote, exchange
             FROM instruments"
            .to_owned();

        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<String> = Vec::new();

        if let Some(its) = instrument_types {
            if !its.is_empty() {
                let ph: Vec<&str> = its.iter().map(|_| "?").collect();
                clauses.push(format!("instrument_type IN ({})", ph.join(", ")));
                params.extend(its.iter().map(|i| i.to_string()));
            }
        }
        if let Some(provs) = providers {
            if !provs.is_empty() {
                let ph: Vec<&str> = provs.iter().map(|_| "?").collect();
                clauses.push(format!("provider IN ({})", ph.join(", ")));
                params.extend(provs.iter().map(|p| p.to_string()));
            }
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
                let it = it_str.parse::<InstrumentType>().unwrap();
                let prov_str: String = row.get(1)?;
                let prov = prov_str.parse::<Provider>().unwrap();
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
        // Deduplicate by (symbol, provider, ex_date), keeping the last occurrence.
        let mut appender = conn.appender("dividends")?;
        for s in &non_empty {
            let prov = s.provider.to_string();
            let mut seen = HashSet::new();
            for div in s.dividends.iter().rev() {
                if seen.insert(div.ex_date) {
                    appender.append_row(params![
                        &s.symbol,
                        &prov,
                        div.ex_date as i64,
                        div.amount,
                    ])?;
                }
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
        let columns =
            ["symbol", "(symbol, interval)", "(symbol, provider)", "(symbol, interval, provider)"];
        for (col, vals) in columns.iter().zip(&groups) {
            if !vals.is_empty() {
                let list = vals.iter().join(", ");
                total_deleted +=
                    conn.execute(&format!("DELETE FROM bars WHERE {col} IN ({list})"), [])? as u64;
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

    fn write_experiment(
        &self,
        config: &crate::backtest::models::experiment_config::ExperimentConfig,
        result: &crate::backtest::models::experiment_result::ExperimentResult,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();

        // Use the inner serializable representation for TOML.
        let inner = crate::backtest::models::experiment_config::ExperimentConfigInner {
            general: config.general.clone(),
            data: config.data.clone(),
            portfolio: config.portfolio.clone(),
            strategy: config.strategy.clone(),
            indicators: config.indicators.clone(),
            exchange: config.exchange.clone(),
            engine: config.engine.clone(),
        };
        let cfg_toml = toml::to_string_pretty(&inner).unwrap_or_default();
        let tags_str = result.tags.join(",");

        // ── Phase 1: parent rows + idempotent cleanup of child rows ─────
        //
        // Equity curves / orders / trades for any run_id we are
        // about to (re-)insert are wiped in a single statement each, so an
        // experiment with N strategies costs 3 DELETEs total instead of
        // 3·N. Wrapped in a transaction with the parent upserts so the
        // database never observes a half-rewritten experiment.
        conn.execute_batch("BEGIN TRANSACTION")?;

        conn.execute(
            "INSERT OR REPLACE INTO experiments
             (id, name, tags, description, config_toml, started_at, finished_at, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                result.experiment_id,
                result.name,
                tags_str,
                config.general.description,
                cfg_toml,
                result.started_at,
                result.finished_at,
                result.status,
            ],
        )?;

        for strat in &result.strategies {
            let metrics_str = serde_json::to_string(&strat.metrics).unwrap_or_default();
            conn.execute(
                "INSERT OR REPLACE INTO experiment_strategies
                 (id, experiment_id, strategy_id, strategy_name, metrics, base_currency, error)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    strat.strategy_id,
                    result.experiment_id,
                    strat.strategy_id,
                    strat.strategy_name,
                    metrics_str,
                    strat.base_currency.to_string(),
                    strat.error,
                ],
            )?;
        }

        // Bulk-delete child rows for every run_id we are about
        // to repopulate. Building the IN (?, ?, …) list dynamically keeps
        // it to a single round trip per child table.
        if !result.strategies.is_empty() {
            let placeholders =
                std::iter::repeat_n("?", result.strategies.len()).collect::<Vec<_>>().join(", ");
            let ids: Vec<&str> = result.strategies.iter().map(|s| s.strategy_id.as_str()).collect();
            for table in ["experiment_equity", "experiment_orders", "experiment_trades"] {
                let sql = format!("DELETE FROM {table} WHERE run_id IN ({placeholders})");
                conn.execute(&sql, params_from_iter(ids.iter()))?;
            }
        }

        conn.execute_batch("COMMIT")?;

        // ── Phase 2: bulk-append child rows via the DuckDB Appender ─────
        //
        // The Appender batches rows into the column store in one shot —
        // orders of magnitude faster than per-row `INSERT` for the equity
        // curve / orders / trades, which can run into the tens-of-thousands
        // of rows on a long backtest.
        if result.strategies.iter().any(|s| !s.equity_curve.is_empty()) {
            let mut appender = conn.appender("experiment_equity")?;
            for strat in &result.strategies {
                for s in &strat.equity_curve {
                    let cash_json = serde_json::to_string(&s.cash).unwrap_or_else(|_| "{}".into());
                    appender.append_row(params![
                        strat.strategy_id,
                        s.timestamp,
                        s.equity,
                        cash_json,
                        s.drawdown,
                    ])?;
                }
            }
            appender.flush()?;
        }

        if result.strategies.iter().any(|s| !s.orders.is_empty()) {
            let mut appender = conn.appender("experiment_orders")?;
            for strat in &result.strategies {
                for o in &strat.orders {
                    appender.append_row(params![
                        strat.strategy_id,
                        o.order.id,
                        o.timestamp,
                        o.order.symbol,
                        o.order.order_type.to_string(),
                        o.order.quantity,
                        o.order.price,
                        o.order.limit_price,
                        o.status,
                        o.fill_price,
                        o.reason,
                        o.commission,
                        o.pnl,
                    ])?;
                }
            }
            appender.flush()?;
        }

        if result.strategies.iter().any(|s| !s.trades.is_empty()) {
            let mut appender = conn.appender("experiment_trades")?;
            for strat in &result.strategies {
                for t in &strat.trades {
                    appender.append_row(params![
                        strat.strategy_id,
                        t.symbol,
                        t.quantity,
                        t.entry_ts,
                        t.exit_ts,
                        t.entry_price,
                        t.exit_price,
                        t.pnl,
                    ])?;
                }
            }
            appender.flush()?;
        }

        Ok(())
    }

    fn query_experiments(
        &self,
        experiment_id: Option<&[String]>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredExperiment>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT e.id, e.name, e.tags, e.description, e.started_at, e.finished_at, e.status,
                    (SELECT MAX(CAST(json_extract(metrics, '$.sharpe') AS DOUBLE))
                       FROM experiment_strategies s
                      WHERE s.experiment_id = e.id
                        AND NOT regexp_matches(s.strategy_name, '^Benchmark\\s*\\(.+\\)$')
                    ) AS best_sharpe,
                    (SELECT COUNT(*) FROM experiment_strategies s WHERE s.experiment_id = e.id) AS n_strategies
             FROM experiments e",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ids) = experiment_id {
            if !ids.is_empty() {
                let placeholders =
                    std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(", ");
                conditions.push(format!("e.id IN ({placeholders})"));
                params_vec.extend(ids.iter().cloned());
            }
        }
        if let Some(q) = search {
            if !q.is_empty() {
                let pat = format!("%{}%", q.to_lowercase());
                conditions.push("(LOWER(name) LIKE ? OR LOWER(tags) LIKE ?)".to_string());
                params_vec.push(pat.clone());
                params_vec.push(pat);
            }
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY started_at DESC");
        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {l}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(params_vec.iter()), |row| {
                let tags_str: String = row.get(2)?;
                let tags = if tags_str.is_empty() {
                    Vec::new()
                } else {
                    tags_str.split(',').map(|s| s.to_owned()).collect()
                };
                Ok(StoredExperiment {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    tags,
                    description: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    status: row.get(6)?,
                    best_sharpe: row.get::<_, Option<f64>>(7)?,
                    n_strategies: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn query_strategy_runs(&self, experiment_id: &str) -> StorageResult<Vec<RunResult>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, strategy_id, strategy_name, metrics, base_currency, error
             FROM experiment_strategies
             WHERE experiment_id = ?
             ORDER BY rowid",
        )?;
        let strats: Vec<(String, String, String, String, Option<String>, Option<String>)> = stmt
            .query_map(params![experiment_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut out = Vec::with_capacity(strats.len());
        for (run_id, strategy_id, name, metrics_str, base_ccy_str, error) in strats {
            let metrics: HashMap<String, f64> =
                serde_json::from_str(&metrics_str).unwrap_or_default();
            let base_currency =
                base_ccy_str.as_deref().and_then(|s| s.parse().ok()).unwrap_or_default();

            let mut eq_stmt = conn.prepare(
                "SELECT ts, equity, CAST(cash AS VARCHAR), drawdown FROM experiment_equity
                 WHERE run_id = ? ORDER BY ts",
            )?;
            let equity_curve = eq_stmt
                .query_map(params![run_id], |row| {
                    let cash_raw: String = row.get(2)?;
                    // Backward compatible: old rows might contain a scalar,
                    // newer rows store a JSON object keyed by currency code.
                    let cash = if cash_raw.trim_start().starts_with('{') {
                        serde_json::from_str::<HashMap<Currency, f64>>(&cash_raw)
                            .unwrap_or_default()
                    } else {
                        let v = cash_raw.parse::<f64>().unwrap_or(0.0);
                        HashMap::from([(base_currency, v)])
                    };
                    Ok(EquitySample {
                        timestamp: row.get(0)?,
                        equity: row.get(1)?,
                        cash,
                        drawdown: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut o_stmt = conn.prepare(
                "SELECT order_id, ts, symbol, order_type, quantity, price, limit_price, status, fill_price, reason, commission, pnl
                 FROM experiment_orders WHERE run_id = ? ORDER BY ts",
            )?;
            let orders = o_stmt
                .query_map(params![run_id], |row| {
                    let ot: String = row.get(3)?;
                    let order_type: OrderType = ot.parse().unwrap_or_default();
                    Ok(OrderRecord {
                        order: Order {
                            id: row.get(0)?,
                            symbol: row.get(2)?,
                            order_type,
                            quantity: row.get(4)?,
                            price: row.get(5)?,
                            limit_price: row.get(6)?,
                        },
                        timestamp: row.get(1)?,
                        status: row.get(7)?,
                        fill_price: row.get(8)?,
                        reason: row.get(9)?,
                        commission: row.get::<_, Option<f64>>(10)?.unwrap_or(0.0),
                        pnl: row.get(11)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut t_stmt = conn.prepare(
                "SELECT symbol, quantity, entry_ts, exit_ts, entry_price, exit_price, pnl
                 FROM experiment_trades WHERE run_id = ? ORDER BY entry_ts",
            )?;
            let trades = t_stmt
                .query_map(params![run_id], |row| {
                    Ok(Trade {
                        symbol: row.get(0)?,
                        quantity: row.get(1)?,
                        entry_ts: row.get(2)?,
                        exit_ts: row.get(3)?,
                        entry_price: row.get(4)?,
                        exit_price: row.get(5)?,
                        pnl: row.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            out.push(RunResult {
                strategy_id,
                strategy_name: name,
                equity_curve,
                trades,
                orders,
                metrics,
                base_currency,
                error,
            });
        }

        Ok(out)
    }

    fn delete_experiment(&self, experiment_id: &str) -> StorageResult<u64> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN TRANSACTION")?;

        // Delete dependent rows first (no FK cascade in DuckDB).
        conn.execute(
            "DELETE FROM experiment_equity WHERE run_id IN
                (SELECT id FROM experiment_strategies WHERE experiment_id = ?)",
            params![experiment_id],
        )?;
        conn.execute(
            "DELETE FROM experiment_orders WHERE run_id IN
                (SELECT id FROM experiment_strategies WHERE experiment_id = ?)",
            params![experiment_id],
        )?;
        conn.execute(
            "DELETE FROM experiment_trades WHERE run_id IN
                (SELECT id FROM experiment_strategies WHERE experiment_id = ?)",
            params![experiment_id],
        )?;
        conn.execute(
            "DELETE FROM experiment_strategies WHERE experiment_id = ?",
            params![experiment_id],
        )?;
        let removed =
            conn.execute("DELETE FROM experiments WHERE id = ?", params![experiment_id])?;

        conn.execute_batch("COMMIT")?;
        Ok(removed as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_db() -> (TempDir, DuckDb) {
        let dir = TempDir::new().unwrap();
        let db = DuckDb::new(&dir.path().to_path_buf()).unwrap();
        db.init().unwrap();
        (dir, db)
    }

    fn sample_bar(open_ts: u64) -> Bar {
        Bar {
            open_ts,
            close_ts: open_ts + 86400,
            open_ts_exchange: open_ts,
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            adj_close: 105.0,
            volume: 1_000_000.0,
            n_trades: Some(500),
        }
    }

    fn sample_instrument(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.to_owned(),
            name: format!("{symbol} Inc."),
            base: None,
            quote: "USD".to_owned(),
            instrument_type: InstrumentType::Stocks,
            exchange: "XNAS".to_owned(),
            provider: Provider::Yahoo,
        }
    }

    // ── init ──────────────────────────────────────────────────────────────

    #[test]
    fn test_init_creates_tables() {
        let (_dir, db) = make_db();
        // init() already called in make_db; calling again is idempotent
        db.init().unwrap();
    }

    // ── write_bars_bulk / query_bars ──────────────────────────────────────

    #[test]
    fn test_write_and_query_bars() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000), sample_bar(1_086_400)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(None, None, None, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "AAPL");
    }

    #[test]
    fn test_write_bars_empty_series_is_noop() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[]).unwrap();
        let rows = db.query_bars(None, None, None, None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_write_bars_empty_bars_in_series() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![],
        }];
        db.write_bars_bulk(&series).unwrap();
        let rows = db.query_bars(None, None, None, None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_query_bars_filter_by_symbol() {
        let (_dir, db) = make_db();
        let series = vec![
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_000_000)],
            },
            BarSeries {
                symbol: "MSFT".to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_000_000)],
            },
        ];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(Some(&["AAPL"]), None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "AAPL");
    }

    #[test]
    fn test_query_bars_filter_by_interval() {
        let (_dir, db) = make_db();
        let series = vec![
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_000_000)],
            },
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneHour,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(2_000_000)],
            },
        ];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(None, Some(&[Interval::OneHour]), None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].interval, "1h");
    }

    #[test]
    fn test_query_bars_filter_by_provider() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(None, None, Some(&[Provider::Binance]), None).unwrap();
        assert!(rows.is_empty());

        let rows = db.query_bars(None, None, Some(&[Provider::Yahoo]), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_bars_with_limit() {
        let (_dir, db) = make_db();
        let bars: Vec<Bar> = (0..10).map(|i| sample_bar(1_000_000 + i * 86400)).collect();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars,
        }];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(None, None, None, Some(3)).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_write_bars_upsert_overwrites() {
        let (_dir, db) = make_db();
        let series1 = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series1).unwrap();

        // Write again with same ts — should replace
        let mut bar = sample_bar(1_000_000);
        bar.close = 999.0;
        let series2 = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![bar],
        }];
        db.write_bars_bulk(&series2).unwrap();

        let rows = db.query_bars(None, None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].bar.close, 999.0);
    }

    // ── query_bar_ranges ─────────────────────────────────────────────────

    #[test]
    fn test_query_bar_ranges_empty() {
        let (_dir, db) = make_db();
        let ranges = db.query_bar_ranges().unwrap();
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_query_bar_ranges() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000), sample_bar(2_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let ranges = db.query_bar_ranges().unwrap();
        let key = ("AAPL".to_owned(), "1d".to_owned(), "yahoo".to_owned());
        assert_eq!(ranges[&key], (1_000_000, 2_000_000));
    }

    // ── query_bars_summary ───────────────────────────────────────────────

    #[test]
    fn test_query_bars_summary_empty() {
        let (_dir, db) = make_db();
        let summaries = db.query_bars_summary().unwrap();
        assert!(summaries.is_empty());
    }

    #[test]
    fn test_query_bars_summary_with_data() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000), sample_bar(2_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let summaries = db.query_bars_summary().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].symbol, "AAPL");
        assert_eq!(summaries[0].n_rows, 2);
        assert_eq!(summaries[0].first_ts, 1_000_000);
        assert_eq!(summaries[0].last_ts, 2_000_000);
        assert_eq!(summaries[0].sparkline.len(), 2);
    }

    // ── write_instruments / query_instruments ────────────────────────────

    #[test]
    fn test_write_and_query_instruments() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();

        let instruments = db.query_instruments(None, None, None, None).unwrap();
        assert_eq!(instruments.len(), 1);
        assert_eq!(instruments[0].symbol, "AAPL");
        assert_eq!(instruments[0].name, "AAPL Inc.");
    }

    #[test]
    fn test_write_instruments_empty_is_noop() {
        let (_dir, db) = make_db();
        db.write_instruments(&[]).unwrap();
        let instruments = db.query_instruments(None, None, None, None).unwrap();
        assert!(instruments.is_empty());
    }

    #[test]
    fn test_write_instruments_upserts() {
        let (_dir, db) = make_db();
        let mut inst = sample_instrument("AAPL");
        db.write_instruments(&[inst.clone()]).unwrap();

        inst.name = "Apple Updated".to_owned();
        db.write_instruments(&[inst]).unwrap();

        let instruments = db.query_instruments(None, None, None, None).unwrap();
        assert_eq!(instruments.len(), 1);
        assert_eq!(instruments[0].name, "Apple Updated");
    }

    #[test]
    fn test_query_instruments_filter_by_type() {
        let (_dir, db) = make_db();
        let mut crypto = sample_instrument("BTC-USD");
        crypto.instrument_type = InstrumentType::Crypto;
        db.write_instruments(&[sample_instrument("AAPL"), crypto]).unwrap();

        let instruments =
            db.query_instruments(Some(&[InstrumentType::Crypto]), None, None, None).unwrap();
        assert_eq!(instruments.len(), 1);
        assert_eq!(instruments[0].symbol, "BTC-USD");
    }

    #[test]
    fn test_query_instruments_filter_by_provider() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();

        let instruments =
            db.query_instruments(None, Some(&[Provider::Binance]), None, None).unwrap();
        assert!(instruments.is_empty());
    }

    #[test]
    fn test_query_instruments_with_limit() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL"), sample_instrument("MSFT")]).unwrap();

        let instruments = db.query_instruments(None, None, None, Some(1)).unwrap();
        assert_eq!(instruments.len(), 1);
    }

    // ── write_dividends_bulk / query_dividends ───────────────────────────

    #[test]
    fn test_write_and_query_dividends() {
        let (_dir, db) = make_db();
        use crate::data::models::dividend::Dividend;
        let series = vec![DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![
                Dividend {
                    ex_date: 1_000_000,
                    amount: 0.82,
                },
                Dividend {
                    ex_date: 2_000_000,
                    amount: 0.85,
                },
            ],
        }];
        db.write_dividends_bulk(&series).unwrap();

        let rows = db.query_dividends(None, None, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "AAPL");
    }

    #[test]
    fn test_write_dividends_empty_series_is_noop() {
        let (_dir, db) = make_db();
        db.write_dividends_bulk(&[]).unwrap();
        let rows = db.query_dividends(None, None, None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_write_dividends_empty_dividends_in_series() {
        let (_dir, db) = make_db();
        let series = vec![DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![],
        }];
        db.write_dividends_bulk(&series).unwrap();
        let rows = db.query_dividends(None, None, None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_query_dividends_filter_by_symbol() {
        let (_dir, db) = make_db();
        use crate::data::models::dividend::Dividend;
        let series = vec![
            DividendSeries {
                symbol: "AAPL".to_owned(),
                provider: Provider::Yahoo,
                dividends: vec![Dividend {
                    ex_date: 1_000_000,
                    amount: 0.82,
                }],
            },
            DividendSeries {
                symbol: "MSFT".to_owned(),
                provider: Provider::Yahoo,
                dividends: vec![Dividend {
                    ex_date: 1_000_000,
                    amount: 1.50,
                }],
            },
        ];
        db.write_dividends_bulk(&series).unwrap();

        let rows = db.query_dividends(Some(&["AAPL"]), None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "AAPL");
    }

    #[test]
    fn test_query_dividends_filter_by_provider() {
        let (_dir, db) = make_db();
        use crate::data::models::dividend::Dividend;
        let series = vec![DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }];
        db.write_dividends_bulk(&series).unwrap();

        let rows = db.query_dividends(None, Some(&[Provider::Binance]), None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_query_dividends_with_limit() {
        let (_dir, db) = make_db();
        use crate::data::models::dividend::Dividend;
        let divs: Vec<Dividend> = (0..5)
            .map(|i| Dividend {
                ex_date: 1_000_000 + i * 86400,
                amount: 0.5,
            })
            .collect();
        let series = vec![DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: divs,
        }];
        db.write_dividends_bulk(&series).unwrap();

        let rows = db.query_dividends(None, None, Some(2)).unwrap();
        assert_eq!(rows.len(), 2);
    }

    // ── delete_symbols ───────────────────────────────────────────────────

    #[test]
    fn test_delete_symbols_empty_is_noop() {
        let (_dir, db) = make_db();
        let deleted = db.delete_symbols(&[]).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_delete_symbols_by_symbol_only() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();

        let deleted = db.delete_symbols(&[("AAPL".to_owned(), None, None)]).unwrap();
        assert_eq!(deleted, 1);

        let rows = db.query_bars(None, None, None, None).unwrap();
        assert!(rows.is_empty());
        // Orphaned instrument should be cleaned up
        let instruments = db.query_instruments(None, None, None, None).unwrap();
        assert!(instruments.is_empty());
    }

    #[test]
    fn test_delete_symbols_by_symbol_and_interval() {
        let (_dir, db) = make_db();
        let series = vec![
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_000_000)],
            },
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneHour,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(2_000_000)],
            },
        ];
        db.write_bars_bulk(&series).unwrap();

        let deleted =
            db.delete_symbols(&[("AAPL".to_owned(), Some(Interval::OneDay), None)]).unwrap();
        assert_eq!(deleted, 1);

        let rows = db.query_bars(None, None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].interval, "1h");
    }

    #[test]
    fn test_delete_symbols_by_symbol_and_provider() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let deleted =
            db.delete_symbols(&[("AAPL".to_owned(), None, Some(Provider::Yahoo))]).unwrap();
        assert_eq!(deleted, 1);
    }

    #[test]
    fn test_delete_symbols_by_symbol_interval_provider() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        let deleted = db
            .delete_symbols(&[("AAPL".to_owned(), Some(Interval::OneDay), Some(Provider::Yahoo))])
            .unwrap();
        assert_eq!(deleted, 1);
    }

    #[test]
    fn test_delete_symbols_cleans_orphaned_dividends() {
        let (_dir, db) = make_db();
        use crate::data::models::dividend::Dividend;

        let bar_series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&bar_series).unwrap();

        let div_series = vec![DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }];
        db.write_dividends_bulk(&div_series).unwrap();

        db.delete_symbols(&[("AAPL".to_owned(), None, None)]).unwrap();

        let divs = db.query_dividends(None, None, None).unwrap();
        assert!(divs.is_empty());
    }

    // ── query_bars with empty filter arrays ──────────────────────────────

    #[test]
    fn test_query_bars_empty_symbol_filter() {
        let (_dir, db) = make_db();
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }];
        db.write_bars_bulk(&series).unwrap();

        // Empty slice means no filter applied
        let empty: &[&str] = &[];
        let rows = db.query_bars(Some(empty), None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    // ── n_trades None ────────────────────────────────────────────────────

    #[test]
    fn test_bar_with_no_trades() {
        let (_dir, db) = make_db();
        let mut bar = sample_bar(1_000_000);
        bar.n_trades = None;
        let series = vec![BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![bar],
        }];
        db.write_bars_bulk(&series).unwrap();

        let rows = db.query_bars(None, None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].bar.n_trades.is_none());
    }

    // ── query_instruments with exchange filter ───────────────────────────

    #[test]
    fn test_query_instruments_filter_by_exchange() {
        let (_dir, db) = make_db();
        let mut inst = sample_instrument("AAPL");
        inst.exchange = "XNAS".to_owned();
        db.write_instruments(&[inst]).unwrap();

        let instruments = db.query_instruments(None, None, Some(&[Exchange::XNAS]), None).unwrap();
        assert_eq!(instruments.len(), 1);

        let instruments = db.query_instruments(None, None, Some(&[Exchange::XNYS]), None).unwrap();
        assert!(instruments.is_empty());
    }
}
