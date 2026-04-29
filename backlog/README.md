# Backlog

This directory is managed by [VAT](https://github.com/) (Versioned Addressable Tasks) — a tiny tool for capturing tasks as plain markdown.

## Files

- `backlog.md` — the flat list of tasks. Jot bullets here as you think of them.
- `vat.toml` — project config (the 3-char ID prefix for this repo: `gbv`).
- `.used-ids` — tombstone list of every ID ever issued. Committed. Don't hand-edit.
- `items/<id>.md` — per-task notes; created when a task has notes, deleted when the task is `done`.

## Workflow

1. Type new bullets into `backlog.md`:
   ```
   - rewrite the cache layer
       why: the LRU is thrashing on hot keys
   ```
2. Run `vat sync` to assign IDs and tuck notes into `items/<id>.md`.
3. Claim a task with `vat start <id>`. Mark it complete with `vat done <id>`.

This README is written once at init time. VAT never reads or rewrites it — feel free to edit or delete.
