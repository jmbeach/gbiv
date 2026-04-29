# gbork Skill

**Created**: 2026-04-28
**Status**: Draft

## Context

The gbork skill is a Claude Code skill (a markdown file with YAML frontmatter) that teaches a Claude Code session what gbork is and which `gbork` CLI subcommands to invoke for which user intents. Without the skill, a user would have to remember API shapes and CLI flags. With it, the user types natural-language requests and the assistant translates.

The skill is the *only* gbork artifact Claude Code itself reads. It is not a feature of the daemon — it is a sibling deliverable that ships alongside the binary.

## Skill File

Location when installed: `~/.claude/skills/gbork/SKILL.md`
Location in repo: `skills/gbork/SKILL.md` (a top-level `skills/` directory in the gbiv workspace)

The skill is a single markdown file (no extra references in v1). Frontmatter:

```yaml
---
name: gbork
description: |
  Orchestrate Claude Code sessions running in gbiv color worktrees. Use when the user
  asks about the status of their parallel sessions, wants to send input to a specific
  worktree, or wants a fleet-wide summary. Requires the gbork daemon to be running
  (gbork start) in the main/ worktree.
---
```

The description is what triggers Claude Code to surface the skill — it should mention "gbiv", "worktree", "session status", "send input", and "fleet."

## When the Skill Activates

The skill is invoked when the user's intent matches any of:

- "What's the status of all my sessions?"
- "Is anyone waiting for input?"
- "What is red doing?"
- "Send red the answer 'yes'"
- "Approve the question in orange"
- "Tell green to run the tests"
- "Summarize the work in flight"

The skill should NOT activate for:

- General gbiv worktree management ("create a new red worktree") — that's gbiv, not gbork.
- Code editing or running commands in the *current* worktree — the user can already do that directly.

## Skill Body Structure

The body teaches three things, in order:

1. **What gbork is** (one paragraph): a daemon that exposes pane state and lets you send keys.
2. **The three primary commands** with examples and when to use each:
   - `gbork status --json` for fleet-wide overview
   - `gbork get <color> --json` for detail on one session
   - `gbork send <color> "<text>"` for sending input
3. **Decision table** mapping user intents → command sequence.

The body's tone is operational: short, command-first, with worked examples. No background, no philosophy.

### Decision Table (in skill body)

| User says | Skill does |
|---|---|
| "What's everyone doing?" / "fleet status" | `gbork status --json`; summarize each color in 1-2 lines, highlight any `pane_status != ok` |
| "Is anyone waiting?" | `gbork status --json`; scan output for prompts (e.g., lines ending in `?` or matching `(y/n)`); call out matches |
| "What's red doing?" | `gbork get red --json --lines 200`; summarize the last meaningful activity |
| "Show me red's full output" | `gbork get red --lines 500`; print the raw output back to the user (don't re-summarize unless asked) |
| "Tell red yes" / "approve red" | Read `gbork get red` first to verify a prompt is pending; then `gbork send red "yes"` (or "y", whichever the prompt expects) |
| "Send red <message>" | `gbork send red "<message>"` directly |
| Daemon not running (exit code 2) | Tell the user; suggest running `gbork start` in the `main/` worktree; do not auto-start |

## Two-Step Pattern for Sends

The skill instructs the assistant to **read before send** when the user's intent is approval-shaped ("approve", "say yes", "answer the question") rather than literal-text-shaped ("tell red <text>"). The pattern:

```
1. gbork get <color> --json --lines 50
2. Inspect the tail for an active prompt
3. Pick the appropriate response (yes / y / 1 / approve / etc.)
4. gbork send <color> "<chosen response>"
5. Optionally: gbork get <color> after a brief pause to confirm the response was received
```

Skipping step 1 risks sending "yes" to a session that wasn't actually waiting for confirmation. The skill body says this explicitly.

## Bootstrapping

The skill does not auto-install gbork. If `gbork status` returns "command not found" the skill instructs the user to install with `cargo install gbork` (or whatever the v1 install method becomes).

The skill also does not auto-start the daemon. The reasoning: starting the daemon ties up a tmux pane (foreground-only). The user needs to make a deliberate choice about where to run it.

## Distribution

The skill ships in the gbiv repo at `skills/gbork/SKILL.md`. v1 installation is manual:

```
cp -r skills/gbork ~/.claude/skills/
```

A future `gbork install-skill` subcommand could automate this. Out of scope for v1.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Skill is a single SKILL.md | Yes | Multi-file with examples/, references/ | v1 surface is small; one file is faster to update |
| Skill auto-starts the daemon | No | Have the skill run `gbork start` in the background | Foreground-only daemon contradicts auto-starting; user should know where the daemon lives |
| Read-before-send pattern | In the skill body | Implemented in `gbork send` itself | Keeping `gbork send` literal lets non-skill callers (scripts) do exactly what they ask. The "read first" judgment is an LLM concern |
| Always use `--json` | Yes for skill, optional for CLI users | Skill parses the human-readable table | Parsing structured output is reliable; humans get the table |
| Skill triggers on "gbiv worktree status" | No, only on session-orchestration intents | Trigger broadly | Avoids overlap with gbiv's own commands; gbork is specifically about agent sessions |
| Skill location | `~/.claude/skills/gbork/SKILL.md` | Project-local skill, plugin install | Global skill works across all projects where gbork is used |

## Edge Cases

| Case | Skill behavior |
|---|---|
| User asks for status, daemon not running | Surface the exit-code-2 message; recommend `gbork start` |
| User asks "send red yes" but red has no claude pane | Don't send; report the resolution status (`no_claude_pane`); ask the user if they want to start a session there |
| User asks for status of a non-ROYGBIV color (e.g., "purple") | Decline; mention that gbork only knows ROYGBIV |
| Output of `gbork get` is huge | Summarize (don't dump); offer to show raw with a follow-up |
| Multiple claude panes in one window (`multiple_claude_panes`) | Surface the ambiguity to the user; do not pick a pane to send to |
| User asks the skill to do something gbork can't (kill a session, etc.) | Decline; point at the gbiv equivalent or tmux directly |

## Technical Debt & Future Work

1. **Manual install.** Until `gbork install-skill` exists, users have to copy the file.
2. **No skill versioning.** When the CLI surface changes, the skill body must be updated in lockstep. Consider a sentinel comment ("requires gbork ≥ 0.2") in the skill body.
3. **Response heuristics are LLM-side.** The skill teaches "read before send" but the model's prompt-detection accuracy is what it is. A future enhancement: have `gbork status` itself flag "looks like a prompt" using a deterministic regex, so the skill doesn't have to.
4. **No multi-language support.** Skill is English-only.

## References

- HLD: `docs/gbork/high-level-design.md` § Components > gbork skill, § Skill-driven UX
- Companion: `docs/gbork/llds/gbork-cli.md` (the commands the skill invokes)
- Claude Code skills format: external (Anthropic docs)
