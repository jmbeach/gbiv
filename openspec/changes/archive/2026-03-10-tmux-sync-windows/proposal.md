## Why

When a user closes a tmux color window (e.g., Indigo) and later adds a new task tagged with that color in GBIV.md, there's no way to recreate just the missing windows. The only option is to destroy the entire session and recreate it. A sync command would non-destructively add missing windows and reorder them to maintain ROYGBIV order.

## What Changes

- Add a new `gbiv tmux sync` subcommand that:
  - Reads GBIV.md to determine which colors have active tasks
  - Compares active colors against existing tmux windows in the session
  - Creates any missing color windows that have tasks in GBIV.md
  - Reorders all color windows to maintain ROYGBIV order (main, red, orange, yellow, green, blue, indigo, violet)
  - Preserves existing windows and their state (no windows are killed)

## Capabilities

### New Capabilities
- `tmux-sync`: Sync tmux windows to match active GBIV.md colors, creating missing windows and reordering to ROYGBIV order.

### Modified Capabilities

## Impact

- New file: `src/commands/tmux/sync.rs`
- Modified: `src/commands/tmux/mod.rs` (add sync subcommand)
- Modified: `src/main.rs` (wire up sync dispatch)
- Uses existing: `gbiv_md.rs` parsing, `colors.rs` ordering, `git_utils.rs` root detection
