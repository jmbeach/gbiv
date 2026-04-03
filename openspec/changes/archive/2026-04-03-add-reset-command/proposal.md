## Why

The `cleanup` command requires that a feature branch is merged before it will reset a worktree back to the default branch. Sometimes you just want to abandon work or reset a worktree to a clean state regardless of merge status — for example, when experimenting or when a branch has gone stale. There's currently no quick way to do this.

## What Changes

- Add a new `reset [<color>]` subcommand that resets a color worktree to a clean state:
  - Checks out the color branch (the branch named after the color, e.g., `indigo`)
  - Runs `git reset --hard` to `origin/main` (or detected default remote branch)
  - Works regardless of whether the current feature branch has been merged
- If `<color>` is omitted, uses the color of the current worktree (inferred from the directory)
- Does **not** modify GBIV.md (unlike `cleanup`, which removes feature entries)

## Capabilities

### New Capabilities
- `reset-worktree`: Reset a color worktree to the default remote branch state, regardless of merge status

### Modified Capabilities
<!-- None — this is a new command that doesn't change existing behavior -->

## Impact

- New subcommand added to the CLI (`reset`)
- Uses existing git utilities: `checkout_branch`, `reset_hard`, `get_remote_main_branch`, `find_gbiv_root`
- No new dependencies required
- No breaking changes to existing commands
