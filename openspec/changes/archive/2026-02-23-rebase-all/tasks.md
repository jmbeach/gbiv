## 1. Refactor Shared Utility

- [x] 1.1 Move `find_repo_in_worktree` from `src/commands/status.rs` to `src/git_utils.rs` and make it `pub`
- [x] 1.2 Update `src/commands/status.rs` to import `find_repo_in_worktree` from `git_utils`

## 2. Implement rebase-all Command

- [x] 2.1 Create `src/commands/rebase_all.rs` with `rebase_all_command() -> Result<(), String>`
- [x] 2.2 Find gbiv root using `find_gbiv_root` and locate the `main` worktree repo path
- [x] 2.3 Run `git pull` in the `main` worktree; abort with error if it fails
- [x] 2.4 Iterate over `COLORS` in order; skip any color whose worktree directory does not exist
- [x] 2.5 For each color worktree, run `git rebase origin/main`; on failure run `git rebase --abort` and record the failure
- [x] 2.6 Print a per-worktree result line (color name + success/failure indicator) using existing ANSI color helpers
- [x] 2.7 Return `Err` if any worktree failed to rebase

## 3. Wire Up CLI

- [x] 3.1 Add `pub mod rebase_all;` to `src/commands/mod.rs`
- [x] 3.2 Register `rebase-all` subcommand in `cli()` in `src/main.rs`
- [x] 3.3 Add dispatch arm `Some(("rebase-all", _))` in `main()` calling `rebase_all_command()`

## 4. Verify

- [x] 4.1 Run `cargo build` and confirm it compiles without errors
- [x] 4.2 Run `gbiv rebase-all` in a gbiv repo and confirm pull + rebase output is correct
