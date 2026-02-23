## ADDED Requirements

### Requirement: Pull main worktree before rebasing
The command SHALL run `git pull` in the `main` worktree as the first step. If the pull fails, the command SHALL abort with an error message and SHALL NOT proceed to rebase any color worktree.

#### Scenario: Successful pull
- **WHEN** `git pull` succeeds in the `main` worktree
- **THEN** the command proceeds to rebase the color worktrees

#### Scenario: Pull fails
- **WHEN** `git pull` fails in the `main` worktree
- **THEN** the command prints an error and exits without touching any color worktree

### Requirement: Rebase all color worktrees onto origin/main
The command SHALL attempt to rebase each existing gbiv-managed color worktree's current branch onto `origin/main`, in order (red, orange, yellow, green, blue, indigo, violet).

#### Scenario: Worktree does not exist
- **WHEN** a color worktree directory is not present on disk
- **THEN** the command SHALL skip it and continue to the next color

#### Scenario: Rebase succeeds
- **WHEN** `git rebase origin/main` completes without conflict in a color worktree
- **THEN** the command records success for that worktree and continues

#### Scenario: Worktree is already mid-rebase
- **WHEN** `.git/rebase-merge` or `.git/rebase-apply` exists inside the color worktree's repo
- **THEN** the command SHALL skip it with a `SKIP (rebase in progress)` status line
- **THEN** the command SHALL NOT attempt a new rebase on that worktree

#### Scenario: Rebase fails with conflict
- **WHEN** `git rebase origin/main` encounters a conflict in a color worktree
- **THEN** the command SHALL leave the worktree in the conflicted state (SHALL NOT run `git rebase --abort`)
- **THEN** the command SHALL record failure for that worktree and continue to the next

### Requirement: Report per-worktree outcome
The command SHALL print a result line for each color worktree indicating whether the rebase succeeded, failed, or was skipped.

#### Scenario: All succeed
- **WHEN** all color worktrees rebase successfully
- **THEN** each is shown with a success indicator

#### Scenario: Some fail
- **WHEN** one or more worktrees fail to rebase
- **THEN** each failed worktree is shown with a failure indicator
- **THEN** the command exits with a non-zero status code
