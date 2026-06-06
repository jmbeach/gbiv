# 🌈 gbiv

> **gbiv** · /ˈjeeːbiv/ · *noun*
>
> **1.** A CLI for managing git worktrees with a rainbow-inspired structure.
>
> *"I gbiv'd my repo and now I have 7 places to abandon features"*

![Red](https://img.shields.io/badge/🔴-red-red)
![Orange](https://img.shields.io/badge/🟠-orange-orange)
![Yellow](https://img.shields.io/badge/🟡-yellow-yellow)
![Green](https://img.shields.io/badge/🟢-green-green)
![Blue](https://img.shields.io/badge/🔵-blue-blue)
![Indigo](https://img.shields.io/badge/🟣-indigo-blueviolet)
![Violet](https://img.shields.io/badge/💜-violet-violet)

## Why gbiv?

**gbiv** gives you seven named worktrees — one per ROYGBIV color — so you always have a clean slot ready:

```
myproject/
├── main/
│   └── myproject/     # main branch
├── red/
│   └── myproject/     # worktree
├── orange/
│   └── myproject/     # worktree
├── yellow/
│   └── myproject/     # worktree
├── green/
│   └── myproject/     # worktree
├── blue/
│   └── myproject/     # worktree
├── indigo/
│   └── myproject/     # worktree
└── violet/
    └── myproject/     # worktree
```

Seven fixed slots means no folder sprawl, no reinstalling dependencies when you context-switch, and a hard cap on how much parallel work you take on. When something ships, `gbiv reset` reclaims the slot.

## Installation

```bash
cargo install gbiv
```

Or build from source:

```bash
git clone https://github.com/jmbeach/gbiv.git
cd gbiv
cargo install --path crates/gbiv
```

`gbiv` lives inside a Cargo workspace (the repo root is a `[workspace]`
manifest, not a package), so `cargo install --path .` will not work —
point cargo at the `crates/gbiv` package directly. The crates.io install
above is unaffected.

## Usage

### Initialize a repository

Run from the **parent folder** of your git repository:

```bash
cd ~/projects
gbiv init myproject
```

Turns `projects/myproject/` into:

```
projects/
└── myproject/
    ├── main/myproject/     # original repo (main branch)
    ├── red/myproject/      # new worktree (red branch)
    ├── orange/myproject/   # new worktree (orange branch)
    ├── yellow/myproject/   # new worktree (yellow branch)
    ├── green/myproject/    # new worktree (green branch)
    ├── blue/myproject/     # new worktree (blue branch)
    ├── indigo/myproject/   # new worktree (indigo branch)
    └── violet/myproject/   # new worktree (violet branch)
```

### Check worktree status

Run from **any worktree** within a gbiv-structured repository:

```bash
gbiv status
```

```
red      red                      clean
orange   feature/login            dirty  merged  3 days  ↑2 ↓0
yellow   yellow                   clean
green    fix/bug-123              clean  not merged  12 days  no upstream
blue     blue                     dirty
indigo   missing
violet   violet                   clean
```

Each row shows the color (in its ANSI color), branch name, and clean/dirty state. When a worktree is on a named branch (meaning actual work is in progress), you also see whether it's merged upstream, how old the last commit is, and the ahead/behind count.

### Start a tmux session

Run from **any worktree** within a gbiv-structured repository:

```bash
gbiv tmux new-session
```

Creates a detached tmux session with one named window per worktree, each opened in its respective directory:

```
main     ~/projects/myproject/main/myproject
red      ~/projects/myproject/red/myproject
orange   ~/projects/myproject/orange/myproject
yellow   ~/projects/myproject/yellow/myproject
green    ~/projects/myproject/green/myproject
blue     ~/projects/myproject/blue/myproject
indigo   ~/projects/myproject/indigo/myproject
violet   ~/projects/myproject/violet/myproject
```

The session is named after the gbiv folder (e.g. `myproject`) by default. Use `--session-name` to override:

```bash
gbiv tmux new-session --session-name work
tmux attach -t work
```

Worktree directories that don't exist are skipped with a warning. The command errors if:
- `tmux` is not installed
- you are not inside a gbiv-structured repository
- a session with that name already exists


### Rebase all worktrees

Run from **any worktree** within a gbiv-structured repository:

```bash
gbiv rebase-all
```

Pulls the latest changes into the main worktree, then rebases every color worktree onto the remote main branch (`origin/main`, `origin/master`, or `origin/develop`):

```
Pulling main worktree (origin/main)...
[red]    OK ✓
[orange] SKIP (no worktree)
[yellow] SKIP (rebase in progress)
[green]  FAILED ✗
[blue]   OK ✓
[indigo] SKIP (no worktree)
[violet] OK ✓
```

- Worktrees that don't exist are skipped
- Worktrees already mid-rebase are skipped (resolve manually and re-run)
- Worktrees with conflicts are left in the conflicted state for you to resolve

### Reset a worktree

Run from **any worktree** within a gbiv-structured repository:

```bash
gbiv reset [<color>]
```

Once a feature branch has been merged into remote main, this command checks out the color branch, pulls the latest, and removes the matching entry from `GBIV.md`. Omit `<color>` to process all worktrees.

```bash
gbiv reset red      # reset a single worktree
gbiv reset          # reset all worktrees
```

- Worktrees already on their color branch are skipped
- Worktrees whose feature branch is not yet merged are skipped with a warning
- Worktrees with no remote configured are skipped with a warning
- The `GBIV.md` entry tagged `[<color>]` is removed from the main worktree after a successful checkout and pull

### GBIV.md

`gbiv init` automatically creates a `GBIV.md` in the root of your repository inside the `main/` worktree (e.g., `main/myproject/GBIV.md`). Add features to it and they will appear at the bottom of `gbiv status`.

**File format:**

- Lines starting with `- ` are feature entries.
- An optional `[color]` tag at the start of a feature line maps it to a rainbow color.
- Any non-blank line that does NOT start with `- ` is treated as a note attached to the preceding feature.
- A `---` line stops parsing — everything below it is ignored.
- The file is optional. When absent or empty, `gbiv status` output is unchanged.

**Example `GBIV.md`:**

```markdown
- [red] Fix critical auth bug
  Blocking release — must ship this week
- [green] Refactor database layer
- Add dark mode
  Low priority, nice to have
---
Old notes below here are ignored
```

**Example `gbiv status` output with GBIV.md:**

```
red      feat/auth-fix            dirty  not merged  1 day   ↑3 ↓0
orange   orange                   clean
yellow   yellow                   clean
green    refactor/db              clean  not merged  2 days  ↑1 ↓0
blue     blue                     clean
indigo   missing
violet   violet                   clean

GBIV.md
  red       Fix critical auth bug
  green     Refactor database layer
  backlog   Add dark mode
```

Tagged features display in their matching ANSI color. Untagged features show a dim `backlog` label.

### Requirements

Before running `init`, ensure:

- You're in the **parent folder** of the target repository
- The target folder is a **git repository**
- The repository has **at least one commit**
- No existing branches named after ROYGBIV colors

## Color Guide

You can assign some sort of meaning to the colors like "urgent = red", but I just gravitate to the colors I like the most. I go to violet. If violet's taken I use indigo etc. These folders could have just as easily been named after numbers, but colors are more fun 💖.

## License

MIT
