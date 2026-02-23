## Why

Users need a way to clean up color worktrees after their work has been merged,
automating the checkout + pull flow and removing associated features from GBIV.md.

## What Changes

- Adds `gbiv cleanup [<color>]` command
- When `<color>` is specified: validates the branch is merged, checks out the
  color branch, pulls latest, and removes the associated feature entry from `GBIV.md`
- When no `<color>` is specified: runs the above for all active color worktrees

## Capabilities

### New Capabilities
- `gbiv-cleanup`: Cleanup command that detects if the feature branch is merged,
  then checks out the color branch, pulls latest, and removes feature entries
  from GBIV.md for one or all color worktrees

### Modified Capabilities
- (none)

## Impact

- New subcommand in the gbiv CLI binary
- Reads/writes `GBIV.md` to remove feature entries
- Interacts with git (branch check, checkout, pull)
- Affects worktree state on disk
