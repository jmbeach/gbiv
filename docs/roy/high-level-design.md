# roy — High-Level Design

**Created**: 2026-04-28
**Status**: Approved (v1)

## What roy Is

roy is a small local HTTP daemon that, on demand, captures the terminal output of every Claude Code pane in a gbiv workspace and lets a caller send keystrokes to any of them. It ships with a Claude Code skill that teaches a Claude Code session how to drive the API.

The expected setup: the user runs `roy start` (foreground) in their `main/` worktree. Any Claude Code session — typically also in `main/` — invokes the skill to ask "what's everyone doing?" or "send red this message," and the skill calls roy over HTTP.

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

The **direct caller** of every roy interface (HTTP API, CLI, skill) is an LLM — specifically a Claude Code session loaded with the roy skill. roy is designed LLM-first: JSON-only output, structured error bodies with verbose `explanation` fields, distinct exit codes, no pretty-printing. Human use is supported (a developer can `roy status | jq`) but if a future change forces a tradeoff between LLM-friendliness and human-friendliness, LLM wins.

## System Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      roy daemon                         │
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
       │ each window has 1+ panes;    │    │  roy skill loaded)│
       │ claude runs in one of them   │    │                     │
       └──────────────────────────────┘    └─────────────────────┘
```

### Components

| Component | Purpose |
|---|---|
| **HTTP Server** | Localhost-only HTTP server. Stateless. Handles three endpoints. Returns JSON. |
| **Pane Locator** | For a given color, lists the panes in the matching tmux window and identifies the pane running Claude Code by walking the process tree from each pane's PID (`#{pane_pid}`) and matching the claude binary's executable path. Self-reported process names are unreliable (Claude Code sets its `process.title` to its version string, e.g., `2.1.122`), so `#{pane_current_command}` alone cannot be used. Multiple panes per window are expected; non-claude panes are ignored. When more than one claude pane is found in a window, the oldest by process start time wins (typical case: a long-running worktree session vs. a transient nested claude); the also-rans are surfaced in the response. |
| **tmux Driver** | Thin wrapper around `tmux capture-pane` and `tmux send-keys`. |
| **roy CLI** | Same binary, multiple subcommands: `roy start` runs the server (foreground only); `roy status` / `roy get <color>` / `roy send <color> <text>` are HTTP clients used by the skill or directly; `roy install-skill` writes the bundled skill into `~/.claude/skills/roy/` (or `<gbiv-root>/.claude/skills/roy/` with `--scope project`). |
| **roy skill** | Markdown skill (`~/.claude/skills/roy/SKILL.md`) shipped with the project. Teaches a Claude Code session what roy is and how to translate user intents into `roy` CLI calls. |

## Key Design Decisions

### Stateless, on-demand capture
No ring buffer. No background poller. Each HTTP request triggers a fresh `tmux capture-pane`. Simpler to reason about and to implement. If output volume becomes a problem later, a Haiku/Sonnet summarizer is the natural next step before adding a buffer.

### Pull, not push; HTTP, not socket
The commander pulls when the user asks. HTTP on `127.0.0.1` for compatibility with locked-down environments where Unix sockets may be restricted. No auth in v1 (localhost-only, single user).

### Pane discovery handles multi-pane windows and unreliable command names
A gbiv color window often has multiple panes (editor, claude, watcher, etc.). The Pane Locator finds the claude pane by walking the process tree under each pane's PID and matching against the claude binary, not by reading tmux's `#{pane_current_command}` — Claude Code renames its own process title to the version string (`2.1.122`), making the tmux-reported name useless for identification. Zero claude panes is surfaced explicitly (`no_claude_pane`). When *multiple* claude panes are found, the locator picks the oldest by process start time and exposes the others alongside (`other_claude_panes`) so the commander knows the disambiguation happened.

### Foreground-only daemon (no `--detach`)
`roy start` runs in the foreground. The user stops it with Ctrl+C. No backgrounding, no PID files for process management, no signal handling beyond default. Keeps v1 small.

### Port discovery via in-workspace file
The daemon writes its bound port to `<main-worktree>/.roy/port`. `roy start` adds `.roy/` to the repo's local git excludes automatically — the user never has to edit `.gitignore`. CLI subcommands and the skill read the port file to find the daemon. The `main/` worktree is the canonical home — that's where users are expected to run `roy start`.

### Skill-driven UX
The user never learns the API. They invoke the skill, which translates intents into `roy` subcommand calls. Examples:
- User: "What's the status of all my sessions?" → `roy status` → Claude summarizes
- User: "Give red approval" → Claude reads `roy get red`, decides what input is needed, runs `roy send red "yes"`

### tmux pane capture for observation
Poll-on-request via `tmux capture-pane -t <session>:<pane>`. Captures everything the session prints — `AskUserQuestion` UI, build output, agent status — at the cost of noise. No worker-side configuration required.

### tmux send-keys for commanding
`POST /session/:color/send` runs `tmux send-keys -t <session>:<pane> "<text>" Enter`. Fire-and-forget; effects visible in the next pane capture.

### Standalone binary in gbiv workspace
New crate in the Cargo workspace. The `gbiv` and `roy` binaries both depend on a shared `gbiv-core` library crate; the specific surface each side reuses is documented in the relevant component LLDs. This enables separate release and avoids blurring gbiv's "no daemon" invariant.

### Reject prompt-shaped input on send
Roy refuses to forward keystrokes that look like answers to a Claude Code permission prompt or `AskUserQuestion` choice. After trimming whitespace, the server rejects `text` matching any of:
- empty string
- `^[yn]$` or `^(yes|no)$` (case-insensitive)
- `^\d{1,3}$` (numeric choice up to 3 digits)
- a single non-alphanumeric character

Rationale: a permission prompt or tool-use confirmation requires a deliberate human decision. If roy could answer one on the user's behalf — at the prompting of any commander Claude Code session — a misread of pane state or an over-eager skill could approve actions the user has not seen. The conservative default is to refuse and direct the user to answer in the worker's window themselves. Multi-word and natural-language messages are unaffected. The rule may be loosened later behind an explicit opt-in.

## HTTP API

```
GET  /sessions?lines=N           (default N=50)
     → [
         {
           color: "red",
           tmux_window: "red",
           claude_pane: "%12" | null,
           other_claude_panes: ["%17"]?,   // present only when locator auto-picked from multiple
           pane_status: "ok" | "no_window" | "no_claude_pane",
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
       or {ok: false, error: "..."} (e.g., no_claude_pane, looks_like_prompt_response)
```

`/sessions` returning per-color output in a single call lets "what's everyone doing?" be one round trip.

## Daemon Lifecycle

1. User: `cd main/repo && roy start`
2. Daemon binds an ephemeral port, writes it to `main/repo/.roy/port`, prints the URL, blocks on `accept()`.
3. CLI subcommands and the skill resolve the port by walking up to find the gbiv root, then reading `main/repo/.roy/port`.
4. User stops with Ctrl+C; daemon removes the port file on clean shutdown (best-effort).

## Cross-Cutting Patterns

### Error Handling

roy follows the standard Rust library/binary split:

- **Library modules** — `tmux_driver`, `pane_locator`, anything in `gbiv-core` that roy reuses — return typed errors via `thiserror`-derived enums (e.g., `TmuxError`). The HTTP layer needs to map specific variants to status codes (`SessionNotFound` → 503, `PaneNotFound` → 404, `SendKeysIncomplete` → 502), so the typed surface is load-bearing.
- **Binary entry points** — `roy start`'s startup path, each CLI subcommand's `main`, HTTP request handlers — use `anyhow::Result<()>` with `.context("…")` breadcrumbs. These layers either succeed, return a typed library error that maps to a status code, or print one user-facing message and exit/respond.
- **Boundary**: typed errors auto-convert into `anyhow::Error` via `?`. Library modules must never `anyhow!` directly — doing so erases the variant a higher layer needs.

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

- `docs/gbiv/high-level-design.md` — gbiv HLD; roy depends on gbiv's tmux window naming convention and root discovery
- `docs/roy/llds/` — component-level designs (forthcoming)
- `docs/roy/specs/` — EARS requirements (forthcoming)
- `docs/roy/planning/` — implementation plans (forthcoming)
