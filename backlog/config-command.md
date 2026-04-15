# `config` Command (`src/commands/config.rs`)

New command: `gbiv config <key> [value]`

Git-style user config stored in a TOML file at `~/.config/gbiv/config.toml`
(respects `$XDG_CONFIG_HOME` if set).

## Behavior
- `gbiv config user.name "Jared"` — sets `user.name`
- `gbiv config user.name` — prints current value, exits non-zero if unset
- Creates the config file + parent dir on first write

## Supported Keys (initial)
- `user.name` — used by `assign` to tag entries with `[by:<name>]`

## Config Loading
Add a small module `src/config.rs`:
- `load_config() -> Config` — reads the TOML, returns empty config if missing
- `Config::user_name(&self) -> Option<&str>`
- Used by `assign` to look up the current user's name
