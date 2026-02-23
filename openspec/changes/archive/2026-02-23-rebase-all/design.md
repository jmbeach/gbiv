## Context

gbiv is a Rust CLI tool that manages a ROYGBIV git worktree structure. Each color (red, orange, yellow, green, blue, indigo, violet) is a separate worktree sharing the same underlying git repo, allowing parallel feature work across branches.

Currently there is no way to bulk-update worktrees. The proposed `rebase-all` command will: (1) pull the `main` worktree to bring the local main branch up to date, then (2) rebase every color worktree's current branch onto `origin/main`.

## Goals / Non-Goals

**Goals:**
- Add a `rebase-all` subcommand that updates all gbiv-managed worktrees in one command
- Pull `main` first so rebases target a fresh local `origin/main`
- Report per-worktree outcome (success or failure)
- Continue processing remaining worktrees after a rebase conflict

**Non-Goals:**
- Interactive conflict resolution — user must resolve conflicts manually
- Parallel execution — sequential is intentional for readable output and simpler error handling
- Pushing rebased branches to remote
- Operating on non-gbiv repos

## Decisions

**Sequential execution (not parallel like `status`)**
The `status` command uses threads because it is read-only. `rebase-all` writes to each worktree's git state. Parallel rebases on the same underlying repo could corrupt state. Sequential execution also produces cleaner, ordered output.

**Pull main first, then rebase all colors**
If `git pull` on `main` fails (e.g., diverged history, no remote), the command aborts before touching any color worktree. This avoids rebasing onto a stale base and makes failure obvious.

**Abort rebase and continue on conflict**
When a color worktree hits a conflict, the command runs `git rebase --abort` to restore the worktree to its pre-rebase state, records the failure, and moves on to the next worktree. This ensures all worktrees are attempted and the user gets a full picture of what succeeded and what needs manual attention.

**New file `src/commands/rebase_all.rs`**
Consistent with the existing pattern (`init.rs`, `status.rs`). Reuses `find_gbiv_root` and `find_repo_in_worktree` (the latter currently private in `status.rs` — will be moved to `git_utils.rs`).

## Risks / Trade-offs

- **Rebase conflict leaves worktree in conflicted state** → Intentional; the user resolves conflicts manually. Worktrees already mid-rebase are skipped entirely to avoid compounding the issue.
- **`git pull` on main may require credentials / SSH** → No mitigation; standard git auth applies, failure message is surfaced to user
- **`find_repo_in_worktree` is private in `status.rs`** → Must be moved/re-exposed in `git_utils.rs` to be shared; minor refactor but low risk
