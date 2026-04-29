# gbork CLI

**Created**: 2026-04-28
**Status**: Draft

## Context

The `gbork` binary serves two roles from a single executable:

1. **Daemon mode** (`gbork start`): runs the HTTP Server in the foreground.
2. **Client mode** (`gbork status`, `gbork get`, `gbork send`): thin HTTP clients that talk to a running daemon.

Splitting these into two binaries was considered and rejected — sharing one binary keeps installation simple (`cargo install gbork`) and lets the skill teach a single command name. clap's subcommand groups handle the dispatch.

## Subcommands

```
gbork start [--session-name <NAME>] [--bind <ADDR>]
gbork status [--lines <N>] [--json]
gbork get <COLOR> [--lines <N>] [--json]
gbork send <COLOR> <TEXT>
```

No `gbork stop` — Ctrl+C in the daemon's terminal is the only stop mechanism in v1 (HLD).
No `gbork restart` — that's `Ctrl+C` then `gbork start` again.

### gbork start

Runs the HTTP Server. Foreground only. See HTTP Server LLD for lifecycle details. Flags:

- `--session-name <NAME>`: override the inferred tmux session name. Mirrors the same flag on `gbiv tmux new-session` for consistency.
- `--bind <ADDR>`: parsed but ignored in v1 (HTTP Server is hard-coded to `127.0.0.1`). Reserved.

Exits non-zero if:
- Not inside a gbiv project
- `tmux -V` fails
- Port can't be bound (another daemon already running, sandbox restriction)

### gbork status

Prints a one-line-per-color summary table to stdout:

```
red       ok          %12   ↳ Building... (last activity 3s ago)
orange    ok          %18   ↳ Waiting for input: "Continue? (y/n)"
yellow    no_window
green     no_claude_pane
blue      ok          %23   ↳ Idle (last activity 2m ago)
indigo    no_window
violet    no_window
```

The "↳" line is a heuristic-extracted last meaningful line from the captured output — a single-line preview chosen by trimming trailing blank lines and prompts. The skill is the primary consumer and parses the JSON form anyway; the human-friendly form exists so a developer can run `gbork status` directly.

Flags:
- `--lines <N>`: forwarded to the API as the lines parameter.
- `--json`: prints the raw API response without formatting. The skill always uses `--json`.

Exits 0 if the daemon responded, even if some colors are unhealthy. Exits 2 if the daemon is not running.

### gbork get

Calls `GET /session/:color`. Default output is the captured pane text directly to stdout, prefixed with a one-line header showing `pane_status` and `captured_at`. With `--json`, prints the API response.

Flags:
- `--lines <N>` (default: API default of 200)
- `--json`

Exits:
- 0 — pane resolved cleanly
- 2 — daemon not running
- 3 — color is invalid or has no window (HTTP 404)
- 4 — color has no claude pane or multiple (HTTP 200 with non-ok status)

The non-zero-but-not-error codes let the skill branch on the outcome without parsing.

### gbork send

Calls `POST /session/:color/send` with `{"text": TEXT}`.

The text argument is taken verbatim. Quoting is the user's (or skill's) responsibility:

```
gbork send red "yes"
gbork send red "y"
gbork send red "1"
```

There is no `--enter-only` flag in v1 — every send appends Enter. If the commander needs to send a literal Tab or Escape, that's a future flag.

Exits:
- 0 — `{ok: true}` from server
- 2 — daemon not running
- 3 — invalid color / no window
- 4 — no claude pane / multiple
- 5 — `SendKeysIncomplete` (502 from server)
- 1 — other error

## Port Discovery

All client subcommands need the daemon's port. The lookup:

1. Walk up from CWD to find the gbiv root (reuses `gbiv-core`'s root-discovery utility).
2. Read `<gbiv-root>/main/<repo>/.gbork/port`. If missing, exit 2 with `"daemon not running (no port file at <path>); start it with: gbork start"`.
3. Parse as a `u16`. If malformed, exit 2 with `"port file at <path> is corrupt"`.
4. Open a TCP connection to `127.0.0.1:<port>`. If `ECONNREFUSED`, exit 2 with `"port file present but daemon not responding (stale?); restart with: gbork start"`.

Stale port files are detected lazily — on the next `gbork start`, the new daemon overwrites the file with its real port.

## HTTP Client

v1 uses `ureq` (sync, blocking, ~10 deps). Rationale:

- Matches the sync style of the HTTP Server.
- No tokio runtime needed.
- Built-in JSON support via the `json` feature.
- Subcommands are short-lived; per-call connection setup cost is negligible.

Each subcommand opens one connection, sends one request, parses one response, exits. No connection pooling, no retries.

Timeouts:
- Connect: 1s (localhost should resolve and accept instantly)
- Read: 30s (gives slow tmux snapshots room to breathe)

## Error Output

Client errors go to stderr in the form:
```
gbork: <subcommand>: <human-readable message>
```

JSON output (`--json`) goes to stdout regardless of error condition. This means a script can run `gbork status --json` and rely on stdout being either valid JSON or empty.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Single binary, multiple subcommands | Yes | Separate `gborkd` daemon binary | One install, one command for the skill to teach |
| HTTP client | `ureq` | `reqwest` (with sync), raw `std::net` + manual parsing | `ureq` is sync, light, JSON-ready; `reqwest` pulls async dep graph |
| Default human output for `status` | Table with heuristic preview | Raw JSON, no preview | A direct user should see something useful immediately; skill uses `--json` |
| Distinct exit codes for daemon-down vs no-pane | Yes (2, 3, 4, 5) | Always 1 with stderr message | Lets the skill branch programmatically without parsing stderr |
| Send appends Enter implicitly | Yes | Require explicit Enter in text | Most use cases want Enter; explicit flag for "no Enter" can come later if needed |
| Port file path | `<main-worktree>/.gbork/port` | `~/.gbork/<hash>.port`, `$XDG_STATE_HOME` | HLD decision: in-workspace, user gitignores `.gbork/` |

## Edge Cases

| Case | Behavior |
|---|---|
| Run any client subcommand outside a gbiv project | Exit 2 with "not inside a gbiv project" |
| Daemon running in workspace A, CLI run in workspace B | Each workspace has its own port file; B has none → exit 2. No cross-workspace confusion |
| Daemon's tmux session was killed mid-session | `gbork status` returns 200 with all colors `no_window` (since `list_windows` will fail to find the session, daemon returns 503; CLI surfaces "tmux session not found"). v1 does not auto-recover |
| `gbork send red ""` | Local validation: exit 1 with "text must not be empty" before opening a connection |
| `gbork send red <very long text>` | Forwarded as-is; HTTP server enforces any size limits, not the CLI |
| Two `gbork send red` racing | Both fire HTTP requests; daemon serializes (HTTP Server: thread-per-request, tmux serializes keys per-pane) |
| `--json` and human flags combined nonsensically | `--json` always wins |
| User invokes `gbork start` while a daemon is already running | TCP bind fails; exit 1 with "another gbork daemon may be running on port <N>; check the port file or run `lsof -i:<N>`" |

## Technical Debt & Future Work

1. **No `gbork stop`.** Foreground-only is the v1 contract. If `--detach` ever ships, `stop` ships with it.
2. **No `--bind` honored.** Reserved flag. Implementing requires HTTP Server changes too.
3. **Heuristic preview line is fragile.** It works well for typical agent output but a smarter summarizer (LLM-based) is on the HLD evolution list.
4. **No completion scripts.** clap can generate them; left for later.
5. **No structured logging.** Stdout for results, stderr for errors, no `--verbose`.

## References

- HLD: `docs/gbork/high-level-design.md` § Components > gbork CLI
- Companion: `docs/gbork/llds/http-server.md` (server side of the HTTP)
- Companion: `docs/gbork/llds/gbork-skill.md` (primary consumer)
