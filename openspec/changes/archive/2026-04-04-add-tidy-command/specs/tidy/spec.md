## ADDED Requirements

### Requirement: Tidy runs rebase-all, reset, and tmux clean in sequence [TIDY-001]
The `gbiv tidy` command SHALL execute three maintenance steps in order:
1. `rebase-all` — rebase all color worktrees onto main
2. `reset` — reset worktrees with merged branches back to main (via `reset_command(None)`)
3. `tmux clean` — close orphaned tmux windows

#### Scenario: All steps succeed
- **WHEN** user runs `gbiv tidy` and all three steps complete successfully
- **THEN** the command SHALL print output from each step and exit with code 0

#### Scenario: rebase-all fails, others continue
- **WHEN** user runs `gbiv tidy` and `rebase-all` returns an error
- **THEN** the command SHALL print the error for `rebase-all`, continue running `reset` and `tmux clean`, and exit with code 1

#### Scenario: tmux clean fails, others already ran
- **WHEN** user runs `gbiv tidy` and `tmux clean` returns an error
- **THEN** the command SHALL exit with code 1 (rebase-all and reset results are unaffected)

#### Scenario: reset errors are swallowed
- **WHEN** user runs `gbiv tidy` and `reset_command(None)` prints errors internally but returns Ok
- **THEN** the command SHALL NOT treat reset as a failure for exit code purposes

### Requirement: Tidy skips tmux clean when tmux is not installed [TIDY-002]
The `gbiv tidy` command SHALL check if tmux is available before running the clean step.

#### Scenario: tmux not installed
- **WHEN** user runs `gbiv tidy` and tmux is not found on PATH
- **THEN** the command SHALL skip the tmux clean step silently

#### Scenario: tmux installed but clean fails
- **WHEN** user runs `gbiv tidy` and tmux is installed but `clean_command()` returns an error
- **THEN** the command SHALL report the error and include it in the exit code

### Requirement: Tidy prints step headers [TIDY-003]
The `gbiv tidy` command SHALL print a header before each step so the user can distinguish output from each sub-command.

#### Scenario: Step headers are displayed
- **WHEN** user runs `gbiv tidy`
- **THEN** the command SHALL print a labeled header (e.g., "Rebasing all worktrees...") before each of the three steps
