## ADDED Requirements

### Requirement: Sync creates missing color windows
The system SHALL parse GBIV.md to determine which colors have active tasks, compare against existing tmux windows in the session, and create new windows for any active colors that are missing from the session.

#### Scenario: Color has tasks but no window
- **WHEN** GBIV.md contains a feature tagged `[indigo]` and no tmux window named "indigo" exists in the session
- **THEN** the system SHALL create a new tmux window named "indigo" with its working directory set to the indigo worktree path

#### Scenario: Color has tasks and window already exists
- **WHEN** GBIV.md contains a feature tagged `[red]` and a tmux window named "red" already exists
- **THEN** the system SHALL leave the existing "red" window untouched

#### Scenario: Color has no tasks
- **WHEN** a ROYGBIV color has no features tagged in GBIV.md
- **THEN** the system SHALL NOT create a window for that color

#### Scenario: Worktree directory missing
- **WHEN** a color has tasks in GBIV.md but its worktree directory does not exist on disk
- **THEN** the system SHALL print a warning and skip creating that window

### Requirement: Windows are reordered to ROYGBIV order
After creating any missing windows, the system SHALL reorder all existing color windows to match the canonical order: main (index 0), red (1), orange (2), yellow (3), green (4), blue (5), indigo (6), violet (7).

#### Scenario: Windows out of order after sync
- **WHEN** the session has windows [main, yellow, red, indigo] (out of order)
- **THEN** after sync, windows SHALL be ordered [main, red, yellow, indigo]

#### Scenario: Non-color windows present
- **WHEN** the session contains windows with names that are not ROYGBIV colors or "main"
- **THEN** those windows SHALL be placed after all ROYGBIV windows, preserving their relative order

### Requirement: Pre-flight guards
The system SHALL validate prerequisites before syncing, consistent with other tmux subcommands.

#### Scenario: tmux not installed
- **WHEN** `tmux` is not available on PATH
- **THEN** the system SHALL exit with an error message indicating tmux is required

#### Scenario: Not in a gbiv project
- **WHEN** the command is run outside a gbiv-initialized directory structure
- **THEN** the system SHALL exit with an error message indicating no gbiv project was found

#### Scenario: Session does not exist
- **WHEN** the target tmux session does not exist
- **THEN** the system SHALL exit with an error message indicating the session was not found

### Requirement: Session name option
The system SHALL accept an optional `--session-name` argument to specify which tmux session to sync, defaulting to the gbiv folder name.

#### Scenario: Custom session name
- **WHEN** the user runs `gbiv tmux sync --session-name myproject`
- **THEN** the system SHALL sync windows in the tmux session named "myproject"

#### Scenario: Default session name
- **WHEN** the user runs `gbiv tmux sync` without `--session-name`
- **THEN** the system SHALL use the gbiv root folder name as the session name
