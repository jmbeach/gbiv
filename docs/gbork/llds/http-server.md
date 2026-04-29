# HTTP Server

**Created**: 2026-04-28
**Status**: Draft

## Context

The HTTP Server is the only inbound surface of the gbork daemon. It binds to `127.0.0.1` on an ephemeral port, accepts requests from local Claude Code sessions (or any other localhost caller), and translates each request into a small dance of Pane Locator + tmux Driver calls.

The server is stateless beyond the bound socket. There is no session, no cache, no in-memory buffer. Every request re-resolves panes and re-captures output.

## Endpoints

```
GET  /sessions[?lines=N]
GET  /session/:color[?lines=N]
POST /session/:color/send       body: {"text": "..."}
```

All responses are `Content-Type: application/json`. Errors carry a JSON body with an `error` field plus an HTTP status code.

### GET /sessions

Returns one entry per ROYGBIV color, in canonical order. For each color, the server resolves the pane via the Pane Locator and (if resolution succeeded) captures the last `N` lines.

Query parameters:
- `lines` (optional, default 50, max 1000)

Response body:
```json
[
  {
    "color": "red",
    "tmux_window": "red",
    "claude_pane": "%12",
    "pane_status": "ok",
    "output": "...last N lines...",
    "captured_at": "2026-04-28T13:45:01Z"
  },
  {
    "color": "orange",
    "tmux_window": null,
    "claude_pane": null,
    "pane_status": "no_window",
    "output": null,
    "captured_at": null
  },
  ...
]
```

`pane_status` is one of: `ok`, `no_window`, `no_claude_pane`, `multiple_claude_panes`. When status is anything other than `ok`, `output` and `captured_at` are `null` and `claude_pane` may be `null` or (for `multiple_claude_panes`) an array stringified into the pane field — see below.

For `multiple_claude_panes`, the entry uses `claude_panes: ["%12", "%17"]` (plural) instead of `claude_pane`. The commander sees the ambiguity surfaced exactly.

Status code: always 200, even when individual colors have problems. A whole-fleet query is a survey, not a precondition.

### GET /session/:color

Returns the last `N` lines of the single named color. Used when the commander wants detail rather than a fleet survey.

Query parameters:
- `lines` (optional, default 200, max 5000)

Response body (success):
```json
{
  "color": "red",
  "claude_pane": "%12",
  "pane_status": "ok",
  "captured_at": "2026-04-28T13:45:01Z",
  "output": "..."
}
```

Status codes:
- `200` — `pane_status: ok`
- `200` — `pane_status: no_claude_pane | multiple_claude_panes` (the color exists but has no usable pane; body explains)
- `404` — color is not a ROYGBIV color, or the tmux window for the color doesn't exist (`pane_status: no_window`)
- `503` — tmux session does not exist (daemon misconfigured or session was killed)

The split between `404` and `200-with-non-ok-status` is intentional: missing-window is a structural problem the caller can't fix from a request, whereas missing-claude is a transient state the caller should observe and react to.

### POST /session/:color/send

Resolves the pane (must be `ok`), then calls `tmux_driver::send_keys`.

Request body:
```json
{"text": "yes"}
```

`text` is the literal string to type into the pane. The server appends an Enter via the tmux Driver — callers do not include their own newline.

Response body (success):
```json
{"ok": true, "sent_to_pane": "%12"}
```

Response body (error):
```json
{"ok": false, "error": "no_claude_pane", "color": "red"}
```

Status codes:
- `200` — keystrokes accepted by tmux
- `400` — request body missing/invalid; `text` is empty
- `404` — color invalid or no window
- `409` — `pane_status` is `no_claude_pane` or `multiple_claude_panes` (resolvable conflict, not malformed input)
- `502` — tmux driver returned `SendKeysIncomplete` (text sent, Enter failed)
- `503` — tmux session does not exist

## Lifecycle

### Startup

1. Discover the gbiv root by walking up from CWD (reuses `gbiv-core`).
2. Discover the tmux session name (folder name unless `--session-name` provided; reuses gbiv's logic).
3. Verify `tmux -V` succeeds → fatal exit if not.
4. Bind a TCP listener on `127.0.0.1:0` (kernel-assigned port).
5. Create `<gbiv-root>/main/<repo>/.gbork/` if missing.
6. Write the bound port to `<gbiv-root>/main/<repo>/.gbork/port` as plain ASCII (e.g., `54321\n`).
7. Print `gbork listening on http://127.0.0.1:<port>` to stdout.
8. Block in the accept loop.

### Shutdown

- Ctrl+C / SIGTERM: best-effort delete the port file, then exit. Listener cleanup is handled by process exit.
- Any other exit (panic, bind failure mid-flight): port file may be left stale. CLI subcommands handle stale port files (see CLI LLD: connection-refused → "is the daemon running?").

### Concurrency

- One worker thread per request. tiny_http (or equivalent sync HTTP lib) gives a thread per accepted connection.
- Pane Locator and tmux Driver are independently safe to call concurrently — they hold no shared state and tmux subprocesses don't conflict at the granularity gbork uses.
- Request handling is bounded by tmux subprocess speed (~tens of ms per call). v1 sets a max of 16 worker threads to cap runaway parallelism if a misbehaving client floods.

## Binding & Security

- **Bind address**: `127.0.0.1` only. Never `0.0.0.0`. Configurable via `--bind` flag if a future use case requires it, but v1 ignores the flag.
- **Authentication**: none. Localhost-only is the trust boundary in v1. The HLD non-goal is restated here.
- **CORS**: not implemented. Browsers are not the expected client.
- **TLS**: not implemented. The whole exchange is on the loopback interface.

## HTTP Library Choice

v1 uses `tiny_http` (sync, no runtime, ~3K LOC). Rationale:

- gbork has 3 endpoints. Async is not earning its keep at this scale.
- gbiv currently has zero async dependencies. Adding `tokio` would dwarf the rest of the project's dep graph.
- `tiny_http` is well-maintained and stable.

JSON serialization uses `serde` + `serde_json`. These are the de-facto standard and pull in cleanly.

If gbork later needs SSE streaming (`/events`), revisit: `hyper` + `tokio` becomes a credible move.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| HTTP library | `tiny_http` | `axum` + `tokio`, `actix-web`, raw `hyper` | Sync is sufficient at 3 endpoints; no async runtime keeps the dep graph small |
| Server statefulness | Stateless | Cache resolutions / outputs for N seconds | Daemon is on-demand; cache invalidation isn't worth the complexity |
| `/sessions` partial-failure handling | Always 200, per-color status in body | 207 Multi-Status, fail-fast on first error | A survey endpoint should not fail the whole survey because one color is sad |
| `multiple_claude_panes` representation | Distinct status + plural `claude_panes` array | Pick one, ignore others | Surface ambiguity to the commander |
| Send body shape | `{"text": "..."}` | Plain string body, multipart, query param | JSON is consistent with responses; explicit field name leaves room for future fields (`text_only_no_enter`, etc.) |
| Pane Locator runs per-request | Yes | Cache for short TTL | Trades a few ms per call for zero invalidation logic; revisit if `/sessions` becomes hot |
| Worker model | Thread per request, capped at 16 | Single-threaded loop, fully async | Threads are simple and adequate; cap prevents pathological clients |

## Edge Cases

| Case | Behavior |
|---|---|
| `lines` exceeds max | Clamped to max; response includes the clamped value implicitly via length |
| `lines` non-numeric | `400 Bad Request` |
| Color in URL is uppercase or has trailing slash | Lowercased and stripped; `RED/` resolves to `red` |
| Body of POST `/send` is not valid JSON | `400` |
| `text` field present but empty string | `400` (HLD: caller responsible for not sending empty) |
| Two simultaneous sends to the same color | Both go through; tmux serializes keystrokes per pane |
| Daemon already running (port file exists, port in use) | New `gbork start` fails to bind, exits with a clear message; existing daemon untouched |
| Port file exists but daemon is dead | `gbork start` succeeds (kernel rejects re-bind only if port still in use); writes a fresh port file |

## Technical Debt & Future Work

1. **No request logging** in v1. A `--log` flag could enable per-request logs; for now stdout is silent except for startup banner.
2. **No rate limiting**. Localhost trust boundary makes this low priority.
3. **Manual JSON shapes**. A future refactor could derive request/response types from a shared schema with the CLI.
4. **No graceful drain on shutdown** — in-flight requests are killed when the process exits. Acceptable for v1's "Ctrl+C and you're done" model.
5. **Worker cap is hard-coded.** Configurable via flag if real workloads ever bump against it.

## References

- HLD: `docs/gbork/high-level-design.md` § Components > HTTP Server, § HTTP API
- Companion: `docs/gbork/llds/pane-locator.md`, `docs/gbork/llds/tmux-driver.md`
- Companion: `docs/gbork/llds/gbork-cli.md` (HTTP client side)
