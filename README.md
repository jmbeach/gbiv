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

```bash
cargo install gbiv
```

Or build from source:

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
