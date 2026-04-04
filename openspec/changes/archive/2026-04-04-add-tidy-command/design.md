## Context

gbiv manages color-named git worktrees. Keeping them tidy involves three sequential operations: rebasing all worktrees onto main (`rebase-all`), resetting merged branches back to main (`reset`), and cleaning up orphaned tmux windows (`tmux clean`). Users currently run these as three separate commands.

All three commands already exist with clean public interfaces:
- `rebase_all_command() -> Result<(), String>`
- `reset_command(color: Option<&str>) -> Result<(), String>`
- `clean_command() -> Result<(), String>`

## Goals / Non-Goals

**Goals:**
- Single `gbiv tidy` command that runs all three maintenance steps in order
- Continue executing remaining steps even if one fails
- Report clear per-step success/failure output

**Non-Goals:**
- Making the sub-steps configurable or skippable (keep it simple)
- Changing the behavior of any existing command
- Adding new flags or arguments to `tidy`

## Decisions

**1. Sequential execution, not parallel**
The steps have logical ordering: rebase first (get latest), then reset (clean merged branches), then tmux clean (remove stale windows). Running them in parallel would be incorrect since reset depends on rebase results.

**2. Continue on failure with asymmetric error handling**
If `rebase-all` fails, `reset` and `tmux clean` should still run. Exit code 1 if `rebase_all_command()` or `clean_command()` returned `Err`. Since `reset_command(None)` always returns `Ok()` (swallowing individual errors internally), its result doesn't affect the exit code. This mirrors the actual risk profile — failed rebase or missing tmux is a blocker, swallowed reset errors are not.

**3. Skip tmux clean when tmux is not installed**
Before calling `clean_command()`, check if tmux is available (`which tmux`). If not found, skip silently. If tmux is present but `clean_command()` fails, treat that as a real error. This makes `tidy` safe for non-tmux users.

**4. Direct function calls, not process spawning**
Call the existing Rust functions directly rather than shelling out to `gbiv` subprocesses. This is simpler, faster, and matches how other commands in the codebase work.

**5. Full output with step headers**
Keep all sub-command output visible with a labeled header before each step. Users expect to see what's happening.

## Risks / Trade-offs

- [Step ordering is hardcoded] -> Acceptable for now since the three operations have a natural order. If more steps are added later, consider a configurable pipeline.
- [No `--skip` flags] -> Keeps the command simple. Users who want to skip a step can run the individual commands instead.
- [Error semantics inconsistency] -> `reset_command(None)` swallows errors while the others propagate. Accepted — matches the risk profile of each operation.
