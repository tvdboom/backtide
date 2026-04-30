//! A `tracing_subscriber::Layer` that mirrors every event emitted inside
//! an "experiment" span to a dedicated `logs.txt` file attached to that
//! span.
//!
//! Usage from the engine:
//!
//! ```ignore
//! let span = tracing::info_span!(
//!     "experiment",
//!     experiment_id = %experiment_id,
//!     log_path      = %log_path.display(),
//! );
//! let _enter = span.enter();
//! tracing::info!("Anything emitted while this guard is alive lands in the file too.");
//! ```
//!
//! The layer is registered exactly once at process start (see
//! `init_logging_with_level`) so the engine itself does not need any
//! bespoke logging plumbing.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::sync::Mutex;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// The well-known span name used to scope per-experiment logging. Events
/// emitted while a span with this name is on the stack are mirrored into
/// the file referenced by the span's `log_path` field.
pub const EXPERIMENT_SPAN: &str = "experiment";

/// Field name carrying the absolute path of the per-experiment log file.
pub const LOG_PATH_FIELD: &str = "log_path";

/// Per-span extension owning the open log-file handle.
struct ExperimentLogFile(Mutex<File>);

/// Mirrors events occurring inside an [`EXPERIMENT_SPAN`] span to the file
/// referenced by that span's [`LOG_PATH_FIELD`] attribute.
pub struct ExperimentFileLayer;

impl<S> Layer<S> for ExperimentFileLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != EXPERIMENT_SPAN {
            return;
        }
        let mut visitor = LogPathVisitor(None);
        attrs.record(&mut visitor);
        let Some(path) = visitor.0 else {
            return;
        };

        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(ExperimentLogFile(Mutex::new(file)));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Walk the current span scope (closest ancestor first) and write
        // to the first attached log file found. We only ever attach the
        // file to the [`EXPERIMENT_SPAN`] span, so this is at most one
        // extra hash lookup per event when no experiment is active.
        let Some(scope) = ctx.event_scope(event) else {
            return;
        };
        for span in scope.from_root() {
            let ext = span.extensions();
            let Some(log) = ext.get::<ExperimentLogFile>() else {
                continue;
            };

            let mut msg = MessageVisitor(String::new());
            event.record(&mut msg);

            let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
            let meta = event.metadata();
            if let Ok(mut f) = log.0.lock() {
                let _ = writeln!(
                    f,
                    "{ts} {level:<5} {target}: {body}",
                    level = meta.level(),
                    target = meta.target(),
                    body = msg.0,
                );
                let _ = f.flush();
            }
            return;
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Field visitors
// ──────────────────────────────────────────────────────────────────────

/// Picks up the `log_path` field from a span's attributes.
struct LogPathVisitor(Option<String>);

impl Visit for LogPathVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == LOG_PATH_FIELD {
            self.0 = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == LOG_PATH_FIELD {
            // `Display`-formatted paths arrive here under the `%foo` macro
            // syntax. Trim surrounding quotes added by `Debug` for `&str`.
            let s = format!("{value:?}");
            self.0 = Some(s.trim_matches('"').to_owned());
        }
    }
}

/// Renders an event's fields as a single human-readable string.
///
/// `message` (the implicit field used by `info!("...")`) is rendered
/// bare; every other field is appended as `key=value`.
struct MessageVisitor(String);

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={value}", field.name()));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{value:?}"));
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={value:?}", field.name()));
        }
    }
}
