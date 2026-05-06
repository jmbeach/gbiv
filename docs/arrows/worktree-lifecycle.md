# Arrow: Worktree Lifecycle

Creation, sync, reset, and maintenance of the 7-color worktree structure.

**Status**: MAPPED (2026-04-23) — Brownfield inventory complete. All current behavior documented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § The Color Worktree, The Maintenance Loop |
| LLD | `docs/gbiv/llds/worktree-lifecycle.md` |
| EARS specs | `docs/gbiv/specs/worktree-lifecycle.md` |
| Source | `src/git_utils.rs`, `src/commands/init.rs`, `src/commands/rebase_all.rs`, `src/commands/reset.rs`, `src/commands/tidy.rs` |
| Tests | `src/commands/reset_tests.rs`, `src/commands/reset_hard_basic_tests.rs`, `src/commands/reset_hard_allcolor_tests.rs`, `src/commands/reset_hard_stash_tests.rs` |

## Key Findings

1. `git_utils.rs` is the most depended-on module (~457 lines). Mixes repo discovery with git command wrappers. Could split but coupling is tight enough to leave.
2. Rebase-all uses parallel threads per color — safe because worktrees are independent.
3. Reset has two distinct modes (soft/hard) with different preconditions. The decision table in the LLD captures all cases.
4. Tidy is a thin orchestrator: rebase-all → reset (soft) → tmux clean. Swallows reset errors.
5. Init has full rollback on failure — removes created worktrees and restores original folder.

## Dependencies

| This arrow depends on | For |
|---|---|
| CLI & Palette | Command routing, COLORS constant |
| Feature Ledger | GBIV.md reads (reset filters by [done], reset removes entries) |

| Depended on by | For |
|---|---|
| Observation | status reads git state via git_utils |
| Tmux Mirror | tidy calls tmux clean; tmux commands use find_gbiv_root |
| Feature Ledger | reset removes GBIV.md entries |
