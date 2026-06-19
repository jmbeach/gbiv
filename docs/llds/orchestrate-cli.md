# gbiv — Orchestration CLI

**Created**: 2026-04-28
**Status**: Draft

> **Decision: `gbiv fleet` command group.** The orchestration client commands live
> under a `gbiv fleet` subcommand group — `gbiv fleet status` / `gbiv fleet get
> <color>` / `gbiv fleet send <color>` — so they don't collide with the worktree
> `gbiv status` (observation domain). The daemon launcher (`gbiv start`) and
> `gbiv install-skill` stay top-level.

## Context

This LLD covers the **orchestration** slice of the single `gbiv` binary — the
`gbiv start` daemon and its HTTP client subcommands. It serves two roles:

1. **Daemon mode** (`gbiv start`): runs the HTTP Server in the foreground.
2. **Client mode** (the orchestration client subcommands): thin HTTP clients that talk to a running daemon.

Splitting these into two binaries was considered and rejected — sharing one binary keeps installation simple (`cargo install gbiv`) and lets the skill teach a single command name. clap's subcommand groups handle the dispatch.

### Audience

**gbiv is designed first and foremost as a tool for an LLM (specifically, a Claude Code session loaded with the gbiv-orchestrate skill).** Every CLI design choice in this document — JSON-only output, structured error bodies, verbose `explanation` fields on guard rejections, distinct exit codes, no pretty-printing — is in service of that consumer. Human use (a developer typing `gbiv fleet status` in a terminal) is *supported* but not optimized for; the recommended human path is to pipe the JSON through `jq`. If a future change forces a tradeoff between LLM-friendliness and human-friendliness, LLM wins.

## Subcommands

```
gbiv start         [--session-name <NAME>] [--bind <ADDR>]
gbiv fleet status        [--lines <N>]
gbiv fleet get <COLOR>   [--lines <N> | --start-line <N> --end-line <N>]
gbiv fleet send <COLOR> <TEXT>
gbiv install-skill [--scope user|project] [--force]
```

`gbiv fleet status` and `gbiv fleet get` always print JSON to stdout — there is no human-pretty mode. See "Output Format" below for the rationale.

No `gbiv stop` — Ctrl+C in the daemon's terminal is the only stop mechanism in v1 (HLD).
No `gbiv restart` — that's `Ctrl+C` then `gbiv start` again.

### gbiv start

Runs the HTTP Server. Foreground only. See HTTP Server LLD for lifecycle details. Flags:

- `--session-name <NAME>`: override the inferred tmux session name. Mirrors the same flag on `gbiv tmux new-session` for consistency.
- `--bind <ADDR>`: parsed but ignored in v1 (HTTP Server is hard-coded to `127.0.0.1`). Reserved.

Exits non-zero if:
- Not inside a gbiv project
- `tmux -V` fails
- Port can't be bound (another daemon already running, sandbox restriction)

### gbiv fleet status

Prints the JSON response from `GET /sessions` to stdout — one entry per color with `pane_status`, `claude_pane`, the last N lines of pane output, and the truncation/range fields. gbiv does **not** classify the session ("idle", "waiting", "building"); the consuming Claude reads the raw lines and decides.

Flags:
- `--lines <N>` (default 35): forwarded to the API as the `lines` parameter. 35 is enough to capture an `AskUserQuestion` prompt with its options, a recent tool-use exchange, or the tail of a build, without flooding context for a 7-color survey.

Exits 0 if the daemon responded, even if some colors are unhealthy. Exits 2 if the daemon is not running.

### gbiv fleet get

Calls `GET /session/:color` and prints the JSON response to stdout — same `output_truncated`, `output_original_bytes`, `output_returned_bytes`, `range_returned` fields as the API. No human-pretty mode for the same reason as `gbiv fleet status` (single surface, JSON is the contract).

Flags:
- `--lines <N>` (default: API default of 200) — tail mode.
- `--start-line <N>` and `--end-line <N>` — pagination mode. Both required if either is given. Mutually exclusive with `--lines`. Use `top` (literal) for `--start-line` to mean "top of history."

The skill paginates by reading `range_returned.start_line` from the previous response and re-calling with `--start-line=<previous - chunk_size> --end-line=<previous - 1>`.

Exits:
- 0 — pane resolved cleanly
- 2 — daemon not running
- 3 — color is invalid or has no window (HTTP 404)
- 4 — color has no claude pane or multiple (HTTP 200 with non-ok status)

The non-zero-but-not-error codes let the skill branch on the outcome without parsing.

### gbiv fleet send

Calls `POST /session/:color/send` with `{"text": TEXT}`.

The text argument is taken verbatim. Quoting is the user's (or skill's) responsibility:

```
gbiv fleet send red "please run the tests"
gbiv fleet send red "let me know when you're done"
```

The CLI applies the prompt-response guard locally **before** opening a connection — same rule set as the HTTP Server (see http-server LLD § "Prompt-Response Guard"). This gives users (and the skill) immediate feedback without a round trip. The server re-checks the same rule, so a stale CLI cannot bypass the guard.

When the guard fires (locally or via server `409`), the CLI prints the full `explanation` text from the server response (or the equivalent canned text for local rejections) to stderr, not just the short `reason`. The verbose explanation is the whole point — the consumer is typically an LLM, and a terse "rejected: yes/no word" message is exactly the prompt that gets an agent to try `gbiv fleet send red "yes please"` next. The CLI must not summarize or trim it.

Examples that the guard rejects (exit 6):
```
gbiv fleet send red "yes"
gbiv fleet send red "y"
gbiv fleet send red "1"
gbiv fleet send red "?"
```

There is no `--enter-only` flag in v1 — every send appends Enter. If the commander needs to send a literal Tab or Escape, that's a future flag. There is also no `--force` flag to bypass the guard in v1.

Exits:
- 0 — `{ok: true}` from server
- 2 — daemon not running
- 3 — invalid color / no window
- 4 — no claude pane / multiple
- 5 — `SendKeysIncomplete` (502 from server)
- 6 — prompt-response guard rejected the input (local pre-check or server `409 looks_like_prompt_response`)
- 1 — other error

### gbiv install-skill

Writes the bundled skill (`SKILL.md` and any future companion files) into the user's Claude Code skills directory so a Claude Code session can discover and load it. The skill source is `include_str!`'d (or `include_dir!`'d) into the `gbiv` binary at compile time so a `cargo install`'d copy with no nearby source tree still works.

Flags:
- `--scope user` (default): writes to `~/.claude/skills/gbiv-orchestrate/`. Available across every Claude Code project on this machine.
- `--scope project`: writes to `<gbiv-root>/.claude/skills/gbiv-orchestrate/`. Available only inside this gbiv workspace; useful when the user is testing a custom skill or wants per-project pinning.
- `--force`: overwrite without confirmation. Default behavior is to refuse if the destination differs from the bundled content (see "Idempotency" below).

The command prints a JSON result so the skill (or a script) can branch on it:

```json
{
  "scope": "user",
  "destination": "/Users/jane/.claude/skills/gbiv-orchestrate/SKILL.md",
  "action": "installed" | "updated" | "unchanged" | "refused",
  "bundled_version": "0.2.0",
  "previous_version": "0.1.5" | null,
  "reason": "destination differs from bundled content; re-run with --force to overwrite" | null
}
```

#### Idempotency

1. Resolve the destination directory based on `--scope`. For `user`, expand `~`. For `project`, walk up to the gbiv root.
2. If the destination doesn't exist, create it (including parents) and write the bundled files. `action: "installed"`.
3. If the destination exists and the on-disk content matches the bundled content byte-for-byte, do nothing. `action: "unchanged"`.
4. If the destination exists and differs, compare the `version:` line in the existing `SKILL.md` frontmatter against the bundled version:
   - Same version, different content → user has hand-edited; refuse without `--force`. `action: "refused"`.
   - Older version → safe upgrade; overwrite. `action: "updated"`.
   - Newer version → user is on a more recent skill than this binary ships; refuse without `--force`. `action: "refused"`.
5. With `--force`, always overwrite regardless. `action: "updated"`.

The `version:` line is added to the skill frontmatter as part of this design (see orchestrate-skill LLD § "Versioning") so install-skill can reason about updates without parsing the body.

Exits:
- 0 — `installed`, `updated`, or `unchanged`
- 1 — generic write failure (permission denied, etc.)
- 2 — `--scope project` but not inside a gbiv workspace
- 7 — `refused` (custom code so the skill can recognize "user has local edits" without parsing JSON)

## Output Format

Every subcommand emits JSON to stdout. There is no `--json` flag because there is no alternative format. This follows directly from the LLM-first audience (see § Context > Audience): structured output is what the skill needs to branch on field values, not parse prose. `gbiv fleet send` likewise emits a small JSON object (`{"ok": true, "sent_to_pane": "%12"}` or the full guard-rejection body) so callers can match on `ok` and `error` fields directly.

## Port Discovery

All client subcommands need the daemon's port. The lookup:

1. Walk up from CWD to find the gbiv root (reuses `core` module's root-discovery utility).
2. Read `<gbiv-root>/main/<repo>/.gbiv/port`. If missing, exit 2 with `"daemon not running (no port file at <path>); start it with: gbiv start"`.
3. Parse as a `u16`. If malformed, exit 2 with `"port file at <path> is corrupt"`.
4. Open a TCP connection to `127.0.0.1:<port>`. If `ECONNREFUSED`, exit 2 with `"port file present but daemon not responding (stale?); restart with: gbiv start"`.

Stale port files are detected lazily — on the next `gbiv start`, the new daemon overwrites the file with its real port.

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

Each subcommand's `main` returns `anyhow::Result<()>`. Failures bubble up via `?`, get one or more `.context("…")` breadcrumbs along the way (e.g., `.context(format!("GET /session/{color}"))`), and the top-level handler in `main` formats them to stderr in the form:

```
gbiv: <subcommand>: <human-readable message>
       caused by: <next layer>
       caused by: <next layer>
```

The "caused by" chain comes from `anyhow`'s default `{:#}` formatter and is suppressed unless `RUST_LOG` includes `debug` (default behavior keeps the top-level message clean for casual users).

Exit codes are still distinct (see each subcommand) — they're set by translating well-known failure shapes back from the server response or from local validation, not by inspecting the anyhow chain. The chain is for humans/logs; the exit code is for scripts.

Successful JSON output goes to stdout; on error the command writes nothing to stdout and prints the error chain to stderr. This means a script can run `gbiv fleet status` and rely on stdout being either valid JSON or empty.

The CLI itself depends on `anyhow` only; any reusable helpers it shares with the daemon (port file resolution, etc.) live in the `core` module and use `thiserror`-typed errors.

## Logging

gbiv uses a structured logging framework from day one — even though v1 only writes to stderr, having levels and structured fields in place avoids a painful retrofit when file/syslog sinks land later.

### Crate choice

v1 uses the `tracing` + `tracing-subscriber` pair. Rationale:

- De-facto standard in the Rust ecosystem; works equally well for sync (`tiny_http`/`ureq`) and any future async code.
- Levels, structured fields, and spans are available from the start without extra wrapping.
- `tracing-subscriber`'s `EnvFilter` gives per-module level control via `RUST_LOG` syntax with no extra code.
- Lighter alternative `log` + `env_logger` was considered; rejected because it lacks structured fields and spans — switching later would touch every call site.

### Levels

Five standard levels, used consistently across both daemon and CLI:

| Level | When to use |
|---|---|
| `error` | Operation failed in a way the user needs to know about (port bind failure, tmux missing, HTTP 5xx from daemon) |
| `warn` | Recoverable oddity worth surfacing (stale port file, clamped `lines` parameter, multiple claude panes in a window) |
| `info` | One-line summary of a notable event (daemon started, daemon shutdown, port file written, HTTP request handled) |
| `debug` | Per-step detail useful when something is misbehaving (pane resolution attempt, tmux subprocess invocation + args, HTTP request/response shapes) |
| `trace` | Verbose internal state — process tree walk visits, raw tmux output sizes. Off by default, off in `-vv` |

### Output destination

- **All log output goes to stderr.** stdout is reserved for the JSON command result.
- File and syslog sinks are out of scope for v1; the framework choice keeps that door open.

### Configuration

Two ways to set the level, checked in order:

1. **`RUST_LOG` env var** — full `EnvFilter` syntax (e.g., `RUST_LOG=gbiv=debug,tiny_http=warn`). If set, takes precedence over flags. Standard Rust convention; lets power users tune per-module without touching the CLI surface.
2. **`-v` / `-vv` flags** on any subcommand:
   - default: `info`
   - `-v`: `debug`
   - `-vv`: `trace`

A `--quiet` flag is **not** added in v1. The default `info` level is already terse for short-lived client subcommands; the daemon's `info` output is the startup banner plus one line per request, which is what users want when running it in the foreground.

### Format

Default human-readable format (`tracing-subscriber`'s `fmt` layer with defaults):

```
2026-04-28T13:45:01.234Z  INFO gbiv::server: listening on http://127.0.0.1:54321
2026-04-28T13:45:03.118Z DEBUG gbiv::pane_locator: resolving color=red window_id=@7 panes=3
2026-04-28T13:45:03.142Z  INFO gbiv::server: GET /session/red 200 24ms
```

Timestamps are UTC, ISO 8601. Module path included so users can target `RUST_LOG` filters. No JSON log format in v1 — when a log aggregator becomes a real requirement, `tracing-subscriber`'s `json` feature is a one-line change.

### Initialization

Both `gbiv start` (daemon) and the client subcommands install the same subscriber at process start, before any other work. The init helper lives in the `core` module so the daemon and the client subcommands cannot drift on log format.

### What gets logged at `info` (v1 baseline)

Daemon:
- Startup: bind address, port, tmux session name, gbiv root path
- Each accepted HTTP request: method, path, status, duration
- Shutdown: signal received, port file removed (or warn if removal failed)

CLI:
- Resolved port file path (one line, before the request)
- Outbound HTTP target and method
- Final exit code on non-zero exit

Anything more detailed (request bodies, tmux command lines, process tree visits) is `debug` or `trace`.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Single binary, multiple subcommands | Yes | Separate `gbivd` daemon binary | One install, one command for the skill to teach |
| HTTP client | `ureq` | `reqwest` (with sync), raw `std::net` + manual parsing | `ureq` is sync, light, JSON-ready; `reqwest` pulls async dep graph |
| `gbiv fleet status` output format | JSON only | Pretty table; header + raw lines; heuristic-classified state ("idle"/"waiting"/etc.) | LLM-first audience (see § Context > Audience). Classification belongs in the LLM, not in gbiv |
| Default `--lines` for `gbiv fleet status` | 35 | 5; 50; match `gbiv fleet get` (200) | Big enough to capture a prompt-with-options or tool-use exchange; small enough that a 7-color survey doesn't blow context |
| Distinct exit codes for daemon-down vs no-pane | Yes (2, 3, 4, 5) | Always 1 with stderr message | Lets the skill branch programmatically without parsing stderr |
| Send appends Enter implicitly | Yes | Require explicit Enter in text | Most use cases want Enter; explicit flag for "no Enter" can come later if needed |
| Port file path | `<main-worktree>/.gbiv/port` | `~/.gbiv/<hash>.port`, `$XDG_STATE_HOME` | HLD decision: in-workspace; `gbiv start` auto-adds `.gbiv/` to `.git/info/exclude` via `core::ensure_gitignore_entry` so the user never edits gitignore |
| Logging framework | `tracing` + `tracing-subscriber` | `log` + `env_logger`, hand-rolled `eprintln!` | Structured fields and spans available from day one; works for sync now and async later; switching to `log` later would touch every call site |
| Log destination in v1 | stderr only | File sink, syslog, both | Foreground daemon model — stderr is what the user sees; framework keeps file/syslog a one-liner away |
| Skill bundling | `include_str!`/`include_dir!` at compile time; written by `gbiv install-skill` | Manual `cp -r`; downloader that fetches from GitHub at install time; separate package | A `cargo install`'d binary with no source tree must still work. Bundling guarantees the skill matches the binary's CLI surface byte-for-byte |
| Skill update conflict policy | Refuse on hand-edit or newer-on-disk; require `--force` | Always overwrite; never overwrite | Users sometimes tweak their skills; clobbering silently is hostile. Refusing with a clear reason is safe |

## Edge Cases

| Case | Behavior |
|---|---|
| Run any client subcommand outside a gbiv project | Exit 2 with "not inside a gbiv project" |
| Daemon running in workspace A, CLI run in workspace B | Each workspace has its own port file; B has none → exit 2. No cross-workspace confusion |
| Daemon's tmux session was killed mid-session | `gbiv fleet status` returns 200 with all colors `no_window` (since `list_windows` will fail to find the session, daemon returns 503; CLI surfaces "tmux session not found"). v1 does not auto-recover |
| `gbiv fleet send red ""` | Local validation: exit 1 with "text must not be empty" before opening a connection |
| `gbiv fleet send red <very long text>` | Forwarded as-is; HTTP server enforces any size limits, not the CLI |
| Two `gbiv fleet send red` racing | Both fire HTTP requests; daemon serializes (HTTP Server: thread-per-request, tmux serializes keys per-pane) |
| `--lines` combined with `--start-line`/`--end-line` on `gbiv fleet get` | clap rejects with a usage error before any HTTP call |
| User invokes `gbiv start` while a daemon is already running | TCP bind fails; exit 1 with "another gbiv daemon may be running on port <N>; check the port file or run `lsof -i:<N>`" |

## Technical Debt & Future Work

1. **No `gbiv stop`.** Foreground-only is the v1 contract. If `--detach` ever ships, `stop` ships with it.
2. **No `--bind` honored.** Reserved flag. Implementing requires HTTP Server changes too.
3. **No server-side summarization.** Raw lines mean a fleet survey carries N×lines bytes back. If that becomes a context problem, a Haiku/Sonnet summarizer at the HTTP layer (HLD evolution vector) is the right move — not a regex.
4. **No completion scripts.** clap can generate them; left for later.
5. **Logging sinks are stderr-only.** File rotation, syslog, and JSON formatting are deferred — `tracing-subscriber` makes any of these a small change when needed.

## References

- HLD: `docs/high-level-design.md` § Components > gbiv CLI
- Companion: `docs/llds/http-server.md` (server side of the HTTP)
- Companion: `docs/llds/orchestrate-skill.md` (primary consumer)
