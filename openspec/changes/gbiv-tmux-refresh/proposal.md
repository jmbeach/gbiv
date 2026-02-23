## Why

`gbiv tmux new-session` always creates all 8 ROYGBIV windows regardless of which colors actually have work assigned to them in `GBIV.md`. As a project evolves, the set of active colors changes — a `refresh` command lets users synchronize an existing session's windows to match the current GBIV.md state without tearing down and recreating the session.

## What Changes

- Adds a new `gbiv tmux refresh` subcommand
- The command targets the tmux session whose name matches the gbiv project folder name
- It reads `GBIV.md` and collects the distinct set of color tags present on feature entries
- For each such color (in ROYGBIV order), it creates a window in the existing session if one does not already exist
- Windows for colors with no tagged features are not created (but existing windows are not removed)
- The `main` window is always ensured, regardless of GBIV.md content
- After ensuring all windows exist, the command reorders windows in the session so they appear in the canonical order: `main` first, then active colors in ROYGBIV order
- Exits with a non-zero status if the target session does not exist or tmux is unavailable

## Capabilities

### New Capabilities

- `gbiv-tmux-refresh`: The `gbiv tmux refresh` subcommand — reads GBIV.md, derives the active color set, and ensures the existing named tmux session has one window per active color.

### Modified Capabilities

- `gbiv-tmux`: The `tmux` subcommand group gains a new `refresh` entry in its help listing.

## Impact

- New source file: `src/commands/tmux/refresh.rs`
- Modified: `src/commands/tmux/mod.rs` — register `refresh` subcommand
- Modified: `src/main.rs` (or wherever subcommand dispatch happens) — handle `refresh` match arm
- Reads `GBIV.md` via existing `parse_gbiv_md` function in `src/gbiv_md.rs`
- No new dependencies
