## Context

The `cleanup_one` function in `src/commands/cleanup.rs` performs these steps:
1. Check if worktree is already on the color branch (skip if so)
2. Verify branch is merged into remote main
3. `checkout_branch` to the color branch
4. `pull_remote` from origin for the color branch
5. Remove GBIV.md entries

Step 4 is incorrect. Color branches are local-only worktree branches that don't exist on the remote. The `pull_remote` call always fails. Instead, the color branch should be reset to `origin/main` so it's clean and up-to-date for the next feature.

The remote main branch ref is already resolved in step 2 via `get_remote_main_branch`, so we can reuse that value.

## Goals / Non-Goals

**Goals:**
- Replace `pull_remote` with `git reset --hard <remote_main>` to reset the color branch to origin/main
- Reuse the `remote_main` value already resolved earlier in the function

**Non-Goals:**
- Changing any other cleanup behavior

## Decisions

**Decision: Use `git reset --hard <remote_main>` after checkout**

After checking out the color branch, run `git reset --hard origin/main` (using the already-resolved remote main ref). This puts the color branch at the same commit as main, ready for the next feature. We already have `remote_main` from the merge check, so no extra lookups needed.

**Decision: Add a `reset_hard` helper to `git_utils.rs`**

Follows the existing pattern of thin wrappers around git commands (`checkout_branch`, `pull_remote`, etc.).

## Risks / Trade-offs

- [Risk: Uncommitted work in the worktree could be lost] → Mitigation: The function already verifies the branch is merged into main before proceeding, so no unmerged work should be present. The `--hard` reset is intentional here.
