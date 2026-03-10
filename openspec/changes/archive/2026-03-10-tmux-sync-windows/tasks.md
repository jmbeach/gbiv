## 1. Module Setup

- [x] 1.1 Create `src/commands/tmux/sync.rs` with the `sync` function signature and imports
- [x] 1.2 Add `mod sync;` to `src/commands/tmux/mod.rs` and wire up the `Sync` subcommand variant with `--session-name` option
- [x] 1.3 Add dispatch arm in `src/main.rs` to call the sync handler

## 2. Core Implementation

- [x] 2.1 Implement pre-flight guards: verify tmux installed, find gbiv root, verify session exists
- [x] 2.2 Parse GBIV.md and extract active color set (colors that have at least one tagged feature)
- [x] 2.3 List existing tmux windows in the session and determine which active colors are missing
- [x] 2.4 Create new tmux windows for missing active colors, setting working directory to the worktree path (skip with warning if worktree doesn't exist)
- [x] 2.5 Reorder all windows to ROYGBIV order using `tmux move-window`, placing non-color windows after color windows

## 3. Testing

- [x] 3.1 Add unit tests for the active-color extraction logic (parsing GBIV.md features into a set of valid colors)
- [x] 3.2 Run `cargo test` to verify all existing and new tests pass
