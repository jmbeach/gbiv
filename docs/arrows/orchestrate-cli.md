# Arrow: gbiv orchestration CLI

The `gbiv` binary: daemon mode (`gbiv start`) and HTTP client subcommands (`gbiv fleet status`, `gbiv fleet get`, `gbiv fleet send`).

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; no code.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/high-level-design.md` |
| LLD | `docs/llds/orchestrate-cli.md` |
| EARS specs | (none yet — pending `docs/specs/orchestrate-cli.md`) |
| Source | (none yet) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Audience:** Designed first for an LLM (Claude Code with the gbiv-orchestrate skill). JSON-only output, structured error bodies, verbose `explanation` fields, distinct exit codes, no pretty-printing. Human use is supported but not optimized for.

**Dispatch:** clap subcommand groups handle daemon vs client routing from one binary.

## Work Required

- Author EARS specs in `docs/specs/orchestrate-cli.md`
- Implement once http-server is in place
