# Extension Runtime

Specs for gbiv's Lua extension runtime: discovery and loading, the `[extensions]`
config, the four `start`/`done` events, the sandbox, the `api_version` 1 host
API, and `gbiv install-extension`. The commands that fire the events
(`gbiv start` / `gbiv done`) are specced in `docs/specs/feature-ledger.md`
(`FL-START-*`, `FL-DONE-*`); config layering in general is specced in
`docs/specs/cli-and-palette.md` (`CLI-CFG-*`). The bundled vat extension has its
own file, `docs/specs/vat-extension.md`.

**Component LLD**: `docs/llds/extension-runtime.md`

## Engine

- [ ] **EXT-ENG-001**: The system shall run extensions on Lua 5.4 embedded through `mlua` with its Lua source vendored into the gbiv binary, requiring no system Lua installation.
- [ ] **EXT-ENG-002**: When a command loads extensions, the system shall create a separate Lua state for each enabled extension, so no extension can read or write another extension's globals.
- [ ] **EXT-ENG-003**: The system shall create extension Lua states per command invocation and keep no extension state between invocations.

## Discovery and Loading

- [ ] **EXT-LOAD-001**: When loading an enabled extension named `<name>`, the system shall look for the directory `<gbiv-root>/.gbiv/extensions/<name>/` first and then `<user-config-dir>/gbiv/extensions/<name>/` (`$XDG_CONFIG_HOME` if set, else `~/.config`), and use the first that exists.
- [ ] **EXT-LOAD-002**: Extension directory names shall be matched to extension names byte for byte (case-sensitively).
- [ ] **EXT-LOAD-003**: If an enabled extension's directory exists in neither location, then the command shall exit `1` before changing anything, with a message naming the extension, both searched paths, and `gbiv install-extension <name>`.
- [ ] **EXT-LOAD-004**: If an extension directory exists but its `extension.toml` is missing or invalid, then the command shall exit `1` naming the file, and shall not fall back to the other location's copy.
- [ ] **EXT-LOAD-005**: The manifest shall require `name` (equal to the directory name and matching `[A-Za-z0-9_-]+`), `version`, and an integer `api_version`; `description` and `capabilities` (a subset of `fs`, `git`, `http`, `env`) shall be optional.
- [ ] **EXT-LOAD-006**: If a manifest's `api_version` is not one this gbiv supports (in this version, only `1`), then loading shall fail with exit `1` and a message naming the extension, its `api_version`, and the supported versions.
- [ ] **EXT-LOAD-007**: When an extension's state is created, the system shall run its `init.lua` and require it to return a table whose keys are a subset of `before_start`, `after_start`, `before_done` and `after_done`, each a function; any other return value or key shall fail loading with exit `1`.
- [ ] **EXT-LOAD-008**: The system shall load every enabled extension before firing any event, so a load failure aborts the command before any extension handler runs.
- [ ] **EXT-LOAD-009**: Only `gbiv start` and `gbiv done` shall load extensions; apart from them, only `gbiv install-extension` (which edits the config, EXT-INST-007) shall read the `[extensions]` config, and every other command shall run without either.
- [ ] **EXT-LOAD-010**: When an extension calls `require(name)`, the system shall load `<extension-dir>/<name with dots as slashes>.lua` as Lua source text, and shall raise a Lua error if the resolved file lies outside the extension directory (including through `..` or a symlink) or does not exist.

## Configuration

- [ ] **EXT-CFG-001**: The effective list of extensions shall be the global config's `[extensions] enabled` names, followed by the project config's `enabled` names not already present, minus every name listed in either file's `[extensions] disabled`; this list is also the load and handler run order.
- [ ] **EXT-CFG-002**: When both config files contain a per-extension table `[extensions.<name>]`, the system shall merge them key by key with the project file's value winning for the same key, and shall replace list values whole rather than merging their elements.
- [ ] **EXT-CFG-003**: If `[extensions] enabled` or `disabled` is not an array of strings, or names the same extension twice within one list, then the command shall exit `1` with a config error naming the file.
- [ ] **EXT-CFG-004**: If `[extensions]` contains a key other than `enabled` or `disabled` whose value is not a table, then the command shall exit `1` with a config error naming the key and file.
- [ ] **EXT-CFG-005**: The system shall treat `paths`, `http` and `env` as reserved keys of a per-extension table and pass every other key of that table to the extension as `ctx.options`, converted from TOML to Lua values.
- [ ] **EXT-CFG-006**: When resolving a granted path from either config file, the system shall expand a leading `~/` to the home directory and resolve a relative path against the main worktree's repository directory (`<gbiv-root>/main/<repo>/`).
- [ ] **EXT-CFG-007**: The system shall ignore a per-extension table for an extension that is not in the effective list.
- [ ] **EXT-CFG-008**: When the config grants an extension a capability (`paths`, `http` or `env`) that its manifest does not declare, the system shall print a warning naming the extension and capability and continue.

## Events

- [ ] **EXT-EVT-001**: When an event fires, the system shall call each loaded extension's handler for that event in effective-list order, skipping extensions that define no handler for it.
- [ ] **EXT-EVT-002**: The system shall pass `before_start` handlers a `ctx` table with `arg` (the `gbiv start` argument exactly as typed), `color`, `text` (`nil` until some handler names the entry, then the named text) and `options`.
- [ ] **EXT-EVT-003**: The system shall pass `after_start`, `before_done` and `after_done` handlers a `ctx` table with `color`, `text` (the entry's text after `- [<color>] `) and `options`.
- [ ] **EXT-EVT-004**: If a `before_start` or `before_done` handler raises a Lua error or hits a runtime fault (including an exceeded sandbox limit), then the system shall stop the command immediately, run no further handlers, leave GBIV.md unchanged, fire no `after_*` event, and exit `1` printing `<extension>: <message>`.
- [ ] **EXT-EVT-005**: When printing a veto message, the system shall strip Lua's `file:line:` position prefix from a string error, print a non-string error value with `tostring`, and print `(no message)` for a `nil` error.
- [ ] **EXT-EVT-006**: The system shall fire no rollback or abort event to handlers that ran before a veto.
- [ ] **EXT-EVT-007**: When a `before_start` handler returns `nil` or `false`, the system shall treat it as naming nothing and continue with the next handler.
- [ ] **EXT-EVT-008**: When a `before_start` handler returns a table with a string `text` field, and no earlier handler named the entry, the system shall use the trimmed `text` as the entry's text, ignoring any other fields.
- [ ] **EXT-EVT-009**: When a `before_start` handler returns a valid naming table after an earlier handler already named the entry, the system shall ignore it and print a warning naming the extension.
- [ ] **EXT-EVT-010**: If a `before_start` handler returns any other value, a table without a string `text`, or a `text` that is empty after trimming, spans more than one line, or starts with `[done]` or `[in-progress]`, then the system shall treat it as a veto by that extension (EXT-EVT-004).
- [ ] **EXT-EVT-012**: The system shall fire `after_start` and `after_done` only after the command has printed its success line (FL-START-010, FL-DONE-008).
- [ ] **EXT-EVT-011**: If an `after_start` or `after_done` handler raises an error, then the system shall print `<extension>: <message>` as a warning, continue with the remaining handlers, and keep the command's exit code `0`.

## Sandbox

- [ ] **EXT-SBX-001**: Each extension state shall open only the `string`, `table`, `math`, `utf8` and `coroutine` libraries, plus an `os` table containing only `os.time`, `os.clock` and `os.date`.
- [ ] **EXT-SBX-002**: Each extension state shall not provide `io`, `debug`, `package`, `load`, `loadfile`, `dofile` or `collectgarbage`, and shall provide `require` only as gbiv's loader (EXT-LOAD-010).
- [ ] **EXT-SBX-003**: The system shall limit each extension state to 64 MiB of memory, raising a Lua error in the running handler when the limit is exceeded.
- [ ] **EXT-SBX-004**: The system shall limit each handler call to 60 seconds of wall time, including time spent in host functions, raising a Lua error in the running handler when the limit is exceeded.
- [ ] **EXT-SBX-005**: When a handler calls a host function that can block (`gbiv.git`, `gbiv.http`, `gbiv.sleep`), the system shall bound that call by the handler's remaining time budget, killing a `git` child process still running at the deadline and raising a Lua error.

## Host API

- [ ] **EXT-API-001**: Every extension state shall have a global `gbiv` table containing `api_version` (the number `1`), `log`, `warn`, `sleep` and `toml.decode`.
- [ ] **EXT-API-002**: `gbiv.log(msg)` shall write `<extension>: <msg>` to stderr, and `gbiv.warn(msg)` shall write `<extension>: warning: <msg>` to stderr.
- [ ] **EXT-API-003**: `gbiv.sleep(seconds)` shall sleep for at most 10 seconds per call.
- [ ] **EXT-API-004**: `gbiv.toml.decode(text)` shall return the parsed TOML as a Lua table and raise a Lua error on invalid TOML.
- [ ] **EXT-API-005**: The system shall include `gbiv.fs`, `gbiv.git`, `gbiv.http` and `gbiv.env` in an extension's `gbiv` table only when the manifest declares the matching capability (`fs`, `git`, `http`, `env`) and the config grants it (`paths` non-empty for `fs` and `git`, `http = true`, `env` non-empty).
- [ ] **EXT-API-006**: `gbiv.fs` shall provide `read(path)` (contents, or `nil` if the path does not exist), `write(path, text)` (creating parent directories), `remove(path)` (no error if absent), `exists(path)` (boolean) and `list(path)` (array of entry names, or `nil` if the path does not exist).
- [ ] **EXT-API-007**: When a `gbiv.fs` function is given a path, the system shall expand `~/`, resolve relative paths against the main repo, resolve `..`, canonicalize the path (or, if it does not exist, its nearest existing ancestor), and raise a Lua error unless the result lies inside one of the extension's canonicalized granted paths.
- [ ] **EXT-API-008**: If `gbiv.fs.write` targets a symlink whose destination lies outside the granted paths, then the system shall raise a Lua error and write nothing.
- [ ] **EXT-API-009**: `gbiv.fs.write` shall write to a temporary file in the target's directory and rename it into place.
- [ ] **EXT-API-010**: If `gbiv.fs.read` is given a directory, `gbiv.fs.list` is given a file, or any `gbiv.fs` call hits an I/O error, then the system shall raise a Lua error.
- [ ] **EXT-API-011**: `gbiv.git(dir, args)` shall run `git -C <dir> <args...>` without a shell and return a table `{code, stdout, stderr}`, returning (not raising) for a non-zero exit code.
- [ ] **EXT-API-012**: If `gbiv.git`'s `dir` does not lie inside a granted path (checked as in EXT-API-007), or any element of `args` before the first element not starting with `-` starts with `-`, then the system shall raise a Lua error without running git.
- [ ] **EXT-API-013**: The system shall run every `gbiv.git` child with stdin closed and the environment variables `GIT_TERMINAL_PROMPT=0` and `LC_ALL=C` set.
- [ ] **EXT-API-014**: `gbiv.http(request)` shall perform the request described by `method`, `url`, `headers` and `body` with a 30-second timeout and return `{status, headers, body}`, returning (not raising) for HTTP error statuses and raising a Lua error on a transport failure.
- [ ] **EXT-API-015**: `gbiv.env(name)` shall return the environment variable's value (or `nil` if unset) when `name` is listed in the extension's `env` grant, and raise a Lua error otherwise.

## `gbiv install-extension`

- [ ] **EXT-INST-001**: The system shall provide a top-level `gbiv install-extension <name>` command with an optional `--scope user|project` flag (default `user`) and an optional `--force` flag.
- [ ] **EXT-INST-002**: The system shall compile the files of each bundled extension (in this version, only `vat`) into the gbiv binary and install them from there, with no source tree needed at run time.
- [ ] **EXT-INST-003**: If `<name>` is not a bundled extension, then the command shall exit `1` with a message listing the bundled names.
- [ ] **EXT-INST-004**: When `--scope user` is given (or no scope), the command shall install to `<user-config-dir>/gbiv/extensions/<name>/`; when `--scope project` is given, to `<gbiv-root>/.gbiv/extensions/<name>/`.
- [ ] **EXT-INST-005**: If `--scope project` is given outside a gbiv workspace, then the command shall exit `2`.
- [ ] **EXT-INST-006**: The command shall decide its action as `gbiv install-skill` does (INSTALL-CLI-020 through INSTALL-CLI-028), comparing directory contents in place of `SKILL.md` and the manifest's `version` in place of the skill's `version:` field: missing → `installed`; identical → `unchanged`, or `updated` with `--force`; different → `updated` with `--force` or when the installed version is lower, otherwise `refused` with nothing written.
- [ ] **EXT-INST-007**: When the action is `installed`, `updated` or `unchanged`, the command shall ensure `<name>` is in the `[extensions] enabled` list of the chosen scope's config file (global for `user`, project for `project`), appending it if absent and creating the file if needed.
- [ ] **EXT-INST-008**: When enabling an extension that ships default grants (vat: `paths = ["backlog", "~/.config/vat"]`, `env = ["XDG_CONFIG_HOME"]`), the command shall write that `[extensions.<name>]` table only if the chosen scope's config file has no such table.
- [ ] **EXT-INST-009**: The command shall edit config files so that comments and formatting outside the changed keys are preserved.
- [ ] **EXT-INST-010**: If the chosen scope's config file exists but is not valid TOML, or its `[extensions] enabled` is not an array, then the command shall exit `1`, copy no extension files, and change no config.
- [ ] **EXT-INST-011**: If `<name>` is listed in `[extensions] disabled` in either config file, then the command shall print a warning that the extension stays off until removed from that list.
- [ ] **EXT-INST-012**: The command shall print the same JSON result object as `gbiv install-skill` (INSTALL-CLI-030, INSTALL-CLI-031) with the extension's manifest versions, plus a boolean `enabled` field stating whether this run added `<name>` to `enabled`.
- [ ] **EXT-INST-013**: The command shall exit `0` for `installed`, `updated` and `unchanged`, `7` for `refused`, `2` per EXT-INST-005, and `1` for other failures.

## References

- LLD: `docs/llds/extension-runtime.md`
- Companion: `docs/specs/feature-ledger.md` (the commands that fire the events)
- Companion: `docs/specs/vat-extension.md` (the bundled reference extension)
- Companion: `docs/specs/cli-and-palette.md` (layered config files)
- Companion: `docs/specs/orchestrate-cli.md` (`INSTALL-CLI-*`, the install pattern mirrored here)
