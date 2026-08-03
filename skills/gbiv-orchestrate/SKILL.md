---
name: gbiv-orchestrate
version: 0.1.0
description: |
  Orchestrate Claude Code sessions running in gbiv color worktrees. Use when the user
  asks about session status for their parallel sessions, wants to send input to a
  specific worktree, or wants a fleet-wide summary. Requires the gbiv daemon to be
  running (gbiv start) in the main/ worktree.
---

# gbiv-orchestrate

gbiv runs a foreground HTTP daemon (`gbiv start`) in the `main/` worktree that
exposes the pane state of every ROYGBIV color's Claude Code session and lets
you send keystrokes into them. This skill translates natural-language
requests about the fleet into the right `gbiv fleet` invocation.

## The three primary commands

- **`gbiv fleet status`** — fleet-wide overview (JSON; last 35 lines per color, no server-side classification).
  ```
  gbiv fleet status
  ```
- **`gbiv fleet get <color>`** — detail on one session (JSON).
  ```
  gbiv fleet get red --lines 200
  ```
- **`gbiv fleet send <color> "<text>"`** — send input to one session (JSON response).
  ```
  gbiv fleet send red "please run the tests"
  ```

## Decision table

| User says | Skill does |
|---|---|
| "What's everyone doing?" / "fleet status" | `gbiv fleet status`. Read the `output` field yourself; for each `ok` color, give the user a one-sentence read of what's happening. For `pane_status != ok`, surface the status verbatim |
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

## What gbiv will not do

- Send `yes`, `y`, `no`, `n`, single digits, or other prompt-shaped responses (the guard will reject them; bypassing the guard by adding filler words is also off-limits as a matter of intent, not just shape)
- Auto-start the gbiv daemon
- Auto-install or auto-update the skill
- Touch GBIV.md or worktree state

The natural reading of "approve red" is "answer the y/n prompt" — push back
rather than guess, regardless of how the request is phrased.
