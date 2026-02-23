## 1. Register subcommand

- [ ] 1.1 Create `src/commands/tmux/refresh.rs` with `refresh_subcommand()` returning a `clap::Command`
- [ ] 1.2 Add `refresh` match arm in `src/commands/tmux/mod.rs` dispatch and register the subcommand in the tmux group
- [ ] 1.3 Update `src/commands/tmux/mod.rs` help text to include `refresh` in listed subcommands

## 2. Core implementation

- [ ] 2.1 Implement `refresh_command()`: guard — check tmux is on PATH
- [ ] 2.2 Implement gbiv root detection (reuse `find_gbiv_root`); exit with error if not found
- [ ] 2.3 Derive session name from `gbiv_root.folder_name`; exit with actionable error if session does not exist (`tmux has-session`)
- [ ] 2.4 Parse GBIV.md via `parse_gbiv_md`; collect distinct ROYGBIV color tags from features; always include `main`
- [ ] 2.5 Filter active set to only recognized ROYGBIV colors (ignore unrecognized tags)
- [ ] 2.6 Query existing window names via `tmux list-windows -t <session> -F '#{window_name}'`
- [ ] 2.7 For each active color not already in the session, create a window with `tmux new-window -t <session> -n <color> -c <path>`; warn and skip if worktree path does not exist on disk

## 3. Window reordering

- [ ] 3.1 Build the desired ordered list: `main` first, then active colors in ROYGBIV sequence
- [ ] 3.2 Move each managed window to its target index using `tmux move-window -t <session>:<current> -t <session>:<target>`
- [ ] 3.3 Append any unmanaged (user-created) windows after all managed windows, preserving their relative order

## 4. Output and error handling

- [ ] 4.1 Print summary on success: number of windows created and confirmation of final order
- [ ] 4.2 Ensure all error paths return non-zero exit status with clear messages

## 5. Tests

- [ ] 5.1 Unit test: `refresh_command` returns error when tmux is not on PATH
- [ ] 5.2 Unit test: `refresh_command` returns error when not inside a gbiv project
- [ ] 5.3 Unit test: active color derivation — only ROYGBIV tags collected, unrecognized tags excluded, `main` always present
- [ ] 5.4 Unit test: window creation skipped for colors already in the session
- [ ] 5.5 Unit test: warning emitted and window skipped when worktree path does not exist
- [ ] 5.6 Unit test: window order after reorder matches canonical ROYGBIV sequence with unmanaged windows at end
