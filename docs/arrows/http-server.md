# Arrow: HTTP Server

The only inbound surface of the gbiv orchestration daemon: binds to 127.0.0.1, translates requests into Pane Locator + tmux Driver calls.

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; no code.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/high-level-design.md` |
| LLD | `docs/llds/http-server.md` |
| EARS specs | (none yet — pending `docs/specs/http-server.md`) |
| Source | (none yet) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Endpoints:**
```
GET  /sessions[?lines=N]
GET  /session/:color[?lines=N]
POST /session/:color/send       body: {"text": "..."}
```

**Statelessness:** No session, no cache, no in-memory buffer. Every request re-resolves panes and re-captures output.

## Work Required

- Author EARS specs in `docs/specs/http-server.md`
- Implement against pane-locator + tmux-driver
