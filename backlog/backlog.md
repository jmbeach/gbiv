---
version: 1
---

- [gbv-q3t] [in-progress] [by:jared] IDEATION: roy (gbiv orchestrator). A separate binary that let's you control agent harnesses that you're already running in your GBIV worktrees. (see ./items/gbv-q3t.md)
- [gbv-x2v] [in-progress] [by:jared] [parent:gbv-q3t] gbiv-core::tmux primitives + gbiv migration
- [gbv-9p1] [blocked-by:gbv-x2v] [parent:gbv-q3t] roy crate skeleton + tracing + tmux driver
- [gbv-h5m] [blocked-by:gbv-9p1] [parent:gbv-q3t] roy pane locator
- [gbv-3wb] [blocked-by:gbv-h5m] [parent:gbv-q3t] roy HTTP server + port file + prompt-response guard + roy start
- [gbv-d6t] [blocked-by:gbv-3wb] [parent:gbv-q3t] roy client CLI (status, get, send)
- [gbv-k0z] [blocked-by:gbv-d6t] [parent:gbv-q3t] roy install-skill subcommand + bundled SKILL.md
- [gbv-k4p] Touch up the readme (un AI-ify)
- [gbv-f3p] Backfill LLDs + EARS specs for the brownfield gbiv-core modules: colors, error, gitignore, root. Each module currently has no LLD; the gbiv-core HLD lists them as "(not yet specified — brownfield)". Walk them through the standard LID phases as separate segments. Resolves the four UNMAPPED entries that this scope leaves in docs/arrows/index.yaml.
