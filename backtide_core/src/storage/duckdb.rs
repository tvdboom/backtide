//! DuckDB storage solution.

use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::traits::Storage;
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
    fn init(&self) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bars (
                symbol            VARCHAR NOT NULL,
                provider          VARCHAR NOT NULL,
                interval          VARCHAR NOT NULL,
                open_ts           INTEGER NOT NULL,
                close_ts          INTEGER NOT NULL,
                open_ts_exchange  INTEGER NOT NULL,
                open              DOUBLE NOT NULL,
                high              DOUBLE NOT NULL,
                low               DOUBLE NOT NULL,
                close             DOUBLE NOT NULL,
                adj_close         DOUBLE NOT NULL,
                volume            DOUBLE NOT NULL,
                n_trades          INTEGER,
                PRIMARY KEY (symbol, provider, interval)
            );
        ",
        )?;

        Ok(())
    }

    fn write_bars(
        &self,
        symbol: &str,
        provider: Provider,
        interval: Interval,
        bars: &[Bar],
    ) -> StorageResult<()> {
        todo!()
    }
}
