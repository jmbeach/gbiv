## Why

After running `gbiv tmux new-session`, users accumulate tmux windows for features that have since been removed from `GBIV.md`. There is no automated way to prune these stale windows, requiring users to close them manually.

## What Changes

- Add `gbiv tmux clean` subcommand to the `tmux` command group
- The command inspects the active tmux session (defaulting to the project folder name), finds windows whose names match a ROYGBIV color, and closes any window whose color has no feature tagged with that color in `GBIV.md`

## Capabilities

### New Capabilities
- `tmux-clean`: Closes orphaned tmux windows in the gbiv session — windows named after a ROYGBIV color that have no feature tagged with that color in `GBIV.md`

### Modified Capabilities
_(none — no existing spec requirements change)_

## Impact

- **New file**: `src/commands/tmux/clean.rs`
- **Modified**: `src/commands/tmux/mod.rs` — registers `clean` subcommand and dispatches it
- **Reads**: `GBIV.md` via existing `gbiv_md` parser (`src/gbiv_md.rs`)
- **Invokes**: `tmux kill-window` for each orphaned window
- **Depends on**: tmux being installed and a session with the project name existing
