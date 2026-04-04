## Why

Running `gbiv rebase-all`, `gbiv reset`, and `gbiv tmux clean` in sequence is a common maintenance workflow to bring all worktrees up to date, clean up finished branches, and remove orphaned tmux windows. A single `gbiv tidy` command eliminates the repetitive typing and ensures the full cleanup is always performed consistently.

## What Changes

- Add a new `tidy` command that sequentially runs:
  1. `rebase-all` — pull main and rebase all color worktrees
  2. `reset` — reset worktrees whose branches have been merged
  3. `tmux clean` — close orphaned tmux windows
- Each step prints its normal output so the user sees full progress
- If any step fails, the command reports the error but continues with remaining steps

## Capabilities

### New Capabilities
- `tidy`: Composite command that orchestrates rebase-all, reset, and tmux clean in sequence

### Modified Capabilities

## Impact

- New file: `src/commands/tidy.rs`
- Modified: `src/commands/mod.rs` (add module declaration)
- Modified: `src/main.rs` (register subcommand + dispatch)
- No new dependencies required — reuses existing command functions
