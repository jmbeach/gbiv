## 1. New command module

- [x] 1.1 Create `src/commands/tmux/clean.rs` with `clean_subcommand()` returning a `clap::Command`
- [x] 1.2 Implement `clean_command()`: guard tmux available → find gbiv root → check session exists → list windows → filter orphans → kill each

## 2. Guard: tmux available

- [x] 2.1 Call `tmux -V` and return `Err("tmux not found. Please install tmux.")` if it fails (mirror `new_session.rs`)

## 3. Guard: gbiv project root

- [x] 3.1 Call `find_gbiv_root(&cwd)` and return an error if `None` (mirror `new_session.rs`)

## 4. Guard: session exists

- [x] 4.1 Call `tmux has-session -t <folder_name>` and return an error suggesting `gbiv tmux new-session` if the session is absent

## 5. Core logic

- [x] 5.1 Call `tmux list-windows -t <session> -F "#{window_name}"` and collect output into `Vec<String>`
- [x] 5.2 Parse `GBIV.md` via `parse_gbiv_md` and collect the set of tagged colors (`HashSet<String>`)
- [x] 5.3 Filter window names: keep only those in `COLORS` that are NOT in the active-color set
- [x] 5.4 For each orphaned window, call `tmux kill-window -t <session>:<name>`; on failure print a warning and set a flag to return non-zero after the loop
- [x] 5.5 Print each closed window name; if no orphans found print "Nothing to clean."

## 6. Wire into tmux command group

- [x] 6.1 Add `pub mod clean;` to `src/commands/tmux/mod.rs`
- [x] 6.2 Register `.subcommand(clean::clean_subcommand())` in `tmux_command()`
- [x] 6.3 Add `Some(("clean", _)) => clean::clean_command()` arm to `dispatch()`

## 7. Tests

- [x] 7.1 Unit test: tmux not found → error contains "tmux not found"
- [x] 7.2 Unit test: session not found → error suggests `gbiv tmux new-session`
- [x] 7.3 Unit test: window list filtering — given a set of window names and active colors, correct orphans are identified
