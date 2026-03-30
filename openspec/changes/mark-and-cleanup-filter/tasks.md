## 1. Extend GBIV.md Parsing

- [ ] 1.1 Add `status: Option<String>` field to `GbivFeature` struct in `gbiv_md.rs`
- [ ] 1.2 Extend `parse_gbiv_md` to detect an optional `[status]` bracket after `[color]`, recognizing only `in-progress` and `done` as valid status values
- [ ] 1.3 Add `set_gbiv_feature_status(path, color, status: Option<&str>)` function that finds entry by color tag and adds/replaces/removes the status bracket tag in-place (`None` removes the tag for --unset)
- [ ] 1.4 Add unit tests for parsing entries with status tags, without status tags, and with unrecognized second brackets
- [ ] 1.5 Add unit tests for `set_gbiv_feature_status` (add status, replace status, unset status, no matching entry, preserve notes)

## 2. Mark Command

- [ ] 2.1 Add `infer_color_from_cwd(gbiv_root: &Path) -> Option<String>` utility in `git_utils.rs` that detects the current color worktree by checking which ROYGBIV color directory the CWD is under
- [ ] 2.2 Create `src/commands/mark.rs` with `mark_command(status: Option<&str>, color: Option<&str>)` that resolves color (explicit or inferred), locates gbiv root, calls `set_gbiv_feature_status`, and prints confirmation
- [ ] 2.3 Add `Mark` variant to `Commands` enum in `main.rs` with mutually exclusive `--in-progress`, `--done`, `--unset` flags (clap argument group) and optional `color` positional argument
- [ ] 2.4 Add `mark` module to `src/commands/mod.rs`
- [ ] 2.5 Wire up `Mark` match arm in `main()` to call `mark_command`

## 3. Cleanup Modifications

- [ ] 3.1 Modify `cleanup_command` (all-color path) to parse GBIV.md, check each color's status tag, and skip worktrees without `[done]` status — printing skip reason for `[in-progress]` entries
- [ ] 3.2 Add summary line at end of all-color cleanup with breakdown of skip reasons (no [done] status, not merged, already clean, missing worktree)
- [ ] 3.3 Single-color `cleanup <color>` retains current behavior (no status check)

## 4. Status Display

- [ ] 4.1 Update status command to read the `status` field from parsed GBIV.md features and display it in the worktree row after the color name (e.g., `red [done]  my-feature-branch ...`)

## 5. Testing

- [ ] 5.1 Add integration test for `mark` command (--done, --in-progress, --unset, invalid color, no flag, no matching entry, color inference from CWD, inference failure from non-color directory)
- [ ] 5.2 Add integration test for cleanup filtering (all-color skips non-done, single-color ignores status, summary line output)
- [ ] 5.3 Add test for status display showing worktree lifecycle status
- [ ] 5.4 Run `cargo nextest run` and verify all tests pass
