# Arrow: roy CLI

The `roy` binary: daemon mode (`roy start`) and HTTP client subcommands (`roy status`, `roy get`, `roy send`).

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; no code.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/roy/high-level-design.md` |
| LLD | `docs/roy/llds/roy-cli.md` |
| EARS specs | (none yet — pending `docs/roy/specs/roy-cli.md`) |
| Source | (none yet) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Audience:** Designed first for an LLM (Claude Code with the roy skill). JSON-only output, structured error bodies, verbose `explanation` fields, distinct exit codes, no pretty-printing. Human use is supported but not optimized for.

**Dispatch:** clap subcommand groups handle daemon vs client routing from one binary.

## Work Required

- Author EARS specs in `docs/roy/specs/roy-cli.md`
- Implement once http-server is in place
