## Why

The `gbiv cleanup` command calls `git pull origin <color>` after checking out the color branch. Color branches are local-only worktree branches and should never exist on the remote, so this pull is incorrect. It causes cleanup to fail with `fatal: couldn't find remote ref <color>`, leaving the worktree uncleaned and skipping the GBIV.md update.

## What Changes

- Replace the `pull_remote` call in `cleanup_one` with a `git reset --hard origin/main` (or whatever the remote main branch is) so the color branch is reset to the latest main, ready for the next feature

## Capabilities

### New Capabilities
- `cleanup-reset-to-main`: After checking out the color branch during cleanup, reset it to origin/main instead of pulling from the (non-existent) remote color branch

### Modified Capabilities

## Impact

- `src/commands/cleanup.rs`: Replace `pull_remote` call with a hard reset to the remote main branch
- `src/git_utils.rs`: Potentially add a `reset_hard` helper function
