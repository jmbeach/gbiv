## 1. Module Setup

- [ ] 1.1 Create `src/commands/exec.rs` with module structure
- [ ] 1.2 Register `exec` subcommand in `src/main.rs` CLI definition with clap (target: `[<color>|all]`, trailing args after `--`)
- [ ] 1.3 Add `exec` match arm in `main.rs` to dispatch to the new module

## 2. Single Color Execution

- [ ] 2.1 Implement `exec_single(root: &Path, color: &str, command: &[String])` — resolves worktree path, runs `sh -c` with inherited stdio
- [ ] 2.2 Validate color against `COLORS` constant and check worktree directory exists
- [ ] 2.3 Implement color inference from CWD using `infer_color_from_path()` when no target is specified

## 3. All-Worktrees Execution

- [ ] 3.1 Implement `exec_all(root: &Path, command: &[String])` — spawns threads per existing color worktree, collects output
- [ ] 3.2 Print collected results in ROYGBIV order with color-labeled headers
- [ ] 3.3 Return non-zero exit status if any command failed

## 4. Testing

- [ ] 4.1 Add tests for single-color exec (valid color, missing worktree, invalid color)
- [ ] 4.2 Add tests for all-worktrees exec (mixed success/failure, missing worktrees skipped)
- [ ] 4.3 Add test for color inference from CWD
