## ADDED Requirements

### Requirement: Require tmux to be installed
The command SHALL verify that `tmux` is available on PATH before proceeding. If `tmux` is not found, the command SHALL exit with a non-zero status and print a clear error.

#### Scenario: tmux not installed
- **WHEN** `tmux` is not available on the system PATH
- **THEN** the command exits with a non-zero status and prints "tmux not found. Please install tmux."

### Requirement: Detect gbiv root from current directory
The command SHALL detect the gbiv project root by walking upward from the current working directory. If no gbiv root is found, the command SHALL exit with an error.

#### Scenario: Run from inside a worktree directory
- **WHEN** user runs `gbiv tmux refresh` from inside any path within a gbiv project
- **THEN** the command finds the gbiv root and proceeds

#### Scenario: Run outside any gbiv project
- **WHEN** user runs `gbiv tmux refresh` from a directory that is not inside a gbiv project
- **THEN** the command exits with a non-zero status and prints an error message explaining that no gbiv project was found

### Requirement: Require the target session to already exist
The command SHALL check that a tmux session named after the gbiv folder already exists. If it does not exist, the command SHALL exit with a non-zero status and print an actionable error message.

#### Scenario: Session does not exist
- **WHEN** no tmux session named after the project folder is found
- **THEN** the command exits with a non-zero status and prints an error suggesting `gbiv tmux new-session` to create it first

### Requirement: Derive active color set from GBIV.md
The command SHALL parse `GBIV.md` at the gbiv root and collect the distinct set of ROYGBIV color tags that appear on at least one feature entry. The `main` window SHALL always be included regardless of GBIV.md content.

#### Scenario: GBIV.md has features tagged with two colors
- **WHEN** GBIV.md contains features tagged `[red]` and `[green]`
- **THEN** the active set is `{main, red, green}`

#### Scenario: GBIV.md is empty or missing
- **WHEN** GBIV.md does not exist or contains no tagged features
- **THEN** the active set contains only `main`

#### Scenario: GBIV.md has unrecognized tags
- **WHEN** GBIV.md contains a feature tagged `[purple]`
- **THEN** `purple` is excluded from the active set (not a ROYGBIV color)

### Requirement: Ensure a window exists for each active color
For each color in the active set, the command SHALL create a new window in the session if a window with that name does not already exist. The window's working directory SHALL be set to `<gbiv-root>/<color>/<folder-name>/`. If the worktree path does not exist on disk, the command SHALL print a warning and skip that window.

#### Scenario: Window for an active color is missing
- **WHEN** the session does not have a window named `red` and `red` is in the active set
- **THEN** a new window named `red` is created with its working directory set to the red worktree path

#### Scenario: Window for an active color already exists
- **WHEN** the session already has a window named `green` and `green` is in the active set
- **THEN** no new window is created for `green`

#### Scenario: Worktree directory does not exist on disk
- **WHEN** the worktree path for an active color is not present on disk
- **THEN** the command prints a warning for that path and skips creating the window

### Requirement: Reorder windows to canonical ROYGBIV order
After ensuring all active-color windows exist, the command SHALL reorder the session's windows so that managed windows appear in the order: `main` first, then active colors in ROYGBIV sequence (`red`, `orange`, `yellow`, `green`, `blue`, `indigo`, `violet`). Windows not in the managed set (user-created windows) SHALL be moved to the end, preserving their relative order.

#### Scenario: Windows are out of order
- **WHEN** the session has windows `green`, `main`, `red` in that order
- **THEN** after refresh the windows appear in the order `main`, `red`, `green`

#### Scenario: All windows already in order
- **WHEN** the session windows are already in canonical order
- **THEN** the command completes without error and order is unchanged

#### Scenario: User-created windows are preserved at the end
- **WHEN** the session has a window named `scratch` not in the managed set
- **THEN** `scratch` appears after all managed windows in the final order

### Requirement: Report results on success
On success, the command SHALL print a summary indicating how many windows were created and confirm the final window order, then exit with status 0.

#### Scenario: Successful refresh with new windows created
- **WHEN** the command creates two new windows and reorders successfully
- **THEN** it prints a message indicating success and exits with status 0
