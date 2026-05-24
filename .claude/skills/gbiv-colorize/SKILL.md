---
name: gbiv-colorize
description: Assign ROYGBIV color tags to bullets in `gbiv.md` top-to-bottom, skipping colors already in use. Stops once every color (red, orange, yellow, green, blue, indigo, violet) has at least one bullet. Use when the user says "colorize gbiv", "assign colors", "tag gbiv with ROYGBIV", or asks to fill in color tags on their gbiv list.
argument-hint: (no args)
allowed-tools: Read, Write
---

# gbiv-colorize skill

Walks the bullets in `gbiv.md` from the top down and prepends a ROYGBIV color tag (`[red]`, `[orange]`, `[yellow]`, `[green]`, `[blue]`, `[indigo]`, `[violet]`) to each currently-untagged bullet, picking colors in ROYGBIV order while skipping any color that's already in use somewhere in the file. Stops as soon as every color has at least one bullet.

## Input / output

- `gbiv.md` at the repo root. Read and rewritten in place.
- Only the **parsed region** (everything above the first standalone `---` line, or the whole file if no separator exists) is touched. Everything from `---` onward is preserved byte-for-byte.

## Color order

`red, orange, yellow, green, blue, indigo, violet` (ROYGBIV — exactly these 7 lowercase tokens).

## What counts as a color tag

The bullet's **first bracketed token after `- `**, if it matches one of the 7 ROYGBIV colors (case-insensitive). Other bracketed tokens (e.g. `[vat-q3m]`) are not color tags. A bullet with a non-color first token like `- [vat-q3m] …` is treated as **uncolored**.

## Procedure

1. Read `gbiv.md`. Missing → abort: `gbiv.md not found at repo root`.
2. Split at the first standalone `---` line. Preserve the freeform region (separator and below) byte-for-byte.
3. Walk the parsed region. For each line that starts with `- ` at column 0, classify it:
   - **Colored**: first bracketed token matches a ROYGBIV color → record that color as taken.
   - **Uncolored**: anything else (no bracket, or first bracket is a non-color token).
4. Build the **available** queue: ROYGBIV order, minus colors already taken.
5. Walk bullets again top-to-bottom. For each uncolored bullet:
   - If `available` is empty → stop the walk.
   - Pop the next color from `available`, prepend `[<color>] ` immediately after the `- ` (before any existing tokens), and mark the color as taken.
6. **Termination**: stop assigning as soon as `taken` covers all 7 colors, even if more uncolored bullets remain. Leave the rest of the file unchanged.
7. Write the file back: parsed region (with insertions) + freeform region (verbatim). End with a single trailing newline. If no changes were made, skip the write and report `unchanged`.
8. Report a one-line summary: `assigned <N> color(s): <c1>, <c2>, ...` or `unchanged` (no uncolored bullets, or every color already taken).

## Edge cases

- A bullet already tagged with a non-ROYGBIV color (e.g. `[teal]`) is treated as uncolored and may be re-tagged with a real color prepended in front of `[teal]`. (Don't try to interpret arbitrary tag namespaces.)
- Two bullets with the same color tag → both count as taken (the color is just removed from `available` once).
- Empty parsed region → write nothing, report `unchanged`.
- No `---` separator → the entire file is the parsed region; preserve trailing content as-is otherwise.

## Files this skill is allowed to touch

- `gbiv.md` — read and write.

Nothing else. In particular: does not touch `backlog/`, item files, or any VAT-owned file.
