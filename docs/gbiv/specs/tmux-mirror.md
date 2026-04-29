# Tmux Mirror

Specs for tmux session creation, window synchronization, and cleanup.

**Component LLD**: `docs/gbiv/llds/tmux-mirror.md`

## New Session

- [x] TMX-SESSION-001: When `gbiv tmux new-session` is invoked and tmux is not installed, the system shall return an error containing "tmux not found".
- [x] TMX-SESSION-002: When `gbiv tmux new-session` is invoked outside a gbiv project, the system shall return an error mentioning `gbiv init`.
- [x] TMX-SESSION-003: When `gbiv tmux new-session` is invoked and a tmux session with the resolved name already exists, the system shall return an error naming the session and suggesting `tmux attach` or `--session-name`.
- [x] TMX-SESSION-004: When `--session-name <NAME>` is provided, the system shall use NAME as the tmux session name.
- [x] TMX-SESSION-005: When `--session-name` is omitted, the system shall use the gbiv folder name as the tmux session name.
- [x] TMX-SESSION-006: When enumerating worktree paths, the system shall consider "main" first, followed by the seven ROYGBIV colors in canonical order (red, orange, yellow, green, blue, indigo, violet).
- [x] TMX-SESSION-007: When a worktree path does not exist on disk, the system shall print a warning to stderr naming the color and path, and exclude it from the session.
- [x] TMX-SESSION-008: When no worktree paths exist, the system shall return an error stating it cannot create a tmux session.
- [x] TMX-SESSION-009: When at least one worktree path exists, the system shall create a detached tmux session (`tmux new-session -d`) using the first existing path, with the window named after its color slot.
- [x] TMX-SESSION-010: For each additional existing worktree path, the system shall create a new window (`tmux new-window`) in the session, named after its color slot, with the working directory set to the worktree path.
- [x] TMX-SESSION-011: When the session is created successfully, the system shall print a confirmation message including the session name and window count.
- [x] TMX-SESSION-012: When `tmux new-session` fails (non-zero exit), the system shall return an error including the exit status.
- [x] TMX-SESSION-013: When `tmux new-window` fails for any additional path, the system shall return an error immediately without creating remaining windows.

## Sync

- [x] TMX-SYNC-001: When `gbiv tmux sync` is invoked and tmux is not installed, the system shall return an error containing "tmux not found".
- [x] TMX-SYNC-002: When `gbiv tmux sync` is invoked outside a gbiv project, the system shall return an error mentioning `gbiv init`.
- [x] TMX-SYNC-003: When `gbiv tmux sync` is invoked and no tmux session with the resolved name exists, the system shall return an error suggesting `gbiv tmux new-session`.
- [x] TMX-SYNC-004: When `--session-name <NAME>` is provided, the system shall use NAME as the session name.
- [x] TMX-SYNC-005: When `--session-name` is omitted, the system shall use the gbiv folder name as the session name.
- [x] TMX-SYNC-006: When listing existing windows, the system shall query tmux with `list-windows -F #{window_name}`.
- [x] TMX-SYNC-007: When parsing GBIV.md, the system shall extract the set of active colors by collecting tags that are valid ROYGBIV color names, deduplicating them.
- [x] TMX-SYNC-008: When a ROYGBIV color is active in GBIV.md but has no corresponding tmux window, the system shall identify it as a missing window.
- [x] TMX-SYNC-009: When a missing window's worktree directory exists on disk, the system shall create a tmux window named after the color with its working directory set to the worktree path.
- [x] TMX-SYNC-010: When a missing window's worktree directory does not exist, the system shall print a warning and skip creating that window.
- [x] TMX-SYNC-011: When window creation fails for a color, the system shall print a warning to stderr and continue with remaining colors.
- [x] TMX-SYNC-012: After creating missing windows, the system shall reorder all windows to canonical order using a two-pass approach: first move all windows to temporary indices (offset +1000), then move them to final positions.
- [x] TMX-SYNC-013: When sorting windows, the system shall place "main" first, then ROYGBIV colors in canonical order (red, orange, yellow, green, blue, indigo, violet), then non-ROYGBIV windows preserving their relative order.
- [x] TMX-SYNC-014: When no new windows were created, the system shall print a message stating no new windows were created and that windows were reordered.
- [x] TMX-SYNC-015: When new windows were created, the system shall print a summary with the count and names of created windows, plus a reorder confirmation.

## Clean

- [x] TMX-CLEAN-001: When `gbiv tmux clean` is invoked and tmux is not installed, the system shall return an error containing "tmux not found".
- [x] TMX-CLEAN-002: When `gbiv tmux clean` is invoked outside a gbiv project, the system shall return an error mentioning `gbiv init`.
- [x] TMX-CLEAN-003: When `gbiv tmux clean` is invoked and no tmux session with the folder name exists, the system shall return an error suggesting `gbiv tmux new-session`.
- [x] TMX-CLEAN-004: The `clean` subcommand shall not accept a `--session-name` flag; it shall always derive the session name from the gbiv folder name.
- [x] TMX-CLEAN-005: When a window name is a valid ROYGBIV color AND is not present as any tag in GBIV.md, the system shall classify that window as orphaned.
- [x] TMX-CLEAN-006: When a window is named "main", the system shall never classify it as orphaned regardless of GBIV.md contents.
- [x] TMX-CLEAN-007: When a window name is not a valid ROYGBIV color (e.g., "bash", "htop"), the system shall never classify it as orphaned.
- [x] TMX-CLEAN-008: When orphaned windows exist, the system shall kill each one via `tmux kill-window -t <session>:<name>`.
- [x] TMX-CLEAN-009: When an orphaned window is successfully killed, the system shall print "Closed: <name>" to stdout.
- [x] TMX-CLEAN-010: When killing a window fails, the system shall print a warning to stderr and continue attempting to kill remaining orphaned windows.
- [x] TMX-CLEAN-011: When any window kill failed, the system shall return an error after processing all orphaned windows.
- [x] TMX-CLEAN-012: When no orphaned windows are found, the system shall print "Nothing to clean." and return Ok.
- [ ] TMX-CLEAN-013: The `clean` subcommand should accept `--session-name` for consistency with `new-session` and `sync`.
