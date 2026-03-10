## Context

The gbiv CLI manages 7 color-named git worktrees with tmux integration. Currently, `gbiv tmux new-session` creates all 8 windows (main + 7 colors) at once, and `gbiv tmux clean` removes windows for colors without GBIV.md tasks. There is no command to add back windows that were cleaned or closed without recreating the entire session.

The existing tmux commands use `std::process::Command` to shell out to tmux. Window listing uses `tmux list-windows -t <session> -F '#{window_name}'`. The GBIV.md parser (`gbiv_md.rs`) extracts color tags from features. The color ordering is defined in `colors.rs::COLORS`.

## Goals / Non-Goals

**Goals:**
- Add `gbiv tmux sync` subcommand that creates missing color windows for active GBIV.md tasks
- Reorder windows to maintain ROYGBIV order after creating new ones
- Preserve existing windows and their running processes
- Follow the same patterns as existing tmux subcommands (guards, error handling)

**Non-Goals:**
- Killing/removing windows (that's `gbiv tmux clean`'s job)
- Creating windows for colors without GBIV.md tasks
- Managing the `main` window (assumed to always exist)

## Decisions

**1. Determine active colors from GBIV.md**
Use existing `parse_gbiv_md()` to get features, extract unique color tags that are valid ROYGBIV colors. This is the same approach used by `tmux clean`.

**2. Window creation approach**
Use `tmux new-window -t <session> -n <color> -c <worktree_path>` for each missing color window, same as `new_session.rs` does. Skip colors whose worktree directories don't exist on disk (with a warning, matching existing behavior).

**3. Window reordering via `tmux move-window`**
After creating missing windows, reorder all color windows to match ROYGBIV sequence. Use `tmux move-window -s <session>:<window_name> -t <session>:<index>` to place each window at the correct position. The order is: main (index 0), then red (1), orange (2), yellow (3), green (4), blue (5), indigo (6), violet (7). Only move windows that exist in the session.

**4. Session detection**
Reuse the same session-name resolution as other tmux commands: default to the gbiv folder name, allow `--session-name` override.

## Risks / Trade-offs

- [Window index conflicts during reordering] → Use `swap-window` or sequential `move-window` with a temporary high index to avoid collisions. Alternatively, use `move-window -r` to renumber after positioning.
- [User has renamed windows] → Only operate on windows whose names match ROYGBIV colors or "main". Custom-named windows are left untouched.
