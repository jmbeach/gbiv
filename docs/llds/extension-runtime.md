# Extension Runtime

**Created**: 2026-10-12
**Status**: Draft (greenfield)

## Context and Design Philosophy

Extensions are how gbiv works with task systems without knowing about any of
them. An extension is a small Lua program that hooks `gbiv start` and
`gbiv done`. It can stop the command, name the task, and update the task system
(vat, Jira, anything). gbiv itself never reads a task system and never touches
the network.

Three principles shape the runtime:

1. **Hooks, not roles.** There is no "task provider" slot. Every enabled
   extension's hooks run, in config order. An extension that talks to a task
   system is just one whose hooks happen to do so.
2. **Nothing is reachable unless granted.** Each extension runs in its own
   sandboxed Lua state. File access, `git`, HTTP and environment variables all go
   through a host API, and each is limited to what the config grants that
   extension by name. A fresh install enables no extensions, so out of the box
   gbiv does nothing an extension could do.
3. **A small, versioned contract.** Four events and a handful of host functions,
   under one `api_version`. Extension authors target a number, not gbiv's
   internals.

The runtime is the third domain in the HLD (§ Extensions domain). The commands
that fire its events, `gbiv start` and `gbiv done`, are owned by the Feature
Ledger (`docs/llds/feature-ledger.md`). Config layering is owned by CLI & Palette
(`docs/llds/cli-and-palette.md`); this document defines only the `[extensions]`
tables inside it.

## Lua Engine

gbiv embeds Lua 5.4 through `mlua` with the `lua54` and `vendored` features, so
the Lua C source is compiled into the binary and no system Lua is needed. gbiv
already needs a C compiler to build (`ureq`'s TLS stack pulls in `ring`), so
vendored Lua adds no new toolchain requirement.

Each enabled extension gets its own `mlua::Lua` state, created per command
invocation and dropped when the command exits. Extensions cannot see each other's
globals. Nothing persists between invocations except what an extension writes to
granted files.

## Extension Layout

An extension is a directory named after the extension:

```
<extensions-dir>/<name>/
  extension.toml     # manifest (required)
  init.lua           # entry point (required)
  *.lua              # optional modules, loaded with require
```

### Manifest — `extension.toml`

```toml
name = "vat"                 # must equal the directory name
version = "0.1.0"            # the extension's own semver; used by install-extension
api_version = 1              # host contract this extension targets
description = "Claim and complete vat backlog tasks"
capabilities = ["fs", "git"] # subset of: fs, git, http, env
```

- `name` must match the directory name and `[A-Za-z0-9_-]+`.
- `api_version` is an integer major version. gbiv supports exactly the
  versions it lists (today: `1`); any other value is a load error.
- `capabilities` lists the host-function groups the extension wants. A
  capability that is not both declared here *and* granted in config is absent
  from the extension's `gbiv` table: the functions are simply not there, so
  calling one is an ordinary Lua error.

### Entry point — `init.lua`

`init.lua` runs once when the state is created and must return a table of hook
functions. Any subset of the four event names may be present; unknown keys are a
load error, so a misspelled hook is loud.

```lua
local backlog = require("backlog")

return {
  before_start = function(ctx) ... end,
  before_done  = function(ctx) ... end,
}
```

`require(name)` is gbiv's own loader: it resolves `name` to `<name>.lua` inside
the extension's directory (dots map to `/`) and loads it as text. It cannot load
files outside the directory, native modules, or precompiled bytecode.

## Discovery and Loading

Two extension directories exist:

| Scope | Directory |
|---|---|
| project | `<gbiv-root>/.gbiv/extensions/` |
| user | `$XDG_CONFIG_HOME/gbiv/extensions/`, else `~/.config/gbiv/extensions/` |

For each name in the effective `[extensions] enabled` list (see Configuration),
in order:

1. Look for `<project-dir>/<name>/`, then `<user-dir>/<name>/`. The first one
   found is used; the other is ignored.
2. Not found in either → hard error naming the extension, both searched paths,
   and the fix (`gbiv install-extension <name>`). The command does nothing.
   Directory names are compared byte for byte (`Vat/` doesn't match `vat`). A
   directory that exists but has a missing or bad manifest is an error; gbiv does
   not fall back to the other scope's copy.
3. Parse and validate the manifest; create the sandboxed state; run `init.lua`;
   validate the returned hook table.

All enabled extensions load before any event fires. A load failure in any one
aborts the command before it changes anything, with exit `1` (as do config errors
and vetoes). Extensions load only for commands
that fire events (`start`, `done`); every other command ignores them, so a broken
extension can never block `gbiv status` or `gbiv fleet start`.

## Configuration

The `[extensions]` table lives in the layered config (global, then project; see
`docs/llds/cli-and-palette.md` § Layered configuration); how its two layers
combine is defined under Merging layers below.

```toml
[extensions]
enabled = ["vat"]                 # adds to the other layer's list
disabled = []                     # switches names off, whichever layer enabled them

[extensions.vat]                  # per-extension table, keyed by name
paths = ["backlog", "~/.config/vat"]   # grants fs + git under these paths
http = false                      # grants http (also needs the capability)
env = []                          # environment variables readable via gbiv.env
backlog = "backlog"               # any other key is passed to the extension
```

**Reserved keys** in a per-extension table are `paths`, `http` and `env`. Every
other key is the extension's own option, handed to it as `ctx.options` (a Lua
table converted from TOML).

**Path resolution.** A relative path resolves against the main worktree's repo
(`<gbiv-root>/main/<repo>/`, where GBIV.md lives), whether it came from the
global or the project file, so one global grant such as `paths = ["backlog"]`
means "this project's backlog" in every project. A leading `~/` expands to the
home directory. Paths are canonicalized at load; a grant that doesn't exist yet is
kept as written and checked again at each call.

**Merging layers.** The effective list of extensions is the global `enabled`
list followed by the project's names not already in it, minus every name in
either file's `disabled` list. That is also the load and run order. Each
per-extension table is merged key by key, the project winning on the same key: a
project `[extensions.vat]` that sets only `backlog` keeps the global `paths`. A
list value inside a per-extension table (`paths`, `env`) is replaced whole.

**Validation.** `enabled` and `disabled` must be arrays of strings; a name
repeated within one list is a config error. Inside `[extensions]`, every other key
must be a table (a per-extension table); a non-table key is a config error, which
catches typos like `enabeld`. `enabled` and `disabled` are therefore reserved
names no extension can use. A per-extension
table for an extension that isn't enabled is ignored. An enabled extension whose manifest doesn't declare a capability
the config grants gets a warning, not an error (the grant is just unused).

## Events

Four events, fired by the Feature Ledger's commands. Each enabled extension's
handler for the event runs in `enabled` order; an extension without a handler is
skipped.

| Event | Fired | Context (`ctx`) | Handler may |
|---|---|---|---|
| `before_start` | after `gbiv start` picks a color, before GBIV.md is written | `arg`, `color`, `text`, `options` | veto; return `{text=}` |
| `after_start` | after the entry is written | `color`, `text`, `options` | nothing (notify only) |
| `before_done` | after `gbiv done` finds the entry, before it is removed | `color`, `text`, `options` | veto |
| `after_done` | after the entry is removed | `color`, `text`, `options` | nothing (notify only) |

- `arg` is the `gbiv start` argument exactly as typed.
- `text` is the entry's text: everything after `- [<color>] ` on its GBIV.md
  line. It is `nil` in `before_start` until some handler names the entry; later
  handlers see the named value. gbiv attaches no meaning to it; an extension
  finds its own tags in it (the vat extension looks for its `[<id>]`).

### Veto

Raising a Lua error in a `before_*` handler vetoes the command:
`error("gbv-1ht is already claimed by sam")`. gbiv stops at once: later handlers
don't run, GBIV.md is not touched, no `after_*` event fires, and gbiv exits `1`
printing `<extension>: <message>`. The message is the error string with Lua's
`file:line:` prefix removed; a non-string error value is printed with
`tostring`, and `error(nil)` prints `<extension>: (no message)`. A runtime fault (a bug, an exceeded limit) is
treated the same way, so a broken `before_*` handler can never let a command
through half-checked.

There is no rollback event. If an earlier handler already acted (say, claimed the
task) before a later one vetoed, that action stands; the user resolves it in the
task system.

### Naming

A `before_start` handler names the entry by returning `{ text = "..." }`.
Returning `nil` or `false` names nothing. Any other non-table value, or a table
without a string `text`, is a veto by that extension. Extra keys are ignored. The
first handler to return a valid table wins; later tables are ignored (gbiv prints
a warning naming the extension).

By convention an extension puts its own ID tag first in `text`, right after the
color tag gbiv adds (`[gbv-1ht] [blocked-by:gbv-x2v] Title`); `gbiv start` uses
that leading tag for its duplicate check, and `gbiv done <id>` finds it.

`text` is trimmed and must be non-empty and a single line. It may start with
bracketed tags (`[gbv-1ht] [blocked-by:gbv-x2v] Title`), but not with `[done]` or
`[in-progress]`, which GBIV.md would read back as a legacy status. An invalid
`text` is a veto by that extension. If no handler names the entry, the Feature
Ledger uses `arg` as the text.

### After events

`after_*` handlers run after the command has printed its success line, so a slow
handler never hides the fact that the change landed. An error in an `after_*`
handler is printed as a warning (`<extension>: <message>`) and the remaining
handlers still run. The command still exits `0`: the GBIV.md change has been made
and is not undone.

## Sandbox

### Standard library

Each state opens `string`, `table`, `math`, `utf8` and `coroutine`, plus a
reduced `os` holding only `os.time`, `os.clock` and `os.date`. Not opened: `io`,
`debug`, `package`, the rest of `os`. The globals `load`, `loadfile`, `dofile`
and `collectgarbage` are removed; `require` is replaced with gbiv's loader.

### Limits

| Limit | Value | On breach |
|---|---|---|
| Memory per state | 64 MiB (`Lua::set_memory_limit`) | Lua error in the running handler |
| Wall time per handler call | 60 s, checked by an instruction-count hook | Lua error in the running handler |

Time spent inside host functions (a slow `git push`) counts toward the
handler's 60 s. The instruction hook can't interrupt Rust code, so every host
call that can block (`git`, `http`, `sleep`) is given the handler's remaining
budget as its own timeout; a `git` child still running at the deadline is killed
and the call raises an error.

## Host API (`api_version` 1)

Every state has a global `gbiv` table. Functions appear in it only for the
capabilities the extension both declares and is granted.

### Always present

| Function | Returns | Notes |
|---|---|---|
| `gbiv.api_version` | `1` | a number, not a function |
| `gbiv.log(msg)` | — | stderr, prefixed `<name>: ` |
| `gbiv.warn(msg)` | — | stderr, prefixed `<name>: warning: ` |
| `gbiv.sleep(seconds)` | — | capped at 10 s per call |
| `gbiv.toml.decode(text)` | Lua table | raises on invalid TOML; lets extensions read config files like vat's |

### `fs` (needs a `paths` grant)

| Function | Returns |
|---|---|
| `gbiv.fs.read(path)` | file contents as a string, or `nil` if it doesn't exist |
| `gbiv.fs.write(path, text)` | — (creates parent directories) |
| `gbiv.fs.remove(path)` | — (no error if absent) |
| `gbiv.fs.exists(path)` | boolean |
| `gbiv.fs.list(path)` | array of entry names |

A relative `path` resolves like a config grant (against the main repo), and a
leading `~/` expands to the home directory. `..` components are resolved first.
The check canonicalizes the path, or for a path that doesn't exist yet its
nearest existing ancestor, and requires the result to lie inside a canonicalized
grant; otherwise the call raises an error. A write whose target is a symlink
pointing outside the grants is refused. Writes go to a temporary file in the same
directory and are renamed into place.

Error shapes: `read` on a directory, `list` on a file, and any I/O failure raise
a Lua error; `list` on a missing path returns `nil`. `require` of a name that
resolves outside the extension directory (including through `..` or a symlink)
raises an error.

### `git` (needs a `paths` grant)

`gbiv.git(dir, {args...})` runs `git -C <dir> <args...>` with no shell and
returns `{ code = <exit code>, stdout = "...", stderr = "..." }`. A non-zero exit
is a return value, not an error, so an extension can branch on it (the vat claim
loop needs to tell a rejected push from other failures). `dir` must lie inside a
granted path. Granting a path grants every git subcommand there, including
`fetch` and `push`; git uses the user's own credentials and configuration.

So that path scoping actually holds, the first element of `args` must be the
subcommand: any argument before it that starts with `-` is refused, which blocks
`-c` (aliases and `core.sshCommand` can run arbitrary programs), `-C`,
`--git-dir`, `--work-tree` and `--exec-path`. git runs with stdin closed,
`GIT_TERMINAL_PROMPT=0` (a credential prompt fails instead of hanging), and
`LC_ALL=C` (so extensions can match git's messages reliably).

### `http` (needs the capability and `http = true`)

`gbiv.http({ method=, url=, headers=, body= })` returns
`{ status=, headers=, body= }`, using gbiv's existing `ureq` client with a 30 s
timeout. Transport failures raise an error; HTTP error statuses are return values.

### `env` (needs names in `env`)

`gbiv.env(name)` returns the variable's value, or `nil` if it is unset. Names not
listed in the extension's `env` grant raise an error.

## `gbiv install-extension`

Copies a bundled extension into an extension directory and enables it. The
bundled extensions (today, only `vat`) are compiled into the binary with
`include_dir!`, so a `cargo install`'d gbiv needs no source tree.

```
gbiv install-extension <name> [--scope user|project] [--force]
```

Behavior mirrors `gbiv install-skill` (`docs/llds/orchestrate-cli.md`
§ `gbiv install-skill`), with the manifest's `version` playing the role of the
skill's `version:` line:

- **Scope.** `user` (default) writes to the user extension directory; `project`
  writes to `<gbiv-root>/.gbiv/extensions/` and requires being inside a gbiv
  workspace.
- **Idempotency.** Missing → `installed`. Identical → `unchanged`. Different →
  with `--force` `updated`; without it, an older installed `version` is
  `updated`, while the same or a newer version (or an unparseable manifest) is
  `refused`, exit `7`.
- **Enabling.** After a successful `installed` or `updated` (and on
  `unchanged`), the command makes sure `<name>` is in that scope's config
  `[extensions] enabled` list, appending it if absent and creating the config file
  if needed. For a bundled extension that ships default grants (vat:
  `paths = ["backlog", "~/.config/vat"]`), it writes the per-extension table only
  if that table doesn't exist yet. The edit uses `toml_edit` so comments and
  formatting elsewhere in the file survive.
- **Output.** The same JSON result shape and exit codes as `install-skill`
  (0 / 1 / 2 / 7), plus `"enabled": true|false` saying whether this run added the
  name to `enabled`.
- An unknown `<name>` (not bundled) exits `1` listing the bundled names.
- A config file that doesn't parse as TOML, or whose `enabled` isn't an array,
  exits `1` and changes nothing (the extension files aren't copied either).
- If the name is in a `disabled` list in either config, the command prints a
  warning that the extension stays off until it's removed from there.

## Error Types

The runtime is a leaf module and returns a typed error, per the HLD's
error-handling rule. Command handlers wrap it with `anyhow` context.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("extension {name:?} is enabled but not installed (looked in {searched:?}); run: gbiv install-extension {name}")]
    NotFound { name: String, searched: Vec<PathBuf> },
    #[error("extension {name:?}: bad manifest {path}: {detail}")]
    BadManifest { name: String, path: PathBuf, detail: String },
    #[error("extension {name:?} targets api_version {wanted}; this gbiv supports {supported:?}")]
    ApiVersion { name: String, wanted: i64, supported: Vec<i64> },
    #[error("extension {name:?} failed to load: {detail}")]
    Load { name: String, detail: String },
    #[error("{name}: {message}")]
    Veto { name: String, message: String },
    #[error("[extensions] config: {detail}")]
    Config { detail: String },
}
```

## Module Shape

```rust
// crates/gbiv/src/extensions/mod.rs
pub struct Runtime { /* loaded extensions, in enabled order */ }

impl Runtime {
    /// Load every enabled extension. Empty `enabled` → a Runtime with none.
    pub fn load(root: &GbivRoot, config: &ExtensionsConfig) -> Result<Runtime, ExtensionError>;
    pub fn before_start(&self, arg: &str, color: &str) -> Result<Option<Naming>, ExtensionError>;
    pub fn after_start(&self, entry: &EntryView) -> Vec<Warning>;
    pub fn before_done(&self, entry: &EntryView) -> Result<(), ExtensionError>;
    pub fn after_done(&self, entry: &EntryView) -> Vec<Warning>;
}
```

The Feature Ledger's `start` and `done` handlers are the only callers.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Plugin mechanism | Embedded Lua | External `gbiv-provider-*` executables over JSON; embedded WASM | Sandboxing, single-folder plugins, and vat logic is small enough to write in Lua. Network via an extension is acceptable as long as nothing happens out of the box. |
| Lua engine | `mlua` + vendored Lua 5.4 | omnilua, piccolo (pure Rust); Rhai, starlark (not Lua) | Mature and real Lua. gbiv already needs a C compiler through `ring`, so vendoring costs no new toolchain. |
| Extension model | Generic hooks on `start`/`done` | A dedicated "task provider" trait with one configured provider | One mechanism; an extension is enabled or not. Anything a provider did is expressible as `before_*` hooks. |
| Veto signal | Raise a Lua error | A `{veto=...}` return value | Idiomatic Lua, and a bug in a `before_*` hook then fails safe (vetoes) rather than letting the command through. |
| Rollback on veto | None | A `start_aborted` event to earlier hooks | Kept out for now; a stray claim is fixed in the task system. |
| Entry naming | Extension returns the entry's whole text | Return a task ID and title for gbiv to format | gbiv stays unaware of task IDs; each extension decides which of its tags to show. |
| Combining config layers | `enabled` adds up; `disabled` removes | Project `enabled` replaces global | Like Claude Code's settings: a user-level install stays active in every project unless a project switches it off. |
| One Lua state per extension | Isolated states | One shared state | Extensions can't interfere with each other's globals; cost is negligible at this scale. |
| Grants by path | `paths` grant enables `fs` and `git` under those paths | Separate fs and git grant lists; unrestricted git | One list is enough for the known extension (vat), and path scoping is what keeps git from touching unrelated repos. |
| git network ops | Allowed under a granted path | Block fetch/push unless an extra grant | vat's claim loop needs push; the promise "nothing out of the box" holds because no extension is enabled by default. |
| Relative grant paths | Resolve against the main repo | Against the config file's directory | Makes a global grant like `paths = ["backlog"]` mean the same thing in every project. |
| Bundled extensions | `include_dir!`, installed by `gbiv install-extension` | Auto-load the embedded copy without installing | An explicit install step was the requested shape, and an installed copy is something the user can read and edit. |
| Load scope | Only `start` and `done` load extensions | Load for every command | A broken extension can't break unrelated commands. |

## Open Questions & Future Decisions

### Resolved
1. ✅ No rollback event on veto (HLD).
2. ✅ No extension-defined subcommands in v1 (HLD non-goal).

### Deferred
1. More events (`rebase-all`, `status`, `repair`), when a concrete extension needs one.
2. A `gbiv extensions list` / `doctor` command to show what's enabled, where each was found, and its grants.

## References

- `docs/high-level-design.md` § Extensions domain
- `docs/llds/feature-ledger.md` — `gbiv start` / `gbiv done`, the event callers
- `docs/llds/vat-extension.md` — the bundled reference extension
- `docs/llds/cli-and-palette.md` — layered config loading
- `docs/llds/orchestrate-cli.md` § `gbiv install-skill` — the install pattern mirrored here
- `mlua`: https://crates.io/crates/mlua
