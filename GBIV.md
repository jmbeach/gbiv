- Add state file module (gbiv_state.rs)
  See backlog/state-file-module.md
- Rewrite GBIV.md parser for flexible tags
  See backlog/rewrite-parser.md
- Add sync/reconcile logic (sync.rs)
  See backlog/sync-logic.md
- Add `assign` command
  See backlog/assign-command.md
- Add `unassign` command
  Accepts [color] or --id (mutually exclusive). Clears state assignment, clears [assigned] status (leaves [in-progress]/[done]), clears [by:name].
- Update `reset` for multi-user
  See backlog/update-reset.md
- Update `mark` for multi-user
  See backlog/update-mark.md
- Update `status` for multi-user
  See backlog/update-status.md
- Update `init` to create .gbiv-state.json
  Create empty state file at gbiv root on init. All state-writing commands also create if missing. Update GBIV.md template to mention [id:xxx] syntax below the --- cutoff.
- Add .gbiv-state.json to GBIV_STATE_FILES in rebase_all.rs
- Integrate sync into command entry points
  Add sync call at start of: status, assign, mark, reset, tidy
- Touch up the readme (un AI-ify)
- Upload to cargo

---
# GBIV.md

Add features above the `---` line. Each feature starts with `- ` and an optional `[color]` tag.

Example:

- [red] My urgent feature
  A note about this feature
- [green] A less urgent feature
- An untagged backlog item

Supported tags match ROYGBIV colors: red, orange, yellow, green, blue, indigo, violet.
Untagged items appear with a dim `backlog` label.
Everything below `---` is ignored by gbiv.
