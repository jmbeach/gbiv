## 1. Module Structure

- [x] 1.1 Create `src/commands/tmux/mod.rs` — define `tmux_command()` returning a `clap::Command` with `new-session` as a subcommand, and dispatch logic
- [x] 1.2 Create `src/commands/tmux/new_session.rs` — empty file with `pub fn new_session_command(session_name: Option<&str>) -> Result<(), String>` stub
- [x] 1.3 Add `pub mod tmux;` to `src/commands/mod.rs`

## 2. gbiv Root Detection

- [x] 2.1 Add `GbivRoot` struct to `src/git_utils.rs` with fields `root: PathBuf` and `folder_name: String`
- [x] 2.2 Implement `find_gbiv_root(start: &Path) -> Option<GbivRoot>` in `src/git_utils.rs` — walk upward from `start`, check if `P/main/<basename(P)>/` exists and is a git repo

## 3. Core new-session Implementation

- [x] 3.1 In `new_session.rs`: check that `tmux` is installed via `tmux -V`; return error "tmux not found" if it fails
- [x] 3.2 In `new_session.rs`: call `find_gbiv_root` on CWD; return error with suggestion to run `gbiv init` if not found
- [x] 3.3 In `new_session.rs`: check for duplicate session with `tmux has-session -t <name>`; return error with `tmux attach` suggestion if it exists
- [x] 3.4 In `new_session.rs`: build the list of 8 worktree paths (`main` + ROYGBIV colors); skip and warn for any path that does not exist on disk
- [x] 3.5 In `new_session.rs`: create the session detached using `tmux new-session -d -s <name> -c <main-path> -n main`
- [x] 3.6 In `new_session.rs`: for each remaining worktree (red → violet), call `tmux new-window -t <name> -n <color> -c <path>`
- [x] 3.7 In `new_session.rs`: print session name and success message on completion

## 4. CLI Wiring

- [x] 4.1 In `src/main.rs`: add `tmux` subcommand to the `cli()` function using `tmux::tmux_command()`
- [x] 4.2 In `src/main.rs`: add `("tmux", ...)` arm to the `match` in `main()` to dispatch into `tmux::dispatch()`
- [x] 4.3 Add `--session-name` optional argument to the `new-session` subcommand definition in `src/commands/tmux/mod.rs`

## 5. Tests

- [x] 5.1 Unit test `find_gbiv_root`: returns `Some` when called from inside a valid gbiv structure
- [x] 5.2 Unit test `find_gbiv_root`: returns `None` when called from a non-gbiv directory
- [x] 5.3 Unit test `new_session_command`: returns error when tmux is not available (mock or skip if tmux present)
- [x] 5.4 Integration smoke test: run `gbiv tmux new-session --help` and verify exit 0
