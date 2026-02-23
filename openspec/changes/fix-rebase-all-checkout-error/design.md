## Context

`gbiv` manages a ROYGBIV set of git worktrees around a single bare/main repository. The `rebase-all` command needs to visit each colour worktree and rebase it onto the latest upstream main branch.

The failure is caused by `.last-branch` — a file written into the worktree root by `gbiv` (or companion tooling) to remember the previous branch for navigation purposes. Because `.last-branch` is not tracked by git and not listed in any `.gitignore`, git treats it as an untracked file. When `rebase-all` calls `git checkout` or `git switch --detach` inside the worktree, git refuses to proceed if the checkout would place a tracked version of that filename in the working tree (which can happen during the detach phase of a rebase).

## Goals / Non-Goals

**Goals:**
- Implement the `gbiv rebase-all` subcommand that rebases every ROYGBIV worktree against the upstream main branch (e.g. `origin/main`).
- Ensure `gbiv`-managed state files (`GBIV_STATE_FILES` = `[".last-branch"]`) never cause a checkout failure.
- Report per-worktree success/failure and continue processing remaining worktrees on non-fatal errors.

**Non-Goals:**
- Push rebased branches to remote.
- Handle merge conflicts interactively (fail fast and report, let the user resolve manually).
- Change how `.last-branch` is created or used elsewhere.

## Decisions

### D1 — Use `.git/info/exclude` to ignore `gbiv` state files (preferred)

**Decision:** During `gbiv init` (and retroactively in `rebase-all` if missing), append `.last-branch` to `<worktree>/.git/info/exclude`.

**Rationale:** This is the cleanest fix — git simply never considers `.last-branch` an obstacle. It requires no per-operation save/restore logic, survives crashes, and doesn't silently destroy state. `info/exclude` is the right place for tooling-generated files that shouldn't be in the shared `.gitignore`.

**Alternatives considered:**
- *Remove-then-restore*: Delete `.last-branch` before checkout, re-create it after. Works, but is fragile if the process is interrupted mid-operation (state is lost permanently).
- *`git checkout --force`*: Would silently overwrite any untracked file, which is too destructive.
- *Root `.gitignore`*: Pollutes the shared ignore file with a tool-specific entry; not all users of the repo use `gbiv`.

### D2 — Rebase strategy: `git rebase <upstream>` per worktree

**Decision:** For each colour worktree, run `git fetch origin` then `git rebase origin/<main-branch>`. The main branch name is discovered via `get_remote_main_branch()` (already in `git_utils.rs`).

**Rationale:** A plain rebase is the most transparent operation; users can see exactly what happened. `git pull --rebase` is equivalent but adds a fetch implicitly, making error attribution harder.

### D3 — Continue on per-worktree failure, report summary

**Decision:** On rebase failure in one worktree, print an error and continue to the next. At the end, print a summary (N succeeded, M failed).

**Rationale:** Consistent with how CI/CD systems handle parallel jobs — one dirty worktree shouldn't block rebasing the others.

## Risks / Trade-offs

- **Worktree has uncommitted changes** → `git rebase` will abort. Mitigation: detect dirty state via `get_quick_status()` before rebasing and skip with a clear warning.
- **`info/exclude` already contains `.last-branch`** → Writing it twice is harmless (git deduplicates). Mitigation: check before appending.
- **Worktree's `.git` is a file (gitfile) not a directory** → For linked worktrees, `.git` is a text file pointing at the main repo's `worktrees/<name>` directory. The real `info/exclude` lives at `<main-git-dir>/worktrees/<name>/info/exclude`. Mitigation: resolve the true git dir via `git rev-parse --git-dir` when writing `info/exclude`.

## Migration Plan

1. `gbiv rebase-all` auto-repairs the `info/exclude` entry on first run (no user action required).
2. Future: `gbiv init` will also write the entry so new setups never encounter this.
3. No rollback needed — appending to `info/exclude` is non-destructive.

## Open Questions

- Should `rebase-all` also fetch before rebasing, or require the user to run `git fetch` first? (Current decision: fetch as part of the command for convenience.)
- Should `rebase-all` skip worktrees whose branch is already up-to-date? (Lean yes — detect via `get_ahead_behind_vs()` and skip with a "already up to date" message.)
