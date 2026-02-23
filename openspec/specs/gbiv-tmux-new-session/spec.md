### Requirement: Detect gbiv root from current directory
The command SHALL detect the gbiv project root by walking upward from the current working directory, looking for a directory `P` where `P/main/<name>/` exists and is a git repository (where `<name>` is the basename of `P`). If no gbiv root is found, the command SHALL exit with an error.

#### Scenario: Run from inside a worktree directory
- **WHEN** user runs `gbiv tmux new-session` from inside any path within a gbiv project
- **THEN** the command finds the gbiv root and proceeds

#### Scenario: Run outside any gbiv project
- **WHEN** user runs `gbiv tmux new-session` from a directory that is not inside a gbiv project
- **THEN** the command exits with a non-zero status and prints an error message explaining that no gbiv project was found and suggesting `gbiv init`

### Requirement: Require tmux to be installed
The command SHALL verify that `tmux` is available on PATH before attempting any session creation. If `tmux` is not found, the command SHALL exit with a non-zero status and print a clear error.

#### Scenario: tmux not installed
- **WHEN** `tmux` is not available on the system PATH
- **THEN** the command exits with a non-zero status and prints "tmux not found. Please install tmux."

### Requirement: Create a detached tmux session with one window per worktree
The command SHALL create a new detached tmux session with 8 windows — one for `main` and one for each ROYGBIV color (`red`, `orange`, `yellow`, `green`, `blue`, `indigo`, `violet`) — in that order. Each window SHALL be named after its color/main and SHALL have its working directory set to the corresponding worktree path (`<root>/<color>/<folder>/`).

#### Scenario: Successful session creation
- **WHEN** user runs `gbiv tmux new-session` inside a valid gbiv project with tmux installed and no conflicting session name
- **THEN** a new detached tmux session is created with 8 named windows, each pointing to the correct worktree directory, and the command prints the session name and exits with status 0

#### Scenario: Windows created in ROYGBIV order
- **WHEN** a session is created successfully
- **THEN** windows appear in the order: main, red, orange, yellow, green, blue, indigo, violet

### Requirement: Default session name is the project folder name
The command SHALL default the tmux session name to the gbiv project folder name (the basename of the gbiv root directory).

#### Scenario: Session named after project folder by default
- **WHEN** user runs `gbiv tmux new-session` without `--session-name` in a project named `myproject`
- **THEN** the created tmux session is named `myproject`

### Requirement: Override session name with --session-name flag
The command SHALL accept a `--session-name <name>` flag that overrides the default session name.

#### Scenario: Custom session name used
- **WHEN** user runs `gbiv tmux new-session --session-name dev`
- **THEN** the created tmux session is named `dev`

### Requirement: Fail if session name already exists
The command SHALL check whether a tmux session with the target name already exists before creating. If a session with that name already exists, the command SHALL exit with a non-zero status and print an actionable error message.

#### Scenario: Session name collision
- **WHEN** a tmux session with the target name already exists
- **THEN** the command exits with a non-zero status and prints an error suggesting `tmux attach -t <name>` or using `--session-name`

### Requirement: Skip missing worktree paths with a warning
If a worktree directory does not exist on disk (e.g., partial or broken init), the command SHALL skip that window, print a warning identifying the missing path, and continue creating windows for the remaining worktrees.

#### Scenario: One worktree directory is missing
- **WHEN** one of the expected worktree paths does not exist on disk
- **THEN** the command prints a warning for that path, skips creating its window, and proceeds with the remaining worktrees
