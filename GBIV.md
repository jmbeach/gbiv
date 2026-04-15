- Add `config` command
  See backlog/config-command.md
- Rewrite GBIV.md parser for flexible tags
  See backlog/rewrite-parser.md
- Add `assign` command
  See backlog/assign-command.md
- Add `unassign` command
  `gbiv unassign [color]`. Removes `[assigned]` status and `[by:name]` tag from the `[color]` entry. Leaves `[in-progress]`/`[done]` alone.
- Update `mark` for multi-user
  See backlog/update-mark.md
- Update `status` for multi-user
  See backlog/update-status.md
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
