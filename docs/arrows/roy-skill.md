# Arrow: roy Skill

A Claude Code skill that teaches a session what roy is and which `roy` CLI subcommands to invoke for which user intents.

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; skill file not authored.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/roy/high-level-design.md` |
| LLD | `docs/roy/llds/roy-skill.md` |
| EARS specs | (none yet — pending `docs/roy/specs/roy-skill.md`) |
| Source | (none yet — target `skills/roy/SKILL.md`) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Form:** Single markdown file with YAML frontmatter. Installed at `~/.claude/skills/roy/SKILL.md`; lives in repo at `skills/roy/SKILL.md`.

**Sibling to the daemon:** The skill is the only roy artifact Claude Code reads. It is not a feature of the daemon — it ships alongside the binary.

## Work Required

- Author EARS specs in `docs/roy/specs/roy-skill.md`
- Draft skill content once roy-cli is stable
