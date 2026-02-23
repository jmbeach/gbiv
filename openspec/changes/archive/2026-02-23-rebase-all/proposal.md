## Why

Working across multiple gbiv-managed worktrees means each branch can fall behind `origin/main`. Currently there is no way to update all worktrees at once — users must manually switch to each worktree and run `git rebase origin/main` individually.

## What Changes

- Add a new `rebase-all` subcommand to `gbiv`
- The command first runs `git pull` on the `main` worktree to bring it up to date
- Then rebases each gbiv-managed worktree's current branch onto `origin/main`
- Outputs per-worktree success/failure status

## Capabilities

### New Capabilities
- `rebase-all`: Runs git pull on `main` worktree first, then iterates over all gbiv-managed worktrees (red, orange, yellow, green, blue, indigo, violet) and rebases each worktree's current branch onto `origin/main`

### Modified Capabilities
<!-- None -->

## Impact

- New file: `src/commands/rebase_all.rs`
- `src/commands/mod.rs`: expose new module
- `src/main.rs`: register `rebase-all` subcommand and dispatch handler
- No changes to existing commands or git utilities (may add a helper to `git_utils.rs` for running rebase)
