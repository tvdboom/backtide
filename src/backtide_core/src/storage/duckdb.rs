//! DuckDB storage solution.

use crate::backtest::models::*;
use crate::constants::Cash;
use crate::data::models::*;
use crate::storage::errors::{StorageError, StorageResult};
use crate::storage::models::*;
use crate::storage::traits::Storage;
use duckdb::params;
use duckdb::params_from_iter;
use duckdb::Connection;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Mutex;

const TABLE_SCHEMAS: &[&str] = &[
    include_str!("../../database/instruments.sql"),
    include_str!("../../database/bars.sql"),
    include_str!("../../database/dividends.sql"),
    include_str!("../../database/experiments.sql"),
    include_str!("../../database/experiment_strategies.sql"),
    include_str!("../../database/experiment_equity.sql"),
    include_str!("../../database/experiment_orders.sql"),
    include_str!("../../database/experiment_trades.sql"),
    include_str!("../../database/live_sessions.sql"),
    include_str!("../../database/live_session_events.sql"),
];

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

    fn begin_transaction(conn: &Connection) -> StorageResult<()> {
        match conn.execute_batch("BEGIN TRANSACTION") {
            Ok(()) => Ok(()),
            Err(e)
                if e.to_string().contains("cannot start a transaction within a transaction")
                    || e.to_string().contains("Current transaction is aborted") =>
            {
                // A previous failing write/appender can leave DuckDB with an open
                // (possibly aborted) transaction on this connection. Clear it and
                // retry so the next operation is not poisoned by the earlier failure.
                let _ = conn.execute_batch("ROLLBACK");
                conn.execute_batch("BEGIN TRANSACTION")?;
                Ok(())
            },
            Err(e) => Err(StorageError::from(e)),
        }
    }

    fn run_transaction<T, F>(&self, f: F) -> StorageResult<T>
    where
        F: FnOnce(&Connection) -> StorageResult<T>,
    {
        let conn = self.conn.lock().unwrap();
        Self::begin_transaction(&conn)?;

        match f(&conn) {
            Ok(value) => match conn.execute_batch("COMMIT") {
                Ok(()) => Ok(value),
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(StorageError::from(e))
                },
            },
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            },
        }
    }
}

impl Storage for DuckDb {
    /// Create every missing table from its canonical schema file.
    fn init(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();

        for schema in TABLE_SCHEMAS {
            conn.execute_batch(schema)?;
        }

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

        // Replace existing rows atomically so an appender failure restores them.
        self.run_transaction(|conn| {
            let pairs: Vec<String> = instruments
                .iter()
                .map(|i| format!("('{}', '{}')", i.symbol.replace('\'', "''"), i.provider))
                .collect();

            conn.execute_batch(&format!(
                "DELETE FROM instruments WHERE (symbol, provider) IN ({})",
                pairs.join(", "),
            ))?;

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
        })
    }

    /// Store multiple series of OHLC data in one bulk operation.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        let non_empty: Vec<&BarSeries> = series.iter().filter(|s| !s.bars.is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(());
        }

        // Delete and replace every overlapping range in one transaction. If
        // appending any row fails, rollback restores all previously stored bars.
        self.run_transaction(|conn| {
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
        })
    }

    /// Store multiple series of dividend events in one bulk operation.
    fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        let non_empty: Vec<&DividendSeries> =
            series.iter().filter(|s| !s.dividends.is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(());
        }

        // Delete and replace dividends atomically. Deduplicate by
        // (symbol, provider, ex_date), keeping the last occurrence.
        self.run_transaction(|conn| {
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
        })
    }

    /// Delete bars (and orphaned dividends/instruments) for one or more series.
    fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64> {
        if series.is_empty() {
            return Ok(0);
        }

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

        self.run_transaction(|conn| {
            let mut total_deleted = 0u64;
            let columns = [
                "symbol",
                "(symbol, interval)",
                "(symbol, provider)",
                "(symbol, interval, provider)",
            ];
            for (col, vals) in columns.iter().zip(&groups) {
                if !vals.is_empty() {
                    let list = vals.iter().join(", ");
                    total_deleted += conn
                        .execute(&format!("DELETE FROM bars WHERE {col} IN ({list})"), [])?
                        as u64;
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

            Ok(total_deleted)
        })
    }

    fn write_experiment(
        &self,
        config: &ExperimentConfig,
        result: &ExperimentResult,
    ) -> StorageResult<()> {
        // Use the inner serializable representation for TOML.
        let inner = ExperimentConfigInner {
            general: config.general.clone(),
            data: config.data.clone(),
            portfolio: config.portfolio.clone(),
            strategy: config.strategy.clone(),
            indicators: config.indicators.clone(),
            metrics: config.metrics.clone(),
            exchange: config.exchange.clone(),
            engine: config.engine.clone(),
        };
        let cfg_toml = toml::to_string_pretty(&inner)
            .map_err(|e| StorageError::Serialization(format!("experiment configuration: {e}")))?;
        let tags_str = result.tags.join(",");

        // ── Phase 1: parent rows + idempotent cleanup of child rows ─────
        self.run_transaction(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO experiments
                 (id, name, icon, tags, description, config_toml, started_at, finished_at, status)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    result.experiment_id,
                    result.name,
                    config.general.icon,
                    tags_str,
                    config.general.description,
                    cfg_toml,
                    result.started_at,
                    result.finished_at,
                    result.status.to_string(),
                ],
            )?;

            for strat in &result.strategies {
                let metrics_str = serde_json::to_string(&strat.metrics).map_err(|e| {
                    StorageError::Serialization(format!(
                        "metrics for strategy run {}: {e}",
                        strat.strategy_id
                    ))
                })?;
                conn.execute(
                    "INSERT OR REPLACE INTO experiment_strategies
                     (id, experiment_id, strategy_id, strategy_name, metrics, base_currency, error, is_benchmark)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        strat.strategy_id,
                        result.experiment_id,
                        strat.strategy_id,
                        strat.strategy_name,
                        metrics_str,
                        strat.base_currency.to_string(),
                        strat.error,
                        strat.is_benchmark,
                    ],
                )?;
            }

            // Bulk-delete child rows for every run_id we are about
            // to repopulate. Building the IN (?, ?, …) list dynamically keeps
            // it to a single round trip per child table.
            if !result.strategies.is_empty() {
                let placeholders = std::iter::repeat_n("?", result.strategies.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                let ids: Vec<&str> =
                    result.strategies.iter().map(|s| s.strategy_id.as_str()).collect();
                for table in ["experiment_equity", "experiment_orders", "experiment_trades"] {
                    let sql = format!("DELETE FROM {table} WHERE run_id IN ({placeholders})");
                    conn.execute(&sql, params_from_iter(ids.iter()))?;
                }
            }

            // Append all child rows before committing the parent transaction.
            // An appender failure therefore restores the previous complete run.
            if result.strategies.iter().any(|s| !s.equity_curve.is_empty()) {
                let mut appender = conn.appender("experiment_equity")?;
                for strat in &result.strategies {
                    for s in &strat.equity_curve {
                        let cash_json = serde_json::to_string(&s.cash).map_err(|e| {
                            StorageError::Serialization(format!(
                                "cash for strategy run {} at {}: {e}",
                                strat.strategy_id, s.timestamp
                            ))
                        })?;
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
                            o.status.to_string(),
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
        })
    }

    fn query_experiments(
        &self,
        experiment_id: Option<&[String]>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredExperiment>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT e.id, e.name, e.icon, e.tags, e.description, e.started_at, e.finished_at, e.status,
                    (SELECT MAX(TRY_CAST(regexp_extract(s.metrics, '\"sharpe\"\\s*:\\s*(-?[0-9.eE+\\-]+)', 1) AS DOUBLE))
                       FROM experiment_strategies s
                      WHERE s.experiment_id = e.id
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
                let tags_str: String = row.get(3)?;
                let tags = if tags_str.is_empty() {
                    Vec::new()
                } else {
                    tags_str.split(',').map(|s| s.to_owned()).collect()
                };
                Ok(StoredExperiment {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    icon: row.get(2)?,
                    tags,
                    description: row.get(4)?,
                    started_at: row.get(5)?,
                    finished_at: row.get(6)?,
                    status: row.get(7)?,
                    best_sharpe: row.get::<_, Option<f64>>(8)?,
                    n_strategies: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn query_strategy_runs(
        &self,
        experiment_id: &str,
        include_equity_curve: bool,
    ) -> StorageResult<Vec<RunResult>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, strategy_id, strategy_name, metrics, base_currency, error, is_benchmark
             FROM experiment_strategies
             WHERE experiment_id = ?
             ORDER BY rowid",
        )?;
        let strats: Vec<(String, String, String, String, Option<String>, Option<String>, bool)> =
            stmt.query_map(params![experiment_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut out = Vec::with_capacity(strats.len());
        for (run_id, strategy_id, name, metrics_str, base_ccy_str, error, is_benchmark) in strats {
            let metrics: HashMap<String, f64> =
                serde_json::from_str(&metrics_str).map_err(|e| {
                    StorageError::CorruptData(format!("metrics for strategy run {run_id}: {e}"))
                })?;
            let base_ccy_str = base_ccy_str.ok_or_else(|| {
                StorageError::CorruptData(format!(
                    "missing base currency for strategy run {run_id}"
                ))
            })?;
            let base_currency: Currency = base_ccy_str.parse().map_err(|e| {
                StorageError::CorruptData(format!("base currency for strategy run {run_id}: {e}"))
            })?;

            let equity_curve = if include_equity_curve {
                let mut eq_stmt = conn.prepare(
                    "SELECT ts, equity, CAST(cash AS VARCHAR), drawdown FROM experiment_equity
                     WHERE run_id = ? ORDER BY ts",
                )?;
                let equity_rows = eq_stmt
                    .query_map(params![run_id], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, f64>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                equity_rows
                    .into_iter()
                    .map(|(timestamp, equity, cash_raw, drawdown)| {
                        // Backward compatible: old rows might contain a scalar,
                        // newer rows store a JSON object keyed by currency code.
                        let cash = if cash_raw.trim_start().starts_with('{') {
                            serde_json::from_str::<Cash>(&cash_raw).map_err(|e| {
                                StorageError::CorruptData(format!(
                                    "cash for strategy run {run_id} at {timestamp}: {e}"
                                ))
                            })?
                        } else {
                            let value = cash_raw.parse::<f64>().map_err(|e| {
                                StorageError::CorruptData(format!(
                                    "cash for strategy run {run_id} at {timestamp}: {e}"
                                ))
                            })?;
                            HashMap::from([(base_currency, value)])
                        };
                        Ok(EquitySample {
                            timestamp,
                            equity,
                            cash,
                            drawdown,
                        })
                    })
                    .collect::<StorageResult<Vec<_>>>()?
            } else {
                Vec::new()
            };

            let mut o_stmt = conn.prepare(
                "SELECT order_id, ts, symbol, order_type, quantity, price, limit_price, status, fill_price, reason, commission, pnl
                 FROM experiment_orders WHERE run_id = ? ORDER BY ts",
            )?;
            type StoredOrderRow = (
                OrderId,
                i64,
                String,
                String,
                f64,
                Option<f64>,
                Option<f64>,
                String,
                Option<f64>,
                String,
                f64,
                Option<f64>,
            );
            let order_rows: Vec<StoredOrderRow> = o_stmt
                .query_map(params![run_id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let orders = order_rows
                .into_iter()
                .map(
                    |(
                        id,
                        timestamp,
                        symbol,
                        order_type,
                        quantity,
                        price,
                        limit_price,
                        status,
                        fill_price,
                        reason,
                        commission,
                        pnl,
                    )| {
                        let parsed_order_type: OrderType = order_type.parse().map_err(|e| {
                            StorageError::CorruptData(format!(
                                "order type {order_type:?} for strategy run {run_id}: {e}"
                            ))
                        })?;
                        let parsed_status: OrderStatus = status.parse().map_err(|e| {
                            StorageError::CorruptData(format!(
                                "order status {status:?} for strategy run {run_id}: {e}"
                            ))
                        })?;
                        Ok(OrderRecord {
                            order: Order {
                                id,
                                symbol,
                                order_type: parsed_order_type,
                                quantity,
                                price,
                                limit_price,
                                sizer: None,
                            },
                            timestamp,
                            status: parsed_status,
                            fill_price,
                            reason,
                            commission,
                            pnl,
                        })
                    },
                )
                .collect::<StorageResult<Vec<_>>>()?;

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
                is_benchmark,
            });
        }

        Ok(out)
    }

    fn write_live_session(&self, session: &StoredLiveSession) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO live_sessions
             (id, status, started_at, finished_at, config, snapshot, health, error)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session.id,
                session.status,
                session.started_at,
                session.finished_at,
                session.config,
                session.snapshot,
                session.health,
                session.error,
            ],
        )?;
        Ok(())
    }

    fn append_live_session_event(&self, session_id: &str, event: &str) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO live_session_events (session_id, kind, event_index, payload)
             SELECT ?, 'event', COALESCE(MAX(event_index), -1) + 1, ?
             FROM live_session_events
             WHERE session_id = ? AND kind = 'event'",
            params![session_id, event, session_id],
        )?;
        Ok(())
    }

    fn write_live_session_warmup(&self, session_id: &str, markets: &[String]) -> StorageResult<()> {
        self.run_transaction(|conn| {
            conn.execute(
                "DELETE FROM live_session_events WHERE session_id = ? AND kind = 'warmup'",
                params![session_id],
            )?;
            let mut stmt = conn.prepare(
                "INSERT INTO live_session_events (session_id, kind, event_index, payload)
                 VALUES (?, 'warmup', ?, ?)",
            )?;
            for (sequence, market) in markets.iter().enumerate() {
                stmt.execute(params![session_id, sequence as i64, market])?;
            }
            Ok(())
        })
    }

    fn query_live_sessions(&self) -> StorageResult<Vec<StoredLiveSession>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, status, started_at, finished_at, config, snapshot, health, error
             FROM live_sessions
             ORDER BY started_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StoredLiveSession {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    started_at: row.get(2)?,
                    finished_at: row.get(3)?,
                    config: row.get(4)?,
                    snapshot: row.get(5)?,
                    health: row.get(6)?,
                    error: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn query_live_session(&self, session_id: &str) -> StorageResult<Option<StoredLiveSession>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, status, started_at, finished_at, config, snapshot, health, error
             FROM live_sessions
             WHERE id = ?",
        )?;
        let mut rows = stmt.query_map(params![session_id], |row| {
            Ok(StoredLiveSession {
                id: row.get(0)?,
                status: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                config: row.get(4)?,
                snapshot: row.get(5)?,
                health: row.get(6)?,
                error: row.get(7)?,
            })
        })?;
        rows.next().transpose().map_err(StorageError::from)
    }

    fn query_live_session_events(&self, session_id: &str) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT payload FROM live_session_events
             WHERE session_id = ? AND kind = 'event'
             ORDER BY event_index",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn query_live_session_warmup(&self, session_id: &str) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT payload FROM live_session_events
             WHERE session_id = ? AND kind = 'warmup'
             ORDER BY event_index",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn delete_live_session(&self, session_id: &str) -> StorageResult<u64> {
        self.run_transaction(|conn| {
            conn.execute(
                "DELETE FROM live_session_events WHERE session_id = ?",
                params![session_id],
            )?;
            let removed =
                conn.execute("DELETE FROM live_sessions WHERE id = ?", params![session_id])?;
            Ok(removed as u64)
        })
    }

    fn delete_experiment(&self, experiment_id: &str) -> StorageResult<u64> {
        self.run_transaction(|conn| {
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

            Ok(removed as u64)
        })
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

    #[test]
    fn live_session_storage_round_trips_and_deletes_related_rows() {
        let (_dir, db) = make_db();
        let session = StoredLiveSession {
            id: "0123456789abcdef".into(),
            status: "running".into(),
            started_at: "2026-08-21T10:00:00+00:00".into(),
            finished_at: None,
            config: r#"{"mode":"live"}"#.into(),
            snapshot: r#"{"equity":1000.0,"metrics":{"custom_score":4.25}}"#.into(),
            health: r#"{"received_events":2}"#.into(),
            error: None,
        };

        db.write_live_session(&session).unwrap();
        db.append_live_session_event(&session.id, r#"{"close":100.0}"#).unwrap();
        db.append_live_session_event(&session.id, r#"{"close":101.0}"#).unwrap();
        db.write_live_session_warmup(
            &session.id,
            &[r#"{"close":98.0}"#.into(), r#"{"close":99.0}"#.into()],
        )
        .unwrap();

        assert_eq!(db.query_live_sessions().unwrap(), vec![session.clone()]);
        assert_eq!(db.query_live_session(&session.id).unwrap(), Some(session.clone()));
        assert_eq!(
            db.query_live_session_events(&session.id).unwrap(),
            vec![r#"{"close":100.0}"#, r#"{"close":101.0}"#]
        );
        assert_eq!(
            db.query_live_session_warmup(&session.id).unwrap(),
            vec![r#"{"close":98.0}"#, r#"{"close":99.0}"#]
        );

        assert_eq!(db.delete_live_session(&session.id).unwrap(), 1);
        assert_eq!(db.query_live_session(&session.id).unwrap(), None);
        assert!(db.query_live_session_events(&session.id).unwrap().is_empty());
        assert!(db.query_live_session_warmup(&session.id).unwrap().is_empty());
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

    fn sample_experiment_config() -> ExperimentConfig {
        ExperimentConfig {
            general: GeneralExpConfig {
                name: "storage regression".to_owned(),
                icon: "".to_string(),
                tags: vec!["custom".to_owned()],
                description: "custom strategy persistence".to_owned(),
            },
            data: DataExpConfig::default(),
            portfolio: PortfolioExpConfig::default(),
            strategy: StrategyExpConfig::default(),
            indicators: IndicatorExpConfig::default(),
            metrics: ExperimentConfigInner::default().metrics,
            exchange: ExchangeExpConfig::default(),
            engine: EngineExpConfig::default(),
        }
    }

    fn sample_experiment_result() -> ExperimentResult {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 9_900.0);

        let mut metrics = HashMap::new();
        metrics.insert("total_return".to_owned(), 0.01);
        metrics.insert("final_equity".to_owned(), 10_100.0);

        ExperimentResult {
            experiment_id: "exp-custom".to_owned(),
            name: "storage regression".to_owned(),
            tags: vec!["custom".to_owned()],
            started_at: 1_700_000_000,
            finished_at: 1_700_000_100,
            status: ExperimentStatus::Success,
            warnings: Vec::new(),
            strategies: vec![RunResult {
                strategy_id: "run-custom".to_owned(),
                strategy_name: "CustomSma".to_owned(),
                equity_curve: vec![EquitySample {
                    timestamp: 1_700_000_000,
                    equity: 10_100.0,
                    cash,
                    drawdown: 0.0,
                }],
                orders: vec![OrderRecord {
                    order: Order {
                        id: OrderId::new(),
                        symbol: "AAPL".to_owned(),
                        order_type: OrderType::Market,
                        quantity: 1.0,
                        price: None,
                        limit_price: None,
                        sizer: None,
                    },
                    timestamp: 1_700_000_000,
                    status: OrderStatus::Filled,
                    fill_price: Some(100.0),
                    reason: String::new(),
                    commission: 0.0,
                    pnl: None,
                }],
                trades: vec![Trade {
                    symbol: "AAPL".to_owned(),
                    quantity: 1.0,
                    entry_ts: 1_700_000_000,
                    exit_ts: 1_700_086_400,
                    entry_price: 100.0,
                    exit_price: 101.0,
                    pnl: 1.0,
                }],
                metrics,
                base_currency: Currency::USD,
                error: None,
                is_benchmark: false,
            }],
        }
    }

    // ── init ──────────────────────────────────────────────────────────────

    #[test]
    fn test_init_creates_all_schema_tables() {
        let (_dir, db) = make_db();

        let conn = db.conn.lock().unwrap();
        for table in [
            "instruments",
            "bars",
            "dividends",
            "experiments",
            "experiment_strategies",
            "experiment_equity",
            "experiment_orders",
            "experiment_trades",
            "live_sessions",
            "live_session_events",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM information_schema.tables
                     WHERE table_schema = 'main' AND table_name = ?",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table: {table}");
        }
    }

    #[test]
    fn test_init_preserves_existing_rows() {
        let (_dir, db) = make_db();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO instruments (symbol, provider, instrument_type)
                 VALUES ('AAPL', 'yahoo', 'stocks')",
                [],
            )
            .unwrap();
        }

        db.init().unwrap();

        let conn = db.conn.lock().unwrap();
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM instruments", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_write_and_query_experiment() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();

        db.write_experiment(&cfg, &result).unwrap();

        let experiments = db
            .query_experiments(Some(std::slice::from_ref(&result.experiment_id)), None, None)
            .unwrap();
        assert_eq!(experiments.len(), 1);
        assert_eq!(experiments[0].id, result.experiment_id);

        let runs = db.query_strategy_runs(&result.experiment_id, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].strategy_name, "CustomSma");
        assert_eq!(runs[0].equity_curve.len(), 1);
        assert_eq!(runs[0].orders.len(), 1);
        assert_eq!(runs[0].trades.len(), 1);
    }

    #[test]
    fn test_query_strategy_runs_can_skip_equity_curve() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        let runs = db.query_strategy_runs(&result.experiment_id, false).unwrap();

        assert_eq!(runs.len(), 1);
        assert!(runs[0].equity_curve.is_empty());
        assert_eq!(runs[0].orders.len(), 1);
        assert_eq!(runs[0].trades.len(), 1);
    }

    #[test]
    fn test_write_experiment_recovers_from_open_transaction() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();

        {
            let conn = db.conn.lock().unwrap();
            conn.execute_batch("BEGIN TRANSACTION").unwrap();
        }

        db.write_experiment(&cfg, &result).unwrap();
        db.write_experiment(&cfg, &result).unwrap();

        let runs = db.query_strategy_runs(&result.experiment_id, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].orders.len(), 1);
    }

    // ── experiment failure handling ───────────────────────────────────────

    #[test]
    fn test_write_experiment_failure_rolls_back_parent_and_children() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let mut original = sample_experiment_result();
        original.name = "Original".to_owned();
        db.write_experiment(&cfg, &original).unwrap();

        let mut replacement = original.clone();
        replacement.name = "Replacement".to_owned();
        let duplicate_order = replacement.strategies[0].orders[0].clone();
        replacement.strategies[0].orders.push(duplicate_order);

        assert!(db.write_experiment(&cfg, &replacement).is_err());

        let experiments = db
            .query_experiments(Some(std::slice::from_ref(&original.experiment_id)), None, None)
            .unwrap();
        assert_eq!(experiments[0].name, "Original");
        let runs = db.query_strategy_runs(&original.experiment_id, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].equity_curve.len(), 1);
        assert_eq!(runs[0].orders.len(), 1);
        assert_eq!(runs[0].trades.len(), 1);
    }

    #[test]
    fn test_query_strategy_runs_rejects_corrupt_order_enums_without_poisoning_storage() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        let run_id = &result.strategies[0].strategy_id;
        db.write_experiment(&cfg, &result).unwrap();

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE experiment_orders SET order_type = 'unknown' WHERE run_id = ?",
                params![run_id],
            )
            .unwrap();
        }
        let error = db.query_strategy_runs(&result.experiment_id, true).unwrap_err();
        assert!(error.to_string().contains("order type"));

        // The failed decode returned normally, so the storage mutex remains usable.
        assert_eq!(db.query_experiments(None, None, None).unwrap().len(), 1);

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE experiment_orders SET order_type = 'market', status = 'unknown' WHERE run_id = ?",
                params![run_id],
            )
            .unwrap();
        }
        let error = db.query_strategy_runs(&result.experiment_id, true).unwrap_err();
        assert!(error.to_string().contains("order status"));
        assert_eq!(db.query_experiments(None, None, None).unwrap().len(), 1);
    }

    #[test]
    fn test_query_strategy_runs_rejects_corrupt_metrics_currency_and_cash() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        let run_id = &result.strategies[0].strategy_id;
        db.write_experiment(&cfg, &result).unwrap();

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE experiment_strategies SET metrics = '{' WHERE id = ?",
                params![run_id],
            )
            .unwrap();
        }
        let error = db.query_strategy_runs(&result.experiment_id, true).unwrap_err();
        assert!(error.to_string().contains("metrics"));

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE experiment_strategies SET metrics = '{}', base_currency = 'invalid' WHERE id = ?",
                params![run_id],
            )
            .unwrap();
        }
        let error = db.query_strategy_runs(&result.experiment_id, true).unwrap_err();
        assert!(error.to_string().contains("base currency"));

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE experiment_strategies SET base_currency = 'USD' WHERE id = ?",
                params![run_id],
            )
            .unwrap();
            conn.execute(
                "UPDATE experiment_equity SET cash = 'invalid' WHERE run_id = ?",
                params![run_id],
            )
            .unwrap();
        }
        let error = db.query_strategy_runs(&result.experiment_id, true).unwrap_err();
        assert!(error.to_string().contains("cash"));
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

    #[test]
    fn test_write_bars_failure_rolls_back_deleted_rows() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }])
        .unwrap();

        let mut duplicate = sample_bar(1_000_000);
        duplicate.close = 999.0;
        let replacement = BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![duplicate, duplicate],
        };

        assert!(db.write_bars_bulk(&[replacement]).is_err());
        let rows = db.query_bars(Some(&["AAPL"]), None, None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].bar.close, 105.0);
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
    fn test_write_instruments_failure_rolls_back_deleted_rows() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();

        let mut replacement = sample_instrument("AAPL");
        replacement.name = "Replacement".to_owned();
        assert!(db.write_instruments(&[replacement.clone(), replacement]).is_err());

        let instruments = db.query_instruments(None, None, None, None).unwrap();
        assert_eq!(instruments.len(), 1);
        assert_eq!(instruments[0].name, "AAPL Inc.");
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
    fn test_write_dividends_failure_rolls_back_deleted_rows() {
        let (_dir, db) = make_db();
        db.write_dividends_bulk(&[DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }])
        .unwrap();

        let duplicate_series = vec![
            DividendSeries {
                symbol: "AAPL".to_owned(),
                provider: Provider::Yahoo,
                dividends: vec![Dividend {
                    ex_date: 1_000_000,
                    amount: 1.0,
                }],
            },
            DividendSeries {
                symbol: "AAPL".to_owned(),
                provider: Provider::Yahoo,
                dividends: vec![Dividend {
                    ex_date: 1_000_000,
                    amount: 2.0,
                }],
            },
        ];
        assert!(db.write_dividends_bulk(&duplicate_series).is_err());

        let rows = db.query_dividends(Some(&["AAPL"]), None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].dividend.amount, 0.82);
    }

    #[test]
    fn test_query_dividends_filter_by_symbol() {
        let (_dir, db) = make_db();
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

    // ── query_experiments search + limit ────────────────────────────────

    #[test]
    fn test_query_experiments_by_search_matches_name() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        // Search by partial match in the name (case-insensitive).
        let rows = db.query_experiments(None, Some("STORAGE"), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_experiments_by_search_matches_tag() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        let rows = db.query_experiments(None, Some("custom"), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_experiments_search_no_match() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        let rows = db.query_experiments(None, Some("nomatch_xyz"), None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_query_experiments_empty_search_string_is_skipped() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        // Empty string -> no filter applied.
        let rows = db.query_experiments(None, Some(""), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_experiments_with_limit() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        for i in 0..3 {
            let mut result = sample_experiment_result();
            result.experiment_id = format!("exp-{i}");
            db.write_experiment(&cfg, &result).unwrap();
        }
        let rows = db.query_experiments(None, None, Some(2)).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_query_experiments_empty_id_filter_is_skipped() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        let empty: &[String] = &[];
        let rows = db.query_experiments(Some(empty), None, None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    // ── delete_experiment ───────────────────────────────────────────────

    #[test]
    fn test_delete_experiment_returns_zero_when_missing() {
        let (_dir, db) = make_db();
        let removed = db.delete_experiment("does-not-exist").unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_delete_experiment_removes_all_rows() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let result = sample_experiment_result();
        db.write_experiment(&cfg, &result).unwrap();

        // Verify the rows exist first.
        assert_eq!(
            db.query_experiments(Some(std::slice::from_ref(&result.experiment_id)), None, None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(db.query_strategy_runs(&result.experiment_id, true).unwrap().len(), 1);

        let removed = db.delete_experiment(&result.experiment_id).unwrap();
        assert_eq!(removed, 1);

        // All cascades cleaned up.
        assert!(db
            .query_experiments(Some(std::slice::from_ref(&result.experiment_id)), None, None)
            .unwrap()
            .is_empty());
        assert!(db.query_strategy_runs(&result.experiment_id, true).unwrap().is_empty());
    }

    // ── query_strategy_runs scalar-cash backward compat ─────────────────

    #[test]
    fn test_query_strategy_runs_empty_for_missing_experiment() {
        let (_dir, db) = make_db();
        let runs = db.query_strategy_runs("none", true).unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn test_query_strategy_runs_handles_error_field() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let mut result = sample_experiment_result();
        // Set an error on the strategy run.
        result.strategies[0].error = Some("strategy crashed".to_owned());
        db.write_experiment(&cfg, &result).unwrap();

        let runs = db.query_strategy_runs(&result.experiment_id, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].error.as_deref(), Some("strategy crashed"));
    }

    #[test]
    fn test_query_strategy_runs_handles_benchmark_flag() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let mut result = sample_experiment_result();
        result.strategies[0].is_benchmark = true;
        db.write_experiment(&cfg, &result).unwrap();

        let runs = db.query_strategy_runs(&result.experiment_id, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert!(runs[0].is_benchmark);
    }

    // ── query_bars_summary covers join + sparkline ──────────────────────

    #[test]
    fn test_query_bars_summary_with_dividends_too() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();
        db.write_bars_bulk(&[BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: (0..5).map(|i| sample_bar(1_000_000 + i * 86400)).collect(),
        }])
        .unwrap();
        db.write_dividends_bulk(&[DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }])
        .unwrap();

        let summaries = db.query_bars_summary().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].n_rows, 5);
        assert_eq!(summaries[0].sparkline.len(), 5);
    }

    // ── query_bar_ranges with multiple groupings ────────────────────────

    #[test]
    fn test_query_bar_ranges_multiple_groups() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_000), sample_bar(2_000)],
            },
            BarSeries {
                symbol: "AAPL".to_owned(),
                interval: Interval::OneHour,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(500)],
            },
        ])
        .unwrap();

        let ranges = db.query_bar_ranges().unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(
            ranges[&("AAPL".to_owned(), "1d".to_owned(), "yahoo".to_owned())],
            (1_000, 2_000)
        );
        assert_eq!(ranges[&("AAPL".to_owned(), "1h".to_owned(), "yahoo".to_owned())], (500, 500));
    }

    // ── query_bars empty filter slices ──────────────────────────────────

    #[test]
    fn test_query_bars_empty_interval_filter() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }])
        .unwrap();
        let empty: &[Interval] = &[];
        let rows = db.query_bars(None, Some(empty), None, None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_bars_empty_provider_filter() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[BarSeries {
            symbol: "AAPL".to_owned(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_000_000)],
        }])
        .unwrap();
        let empty: &[Provider] = &[];
        let rows = db.query_bars(None, None, Some(empty), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    // ── query_dividends empty filter slices ─────────────────────────────

    #[test]
    fn test_query_dividends_empty_symbol_filter() {
        let (_dir, db) = make_db();
        db.write_dividends_bulk(&[DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }])
        .unwrap();
        let empty: &[&str] = &[];
        let rows = db.query_dividends(Some(empty), None, None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_dividends_empty_provider_filter() {
        let (_dir, db) = make_db();
        db.write_dividends_bulk(&[DividendSeries {
            symbol: "AAPL".to_owned(),
            provider: Provider::Yahoo,
            dividends: vec![Dividend {
                ex_date: 1_000_000,
                amount: 0.82,
            }],
        }])
        .unwrap();
        let empty: &[Provider] = &[];
        let rows = db.query_dividends(None, Some(empty), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    // ── query_instruments empty filter slices ───────────────────────────

    #[test]
    fn test_query_instruments_empty_filter_slices() {
        let (_dir, db) = make_db();
        db.write_instruments(&[sample_instrument("AAPL")]).unwrap();
        let empty_it: &[InstrumentType] = &[];
        let empty_pr: &[Provider] = &[];
        let empty_ex: &[Exchange] = &[];
        let rows =
            db.query_instruments(Some(empty_it), Some(empty_pr), Some(empty_ex), None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    // ── delete_symbols with empty filter conditions ─────────────────────

    #[test]
    fn test_delete_symbols_multiple_targets() {
        let (_dir, db) = make_db();
        db.write_bars_bulk(&[
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
        ])
        .unwrap();

        let removed = db
            .delete_symbols(&[("AAPL".to_owned(), None, None), ("MSFT".to_owned(), None, None)])
            .unwrap();
        assert_eq!(removed, 2);
        assert!(db.query_bars(None, None, None, None).unwrap().is_empty());
    }

    // ── write_experiment with status Error + best_sharpe absent ─────────

    #[test]
    fn test_write_experiment_with_error_status() {
        let (_dir, db) = make_db();
        let cfg = sample_experiment_config();
        let mut result = sample_experiment_result();
        result.status = ExperimentStatus::Error;
        result.strategies[0].metrics.clear();
        db.write_experiment(&cfg, &result).unwrap();

        let rows = db
            .query_experiments(Some(std::slice::from_ref(&result.experiment_id)), None, None)
            .unwrap();
        assert_eq!(rows.len(), 1);
        // best_sharpe is None because no sharpe key in metrics.
        assert!(rows[0].best_sharpe.is_none());
    }
}
