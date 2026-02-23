## 1. git_utils helpers

- [x] 1.1 Add `get_git_dir(path: &Path) -> Option<PathBuf>` to `git_utils.rs` — runs `git rev-parse --git-dir` and returns the absolute path to the `.git` directory (handles linked worktrees where `.git` is a file)
- [x] 1.2 Add `ensure_gitignore_entry(git_dir: &Path, entry: &str) -> Result<(), String>` to `git_utils.rs` — appends `entry` to `<git_dir>/info/exclude` if not already present, creating `info/` dir if needed
- [x] 1.3 Add `fetch_remote(path: &Path) -> Result<(), String>` to `git_utils.rs` — runs `git fetch origin` in the given path and returns stderr on failure
- [x] 1.4 Add `rebase_onto(path: &Path, upstream: &str) -> Result<(), String>` to `git_utils.rs` — runs `git rebase <upstream>` and on failure aborts with `git rebase --abort`, returning the error message

## 2. rebase-all command implementation

- [x] 2.1 Create `src/commands/rebase_all.rs` with a `rebase_all_command()` function
- [x] 2.2 In `rebase_all_command`, call `find_gbiv_root()` and error if not inside a gbiv tree
- [x] 2.3 Iterate over each colour in `COLORS`; for each colour worktree path `<root>/<color>/<repo>`:
  - [x] 2.3.1 Call `get_quick_status()` — skip with a warning if the worktree is dirty
  - [x] 2.3.2 Call `get_git_dir()` and `ensure_gitignore_entry()` to register `.last-branch` in `info/exclude`
  - [x] 2.3.3 Call `get_remote_main_branch()` to discover `origin/main` (or `origin/master`)
  - [x] 2.3.4 Call `get_ahead_behind_vs()` with the upstream ref — skip with "already up to date" if behind == 0
  - [x] 2.3.5 Call `fetch_remote()` then `rebase_onto()` — record success or failure for the summary
- [x] 2.4 After processing all colours, print a summary: "N/M worktrees rebased successfully" and exit non-zero if any failed

## 3. Wire up the subcommand

- [x] 3.1 In `src/commands/mod.rs`, add `pub mod rebase_all;`
- [x] 3.2 In `src/main.rs`, import `rebase_all_command` and add a `rebase-all` subcommand to the `cli()` builder (no arguments required)
- [x] 3.3 In the `main()` match block, handle `("rebase-all", _)` and call `rebase_all_command()`

## 4. Tests

- [x] 4.1 In `rebase_all.rs`, add a unit test that verifies `ensure_gitignore_entry` (via `git_utils`) does not duplicate entries
- [x] 4.2 Add an integration test that sets up a bare gbiv structure, creates a `.last-branch` file in a colour worktree, runs `rebase_all_command()`, and asserts the entry appears in `info/exclude`
- [x] 4.3 Add an integration test for a dirty worktree — assert the worktree is skipped and the function still returns without panicking
- [x] 4.4 Run `cargo test` and confirm all tests pass
