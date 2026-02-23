## 1. git_utils helpers

- [x] 1.1 Add `checkout_branch(path: &Path, branch: &str) -> Result<(), String>` to `git_utils.rs`
- [x] 1.2 Add `pull(path: &Path) -> Result<(), String>` to `git_utils.rs`

## 2. gbiv_md write support

- [x] 2.1 Add `remove_gbiv_features_by_tag(path: &Path, tag: &str) -> Result<(), String>` to `gbiv_md.rs` that reads, filters out entries matching the tag, and writes back while preserving content after `---`

## 3. Cleanup command implementation

- [x] 3.1 Create `src/commands/cleanup.rs` with `cleanup_one(gbiv_root, color) -> Result<(), String>` that: detects already-on-color-branch case, checks for remote, detects merge status, runs checkout + pull, removes GBIV.md entry
- [x] 3.2 Create `cleanup_command(color: Option<&str>) -> Result<(), String>` in the same file that dispatches to `cleanup_one` for each applicable color, continuing on non-fatal errors in all-color mode
- [x] 3.3 Add `pub mod cleanup;` to `src/commands/mod.rs`

## 4. CLI wiring

- [x] 4.1 Add `cleanup` subcommand to `cli()` in `main.rs` with an optional `color` positional argument validated against `COLORS`
- [x] 4.2 Add `Some(("cleanup", sub_matches)) =>` arm in `main()` dispatching to `cleanup_command`

## 5. Tests

- [x] 5.1 Unit test `remove_gbiv_features_by_tag`: removes matching entry, no-op when missing, removes multiple, preserves post-separator content
- [x] 5.2 Unit test `cleanup_one`: skips when already on color branch, errors when not merged, errors when no remote
- [x] 5.3 Run `cargo test` to confirm all tests pass
