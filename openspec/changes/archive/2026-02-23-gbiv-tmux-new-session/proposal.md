## Why

Working across all 8 ROYGBIV worktrees requires manually opening a terminal window or pane for each one. `gbiv tmux new-session` automates this by creating a named tmux session with one window per worktree, so a developer can jump straight into work without setup overhead.

## What Changes

- Add a new `tmux` subcommand group to the `gbiv` CLI.
- Add `new-session` as a subcommand of `tmux`.
- `gbiv tmux new-session` discovers all gbiv worktrees rooted at the current directory (or a given path), creates a new tmux session, and opens one named window per worktree (`main`, `red`, `orange`, `yellow`, `green`, `blue`, `indigo`, `violet`), each with its working directory set to the worktree path (`<root>/<color>/<folder>/`).
- Exits with an error if `tmux` is not installed, no gbiv root is detected, or a session with the same name already exists.

## Capabilities

### New Capabilities

- `gbiv-tmux`: The `gbiv tmux` subcommand group. Entry point for all tmux-related gbiv commands.
- `gbiv-tmux-new-session`: The `gbiv tmux new-session` subcommand. Discovers worktrees under a gbiv root, starts a new tmux session, and opens one window per worktree.

### Modified Capabilities

<!-- No existing spec-level behavior changes. -->

## Impact

- **`src/main.rs`**: Register the `tmux` subcommand group and `new-session` subcommand.
- **`src/commands/mod.rs`**: Add `pub mod tmux`.
- **`src/commands/tmux/mod.rs`** (new): Define the `tmux` subcommand and dispatch to sub-subcommands.
- **`src/commands/tmux/new_session.rs`** (new): Implement worktree discovery and tmux session creation via `tmux new-session` / `tmux new-window` shell invocations.
- **`src/git_utils.rs`**: Possibly extend to add a helper that, given a path, detects whether it is a gbiv root and returns the list of worktree paths.
- **Dependencies**: No new Cargo dependencies required; tmux is called as an external process via `std::process::Command` (same pattern as existing git calls).
