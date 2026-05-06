# Arrow: Feature Ledger

GBIV.md parsing, mutation, and the mark command.

**Status**: MAPPED (2026-04-23) — Brownfield inventory complete. All current behavior documented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § The Feature Lifecycle |
| LLD | `docs/gbiv/llds/feature-ledger.md` |
| EARS specs | `docs/gbiv/specs/feature-ledger.md` |
| Source | `src/gbiv_md.rs`, `src/commands/mark.rs` |
| Tests | `src/gbiv_md.rs` (inline tests) |

## Key Findings

1. GBIV.md is the sole persistent store for feature metadata — no database, no JSON.
2. Parser is line-based and greedy: consumes brackets until it hits an unrecognized one.
3. First-match vs all-match inconsistency: `set_gbiv_feature_status` affects first match, `remove_gbiv_features_by_tag` affects all matches.
4. Mark --unset is a no-op for missing entries; mark --done errors. Intentional asymmetry.
5. GBIV.md is always sourced from main worktree regardless of CWD.

## Dependencies

| This arrow depends on | For |
|---|---|
| CLI & Palette | Command routing |
| Worktree Lifecycle | git_utils for root discovery, color inference |

| Depended on by | For |
|---|---|
| Worktree Lifecycle | reset reads [done] status, removes entries |
| Observation | status displays ledger section |
| Tmux Mirror | sync/clean read active color tags |
