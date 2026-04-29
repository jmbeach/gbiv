# tmux Driver

**Created**: 2026-04-28
**Status**: Draft

## Context

The tmux Driver is the only component in gbork that touches the tmux CLI. Everything above it — Pane Locator, HTTP Server — calls into the driver instead of running `tmux` themselves. This keeps subprocess invocation, argument escaping, and exit-code handling in one place and makes the rest of gbork easy to test against a fake driver.

The driver covers four operations gbork needs:

1. **list_windows** — enumerate tmux windows in a session (used to find the gbiv color windows)
2. **list_panes** — enumerate panes in a window with the metadata needed for claude detection (`pane_id`, `pane_pid`, `pane_current_command`, `pane_current_path`)
3. **capture_pane** — capture the recent textual contents of a pane
4. **send_keys** — type text into a pane and press Enter

It does not own session naming, pane selection, or process-tree walking — those live in higher layers.

## Pane Targeting

Every operation takes a `PaneTarget`. The driver supports two forms:

- **By pane ID** — `%<n>` (e.g., `%12`). Stable for the pane's lifetime, immune to window/pane reindexing. Used everywhere after the Pane Locator resolves a window to a specific pane.
- **By window** — `<session>:<window>` (e.g., `myproject:red`). Used only by `list_panes` when the caller hasn't resolved a pane ID yet.

Window-only targets are not accepted by `capture_pane` or `send_keys`; those callers must pass a pane ID. This eliminates "which pane in this window?" ambiguity at the type level.

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
capture_pane(pane_id: &str, lines: usize) → Result<String>
```

Runs:
```
tmux capture-pane -t <pane_id> -p -S -<lines> -J
```

- `-p` prints to stdout (no buffer indirection)
- `-S -<lines>` starts capture `lines` rows above the bottom of the visible pane, pulling from history if available
- `-J` joins lines that tmux wrapped due to terminal width — the commander gets logical lines, not screen lines
- ANSI escape sequences are stripped by default (no `-e`); the commander reads plain text

Returns the captured text verbatim. If the pane no longer exists, returns `Err(TmuxError::PaneNotFound)`.

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
enum TmuxError {
    NotInstalled,           // tmux binary not on PATH
    SessionNotFound,
    PaneNotFound,
    SendKeysIncomplete,     // text sent, Enter failed
    Other(String),          // unmapped tmux stderr
}
```

`NotInstalled` is detected once at daemon startup (via `tmux -V`) and converted into a fatal startup error. The driver does not re-check on every call.

## Subprocess Conventions

- All invocations use `std::process::Command` directly — no async tmux library, no channels.
- `stdout` and `stderr` are captured separately and read fully into memory. The `lines` parameter on `capture_pane` bounds output size; v1 does not stream.
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
| Driver owns session naming | No | Yes, via `gbiv-core` reuse | Session selection is daemon-startup concern, not per-call concern; keeps the driver stateless |

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
| `lines=0` to `capture_pane` | tmux returns empty; passed through |

## Technical Debt & Future Work

1. **No streaming capture**: `capture_pane` is one-shot. SSE / live-tail mode would need a different driver entry point.
2. **No window-relative pane targeting in send/capture**: by design, but if a future caller has only a window target it must call `list_panes` first.
3. **`Other` error variant is a catch-all**: as gbork matures, more tmux failure modes will earn their own variants.

## References

- HLD: `docs/gbork/high-level-design.md` § Components > tmux Driver
- gbiv tmux usage (similar patterns): `docs/gbiv/llds/tmux-mirror.md`
