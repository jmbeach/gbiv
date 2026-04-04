## ADDED Requirements

### Requirement: Hard reset flag on reset command
The `gbiv reset` command SHALL accept a `--hard` flag that force-resets color worktrees to the remote main branch, bypassing merge and status checks.

#### Scenario: Single-color hard reset with unmerged branch
- **WHEN** user runs `gbiv reset red --hard` and the feature branch on `red` is NOT merged into remote main
- **THEN** the system SHALL checkout the `red` branch and reset it hard to the remote main branch (e.g., `origin/main`)

#### Scenario: Single-color hard reset with merged branch
- **WHEN** user runs `gbiv reset red --hard` and the feature branch on `red` IS merged into remote main
- **THEN** the system SHALL checkout the `red` branch and reset it hard to the remote main branch

#### Scenario: Hard reset on worktree already on color branch
- **WHEN** user runs `gbiv reset red --hard` and the `red` worktree is already on the `red` branch
- **THEN** the system SHALL still reset hard to the remote main branch (unlike normal reset which skips)

#### Scenario: Default reset behavior unchanged
- **WHEN** user runs `gbiv reset` or `gbiv reset red` without the `--hard` flag
- **THEN** the system SHALL behave exactly as it does today, with merge and status checks enforced

### Requirement: All-color hard reset resets all worktrees
When `--hard` is used without specifying a color, the system SHALL reset ALL 7 color worktrees unconditionally — bypassing the `[done]` status filter and the GBIV.md entry requirement.

#### Scenario: All-color hard reset bypasses done filter
- **WHEN** user runs `gbiv reset --hard` and some color worktrees have entries in GBIV.md without `[done]` status
- **THEN** the system SHALL reset those worktrees anyway

#### Scenario: All-color hard reset includes worktrees without GBIV.md entries
- **WHEN** user runs `gbiv reset --hard` and some color worktrees have no entry in GBIV.md
- **THEN** the system SHALL reset those worktrees anyway

### Requirement: Confirmation prompt for all-color hard reset
When `gbiv reset --hard` is run (all-color mode), the system SHALL display a confirmation prompt before proceeding.

#### Scenario: Confirmation prompt shown
- **WHEN** user runs `gbiv reset --hard` without `--yes`
- **THEN** the system SHALL list each color worktree with its current branch and ask `Continue? [y/N]`, defaulting to No

#### Scenario: User declines confirmation
- **WHEN** user responds with anything other than `y` or `Y` to the prompt
- **THEN** the system SHALL abort without resetting any worktrees

#### Scenario: Yes flag skips prompt
- **WHEN** user runs `gbiv reset --hard --yes`
- **THEN** the system SHALL skip the confirmation prompt and proceed with all resets

#### Scenario: Single-color hard reset has no prompt
- **WHEN** user runs `gbiv reset red --hard`
- **THEN** the system SHALL NOT show a confirmation prompt

### Requirement: Stash uncommitted changes before hard reset
Before performing a hard reset, the system SHALL stash uncommitted changes (including untracked files) as a safety net.

#### Scenario: Dirty worktree is stashed before reset
- **WHEN** a worktree has uncommitted changes (tracked or untracked) and `--hard` is used
- **THEN** the system SHALL run `git stash push -u -m "gbiv hard-reset: <branch> on <color> worktree"` before checking out the color branch

#### Scenario: Clean worktree skips stash
- **WHEN** a worktree has no uncommitted changes and `--hard` is used
- **THEN** the system SHALL skip the stash step and proceed directly with checkout and reset

#### Scenario: Stash failure aborts reset for that worktree
- **WHEN** `git stash push` fails for a worktree
- **THEN** the system SHALL abort the reset for that worktree, report the error, and continue with remaining worktrees

#### Scenario: Stash only applies to hard reset
- **WHEN** user runs `gbiv reset` or `gbiv reset red` without `--hard`
- **THEN** the system SHALL NOT stash any changes (existing behavior preserved)

### Requirement: Hard reset removes GBIV.md entry
After a successful hard reset of a color worktree, the system SHALL remove the corresponding entry from GBIV.md, same as a normal reset.

#### Scenario: GBIV.md entry removed after hard reset
- **WHEN** user runs `gbiv reset red --hard` and the reset succeeds and a GBIV.md entry exists for red
- **THEN** the `red` entry in GBIV.md SHALL be removed

#### Scenario: No GBIV.md entry to remove
- **WHEN** user runs `gbiv reset red --hard` and no GBIV.md entry exists for red
- **THEN** the system SHALL proceed without error (no-op for GBIV.md cleanup)
