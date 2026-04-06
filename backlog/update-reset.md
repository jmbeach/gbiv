# Update `reset` for Multi-User

Modify `src/commands/reset.rs` to work with the state file and handle untracked worktrees.

## Changes to `reset_one`
1. Load state file
2. After resetting worktree: call `unassign_by_color`, save state
3. Remove GBIV.md entry by ID (looked up from state) instead of by color tag
4. If no state entry exists: still reset worktree, skip GBIV.md removal, print warning

## Changes to `reset_all_to_vec`
Replace color-tag lookup with state-file lookup:
- For each color: check state file for assignment -> find feature by ID -> check `[done]` status
- Handle "no state entry" case: if worktree is on a feature branch, stash if dirty, reset to color branch (no merge check)
- If already on color branch with no state entry: skip (already clean)

## Reset Decision Table

| Scenario | Normal | --hard |
|----------|--------|--------|
| State entry + [done] + merged | Reset + clean state + remove entry | Same |
| State entry + [done] + not merged | Error | Reset + stash + clean all |
| State entry + not [done] | Skip | Reset + stash + clean all |
| No state entry + feature branch | Stash if dirty, reset (no merge check) | Same |
| No state entry + color branch | Skip | Same |
