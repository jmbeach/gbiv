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

Working with git worktrees is powerful, but managing them can get messy. **gbiv** standardizes your worktrees into 7 ever-present folders named after the colors of the rainbow (ROYGBIV):

```
myproject/
├── 🏠 main/
│   └── myproject/     # Your main branch lives here
├── 🔴 red/
│   └── myproject/     # Worktree 1
├── 🟠 orange/
│   └── myproject/     # Worktree 2
├── 🟡 yellow/
│   └── myproject/     # Worktree 3
├── 🟢 green/
│   └── myproject/     # Worktree 4
├── 🔵 blue/
│   └── myproject/     # Worktree 5
├── 🟣 indigo/
│   └── myproject/     # Worktree 6
└── 💜 violet/
    └── myproject/     # Worktree 7
```

### Benefits

| Benefit | Description |
|---------|-------------|
| 🚫 **No stale folders** | You won't end up with 100 folders for abandoned features |
| 🔧 **Persistent environments** | No reinstalling dependencies or copying configs for each new task |
| 🧠 **Lower cognitive load** | You're never working on more than 7 tasks in parallel |
| 🤖 **AI-agent friendly** | Let Claude or other AI agents work on multiple features (or parts of a feature) simultaneously, but in a way you can keep track of |

## Installation

Build from source (TODO: Upload to cargo):

```bash
git clone https://github.com/jmbeach/gbiv.git
cd gbiv
cargo build --release
```

## Usage

### Initialize a repository

Run from the **parent folder** of your git repository:

```bash
cd ~/projects
gbiv init myproject
```

This transforms:
```
projects/
└── myproject/          # Your existing repo
```

Into:
```
projects/
└── myproject/
    ├── main/myproject/     # Original repo (main branch)
    ├── red/myproject/      # New worktree (red branch)
    ├── orange/myproject/   # New worktree (orange branch)
    ├── yellow/myproject/   # New worktree (yellow branch)
    ├── green/myproject/    # New worktree (green branch)
    ├── blue/myproject/     # New worktree (blue branch)
    ├── indigo/myproject/   # New worktree (indigo branch)
    └── violet/myproject/   # New worktree (violet branch)
```

### Check worktree status

Run from **any worktree** within a gbiv-structured repository:

```bash
gbiv status
```

Output:
```
red      red                      clean
orange   feature/login            dirty  merged  3 days  ↑2 ↓0
yellow   yellow                   clean
green    fix/bug-123              clean  not merged  12 days  no upstream
blue     blue                     dirty
indigo   missing
violet   violet                   clean
```

For each worktree, shows:
- **Color name** (in color!)
- **Branch name**
- **Clean/dirty status** (unstaged, staged, or untracked changes)

If the branch differs from the worktree color, also shows:
- **Merged status** - whether merged into remote main/master/develop
- **Commit age** - how long since the last commit
- **Ahead/behind** - commits ahead/behind upstream (or "no upstream")

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
