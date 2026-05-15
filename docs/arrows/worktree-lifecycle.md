# Arrow: Worktree Lifecycle

Creation, sync, reset, and maintenance of the 7-color worktree structure.

**Status**: AUDITED (audited 2026-05-15, git `1638fe0`) — 73 of 74 specs implemented; WTL-INIT-011 marker drift (see index.yaml `drift` field).

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § The Color Worktree, The Maintenance Loop |
| LLD | `docs/gbiv/llds/worktree-lifecycle.md` |
| EARS specs | `docs/gbiv/specs/worktree-lifecycle.md` |
| Source | `crates/gbiv-core/src/root.rs`, `crates/gbiv-core/src/gitignore.rs`, `crates/gbiv/src/git_utils.rs`, `crates/gbiv/src/commands/init.rs`, `crates/gbiv/src/commands/rebase_all.rs`, `crates/gbiv/src/commands/reset.rs`, `crates/gbiv/src/commands/tidy.rs` |
| Tests | `crates/gbiv/src/commands/reset_tests.rs`, `crates/gbiv/src/commands/reset_hard_basic_tests.rs`, `crates/gbiv/src/commands/reset_hard_allcolor_tests.rs`, `crates/gbiv/src/commands/reset_hard_stash_tests.rs`, `crates/gbiv/src/commands/tidy_tests.rs` |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| Init | WTL-INIT-001..011 | 10 | 0 | 1 (WTL-INIT-011 marker only) |
| Rebase | WTL-REBASE-001..017 | 17 | 0 | 0 |
| Reset | WTL-RESET-001..020 | 20 | 0 | 0 |
| Tidy | WTL-TIDY-001..007 | 7 | 0 | 0 |
| Utility Helpers | WTL-UTIL-001..019 | 19 | 0 | 0 |
| **Total** | | **73** | **0** | **1** |

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
