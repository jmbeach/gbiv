## 1. Add cleanup success message

- [ ] 1.1 In `cleanup_one` in `src/commands/cleanup.rs`, add a `println!` after the `reset_hard` call (before the GBIV.md update) that prints: `"{color} worktree cleaned up (was on {branch}), reset to {remote_main}"`

## 2. Tests

- [ ] 2.1 Update the `cleanup_resets_color_branch_head_to_origin_main` test to verify the success message is printed (or add a new test that captures stdout and checks for the cleanup message)
