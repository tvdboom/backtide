//! Trait that all market data providers must implement.
//!
//! Adding a new provider (e.g. Bloomberg, Alpaca) only requires implementing
//! this trait — no changes to the Python bindings or callers are needed.

use async_trait::async_trait;
