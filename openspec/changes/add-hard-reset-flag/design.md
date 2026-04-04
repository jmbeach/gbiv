## Context

The `gbiv reset` command currently resets color worktrees back to the remote main branch after a feature branch has been merged. It has two safety checks:
1. Single-color reset (`reset_one`): verifies the feature branch is merged into remote main via `git merge-base --is-ancestor`
2. All-color reset (`reset_all_to_vec`): additionally requires `[done]` status in GBIV.md and a GBIV.md entry

These checks prevent accidental loss of unmerged work, but sometimes users need to force-reset regardless — abandoned branches, squash-merged PRs where ancestry detection fails, worktrees that were never tracked in GBIV.md, or simply wanting a clean slate.

## Goals / Non-Goals

**Goals:**
- Allow users to force-reset a color worktree with `--hard`, bypassing merge, status, and GBIV.md entry checks
- Stash uncommitted changes (including untracked files) before resetting as a safety net
- Support both single-color and all-color modes
- Show a confirmation prompt for all-color `--hard` (with `--yes`/`-y` to skip)
- Maintain the existing safe behavior as the default (no flag = current behavior)

**Non-Goals:**
- Changing the default (non-hard) reset behavior
- Adding stashing to non-hard resets
- Implicit `git fetch` before resetting (stay consistent with current behavior)

## Decisions

**1. Flag name: `--hard`**
Mirrors `git reset --hard` semantics that Rust/git users already understand. Considered `--force` but `--hard` more precisely describes the operation (hard reset to remote main).

**2. Thread the `hard` boolean through existing functions**
Add a `hard: bool` parameter to `reset_one` and `reset_all_to_vec` rather than creating separate functions. The logic difference is small (skip checks, add stash), so branching within the existing flow is cleaner.

**3. In all-color mode with `--hard`, reset ALL 7 worktrees**
Skip both the `[done]` filter and the GBIV.md entry requirement. Iterate all color worktrees unconditionally. This covers the use case of worktrees that were never tracked in GBIV.md.

**4. Reset even when already on color branch**
When `--hard` is set, do not skip worktrees that are already on the color branch. Run `git reset --hard origin/main` regardless — the user wants a clean slate.

**5. Stash before checkout, only when dirty**
Before checking out the color branch, check if the worktree has uncommitted changes (via `get_quick_status`). If dirty, run `git stash push -u -m "gbiv hard-reset: <branch> on <color> worktree"`. This preserves work as a safety net. If the stash fails, abort the reset for that worktree and report the error — the user can resolve and retry.

**6. Confirmation prompt for all-color `--hard`**
Show each color worktree with its current branch and ask `Continue? [y/N]`. Default to No. Add `--yes`/`-y` flag to bypass the prompt for scripting. Single-color `--hard` does not prompt — the user already specified exactly what they want.

**7. No implicit fetch**
Consistent with current behavior. The user can `gbiv rebase-all` or `git fetch` first.

**8. Continue through failures**
If a worktree fails to reset (after stash succeeds), continue with the remaining worktrees and report failures in the summary. Same pattern as current all-color reset.

## Risks / Trade-offs

- [Data loss from force-reset] → Mitigated by stashing uncommitted changes before reset, confirmation prompt on all-color mode, and the explicit `--hard` flag as opt-in.
- [Stash cluttering reflog] → Stash messages include context (`gbiv hard-reset: <branch> on <color> worktree`) so they're identifiable. Only created when there are actual changes.
- [Stash failure blocks reset] → By design. If we can't save the user's work, we don't destroy it. User can resolve the issue and retry.
