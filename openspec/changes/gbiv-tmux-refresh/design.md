## Context

`gbiv tmux new-session` creates a fixed set of 8 windows (main + all ROYGBIV colors). There is no way to update an existing session as GBIV.md evolves — users must kill and recreate the session. The `refresh` command fills this gap by operating on an existing session and syncing its windows to match the active color set from GBIV.md.

The project already has `parse_gbiv_md` in `src/gbiv_md.rs` for reading tagged features, and `find_gbiv_root` in `src/git_utils.rs` for project detection. The `new-session` command in `src/commands/tmux/new_session.rs` provides the pattern for tmux interaction.

## Goals / Non-Goals

**Goals:**
- Ensure a running tmux session has one window per active color (colors with at least one tagged feature in GBIV.md), plus `main`
- Create missing windows; skip windows that already exist
- Reorder all managed windows to canonical ROYGBIV order after ensuring they exist
- Fail fast with clear errors if preconditions aren't met

**Non-Goals:**
- Creating the tmux session (session must already exist)
- Removing windows for colors that are no longer active
- Accepting a `--session-name` flag (session name is always the folder name)
- Modifying window contents or running commands inside windows

## Decisions

### Use `tmux move-window` for reordering
tmux windows have indices, and `move-window -t <session>:<index>` can reposition them. After all windows are ensured to exist, iterate the desired order and move each window to the next sequential index.

**Alternative considered**: Delete and recreate windows in order. Rejected — destroys any running shell state in existing windows.

### Collect active colors from GBIV.md before touching tmux
Parse GBIV.md once up front, build the ordered list of active colors (`main` always included), then perform all tmux operations. This keeps the tmux interaction simple and predictable.

**Alternative considered**: Query existing tmux windows and diff against GBIV.md. Unnecessary — creating a window that already exists is detectable via `tmux has-session`-style checks, but simpler to just check the window name list first.

### Check for existing window by name before creating
Use `tmux list-windows -t <session> -F '#{window_name}'` to get existing window names, then skip `new-window` for any name already present.

### Session name is always the folder name
Unlike `new-session`, `refresh` has no `--session-name` flag. The command is specifically about keeping the canonical session (named after the project) in sync.

## Risks / Trade-offs

- **Window name collisions**: If the user has manually renamed a color window, `refresh` won't match it and will create a duplicate. → Mitigation: documented behavior; refresh only manages windows by their canonical color names.
- **Index gaps after move-window**: tmux may leave index gaps when windows are moved. → Mitigation: use `move-window -r` (renumber) at the end, or simply move in order starting at index 0.
- **Session exists but target window belongs to a different pane layout**: Not a concern — `new-window` creates a fresh window with a new shell, it doesn't touch existing panes.
