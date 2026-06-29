# Observability (Tracing Initialization)

**Created**: 2026-06-27
**Status**: Draft

## Context and Design Philosophy

gbiv emits structured, levelled logs through the `tracing` framework. This LLD
scopes one thing: the **shared initialization helper** that every gbiv entry
point calls once at process start to install the global tracing subscriber.

The helper lives in the `core` module (`gbiv-core`) so the worktree binary, the
`gbiv start` daemon, and the fleet client subcommands all install the *same*
subscriber the *same* way and cannot drift on log format or filter behavior. This
is the home for the init contract. The broader logging config surface — the
`-v`/`-vv` verbosity flags, the level-usage table, the per-request `info` lines —
is owned by `docs/llds/orchestrate-cli.md § Logging`. The per-operation log call
sites belong to the components that emit them (HTTP server, pane locator, tmux
driver). The crate choice (`tracing` + `tracing-subscriber`) and its rationale are
recorded in `orchestrate-cli.md § Logging`; this LLD does not re-decide that.

## The init helper

```
core::observability::init(default_level: LevelFilter)
```

Installs the process-global `tracing` subscriber. Every gbiv entry point calls it
once, before any other work. The caller supplies the baseline level: the worktree
binary's `main()` passes `LevelFilter::INFO`; the daemon and fleet subcommands pass
the level their `-v`/`-vv` flags select (`info` → `debug` → `trace`). `RUST_LOG`,
when set, overrides that baseline.

### Filter resolution — `RUST_LOG` precedence

The effective filter is resolved from two inputs, `RUST_LOG` winning:

1. When `RUST_LOG` holds a non-empty value, its full `EnvFilter` directive string
   is used verbatim (e.g. `RUST_LOG=gbiv=debug,tiny_http=warn`). This is the
   standard Rust convention and lets a user tune per-module without touching any
   CLI surface.
2. Otherwise the `default_level` argument is used as a single-level directive.

An empty or whitespace-only `RUST_LOG` is treated as unset and falls back to the
default, so `RUST_LOG=` never silences all output.

The resolution is a pure function over `(default_level, RUST_LOG value)`, which
keeps `RUST_LOG`-precedence testable without touching global subscriber state:

```
fn filter_directive(default_level: &str, rust_log: Option<&str>) -> String
```

### Subscriber shape

The installed subscriber (built with `tracing_subscriber::fmt`) has:

- **Writer**: stderr. stdout is reserved for command results (JSON, status output).
- **Filter**: the `EnvFilter` from § Filter resolution.
- **Timestamps**: UTC, ISO-8601 (RFC 3339), e.g. `2026-06-27T13:45:01.234567Z`.
- **Target**: the emitting module path is shown, so `RUST_LOG` filters can target
  it (e.g. `gbiv::orchestration::tmux_driver`).
- **Plain text**: ANSI color is disabled so redirected or captured logs stay clean
  and greppable.

These match the example output in `orchestrate-cli.md § Logging > Format`.

### Verbosity query — `debug_enabled()`

```
core::observability::debug_enabled() -> bool
```

Returns `true` when the installed subscriber's effective maximum level is `DEBUG`
or more verbose (`TRACE`), i.e. `LevelFilter::current() >= LevelFilter::DEBUG`.
Because it reads the *installed filter's* level rather than a raw string, it
answers correctly however verbosity was set: `RUST_LOG=debug`, `RUST_LOG=gbiv=debug`,
and a `-v` flag all yield `true`; `info`/`warn`/`error` yield `false`.

The query exists so a caller that widens its output under debug logging stays
consistent with the one filter the user configured, rather than re-parsing
`RUST_LOG` itself. Its consumer is the worktree binary's top-level error
formatter (see `cli-and-palette.md`): on `Err`, the full `anyhow` cause chain is
printed when `debug_enabled()`, otherwise only the top-level message. The query is
meaningful once `init` has installed the subscriber; before that it reflects the
process default.

### Idempotency

A global default subscriber installs successfully only once per process. `init` is
therefore best-effort idempotent: it uses `try_init()` and ignores an
"already initialized" error rather than panicking. The first installation wins;
subsequent calls are no-ops. This keeps a test binary that calls `init` from many
tests, and an entry point that wraps another, from aborting because logging is
already up.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Init helper home | `core` (`gbiv-core::observability`) | Per-binary init; a `logging` mod in the gbiv binary | Daemon + CLI + worktree binary must not drift on format/filter; `core` is the shared floor |
| `default_level` parameter | Caller passes a `LevelFilter` | Hard-code `info`; read only `RUST_LOG` | One init contract serves callers with different baselines (`INFO` for worktree commands, `-v`/`-vv`-derived for the daemon/CLI) |
| `RUST_LOG` vs default | `RUST_LOG` (non-empty) overrides `default_level` | Default overrides env; merge both | Standard Rust convention; per-module escape hatch documented in orchestrate-cli LLD |
| Empty `RUST_LOG` | Treat as unset → use default | Honor it (parses to "everything off") | An empty value almost never means "silence everything"; falling back is least-surprise |
| Double-init | Best-effort `try_init`, ignore already-set | `init()` that panics; return `Result` to caller | Tests and nested entry points call init repeatedly; panicking or forcing every caller to handle an error is noise |
| Filter resolution factored out | Pure `filter_directive` fn | Resolve inline inside `init` | Lets `RUST_LOG`-precedence be unit-tested without global subscriber state |
| Timestamp format | UTC ISO-8601 (RFC 3339) | Local time; Unix epoch; uptime-relative | Logs from parallel color worktrees and the daemon need a stable, sortable, timezone-unambiguous stamp |
| Error-verbosity source | `debug_enabled()` reads the installed filter's level | A `RUST_LOG == "debug"` string check in `main()`; a separate `--verbose` flag | One env var, one source of truth: error-chain verbosity tracks the same filter that governs logs, so `RUST_LOG=gbiv=debug` behaves like `RUST_LOG=debug` |

## Edge Cases

| Case | Behavior |
|---|---|
| `init` called twice in one process | Second call is a no-op (already-initialized error swallowed) |
| `RUST_LOG` unset | Subscriber uses `default_level` |
| `RUST_LOG=` (empty) or whitespace | Treated as unset → `default_level` |
| `RUST_LOG=gbiv=debug,tiny_http=warn` | Full directive used verbatim; `default_level` ignored |
| `RUST_LOG` set to a malformed directive | `EnvFilter` parse falls back to `default_level` rather than aborting startup |
| stdout redirected/piped | Unaffected — logs go to stderr only |

## References

- HLD: `docs/high-level-design.md` § Components (orchestration domain)
- Logging config surface (levels, `-v`/`-vv`, format examples): `docs/llds/orchestrate-cli.md § Logging`
- Consumer of `debug_enabled()`: `docs/llds/cli-and-palette.md § Dispatch Flow`
