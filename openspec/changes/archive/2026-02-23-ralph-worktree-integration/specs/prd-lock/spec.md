## ADDED Requirements

### Requirement: Lock prd.json for exclusive access
The system SHALL provide a `gbiv prd lock` command that acquires an exclusive advisory lock on `prd.json` in the main worktree, blocking until the lock is available or a timeout is reached.

#### Scenario: Acquire lock when none exists
- **WHEN** no `.prd.lock` file exists in the main worktree
- **THEN** the command creates `.prd.lock` with the calling worktree's name and PID, prints the lock path to stderr, and exits with code 0

#### Scenario: Block and retry when lock is already held
- **WHEN** a valid `.prd.lock` file exists
- **THEN** the command retries acquisition every 500ms until the lock is released or timeout is reached

#### Scenario: Timeout when lock cannot be acquired
- **WHEN** the lock cannot be acquired within 30 seconds (or the value of `--timeout`)
- **THEN** the command exits with a non-zero exit code and prints an error identifying the current lock owner

#### Scenario: Atomic creation prevents race condition
- **WHEN** two worktrees attempt to acquire the lock simultaneously
- **THEN** exactly one succeeds (via `O_CREAT | O_EXCL`) and the other retries

### Requirement: Release lock held by calling worktree
The system SHALL provide a `gbiv prd unlock` command that removes the `.prd.lock` file, but only if the calling worktree is the current owner.

#### Scenario: Successful unlock by owner
- **WHEN** `.prd.lock` exists and the calling worktree's name matches the `worktree` field in the lock file
- **THEN** the lock file is deleted and the command exits with code 0

#### Scenario: Reject unlock from non-owner
- **WHEN** `.prd.lock` exists but the calling worktree is not the owner
- **THEN** the command exits with a non-zero exit code and prints an error identifying the current owner

#### Scenario: Force unlock by non-owner
- **WHEN** `--force` flag is provided and `.prd.lock` exists
- **THEN** the lock file is deleted regardless of ownership and the command exits with code 0

#### Scenario: Unlock when no lock exists
- **WHEN** no `.prd.lock` file exists
- **THEN** the command exits with code 0 (idempotent — no error)

### Requirement: Locate prd.json from any worktree
The system SHALL resolve the main worktree path using the existing `find_gbiv_root` utility, regardless of which ROYGBIV worktree the command is invoked from.

#### Scenario: Resolve from a color worktree
- **WHEN** `gbiv prd lock` is run from `<root>/blue/<project>/`
- **THEN** the lock file is created at `<root>/main/<project>/.prd.lock`

#### Scenario: Fail gracefully outside gbiv structure
- **WHEN** `gbiv prd lock` is run from a directory that is not part of a ROYGBIV structure
- **THEN** the command exits with a non-zero exit code and an informative error message

### Requirement: Lock file format is valid JSON
The `.prd.lock` file SHALL contain a JSON object with `worktree` (string) and `pid` (integer) fields.

#### Scenario: Lock file is readable JSON
- **WHEN** a lock is held
- **THEN** the contents of `.prd.lock` are valid JSON matching `{ "worktree": "<color>", "pid": <number> }`

#### Scenario: Corrupted lock file surfaces an error
- **WHEN** `.prd.lock` exists but contains invalid JSON
- **THEN** the command exits with a non-zero exit code and an error message including the path to delete manually
