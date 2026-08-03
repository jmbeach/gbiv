# Orchestrate CLI — `gbiv fleet` client commands

Specs for the fleet orchestration **client** subcommands — `gbiv fleet status`,
`gbiv fleet get <color>`, and `gbiv fleet send <color> <text>` — thin HTTP
clients over the `gbiv start` daemon (`docs/specs/http-server.md`). Scoped to
this slice only: `gbiv start` itself is specced in `docs/specs/http-server.md`
(HTTP-SRV-057 through HTTP-SRV-059). `gbiv install-skill` is a distinct
segment of the same LLD (a filesystem installer, not an HTTP client) and gets
its own ID prefix, `INSTALL-CLI-*`, in the section below. The bundled
`SKILL.md`'s own content/frontmatter requirements are specced separately in
`docs/specs/orchestrate-skill.md`.

**Component LLD**: `docs/llds/orchestrate-cli.md`

## Command Surface

- [x] **FLEET-CLI-001**: The system shall provide a `gbiv fleet` subcommand group containing `status`, `get`, and `send` subcommands.
- [x] **FLEET-CLI-002**: `gbiv fleet status` shall accept an optional `--lines <N>` flag (default `35`), forwarded verbatim as the `lines` query parameter on `GET /sessions`.
- [x] **FLEET-CLI-003**: `gbiv fleet get <COLOR>` shall accept an optional `--lines <N>` flag in tail mode, forwarded verbatim as the `lines` query parameter on `GET /session/:color`; when omitted, no `lines` parameter is sent and the server's own default (HTTP-SRV-028) applies.
- [x] **FLEET-CLI-004**: `gbiv fleet get <COLOR>` shall accept optional `--start-line <N>` and `--end-line <N>` flags for window mode, forwarded verbatim as the `start_line`/`end_line` query parameters; the literal value `top` for `--start-line` shall be forwarded as the literal string `top` (HTTP-SRV-029 maps it to `i32::MIN` server-side).
- [x] **FLEET-CLI-005**: If `gbiv fleet get` is invoked with `--lines` together with either `--start-line` or `--end-line`, the system shall reject the invocation with a clap usage error before any HTTP call is made.
- [x] **FLEET-CLI-006**: If `gbiv fleet get` is invoked with exactly one of `--start-line`/`--end-line`, the system shall reject the invocation with a clap usage error before any HTTP call is made.
- [x] **FLEET-CLI-007**: `gbiv fleet send <COLOR> <TEXT>` shall take `TEXT` as a single positional argument and forward it verbatim (before the trimming in FLEET-CLI-039) as the JSON body's `text` field.

## Port Discovery & HTTP Client

- [x] **FLEET-CLI-010**: Before issuing any HTTP request, every `gbiv fleet` subcommand shall resolve the gbiv root from the current working directory via `core::find_gbiv_root`; if none is found, the command shall exit `2` with a message stating it is not inside a gbiv project.
- [x] **FLEET-CLI-011**: The system shall resolve the daemon's port by reading `<gbiv-root>/main/<repo>/.gbiv/port` (`<repo>` located via `core::find_repo_in_worktree`); if the file does not exist, the command shall exit `2` with a message naming the expected path and suggesting `gbiv start`.
- [x] **FLEET-CLI-012**: If the port file's trimmed content does not parse as a `u16`, the command shall exit `2` with a message stating the port file is corrupt.
- [x] **FLEET-CLI-013**: The system shall issue exactly one HTTP request per subcommand invocation to `127.0.0.1:<port>`, with a 1-second connect timeout and a 30-second read timeout.
- [x] **FLEET-CLI-014**: If the connection is refused, or the connect/read timeout elapses before a response is received, the command shall exit `2` with a message stating the daemon is not responding and suggesting `gbiv start` — the same exit code as FLEET-CLI-011/012 (a stale port file and a missing one both mean "no reachable daemon" to the caller), with a message that distinguishes the two cases for a human reading stderr.
- [x] **FLEET-CLI-015**: If a response is received but is not valid JSON, or does not match the expected shape for that endpoint (e.g. an unrelated local process is listening on the recorded port), the command shall exit `1` with a message describing the unexpected response — distinct from FLEET-CLI-014, since a response was in fact received.

## `gbiv fleet status`

- [x] **FLEET-CLI-020**: On a `200` response from `GET /sessions`, `gbiv fleet status` shall print the response body verbatim as JSON to stdout and exit `0`, regardless of individual colors' `pane_status` values, including `"error"` (HTTP-SRV-065) — a partial-failure survey is not itself a command error (mirrors HTTP-SRV-026).
- [x] **FLEET-CLI-021**: If `GET /sessions` responds `503` (the tmux session itself was not found, HTTP-SRV-027), `gbiv fleet status` shall print the server's error message to stderr and exit `1`.

## `gbiv fleet get`

- [x] **FLEET-CLI-030**: On a `200` response with `pane_status: "ok"`, `gbiv fleet get` shall print the response body verbatim as JSON to stdout and exit `0` — including when `other_claude_panes` is present (HTTP-SRV-034's multi-pane auto-pick case is not an error state).
- [x] **FLEET-CLI-031**: On a `200` response with `pane_status: "no_claude_pane"`, `gbiv fleet get` shall print the response body verbatim as JSON to stdout and exit `4`.
- [x] **FLEET-CLI-032**: On a `404` response (invalid `:color` or no tmux window, HTTP-SRV-035), `gbiv fleet get` shall print the server's error message to stderr and exit `3`.
- [x] **FLEET-CLI-033**: On a `400` response (a malformed query combination that reached the server, e.g. HTTP-SRV-064's `start_line` after `end_line`), `gbiv fleet get` shall print the server's error message to stderr and exit `1`.
- [x] **FLEET-CLI-034**: On a `500` or `503` response, `gbiv fleet get` shall print the server's error message to stderr and exit `1`.

## `gbiv fleet send`

Local validation runs in the order below, before any HTTP request is issued —
mirroring the server's own fixed validation order (http-server.md § `POST
/session/:color/send`: color, then body/text, then guard, then pane
resolution, then send) exactly, so a rejection is reported to the user as fast
and cheaply as possible without a network round trip, and so `send`'s local
checks resolve compound-invalid input (e.g. an unknown color paired with
guard-shaped text) the same way the server would.

- [x] **FLEET-CLI-038**: Before any other local check, `gbiv fleet send <COLOR> <TEXT>` shall validate `COLOR` against the active palette, loaded via `core::palette::Palette::load` from the gbiv root already resolved in FLEET-CLI-010; if `COLOR` (after the same lowercase-and-strip-trailing-slash normalization as HTTP-SRV-017) is not in the active palette, the command shall exit `3` with a message stating the color is unknown, without opening a connection.
- [x] **FLEET-CLI-039**: If `COLOR` passes FLEET-CLI-038, the command shall trim `TEXT`; if the trimmed text is empty, the command shall exit `1` with a message stating text must not be empty, without opening a connection.
- [x] **FLEET-CLI-040**: If the trimmed text (FLEET-CLI-039) is non-empty, the command shall evaluate it against `orchestration::http_server::guard_check` (the same function `POST /session/:color/send`'s handler calls — not a reimplementation) before opening a connection.
- [x] **FLEET-CLI-041**: If the local guard check (FLEET-CLI-040) rejects the text, the command shall print the rejection's full `guard_explanation` text — built with the normalized color from FLEET-CLI-038, matching the casing a server-echoed rejection would use — to stderr and exit `6`, without opening a connection.
- [x] **FLEET-CLI-042**: If the local guard check passes, the command shall `POST` a JSON body `{"text": "<trimmed text>"}` to `/session/<normalized color>/send`.
- [x] **FLEET-CLI-044**: On a `200` response, `gbiv fleet send` shall print the response body verbatim as JSON to stdout and exit `0` — including when `other_claude_panes` is present (HTTP-SRV-049's multi-pane case is not an error state).
- [x] **FLEET-CLI-045**: On a `404` response (the color passed the local check in FLEET-CLI-038 but the server rejects it anyway — e.g. the daemon's palette changed after `send`'s local `Palette::load`, or no tmux window exists for the color yet), the command shall print the server's error message to stderr and exit `3`, identically to a locally-caught invalid color.
- [x] **FLEET-CLI-046**: On a `409` response with `error: "no_claude_pane"`, the command shall print the response body to stderr and exit `4`.
- [x] **FLEET-CLI-047**: On a `409` response with `error: "looks_like_prompt_response"` (a guard rejection the local pre-check did not catch — e.g. a stale CLI binary talking to a newer daemon whose guard rule set changed), the command shall print the full `explanation` field from the response body to stderr and exit `6`, identically to a local rejection (FLEET-CLI-042).
- [x] **FLEET-CLI-048**: On a `502` response (`SendKeysIncomplete`), the command shall print the server's error message to stderr and exit `5`.
- [x] **FLEET-CLI-049**: On a `500` or `503` response, the command shall print the server's error message to stderr and exit `1`.

## Output & Logging

- [x] **FLEET-CLI-050**: Every `gbiv fleet` subcommand shall write nothing to stdout on any non-zero exit — a failure's only stdout-visible trace is the process exit code; all failure detail goes to stderr.
- [x] **FLEET-CLI-051**: Each `gbiv fleet` subcommand shall log the resolved port file path at `info` level before issuing its HTTP request.
- [x] **FLEET-CLI-052**: Each `gbiv fleet` subcommand shall log the outbound HTTP method and target URL at `info` level before issuing its request.
- [x] **FLEET-CLI-053**: Each `gbiv fleet` subcommand shall log its final exit code at `info` level immediately before exiting with a non-zero code.

## `gbiv install-skill`

Writes the bundled `gbiv-orchestrate` skill to disk. Unlike the `fleet`
subcommands, this is pure filesystem I/O — no daemon, no port file, no HTTP
call.

### Command Surface

- [x] **INSTALL-CLI-001**: The system shall provide a top-level `gbiv install-skill` subcommand accepting an optional `--scope <user|project>` flag (default `user`) and an optional `--force` boolean flag.
- [x] **INSTALL-CLI-002**: The bundled skill content (`SKILL.md`, byte-for-byte) shall be embedded into the `gbiv` binary at compile time via `include_str!`, so a `cargo install`'d binary with no nearby source tree can still install it.

### Destination Resolution

- [x] **INSTALL-CLI-010**: With `--scope user` (or no `--scope` flag), the destination directory shall be `<home>/.claude/skills/gbiv-orchestrate/`, where `<home>` is read from the `HOME` environment variable; if `HOME` is unset or empty, the command shall exit `1` with a message stating `HOME` could not be resolved, without touching the filesystem.
- [x] **INSTALL-CLI-011**: With `--scope project`, the system shall resolve the gbiv root from the current working directory via `core::find_gbiv_root`; if none is found, the command shall exit `2` with a message stating it is not inside a gbiv workspace, without touching the filesystem.
- [x] **INSTALL-CLI-012**: With `--scope project`, the destination directory shall be `<gbiv-root>/.claude/skills/gbiv-orchestrate/`.
- [x] **INSTALL-CLI-013**: The destination file within the resolved directory shall be named `SKILL.md`.

### Idempotency Decision

Applies uniformly to both scopes once the destination directory (INSTALL-CLI-010/012) is resolved.

- [x] **INSTALL-CLI-020**: If the destination `SKILL.md` does not exist, the system shall create the destination directory (including parents) if needed, write the bundled content, and report `action: "installed"` — regardless of whether `--force` was given.
- [x] **INSTALL-CLI-021**: If the destination `SKILL.md` exists and its content is byte-for-byte identical to the bundled content, and `--force` was not given, the system shall write nothing and report `action: "unchanged"`.
- [x] **INSTALL-CLI-022**: If the destination `SKILL.md` exists and its content is byte-for-byte identical to the bundled content, and `--force` was given, the system shall overwrite it (a no-op write) and report `action: "updated"`.
- [x] **INSTALL-CLI-023**: If the destination `SKILL.md` exists, its content differs from the bundled content, and `--force` was given, the system shall overwrite it and report `action: "updated"`, without comparing versions.
- [x] **INSTALL-CLI-024**: If the destination `SKILL.md` exists, its content differs from the bundled content, and `--force` was not given, the system shall parse the `version:` field from the existing file's YAML frontmatter and compare it against the bundled version (`CARGO_PKG_VERSION`) using dot-separated numeric segment comparison.
- [x] **INSTALL-CLI-025**: Under INSTALL-CLI-024, if the existing file's version equals the bundled version, the system shall write nothing and report `action: "refused"` with `reason: "destination differs from bundled content; re-run with --force to overwrite"` — the same version with different content means a hand-edit.
- [x] **INSTALL-CLI-026**: Under INSTALL-CLI-024, if the existing file's version is lower (by INSTALL-CLI-024's comparison) than the bundled version, the system shall overwrite it and report `action: "updated"`.
- [x] **INSTALL-CLI-027**: Under INSTALL-CLI-024, if the existing file's version is higher than the bundled version, the system shall write nothing and report `action: "refused"` with `reason: "on-disk skill (version <existing>) is newer than this binary ships (version <bundled>); re-run with --force to overwrite"`.
- [x] **INSTALL-CLI-028**: Under INSTALL-CLI-024, if the existing file has no parseable `version:` field in its frontmatter, the system shall treat it the same as INSTALL-CLI-025 — write nothing and report `action: "refused"` with `reason: "destination has no parseable version; re-run with --force to overwrite"` — since an unversioned or malformed file cannot be distinguished from a hand-edit.

### Output

- [x] **INSTALL-CLI-030**: On `action` values `"installed"`, `"updated"`, or `"unchanged"` (exit `0`), the system shall print a JSON object to stdout with fields `scope`, `destination` (absolute path), `action`, `bundled_version`, `previous_version` (the existing file's parsed version, or `null` if the file did not exist or had no parseable version), and `reason: null`.
- [x] **INSTALL-CLI-031**: On `action: "refused"` (exit `7`), the system shall print the same JSON object shape as INSTALL-CLI-030 to stdout (not stderr) with `reason` populated — the refusal is an expected decision outcome the caller branches on by parsing `action`/`reason`, not a hard failure.
- [x] **INSTALL-CLI-032**: A hard failure not covered by the idempotency decision (INSTALL-CLI-010's unresolved `HOME`, INSTALL-CLI-011's "not a gbiv workspace", or a filesystem write error such as permission denied) shall print a plain-text message to stderr, print nothing to stdout, and exit non-zero — no JSON envelope, since the decision logic never ran.

### Exit Codes

- [x] **INSTALL-CLI-040**: The command shall exit `0` for `action` values `"installed"`, `"updated"`, and `"unchanged"`.
- [x] **INSTALL-CLI-041**: The command shall exit `1` for a generic write failure (permission denied, etc.) or an unresolved `HOME` (INSTALL-CLI-010).
- [x] **INSTALL-CLI-042**: The command shall exit `2` when `--scope project` is given outside a gbiv workspace (INSTALL-CLI-011).
- [x] **INSTALL-CLI-043**: The command shall exit `7` for `action: "refused"`.

### Logging

- [x] **INSTALL-CLI-050**: The system shall log the resolved destination path at `info` level before performing the idempotency decision.
- [x] **INSTALL-CLI-051**: The system shall log the final `action` and exit code at `info` level immediately before exiting.

## References

- LLD: `docs/llds/orchestrate-cli.md`
- Companion: `docs/specs/http-server.md` (server side of the HTTP contract; `gbiv start` itself)
- Companion: `docs/specs/orchestrate-skill.md` (the bundled `SKILL.md`'s own content requirements)
