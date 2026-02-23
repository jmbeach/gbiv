## Why

The `rebase-all` command fails when a `.last-branch` file exists as an untracked file inside a worktree. When `rebase-all` attempts a `git checkout` (or `git switch --detach`) in that worktree, git refuses because the untracked `.last-branch` file would be overwritten, aborting the operation with a non-zero exit code.

## What Changes

- Add a `rebase-all` subcommand to `gbiv` that rebases all ROYGBIV worktrees against the main branch.
- Before performing any checkout/detach in a worktree, detect and temporarily remove `.last-branch` (or any other `gbiv`-managed state files); restore it afterwards.
- Alternatively, add `.last-branch` to a per-worktree `.git/info/exclude` (or a root-level `.gitignore`) so git treats it as ignored and never flags it as an untracked file that would be overwritten.

## Capabilities

### New Capabilities
- `rebase-all`: Iterates over each ROYGBIV worktree and rebases it against the upstream main branch, handling `gbiv`-managed state files (e.g. `.last-branch`) that are untracked by git.

### Modified Capabilities
<!-- No existing spec-level requirements are changing. -->

## Impact

- **New file**: `src/commands/rebase_all.rs`
- **Modified file**: `src/commands/mod.rs` — expose the new module
- **Modified file**: `src/main.rs` — register the `rebase-all` subcommand
- **Modified file**: `src/git_utils.rs` — may gain helpers for checkout/rebase operations
- No external dependencies added; uses existing `std::process::Command` + git invocations
