## Why

The current `gbiv reset` command has safety checks that prevent resetting a worktree if the feature branch isn't merged into remote main. Sometimes you need to force-reset a worktree regardless — e.g., when abandoning work, when a branch was squash-merged and merge-base detection fails, or when you just want a clean slate.

## What Changes

- Add a `--hard` flag to `gbiv reset` that bypasses the merge check and force-resets the color worktree
- When `--hard` is used: stash uncommitted changes (including untracked files), checkout the color branch, then `git reset --hard` to the remote main branch (e.g., `origin/main`), regardless of merge status
- Works with both single-color (`gbiv reset red --hard`) and all-color (`gbiv reset --hard`) modes
- When resetting all with `--hard`, resets ALL 7 color worktrees — bypasses both the `[done]` status requirement and the GBIV.md entry requirement
- All-color `--hard` shows a confirmation prompt listing affected worktrees before proceeding (defaults to No)
- Add `--yes`/`-y` flag to skip the confirmation prompt (for scripting)
- Single-color `--hard` does not prompt
- If stash fails for a worktree, abort the reset for that worktree (user can resolve and retry)

## Capabilities

### New Capabilities
- `hard-reset`: Force-reset color worktrees to remote main, bypassing merge and status checks

### Modified Capabilities

## Impact

- `src/main.rs`: Add `--hard` and `--yes`/`-y` flags to reset command args
- `src/commands/reset.rs`: Pass hard flag through `reset_command`, `reset_one`, and `reset_all_to_vec`; skip merge check, `[done]` check, and GBIV.md entry check when hard is true; add stash-before-reset logic and confirmation prompt
- `src/git_utils.rs`: Add `stash_push` helper function
- No external dependency changes
