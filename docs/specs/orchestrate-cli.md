# Orchestrate CLI — `gbiv fleet` client commands

Specs for the fleet orchestration **client** subcommands — `gbiv fleet status`,
`gbiv fleet get <color>`, and `gbiv fleet send <color> <text>` — thin HTTP
clients over the `gbiv start` daemon (`docs/specs/http-server.md`). Scoped to
this slice only: `gbiv start` itself is specced in `docs/specs/http-server.md`
(HTTP-SRV-057 through HTTP-SRV-059); `gbiv install-skill` is a separate,
not-yet-specced segment of the same LLD.

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

## References

- LLD: `docs/llds/orchestrate-cli.md`
- Companion: `docs/specs/http-server.md` (server side of the HTTP contract; `gbiv start` itself)
