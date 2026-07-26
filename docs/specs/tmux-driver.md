# tmux Driver

Specs for the shared `core::tmux` primitives (implemented in `gbiv-core`) and the
orchestration-only tmux driver operations (implemented in the `gbiv` binary's
`orchestration::tmux_driver` module).

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

- [x] TMX-DRV-023: `PaneInfo` shall be a struct with public fields `id: String` (tmux pane ID, e.g. `%12`), `pid: u32` (pane process PID), `current_command: String`, and `current_path: String`.
- [x] TMX-DRV-013: `list_panes(window_target)` shall run `tmux list-panes -t <window_target> -F '#{pane_id}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}'` and return `Vec<PaneInfo>`.
- [x] TMX-DRV-014: When `list_panes` is called for a window target that does not exist, it shall return `Err(TmuxError::PaneNotFound(target.to_string()))`.
- [x] TMX-DRV-024: When a line in `list_panes` output has fewer than four tab-separated fields or a non-numeric pid, the system shall return `Err(TmuxError::Other(...))` including the malformed line.
- [x] TMX-DRV-015: `capture_pane(pane_id, CaptureRange::Tail { lines }, max_bytes)` shall run `tmux capture-pane -t <pane_id> -p -S -<lines> -J` and return a `Capture` with `text`, `truncated`, `original_bytes`, `returned_bytes`, `range_requested`, and `range_returned` fields.
- [x] TMX-DRV-016: `capture_pane(pane_id, CaptureRange::Window { start, end }, max_bytes)` shall run `tmux capture-pane -t <pane_id> -p -S <start> -E <end> -J`; when `start == i32::MIN`, the `-S` argument shall be the literal `-`.
- [x] TMX-DRV-017: When `CaptureRange::Window { start, end }` has `start > end`, `capture_pane` shall return `Err(TmuxError::Other("invalid range".to_string()))` without invoking tmux.
- [x] TMX-DRV-018: When captured output exceeds the requested `max_bytes` cap (default `DEFAULT_CAP_BYTES` = 64 KiB), `capture_pane` shall truncate to the most recent bytes at a UTF-8 boundary, set `truncated = true`, and prepend the truncation marker line.
- [x] TMX-DRV-019: When the requested `max_bytes` exceeds `HARD_MAX_BYTES` (256 KiB), `capture_pane` shall clamp the effective cap to 256 KiB regardless of the requested range or caller-supplied `lines`.
- [x] TMX-DRV-020: When `capture_pane` is called for a pane that no longer exists, it shall return `Err(TmuxError::PaneNotFound(pane_id.to_string()))`.
- [x] TMX-DRV-021: `send_keys(pane_id, text)` shall issue two calls: `tmux send-keys -t <pane_id> -l -- <text>`, then `tmux send-keys -t <pane_id> Enter`.
- [x] TMX-DRV-022: When the text send call succeeds but the Enter call fails, `send_keys` shall return `Err(TmuxError::SendKeysIncomplete(pane_id.to_string()))`.
