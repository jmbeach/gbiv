## 1. Add reset_hard helper

- [ ] 1.1 Add a `reset_hard(path: &Path, target: &str) -> Result<(), String>` function to `src/git_utils.rs` that runs `git reset --hard <target>`

## 2. Core Fix

- [ ] 2.1 In `cleanup_one` (`src/commands/cleanup.rs`), replace the `pull_remote(&repo_path, "origin", color)?` call with `reset_hard(&repo_path, &remote_main)?`, reusing the `remote_main` variable from the merge check
- [ ] 2.2 Remove the `pull_remote` import if no longer used in the file

## 3. Tests

- [ ] 3.1 Add a test for `cleanup_one` where cleanup succeeds — verify the color branch is reset to origin/main after cleanup
- [ ] 3.2 Run `cargo test` to verify all existing tests still pass
