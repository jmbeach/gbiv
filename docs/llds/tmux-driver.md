# tmux Driver

**Created**: 2026-04-28
**Status**: Draft

## Context

The tmux Driver is the only component in gbiv that touches the tmux CLI. Everything above it — Pane Locator, HTTP Server — calls into the driver instead of running `tmux` themselves. This keeps subprocess invocation, argument escaping, and exit-code handling in one place and makes the rest of gbiv easy to test against a fake driver.

The driver covers four operations gbiv needs:

1. **list_windows** — enumerate tmux windows in a session (used to find the gbiv color windows)
2. **list_panes** — enumerate panes in a window with the metadata needed for claude detection (`pane_id`, `pane_pid`, `pane_current_command`, `pane_current_path`)
3. **capture_pane** — capture the recent textual contents of a pane
4. **send_keys** — type text into a pane and press Enter

It does not own session naming, pane selection, or process-tree walking — those live in higher layers.

## Pane Targeting

Targets are passed as plain `&str` in two forms:

- **By pane ID** — `%<n>` (e.g., `%12`). Stable for the pane's lifetime, immune to window/pane reindexing. `capture_pane` and `send_keys` take a pane ID.
- **By window** — `<session>:<window>` (e.g., `myproject:red`). `list_panes` takes a window target when the caller has not yet resolved a pane ID.

The pane-vs-window distinction is enforced by *which operation the caller invokes* — `capture_pane`/`send_keys` operate on a resolved pane ID, `list_panes` on a window — not by a wrapper type. This matches the plain-`&str` style of the shared `core::tmux` primitives (`list_windows(&str)`).

## Operations

### list_windows

```
list_windows(session: &str) → Result<Vec<WindowInfo>>
```

Runs:
```
tmux list-windows -t <session> -F '#{window_id}\t#{window_name}'
```

Returns a `Vec<{id, name}>` parsed from tab-separated stdout. If the session does not exist, tmux exits non-zero with `can't find session`; the driver returns `Err(TmuxError::SessionNotFound)`.

### list_panes

```
list_panes(window_target: &str) → Result<Vec<PaneInfo>>
```

Runs:
```
tmux list-panes -t <window_target> -F '#{pane_id}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}'
```

Returns a `Vec<{id, pid, current_command, current_path}>`. The Pane Locator uses `pid` to walk the process tree (since `current_command` is unreliable for Claude Code, see HLD). `current_path` is informational.

### capture_pane

```
capture_pane(pane_id: &str, range: CaptureRange, max_bytes: usize) → Result<Capture>

enum CaptureRange {
    /// Tail of the buffer — most common. Equivalent to `-S -<lines>` with
    /// no `-E`, i.e., from `lines` rows up to the bottom of the visible pane.
    Tail { lines: usize },

    /// Explicit row window. Both bounds are tmux row offsets:
    ///   negative = rows back from the bottom of the visible pane
    ///   0        = top of the visible pane
    ///   positive = rows down into the visible pane
    /// `start` must be ≤ `end`. Use `start: i32::MIN` to mean
    /// "top of history" (mapped to tmux's `-` literal).
    Window { start: i32, end: i32 },
}

struct Capture {
    text: String,
    truncated: bool,
    original_bytes: usize,    // byte length of the raw tmux output for the requested range
    returned_bytes: usize,    // byte length of `text` after any truncation
    range_requested: CaptureRange,
    range_returned: (i32, i32), // the requested row window: Tail { lines } → (-lines, 0),
                                 // Window { start, end } → (start, end). A byte-cap head-drop
                                 // is reported via truncated/*_bytes, not by shifting this
                                 // (see § Technical Debt #6).
}
```

Runs (Tail):
```
tmux capture-pane -t <pane_id> -p -S -<lines> -J
```

Runs (Window):
```
tmux capture-pane -t <pane_id> -p -S <start> -E <end> -J
```

(`start: i32::MIN` becomes the literal `-` argument to `-S`, meaning the top of history.)

- `-p` prints to stdout (no buffer indirection)
- `-S` / `-E` bound the row range — see tmux(1) for the offset semantics
- `-J` joins lines that tmux wrapped due to terminal width — the commander gets logical lines, not screen lines
- ANSI escape sequences are stripped by default (no `-e`); the commander reads plain text

Returns the captured text along with truncation/range metadata. If the pane no longer exists, returns `Err(TmuxError::PaneNotFound)`.

#### Output Cap

The caller passes `max_bytes`, the byte cap applied to captured output before returning. Rationale: the typical caller is an LLM (Claude Code via the skill) and an unbounded pane capture can blow a context budget — a noisy build or test runner can produce hundreds of KB in a single capture. Anthropic's own guidance for tool authors caps Claude Code MCP tool responses at 25,000 tokens by default; gbiv follows the same shape with byte-based limits since the driver does not tokenize.

- **Default cap** — `DEFAULT_CAP_BYTES = 64 KiB` (~16k tokens at typical English/code ratios — leaves headroom under the 25k-token reference point). The value callers pass for `max_bytes` unless they have a reason to request more.
- **Hard maximum** — `HARD_MAX_BYTES = 256 KiB`. The driver clamps `max_bytes` to this ceiling, so a caller requesting more rows (or a larger `max_bytes`) still gets ≤256 KiB. Beyond this, the caller is misusing the API and should paginate via repeated calls with smaller ranges.
- **Trim direction**: keep the **tail** (the most recent output). Discard the head — pane scrollback is read for "what just happened," and the bottom of the buffer is what matters. The first byte kept always starts at a UTF-8 boundary; the driver scans forward from the cut point until it finds one to avoid emitting invalid UTF-8.
- **Marker**: when `truncated == true`, the returned `text` is prefixed with a single line:
  ```
  […truncated 482312 of 547848 bytes from the head; showing the most recent 65536. To page earlier history, re-call with start_line/end_line bounding the dropped range.]
  ```
  The marker is plain text (no ANSI), ends with a newline, and is **included in `returned_bytes`** so the consumer's accounting matches what they actually receive.
- The marker text is stable so the skill / CLI can pattern-match on `[…truncated ` if it wants to react.

#### Pagination

Callers that want to read past the byte cap use `CaptureRange::Window` to step backward through history:

1. Call with `Tail { lines }` (or any `Window`). If `truncated == true`, step the window further back using its own bounds.
2. Re-call with a `Window` whose `end` is the previous request's `start - 1` and whose `start` reaches a cap-sized span earlier (e.g. `start - 200` rows). Repeat until either the chunk fits without truncation or the caller has gone as far back as they want. (A precise row anchor for the byte-cap-dropped head is deferred — see § Technical Debt #6 — so callers page by row window, not by the dropped-byte boundary.)
3. To explicitly start from the top of history, use `Window { start: i32::MIN, end: ... }`.

The driver does not retain any cursor state between calls — pagination is purely client-driven via row offsets. Pane scrollback can change between calls (new output pushes old rows further into the past); the row-offset semantics stay consistent (offsets are relative to the bottom of the visible pane *at call time*) but the absolute content at a given offset may shift. For typical gbiv use (an agent that paused and is no longer producing output), this is a non-issue.

The cap is applied in the driver, not in the HTTP layer, so any future caller of the driver gets the same protection without re-implementing it.

### send_keys

```
send_keys(pane_id: &str, text: &str) → Result<()>
```

Two-step invocation to keep text and Enter unambiguous:

```
tmux send-keys -t <pane_id> -l -- <text>
tmux send-keys -t <pane_id> Enter
```

- `-l` is literal mode: tmux does not interpret key names like `Enter` or `C-c` inside `text`. A user message containing the word "Enter" stays as the word "Enter."
- The trailing `--` ends option parsing so `text` starting with `-` is safe.
- The Enter keypress is a separate call so it cannot collide with the literal text.

If the first call succeeds and the second fails, the pane has received the text but not the Enter. The driver surfaces this as `Err(TmuxError::SendKeysIncomplete)` so the HTTP layer can return a precise error.

## Error Surface

```rust
#[derive(Debug, thiserror::Error)]
enum TmuxError {
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

The driver returns `Result<T, TmuxError>` from every operation. `thiserror` generates the `Display` and `std::error::Error` impls; the HTTP layer matches on variants to pick a status code. `NotInstalled` is detected once at daemon startup (via `tmux -V`) and converted into a fatal startup error. The driver does not re-check on every call.

The driver never uses `anyhow` — staying typed is the whole point of this layer.

## Shared Primitives in the `core` module

gbiv is one crate. A handful of tmux operations are needed by both caller groups —
the worktree commands (sync/clean/new-session) and the orchestration daemon
(locator startup). Those — and only those — live in the internal `core::tmux`
module:

| Primitive | Signature | Used by |
|---|---|---|
| `tmux_available()` | `Result<(), TmuxError>` (maps to `NotInstalled`) | both |
| `has_session(name)` | `Result<bool, TmuxError>` | both |
| `list_windows(session)` | `Result<Vec<Window>, TmuxError>` | both |
| `session_name_for_root(root)` | `String` (folder-name derivation) | both |

`TmuxError` is defined in `core` so every caller maps the same variants. The tmux
driver re-exports it (or layers its own variants on top — `PaneNotFound`,
`SendKeysIncomplete`, etc., are orchestration-only).

Everything else stays in its own module:

- **Worktree-only**: `new_session`, `new_window`, `kill_window`, `move_window` — window mutation, used only by the tmux-mirror commands.
- **Orchestration-only**: `list_panes`, `capture_pane`, `send_keys` — pane-level read/write, used only by the daemon.

## Subprocess Conventions

- All invocations use `std::process::Command` directly — no async tmux library, no channels.
- `stdout` and `stderr` are captured separately and read fully into memory. The `lines` parameter on `capture_pane` bounds output size loosely; the byte cap (see "Output Cap" above) is the hard upper bound. v1 does not stream.
- Errors are constructed from `stderr` only. Exit code zero means success; any non-zero is mapped to a `TmuxError` variant by matching well-known stderr substrings, falling back to `Other`.
- `ENOENT` at exec time (tmux removed/replaced after the startup `-V` check) is mapped to `TmuxError::NotInstalled` on every call, not just startup.
- Output parsers (for `list_windows` and `list_panes`) require the exact field count produced by the `-F` format string. Malformed lines yield `TmuxError::Other` with the offending line in the message.
- No timeouts. tmux operations are local and synchronous; the v1 daemon accepts that a wedged tmux server could block a request indefinitely. Adding per-call timeouts is an evolution vector if it becomes a real problem.
- The driver never reads from `stdin` — every operation is unidirectional.
- No retries. If tmux fails, the caller decides.

## Identifier Stability

- **Pane IDs (`%<n>`)** are stable for the lifetime of the pane and never reused within a tmux server's lifetime. Safe to cache between operations within a single HTTP request.
- **Window IDs (`@<n>`)** are similarly stable; window *names* are not (users can rename, gbiv may rename during sync).
- The driver caches no IDs across requests. Each HTTP request re-runs `list_windows` / `list_panes`. This sidesteps invalidation entirely.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| tmux interface | `Command` exec | `tmux-rs` library, libtmux IPC | Matches the existing gbiv pattern; no third-party tmux deps; exec is fast enough for on-demand calls |
| send-keys split | Two calls (text, then Enter) | One call with `Enter` appended | Eliminates accidental key-name interpretation in user-supplied text |
| Targeting type | Pane ID for capture/send, window target only for list | Always accept either | Rules out "which pane?" ambiguity at the API surface |
| Wrap handling | `-J` (join) | Leave wrapped | Commander reads logical lines, not screen lines |
| ANSI handling | Strip (default) | `-e` to preserve | Commander parses plain text; ANSI is noise |
| Retry policy | None | Retry on transient tmux errors | tmux operations are local; transient failures are vanishingly rare and the HTTP layer can retry if needed |
| Driver owns session naming | No | Yes, via the `core` module reuse | Session selection is daemon-startup concern, not per-call concern; keeps the driver stateless |
| Capture output cap | 64 KiB default, 256 KiB hard max, applied in the driver | No cap (rely on caller's `lines`); cap in HTTP layer; cap by line count only | LLM consumers have finite context. Bytes is the right unit because line lengths vary wildly. Driver-level enforcement protects every future caller |
| Trim direction when capping | Keep tail (most recent) | Keep head; keep middle | Pane scrollback is read for "what just happened"; the bottom is the signal |
| Truncation signaling | Inline marker at top of `text` + structured `truncated`/`*_bytes` fields | Header-only metadata; throw an error and force smaller request | Inline marker means an LLM that only reads `text` still notices; structured fields let the CLI/skill reason precisely |
| Pagination model | Row-window params (`Window { start, end }` mapped to tmux `-S/-E`) | Opaque cursor; offset/limit by bytes; no pagination | Mirrors tmux's native primitive; no server-side cursor state to design or invalidate; cursor scheme can replace it later without breaking `Tail { lines }` |

## Edge Cases

| Case | Behavior |
|---|---|
| Pane was killed between `list_panes` and `capture_pane` | `capture_pane` returns `PaneNotFound` |
| Window has zero panes (impossible in practice but listable) | `list_panes` returns empty `Vec`; Pane Locator handles |
| `text` to `send_keys` is empty | Both calls run; pane receives a bare Enter. Acceptable — caller is responsible for not sending empty text |
| `text` contains a NUL byte | tmux rejects; mapped to `TmuxError::Other` |
| `text` contains newlines | Literal mode preserves them as `\n` characters in the pane buffer; tmux does not auto-press Enter for embedded newlines |
| Very large `text` (>argv limit) | Exec fails with `E2BIG`; surfaced as `TmuxError::Other`. Higher layers may chunk if this matters; v1 does not |
| Session renamed between calls | `SessionNotFound`; daemon restart needed |
| `Tail { lines: 0 }` | tmux returns empty; passed through. `truncated == false`, `original_bytes == returned_bytes == 0` |
| Capture exceeds `max_bytes` cap | `text` carries the marker line followed by the most recent ≤`max_bytes`; `truncated == true`; `original_bytes` reflects what tmux produced; `returned_bytes` reflects what was actually returned; `range_returned.0` shifts toward `range_returned.1` to reflect the dropped head |
| Caller passes `max_bytes` > `HARD_MAX_BYTES` (256 KiB) | Driver clamps the effective cap to 256 KiB; behavior identical to the `max_bytes` case but with the clamped cap |
| UTF-8 character is split at the cut point | Driver advances the cut forward to the next valid UTF-8 boundary so the returned `text` is always valid UTF-8 |
| `Window { start, end }` with `start > end` | Driver rejects with `TmuxError::Other("invalid range")` before invoking tmux |
| `Window` requesting rows beyond the start of history | tmux clamps to available history; `range_returned.0` reflects the actual top of what was captured |
| Pane buffer changed between two paginated calls | New rows shift the offset frame. The driver does not detect this; callers paginating across a still-active pane should not assume row offsets are stable across calls (call them out as such in the skill) |

## Technical Debt & Future Work

1. **No window-relative pane targeting in send/capture**: by design, but if a future caller has only a window target it must call `list_panes` first.
2. **`Other` error variant is a catch-all**: as gbiv matures, more tmux failure modes will earn their own variants.
3. **Cap is byte-based, not token-based**: a future revision could use a tokenizer (or a quick char-based estimate) to cap closer to the model's real budget. Bytes are a safe, dependency-free proxy for v1.
4. **Pagination is row-offset, not cursor**: callers do their own row arithmetic. An opaque `?before=<cursor>` scheme could replace this without breaking the simple `Tail { lines }` shorthand.
5. **Row offsets shift while a pane is still producing output**: stable across paused panes (the typical gbiv use case), unstable across active ones. A snapshot ID could anchor pagination if this becomes a real problem.
6. **`range_returned` does not reflect the byte-cap head-drop**: it carries the requested row window (`Tail { lines }` → `(-lines, 0)`, `Window { start, end }` → `(start, end)`). When the byte cap drops the head of a capture, that loss is reported via `truncated`/`original_bytes`/`returned_bytes`, not by shifting `range_returned.0` — bytes cannot be mapped back to exact tmux rows from the capture alone. Pagination that needs a precise row anchor for the dropped head is deferred until the HTTP layer needs it.

## References

- HLD: `docs/high-level-design.md` § Components > tmux Driver
- gbiv tmux usage (similar patterns): `docs/llds/tmux-mirror.md`
