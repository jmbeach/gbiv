//! Shared tracing initialization for every gbiv entry point.
//!
//! See `docs/llds/observability.md`. The init helper lives here in `core` so the
//! worktree binary, the `gbiv start` daemon, and the fleet client subcommands all
//! install the same subscriber the same way and cannot drift on format or filter.

use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Install the process-global tracing subscriber. Best-effort idempotent: a
/// second call is a no-op rather than a panic.
// @spec LOG-001, LOG-005, LOG-006, LOG-007
pub fn init(default_level: LevelFilter) {
    let rust_log = std::env::var("RUST_LOG").ok();
    let _ = build_subscriber(default_level, rust_log.as_deref(), std::io::stderr).try_init();
}

/// Whether the installed subscriber's effective maximum level is `DEBUG` or more
/// verbose. Callers widen their output (e.g. full error chains) when this is true.
// @spec LOG-008
pub fn debug_enabled() -> bool {
    level_at_least_debug(LevelFilter::current())
}

/// Pure threshold used by `debug_enabled`, factored out so it is testable without
/// touching global subscriber state.
fn level_at_least_debug(level: LevelFilter) -> bool {
    level >= LevelFilter::DEBUG
}

/// Pick the `EnvFilter` directive string: a non-empty `RUST_LOG` wins, otherwise
/// the caller's default level. Empty/whitespace `RUST_LOG` is treated as unset.
fn filter_directive(default_level: &str, rust_log: Option<&str>) -> String {
    match rust_log {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => default_level.to_string(),
    }
}

/// Build the `EnvFilter` from the resolved directive, falling back to
/// `default_level` when the directive fails to parse.
fn build_env_filter(default_level: LevelFilter, rust_log: Option<&str>) -> EnvFilter {
    let directive = filter_directive(&default_level.to_string(), rust_log);
    EnvFilter::try_new(directive).unwrap_or_else(|_| EnvFilter::new(default_level.to_string()))
}

/// Construct (but do not install) the fmt subscriber. `init` calls this with
/// stderr; tests call it with an in-memory writer to assert the rendered format.
/// Keeping one builder means init and tests cannot drift on format config.
fn build_subscriber<W>(
    default_level: LevelFilter,
    rust_log: Option<&str>,
    writer: W,
) -> impl tracing::Subscriber
where
    W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_env_filter(build_env_filter(default_level, rust_log))
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        // Plain text: keep redirected/captured logs clean and greppable.
        .with_ansi(false)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing::{debug, info};

    /// In-memory `MakeWriter` so a test can capture and assert rendered log lines.
    #[derive(Clone)]
    struct VecWriter(Arc<Mutex<Vec<u8>>>);

    impl VecWriter {
        fn new() -> Self {
            VecWriter(Arc::new(Mutex::new(Vec::new())))
        }
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl io::Write for VecWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for VecWriter {
        type Writer = VecWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    // @spec LOG-002
    #[test]
    fn filter_directive_uses_non_empty_rust_log() {
        assert_eq!(
            filter_directive("info", Some("gbiv=debug,tiny_http=warn")),
            "gbiv=debug,tiny_http=warn"
        );
    }

    // @spec LOG-003
    #[test]
    fn filter_directive_falls_back_to_default() {
        assert_eq!(filter_directive("info", None), "info");
        assert_eq!(filter_directive("info", Some("")), "info");
        assert_eq!(filter_directive("info", Some("   ")), "info");
    }

    // @spec LOG-004
    #[test]
    fn malformed_rust_log_falls_back_to_default_level() {
        let buf = VecWriter::new();
        // "foo=notalevel" fails to parse (invalid level), forcing the fallback.
        let sub = build_subscriber(LevelFilter::INFO, Some("foo=notalevel"), buf.clone());
        tracing::subscriber::with_default(sub, || {
            debug!("dbg-line");
            info!("inf-line");
        });
        let out = buf.contents();
        // Fell back to the INFO default: info passes, debug is filtered out.
        assert!(out.contains("inf-line"), "info should pass; got: {out:?}");
        assert!(!out.contains("dbg-line"), "debug should be filtered; got: {out:?}");
    }

    // @spec LOG-008
    #[test]
    fn debug_enabled_threshold_is_debug_or_more_verbose() {
        assert!(!level_at_least_debug(LevelFilter::OFF));
        assert!(!level_at_least_debug(LevelFilter::ERROR));
        assert!(!level_at_least_debug(LevelFilter::WARN));
        assert!(!level_at_least_debug(LevelFilter::INFO));
        assert!(level_at_least_debug(LevelFilter::DEBUG));
        assert!(level_at_least_debug(LevelFilter::TRACE));
    }

    // @spec LOG-006, LOG-007
    #[test]
    fn rendered_line_is_utc_iso8601_with_target() {
        let buf = VecWriter::new();
        let sub = build_subscriber(LevelFilter::INFO, None, buf.clone());
        tracing::subscriber::with_default(sub, || {
            info!(target: "gbiv::sample", "hello");
        });
        let out = buf.contents();
        // Output landed in the configured writer (LOG-006).
        assert!(out.contains("hello"), "message missing; got: {out:?}");
        // Target/module path is shown (LOG-007).
        assert!(out.contains("gbiv::sample"), "target missing; got: {out:?}");
        // Timestamp is UTC ISO-8601 (RFC 3339): YYYY-MM-DDThh:mm:ss...Z/+00:00 (LOG-007).
        let first = out.lines().next().unwrap_or_default();
        let date = first.get(0..10).unwrap_or_default();
        assert!(
            date.len() == 10
                && date.as_bytes()[4] == b'-'
                && date.as_bytes()[7] == b'-'
                && date[0..4].chars().all(|c| c.is_ascii_digit()),
            "expected ISO-8601 date prefix; got: {first:?}"
        );
        assert!(first.contains('T'), "expected 'T' separator; got: {first:?}");
        assert!(
            first.contains('Z') || first.contains("+00:00"),
            "expected UTC marker; got: {first:?}"
        );
    }

    // @spec LOG-005
    #[test]
    #[serial]
    fn init_is_idempotent() {
        // Two installs in one process must not panic; the first wins.
        init(LevelFilter::INFO);
        init(LevelFilter::DEBUG);
    }
}
