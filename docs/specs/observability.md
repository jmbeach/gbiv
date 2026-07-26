# Observability (Tracing Initialization)

Specs for the shared tracing initialization helper in `core::observability`,
installed once at process start by every gbiv entry point.

**Component LLD**: `docs/llds/observability.md`

## Initialization

- [x] LOG-001: When a gbiv entry point starts, it shall call `core::observability::init(default_level)` once, before command dispatch or any other work, to install the process-global `tracing` subscriber.
- [x] LOG-002: When `RUST_LOG` holds a non-empty value, `init` shall use that value verbatim as the `EnvFilter` directive, overriding `default_level`.
- [x] LOG-003: When `RUST_LOG` is unset, empty, or whitespace-only, `init` shall use `default_level` as the filter directive.
- [x] LOG-004: When `RUST_LOG` holds a directive that fails to parse as an `EnvFilter`, `init` shall fall back to `default_level` rather than failing to start.
- [x] LOG-005: When `init` is called and a global subscriber is already installed, it shall treat the installation as a no-op rather than panicking (best-effort idempotent via `try_init`).

## Subscriber Shape

- [x] LOG-006: The installed subscriber shall write all log output to stderr, leaving stdout for command results.
- [x] LOG-007: The installed subscriber shall format timestamps as UTC ISO-8601 (RFC 3339) and shall include the emitting module path (target) in each line.

## Verbosity Query

- [x] LOG-008: When `debug_enabled()` is called, it shall return `true` if and only if the installed subscriber's effective maximum level is `DEBUG` or more verbose (`TRACE`).
