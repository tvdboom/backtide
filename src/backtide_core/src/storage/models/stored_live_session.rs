//! Persisted paper-trading or replay session metadata.

/// One live-session manifest stored in DuckDB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLiveSession {
    /// Stable 16-character hexadecimal session identifier.
    pub id: String,

    /// Current terminal or active session status.
    pub status: String,

    /// UTC ISO-8601 timestamp at which the session started.
    pub started_at: String,

    /// UTC ISO-8601 timestamp at which the session finished, when terminal.
    pub finished_at: Option<String>,

    /// JSON-encoded session configuration.
    pub config: String,

    /// JSON-encoded latest account snapshot.
    pub snapshot: String,

    /// JSON-encoded connection and replay health state.
    pub health: String,

    /// Terminal error message, when the session failed.
    pub error: Option<String>,
}
