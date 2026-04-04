## MODIFIED Requirements

### Requirement: Reset success message
When a worktree is successfully reset (branch was merged, checked out to color branch, and reset), the command SHALL print a message indicating the worktree was reset, the branch it was previously on, and the remote ref it was reset to.

#### Scenario: Worktree with merged feature branch is reset
- **WHEN** `reset_one` is called for a color worktree whose current branch is a merged feature branch (e.g., "my-feature")
- **THEN** the command prints a message like `"red worktree reset (was on my-feature), reset to origin/main"`

#### Scenario: Multiple worktrees reset
- **WHEN** `reset_command` is called with no color argument and multiple worktrees have merged feature branches
- **THEN** each reset worktree prints its own success message with the previous branch name and reset target

## RENAMED Requirements

### Requirement: Cleanup success message
FROM: Cleanup success message
TO: Reset success message
