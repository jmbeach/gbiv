# tmux Primitives

**Created**: 2026-05-16
**Status**: Greenfield draft (Phase 2 of gbv-x2v)

## Context and Design Philosophy

`gbiv-core::tmux` owns the small set of tmux operations that both `gbiv` and `roy` need: detecting whether tmux is installed, checking session existence, listing windows in a session, and deriving the canonical tmux session name from a gbiv project root.

The principle is *narrowest possible shared surface*. Window mutation (`new-window`, `move-window`, `kill-window`) stays in `gbiv` — `roy` has no use for it. Pane operations (`list-panes`, `capture-pane`, `send-keys`) stay in `roy` — `gbiv` has no use for them. The shared surface is exactly the lookup operations that, if implemented inconsistently across binaries, would produce confusing cross-binary disagreement about "what session are we in?" or "is tmux installed?"

This LLD is the single source of truth for the four shared primitives and the `TmuxError` enum. `docs/gbiv/llds/tmux-mirror.md` and `docs/roy/llds/tmux-driver.md` link here for the contract and describe only what they add on top.

## Public Surface

```rust
pub fn tmux_available() -> Result<(), TmuxError>;
pub fn has_session(name: &str) -> Result<bool, TmuxError>;
pub fn list_windows(session: &str) -> Result<Vec<WindowInfo>, TmuxError>;
pub fn session_name_for_root(folder_name: &str) -> String;

pub struct WindowInfo {
    pub id: String,    // tmux @<n> form
    pub name: String,  // user-facing window name
}

#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("tmux binary not on PATH")]
    NotInstalled,
    #[error("tmux session not found: {0}")]
    SessionNotFound(String),
    #[error("tmux pane not found: {0}")]
    PaneNotFound(String),
    #[error("send-keys completed for text but Enter failed for pane {0}")]
    SendKeysIncomplete(String),
    #[error("tmux: {0}")]
    Other(String),
}
```

Module path: `gbiv_core::tmux`. Re-exported at `gbiv_core::tmux::*`.

## Operations

### `tmux_available()`

Runs `tmux -V`. Maps:

- Exit 0 → `Ok(())`. Version contents are not inspected.
- `ENOENT` at exec (binary not on `PATH`) → `Err(NotInstalled)`.
- Any other failure (permission denied, non-zero exit) → `Err(Other(<stderr-or-explanation>))` per the Subprocess Conventions message rule.

No minimum tmux version is declared. The only tmux installation state this primitive distinguishes is "tmux is on `PATH` and exec's" vs. "tmux is not on `PATH`." If an old or unusual tmux later breaks `has-session` stderr parsing or `list-windows` field layout, those failures surface as `Other(...)` at the relevant primitive — not as a version-gate at startup. Rationale: the workspace targets recent tmux in practice; pre-emptively gating on a version number creates a knob to maintain (bumping the bar, updating tests, mapping the bespoke error message on the gbiv side) without a real failure mode to point at.

The result is **not cached**. Each call re-invokes. Callers that want amortized cost wrap with their own `OnceCell` at the consumer layer (see HLD § "No shared state").

The function deliberately returns `Result<(), TmuxError>` rather than `bool`. Callers that need a boolean use `.is_ok()`. The typed return preserves the distinction between "tmux is missing" (`NotInstalled`) and "tmux is broken in some other way" (`Other`) — collapsing both into `false` discards the diagnostic that callers want in their error messages.

### `has_session(name)`

Runs `tmux has-session -t <name>`. Maps:

- Exit 0 → `Ok(true)`.
- Non-zero exit with stderr matching `can't find session` (case-insensitive substring) → `Ok(false)`.
- `ENOENT` at exec → `Err(NotInstalled)`.
- Any other non-zero exit → `Err(Other(<stderr>))`.

Returning `Ok(true)/Ok(false)` for the existence check (rather than `Err(SessionNotFound)` for the missing case) matches how callers actually consume this: a guard like *"create session only if it doesn't exist"* reads more naturally as `if !has_session(name)?` than as `if matches!(has_session(name), Err(SessionNotFound(_)))`. `SessionNotFound` is reserved for callers that asked for a specific session operation and got back "no such session" (`list_windows`, future ops) — i.e., the variant carries a positive intent of *"I expected this session to be there."*

### `list_windows(session)`

Runs `tmux list-windows -t <session> -F '#{window_id}\t#{window_name}'`. Returns `Vec<WindowInfo>` parsed from tab-separated stdout.

Maps:

- Exit 0 with parseable output → `Ok(Vec<WindowInfo>)`.
- Exit 0 with at least one malformed line (wrong field count after splitting on `\t`) → `Err(Other(<line>))`. All-or-nothing — a single bad line aborts the call.
- Non-zero with `can't find session` in stderr → `Err(SessionNotFound(<session-name>))`.
- `ENOENT` at exec → `Err(NotInstalled)`.
- Other non-zero → `Err(Other(<stderr>))`.

`WindowInfo` exposes both fields even though gbiv only consumes `.name` today. The cost of carrying `.id` is one allocation per window; the alternative (two functions, or a `list_windows_minimal()` variant) splits the consumer-side trace for no real savings. Future gbiv operations that need to disambiguate windows under rename (e.g., a robust move) will already have the IDs.

`#{window_id}` returns the `@<n>` form — stable for the window's lifetime, never reused within a tmux server session. `WindowInfo.id` is `String` rather than a newtype in v1; introducing `WindowId(String)` is an evolution vector if `gbiv-core` ever surfaces both window and pane IDs and the mix-up risk becomes real.

### `session_name_for_root(folder_name)`

Pure function: returns `folder_name.to_string()`. No subprocess, no validation. If the folder name contains a tmux-forbidden character (`:`, `.`, `\0`), the downstream `has_session` / `new-session` call will fail and the consumer surfaces the error there — pushing validation into this primitive would only duplicate tmux's own rejection.

Takes `&str` rather than `&GbivRoot` or `&Path`. The function is pure string manipulation; accepting `&str` keeps it trivially unit-testable and avoids coupling `gbiv-core::tmux` to the `root` module's type. Callers pass `&root.folder_name` at the call site — that one-line boilerplate makes it explicit that the session name is the folder name, no other root state is consulted, and the contract is stable if the `GbivRoot` struct evolves.

If derivation ever needs more than the folder name (e.g., disambiguating two projects with the same folder name), the function will gain extra parameters explicitly rather than silently widening its access to `GbivRoot` internals.

## Error Surface

`TmuxError` is the single tmux error type for the workspace. It lives in `gbiv-core` and unions the failure modes both binaries produce. Variants populated by code in this crate vs. populated only by roy's tmux driver are tagged below:

| Variant | Trigger | Populated by |
|---|---|---|
| `NotInstalled` | `ENOENT` at exec; `tmux -V` not on `PATH`. | both |
| `SessionNotFound(name)` | A session-targeting op got "can't find session" from tmux. | both |
| `PaneNotFound(id)` | A pane-targeting op (capture/send) got "can't find pane" from tmux. | roy only |
| `SendKeysIncomplete(pane)` | Two-step `send-keys` invocation: the text reached the pane but the Enter step failed. | roy only |
| `Other(msg)` | Any other tmux failure: parse error, non-zero exit with unfamiliar stderr, etc. | both |

One enum, one source of truth, no `#[from]` plumbing in roy. The pane variants are listed here because roy populates them via the *same* `TmuxError` type — that way roy's `tmux_driver` module and `gbiv-core::tmux` can return into the same `Result<T, TmuxError>` without conversion. gbiv code that exhaustively matches on `TmuxError` (rare in practice — most call sites use `?` or `.map_err(anyhow!)`) covers the pane variants with `_ => unreachable!()` or, more honestly, a wildcard that surfaces the unexpected variant.

This is an explicit relaxation of the HLD inclusion bar at the *variant* level: the primitive (the enum) is genuinely shared, and listing pane variants here is cheaper than the alternative wrapping pattern. The HLD bar still governs whether a *function* belongs in `gbiv-core`.

See `docs/roy/llds/tmux-driver.md` § "Error Surface" for which roy operations produce which variants.

## Subprocess Conventions

- All invocations use `std::process::Command` directly. No async, no third-party tmux library.
- The tmux binary is resolved via `PATH` only — `Command::new("tmux")`. No `TMUX_BIN` env override in v1; the existing gbiv code does the same and no caller has needed otherwise.
- `stdout` and `stderr` are captured fully into memory. The shared primitives do not stream and do not page; pane capture (which has volume concerns) lives in roy.
- **UTF-8 decoding** of tmux stdout/stderr uses `String::from_utf8_lossy` over the whole captured buffer before any parsing. Non-UTF-8 bytes (a user named a window with a non-UTF-8 sequence; tmux echoes a stderr message in a non-UTF-8 locale) become `U+FFFD` and the operation continues. Rationale: window names are display-only here, and a non-UTF-8 byte deep in stderr should not turn into a different error class than the same message in UTF-8.
- **`TmuxError::Other` message format**: when a tmux call returns a non-zero exit and stderr does not match a known phrase, the `Other(msg)` payload is built as `stderr.trim()` if non-empty, otherwise `format!("exit status: {code}")` where `code` is the numeric exit code (or the string `"signal"` if the process was signaled). Call sites do not invent free-form wrappers; uniform messages make grep, log-correlation, and test assertions stable.
- **stderr on success is ignored.** If exit code is 0, any bytes on stderr are dropped without inspection. tmux occasionally emits cosmetic warnings (`TERM` mismatches, deprecated-option notices) at exit 0; surfacing them would create per-environment test flakiness for no benefit. Consumers that genuinely need tmux stderr capture can build their own subprocess wrapper outside `gbiv-core`.
- Output parsers for `list_windows` require the exact field count produced by the `-F` format. Malformed lines yield `TmuxError::Other` with the offending line in the message.
- No retries, no timeouts. The shared primitives are local subprocess calls; a wedged tmux server blocks indefinitely. Consumer binaries can add timeouts at their layer if they ever need to.
- The primitives never read from `stdin`.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| `TmuxError` scope | Single enum in gbiv-core with all variants (including pane ones populated only by roy) | Shared variants in gbiv-core + roy wraps via `#[from]`; per-function error enums | One enum across the workspace; no shadowing or conversion. Roy's tmux driver and `gbiv-core::tmux` return into the same `Result<T, TmuxError>` directly. Explicit variant-level relaxation of the HLD inclusion bar; the rule governs functions, not variants of an enum that is itself shared. |
| `has_session` return | `Result<bool, TmuxError>` with `Ok(false)` for missing session | `Result<(), TmuxError>` with `Err(SessionNotFound)` for missing; pure exit-status `Ok(bool)` | Existence checks are typically guards (`if !has_session(name)?`); returning `Err` for the common "not found" case forces match noise everywhere. `SessionNotFound` is reserved for ops that meant to use the session and discovered it was gone. |
| `WindowInfo` shape | Single struct `{id, name}` | Two functions (`list_window_names() → Vec<String>` + `list_windows() → Vec<WindowInfo>`); generic over a format type | One allocation per window is negligible. The two-function split would force every roy/gbiv consumer to remember which one they want; a single struct lets gbiv ignore `.id` and roy use it. |
| `session_name_for_root` input | `&str` (folder name) | `&GbivRoot`; `&Path` | Pure string function; trivial to unit-test without constructing a root. Avoids coupling `gbiv-core::tmux` to the `root` module. Callers pass `&root.folder_name` explicitly, signaling that the session name is the folder name and nothing else from the root struct is consulted. |
| Caching `tmux_available` | None | `OnceCell` inside the module | HLD says no shared state. Consumers cache at their layer if needed. |
| tmux binary lookup | `PATH` only | `TMUX_BIN` env var override | No caller has needed it; YAGNI. Easy to add later without breaking changes. |
| `WindowInfo.id` type | `String` | `WindowId(String)` newtype | Newtype prevents window/pane ID mix-up, but `gbiv-core` only surfaces window IDs. If pane IDs ever land here, promote both sides simultaneously. |
| `list_windows` parse failure | All-or-nothing — one bad line aborts the call | Best-effort: skip bad lines, return what parsed | A malformed line means the `-F` format and the parser have drifted; silently dropping data hides the bug. Loud failure is better than partial truth. |
| UTF-8 handling on tmux output | `String::from_utf8_lossy` on the whole stdout/stderr buffer | Fail loud on bad bytes; expose `Vec<u8>` / `OsString` to consumers | Names are display-only; lossy keeps the parser robust without inventing a third error class. Consumers that care about exact bytes are not the target audience for these primitives. |
| `Other` message format | `stderr.trim()` else `exit status: {code}` | Always include both; free-form per call site | One predictable shape makes assertions and log scraping stable across the workspace. |
| stderr-on-success | Ignored | Capture to a caller sink | Cosmetic warnings under odd `TERM`s would otherwise cause flaky tests for no signal. |
| Minimum tmux version | Not declared; `tmux_available()` distinguishes installed vs. not, nothing else | Declare `>= 2.4` with a runtime version check | Pre-emptive version gating adds maintenance (bar-bumping, message-mapping at every consumer) without a real failure mode to point at; old-tmux quirks would surface as `Other(...)` at the affected primitive if they ever happen. |
| Subprocess timeouts | None | Per-call timeout | Local tmux ops are reliably fast or wedged; per-call timeouts add a knob without a real failure mode behind it. Evolution vector if a wedged tmux server becomes a real problem. |

## Edge Cases

| Case | Behavior |
|---|---|
| `tmux -V` returns 0 but with empty stdout | `Ok(())` — we only care about exit code, not version contents |
| `has_session` called with empty name | Passed through; tmux rejects with non-zero and stderr; we map to `Other` |
| `has_session` stderr varies across tmux versions | We match `can't find session` case-insensitively; if a future tmux changes the phrasing, that case-not-found path collapses into `Other`. Acceptable; we'd surface this when it breaks. |
| `list_windows` returns 0 windows | `Ok(vec![])`. Theoretical only — tmux sessions always have at least one window. |
| Window name contains a tab character | tmux output has an extra field per line → parser yields `Other`. Edge case; tmux UI itself allows tab in names but our `-F` format is unambiguous only without tabs. |
| Window name contains a newline | The line-oriented parser sees two lines, both malformed (wrong field count after split) → `Other`. Acceptable: users who name a window with a literal newline get the same error tmux's own list-windows already produces in odd ways. |
| `session_name_for_root` on a root with `:` in folder name | Returns the bad name unchanged; the next `has_session`/`new-session` call fails with `Other`. Validation lives at the tmux boundary, not here. |
| Concurrent calls from different threads | Each call is its own subprocess; tmux server serializes server-side. No locking in `gbiv-core`. |
| tmux server restarts between two calls | Each call independent; no cached state to invalidate. |
| Two callers race on `has_session` → `new-session` | Each primitive is atomic in isolation; the *guard-then-create* sequence is not. tmux's own `new-session` is idempotent enough (duplicate name returns non-zero), so the race surfaces as `Other("duplicate session: ...")` at the consumer. Out of scope for `gbiv-core`; consumer binaries that care add their own serialization. |
| `tmux` server IPC socket exists but is not readable/writable by the caller | tmux exits non-zero with stderr like `error connecting to /tmp/tmux-1000/default (Permission denied)`; mapped to `Other(<stderr>)`. Recovery (chmod, kill the server) is a consumer concern. Out of scope for `gbiv-core`. |
| `tmux -V` returns a version line we cannot parse | Not inspected. `tmux_available()` returns `Ok(())` on any exit-0; version contents are irrelevant. |

## Technical Debt & Future Work

1. **`Other` is a catch-all.** As shared primitives accumulate, more failure modes earn their own variants. The first promotion candidate is probably `ParseError` for `list_windows` malformed-line cases — currently buried in `Other`.
2. **No tmux binary override.** A `TMUX_BIN` env var could land in v2 if multi-version testing demands it.
3. **No `WindowId` newtype.** Promote when pane IDs also live in `gbiv-core`, not before.
4. **No streaming `list_windows`.** Sessions with thousands of windows aren't a real workload; if they become one, paginate.
5. **`session_name_for_root` is a one-line `.clone()`.** If the derivation ever grows (e.g., to disambiguate two gbiv projects with the same folder name), it moves into a proper module-level function with its own tests.

## References

- `docs/gbiv-core/high-level-design.md` — parent HLD; declares the inclusion bar and the typed-error discipline.
- `docs/gbiv/llds/tmux-mirror.md` — gbiv-side consumer; documents window mutation that stays in gbiv.
- `docs/roy/llds/tmux-driver.md` — roy-side consumer; documents pane-level ops that stay in roy, and roy's `TmuxError` that wraps `gbiv-core::tmux::TmuxError` via `#[from]`.
