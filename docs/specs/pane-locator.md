# Pane Locator

Specs for the orchestration Pane Locator, which maps a ROYGBIV color to the tmux
pane running Claude Code in that color's window. Implemented in the `gbiv`
binary's `orchestration::pane_locator` module, built on the `orchestration::tmux_driver`
operations (`list_panes`) and the shared `gbiv_core::tmux` primitives
(`list_windows`, `TmuxError`).

**Component LLD**: `docs/llds/pane-locator.md`

## Types

- [x] **PANE-LOC-001**: `Resolution` shall be an enum with variants `Ok { pane_id: String, other_pane_ids: Vec<String> }` (a claude pane was located), `NoWindow` (no tmux window exists for the color), and `NoClaudePane` (a window exists but no pane is running claude).
- [x] **PANE-LOC-002**: `LocatorError` shall be a typed error enum whose sole variant `TmuxSession(TmuxError)` wraps a `gbiv_core::tmux::TmuxError` via `#[from]`, and the locator shall never use `anyhow`.

## Window Resolution

- [x] **PANE-LOC-003**: When `locate_pane(session, color)` is called and `list_windows(session)` returns no window whose `name` equals `color`, the locator shall return `Ok(Resolution::NoWindow)`.
- [x] **PANE-LOC-004**: If `list_windows(session)` returns `Err(TmuxError::SessionNotFound(_))` (the gbiv tmux session itself is missing), then `locate_pane` shall return `Err(LocatorError::TmuxSession(_))` rather than any `Resolution` variant.
- [x] **PANE-LOC-005**: When a window whose `name` equals `color` exists, `locate_pane` shall enumerate that window's panes via `list_panes(format!("{session}:{color}"))`.
- [x] **PANE-LOC-022**: When more than one window's `name` equals `color`, `locate_pane` shall resolve panes from the first such window in `list_windows` order (the daemon creates exactly one window per color; a duplicate is a tolerated anomaly, not an error).
- [x] **PANE-LOC-023**: If `list_panes` returns an `Err` for a window that was found (the window or its panes vanished between `list_windows` and `list_panes`, or any other tmux failure), then `locate_pane` shall return `Err(LocatorError::TmuxSession(_))` rather than a `Resolution` variant.

## Claude-Pane Identification within a Window

- [x] **PANE-LOC-006**: When resolving a window's panes, the locator shall classify each pane as claude-running or not by calling the process-tree walk on that pane's `pid`.
- [x] **PANE-LOC-007**: When zero panes in the window are classified as claude-running, `locate_pane` shall return `Ok(Resolution::NoClaudePane)`.
- [x] **PANE-LOC-008**: When exactly one pane in the window is classified as claude-running, `locate_pane` shall return `Ok(Resolution::Ok { pane_id, other_pane_ids: [] })` for that pane.
- [x] **PANE-LOC-009**: When more than one pane is classified as claude-running, `locate_pane` shall order the claude panes ascending by the start time of the matching claude process (oldest first) and return `Ok(Resolution::Ok { pane_id: <oldest>, other_pane_ids: <the remaining pane ids in the same ascending order> })`.
- [x] **PANE-LOC-010**: When two claude panes have equal claude-process start times, the locator shall break the tie by lower `pid` first, then by lexicographically smaller pane id, so ordering is fully deterministic.
- [x] **PANE-LOC-011**: If the claude-process start time cannot be read for a claude pane (process exited mid-resolution, permission error), then the locator shall sort that pane to the back of the ordering, behind every pane whose start time is known.

## Process-Tree Walk

- [x] **PANE-LOC-012**: The process-tree walk shall return `true` when the root pid or any of its descendants has an executable path whose basename is exactly `claude` or `claude-code`, and `false` otherwise; the root pid itself is included in the check.
- [x] **PANE-LOC-013**: The walk shall match the executable basename case-sensitively, so a binary named `Claude` shall not be classified as claude.
- [x] **PANE-LOC-014**: The walk shall classify a process using its executable path only, ignoring the process's self-reported name, `argv[0]`, and tmux's `pane_current_command` (Claude Code rewrites its title to a version string and runs under `node`).
- [x] **PANE-LOC-015**: The walk shall visit every reachable descendant within its bounds rather than short-circuiting on the first claude match, so a wrapper or shell named `claude` does not mask a real claude deeper in the tree.
- [x] **PANE-LOC-016**: The walk shall be bounded to a depth of at most 8 and at most 64 total descendants visited, terminating without error when a bound is reached.
- [x] **PANE-LOC-017**: If the walk cannot read a process's children or executable (permission error, process exited, unreadable), then that pane shall be treated as non-claude rather than propagating an error — the locator never sends keystrokes to a pane it cannot prove is claude.
- [x] **PANE-LOC-018**: When a single pane's process tree contains more than one claude descendant, the pane shall be counted once, and the earliest claude descendant's start time shall represent that pane in the multi-pane ordering.

## Platform Mechanism

- [x] **PANE-LOC-019**: On macOS the walk shall build the child map via `ps -A -o pid=,ppid=` and resolve each process's full executable path via `ps -p <pid> -o comm=` (per-pid, since the bulk listing truncates `comm`), and read start time via `ps -p <pid> -o lstart=`.
- [x] **PANE-LOC-020**: On Linux the walk shall resolve each process's executable via `/proc/<pid>/exe`, enumerate descendants via `/proc/<pid>/stat` `ppid` fields, and read start time from field 22 of `/proc/<pid>/stat`.

## Caching

- [x] **PANE-LOC-021**: The locator shall re-resolve on every call and shall not cache resolutions, so pane state that changes between requests is always observed fresh.
