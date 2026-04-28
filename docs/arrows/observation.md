# Arrow: Observation

Status dashboard and cross-worktree command execution.

**Status**: MAPPED (2026-04-23) — Brownfield inventory complete. All current behavior documented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/high-level-design.md` § Component Architecture > Observation |
| LLD | `docs/llds/observation.md` |
| EARS specs | `docs/specs/observation.md` |
| Source | `src/commands/status.rs`, `src/commands/exec.rs` |
| Tests | `src/commands/exec.rs` (inline tests) |

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
