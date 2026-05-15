# Arrow: Observation

Status dashboard and cross-worktree command execution.

**Status**: AUDITED (audited 2026-05-15, git `1638fe0`) — All 46 specs implemented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § Component Architecture > Observation |
| LLD | `docs/gbiv/llds/observation.md` |
| EARS specs | `docs/gbiv/specs/observation.md` |
| Source | `crates/gbiv/src/commands/status.rs`, `crates/gbiv/src/commands/exec.rs` |
| Tests | `crates/gbiv/src/commands/exec.rs` (inline tests) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| Status | OBS-STATUS-001..026 | 26 | 0 | 0 |
| Exec | OBS-EXEC-001..020 | 20 | 0 | 0 |
| **Total** | | **46** | **0** | **0** |

## Key Findings

1. Status is read-only — no mutations to git state or GBIV.md.
2. Status collects git state in parallel (7 threads), joins in ROYGBIV order.
3. Conditional computation: merged/age/ahead-behind only computed when on a feature branch (branch != color).
4. Exec runs commands via `sh -c` — full shell semantics (pipes, redirects) work.
5. Exec "all" mode is all-or-nothing: any failure → overall Err, but output still contains all results.
6. Exec target parsing lives in main.rs, not exec.rs (split due to clap's flat arg handling).

## Dependencies

| This arrow depends on | For |
|---|---|
| CLI & Palette | Command routing, ANSI color codes, exec arg parsing |
| Worktree Lifecycle | git_utils for root discovery, status queries, color inference |
| Feature Ledger | status reads GBIV.md for ledger display section |

| Depended on by | For |
|---|---|
| (none) | Terminal output only — no other component reads from status or exec |
