# State File Module (`src/gbiv_state.rs`)

Create a new module for managing `.gbiv-state.json` — the per-clone file that maps feature IDs to color worktrees.

## Struct
- `GbivState` with `assignments: HashMap<String, String>` (id -> color)
- Derive `Serialize`/`Deserialize` via serde

## Functions
- `load_state(gbiv_root: &Path) -> GbivState` — reads `.gbiv-state.json`, returns empty state if file missing
- `save_state(gbiv_root: &Path, state: &GbivState) -> Result<(), String>` — writes JSON via serde_json
- `assign(state: &mut GbivState, id: &str, color: &str)` — adds/overwrites assignment
- `unassign_by_color(state: &mut GbivState, color: &str) -> Option<String>` — removes assignment, returns ID
- `unassign_by_id(state: &mut GbivState, id: &str) -> Option<String>` — removes assignment, returns color
- `get_id_for_color(state: &GbivState, color: &str) -> Option<&str>` — reverse lookup
- `get_color_for_id(state: &GbivState, id: &str) -> Option<&str>` — forward lookup

## ID Generation
- `generate_id(existing_ids: &HashSet<String>) -> Result<String, String>` — 4-char lowercase alphanumeric via `rand` crate
- Collision check: retry up to 10 times, error if all collide

## Dependencies
Add to `Cargo.toml`: `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `rand = "0.8"`

## File Location
`.gbiv-state.json` lives at `<gbiv-root>/` (alongside color dirs, outside any git repo — no gitignore needed)
