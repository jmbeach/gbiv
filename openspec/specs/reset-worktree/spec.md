# reset-worktree Specification

## Purpose
TBD - created by archiving change add-reset-command. Update Purpose after archive.
## Requirements
### Requirement: Reset with explicit color argument
The system SHALL accept a `reset <color>` command that resets the specified color worktree to match the remote default branch, regardless of whether the current feature branch has been merged.

#### Scenario: Reset a color worktree with unmerged changes
- **WHEN** user runs `gbiv reset indigo` and the `indigo` worktree has an unmerged feature branch checked out
- **THEN** the system SHALL checkout the `indigo` branch, reset it hard to the remote default branch (e.g., `origin/main`), and exit successfully

#### Scenario: Reset a color worktree that is already on the color branch
- **WHEN** user runs `gbiv reset indigo` and the `indigo` worktree is already on the `indigo` branch
- **THEN** the system SHALL reset the `indigo` branch hard to the remote default branch and exit successfully

#### Scenario: Reset with invalid color
- **WHEN** user runs `gbiv reset purple` where `purple` is not a valid ROYGBIV color
- **THEN** the system SHALL return an error indicating the color is invalid

### Requirement: Reset with inferred color from current directory
The system SHALL infer the color from the current working directory when no `<color>` argument is provided.

#### Scenario: Reset current worktree without specifying color
- **WHEN** user runs `gbiv reset` while inside the `blue` worktree directory
- **THEN** the system SHALL reset the `blue` worktree as if `gbiv reset blue` was run

#### Scenario: Reset from non-color directory without argument
- **WHEN** user runs `gbiv reset` while inside the `main` worktree or outside any gbiv worktree
- **THEN** the system SHALL return an error indicating it cannot determine which color to reset

### Requirement: Remote default branch detection
The system SHALL detect the remote default branch using the existing detection logic (trying `origin/main`, `origin/master`, `origin/develop` in order).

#### Scenario: Remote main branch not found
- **WHEN** user runs `gbiv reset indigo` and no remote default branch can be detected
- **THEN** the system SHALL return an error indicating the remote default branch could not be found

### Requirement: Reset does not modify GBIV.md
The system SHALL NOT modify GBIV.md when restoring a worktree.

#### Scenario: GBIV.md unchanged after reset
- **WHEN** user runs `gbiv reset indigo` and GBIV.md has an entry tagged with `[indigo]`
- **THEN** the GBIV.md file SHALL remain unchanged after the reset completes

