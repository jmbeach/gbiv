---
name: gbork
description: |
  Orchestrate Claude Code sessions running in gbiv color worktrees. Use when the user
  asks about the status of their parallel sessions, wants to send input to a specific
  worktree (red/orange/yellow/green/blue/indigo/violet), or wants a fleet-wide summary.
  Requires the gbork daemon to be running (`gbork start`) in the main/ worktree.
---

# gbork

You are coordinating multiple Claude Code agents running in parallel gbiv worktrees. The `gbork` daemon exposes their pane state and lets you send keystrokes.

## Activation

Use this skill when the user says things like:
- "what's the status of all my sessions"
- "is anyone waiting for input"
- "what is red doing"
- "send red the answer 'yes'"
- "approve the question in orange"

Do **not** activate for general gbiv worktree management ("create a new red worktree" — that's gbiv) or for editing code in the current worktree.

## Commands

All commands are subcommands of the `gbork` CLI. They talk to a local daemon over HTTP. Always use `--json` when parsing output.

| User intent | What you do |
|---|---|
| Fleet status / "what's everyone doing" | `gbork status --json`; summarize each color in 1-2 lines, highlight any `pane_status != ok` |
| "Is anyone waiting on input" | `gbork status --json`; scan each `output` for prompts (lines ending in `?`, `(y/n)`, `>`, etc.); call out matches |
| "What's <color> doing" | `gbork get <color> --json --lines 200`; summarize the last meaningful activity |
| "Show me <color>'s full output" | `gbork get <color> --lines 500`; print the raw output back to the user |
| "Tell <color> <message>" (literal text) | `gbork send <color> "<message>"` |
| "Approve <color>" / "say yes to <color>" | First `gbork get <color> --json --lines 50` to see what the prompt expects, THEN `gbork send <color> "<chosen response>"` |

## Read-before-send

When the intent is approval-shaped ("approve", "say yes", "answer the question") — not literal-text-shaped — always read first:

1. `gbork get <color> --json --lines 50`
2. Inspect the tail for an active prompt
3. Pick the appropriate response (`yes`, `y`, `1`, `approve`, etc. depending on what the prompt accepts)
4. `gbork send <color> "<chosen response>"`
5. Optionally: `gbork get <color>` after a brief pause to confirm

Never blindly send "yes" — the session might not be waiting for confirmation.

## Exit codes

- `0` — success
- `2` — daemon not running (tell user to run `gbork start` in the main/ worktree; do not auto-start)
- `3` — color invalid or has no tmux window
- `4` — window exists but has no claude pane (or has multiple)
- `5` — text was sent but Enter failed
- `1` — other error

## Constraints

- `gbork` only knows the seven ROYGBIV colors (red, orange, yellow, green, blue, indigo, violet). Reject other colors politely.
- Do not start the daemon for the user. The daemon is foreground-only and the user needs to choose where to run it.
- For `multiple_claude_panes`, surface the ambiguity — do not pick a pane.
- gbork cannot kill sessions, create worktrees, or do anything besides read panes and send keystrokes. For everything else, point at gbiv or tmux.
