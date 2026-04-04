## 1. Rename Source File and Module

- [ ] 1.1 Rename `src/commands/cleanup.rs` to `src/commands/reset.rs`
- [ ] 1.2 Update `src/commands/mod.rs`: change `pub mod cleanup` to `pub mod reset`

## 2. Rename Functions and Update Internals

- [ ] 2.1 Rename `cleanup_one` to `reset_one` in `src/commands/reset.rs`
- [ ] 2.2 Rename `cleanup_all_to_vec` to `reset_all_to_vec` in `src/commands/reset.rs`
- [ ] 2.3 Rename `cleanup_command` to `reset_command` in `src/commands/reset.rs`
- [ ] 2.4 Update all user-facing strings in `src/commands/reset.rs` (e.g., "cleaned up" → "reset", "already clean" → "already reset")

## 3. Update CLI Registration

- [ ] 3.1 In `src/main.rs`, rename the subcommand from `"cleanup"` to `"reset"` and update help text
- [ ] 3.2 Update the match arm from `Some(("cleanup", ...))` to `Some(("reset", ...))` and call `reset_command`
- [ ] 3.3 Update the import from `commands::cleanup` to `commands::reset`

## 4. Update Tests

- [ ] 4.1 Update test function names and assertions in `src/commands/reset.rs` to use "reset" terminology
- [ ] 4.2 Run `cargo nextest run` and verify all tests pass

## 5. Update Documentation

- [ ] 5.1 Update README.md: replace all `gbiv cleanup` references with `gbiv reset`
