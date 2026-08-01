# HTTP Server

**Created**: 2026-04-28
**Status**: Draft

## Context

The HTTP Server is the only inbound surface of the gbiv daemon. It binds to `127.0.0.1` on an ephemeral port, accepts requests from local Claude Code sessions (or any other localhost caller), and translates each request into a small dance of Pane Locator + tmux Driver calls.

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
- `lines` (optional, default 35, max 1000) — 35 captures an `AskUserQuestion` prompt with its options, a recent tool-use exchange, or the tail of a build, without flooding context for a 7-color survey. The consumer is expected to follow up with `GET /session/:color` for full detail when needed.

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

`pane_status` is one of: `ok`, `no_window`, `no_claude_pane`. When status is anything other than `ok`, `output`, `captured_at`, and `claude_pane` are `null`.

When the locator finds more than one claude pane in a window it picks the oldest by process start time (see Pane Locator LLD § "Resolve") and the entry's `pane_status` stays `ok`. The remaining claude pane IDs are exposed as `other_claude_panes: ["%17", "%23"]` so the commander can see the ambiguity was resolved automatically. The field is omitted (or empty) when only one claude pane was found.

Status code: always 200, even when individual colors have problems. A whole-fleet query is a survey, not a precondition.

### GET /session/:color

Returns rows from the named color's claude pane. Used when the commander wants detail rather than a fleet survey.

Query parameters (mutually exclusive groups):
- `lines` (optional, default 200, max 5000) — tail mode: last N rows. Maps to driver `CaptureRange::Tail { lines }`.
- `start_line` + `end_line` (optional pair, both signed integers in tmux row-offset semantics) — window mode for pagination. Maps to driver `CaptureRange::Window`. Use `start_line=top` (literal string) to mean "top of history" (driver `i32::MIN`). If either param is supplied without the other, returns `400`. If both are supplied along with `lines`, returns `400`.

Response body (success):
```json
{
  "color": "red",
  "claude_pane": "%12",
  "pane_status": "ok",
  "captured_at": "2026-04-28T13:45:01Z",
  "output": "...",
  "output_truncated": false,
  "output_original_bytes": 4821,
  "output_returned_bytes": 4821,
  "range_returned": {"start_line": -200, "end_line": 0}
}
```

When the underlying capture exceeded the driver's byte cap (see tmux-driver LLD § "Output Cap"), `output_truncated` is `true`, `output` is prefixed with the driver's `[…truncated …]` marker line, and the `*_bytes` fields report the discrepancy. `range_returned.start_line` will have shifted forward (toward `end_line`) to reflect the rows actually returned — the caller paginates earlier history by re-calling with `end_line = <previous range_returned.start_line - 1>`. The same fields appear on each entry of `GET /sessions`. The HTTP layer does not re-truncate — the driver is the single point of enforcement.

A typical pagination flow:
```
GET /session/red?lines=500          → output_truncated=true, range_returned={start_line:-312,end_line:0}
GET /session/red?start_line=-700&end_line=-313  → next-older chunk
GET /session/red?start_line=top&end_line=-701   → all the rest, from top of history
```

Status codes:
- `200` — `pane_status: ok` (single claude pane, or oldest-of-many auto-picked; in the latter case the body includes `other_claude_panes`)
- `200` — `pane_status: no_claude_pane` (the color exists but has no claude pane; body explains)
- `404` — color is not a ROYGBIV color, or the tmux window for the color doesn't exist (`pane_status: no_window`)
- `503` — tmux session does not exist (daemon misconfigured or session was killed)

The split between `404` and `200-with-non-ok-status` is intentional: missing-window is a structural problem the caller can't fix from a request, whereas missing-claude is a transient state the caller should observe and react to.

### POST /session/:color/send

Validates `:color` against the active palette *before touching the request body at all* — an unrecognized color 404s without a single byte being read off the socket, not merely without JSON-parsing an already-buffered body (this ordering lives in the routing layer, not inside the handler, since a handler can only reject a body it has already been handed one). Once the color is valid, the body is read capped at a fixed size (64 KiB — generous for a `{"text": "..."}` payload, small enough to bound what one worker thread will buffer for a misbehaving local client; loopback-only per § "Binding & Security" so this is defense-in-depth, not network hardening). Then: parses input shape, runs the prompt-response guard, resolves the pane (must be `ok`), then calls `tmux_driver::send_keys`.

Request body:
```json
{"text": "please run the tests"}
```

`text` is the literal string to type into the pane. The server appends an Enter via the tmux Driver — callers do not include their own newline.

Response body (success):
```json
{"ok": true, "sent_to_pane": "%12"}
```

Response body (error):
```json
{
  "ok": false,
  "error": "looks_like_prompt_response",
  "reason": "yes/no word",
  "color": "red",
  "explanation": "gbiv refused this send because the trimmed text matches the shape of a response to a Claude Code permission prompt or AskUserQuestion choice (rule: yes/no word). gbiv never answers prompts on the user's behalf in v1 — a misread of pane state could approve actions the user has not seen. Do NOT retry by paraphrasing, padding with filler words, or otherwise reshaping the same intent to slip past the guard; the guard is shape-based but the rule's purpose is intent-based, and bypassing it violates the user's trust. Correct action: tell the user that red appears to be waiting on a prompt and ask them to answer it themselves in red's tmux window. If the user genuinely wants to send substantive natural-language guidance (not a prompt answer), send that instead.",
  "docs": "docs/high-level-design.md#reject-prompt-shaped-input-on-send"
}
```

The `explanation` field is verbose on purpose: the typical caller is an LLM, and a terse error invites creative workarounds. Spelling out *why* the rule exists and *what to do instead* is cheaper than letting an agent re-derive (and likely re-violate) the intent. The `reason` field stays short and machine-friendly for programmatic branching.

Status codes:
- `200` — keystrokes accepted by tmux
- `400` — request body missing/invalid; `text` is empty
- `404` — color invalid or no window
- `409` — input was rejected by the prompt-response guard, OR `pane_status` is `no_claude_pane` (resolvable conflict, not malformed input). The `no_claude_pane` case's body is `{"ok": false, "error": "no_claude_pane", "color": "<color>"}` — no `explanation` field, since there's no guard rule to explain (the caller just needs to know the window has no active claude pane to send to). Multiple-claude-panes is **not** a 409 — the locator auto-picks the oldest and returns `ok`; the response body includes `other_claude_panes` so the caller knows.
- `502` — tmux driver returned `SendKeysIncomplete` (text sent, Enter failed)
- `503` — tmux session does not exist

#### Prompt-Response Guard

Before pane resolution, `text` is trimmed of leading/trailing whitespace and tested against the rule set below. Any match is rejected with `409` and `error: "looks_like_prompt_response"`. The `reason` field names the matching rule (machine-readable). The `explanation` field (see response body above) carries a verbose human/LLM-readable rationale and an explicit "do not try to bypass this" instruction — the consumer is almost always an LLM, and a one-line error invites creative re-attempts.

| Pattern (case-insensitive, on trimmed text) | `reason` value |
|---|---|
| `^[yn]$` | `single-letter yes/no` |
| `^(yes\|no)$` | `yes/no word` |
| `^\d{1,3}$` | `numeric choice` |
| single non-alphanumeric character | `bare punctuation` |

Multi-word natural-language text (e.g., `"yes please run that"`) is **not** rejected — only the trimmed text as a whole is matched against the patterns. Empty text after trim continues to return `400`, not the guard error, since it's malformed rather than dangerous-shaped. Rationale and the path to loosening this in a future version are in the HLD § "Reject prompt-shaped input on send."

## Lifecycle

### Startup

1. Discover the gbiv root by walking up from CWD (`core::find_gbiv_root`).
2. Resolve `main/<repo>/` (`core::find_repo_in_worktree`) and the tmux session name (folder-derived via `core::tmux::session_name_for_root` unless `--session-name` is provided; both come from the `core` module so the daemon and the worktree commands cannot disagree).
3. Load the active palette (`gbiv_core::palette::Palette::load(&gbiv_root)`, base ROYGBIV plus any configured `.gbiv/config.toml` extras) once at startup and hold it for the process lifetime — see § "Active Palette" below.
4. Verify `tmux -V` succeeds (`core::tmux::tmux_available`) → fatal exit if not.
5. Check the existing `.gbiv/port` file (if any) for a live daemon — see § "Single-Instance Guard" below. Fatal exit if one is found; otherwise continue.
6. Bind a TCP listener on `127.0.0.1:0` (kernel-assigned port).
7. Create `<gbiv-root>/main/<repo>/.gbiv/` if missing.
8. Write the bound port to `<gbiv-root>/main/<repo>/.gbiv/port` as plain ASCII (e.g., `54321\n`).
9. Ensure `.gbiv/` is in `.git/info/exclude` (`core::ensure_gitignore_entry`) so the user doesn't have to edit anything to keep the port file out of git.
10. Validate `:color` URL params against the loaded `Palette::contains` at request time (rejected at the routing layer before the locator is called) — see § "Active Palette".
11. Print `gbiv listening on http://127.0.0.1:<port>` to stdout.
12. Block in the accept loop.

### Single-Instance Guard

Binding always requests a fresh ephemeral port (`127.0.0.1:0`), so an OS-level bind failure can never mean "another gbiv daemon already holds this workspace's port" — the kernel will happily hand out a different port to a second daemon. Detecting an already-running daemon is therefore a separate, explicit check *before* binding: if `.gbiv/port` exists, the new process attempts a short-timeout (200ms) loopback TCP connect to the port it names.

- **Connect succeeds** → a daemon is still listening there. The new process exits non-zero without binding a listener or touching the port file, so the running daemon is never orphaned by a second one silently taking over the file.
- **Connect fails** (refused/timeout) → the port file is stale (previous daemon exited without cleanup, e.g. `kill -9`). Startup proceeds normally: bind, then overwrite the port file.

This is a liveness probe, not process-identity verification — it can't distinguish "a gbiv daemon" from "some other process that happens to be listening on that exact port," but that's an acceptable tradeoff for a v1, single-workspace, developer-local tool.

### Active Palette

The server validates and iterates colors against the **active palette** (base ROYGBIV plus any `.gbiv/config.toml` extras), not a hard-coded `BASE_COLORS` list — consistent with every other gbiv surface (`status`, `exec all`, `tmux sync`), which already treat configured extras as first-class. The palette is loaded once at startup (step 3 above) rather than per-request: it changes only when a human edits `.gbiv/config.toml`, and a running daemon reflecting a stale palette until restarted is an acceptable tradeoff against re-reading a config file on every request. `GET /sessions` iterates the loaded palette's `names()` in order; `GET /session/:color` and `POST /session/:color/send` validate `:color` via `Palette::contains`.

### Shutdown

- Ctrl+C / SIGTERM: best-effort delete the port file, then exit. Registered via the `ctrlc` crate (small, cross-platform SIGINT handling; SIGTERM too on unix), the only signal-handling dependency in the workspace. Listener cleanup is handled by process exit.
- Any other exit (panic, bind failure mid-flight): port file may be left stale. CLI subcommands handle stale port files (see CLI LLD: connection-refused → "is the daemon running?").

### Concurrency

- **16 long-lived worker threads**, each looping `tiny_http::Server::recv()` on the one shared `Server` instance. `tiny_http::Server` is documented as safe to call `.recv()` from multiple threads concurrently — this is the library's intended pattern for bounded parallelism, and it naturally caps concurrency at exactly 16 in-flight requests without a separate semaphore or thread-spawn-per-connection.
- Pane Locator and tmux Driver are independently safe to call concurrently — they hold no shared state and tmux subprocesses don't conflict at the granularity gbiv uses.
- Request handling is bounded by tmux subprocess speed (~tens of ms per call). The worker count (16) caps runaway parallelism if a misbehaving client floods; a 17th concurrent request queues in the OS accept backlog until a worker frees up.

## Binding & Security

- **Bind address**: `127.0.0.1` only. Never `0.0.0.0`. Configurable via `--bind` flag if a future use case requires it, but v1 ignores the flag.
- **Authentication**: none. Localhost-only is the trust boundary in v1. The HLD non-goal is restated here.
- **CORS**: not implemented. Browsers are not the expected client.
- **TLS**: not implemented. The whole exchange is on the loopback interface.

## Error Handling

Request handlers are written as `fn handle_xxx(...) -> anyhow::Result<HttpResponse>`. Inside a handler, `?` on a `TmuxError` or `LocatorError` short-circuits to a single error path, where a small `into_response()` helper inspects the chain (via `downcast_ref::<TmuxError>()` etc.) to pick the right HTTP status:

| Typed error | Status |
|---|---|
| `TmuxError::SessionNotFound` | 503 |
| `TmuxError::PaneNotFound` | 404 |
| `TmuxError::SendKeysIncomplete` | 502 |
| `TmuxError::Other` | 500 |
| `LocatorError::TmuxSession(...)` | unwraps to the inner `TmuxError` mapping above |
| Anything else (anyhow opaque) | 500 |

`anyhow::Error::context("…")` is used liberally to attach handler-level breadcrumbs (the route, color, and request id). Library modules never return `anyhow::Error` themselves — they return their typed enums and the handler is the conversion point.

## HTTP Library Choice

v1 uses `tiny_http` (sync, no runtime, ~3K LOC). Rationale:

- gbiv has 3 endpoints. Async is not earning its keep at this scale.
- gbiv currently has zero async dependencies. Adding `tokio` would dwarf the rest of the project's dep graph.
- `tiny_http` is well-maintained and stable.

JSON serialization uses `serde` + `serde_json`. These are the de-facto standard and pull in cleanly.

If gbiv later needs SSE streaming (`/events`), revisit: `hyper` + `tokio` becomes a credible move.

Shutdown signal handling uses the `ctrlc` crate (small, ~2 transitive deps) rather than hand-rolled `signal-hook`/raw `libc` calls — it handles SIGINT portably and SIGTERM on unix with one `ctrlc::set_handler` call, which is all the port-file cleanup in § "Shutdown" needs.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| HTTP library | `tiny_http` | `axum` + `tokio`, `actix-web`, raw `hyper` | Sync is sufficient at 3 endpoints; no async runtime keeps the dep graph small |
| Server statefulness | Stateless | Cache resolutions / outputs for N seconds | Daemon is on-demand; cache invalidation isn't worth the complexity |
| `/sessions` partial-failure handling | Always 200, per-color status in body | 207 Multi-Status, fail-fast on first error | A survey endpoint should not fail the whole survey because one color is sad |
| Multiple claude panes | Locator auto-picks oldest; HTTP layer returns `ok` plus `other_claude_panes: [...]` | Distinct status (`multiple_claude_panes`); pick first; ignore others silently | Old session is almost always the worktree's primary claude. Surfacing the also-rans keeps it transparent without forcing the caller to handle a separate error path |
| Send body shape | `{"text": "..."}` | Plain string body, multipart, query param | JSON is consistent with responses; explicit field name leaves room for future fields (`text_only_no_enter`, etc.) |
| Pane Locator runs per-request | Yes | Cache for short TTL | Trades a few ms per call for zero invalidation logic; revisit if `/sessions` becomes hot |
| Worker model | 16 worker threads looping `Server::recv()` | Thread spawned per accepted connection; single-threaded loop; fully async | Matches tiny_http's documented concurrent-`recv()` pattern; caps concurrency without a thread-spawn-per-request or a separate semaphore |
| `:color` validation scope | Active palette (base ROYGBIV + configured `.gbiv/config.toml` extras), loaded once at startup | Hard-coded `BASE_COLORS` only | Consistent with every other gbiv surface (status/exec/tmux already treat extras as first-class) |
| Shutdown signal handling | `ctrlc` crate | Hand-rolled `signal-hook` or raw `libc` | One-call portable SIGINT/SIGTERM handling; workspace's only signal-handling dependency |
| Single-instance detection | Loopback TCP liveness probe against the existing port file, before binding | Rely on OS bind failure; PID file + `kill -0` | Binding always requests an ephemeral port, so bind can never fail on "port in use by another gbiv daemon" — a probe against the *previous* port is the only way to detect it |
| POST /send body size | Capped at 64 KiB, checked before the color-validated body is read | No cap; cap after JSON parsing | Bounds per-request memory on a loopback-only surface without touching the common case (real `text` payloads are tiny) |

## Edge Cases

| Case | Behavior |
|---|---|
| `lines` exceeds max | Clamped to the endpoint's documented max before being passed to the driver; the driver's byte cap may further trim the result and set `output_truncated: true` |
| `lines` non-numeric | `400 Bad Request` |
| Color in URL is uppercase or has trailing slash | Lowercased and stripped; `RED/` resolves to `red` |
| Body of POST `/send` is not valid JSON | `400` |
| `text` field present but empty string | `400` (HLD: caller responsible for not sending empty) |
| Two simultaneous sends to the same color | Both go through; tmux serializes keystrokes per pane |
| Daemon already running (port file exists and its port answers a loopback connect) | New `gbiv start` refuses to bind or touch the port file, exits with a clear message; existing daemon untouched (Single-Instance Guard, below) |
| Port file exists but daemon is dead (port doesn't answer) | `gbiv start` proceeds normally, binds a fresh port, and overwrites the port file |
| `POST /send` body exceeds the size cap (declared `Content-Length` or actual bytes) | `400`, body not treated as complete/parsed even if truncated bytes happen to be valid JSON |

## Technical Debt & Future Work

1. **Logging sinks are stderr-only** in v1 — file/syslog/JSON are deferred. The framework (`tracing`) and the per-request `info` line are in place from day one; see orchestrate-cli LLD § "Logging" for the shared design.
2. **No rate limiting**. Localhost trust boundary makes this low priority.
3. **Manual JSON shapes**. A future refactor could derive request/response types from a shared schema with the CLI.
4. **No graceful drain on shutdown** — in-flight requests are killed when the process exits. Acceptable for v1's "Ctrl+C and you're done" model.
5. **Worker cap is hard-coded.** Configurable via flag if real workloads ever bump against it.

## References

- HLD: `docs/high-level-design.md` § Components > HTTP Server, § HTTP API
- Companion: `docs/llds/pane-locator.md`, `docs/llds/tmux-driver.md`
- Companion: `docs/llds/orchestrate-cli.md` (HTTP client side)
