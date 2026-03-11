## Why

The `gbiv cleanup` command gives no output for worktrees that are actually cleaned up (checked out back to color branch and reset). It also doesn't show the active branch name when reporting skipped worktrees. Users can't tell what happened to cleaned-up worktrees or what branch a worktree was on before being skipped.

## What Changes

- Add a success message when a worktree is cleaned up, showing the previous branch and that it was reset (e.g., "red worktree cleaned up (was on feature-branch), reset to origin/main")
- Include the active branch name in the "already on color branch" skip message (e.g., "red worktree is already on the red branch, skipping")

## Capabilities

### New Capabilities
- `cleanup-output`: Improved output messages for the cleanup command covering both cleaned-up and skipped worktrees

### Modified Capabilities

## Impact

- `src/commands/cleanup.rs`: `cleanup_one` function — add println for successful cleanup, modify existing skip message to include branch info
