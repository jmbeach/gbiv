# Arrow: Tmux Mirror

Tmux session/window lifecycle synchronized with worktree layout.

**Status**: OK (audited 2026-05-15, git `1638fe0`) — All 40 specs implemented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § Component Architecture > Tmux Mirror |
| LLD | `docs/gbiv/llds/tmux-mirror.md` |
| EARS specs | `docs/gbiv/specs/tmux-mirror.md` |
| Source | `crates/gbiv/src/commands/tmux/mod.rs`, `crates/gbiv/src/commands/tmux/new_session.rs`, `crates/gbiv/src/commands/tmux/sync.rs`, `crates/gbiv/src/commands/tmux/clean.rs` |
| Tests | (no dedicated test files) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| New Session | TMX-SESSION-001..013 | 13 | 0 | 0 |
| Sync | TMX-SYNC-001..015 | 15 | 0 | 0 |
| Clean | TMX-CLEAN-001..012 | 12 | 0 | 0 |
| **Total** | | **40** | **0** | **0** |

## Key Findings

1. One tmux session per gbiv project, named after the repo folder.
2. Window names = color names. Canonical ROYGBIV ordering enforced by two-pass reorder.
3. Sync creates but never removes windows. Clean removes but never creates. Tidy composes both.
4. Clean has no `--session-name` flag — inconsistency with new-session and sync.
5. Sync and clean have slightly different active-color extraction logic (should share a helper).
6. No `delete-session` command — users must use raw tmux.

## Dependencies

| This arrow depends on | For |
|---|---|
| CLI & Palette | Command routing, COLORS constant |
| Worktree Lifecycle | git_utils for root discovery; tidy calls clean |
| Feature Ledger | sync/clean parse GBIV.md to determine active colors |

| Depended on by | For |
|---|---|
| Worktree Lifecycle | tidy calls clean_command() as step 3 |
