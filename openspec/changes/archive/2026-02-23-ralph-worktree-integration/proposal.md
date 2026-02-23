## Why

Running Ralph in parallel across all 7 ROYGBIV worktrees requires each agent to have exclusive, atomic access to the shared `prd.json` in the main worktree when selecting its next story. Without coordination, two agents can read the same unclaimed story simultaneously and both begin working on it. The gbiv CLI is the natural owner of this coordination since it already understands the worktree structure.

## What Changes

- New `gbiv prd lock` command: acquires an exclusive lock on `prd.json` by writing a `.prd.lock` file to the main worktree. Blocks (with retries) if already locked. Exits with error if lock cannot be acquired within timeout.
- New `gbiv prd unlock` command: releases the lock, but only if the calling worktree owns it. Exits with error if not the owner.
- Lock file lives at `<root>/main/<project>/prd.lock` alongside `prd.json`.
- Lock file contents: JSON with `worktree` (color name) and `pid` fields so stale locks can be detected.

## Capabilities

### New Capabilities
- `prd-lock`: Acquire and release an exclusive advisory lock on `prd.json` in the main worktree, enabling safe parallel Ralph agent execution across all ROYGBIV worktrees.

### Modified Capabilities
<!-- none -->

## Impact

- **New files**: `src/commands/prd.rs`
- **Modified files**: `src/commands/mod.rs`, `src/main.rs`
- **Runtime dependency**: None (uses filesystem locking only — no external deps)
- **Users**: Ralph agents running in any ROYGBIV worktree call `gbiv prd lock` before reading/modifying `prd.json`, then `gbiv prd unlock` after.
- **No breaking changes** to existing commands.
