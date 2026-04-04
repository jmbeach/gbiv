## Why

The `gbiv cleanup` command name doesn't clearly convey what the command does — it resets a color worktree back to the main branch after a feature is merged. "Reset" better describes this action (resetting the worktree to a clean state ready for new work) and aligns with common git terminology (`git reset`).

## What Changes

- **BREAKING**: Rename the `cleanup` subcommand to `reset` in the CLI
- Rename `src/commands/cleanup.rs` to `src/commands/reset.rs` and update module declaration
- Update all function names from `cleanup_*` to `reset_*`
- Update help text, error messages, and user-facing strings to use "reset" terminology
- Update README documentation to reference `gbiv reset`

## Capabilities

### New Capabilities

_(none — this is a rename of existing functionality)_

### Modified Capabilities

- `gbiv-cleanup`: Command is renamed from `cleanup` to `reset`. All behavior remains identical; only the command name and related identifiers change.
- `cleanup-output`: Output messages updated to use "reset" terminology instead of "cleanup".

## Impact

- **CLI**: Breaking change — users must use `gbiv reset` instead of `gbiv cleanup`
- **Code**: `src/commands/cleanup.rs` → `src/commands/reset.rs`, module and function renames
- **Docs**: README usage examples updated
- **Specs**: Existing `gbiv-cleanup` and `cleanup-output` specs updated to reflect new naming
