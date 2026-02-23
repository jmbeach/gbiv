## Context

The `gbiv tmux` command group already has `new-session`, which creates one tmux window per ROYGBIV color. Over time, features are removed from `GBIV.md`, leaving stale windows. The `clean` subcommand reclaims them by closing windows whose color has no tagged feature.

The existing `parse_gbiv_md` function in `src/gbiv_md.rs` returns `Vec<GbivFeature>` where each feature may have an `Option<String>` tag. The `new_session.rs` file serves as the implementation pattern to follow: guard → find root → derive session name → shell out to tmux.

## Goals / Non-Goals

**Goals:**
- Close tmux windows (in the project session) whose name is a ROYGBIV color with no corresponding `[color]`-tagged feature in `GBIV.md`
- Follow the same guard sequence and code structure as `new_session.rs`
- Print a summary of which windows were closed (or a "nothing to clean" message)

**Non-Goals:**
- Do not close the `main` window — it is always kept regardless of `GBIV.md` contents
- Do not create missing windows; this command only removes
- Do not operate on sessions other than the project session (no `--session-name` flag for now)

## Decisions

### Use `tmux list-windows -F "#{window_name}"` to enumerate windows

Alternatives considered:
- Parsing `tmux ls` output — less reliable, session-level not window-level
- Using a tmux plugin — unnecessary dependency

`list-windows -t <session> -F "#{window_name}"` gives one window name per line, easy to collect into a `Vec<String>`.

### Match windows against COLORS constant, then filter by GBIV.md tags

Only windows whose name appears in the `COLORS` array (`src/colors.rs`) are eligible for cleanup. This prevents accidentally killing user-created windows with arbitrary names.

The set of "active" colors is built by collecting the `tag` field from all `GbivFeature` entries that have a tag. A window is closed if `COLORS.contains(window_name) && !active_colors.contains(window_name)`.

### Use `tmux kill-window -t <session>:<name>` to close each orphaned window

Alternatives considered:
- `tmux kill-pane` — pane-level, more complex
- Moving windows to a hidden session — unnecessary complexity

`kill-window` is the direct inverse of `new-window` used in `new_session.rs`.

### Error if session does not exist

If `tmux has-session -t <name>` fails, exit with an error pointing the user to `gbiv tmux new-session`. A missing session means there is nothing to clean — a silent no-op would mask mistakes.

## Risks / Trade-offs

- **Risk**: User has a window named `red` that is not a gbiv worktree window → **Mitigation**: Only windows in the project-named session are touched; documented as expected behavior.
- **Risk**: Partial kill (tmux exits mid-loop) → **Mitigation**: Each `kill-window` is independent; continue on per-window failure, emit a warning.

## Migration Plan

No data migration needed. The command is purely additive. Rollback = don't call `gbiv tmux clean`.
