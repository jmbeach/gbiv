# cleanup-output Specification

## Purpose
TBD - created by archiving change cleanup-output-improvements. Update Purpose after archive.
## Requirements
### Requirement: Cleanup success message
When a worktree is successfully cleaned up (branch was merged, checked out to color branch, and reset), the command SHALL print a message indicating the worktree was cleaned up, the branch it was previously on, and the remote ref it was reset to.

#### Scenario: Worktree with merged feature branch is cleaned up
- **WHEN** `cleanup_one` is called for a color worktree whose current branch is a merged feature branch (e.g., "my-feature")
- **THEN** the command prints a message like `"red worktree cleaned up (was on my-feature), reset to origin/main"`

#### Scenario: Multiple worktrees cleaned up
- **WHEN** `cleanup_command` is called with no color argument and multiple worktrees have merged feature branches
- **THEN** each cleaned-up worktree prints its own success message with the previous branch name and reset target

