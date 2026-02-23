### Requirement: tmux subcommand group exists
The `gbiv` CLI SHALL expose a `tmux` subcommand group that acts as a namespace for all tmux-related gbiv commands. Running `gbiv tmux` without a subcommand SHALL print help and exit with a non-zero status code.

#### Scenario: Help shown when no subcommand given
- **WHEN** user runs `gbiv tmux` with no additional arguments
- **THEN** the CLI prints usage help listing available subcommands and exits with a non-zero status

#### Scenario: Unknown subcommand rejected
- **WHEN** user runs `gbiv tmux unknown-cmd`
- **THEN** the CLI prints an error indicating the subcommand is unrecognized and exits with a non-zero status
