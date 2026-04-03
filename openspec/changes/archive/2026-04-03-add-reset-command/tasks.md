## 1. Core Implementation

- [ ] 1.1 Create `src/commands/reset.rs` with `reset` function that accepts an `Option<String>` color argument
- [ ] 1.2 Implement color inference: if no color provided, detect current color from working directory path using `find_gbiv_root()` and path matching against COLORS
- [ ] 1.3 Implement reset logic: find worktree path, `checkout_branch` to color branch, `get_remote_main_branch`, `reset_hard` to remote main
- [ ] 1.4 Add error handling for: invalid color, not in gbiv project, color worktree not found, remote main branch not detected

## 2. CLI Integration

- [ ] 2.1 Add `reset` subcommand to `cli()` in `main.rs` with optional `color` positional argument (using COLORS value parser)
- [ ] 2.2 Add match arm in `main()` to dispatch to `reset::reset()`
- [ ] 2.3 Add `pub mod reset;` to `src/commands/mod.rs`

## 3. Testing

- [ ] 3.1 Add test for reset with explicit color argument on an unmerged feature branch
- [ ] 3.2 Add test for reset with inferred color from current directory
- [ ] 3.3 Add test for reset when already on color branch (reset only, no checkout needed)
- [ ] 3.4 Add test for error case: invalid color or not in gbiv project
- [ ] 3.5 Add test verifying GBIV.md is not modified after reset
