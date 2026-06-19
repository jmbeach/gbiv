# gbiv-orchestrate Skill

**Created**: 2026-04-28
**Status**: Draft

## Context

The gbiv-orchestrate skill is a Claude Code skill (a markdown file with YAML frontmatter) that teaches a Claude Code session what gbiv is and which `gbiv` CLI subcommands to invoke for which user intents. Without the skill, a user would have to remember API shapes and CLI flags. With it, the user types natural-language requests and the assistant translates.

The skill is the *only* gbiv artifact Claude Code itself reads. It is not a feature of the daemon — it is a sibling deliverable that ships alongside the binary.

## Skill File

Location when installed: `~/.claude/skills/gbiv-orchestrate/SKILL.md`
Location in repo: `skills/gbiv-orchestrate/SKILL.md` (a top-level `skills/` directory in the gbiv workspace)

The skill is a single markdown file (no extra references in v1). Frontmatter:

```yaml
---
name: gbiv-orchestrate
description: |
  Orchestrate Claude Code sessions running in gbiv color worktrees. Use when the user
  asks about the status of their parallel sessions, wants to send input to a specific
  worktree, or wants a fleet-wide summary. Requires the gbiv daemon to be running
  (gbiv start) in the main/ worktree.
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

- General gbiv worktree management ("create a new red worktree") — that's gbiv, not gbiv.
- Code editing or running commands in the *current* worktree — the user can already do that directly.

## Skill Body Structure

The body teaches three things, in order:

1. **What gbiv is** (one paragraph): a daemon that exposes pane state and lets you send keys.
2. **The three primary commands** with examples and when to use each:
   - `gbiv fleet status` for fleet-wide overview (always JSON)
   - `gbiv fleet get <color>` for detail on one session (always JSON)
   - `gbiv fleet send <color> "<text>"` for sending input (JSON response)
3. **Decision table** mapping user intents → command sequence.

The body's tone is operational: short, command-first, with worked examples. No background, no philosophy.

### Decision Table (in skill body)

| User says | Skill does |
|---|---|
| "What's everyone doing?" / "fleet status" | `gbiv fleet status` (JSON; returns last 35 lines per color, no server-side classification). Read the `output` field yourself; for each `ok` color, give the user a one-sentence read of what's happening. For `pane_status != ok`, surface the status verbatim |
| "Is anyone waiting?" | `gbiv fleet status`; read the tail of each `ok` color's `output` and decide which (if any) look like prompts (`(y/n)`, "Continue?", numbered choices, AskUserQuestion text). Call out matches but **do not offer to answer them**. If 35 lines isn't enough to tell, follow up with `gbiv fleet get <color> --lines 100` |
| "What's red doing?" | `gbiv fleet get red --lines 200`; summarize the last meaningful activity from the `output` field |
| "Show me red's full output" | `gbiv fleet get red --lines 500`; print the `output` field back to the user (don't re-summarize unless asked). If the response sets `output_truncated: true`, mention it and offer to page back |
| "Show me everything red has done" / "page back further" | Read `range_returned.start_line` from the previous response; call `gbiv fleet get red --start-line=<previous_start - 500> --end-line=<previous_start - 1>` and concatenate the `output` fields. Stop when `output_truncated` becomes false or the user has seen enough. Use `--start-line=top` for the final chunk |
| "Tell red yes" / "approve red" / "answer red's prompt" | **Decline.** gbiv refuses to send prompt-shaped responses (`yes`, `y`, `1`, etc.). Tell the user they need to switch to red's tmux window and answer the prompt themselves. Do not try to dress up the answer as natural language to bypass the guard |
| "Send red <multi-word message>" | `gbiv fleet send red "<message>"` directly |
| Daemon not running (exit code 2) | Tell the user; suggest running `gbiv start` in the `main/` worktree; do not auto-start |
| User asks how to install/update the skill | Tell them to run `gbiv install-skill` (user-scope) or `gbiv install-skill --scope project` (project-scope); do not run it for them — installation is a deliberate user choice |
| `gbiv install-skill` exited 7 (`refused`) | Surface the `reason` field verbatim so the user can decide whether to `--force` or reconcile their hand-edits manually |
| `gbiv fleet send` exits 6 (`looks_like_prompt_response`) | Read the full `explanation` field from the response and follow it. Tell the user the worker appears to be waiting on a prompt and ask them to answer it in the worker's tmux window themselves. Do **not** retry with a paraphrased version of the same intent |

## What gbiv Will Not Do

The skill body has a short, prominent section telling the assistant **not** to:

- Send `yes`, `y`, `no`, `n`, single digits, or other prompt-shaped responses (the guard will reject them; bypassing the guard by adding filler words is also off-limits as a matter of intent, not just shape)
- Auto-start the gbiv daemon
- Auto-install the skill
- Touch GBIV.md or worktree state

These are enumerated explicitly because the natural reading of "approve red" is "answer the y/n prompt" — and the assistant must learn to push back rather than guess.

## Bootstrapping

The skill does not auto-install gbiv. If `gbiv fleet status` returns "command not found" the skill instructs the user to install with `cargo install gbiv` (or whatever the v1 install method becomes).

The skill also does not auto-start the daemon. The reasoning: starting the daemon ties up a tmux pane (foreground-only). The user needs to make a deliberate choice about where to run it.

## Versioning

The skill's frontmatter carries a `version:` field that matches the `gbiv` crate version it shipped with:

```yaml
---
name: gbiv-orchestrate
version: 0.2.0
description: |
  ...
---
```

`gbiv install-skill` reads this field on both the bundled and on-disk copies to decide whether an update is safe (same version + different content = user hand-edit, refuse; older version = safe upgrade). The skill body itself does not need to consult the version — it's metadata for the installer.

## Distribution

The skill is bundled into the `gbiv` binary at compile time (`include_str!` / `include_dir!`) and written to disk by `gbiv install-skill`:

```
gbiv install-skill                  # writes ~/.claude/skills/gbiv-orchestrate/
gbiv install-skill --scope project  # writes .claude/skills/gbiv-orchestrate/
```

See orchestrate-cli LLD § "gbiv install-skill" for the full surface (idempotency, conflict policy, exit codes, JSON output shape).

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Skill is a single SKILL.md | Yes | Multi-file with examples/, references/ | v1 surface is small; one file is faster to update |
| Skill auto-starts the daemon | No | Have the skill run `gbiv start` in the background | Foreground-only daemon contradicts auto-starting; user should know where the daemon lives |
| Read-before-send pattern | In the skill body | Implemented in `gbiv fleet send` itself | Keeping `gbiv fleet send` literal lets non-skill callers (scripts) do exactly what they ask. The "read first" judgment is an LLM concern |
| CLI is JSON-only | Yes — the skill parses `output` and `range_returned` fields directly | Pretty/table mode for humans, JSON for skill | One surface, no flag confusion. Humans use `jq` |
| Skill triggers on "gbiv worktree status" | No, only on session-orchestration intents | Trigger broadly | Avoids overlap with gbiv's own commands; gbiv is specifically about agent sessions |
| Skill location | `~/.claude/skills/gbiv-orchestrate/SKILL.md` | Project-local skill, plugin install | Global skill works across all projects where gbiv is used |

## Edge Cases

| Case | Skill behavior |
|---|---|
| User asks for status, daemon not running | Surface the exit-code-2 message; recommend `gbiv start` |
| User asks "send red <multi-word message>" but red has no claude pane | Don't retry; surface the resolution status (`no_claude_pane`) verbatim and tell the user red's window has no claude process to receive input. Suggest they check whether claude is actually running in red |
| User insists "just send 'yes please' so it goes through the guard" | Decline. The guard is shape-based, not intent-based, so paraphrases would technically pass — but the rule's purpose is to prevent the assistant from answering prompts on the user's behalf. Honor the spirit |
| User asks for status of a non-ROYGBIV color (e.g., "purple") | Decline; mention that gbiv only knows ROYGBIV |
| Output of `gbiv fleet get` is huge | Summarize (don't dump); offer to show raw with a follow-up. If `output_truncated: true`, the visible portion is only the most recent rows — say so before summarizing, so the user knows the summary may be missing earlier context |
| Pane is still actively producing output during paginated reads | Row offsets are relative to the bottom-of-pane *at call time*, so chunks may overlap or skip if the buffer scrolls between calls. Tell the user the snapshot is approximate; recommend re-reading from the tail rather than continuing pagination |
| Multiple claude panes in one window (response includes `other_claude_panes`) | The send/get already targeted the oldest claude pane automatically. Mention to the user that other claude panes were found in that window in case the wrong one was picked, but do not block the operation |
| User asks the skill to do something gbiv can't (kill a session, etc.) | Decline; point at the gbiv equivalent or tmux directly |

## Technical Debt & Future Work

1. **Skill version tracks the `gbiv` crate exactly.** A skill body change without a CLI surface change still bumps the version because they're co-released. A separate skill version that drifts from the binary version is possible later if the skill develops an independent release cadence.
2. **Prompt-response guard is shape-based.** A user could theoretically ask the skill to send "yep okay" and the guard would let it through. The skill body asks the assistant to honor intent, not just shape. A stricter guard (e.g., LLM-side classification, or refusing all sends shorter than N characters) is a future option.
3. **No multi-language support.** Skill is English-only.
4. **`install-skill` does not unwind a partial write.** If `SKILL.md` writes succeed but a future companion file fails, the skill directory is left half-updated. Acceptable while the skill is one file; revisit when that changes.

## References

- HLD: `docs/high-level-design.md` § Components > gbiv-orchestrate skill, § Skill-driven UX
- Companion: `docs/llds/orchestrate-cli.md` (the commands the skill invokes)
- Claude Code skills format: external (Anthropic docs)
