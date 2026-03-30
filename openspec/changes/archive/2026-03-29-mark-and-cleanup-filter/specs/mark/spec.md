## ADDED Requirements

### Requirement: mark subcommand is available
The CLI SHALL expose a `mark` subcommand accepting exactly one of three mutually exclusive flags (`--in-progress`, `--done`, `--unset`) and an optional positional `[color]` argument restricted to valid ROYGBIV colors. When `color` is omitted, the system SHALL infer it from the current working directory by checking which ROYGBIV color directory the CWD is under within the gbiv root.

#### Scenario: Help text is shown
- **WHEN** the user runs `gbiv mark --help`
- **THEN** the CLI prints usage describing the mutually exclusive flags and optional color argument

#### Scenario: Explicit color provided
- **WHEN** the user runs `gbiv mark --done red`
- **THEN** the command targets the `red` worktree

#### Scenario: Color inferred from current worktree
- **WHEN** the user runs `gbiv mark --done` from within the red worktree directory
- **THEN** the command infers color `red` and targets the `red` worktree

#### Scenario: Color cannot be inferred
- **WHEN** the user runs `gbiv mark --done` from the main worktree or the gbiv root (not inside a color worktree)
- **THEN** the CLI exits with a non-zero status and prints an error indicating the color could not be inferred

#### Scenario: Invalid color is rejected
- **WHEN** the user runs `gbiv mark --done purple`
- **THEN** the CLI exits with a non-zero status and prints an error indicating the color is invalid

#### Scenario: No flag provided
- **WHEN** the user runs `gbiv mark red` with no status flag
- **THEN** the CLI exits with a non-zero status and prints usage indicating one of `--in-progress`, `--done`, or `--unset` is required

#### Scenario: Multiple flags provided
- **WHEN** the user runs `gbiv mark --done --in-progress red`
- **THEN** the CLI exits with a non-zero status and prints an error indicating the flags are mutually exclusive

### Requirement: mark --done sets done status in GBIV.md
The `mark --done` command SHALL find the GBIV.md feature entry matching the given color tag and add or replace the status tag with `[done]`.

#### Scenario: Add done status to entry with no status
- **WHEN** GBIV.md contains `- [red] Fix critical bug` and the user runs `gbiv mark --done red`
- **THEN** GBIV.md is updated to `- [red] [done] Fix critical bug`

#### Scenario: Replace existing status with done
- **WHEN** GBIV.md contains `- [red] [in-progress] Fix critical bug` and the user runs `gbiv mark --done red`
- **THEN** GBIV.md is updated to `- [red] [done] Fix critical bug`

### Requirement: mark --in-progress sets in-progress status in GBIV.md
The `mark --in-progress` command SHALL find the GBIV.md feature entry matching the given color tag and add or replace the status tag with `[in-progress]`.

#### Scenario: Add in-progress status
- **WHEN** GBIV.md contains `- [red] Fix critical bug` and the user runs `gbiv mark --in-progress red`
- **THEN** GBIV.md is updated to `- [red] [in-progress] Fix critical bug`

### Requirement: mark --unset removes status from GBIV.md
The `mark --unset` command SHALL find the GBIV.md feature entry matching the given color tag and remove the status tag, leaving the color tag and description intact.

#### Scenario: Remove done status
- **WHEN** GBIV.md contains `- [red] [done] Fix critical bug` and the user runs `gbiv mark --unset red`
- **THEN** GBIV.md is updated to `- [red] Fix critical bug`

#### Scenario: Unset when no status exists
- **WHEN** GBIV.md contains `- [red] Fix critical bug` and the user runs `gbiv mark --unset red`
- **THEN** GBIV.md is unchanged and no error is raised

### Requirement: mark errors when no matching color entry exists for --done and --in-progress
If GBIV.md has no entry tagged with the target color, the `--done` and `--in-progress` flags SHALL cause the command to exit with a non-zero status and print an error. The `--unset` flag SHALL no-op silently.

#### Scenario: No matching color entry with --done
- **WHEN** GBIV.md has no entry tagged `[red]` and the user runs `gbiv mark --done red`
- **THEN** the command exits with a non-zero status and prints an error indicating no feature is assigned to that color

#### Scenario: No matching color entry with --unset
- **WHEN** GBIV.md has no entry tagged `[red]` and the user runs `gbiv mark --unset red`
- **THEN** the command exits successfully with no output

### Requirement: mark preserves notes
When updating a status tag, all note lines attached to the feature entry SHALL be preserved unchanged.

#### Scenario: Notes are preserved
- **WHEN** GBIV.md contains a `[red]` entry with attached note lines and the user runs `gbiv mark --done red`
- **THEN** the note lines are preserved unchanged

### Requirement: mark must be run from within a gbiv-structured repository
The system SHALL locate the gbiv root using the existing `find_gbiv_root` logic and SHALL fail with a clear error if not in a gbiv-structured repo.

#### Scenario: Not in a gbiv repo
- **WHEN** the user runs `gbiv mark --done` from a directory that is not inside a gbiv-structured repository
- **THEN** the command exits with a non-zero status and prints "Not in a gbiv-structured repository"

### Requirement: mark prints confirmation
After successfully updating GBIV.md, the command SHALL print a confirmation message.

#### Scenario: Confirmation output
- **WHEN** the user runs `gbiv mark --done red`
- **THEN** the command prints a message like `red: marked as done`

#### Scenario: Confirmation for unset
- **WHEN** the user runs `gbiv mark --unset red`
- **THEN** the command prints a message like `red: status cleared`

#### Scenario: Confirmation with inferred color
- **WHEN** the user runs `gbiv mark --done` from the red worktree
- **THEN** the command prints a message like `red: marked as done`
