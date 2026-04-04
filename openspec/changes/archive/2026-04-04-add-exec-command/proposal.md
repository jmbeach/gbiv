## Why

When working across multiple color worktrees, it's common to need to run the same command (e.g., `cargo build`, `git status`, `npm install`) in one or all worktrees. Currently there's no way to do this without manually `cd`-ing into each worktree directory. An `exec` subcommand would save time and reduce context-switching, especially for operations like installing dependencies or running builds across all worktrees.

## What Changes

- Add a new `exec` subcommand to the CLI: `gbiv exec [<color>|all] -- <command...>`
- When a single color is specified, run the command in that color's worktree repo directory
- When `all` is specified, run the command in every color worktree that exists, with colored output labels
- Support parallel execution across all worktrees (consistent with `rebase-all` pattern)
- Exit with non-zero status if any execution fails

## Capabilities

### New Capabilities
- `exec-command`: Execute arbitrary shell commands in one or all color worktree directories

### Modified Capabilities

_(none)_

## Impact

- **Code**: New `src/commands/exec.rs` module, new subcommand registration in `src/main.rs`
- **Dependencies**: No new crate dependencies expected (uses `std::process::Command`)
- **APIs**: New CLI subcommand `gbiv exec`
