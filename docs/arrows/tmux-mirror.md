# Arrow: Tmux Mirror

Tmux session/window lifecycle synchronized with worktree layout.

**Status**: MAPPED (2026-04-23) — Brownfield inventory complete. All current behavior documented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § Component Architecture > Tmux Mirror |
| LLD | `docs/gbiv/llds/tmux-mirror.md` |
| EARS specs | `docs/gbiv/specs/tmux-mirror.md` |
| Source | `src/commands/tmux/mod.rs`, `src/commands/tmux/new_session.rs`, `src/commands/tmux/sync.rs`, `src/commands/tmux/clean.rs` |
| Tests | (no dedicated test files) |

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
