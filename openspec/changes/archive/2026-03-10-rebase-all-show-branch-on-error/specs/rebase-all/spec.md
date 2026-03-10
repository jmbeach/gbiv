## MODIFIED Requirements

### Requirement: Report per-worktree outcome
The command SHALL print a result line for each color worktree indicating whether the rebase succeeded, failed, or was skipped. When a rebase fails, the status line MUST include the branch/color name so the user can identify which worktree had the error.

#### Scenario: All succeed
- **WHEN** all color worktrees rebase successfully
- **THEN** each is shown with a success indicator

#### Scenario: Some fail
- **WHEN** one or more worktrees fail to rebase
- **THEN** each failed worktree is shown with a failure indicator that includes the color name on the same line as the error summary
- **THEN** any multi-line error detail from git SHALL be printed as indented lines below the status line
- **THEN** the command exits with a non-zero status code

#### Scenario: Rebase conflict produces multi-line git output
- **WHEN** `git rebase` fails with a conflict that produces multi-line output (stdout and stderr)
- **THEN** all git output SHALL be captured (not printed directly to the terminal)
- **THEN** the color name and a one-line error summary SHALL be printed as the status line
- **THEN** additional detail lines SHALL be printed indented below the status line
