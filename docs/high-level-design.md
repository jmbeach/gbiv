# gbiv — High-Level Design

**Created**: 2026-06-03
**Status**: Draft

## What gbiv Is

gbiv is a binary CLI that turns one git repository into seven parallel
workspaces — one per ROYGBIV color — using git worktrees, and lets one Claude
Code session observe and drive the others across those worktrees. The seven
colors are the default palette; an advanced, opt-in per-project config can append
extra named worktrees beyond ROYGBIV for the rare case that seven aren't enough.

The name comes from the colors: gbiv manages ROYGBIV color-named worktrees and
is named for them.

gbiv has two functional domains:

1. **Worktree management**: the day-to-day commands a developer runs to create,
   observe, and maintain the seven color worktrees. Synchronous, daemon-free, one
   process per invocation.
2. **Fleet orchestration**: an on-demand foreground daemon (`gbiv start`) that
   captures the terminal output of every Claude Code pane in the workspace and
   forwards keystrokes to any of them, plus a Claude Code skill that teaches a
   session how to drive it.

Both domains ship in one binary, one crate.

## Problem

A developer juggling several features at once pays a constant tax: stashing,
branch-switching, and losing track of which change is where. Worse, once those
features are each driven by a Claude Code agent in its own worktree, there is no
fleet view — sessions stall on prompts, finish unnoticed, or duplicate work, and
no one session can ask about the others.

gbiv addresses both halves with one tool: a color-worktree layout that makes
"seven features in flight" cheap, and an orchestration mode that makes the fleet
of agents observable on demand.

## Approach

- **One repository, seven color worktrees.** `gbiv init` turns `repo/` into
  `repo/{main,red,orange,yellow,green,blue,indigo,violet}/repo/`, each a full
  git worktree sharing one object store. The color name does triple duty:
  directory, branch, and tmux window.
- **A plain-text ledger.** `GBIV.md` records which feature is assigned to which
  color and its lifecycle status. Human-editable, committed to git. No database,
  no JSON state file.
- **Delegate to the tools that already exist.** All git work shells out to `git`;
  all terminal work shells out to `tmux`. gbiv owns the layout and the
  conventions, not reimplementations.
- **Orchestration is an explicit, opt-in mode.** Worktree commands never spawn a
  background process. The only long-running mode is `gbiv start`, run in the
  foreground and stopped with Ctrl+C.
- **An extensible palette, ROYGBIV by default.** The seven color names are the
  built-in default. A power user who needs more than seven concurrent worktrees
  can declare extra named slots in an optional `.gbiv/config.toml`
  at the project root, then run `gbiv repair` to materialize them. Absent that file,
  behavior is exactly the seven-color layout. The base ROYGBIV names are fixed —
  the config only *appends* extra names; it never renames or removes a color.
- **`gbiv repair` makes the layout match the palette.** A single idempotent,
  append-only command that creates any worktree in the active palette that is
  missing on disk — whether a configured extra not yet materialized, or a base
  ROYGBIV worktree that was deleted. It never removes or renames worktrees;
  destructive cleanup stays with `reset`/`tidy`.

## Target Users

Developers running several features — increasingly, several Claude Code agents —
in parallel and wanting a single tool to lay out the worktrees and, on demand,
watch the fleet.

The two domains serve different callers. Worktree commands are **human-first**:
readable status output, ANSI color. Orchestration is **LLM-first** — its direct
caller is a Claude Code session loaded with the gbiv-orchestrate skill, so that surface is
JSON-only with structured error bodies and verbose explanations. Where the two
conflict, each domain keeps its own bias rather than forcing one house style.

## Goals

- A single command (`gbiv`) for the whole color-worktree lifecycle: init, status,
  exec, rebase-all, reset, mark, tidy, tmux mirroring.
- A plain-text, git-committed source of truth (`GBIV.md`) — no hidden state.
- On-demand visibility into every active Claude Code pane in the workspace, and a
  way for one session to send keystrokes to a worker.
- Pure pull model for orchestration — never interrupt the user.
- One binary, one crate, easy to `cargo install`.

## Non-Goals

- **No persistent daemon for worktree work.** Worktree commands are synchronous
  and exit. The only daemon is the explicit `gbiv start` foreground mode.
- **No remote git operations beyond fetch/pull** (no push, no PR creation).
- **No CI/CD integration, no multi-repo orchestration** (one gbiv project = one
  git repo).
- **For orchestration:** no buffering or history, no event push or streaming, no
  backgrounding or process management, no auth, no `GBIV.md` mutations, and no
  harnesses other than Claude Code in tmux.

## System Design

```
                          gbiv  (one binary)
   ┌─────────────────────────────────────────────────────────────────┐
   │                                                                 │
   │      Worktree management                Fleet orchestration     │
   │                                                                 │
   │   ┌─────────────────────────────┐        ┌──────────────────┐   │
   │   │        CLI & Palette        │        │   gbiv start →   │   │
   │   │   dispatch · ROYGBIV consts │        │   HTTP daemon    │   │
   │   └──┬────────┬────────┬────────┘        │  (foreground)    │   │
   │      │        │        │                 │                  │   │
   │  ┌───▼──┐ ┌───▼───┐ ┌──▼────┐ ┌───────┐  │  Pane Locator    │   │
   │  │Work- │ │Feature│ │Obser- │ │ Tmux  │  │  tmux Driver     │   │
   │  │tree  │ │Ledger │ │vation │ │Mirror │  │  orchestrate cmds│   │
   │  │Life- │ │(GBIV. │ │status │ │new/   │  │  gbiv-orchestrate│   │
   │  │cycle │ │md mark│ │ exec  │ │sync/  │  └──────────────────┘   │
   │  └───┬──┘ └───┬───┘ └──┬────┘ │clean) │                         │
   │      │        │        │      └───┬───┘                         │
   │  ┌───▼────────▼────────▼──────────▼───┐                         │
   │  │   core primitives (root discovery, │                         │
   │  │   git_utils, colors, gitignore)    │                         │
   │  └────────────────────────────────────┘                         │
   └─────────────────────────────────────────────────────────────────┘
            │ git / tmux CLIs                  ▲ HTTP (orchestration)
            ▼                                  │
   color worktree tmux windows     Claude Code session (gbiv-orchestrate)
```

### Components

| Component | Domain | Purpose | LLD |
|---|---|---|---|
| **CLI & Palette** | worktree | Parse commands, route to handlers, load the active palette (ROYGBIV + optional extras) and provide ANSI formatting | `docs/llds/cli-and-palette.md` |
| **Worktree Lifecycle** | worktree | Create, repair, reset, and maintain the color worktree structure, including `gbiv repair` palette reconciliation (restore missing worktrees + materialize configured extras) | `docs/llds/worktree-lifecycle.md` |
| **Feature Ledger** | worktree | Parse and mutate `GBIV.md` as the source of truth for feature assignments and status | `docs/llds/feature-ledger.md` |
| **Observation** | worktree | Surface worktree health and run arbitrary commands across worktrees | `docs/llds/observation.md` |
| **Tmux Mirror** | worktree | Keep tmux windows synchronized with the worktree layout | `docs/llds/tmux-mirror.md` |
| **HTTP Server** | orchestration | Localhost-only, stateless server exposing per-session endpoints as JSON | `docs/llds/http-server.md` |
| **Pane Locator** | orchestration | Identify the Claude Code pane in each color window | `docs/llds/pane-locator.md` |
| **tmux Driver** | orchestration | Thin wrapper around `tmux capture-pane` and `send-keys` | `docs/llds/tmux-driver.md` |
| **Orchestrate Commands** | orchestration | The `gbiv start` server plus the client subcommands the skill calls | `docs/llds/orchestrate-cli.md` |
| **gbiv-orchestrate skill** | orchestration | Markdown skill teaching a Claude Code session how to drive `gbiv start` | `docs/llds/orchestrate-skill.md` |

### Cross-Cutting Patterns

**Root discovery.** Every command starts by finding the gbiv root — walking up
from CWD until a directory with `main/` plus at least one color subdirectory and a
git repo is found. The universal entry point for both domains.

**Active palette.** The list of worktree names is ROYGBIV by default, loaded at
runtime from the gbiv root. When `.gbiv/config.toml` declares extra names, the
active palette is ROYGBIV followed by those names, in order. Validation, iteration
order, and color inference all operate over the active palette. The palette is a
*configuration input*, not authoritative state, and is exactly ROYGBIV when no
config file is present.

**Color inference.** When a `<color>` argument is optional, commands infer it from
CWD by matching the first path component after the root against the active palette,
so `cd red/repo && gbiv mark --done` works without naming `red`.

**Canonical ordering.** Output and iteration always follow the active palette's
order — ROYGBIV first, then any configured extras. Status, exec, tmux window
order, and rebase/reset processing are all consistent.

**Parallel-by-worktree.** `status`, `exec all`, and `rebase-all` process every
worktree in the active palette in parallel, joining in palette order for
deterministic output. Safe because worktrees are independent.

**Error handling.** gbiv splits typed errors from contextual ones by *module
role*, not by crate — it is one crate, so the boundary is internal. Leaf modules
that hold core logic (`git_utils`, ledger parsing, `tmux_driver`, `pane_locator`,
the shared primitives) return typed `thiserror`-derived errors so callers can
match on variants. The outer layer (`main()`, command handlers, HTTP request
handlers) uses `anyhow::Result<()>` with `.context(…)` breadcrumbs and either
succeeds or prints one user-facing line. Leaf modules never `anyhow!` directly —
that erases the variant a higher layer needs. Multi-color operations collect
per-color `Result`s rather than failing fast, so one failing color doesn't abort
the others.

### Worktree domain — data flow

```
        GBIV.md (text file)             git worktree state
       ╱      │       ╲                 (branches, dirty, merged)
   mark   reset      status ◀───────────────────┘
  (writes)(removes)  (reads both)
                │
        rebase-all / reset / tmux sync / tmux clean
```

The two authoritative stores are **`GBIV.md`** (feature assignments and lifecycle)
and **git state** (branch positions, cleanliness, merge status). There is no third
store for *feature or worktree state* — no database, no JSON state file. The
optional `.gbiv/config.toml` is a *configuration* input, not a state store: it
names extra worktree slots but holds no feature, lifecycle, or branch state, and
is absent in the default seven-color setup.

### Orchestration domain — how it works

`gbiv start` binds an ephemeral port on `127.0.0.1`, writes it to
`<main>/.gbiv/port` (adding `.gbiv/` to the repo's local git excludes), prints the
URL, and blocks in the foreground until Ctrl+C. The client subcommands live under a
`gbiv fleet` group — `gbiv fleet status` (every color at a glance),
`gbiv fleet get <color>` (one session in detail), `gbiv fleet send <color> <text>`
— grouped so they don't collide with the worktree `gbiv status`. The clients and
the skill resolve the port by walking to the gbiv root and reading that file.

The server is **stateless and on-demand**: each request triggers a fresh
`tmux capture-pane`; there is no ring buffer or background poller. The **Pane
Locator** finds each color window's Claude Code pane by walking the process tree
from each pane's PID and matching the claude binary — Claude Code renames its own
process title to its version string, so `pane_current_command` is unreliable. When
a window has multiple claude panes, the oldest by start time wins and the others
are surfaced alongside. The **tmux Driver** performs the actual capture and
`send-keys`.

On send, gbiv **refuses prompt-shaped input** — empty strings, bare `y`/`n`/`yes`/
`no`, a numeric-only choice, or a single non-alphanumeric character — because a
permission prompt or `AskUserQuestion` choice requires a deliberate human decision
the user must make in the worker's own window. Multi-word, natural-language
messages pass through.

## Architectural Boundaries

### What gbiv owns
- The `project/{main,red,…,violet,…}/repo/` directory layout
- `GBIV.md` format and mutations
- Color branch naming, and tmux session/window naming
- The optional `.gbiv/config.toml` palette config and the `gbiv repair`
  reconciliation it drives (append-only worktree creation; warn-on-drift)
- The fleet HTTP API and its port-file discovery convention

### What gbiv delegates
- All git operations → `git` CLI
- All tmux operations → `tmux` CLI
- Shell command execution → `sh -c` (the exec command)
- Merge conflict resolution → the developer
- Answering permission prompts → the developer, in the worker's own window

## Open Questions & Future Decisions

None open. Skill packaging (`gbiv install-skill`'s `--scope user|project` split,
default user) is resolved — see `docs/llds/orchestrate-cli.md` § `gbiv install-skill`.

## Evolution Vectors

1. **Pre-summarization** of pane output via a Haiku/Sonnet pass before returning it.
2. **Other harnesses**: detect Codex / shell panes; surface harness type.
3. **Streaming**: an SSE endpoint if pull becomes insufficient.
4. **Cross-workspace**: one daemon serving multiple gbiv roots.
5. **Cargo publication**: packaging for `cargo install gbiv`.

## References

- `docs/llds/` — the ten component designs (worktree + orchestration)
- `docs/specs/` — EARS requirements
- `docs/arrows/index.yaml` — arrow-of-intent tracking and implementation status
