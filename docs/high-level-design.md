# gbiv — High-Level Design

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## What gbiv Is

gbiv is a CLI tool that turns a single git repository into seven parallel workspaces — one per ROYGBIV color — using git worktrees. A developer can have up to seven features in flight simultaneously, switching between them by changing directories (or tmux windows) rather than stashing and switching branches.

A plain text file (GBIV.md) tracks which feature is assigned to which color and its lifecycle status. A tmux integration mirrors the workspace layout into terminal windows.

## Core Concepts

### The Color Worktree

The central abstraction. After `gbiv init`, a repository becomes:

```
project/
├── main/repo/      ← canonical repo, on main branch
├── red/repo/       ← worktree on 'red' branch
├── orange/repo/    ← worktree on 'orange' branch
├── ...             ← yellow, green, blue, indigo
└── violet/repo/    ← worktree on 'violet' branch
```

Each color worktree is a full git worktree sharing the same object store. The color name serves triple duty: directory name, branch name, and tmux window name.

### The Feature Lifecycle

A feature moves through these states:

```
backlog → assigned to color → in-progress → done → reset (worktree reclaimed)
```

State is tracked in two places:
- **GBIV.md**: `- [red] [in-progress] Fix auth bug` — human-editable, committed to git
- **Git branch state**: which branch the worktree is on, whether it's merged upstream

### The Maintenance Loop

The recurring developer workflow:

```
gbiv rebase-all    →  pull main, rebase all colors onto it
gbiv mark --done   →  mark completed features
gbiv reset         →  reclaim worktrees with [done] features
gbiv tmux clean    →  remove stale tmux windows
```

Or in one command: `gbiv tidy`

## Component Architecture

```
┌─────────────────────────────────────────────────────┐
│                   CLI & Palette                      │
│            (main.rs, colors.rs)                      │
│         dispatch · ROYGBIV constants · ANSI          │
└──────────┬──────────┬──────────┬──────────┬─────────┘
           │          │          │          │
     ┌─────▼────┐ ┌───▼────┐ ┌──▼───┐ ┌───▼──────┐
     │ Worktree │ │Feature │ │Obser-│ │  Tmux    │
     │Lifecycle │ │ Ledger │ │vation│ │  Mirror  │
     │          │ │        │ │      │ │          │
     │ init     │ │gbiv_md │ │status│ │new-session│
     │ reset    │ │ mark   │ │ exec │ │  sync    │
     │rebase-all│ │        │ │      │ │  clean   │
     │  tidy    │ │        │ │      │ │          │
     └─────┬────┘ └───┬────┘ └──┬───┘ └───┬──────┘
           │          │         │          │
     ┌─────▼──────────▼─────────▼──────────▼─────────┐
     │              git_utils.rs                       │
     │    root discovery · git commands · color        │
     │    inference · worktree navigation              │
     └────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | One-sentence purpose | LLD |
|---|---|---|
| **CLI & Palette** | Parse commands, route to handlers, provide ROYGBIV constants and ANSI formatting | `docs/llds/cli-and-palette.md` |
| **Worktree Lifecycle** | Create, sync, reset, and maintain the 7-color worktree structure | `docs/llds/worktree-lifecycle.md` |
| **Feature Ledger** | Parse and mutate GBIV.md as the source of truth for feature assignments and status | `docs/llds/feature-ledger.md` |
| **Observation** | Surface worktree health and run arbitrary commands across worktrees | `docs/llds/observation.md` |
| **Tmux Mirror** | Keep tmux windows synchronized with the worktree layout | `docs/llds/tmux-mirror.md` |

### Data Flow

```
                    GBIV.md (text file)
                   ╱       │        ╲
          mark ──▶╱   reset ──▶      ╲
                 ╱    (removes)       ╲
                ╱         │            ╲
         status ◀─────────┤      tmux sync ◀──
         (reads)          │      tmux clean ◀──
                          │
              ┌───────────▼───────────┐
              │   git worktree state   │
              │  (branches, commits,   │
              │   dirty/clean, merged) │
              └───────────┬───────────┘
                          │
         ┌────────────────┼────────────────┐
         ▼                ▼                ▼
    rebase-all         reset           status
   (rebases onto    (checks out      (reads branch,
    remote main)    color branch,     dirty, merged,
                    resets to main)   ahead/behind)
```

The two authoritative data stores are:
1. **GBIV.md** — feature assignments and lifecycle status (written by humans + mark, read by status + reset + tmux)
2. **Git state** — branch positions, merge status, working tree cleanliness (written by git operations, read by status + reset preconditions)

There is no third store — no database, no JSON state file, no config beyond what's in these two.

## Cross-Cutting Patterns

### Root Discovery

Every command starts by finding the gbiv root — walking up from CWD until a directory with `main/` + at least one color subdirectory + a git repo is found. This is the universal entry point.

### Color Inference

When a `<color>` argument is optional, commands infer it from CWD by matching the first path component after the gbiv root against the ROYGBIV constant. This allows `cd red/repo && gbiv mark --done` without specifying `red`.

### Parallel-by-Color

Three commands process all 7 colors in parallel via `thread::spawn`: `status`, `exec all`, and `rebase-all`. Each spawns one thread per color, joins in ROYGBIV order for deterministic output. This is safe because worktrees are independent — parallel operations on different worktrees don't conflict.

### ROYGBIV Ordering

Output and iteration always follow the canonical ROYGBIV order defined by `COLORS: [&str; 7]`. This provides consistency across status output, exec output, tmux window order, and rebase/reset processing.

### Error Propagation

All command handlers return `Result<_, String>`. Errors are user-facing messages (not stack traces). `main()` prints errors to stderr and exits with code 1. Multi-color operations (reset-all, rebase-all, exec-all) collect per-color results rather than failing on the first error.

## Architectural Boundaries

### What gbiv owns
- The `project/{main,red,...,violet}/repo/` directory layout
- `GBIV.md` format and mutations
- Color branch naming convention
- Tmux session/window naming convention

### What gbiv delegates
- All git operations → `git` CLI (via `std::process::Command`)
- All tmux operations → `tmux` CLI (via `std::process::Command`)
- Shell command execution → `sh -c` (via exec command)
- Merge conflict resolution → developer (gbiv leaves conflicts in place)
- Feature branch creation → developer (gbiv creates color branches; feature branches are manual)

### What gbiv does not do
- No remote operations beyond fetch/pull (no push, no PR creation)
- No CI/CD integration
- No multi-repo orchestration (one gbiv project = one git repo)
- No persistent daemon or background process
- No network communication (all operations are local + git remote)

## Evolution Vectors

Areas where the design is likely to grow:

1. **Cargo publication**: Packaging for `cargo install gbiv`.

## References

- `docs/llds/worktree-lifecycle.md` — init, reset, rebase-all, tidy, git_utils
- `docs/llds/feature-ledger.md` — GBIV.md format, mark command
- `docs/llds/observation.md` — status, exec
- `docs/llds/tmux-mirror.md` — tmux new-session, sync, clean
- `docs/llds/cli-and-palette.md` — main.rs dispatch, colors.rs constants
