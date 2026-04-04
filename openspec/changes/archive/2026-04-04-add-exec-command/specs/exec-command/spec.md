## ADDED Requirements

### Requirement: Execute command in a single color worktree
The system SHALL accept `gbiv exec <color> -- <command...>` and execute the given command in the repo directory of the specified color worktree. The command SHALL be run via `sh -c` with stdio inherited so the user sees real-time output.

#### Scenario: Run command in a specific color worktree
- **WHEN** user runs `gbiv exec red -- cargo build`
- **THEN** the system executes `sh -c "cargo build"` with `current_dir` set to the red worktree's repo directory, and the user sees the command's stdout/stderr in real time

#### Scenario: Specified color worktree does not exist
- **WHEN** user runs `gbiv exec indigo -- ls` and the indigo worktree directory does not exist
- **THEN** the system prints an error message indicating the worktree does not exist and exits with a non-zero status code

#### Scenario: Invalid color name
- **WHEN** user runs `gbiv exec purple -- ls`
- **THEN** the system prints an error indicating "purple" is not a valid color and exits with a non-zero status code

### Requirement: Execute command across all color worktrees
The system SHALL accept `gbiv exec all -- <command...>` and execute the given command in every existing color worktree's repo directory in parallel. Output SHALL be collected and printed per-color in ROYGBIV order with color-labeled headers.

#### Scenario: Run command across all worktrees
- **WHEN** user runs `gbiv exec all -- git status`
- **THEN** the system executes the command in each existing color worktree in parallel, then prints each worktree's output with a colored header label in ROYGBIV order

#### Scenario: Some worktrees do not exist
- **WHEN** user runs `gbiv exec all -- ls` and only red, green, and blue worktree directories exist
- **THEN** the system runs the command only in the existing worktrees and skips missing ones without error

#### Scenario: Command fails in one or more worktrees
- **WHEN** user runs `gbiv exec all -- cargo test` and the command fails in the red worktree but succeeds in others
- **THEN** the system prints all results (including the failure output for red) and exits with a non-zero status code

### Requirement: Infer color from current working directory
The system SHALL accept `gbiv exec -- <command...>` (no target specified) and infer the color from the current working directory. If the CWD is not inside a color worktree, the system SHALL print an error.

#### Scenario: Infer color from CWD
- **WHEN** user is in the green worktree directory and runs `gbiv exec -- cargo build`
- **THEN** the system infers the target is "green" and executes the command in the green worktree's repo directory

#### Scenario: CWD is not in a color worktree
- **WHEN** user is in the main worktree directory and runs `gbiv exec -- cargo build`
- **THEN** the system prints an error indicating it cannot infer a color from the current directory and exits with a non-zero status code

### Requirement: Exit status reflects command success
The system SHALL exit with status code 0 only if all executed commands succeed. If any command exits with a non-zero status, the system SHALL exit with a non-zero status.

#### Scenario: All commands succeed
- **WHEN** user runs `gbiv exec all -- echo hello` and all worktrees succeed
- **THEN** the system exits with status code 0

#### Scenario: Any command fails
- **WHEN** user runs `gbiv exec all -- false` (which always exits non-zero)
- **THEN** the system exits with a non-zero status code
