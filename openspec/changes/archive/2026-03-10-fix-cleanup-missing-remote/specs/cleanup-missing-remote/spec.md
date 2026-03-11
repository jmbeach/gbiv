## ADDED Requirements

### Requirement: Cleanup resets color branch to remote main
After checking out the color branch, `cleanup_one` SHALL run `git reset --hard <remote_main>` to reset the color branch to the latest main. The remote main ref SHALL be the same one already resolved for the merge check.

#### Scenario: Cleanup after feature branch merged
- **WHEN** a worktree is on a merged feature branch and cleanup is run
- **THEN** cleanup SHALL checkout the color branch, reset it to origin/main, and update GBIV.md

#### Scenario: Color branch is at same commit as main after cleanup
- **WHEN** cleanup completes successfully
- **THEN** the color branch HEAD SHALL point to the same commit as the remote main branch

### Requirement: Cleanup does not pull color branch from remote
The `cleanup_one` function SHALL NOT call `pull_remote` for the color branch. Color branches are local-only and do not exist on the remote.

#### Scenario: No pull attempt during cleanup
- **WHEN** cleanup is run on any worktree
- **THEN** the system SHALL NOT attempt `git pull origin <color>`
