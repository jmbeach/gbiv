---
version: 1
---

- [gbv-k4p] Touch up the readme (un AI-ify)
- [gbv-q3t] [in-progress] [by:jared] IDEATION: orchestration as a domain inside the single gbiv binary — gbiv start (foreground daemon) + gbiv fleet status|get|send (clients) + bundled gbiv-orchestrate skill. See docs/high-level-design.md § Orchestration. (see ./items/gbv-q3t.md)
- [gbv-x2v] [parent:gbv-q3t] [agent-ready] core::tmux module (single crate) + migrate gbiv tmux ops into it
- [gbv-9p1] [blocked-by:gbv-x2v] [parent:gbv-q3t] Orchestration module skeleton in the gbiv binary + tracing + tmux driver
- [gbv-h5m] [blocked-by:gbv-9p1] [parent:gbv-q3t] Pane locator (spec: docs/specs/pane-locator.md)
- [gbv-3wb] [blocked-by:gbv-h5m] [parent:gbv-q3t] HTTP server + port file (.gbiv/port) + prompt-response guard + gbiv start
- [gbv-d6t] [blocked-by:gbv-3wb] [parent:gbv-q3t] gbiv fleet client CLI (status, get, send)
- [gbv-k0z] [blocked-by:gbv-d6t] [parent:gbv-q3t] gbiv install-skill + bundled gbiv-orchestrate SKILL.md
