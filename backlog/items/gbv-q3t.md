---
id: gbv-q3t
---

Orchestration as a domain inside the single gbiv binary (NOT a separate binary, NOT a rename to roy — everything stays gbiv).

- `gbiv start` — foreground daemon (the commander).
- `gbiv fleet status|get|send` — clients that query and command the other gbiv worktree sessions.
- Bundled `gbiv-orchestrate` skill drives a commander Claude Code instance.

The commander gets notified about changes to the other gbiv worktree sessions, can command them (through tmux), and read their session data. For the initial phase it works with Claude Code, but aims to expand to other harnesses like Codex.

See docs/high-level-design.md § Orchestration.
