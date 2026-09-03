//! A `tracing_subscriber::Layer` that mirrors every event emitted inside
//! an "experiment" span to a dedicated `logs.txt` file attached to that
//! span. The layer is registered exactly once at process start (see
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

/// The span name used to scope per-experiment logging. Events emitted while a
/// span with this name is on the stack are mirrored into the file referenced
/// by the span's `log_path` field.
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

            let now = chrono::Utc::now();
            let tz = crate::config::interface::Config::get()
                .ok()
                .and_then(|c| c.display.timezone.as_deref())
                .and_then(|s| s.trim().parse::<chrono_tz::Tz>().ok());
            let ts = match tz {
                Some(tz) => now.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string(),
                None => {
                    now.with_timezone(&chrono::Local).format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
                },
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn experiment_layer_ignores_unscoped_and_invalid_spans_and_writes_scoped_events() {
        let temp = tempdir().unwrap();
        let log_path = temp.path().join("nested").join("logs.txt");
        let blocked = temp.path().join("blocked");
        std::fs::write(&blocked, "file").unwrap();
        let invalid_path = blocked.join("logs.txt");
        let log_path_string = log_path.to_string_lossy().into_owned();
        let subscriber = tracing_subscriber::registry().with(ExperimentFileLayer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(message = "outside");

            let wrong = tracing::info_span!("not_experiment", log_path = %log_path.display());
            let _wrong_guard = wrong.enter();
            tracing::info!(message = "wrong span");
            drop(_wrong_guard);

            let no_path = tracing::info_span!("experiment", unrelated = true);
            let _no_path_guard = no_path.enter();
            tracing::info!(message = "missing path");
            drop(_no_path_guard);

            let invalid = tracing::info_span!("experiment", log_path = %invalid_path.display());
            let _invalid_guard = invalid.enter();
            tracing::info!(message = "invalid path");
            drop(_invalid_guard);

            let span = tracing::info_span!("experiment", log_path = log_path_string.as_str());
            let _guard = span.enter();
            tracing::info!(message = "completed", symbol = "AAPL", count = 2);
            tracing::info!(message = ?"debug message", result = ?Some(3));
        });

        let text = std::fs::read_to_string(log_path).unwrap();
        assert!(text.contains("completed"));
        assert!(text.contains("symbol=AAPL"));
        assert!(text.contains("count=2"));
        assert!(text.contains("debug message"));
        assert!(!text.contains("outside"));
    }
}
