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

gbiv has three functional domains:

1. **Worktree management**: the day-to-day commands a developer runs to create,
   observe, and maintain the seven color worktrees, including `gbiv start` and
   `gbiv done`, which put a task on a color and take it off again. Synchronous,
   daemon-free, one process per invocation.
2. **Fleet orchestration**: an on-demand foreground daemon (`gbiv fleet start`)
   that captures the terminal output of every Claude Code pane in the workspace
   and forwards keystrokes to any of them, plus a Claude Code skill that teaches
   a session how to drive it.
3. **Extensions**: sandboxed Lua scripts that hook into gbiv commands. This is
   how gbiv works with task systems: vat ships as the reference extension, and
   anyone can write one for Jira or another system.

All three domains ship in one binary.

## Problem

A developer juggling several features at once pays a constant tax: stashing,
branch-switching, and losing track of which change is where. Worse, once those
features are each driven by a Claude Code agent in its own worktree, there is no
fleet view — sessions stall on prompts, finish unnoticed, or duplicate work, and
no one session can ask about the others. And the tasks themselves live in a
separate tracker, so claiming a task and giving it a worktree are two chores that
drift apart.

gbiv addresses all three with one tool: a color-worktree layout that makes
"seven features in flight" cheap, an orchestration mode that makes the fleet of
agents observable on demand, and `start`/`done` commands that extensions tie to
whatever task system the team uses.

## Approach

- **One repository, seven color worktrees.** `gbiv init` turns `repo/` into
  `repo/{main,red,orange,yellow,green,blue,indigo,violet}/repo/`, each a full
  git worktree sharing one object store. The color name does triple duty:
  directory, branch, and tmux window.
- **A plain-text ledger.** `GBIV.md` records which feature is assigned to which
  color. An entry on a color is in progress; `gbiv done` removes it. Human-editable,
  committed to git. No database, no JSON state file.
- **Two verbs for a task's life on a color.** `gbiv start <task>` puts a task on
  the next free color in palette order (ROYGBIV first); `gbiv done <task|color>`
  takes it off. Neither touches the worktree itself. With no extensions enabled,
  the argument to `start` is simply the entry's title.
- **Task systems plug in as Lua extensions.** gbiv embeds Lua 5.4 (via `mlua`) and
  runs each enabled extension in a sandbox. Extensions hook `start` and `done`;
  they can veto the command, name the task, and update the task system. gbiv
  itself never talks to a task system or the network.
- **Delegate to the tools that already exist.** All git work shells out to `git`;
  all terminal work shells out to `tmux`. gbiv owns the layout and the
  conventions, not reimplementations.
- **Orchestration is an explicit, opt-in mode.** Worktree commands never spawn a
  background process. The only long-running mode is `gbiv fleet start`, run in
  the foreground and stopped with Ctrl+C.
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

The domains serve different callers. Worktree commands are **human-first**:
readable status output, ANSI color. Orchestration is **LLM-first** — its direct
caller is a Claude Code session loaded with the gbiv-orchestrate skill, so that surface is
JSON-only with structured error bodies and verbose explanations. Where the two
conflict, each domain keeps its own bias rather than forcing one house style.
Extensions are **author-first**: their caller is a Lua script, so the contract
is a small, versioned host API and event set.

## Goals

- A single command (`gbiv`) for the whole color-worktree lifecycle: init, status,
  start, done, exec, rebase-all, reset, tidy, tmux mirroring.
- A plain-text, git-committed source of truth (`GBIV.md`) — no hidden state.
- Pluggable task systems: vat works once its extension is installed, and anyone
  can add another system as a Lua extension without changing gbiv.
- On-demand visibility into every active Claude Code pane in the workspace, and a
  way for one session to send keystrokes to a worker.
- Pure pull model for orchestration — never interrupt the user.
- One binary, easy to `cargo install`.

## Non-Goals

- **No persistent daemon for worktree work.** Worktree commands are synchronous
  and exit. The only daemon is the explicit `gbiv fleet start` foreground mode.
- **gbiv itself makes no network calls and no remote git operations beyond
  fetch/pull** (no push, no PR creation). An enabled extension may push to a
  task system's repository or call a network API, but only through host
  functions that the config allows for that extension.
- **No extension registry or marketplace.** Extensions are files in a folder;
  gbiv bundles vat and installs it on request.
- **Extensions hook commands; they don't add them.** No extension-defined
  subcommands in the first version.
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
   │   │        CLI & Palette        │        │ gbiv fleet start │   │
   │   │ dispatch · palette · config │        │ → HTTP daemon    │   │
   │   └──┬────────┬────────┬────────┘        │  (foreground)    │   │
   │      │        │        │                 │                  │   │
   │  ┌───▼──┐ ┌───▼───┐ ┌──▼────┐ ┌───────┐  │  Pane Locator    │   │
   │  │Work- │ │Feature│ │Obser- │ │ Tmux  │  │  tmux Driver     │   │
   │  │tree  │ │Ledger │ │vation │ │Mirror │  │  orchestrate cmds│   │
   │  │Life- │ │(GBIV. │ │status │ │new/   │  │  gbiv-orchestrate│   │
   │  │cycle │ │md     │ │ exec  │ │sync/  │  └──────────────────┘   │
   │  │      │ │start/ │ │       │ │clean) │                         │
   │  │      │ │done)  │ │       │ │       │       Extensions        │
   │  └───┬──┘ └───┬───┘ └──┬────┘ └───┬───┘  ┌──────────────────┐   │
   │      │        │        │          │      │ Extension Runtime│   │
   │      │        │        │          │      │ Lua 5.4 sandbox  │   │
   │      │        │        │          │      │ start/done hooks │   │
   │  ┌───▼────────▼────────▼──────────▼───┐  │ install-extension│   │
   │  │   core primitives (root discovery, │  ├──────────────────┤   │
   │  │   git_utils, colors, gitignore,    │  │ vat extension    │   │
   │  │   layered config)                  │  │ (bundled Lua)    │   │
   │  └────────────────────────────────────┘  └──────────────────┘   │
   └─────────────────────────────────────────────────────────────────┘
            │ git / tmux CLIs                  ▲ HTTP (orchestration)
            ▼                                  │
   color worktree tmux windows     Claude Code session (gbiv-orchestrate)
```

### Components

| Component | Domain | Purpose | LLD |
|---|---|---|---|
| **CLI & Palette** | worktree | Parse commands, route to handlers, load layered config (global, then project) and the active palette (ROYGBIV + optional extras), and provide ANSI formatting | `docs/llds/cli-and-palette.md` |
| **Worktree Lifecycle** | worktree | Create, repair, reset, and maintain the color worktree structure, including `gbiv repair` palette reconciliation (restore missing worktrees + materialize configured extras) | `docs/llds/worktree-lifecycle.md` |
| **Feature Ledger** | worktree | Parse and mutate `GBIV.md` as the source of truth for feature assignments; `gbiv start` puts a task on the next free color and `gbiv done` takes it off, firing extension events around each | `docs/llds/feature-ledger.md` |
| **Observation** | worktree | Surface worktree health and run arbitrary commands across worktrees | `docs/llds/observation.md` |
| **Tmux Mirror** | worktree | Keep tmux windows synchronized with the worktree layout | `docs/llds/tmux-mirror.md` |
| **HTTP Server** | orchestration | Localhost-only, stateless server exposing per-session endpoints as JSON | `docs/llds/http-server.md` |
| **Pane Locator** | orchestration | Identify the Claude Code pane in each color window | `docs/llds/pane-locator.md` |
| **tmux Driver** | orchestration | Thin wrapper around `tmux capture-pane` and `send-keys` | `docs/llds/tmux-driver.md` |
| **Orchestrate Commands** | orchestration | The `gbiv fleet start` server plus the client subcommands the skill calls | `docs/llds/orchestrate-cli.md` |
| **gbiv-orchestrate skill** | orchestration | Markdown skill teaching a Claude Code session how to drive `gbiv fleet start` | `docs/llds/orchestrate-skill.md` |
| **Extension Runtime** | extensions | Embed Lua 5.4 (`mlua`) in a sandbox, load enabled extensions from the user and project folders, fire `start`/`done` events, expose the host API (scoped files, scoped `git`, opt-in HTTP), and install bundled extensions | `docs/llds/extension-runtime.md` |
| **vat Extension** | extensions | The reference extension, written in Lua and bundled in gbiv: claims a vat task on `start`, completes it on `done`, including the git claim loop for a backlog in its own repo | `docs/llds/vat-extension.md` |

### Cross-Cutting Patterns

**Root discovery.** Every command starts by finding the gbiv root — walking up
from CWD until a directory with `main/` plus at least one color subdirectory and a
git repo is found. The universal entry point for every domain.

**Active palette.** The list of worktree names is ROYGBIV by default, loaded at
runtime from the gbiv root. When `.gbiv/config.toml` declares extra names, the
active palette is ROYGBIV followed by those names, in order. Validation, iteration
order, and color inference all operate over the active palette. The palette is a
*configuration input*, not authoritative state, and is exactly ROYGBIV when no
config file is present.

**Color inference.** When a `<color>` argument is optional, commands infer it from
CWD by matching the first path component after the root against the active palette,
so `cd red/repo && gbiv done` works without naming `red`.

**Layered configuration.** gbiv reads a global config
(`$XDG_CONFIG_HOME/gbiv/config.toml`, else `~/.config/gbiv/config.toml`) and then
the project's `<root>/.gbiv/config.toml`; a key set in the project file overrides
the same key in the global one. A list value, such as `[extensions] enabled`, is
replaced as a whole, not merged. Neither file is required.

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
 start/   reset     status ◀───────────────────┘
  done   (removes)  (reads both)
 (writes)
                │
        rebase-all / reset / tmux sync / tmux clean
```

The two authoritative stores gbiv keeps are **`GBIV.md`** (which task is on which
color) and **git state** (branch positions, cleanliness, merge status). There is
no third gbiv store for *feature or worktree state* — no database, no JSON state
file. A task's own state (claimed, done, assignee) lives in whatever task system
an enabled extension talks to; gbiv never reads or caches it. The optional global
and project config files are *configuration* inputs, not state stores: they name
extra worktree slots and enabled extensions but hold no feature, lifecycle, or
branch state, and both are absent in the default seven-color setup.

### Orchestration domain — how it works

`gbiv fleet start` binds an ephemeral port on `127.0.0.1`, writes it to
`<main>/.gbiv/port` (adding `.gbiv/` to the repo's local git excludes), prints the
URL, and blocks in the foreground until Ctrl+C. It and the client subcommands
live under a `gbiv fleet` group — `gbiv fleet status` (every color at a glance),
`gbiv fleet get <color>` (one session in detail), `gbiv fleet send <color> <text>`
— grouped so they don't collide with the worktree `gbiv status` and `gbiv start`. The clients and
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

### Extensions domain — how it works

An **extension** is a folder of Lua files plus a manifest naming it and the
`api_version` it targets. gbiv looks for `<name>/` under the project folder
`<root>/.gbiv/extensions/` first, then the user folder
`~/.config/gbiv/extensions/`. Only extensions listed in `[extensions] enabled`
load, and they run in that list's order. There is no special "task provider"
role: an extension that talks to a task system is just one that hooks `start`
and `done`.

```
 gbiv start <arg>                        gbiv done <task-id | color>
   pick next free color (else error)       find the GBIV.md entry (else error)
   before_start ─ any hook may veto;       before_done ─ any hook may veto
                  first title wins
   write  - [<color>] [<task-id>] <title>  remove the entry
   after_start ─ notify only               after_done ─ notify only
```

- **Veto.** A `before_*` hook can stop the command with a message; gbiv then
  writes nothing. There is no rollback event: if an earlier hook already acted
  (say, claimed the task) before a later one vetoed, that action stands.
- **Naming.** The first `before_start` hook that returns a task ID and title names
  the entry. If none does, the argument is the title and the entry has no task ID,
  so `gbiv start "Fix login bug"` works with no extensions enabled.
- **After hooks** report failures but never undo the GBIV.md change.
- **Neither command touches the worktree.** A color is free when GBIV.md has no
  entry for it, whatever its worktree holds.

**Sandbox.** Each extension gets Lua's standard library without `io`, `os`
process and file functions, `debug`, or native module loading. gbiv's host API
adds file read/write and a `git` runner limited to the paths the config grants
that extension, and HTTP only when the extension's manifest asks for it *and*
the config allows it. Secrets reach an extension only through environment
variables the config names. Out of the box nothing touches the network.

**Contract.** The host API and the four events are a versioned contract: an
extension declares the `api_version` it needs, and gbiv refuses to load one whose
major version it doesn't support.

**Installing.** gbiv embeds the Lua source of its bundled extensions (today, vat).
`gbiv install-extension <name> [--scope user|project]` copies one to that scope's
extension folder and adds it to that scope's `enabled` list, mirroring
`gbiv install-skill`.

**The vat extension** reads and writes vat's markdown backlog directly. It claims
the task on `before_start` and completes it on `before_done` (removing it from
`backlog.md`, as `vat done` does). When the backlog is its own git repository, it
runs vat's first-push-wins claim loop through the host `git` runner. It is a third implementation of vat's format, beside
vat-cli and the vat skill, so it is checked against vat's golden test files.

## Architectural Boundaries

### What gbiv owns
- The `project/{main,red,…,violet,…}/repo/` directory layout
- `GBIV.md` format and mutations
- Color branch naming, and tmux session/window naming
- The optional `.gbiv/config.toml` palette config and the `gbiv repair`
  reconciliation it drives (append-only worktree creation; warn-on-drift)
- The fleet HTTP API and its port-file discovery convention
- Layered global + project config
- The extension runtime: sandbox, host API, the `start`/`done` event contract and
  its `api_version`, and `gbiv install-extension`
- The bundled vat extension

### What gbiv delegates
- All git operations → `git` CLI
- All tmux operations → `tmux` CLI
- Shell command execution → `sh -c` (the exec command)
- Merge conflict resolution → the developer
- Answering permission prompts → the developer, in the worker's own window
- Task state (claims, completion, assignees) → the task system, through its extension
- Network calls and credentials → extensions, only where the config allows

## Open Questions & Future Decisions

Skill packaging (`gbiv install-skill`'s `--scope user|project` split, default user)
is resolved — see `docs/llds/orchestrate-cli.md` § `gbiv install-skill`.

None open. Two extension questions are resolved for now: a vetoed command fires
no rollback event, and `gbiv start` judges a color free by GBIV.md alone, not by
its worktree's state (see § Extensions domain).

## Evolution Vectors

1. **Pre-summarization** of pane output via a Haiku/Sonnet pass before returning it.
2. **Other harnesses**: detect Codex / shell panes; surface harness type.
3. **Streaming**: an SSE endpoint if pull becomes insufficient.
4. **Cross-workspace**: one daemon serving multiple gbiv roots.
5. **Cargo publication**: packaging for `cargo install gbiv`.
6. **More extension events**: hooks on `rebase-all`, `status`, `repair`.
7. **Extension-defined subcommands**, if hooks alone prove too narrow.
8. **More task systems**: a Jira extension, using the opt-in HTTP host API.

## References

- `docs/llds/` — the twelve component designs (worktree, orchestration, extensions)
- `docs/specs/` — EARS requirements
- `docs/arrows/index.yaml` — arrow-of-intent tracking and implementation status
