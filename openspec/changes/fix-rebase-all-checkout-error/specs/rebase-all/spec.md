## ADDED Requirements

### Requirement: rebase-all command exists
`gbiv` SHALL expose a `rebase-all` subcommand that rebases every ROYGBIV colour worktree against the upstream main branch.

#### Scenario: Command is registered
- **WHEN** user runs `gbiv rebase-all --help`
- **THEN** the help text for the `rebase-all` subcommand is displayed without error

### Requirement: Each worktree is rebased
For each colour worktree discovered under the gbiv root, the `rebase-all` command SHALL fetch the remote and rebase the worktree's branch onto `origin/<main-branch>`.

#### Scenario: Clean worktree is rebased successfully
- **WHEN** user runs `gbiv rebase-all` in a gbiv root
- **AND** each colour worktree has no uncommitted changes
- **THEN** each worktree is rebased onto `origin/<main-branch>` and a success message is printed per worktree

#### Scenario: Already up-to-date worktree is skipped
- **WHEN** a colour worktree's branch is already up-to-date with `origin/<main-branch>`
- **THEN** the command prints "already up to date" for that worktree and moves on without error

### Requirement: gbiv state files do not block checkout
The `rebase-all` command SHALL ensure that `gbiv`-managed state files (`.last-branch`) never cause a git checkout failure. Before rebasing a worktree, the command SHALL append `.last-branch` to that worktree's `.git/info/exclude` if not already present.

#### Scenario: .last-branch present but not ignored
- **WHEN** `.last-branch` exists in a worktree root
- **AND** `.last-branch` is not listed in `.git/info/exclude`
- **THEN** `rebase-all` appends `.last-branch` to `.git/info/exclude` before performing the rebase
- **AND** the rebase proceeds without the "untracked working tree files would be overwritten" error

#### Scenario: .last-branch already ignored
- **WHEN** `.last-branch` is already present in `.git/info/exclude`
- **THEN** `rebase-all` does not duplicate the entry and proceeds normally

#### Scenario: Worktree uses a gitfile (.git is a file, not a directory)
- **WHEN** a linked worktree has `.git` as a text file (standard for `git worktree add`)
- **THEN** `rebase-all` resolves the true git directory via `git rev-parse --git-dir` and writes `info/exclude` to the correct location

### Requirement: Per-worktree result summary
After processing all worktrees, `rebase-all` SHALL print a summary showing how many worktrees succeeded and how many failed.

#### Scenario: All worktrees succeed
- **WHEN** all colour worktrees are rebased without error
- **THEN** the summary reports N/N succeeded

#### Scenario: One or more worktrees fail
- **WHEN** one or more worktrees encounter a rebase conflict or other fatal error
- **THEN** the summary reports the count of successes and failures
- **AND** the command exits with a non-zero exit code

### Requirement: Failure in one worktree does not stop others
The `rebase-all` command SHALL continue processing remaining worktrees after a non-fatal failure in any single worktree.

#### Scenario: Rebase conflict in one worktree
- **WHEN** a rebase conflict occurs in one colour worktree
- **THEN** the command aborts the rebase for that worktree (leaving it in its pre-rebase state)
- **AND** continues to process the remaining colour worktrees
