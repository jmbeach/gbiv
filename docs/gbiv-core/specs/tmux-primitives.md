# Tmux Primitives

EARS specs for the shared tmux primitives in `gbiv-core::tmux`.

**Component LLD**: `docs/gbiv-core/llds/tmux-primitives.md`

Marker convention: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Public Surface

- [x] TMX-CORE-001: The `gbiv_core::tmux` module shall expose exactly the public items `tmux_available`, `has_session`, `list_windows`, `session_name_for_root`, `WindowInfo`, and `TmuxError`.
- [x] TMX-CORE-002: `WindowInfo` shall expose two public fields: `id: String` and `name: String`.
- [x] TMX-CORE-003: `TmuxError` shall expose exactly the variants `NotInstalled`, `SessionNotFound(String)`, `PaneNotFound(String)`, `SendKeysIncomplete(String)`, and `Other(String)`, and shall derive `Debug` and `thiserror::Error`.

## `tmux_available`

- [x] TMX-CORE-010: When `tmux_available` is invoked and `tmux -V` exits 0, the system shall return `Ok(())` without inspecting the stdout contents.
- [x] TMX-CORE-013: When `tmux_available` is invoked and the `tmux` binary is not on `PATH` (exec returns `ENOENT`), the system shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-CORE-014: When `tmux_available` is invoked and `tmux -V` exits non-zero for any other reason, the system shall return `Err(TmuxError::Other(msg))` constructed per the conventions in TMX-CORE-060/061.
- [x] TMX-CORE-015: `tmux_available` shall not cache its result; each invocation shall re-exec `tmux -V`.
- [x] TMX-CORE-016: `tmux_available` shall not declare or enforce a minimum tmux version; "tmux is installed" is the only signal it produces beyond raw failures.

## `has_session`

- [x] TMX-CORE-020: When `has_session(name)` is invoked and `tmux has-session -t <name>` exits 0, the system shall return `Ok(true)`.
- [x] TMX-CORE-021: When `has_session(name)` is invoked and `tmux has-session -t <name>` exits non-zero with stderr containing the case-insensitive substring `"can't find session"`, the system shall return `Ok(false)`.
- [x] TMX-CORE-022: When `has_session(name)` is invoked and the `tmux` binary is not on `PATH`, the system shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-CORE-023: When `has_session(name)` is invoked and `tmux has-session` exits non-zero with stderr that does not match the missing-session phrase, the system shall return `Err(TmuxError::Other(msg))` constructed per TMX-CORE-060/061.
- [x] TMX-CORE-024: `has_session` shall not return `Err(TmuxError::SessionNotFound(_))` for the existence check; `SessionNotFound` is reserved for operations that target a specific session and discover it missing.

## `list_windows`

- [x] TMX-CORE-030: When `list_windows(session)` is invoked, the system shall execute `tmux list-windows -t <session> -F '#{window_id}\t#{window_name}'`.
- [x] TMX-CORE-031: When `list_windows` succeeds with exit 0 and every line of stdout splits into exactly two `\t`-separated fields, the system shall return `Ok(Vec<WindowInfo>)` with one entry per non-empty line, in the order tmux produced them.
- [x] TMX-CORE-032: When any single line of `list-windows` stdout fails to split into exactly two `\t`-separated fields, the system shall return `Err(TmuxError::Other(msg))` where `msg` contains the offending raw line, and shall not return a partial result.
- [x] TMX-CORE-033: When `list_windows` exits non-zero with stderr containing the case-insensitive substring `"can't find session"`, the system shall return `Err(TmuxError::SessionNotFound(session))` carrying the session name the caller passed in.
- [x] TMX-CORE-034: When `list_windows` is invoked and the `tmux` binary is not on `PATH`, the system shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-CORE-035: When `list_windows` exits non-zero for any other reason, the system shall return `Err(TmuxError::Other(msg))` constructed per TMX-CORE-060/061.
- [x] TMX-CORE-036: When `list_windows` exits 0 with empty stdout, the system shall return `Ok(vec![])`.

## `session_name_for_root`

- [x] TMX-CORE-040: When `session_name_for_root(folder_name)` is invoked, the system shall return a `String` equal to `folder_name.to_string()` without modification.
- [x] TMX-CORE-041: `session_name_for_root` shall not invoke any subprocess and shall not consult the filesystem.
- [x] TMX-CORE-042: `session_name_for_root` shall not validate `folder_name` against tmux's character rules; downstream tmux calls surface invalid names as their own errors.

## Subprocess Conventions

- [x] TMX-CORE-050: Every primitive that exec's tmux shall invoke `std::process::Command::new("tmux")` and shall not honor any `TMUX_BIN`-style override in v1.
- [x] TMX-CORE-051: When decoding tmux stdout or stderr for parsing or error construction, the system shall use `String::from_utf8_lossy` over the whole captured buffer.
- [x] TMX-CORE-052: When a tmux call exits with code 0, the system shall ignore any bytes written to stderr.
- [x] TMX-CORE-053: The primitives shall not write to tmux's stdin, shall not impose per-call timeouts, and shall not retry on failure.

## `Other` Message Format

- [x] TMX-CORE-060: When the system constructs a `TmuxError::Other(msg)` for a non-zero exit and the trimmed stderr is non-empty, `msg` shall equal the trimmed stderr.
- [x] TMX-CORE-061: When the system constructs a `TmuxError::Other(msg)` for a non-zero exit and the trimmed stderr is empty, `msg` shall equal `format!("exit status: {code}")` where `code` is the numeric exit code, or the literal string `"exit status: signal"` if the process was terminated by a signal.

## Error Surface Invariants

- [x] TMX-CORE-070: `gbiv_core::tmux` shall not depend on `anyhow`; every fallible primitive shall return `Result<T, TmuxError>`.
- [x] TMX-CORE-071: The `PaneNotFound` and `SendKeysIncomplete` variants of `TmuxError` shall not be constructed by any code in `gbiv-core`; they exist for consumer crates (currently `roy`) that return into the same `Result<T, TmuxError>`.
