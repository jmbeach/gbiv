# Arrow: gbiv-orchestrate skill

A Claude Code skill that teaches a session what gbiv orchestration is and which `gbiv` CLI subcommands to invoke for which user intents.

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; skill file not authored.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/high-level-design.md` |
| LLD | `docs/llds/orchestrate-skill.md` |
| EARS specs | (none yet — pending `docs/specs/orchestrate-skill.md`) |
| Source | (none yet — target `skills/gbiv-orchestrate/SKILL.md`) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Form:** Single markdown file with YAML frontmatter. Installed at `~/.claude/skills/gbiv-orchestrate/SKILL.md`; lives in repo at `skills/gbiv-orchestrate/SKILL.md`.

**Sibling to the daemon:** The skill is the only gbiv-orchestrate artifact Claude Code reads. It is not a feature of the daemon — it ships alongside the binary.

## Work Required

- Author EARS specs in `docs/specs/orchestrate-skill.md`
- Draft skill content once orchestrate-cli is stable
