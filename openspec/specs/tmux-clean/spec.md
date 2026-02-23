### Requirement: tmux clean subcommand exists
The `gbiv tmux` command group SHALL expose a `clean` subcommand. Running `gbiv tmux clean` SHALL inspect the project tmux session and close orphaned windows.

#### Scenario: Subcommand is registered
- **WHEN** user runs `gbiv tmux clean --help`
- **THEN** the CLI prints usage help for the `clean` subcommand and exits with status 0

### Requirement: Require tmux to be installed
The command SHALL verify that `tmux` is available on PATH before proceeding. If not found, exit with a non-zero status and print a clear error.

#### Scenario: tmux not installed
- **WHEN** `tmux` is not available on PATH
- **THEN** the command exits with a non-zero status and prints "tmux not found. Please install tmux."

### Requirement: Require a gbiv project root
The command SHALL detect the gbiv project root by walking upward from the current working directory. If no gbiv root is found, the command SHALL exit with a non-zero status and print an error.

#### Scenario: Run outside any gbiv project
- **WHEN** user runs `gbiv tmux clean` from a directory not inside a gbiv project
- **THEN** the command exits with a non-zero status and prints an error explaining no gbiv project was found

### Requirement: Require the session to exist
The command SHALL check whether a tmux session named after the project folder exists. If the session does not exist, the command SHALL exit with a non-zero status and print an actionable error.

#### Scenario: Session does not exist
- **WHEN** no tmux session with the project name exists
- **THEN** the command exits with a non-zero status and prints an error suggesting `gbiv tmux new-session`

### Requirement: Close windows with no tagged feature
The command SHALL enumerate windows in the project session, identify those whose name is a ROYGBIV color that has no corresponding `[color]`-tagged feature in `GBIV.md`, and close each such window using `tmux kill-window`.

#### Scenario: Window with no matching tagged feature is closed
- **WHEN** the session has a window named `orange` and no feature in `GBIV.md` has the tag `[orange]`
- **THEN** the command closes the `orange` window and prints a message indicating it was closed

#### Scenario: Window with a matching tagged feature is kept
- **WHEN** the session has a window named `orange` and `GBIV.md` contains `- [orange] Some feature`
- **THEN** the `orange` window is NOT closed

#### Scenario: Nothing to clean
- **WHEN** every ROYGBIV color window in the session has at least one matching tagged feature
- **THEN** the command prints a message indicating nothing was cleaned and exits with status 0

### Requirement: Never close the main window
The command SHALL NOT close a window named `main` regardless of `GBIV.md` contents.

#### Scenario: main window is always preserved
- **WHEN** the session has a `main` window and `GBIV.md` has no features at all
- **THEN** the `main` window is NOT closed

### Requirement: Skip non-ROYGBIV windows
The command SHALL only evaluate windows whose names appear in the ROYGBIV color list. Windows with any other name SHALL be silently ignored.

#### Scenario: User-created window with arbitrary name is untouched
- **WHEN** the session contains a window named `scratch` (not a ROYGBIV color)
- **THEN** the `scratch` window is NOT closed

### Requirement: Continue on per-window kill failure
If closing an individual window fails, the command SHALL print a warning identifying that window, continue with remaining windows, and exit with a non-zero status after the loop.

#### Scenario: One window fails to close
- **WHEN** `tmux kill-window` fails for one window
- **THEN** the command prints a warning for that window, proceeds to close remaining eligible windows, and exits with a non-zero status
