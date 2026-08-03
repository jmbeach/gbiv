# gbiv-orchestrate Skill — bundled `SKILL.md` content

Specs for the **content** of the bundled `gbiv-orchestrate` skill file itself
(frontmatter shape, required body sections) — a static-content contract, not
runtime behavior. `gbiv install-skill`'s file-placement, idempotency, and
versioning *mechanics* are specced separately in
`docs/specs/orchestrate-cli.md` (`INSTALL-CLI-*`); this file only specs what
the bundled `SKILL.md` must contain.

**Component LLD**: `docs/llds/orchestrate-skill.md`

## Frontmatter

- [x] **ORCH-SKILL-001**: The bundled `SKILL.md` shall have a YAML frontmatter block containing `name: gbiv-orchestrate`.
- [x] **ORCH-SKILL-002**: The frontmatter shall contain a `description` field mentioning "gbiv", "worktree", "session status", "send input", and "fleet" — the terms Claude Code's skill-matching uses to decide when to surface the skill.
- [x] **ORCH-SKILL-003**: The frontmatter shall contain a `version` field whose value is identical to the `gbiv` crate's `CARGO_PKG_VERSION` at build time (see `docs/specs/orchestrate-cli.md` `INSTALL-CLI-024` through `INSTALL-CLI-028`, which parse this field).

## Body Content

- [x] **ORCH-SKILL-010**: The body shall document the three primary commands — `gbiv fleet status`, `gbiv fleet get <color>`, `gbiv fleet send <color> <text>` — each with at least one worked example.
- [x] **ORCH-SKILL-011**: The body shall contain a decision table mapping user intents (fleet-wide status, single-color status, sending input, pagination, daemon-not-running, install/update requests) to the exact command or action to take, matching `docs/llds/orchestrate-skill.md` § "Decision Table".
- [x] **ORCH-SKILL-012**: The body shall contain a section explicitly enumerating actions the assistant must not take: sending prompt-shaped text (`yes`/`y`/`no`/`n`/single digits) on the user's behalf, auto-starting the daemon, auto-installing or auto-updating the skill, and touching `GBIV.md` or worktree state.
- [x] **ORCH-SKILL-013**: The body shall instruct the assistant to decline requests to answer a prompt in a worker's pane on the user's behalf, directing the user to the worker's own tmux window instead — regardless of phrasing, including paraphrases intended to slip past the shape-based guard (`docs/specs/http-server.md`'s guard specs).
- [x] **ORCH-SKILL-014**: The body shall instruct the assistant, on receiving a `gbiv fleet` exit code `2` (daemon not running), to inform the user and suggest `gbiv start`, without invoking it automatically.
- [x] **ORCH-SKILL-015**: The body shall instruct the assistant, when the user asks how to install or update the skill, to tell them to run `gbiv install-skill` (or `gbiv install-skill --scope project`) themselves rather than running it on their behalf.
- [x] **ORCH-SKILL-016**: The body shall instruct the assistant, on a `gbiv install-skill` exit code `7` (`refused`), to surface the response's `reason` field verbatim so the user can decide whether to re-run with `--force` or reconcile a hand-edit manually.
- [x] **ORCH-SKILL-017**: The body shall instruct the assistant, on a `gbiv fleet send` exit code `6` (`looks_like_prompt_response`), to read the full `explanation` field and relay its guidance rather than retrying with paraphrased text.

## References

- LLD: `docs/llds/orchestrate-skill.md`
- Companion: `docs/specs/orchestrate-cli.md` (`INSTALL-CLI-*` — the installer that places this file on disk)
- Companion: `docs/specs/http-server.md` (the prompt-response guard this skill's body must respect)
