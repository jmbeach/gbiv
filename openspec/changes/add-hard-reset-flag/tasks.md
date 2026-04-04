## 1. CLI Arguments

- [ ] 1.1 Add `--hard` flag to the reset command definition in `src/main.rs` (clap arg, bool)
- [ ] 1.2 Add `--yes`/`-y` flag to the reset command definition in `src/main.rs` (clap arg, bool)

## 2. Git Utilities

- [ ] 2.1 Add `stash_push(path, message)` helper to `src/git_utils.rs` that runs `git stash push -u -m "<message>"`
- [ ] 2.2 Add a way to check if a worktree is dirty (may already be covered by `get_quick_status`)

## 3. Core Implementation

- [ ] 3.1 Add `hard: bool` parameter to `reset_one` in `src/commands/reset.rs` — when hard: skip "already on color branch" early return, skip merge check, stash if dirty before checkout (abort on stash failure)
- [ ] 3.2 Add `hard: bool` parameter to `reset_all_to_vec` in `src/commands/reset.rs` — when hard: iterate ALL 7 colors unconditionally (skip `[done]` filter and GBIV.md entry requirement)
- [ ] 3.3 Update `reset_command` to pass `hard` and `yes` flags through; add confirmation prompt for all-color `--hard` (list worktrees and current branches, default No, skip with `--yes`)

## 4. Tests

- [ ] 4.1 Add test for single-color hard reset with unmerged branch (should succeed)
- [ ] 4.2 Add test for hard reset when already on color branch (should still reset)
- [ ] 4.3 Add test for all-color hard reset resetting worktrees without GBIV.md entries
- [ ] 4.4 Add test for stash being created when worktree is dirty during hard reset
- [ ] 4.5 Add test for stash failure aborting reset for that worktree
- [ ] 4.6 Verify existing tests still pass (no regression in default behavior)
