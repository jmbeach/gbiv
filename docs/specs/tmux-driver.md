# tmux Driver

Specs for the shared `core::tmux` primitives (implemented in `gbiv-core`) and the
orchestration-only tmux driver operations (deferred to the fleet orchestration phase).

**Component LLD**: `docs/llds/tmux-driver.md`

## Shared Core Primitives

- [x] TMX-DRV-001: `TmuxError` shall be a typed error enum with variants: `NotInstalled`, `SessionNotFound(String)`, `PaneNotFound(String)`, `SendKeysIncomplete(String)`, `Other(String)`.
- [x] TMX-DRV-002: `WindowInfo` shall be a struct with public fields `id: String` (tmux window ID, e.g. `@12`) and `name: String` (window name).
- [x] TMX-DRV-003: When `tmux_available()` is called and the tmux binary is not on PATH, it shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-DRV-004: When `tmux_available()` is called and `tmux -V` exits successfully, it shall return `Ok(())`.
- [x] TMX-DRV-005: When `has_session(session)` is called and the named tmux session exists, it shall return `Ok(true)`.
- [x] TMX-DRV-006: When `has_session(session)` is called and the named tmux session does not exist, it shall return `Ok(false)`.
- [x] TMX-DRV-007: When `has_session(session)` is called and tmux is not on PATH, it shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-DRV-008: When `list_windows(session)` is called for an existing session, it shall return `Ok(Vec<WindowInfo>)` parsed from `tmux list-windows -t <session> -F '#{window_id}\t#{window_name}'`.
- [x] TMX-DRV-009: When `list_windows(session)` is called for a non-existent session, it shall return `Err(TmuxError::SessionNotFound(session.to_string()))`.
- [x] TMX-DRV-010: When `list_windows(session)` is called and tmux is not on PATH, it shall return `Err(TmuxError::NotInstalled)`.
- [x] TMX-DRV-011: When a line in `list_windows` output is missing a field (no tab separator or empty field), the system shall return `Err(TmuxError::Other(...))` including the malformed line.
- [x] TMX-DRV-012: `session_name_for_root(folder_name)` shall return the folder name as a `String`, establishing the convention that the tmux session name equals the gbiv folder name.

## Orchestration-only Operations

- [D] TMX-DRV-013: `list_panes(window_target)` shall run `tmux list-panes -t <window_target> -F '#{pane_id}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}'` and return `Vec<PaneInfo>`.
- [D] TMX-DRV-014: When `list_panes` is called for a window target that does not exist, it shall return `Err(TmuxError::PaneNotFound(target.to_string()))`.
- [D] TMX-DRV-015: `capture_pane(pane_id, CaptureRange::Tail { lines })` shall run `tmux capture-pane -t <pane_id> -p -S -<lines> -J` and return a `Capture` with `text`, `truncated`, `original_bytes`, `returned_bytes`, `range_requested`, and `range_returned` fields.
- [D] TMX-DRV-016: `capture_pane(pane_id, CaptureRange::Window { start, end })` shall run `tmux capture-pane -t <pane_id> -p -S <start> -E <end> -J`; when `start == i32::MIN`, the `-S` argument shall be the literal `-`.
- [D] TMX-DRV-017: When `CaptureRange::Window { start, end }` has `start > end`, `capture_pane` shall return `Err(TmuxError::Other("invalid range".to_string()))` without invoking tmux.
- [D] TMX-DRV-018: When captured output exceeds 64 KiB, `capture_pane` shall truncate to the most recent bytes at a UTF-8 boundary, set `truncated = true`, and prepend the truncation marker line.
- [D] TMX-DRV-019: When captured output exceeds 256 KiB, `capture_pane` shall apply the hard cap regardless of the requested range or caller-supplied `lines`.
- [D] TMX-DRV-020: When `capture_pane` is called for a pane that no longer exists, it shall return `Err(TmuxError::PaneNotFound(pane_id.to_string()))`.
- [D] TMX-DRV-021: `send_keys(pane_id, text)` shall issue two calls: `tmux send-keys -t <pane_id> -l -- <text>`, then `tmux send-keys -t <pane_id> Enter`.
- [D] TMX-DRV-022: When the text send call succeeds but the Enter call fails, `send_keys` shall return `Err(TmuxError::SendKeysIncomplete(pane_id.to_string()))`.
