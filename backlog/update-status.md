# Update `status` for Multi-User

Modify `src/commands/status.rs` to surface `[by:name]` info.

## Default Display
For each color worktree, if the `[color]` entry has a `[by:name]` tag,
show the name alongside the feature description (e.g. dimmed `(jared)` suffix).

## `--mine` Flag
When passed, filter the display to only show worktrees whose `[color]` entry
has `[by:<user.name>]` matching the configured user. Error if `user.name` is unset.
