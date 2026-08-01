# HTTP Server

Specs for the orchestration HTTP Server — the `gbiv start` daemon's inbound
surface, its port-file discovery convention, and the prompt-response guard on
sends. Implemented in the `gbiv` binary's `orchestration::http_server` module
(daemon lifecycle + routing) plus the `start` CLI subcommand, built on the
`orchestration::pane_locator` and `orchestration::tmux_driver` modules and the
shared `gbiv_core` primitives (`root`, `tmux`, `gitignore`, `palette`,
`observability`).

**Component LLD**: `docs/llds/http-server.md`

## Startup & Discovery

- [x] **HTTP-SRV-001**: When `gbiv start` runs, the daemon shall discover the gbiv root from the current working directory via `core::find_gbiv_root`; if none is found, the process shall exit non-zero with a message stating it is not inside a gbiv project.
- [x] **HTTP-SRV-002**: The daemon shall resolve the main worktree's repo directory via `core::find_repo_in_worktree(<gbiv-root>/main)`.
- [x] **HTTP-SRV-003**: The daemon shall resolve the tmux session name via `core::tmux::session_name_for_root(folder_name)` unless a `--session-name` value was supplied, in which case the supplied value is used verbatim (identical resolution order to `gbiv tmux new-session`/`gbiv tmux sync`).
- [x] **HTTP-SRV-004**: The daemon shall load the active palette once at startup via `gbiv_core::palette::Palette::load(&gbiv_root)` and hold it for the process lifetime; a malformed `.gbiv/config.toml` shall cause the process to exit non-zero with the underlying `ConfigError`.
- [x] **HTTP-SRV-005**: The daemon shall verify tmux is available via `core::tmux::tmux_available()` before binding; if it returns `Err`, the process shall exit non-zero with a message describing the failure.
- [x] **HTTP-SRV-006**: The daemon shall bind a TCP listener on `127.0.0.1:0` (kernel-assigned ephemeral port); if the bind fails (e.g. another daemon already running in this workspace), the process shall exit non-zero with a message naming the likely cause.
- [x] **HTTP-SRV-007**: The daemon shall create `<gbiv-root>/main/<repo>/.gbiv/` if it does not already exist.
- [x] **HTTP-SRV-008**: The daemon shall write the bound port to `<gbiv-root>/main/<repo>/.gbiv/port` as plain ASCII decimal followed by a newline (e.g. `54321\n`), overwriting any existing content.
- [x] **HTTP-SRV-009**: The daemon shall ensure `.gbiv/` is present in `.git/info/exclude` via `core::gitignore::ensure_gitignore_entry`, idempotently (a second `gbiv start` does not duplicate the entry).
- [x] **HTTP-SRV-010**: On successful bind, the daemon shall print `gbiv listening on http://127.0.0.1:<port>` to stdout and log an equivalent `info`-level line (host, port, tmux session name, gbiv root path).
- [x] **HTTP-SRV-011**: After startup, the daemon shall block in a request-accept loop until a shutdown signal is received.

## Shutdown

- [x] **HTTP-SRV-012**: On SIGINT or SIGTERM (registered via the `ctrlc` crate), the daemon shall attempt to delete the port file and then exit with status 0, regardless of whether the deletion succeeded.
- [x] **HTTP-SRV-013**: If port-file deletion fails during shutdown (e.g. permission error, file already gone), the daemon shall log a `warn` and proceed with process exit rather than panicking or hanging.

## Concurrency

- [x] **HTTP-SRV-014**: The daemon shall run exactly 16 long-lived worker threads, each looping a call to `tiny_http::Server::recv()` on the one shared `Server` instance, so at most 16 requests are handled concurrently.

## Binding & Security

- [x] **HTTP-SRV-015**: The daemon shall bind only to `127.0.0.1`; it shall never bind `0.0.0.0` regardless of any `--bind` value supplied (see HTTP-SRV-058).
- [x] **HTTP-SRV-016**: The daemon shall not require or check any authentication credential on any request — the loopback interface is the v1 trust boundary.

## Active Palette & Color Validation

- [x] **HTTP-SRV-017**: When a route contains a `:color` path parameter, the daemon shall lowercase it and strip any trailing `/` before validation (`RED/` resolves to `red`).
- [x] **HTTP-SRV-018**: The daemon shall validate a normalized `:color` against the palette loaded at startup (HTTP-SRV-004) via `Palette::contains`, accepting base ROYGBIV colors and any configured `.gbiv/config.toml` extras alike.
- [x] **HTTP-SRV-019**: If a normalized `:color` is not in the active palette, the daemon shall respond `404` with a JSON error body before any Pane Locator call is made.
- [x] **HTTP-SRV-020**: `GET /sessions` shall iterate the active palette's colors in `Palette::names()` order (base ROYGBIV first, then extras in declared order).

## GET /sessions

- [x] **HTTP-SRV-021**: `GET /sessions` shall accept an optional `lines` query parameter (default `35`, max `1000`), clamped to the max before being passed to the driver as `CaptureRange::Tail { lines }`.
- [x] **HTTP-SRV-022**: If `lines` is present and not a valid non-negative integer, `GET /sessions` shall respond `400`.
- [x] **HTTP-SRV-023**: `GET /sessions` shall resolve all colors in one call to the batch `pane_locator::locate_panes(session, colors)` (sharing one host process scan and window list across every color, per PANE-LOC-024) rather than looping per-color `locate_pane` calls; for each color whose resolution is `Resolution::Ok`, the handler shall capture the tail via the tmux Driver and include `pane_status: "ok"`, `claude_pane`, `output`, `captured_at` (UTC ISO-8601), and — when `other_pane_ids` is non-empty — `other_claude_panes`.
- [x] **HTTP-SRV-024**: When a color's resolution is `Resolution::NoWindow`, its entry shall have `pane_status: "no_window"`, and `tmux_window`, `claude_pane`, `output`, and `captured_at` all `null`.
- [x] **HTTP-SRV-025**: When a color's resolution is `Resolution::NoClaudePane`, its entry shall have `pane_status: "no_claude_pane"`, `tmux_window` set to the color name (the window exists), and `claude_pane`, `output`, `captured_at` all `null`.
- [x] **HTTP-SRV-026**: `GET /sessions` shall respond `200` regardless of individual colors' `pane_status` values — a partial-failure survey is not itself an error.
- [x] **HTTP-SRV-027**: If the shared tmux session itself does not exist (`locate_panes`'s single window-listing call fails with `LocatorError::TmuxSession(TmuxError::SessionNotFound(_))`, per PANE-LOC-025), `GET /sessions` shall respond `503` for the whole request rather than a per-color status.

## GET /session/:color

- [x] **HTTP-SRV-028**: `GET /session/:color` shall accept an optional `lines` query parameter (default `200`, max `5000`) in tail mode, mapping to `CaptureRange::Tail { lines }`, clamped to the max before being passed to the driver.
- [x] **HTTP-SRV-029**: `GET /session/:color` shall accept an optional `start_line` + `end_line` pair (both required together) in window mode, mapping to `CaptureRange::Window { start, end }`; the literal string `top` for `start_line` maps to `i32::MIN`.
- [x] **HTTP-SRV-030**: If exactly one of `start_line`/`end_line` is supplied without the other, `GET /session/:color` shall respond `400`.
- [x] **HTTP-SRV-031**: If `lines` is supplied together with `start_line` or `end_line`, `GET /session/:color` shall respond `400`.
- [x] **HTTP-SRV-032**: If `lines`, `start_line`, or `end_line` is present and not a valid integer in its expected form, `GET /session/:color` shall respond `400`.
- [x] **HTTP-SRV-033**: On success, `GET /session/:color`'s response body shall include `color`, `claude_pane`, `pane_status`, `captured_at`, `output`, `output_truncated`, `output_original_bytes`, `output_returned_bytes`, and `range_returned: { start_line, end_line }`, sourced from the tmux Driver's `Capture` verbatim (the HTTP layer does not re-truncate).
- [x] **HTTP-SRV-034**: `GET /session/:color` shall respond `200` when `pane_status` is `"ok"` (including the multi-claude-pane auto-pick case, whose body includes `other_claude_panes`) or `"no_claude_pane"`.
- [x] **HTTP-SRV-035**: `GET /session/:color` shall respond `404` when `:color` fails active-palette validation (HTTP-SRV-019) or when its resolution is `Resolution::NoWindow`.
- [x] **HTTP-SRV-036**: `GET /session/:color` shall resolve its color via the single-color `pane_locator::locate_pane(session, color)` and shall respond `503` when that call fails with `LocatorError::TmuxSession(TmuxError::SessionNotFound(_))`.

## POST /session/:color/send

Validation proceeds in a fixed order, so that when more than one thing is
wrong with a request, exactly one deterministic status code results: (1)
`:color` route validation, (2) body/text validation, (3) prompt-response
guard, (4) pane resolution, (5) send. This mirrors the GET endpoints' existing
"validated at the routing layer before the locator is called" precedent — the
guard runs after route/body validation but always before any Pane Locator or
tmux Driver call.

- [x] **HTTP-SRV-037**: `POST /session/:color/send` shall validate the normalized `:color` against the active palette (HTTP-SRV-017, HTTP-SRV-018) **before** parsing the request body; an unrecognized color shall respond `404` without reading or validating the body.
- [x] **HTTP-SRV-038**: `POST /session/:color/send` shall require a JSON body with a string `text` field; if the body is not valid JSON, the daemon shall respond `400`.
- [x] **HTTP-SRV-039**: If `text` is missing, or is empty or all-whitespace after trimming, `POST /session/:color/send` shall respond `400` without evaluating any guard rule.
- [x] **HTTP-SRV-040**: `POST /session/:color/send` shall run the prompt-response guard (HTTP-SRV-041 through HTTP-SRV-045) against the trimmed `text` after route (HTTP-SRV-037) and body (HTTP-SRV-038/039) validation pass, and before any Pane Locator or tmux Driver call.
- [x] **HTTP-SRV-041**: If the trimmed `text` matches `^[yn]$` (case-insensitive), the guard shall reject with `reason: "single-letter yes/no"`.
- [x] **HTTP-SRV-042**: If the trimmed `text` matches `^(yes|no)$` (case-insensitive), the guard shall reject with `reason: "yes/no word"`.
- [x] **HTTP-SRV-043**: If the trimmed `text` matches `^\d{1,3}$`, the guard shall reject with `reason: "numeric choice"`.
- [x] **HTTP-SRV-044**: If the trimmed `text` is exactly one non-alphanumeric character, the guard shall reject with `reason: "bare punctuation"`.
- [x] **HTTP-SRV-045**: When the guard rejects, `POST /session/:color/send` shall respond `409` with a JSON body containing `ok: false`, `error: "looks_like_prompt_response"`, the matching `reason`, `color`, a verbose `explanation` string, and a `docs` pointer, and it shall not call the Pane Locator or tmux Driver.
- [x] **HTTP-SRV-046**: Multi-word natural-language `text` (e.g. `"yes please run that"`) shall not match any guard rule and shall pass through to pane resolution.
- [x] **HTTP-SRV-047**: After the guard passes, the daemon shall resolve the pane via the single-color `pane_locator::locate_pane(session, color)`; a `Resolution::NoWindow` result shall respond `404` (covering the case where the color is palette-valid but its tmux window doesn't exist yet).
- [x] **HTTP-SRV-048**: If pane resolution yields `Resolution::NoClaudePane`, `POST /session/:color/send` shall respond `409` with a JSON body `{"ok": false, "error": "no_claude_pane", "color": "<color>"}` and shall not call the tmux Driver.
- [x] **HTTP-SRV-049**: If pane resolution yields `Resolution::Ok` with a non-empty `other_pane_ids`, `POST /session/:color/send` shall proceed to send to the auto-picked (oldest) pane and respond `200`, including `other_claude_panes` in the success body — multiple claude panes is not itself a `409`.
- [x] **HTTP-SRV-050**: On a successful `tmux_driver::send_keys` call, `POST /session/:color/send` shall respond `200` with `{"ok": true, "sent_to_pane": "<pane_id>"}`.
- [x] **HTTP-SRV-051**: If `tmux_driver::send_keys` returns `TmuxError::SendKeysIncomplete`, `POST /session/:color/send` shall respond `502`.
- [x] **HTTP-SRV-052**: If pane resolution or send fails with `TmuxError::SessionNotFound` (via `LocatorError::TmuxSession` or directly), `POST /session/:color/send` shall respond `503`.

## Error Handling

- [x] **HTTP-SRV-053**: A handler-level `TmuxError::SessionNotFound` (bare or wrapped in `LocatorError::TmuxSession`) shall map to HTTP `503`.
- [x] **HTTP-SRV-054**: A handler-level `TmuxError::PaneNotFound` shall map to HTTP `404`.
- [x] **HTTP-SRV-055**: A handler-level `TmuxError::SendKeysIncomplete` shall map to HTTP `502`.
- [x] **HTTP-SRV-056**: A handler-level `TmuxError::Other`, or any other unclassified error, shall map to HTTP `500`.

## `gbiv start` CLI

- [x] **HTTP-SRV-057**: `gbiv start` shall accept an optional `--session-name <NAME>` flag with identical resolution semantics to `gbiv tmux new-session --session-name` / `gbiv tmux sync --session-name` (HTTP-SRV-003).
- [x] **HTTP-SRV-058**: `gbiv start` shall accept an optional `--bind <ADDR>` flag which is parsed but ignored in v1 — the daemon always binds `127.0.0.1` regardless of its value (HTTP-SRV-015).
- [x] **HTTP-SRV-059**: `gbiv start` shall run in the foreground and never daemonize or fork into the background.

## References

- LLD: `docs/llds/http-server.md`
- Companion: `docs/specs/pane-locator.md`, `docs/specs/tmux-driver.md`
- Companion: `docs/llds/orchestrate-cli.md` (the `gbiv fleet` HTTP *client* subcommands — a separate, still-unwritten segment that depends on this one)
