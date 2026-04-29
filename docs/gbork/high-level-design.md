# gbork — High-Level Design

**Created**: 2026-04-28
**Status**: Approved (v1)

## What gbork Is

gbork is a small local HTTP daemon that, on demand, captures the terminal output of every Claude Code pane in a gbiv workspace and lets a caller send keystrokes to any of them. It ships with a Claude Code skill that teaches a Claude Code session how to drive the API.

The expected setup: the user runs `gbork start` (foreground) in their `main/` worktree. Any Claude Code session — typically also in `main/` — invokes the skill to ask "what's everyone doing?" or "send red this message," and the skill calls gbork over HTTP.

## Problem Statement and Goals

Running Claude Code agents across multiple gbiv color worktrees gives you no fleet view. Sessions stall on `AskUserQuestion`, finish unnoticed, or duplicate work. There's no way for one Claude Code instance to ask about the others.

**Goals:**
- On-demand visibility into every active Claude Code pane in the workspace
- A way for any Claude Code instance to send keystrokes to a worker pane
- Pure pull model — never interrupt the user
- Zero per-worker setup; works with sessions already running

**Initial scope:** Claude Code panes inside tmux windows that follow gbiv color naming. Other harnesses are a future extension.

## Target Users

Developers using gbiv who want to point one Claude Code session at the rest of their fleet on demand.

## System Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      gbork daemon                         │
│                  (foreground, Ctrl+C to stop)             │
│                                                           │
│  ┌─────────────────────────────────────────────────────┐ │
│  │                  HTTP Server                         │ │
│  │             127.0.0.1:<ephemeral port>               │ │
│  │                                                      │ │
│  │  GET  /sessions[?lines=N]                            │ │
│  │  GET  /session/:color[?lines=N]                      │ │
│  │  POST /session/:color/send  body {text}              │ │
│  └─────────┬────────────────────────────┬───────────────┘ │
│            │ on every request            │                │
│   ┌────────▼────────┐         ┌─────────▼──────────┐     │
│   │  Pane Locator   │         │  tmux Driver        │     │
│   │                 │         │                     │     │
│   │ list windows in │         │  capture-pane       │     │
│   │ gbiv tmux sess; │         │  send-keys          │     │
│   │ pick pane(s)    │         │                     │     │
│   │ running claude  │         │                     │     │
│   └─────────────────┘         └─────────────────────┘     │
└────────────────────────┬─────────────────────────────────┘
                         │ tmux                    ▲ HTTP
                         ▼                         │
       ┌──────────────────────────────┐    ┌──────┴──────────────┐
       │ gbiv worktree tmux windows   │    │ Claude Code instance│
       │ [red] [orange] [yellow] ...  │    │ (in main/, with     │
       │ each window has 1+ panes;    │    │  gbork skill loaded)│
       │ claude runs in one of them   │    │                     │
       └──────────────────────────────┘    └─────────────────────┘
```

### Components

| Component | Purpose |
|---|---|
| **HTTP Server** | Localhost-only HTTP server. Stateless. Handles three endpoints. Returns JSON. |
| **Pane Locator** | For a given color, lists the panes in the matching tmux window and identifies the pane running Claude Code by walking the process tree from each pane's PID (`#{pane_pid}`) and matching the claude binary's executable path. Self-reported process names are unreliable (Claude Code sets its `process.title` to its version string, e.g., `2.1.122`), so `#{pane_current_command}` alone cannot be used. Multiple panes per window are expected; non-claude panes are ignored. |
| **tmux Driver** | Thin wrapper around `tmux capture-pane` and `tmux send-keys`. |
| **gbork CLI** | Same binary, multiple subcommands: `gbork start` runs the server (foreground only); `gbork status` / `gbork get <color>` / `gbork send <color> <text>` are HTTP clients used by the skill or directly. |
| **gbork skill** | Markdown skill (`~/.claude/skills/gbork/SKILL.md`) shipped with the project. Teaches a Claude Code session what gbork is and how to translate user intents into `gbork` CLI calls. |

## Key Design Decisions

### Stateless, on-demand capture
No ring buffer. No background poller. Each HTTP request triggers a fresh `tmux capture-pane`. Simpler to reason about and to implement. If output volume becomes a problem later, a Haiku/Sonnet summarizer is the natural next step before adding a buffer.

### Pull, not push; HTTP, not socket
The commander pulls when the user asks. HTTP on `127.0.0.1` for compatibility with locked-down environments where Unix sockets may be restricted. No auth in v1 (localhost-only, single user).

### Pane discovery handles multi-pane windows and unreliable command names
A gbiv color window often has multiple panes (editor, claude, watcher, etc.). The Pane Locator finds the claude pane by walking the process tree under each pane's PID and matching against the claude binary, not by reading tmux's `#{pane_current_command}` — Claude Code renames its own process title to the version string (`2.1.122`), making the tmux-reported name useless for identification. If zero or multiple claude panes are found in a window, the response surfaces that explicitly rather than guessing.

### Foreground-only daemon (no `--detach`)
`gbork start` runs in the foreground. The user stops it with Ctrl+C. No backgrounding, no PID files for process management, no signal handling beyond default. Keeps v1 small.

### Port discovery via in-workspace file
The daemon writes its bound port to `<main-worktree>/.gbork/port`. Users add `.gbork/` to `.gitignore`. CLI subcommands and the skill read this file to find the daemon. The `main/` worktree is the canonical home — that's where users are expected to run `gbork start`.

### Skill-driven UX
The user never learns the API. They invoke the skill, which translates intents into `gbork` subcommand calls. Examples:
- User: "What's the status of all my sessions?" → `gbork status` → Claude summarizes
- User: "Give red approval" → Claude reads `gbork get red`, decides what input is needed, runs `gbork send red "yes"`

### tmux pane capture for observation
Poll-on-request via `tmux capture-pane -t <session>:<pane>`. Captures everything the session prints — `AskUserQuestion` UI, build output, agent status — at the cost of noise. No worker-side configuration required.

### tmux send-keys for commanding
`POST /session/:color/send` runs `tmux send-keys -t <session>:<pane> "<text>" Enter`. Fire-and-forget; effects visible in the next pane capture.

### Standalone binary in gbiv workspace
New crate in the Cargo workspace. Shared utilities (root discovery, color constants) move to a `gbiv-core` library crate. The `gbiv` binary and `gbork` binary both depend on `gbiv-core`. This enables separate release and avoids blurring gbiv's "no daemon" invariant.

## HTTP API

```
GET  /sessions?lines=N           (default N=50)
     → [
         {
           color: "red",
           tmux_window: "red",
           claude_pane: "%12" | null,
           pane_status: "ok" | "no_claude_pane" | "multiple_claude_panes",
           output: "<last N lines>" | null
         },
         ...
       ]

GET  /session/:color?lines=N     (default N=200)
     → {color, claude_pane, pane_status, captured_at, output}
       or 404 if color window doesn't exist

POST /session/:color/send
     body: {text: "..."}
     → {ok: true, sent_to_pane: "%12"}
       or {ok: false, error: "..."} (e.g., no_claude_pane)
```

`/sessions` returning per-color output in a single call lets "what's everyone doing?" be one round trip.

## Daemon Lifecycle

1. User: `cd main/repo && gbork start`
2. Daemon binds an ephemeral port, writes it to `main/repo/.gbork/port`, prints the URL, blocks on `accept()`.
3. CLI subcommands and the skill resolve the port by walking up to find the gbiv root, then reading `main/repo/.gbork/port`.
4. User stops with Ctrl+C; daemon removes the port file on clean shutdown (best-effort).

## Non-Goals

- **No buffering or history**: only "what the pane shows right now."
- **No event push, streaming, or notifications.**
- **No backgrounding or process management.**
- **No auth.**
- **No other harnesses in v1.**
- **No GBIV.md mutations.**
- **No support for non-standard Claude Code launchers**: identification assumes the claude binary is invoked directly. Wrapper scripts or aliases that obscure the binary path may not be detected; this can be revisited if it bites.

## Evolution Vectors

1. **Pre-summarization**: a Haiku/Sonnet pass that condenses raw pane output before returning it.
2. **Other harnesses**: detect Codex / shell panes; surface harness type in `/sessions`.
3. **Streaming**: SSE endpoint if pull becomes insufficient.
4. **Cross-workspace**: support multiple gbiv roots from one daemon.

## References

- `docs/gbiv/high-level-design.md` — gbiv HLD; gbork depends on gbiv's tmux window naming convention and root discovery
- `docs/gbork/llds/` — component-level designs (forthcoming)
- `docs/gbork/specs/` — EARS requirements (forthcoming)
- `docs/gbork/planning/` — implementation plans (forthcoming)
