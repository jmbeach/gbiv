## 1. Create tidy command module

- [x] 1.1 Create `src/commands/tidy.rs` with `tidy_command() -> Result<(), String>` that:
  - Prints step headers before each sub-command
  - Calls `rebase_all_command()`, tracks if it returned Err
  - Calls `reset_command(None)` (result doesn't affect exit code)
  - Checks if tmux is on PATH; if so calls `clean_command()` and tracks Err; if not, skips silently
  - Returns `Err` if rebase-all or tmux clean failed, `Ok` otherwise
- [x] 1.2 Add `pub mod tidy;` to `src/commands/mod.rs`

## 2. Register CLI subcommand

- [x] 2.1 Add `tidy` subcommand to `cli()` in `src/main.rs` with `.about("Rebase all worktrees, reset merged branches, and clean tmux windows")`
- [x] 2.2 Add match arm in `main()` to dispatch to `tidy_command()`

## 3. Test

- [x] 3.1 Run tests to verify no regressions
