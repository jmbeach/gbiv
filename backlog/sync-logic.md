# Sync/Reconcile Logic (`src/sync.rs`)

Create the sync step that reconciles GBIV.md with the state file. Runs at the start of key commands.

## Function
`sync_gbiv_md_with_state(gbiv_md_path: &Path, gbiv_root: &Path) -> Result<(), String>`

## Logic
1. Parse GBIV.md, load state file
2. For every entry that has NO `[id:xxx]`: generate an ID, write it into the entry
3. For every entry that has a color tag (`[red]`, `[green]`, etc.):
   - If entry also has an `[id:xxx]`: write assignment (id -> color) to state, strip color tag
   - If entry has no ID (shouldn't happen after step 2, but defensive): generate ID first, then assign
4. Rewrite GBIV.md with updated entries
5. Save state file

## Integration Points
Add `sync` call at the start of: `status`, `assign`, `mark`, `reset`, `tidy`

## Key Properties
- Idempotent: running sync twice produces the same result
- If `[id:xxx]` already exists, never regenerate
- Handles migration: old-style `[red]` entries auto-convert on first run
