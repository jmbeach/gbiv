## MODIFIED Requirements

### Requirement: All-color cleanup runs cleanup for every color worktree
When no color argument is provided, the system SHALL attempt cleanup only for color worktrees whose GBIV.md entry has a `[done]` status tag. Worktrees without a `[done]` status tag SHALL be skipped with an informational message.

#### Scenario: All done worktrees have merged branches
- **WHEN** the user runs `gbiv cleanup` and all color worktrees with `[done]` status have feature branches merged into remote main
- **THEN** each done worktree is checked out to its color branch, reset to remote main, and its GBIV.md entry removed

#### Scenario: Worktree has done status but branch not merged
- **WHEN** the user runs `gbiv cleanup` and a color worktree has `[done]` status but its branch is not merged
- **THEN** that color is skipped with a warning and cleanup continues for the remaining colors

#### Scenario: Worktree has in-progress status
- **WHEN** the user runs `gbiv cleanup` and a color worktree has `[in-progress]` status
- **THEN** that color is skipped with a message indicating it is in-progress

#### Scenario: Worktree has no status set
- **WHEN** the user runs `gbiv cleanup` and a color worktree's GBIV.md entry has no status tag
- **THEN** that color is skipped (no message needed — it's idle)

#### Scenario: Worktree is already on color branch with done status
- **WHEN** the user runs `gbiv cleanup` and a color worktree has `[done]` status but is already on its color branch
- **THEN** that color is skipped with an "already clean" notice and its GBIV.md entry is removed

#### Scenario: One worktree is missing
- **WHEN** the user runs `gbiv cleanup` and a color worktree directory does not exist
- **THEN** that color is skipped with a warning and cleanup continues for the remaining colors

#### Scenario: No GBIV.md entry for a color
- **WHEN** the user runs `gbiv cleanup` and a color has no entry in GBIV.md
- **THEN** that color is skipped silently

### Requirement: All-color cleanup prints a summary
After processing all colors, the system SHALL print a summary line indicating how many worktrees were cleaned and a breakdown of skip reasons. Skip reasons SHALL be distinguished: no `[done]` status, not merged, already clean, missing worktree, and other errors. Only non-zero skip categories SHALL be included in the summary.

#### Scenario: No worktrees cleaned, some without done status
- **WHEN** the user runs `gbiv cleanup` and 3 worktrees lack `[done]` status
- **THEN** the command prints a summary like `0 cleaned (3 without [done] status)`

#### Scenario: Mixed skip reasons
- **WHEN** the user runs `gbiv cleanup` and 1 worktree is cleaned, 1 is not merged, and 2 lack `[done]` status
- **THEN** the command prints a summary like `1 cleaned (1 not merged, 2 without [done] status)`

#### Scenario: All done worktrees cleaned
- **WHEN** the user runs `gbiv cleanup` and all done worktrees are successfully cleaned with none skipped
- **THEN** the command prints a summary like `3 cleaned`

#### Scenario: Already clean worktrees
- **WHEN** the user runs `gbiv cleanup` and 1 worktree is cleaned and 1 was already on its color branch
- **THEN** the command prints a summary like `1 cleaned (1 already clean)`
