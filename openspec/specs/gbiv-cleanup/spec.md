### Requirement: Cleanup command is available as a subcommand
The CLI SHALL expose a `cleanup` subcommand accepting an optional positional `<color>` argument restricted to valid ROYGBIV colors.

#### Scenario: Help text is shown
- **WHEN** the user runs `gbiv cleanup --help`
- **THEN** the CLI prints usage describing the optional color argument

#### Scenario: Invalid color is rejected
- **WHEN** the user runs `gbiv cleanup purple`
- **THEN** the CLI exits with a non-zero status and prints an error indicating the color is invalid

### Requirement: Single-color cleanup detects merge status before acting
When a color is specified, the system SHALL check whether the worktree's current branch is merged into remote main before performing any destructive action.

#### Scenario: Branch is not merged
- **WHEN** the user runs `gbiv cleanup red` and the red worktree's current feature branch is NOT an ancestor of remote main
- **THEN** the command prints an error message indicating the branch is not merged and exits with a non-zero status without modifying any files or branches

#### Scenario: Branch is already the color branch
- **WHEN** the user runs `gbiv cleanup red` and the red worktree is already on the `red` branch
- **THEN** the command prints a notice that the worktree is already clean and exits successfully without modifying anything

#### Scenario: No remote configured
- **WHEN** the user runs `gbiv cleanup red` and the red worktree has no remote tracking branch
- **THEN** the command prints a warning that merge status cannot be determined and exits with a non-zero status

### Requirement: Single-color cleanup checks out color branch and pulls
After confirming the feature branch is merged, the system SHALL check out the color branch and pull the latest remote changes in that worktree.

#### Scenario: Successful checkout and pull
- **WHEN** the user runs `gbiv cleanup red` and the red worktree's feature branch is merged into remote main
- **THEN** the command runs `git checkout red` then `git pull` in the red worktree's repo directory

#### Scenario: Checkout fails due to dirty working tree
- **WHEN** the user runs `gbiv cleanup red` and the red worktree has uncommitted changes
- **THEN** the command surfaces the git error message and exits with a non-zero status without modifying GBIV.md

### Requirement: Single-color cleanup removes the color's GBIV.md entry
After a successful checkout and pull, the system SHALL remove all GBIV.md feature entries whose tag matches the cleaned-up color.

#### Scenario: Matching entry is removed
- **WHEN** cleanup for `red` succeeds and GBIV.md contains `- [red] Fix critical bug`
- **THEN** that entry is removed from GBIV.md and the file is written back

#### Scenario: No matching entry exists
- **WHEN** cleanup for `red` succeeds and GBIV.md has no entry tagged `[red]`
- **THEN** GBIV.md is unchanged and no error is raised

#### Scenario: Multiple entries with same color tag
- **WHEN** cleanup for `red` succeeds and GBIV.md contains multiple `[red]` entries
- **THEN** all `[red]` entries are removed from GBIV.md

#### Scenario: Content after separator is preserved
- **WHEN** GBIV.md contains a `---` separator with content after it
- **THEN** the content after `---` is unchanged after cleanup writes the file

### Requirement: All-color cleanup runs cleanup for every color worktree
When no color argument is provided, the system SHALL attempt cleanup for each of the seven ROYGBIV colors in sequence.

#### Scenario: All worktrees have merged branches
- **WHEN** the user runs `gbiv cleanup` and all color worktrees have feature branches merged into remote main
- **THEN** each worktree is checked out to its color branch, pulled, and its GBIV.md entry removed

#### Scenario: One worktree is not merged
- **WHEN** the user runs `gbiv cleanup` and one color's feature branch is not merged
- **THEN** that color is skipped with a warning and cleanup continues for the remaining colors

#### Scenario: One worktree is missing
- **WHEN** the user runs `gbiv cleanup` and a color worktree directory does not exist
- **THEN** that color is skipped with a warning and cleanup continues for the remaining colors

#### Scenario: One worktree is already on color branch
- **WHEN** the user runs `gbiv cleanup` and a color worktree is already on its color branch
- **THEN** that color is skipped with an "already clean" notice and cleanup continues for the remaining colors

### Requirement: Cleanup must be run from within a gbiv-structured repository
The system SHALL locate the gbiv root using the existing `find_gbiv_root` logic and SHALL fail with a clear error if not in a gbiv-structured repo.

#### Scenario: Not in a gbiv repo
- **WHEN** the user runs `gbiv cleanup` from a directory that is not inside a gbiv-structured repository
- **THEN** the command exits with a non-zero status and prints "Not in a gbiv-structured repository"
