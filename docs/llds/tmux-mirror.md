# Tmux Mirror

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

The tmux mirror maintains a tmux session whose windows correspond to the gbiv worktree layout. Each window is named after a color (or `main`), has its working directory set to that worktree's repo, and is ordered in canonical ROYGBIV sequence. The developer switches between feature branches by selecting tmux windows rather than `cd`-ing between directories.

Three subcommands manage the session lifecycle:
- `new-session` — creates the initial session
- `sync` — adds missing windows and reorders to ROYGBIV
- `clean` — removes orphaned windows

All three are behind the `gbiv tmux` subcommand group. `gbiv tmux` alone prints help.

## Session Architecture

```
tmux session: "myproject"
┌─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────┐
│ 0: main │ 1: red  │ 2: orange│ 3: yellow│ 4: green│ 5: blue │ 6: indigo│7: violet│
│ cwd:    │ cwd:    │ cwd:     │ cwd:     │ cwd:    │ cwd:    │ cwd:     │ cwd:    │
│ main/   │ red/    │ orange/  │ yellow/  │ green/  │ blue/   │ indigo/  │ violet/ │
│ repo    │ repo    │ repo     │ repo     │ repo    │ repo    │ repo     │ repo    │
└─────────┴─────────┴─────────┴─────────┴─────────┴─────────┴─────────┴─────────┘
```

- **One session per gbiv project**, named after the project folder (or overridden with `--session-name`)
- **One window per worktree** that exists on disk
- **Window names = color names** (or `main`)
- **Canonical order**: main first, then red → orange → yellow → green → blue → indigo → violet, then any non-ROYGBIV windows preserving their relative order

## New Session

`gbiv tmux new-session [--session-name <NAME>]`

### Preconditions
- tmux is installed (checked via `tmux -V`)
- CWD is inside a gbiv project
- No session with the target name already exists

### Steps
1. Determine session name: explicit `--session-name` or gbiv folder name
2. Enumerate worktree paths: `main`, then each color in ROYGBIV order
3. Filter to paths that exist on disk; warn for missing ones
4. Require at least one path exists
5. Create detached session with first worktree: `tmux new-session -d -s <name> -n <first> -c <path>`
6. Add remaining worktrees: `tmux new-window -t <name> -n <color> -c <path>`
7. Print confirmation with window count

### Session Name
Defaults to `gbiv_root.folder_name` — the name of the repo directory inside `main/`. This means two gbiv projects with the same repo name would collide. The `--session-name` flag is the escape hatch.

## Sync

`gbiv tmux sync [--session-name <NAME>]`

Ensures tmux windows match active GBIV.md entries and are in canonical order.

### Steps
1. Verify tmux, gbiv root, session exists
2. List existing windows: `tmux list-windows -t <session> -F #{window_name}`
3. Parse GBIV.md, extract active colors (entries with a valid ROYGBIV color tag)
4. Compute missing windows: active colors that don't have a window yet
5. For each missing color:
   - Verify worktree directory exists (skip if not)
   - `tmux new-window -t <session> -n <color> -c <worktree_path>`
6. Re-list windows (now includes newly created ones)
7. Reorder to canonical ROYGBIV order via two-pass move

### Window Reordering

The reorder uses a two-pass algorithm to avoid index collisions:

**Pass 1**: Move all windows to temporary high indices (offset +1000)
```
main → 1000, red → 1001, blue → 1002, ...
```

**Pass 2**: Move from temporary indices to final positions
```
1000 → 0 (main), 1001 → 1 (red), 1002 → 5 (blue), ...
```

This avoids the problem where moving window A to index 3 would collide with window B already at index 3.

### Sort Order

`sort_windows_roygbiv()` produces the canonical order:
1. `main` (always first)
2. ROYGBIV colors in order (red, orange, yellow, green, blue, indigo, violet)
3. Any non-ROYGBIV windows (e.g., user-created windows) preserving their relative order at the end

### Active Color Detection

`active_colors_from_features()` extracts colors from parsed GBIV.md features:
- Only includes entries whose `tag` is a valid ROYGBIV color
- Returns a `HashSet<String>` for O(1) lookup
- Entries without tags (backlog) are ignored

## Clean

`gbiv tmux clean` — no arguments.

Closes windows for colors that have no corresponding GBIV.md entry.

### Steps
1. Verify tmux, gbiv root, session exists
2. List existing windows
3. Parse GBIV.md, extract active color tags into a `HashSet`
4. For each window, check if orphaned:
   - Window name is a valid ROYGBIV color AND not in the active set → orphaned
   - Window name is `main` or non-ROYGBIV → never orphaned
5. For each orphaned window: `tmux kill-window -t <session>:<name>`
6. Print per-window confirmation or "Nothing to clean."

### Integration with Tidy

`gbiv tidy` calls `clean_command()` as its third and final step, after rebase-all and reset. This means:
- After reset removes `[done]` entries from GBIV.md, clean removes the corresponding tmux windows
- The sequence is: rebase-all (sync git) → reset (reclaim worktrees + update GBIV.md) → clean (sync tmux)
- Tidy skips the clean step entirely if tmux is not installed

## Tmux Commands Used

| gbiv operation | tmux command |
|---|---|
| Check tmux available | `tmux -V` |
| Check session exists | `tmux has-session -t <name>` |
| Create session | `tmux new-session -d -s <name> -n <window> -c <path>` |
| Add window | `tmux new-window -t <session> -n <window> -c <path>` |
| List windows | `tmux list-windows -t <session> -F #{window_name}` |
| Move window | `tmux move-window -s <session>:<from> -t <session>:<to>` |
| Kill window | `tmux kill-window -t <session>:<window>` |

All tmux interaction is via `std::process::Command` — no tmux library or API.

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| One session per project | Session named after folder | One session with all projects, nested sessions | Simple mapping. Multiple projects = multiple sessions. `tmux switch-client` navigates between projects. |
| Windows named by color | `red`, `orange`, etc. | Numbered, branch-named | Color names are stable identifiers. Branch names change as features rotate. |
| Detached session creation | `-d` flag, user attaches later | Create and attach immediately | Allows scripted setup. User may want to create session in one terminal, attach in another. |
| Two-pass reorder | Temporary high indices | In-place swaps, destroy+recreate | Avoids index collision. Simple to reason about. Costs 2N tmux commands but N ≤ 9 so irrelevant. |
| Clean only kills ROYGBIV windows | Non-color windows are untouched | Clean all non-active windows | Users may create custom tmux windows (e.g., `htop`, `logs`). Killing those would be destructive. |
| Sync creates but doesn't kill | Sync adds missing, clean removes orphaned | Sync does both | Separation of concerns. `tidy` composes them. Running `sync` alone is safe — it never removes. |

## Technical Debt & Inconsistencies

1. **No `--session-name` on clean**: `new-session` and `sync` accept `--session-name` but `clean` doesn't — it always uses the folder name. If you created a session with a custom name, `clean` won't find it.

2. **Sync and clean have slightly different active-color logic**: `sync` uses `active_colors_from_features()` which builds a `HashSet`. `clean` builds its own `HashSet` inline with similar but not identical logic. Both extract color tags from `parse_gbiv_md()`, but the helper function only exists in `sync.rs`.

3. **No session lifecycle management**: There's `new-session` but no `delete-session` or `rename-session`. Users must use raw tmux commands to tear down a session.

## Behavioral Quirks

1. **Sync reorders even if no windows were created**: The reorder step always runs, even if the window set hasn't changed. This is harmless but means `gbiv tmux sync` always produces "Windows reordered" output.

2. **New-session skips missing worktrees but clean doesn't know about them**: If `red/` directory doesn't exist, `new-session` skips the `red` window. Later, if someone adds `- [red] task` to GBIV.md, `sync` will try to create a `red` window and skip it (no worktree). `clean` will never see it as orphaned because the window was never created.

3. **Window names are case-sensitive**: tmux window names are case-sensitive. gbiv always uses lowercase color names. A user manually renaming a window to `Red` would make it invisible to sync/clean.

4. **Session name collision**: Two gbiv projects with the same repo folder name (e.g., both called `app`) would try to create sessions with the same name. The second `new-session` would fail with "session exists."

## References

- `src/commands/tmux/mod.rs` — subcommand group declaration
- `src/commands/tmux/new_session.rs` — session creation
- `src/commands/tmux/sync.rs` — window sync + reorder
- `src/commands/tmux/clean.rs` — orphaned window cleanup
