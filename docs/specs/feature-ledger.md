# Feature Ledger

Specs for GBIV.md parsing and mutation, and the `gbiv start` / `gbiv done` commands that put a task on a color and take it off (replacing `gbiv mark`). The extension events those commands fire are specced in `docs/specs/extension-runtime.md`.

**Component LLD**: `docs/llds/feature-ledger.md`

## Parsing

- [x] FL-PARSE-001: When a GBIV.md file contains a line starting with `- `, the system shall treat that line as a feature entry.
- [x] FL-PARSE-002: When a feature line starts with `- [`, the system shall extract the content between the first `[` and `]` as the feature's tag.
- [x] FL-PARSE-003: When a feature line has a tag and is followed by a second bracketed token that is exactly `done`, the system shall set the feature's status to `"done"`.
- [x] FL-PARSE-004: When a feature line has a tag and is followed by a second bracketed token that is exactly `in-progress`, the system shall set the feature's status to `"in-progress"`.
- [x] FL-PARSE-005: When a feature line has a tag and is followed by a second bracketed token that is neither `done` nor `in-progress`, the system shall leave status as None and include the bracketed token as part of the description.
- [x] FL-PARSE-006: When a feature line has no bracketed prefix, the system shall set both tag and status to None and use the full text after `- ` as the description.
- [x] FL-PARSE-007: When non-feature, non-empty lines appear after a feature entry, the system shall attach them to the preceding feature's notes vector.
- [x] FL-PARSE-008: When non-feature lines appear before the first feature entry, the system shall discard them.
- [x] FL-PARSE-009: When the parser encounters a line containing exactly `---`, the system shall stop parsing and ignore all subsequent lines.
- [x] FL-PARSE-010: When the GBIV.md file is missing, the system shall return an empty feature list without error.
- [x] FL-PARSE-011: When the GBIV.md file cannot be read for reasons other than not-found, the system shall print a warning to stderr and return an empty feature list.
- [x] FL-PARSE-012: When the GBIV.md file is empty, the system shall return an empty feature list.
- [x] FL-PARSE-013: When a feature line starts with `- [` but has no closing `]`, the system shall treat the entire text after `- ` as the description with tag set to None.
- [x] FL-PARSE-014: When empty lines appear in the feature section, the system shall not attach them as notes to any feature.

## Mutations

- [ ] FL-MUTATE-012: When `append_gbiv_entry` is called with a color and text, the system shall insert the line `- [<color>] <text>` after the last entry (and its note lines) in the parsed region above the first `---` line.
- [ ] FL-MUTATE-013: When `append_gbiv_entry` is called and the parsed region has no entries, the system shall insert the line after any non-blank preamble lines, otherwise at the top of the file, before blank lines and the `---` separator.
- [ ] FL-MUTATE-014: When `append_gbiv_entry` is called and GBIV.md does not exist, the system shall create it containing only the entry line and a trailing newline.
- [ ] FL-MUTATE-015: When `append_gbiv_entry` writes, the system shall preserve every other line byte for byte, use the file's existing line ending (`\r\n` if the file uses it) for the new line, and keep the file's trailing-newline state.
- [ ] FL-MUTATE-016: When `remove_gbiv_entry` is called with a color tag and text, the system shall re-read GBIV.md and remove the single entry whose color tag and text both match, together with its note lines, using the same blank-line cleanup as `remove_gbiv_features_by_tag` (FL-MUTATE-009).
- [ ] FL-MUTATE-017: If `remove_gbiv_entry` finds no entry matching both the color tag and the text, then the system shall return an error and leave the file unchanged.
- [x] FL-MUTATE-007: When `remove_gbiv_features_by_tag` is called with a tag, the system shall remove all entries matching that tag and their attached notes.
- [x] FL-MUTATE-008: When `remove_gbiv_features_by_tag` removes entries, the system shall preserve the `---` separator and all content after it.
- [x] FL-MUTATE-009: When `remove_gbiv_features_by_tag` removes an entry that was followed by a blank line, the system shall also remove that blank line to avoid stray whitespace.
- [x] FL-MUTATE-010: When `remove_gbiv_features_by_tag` is called on a file with no matching entries, the system shall leave the file unchanged.
- [x] FL-MUTATE-011: When `remove_gbiv_features_by_tag` is called on a missing file, the system shall return Ok without error.

## Tag Matching

- [ ] FL-TAG-001: An entry shall be considered to carry the text tag `[X]` when its text (the part after its color and legacy status tags) contains the exact bracketed token `[X]`, compared case-insensitively, at any position.

## Start Command

- [ ] FL-START-001: The system shall provide a top-level `gbiv start <task>` command with an optional `--color <color>` flag.
- [ ] FL-START-002: If `<task>`, after trimming, is empty, spans more than one line, or starts with `[done]` or `[in-progress]`, then `gbiv start` shall exit `1` with an error and change nothing.
- [ ] FL-START-003: When `gbiv start` runs without `--color`, the system shall choose the first color in active-palette order (ROYGBIV, then configured extras) that no GBIV.md entry has as its color tag, regardless of that color's worktree state.
- [ ] FL-START-004: When `gbiv start` runs with `--color <color>`, the system shall use that color, and shall exit `1` with an error if the color is not in the active palette or already has a GBIV.md entry.
- [ ] FL-START-005: If no palette color is free, then `gbiv start` shall exit `1` with `no free color (all <N> colors have GBIV.md entries)` before loading any extension.
- [ ] FL-START-006: If `<task>` (trimmed) contains no whitespace and some GBIV.md entry carries the tag `[<task>]` (FL-TAG-001), then `gbiv start` shall exit `1` with `<task> is already on <color>` before loading any extension.
- [ ] FL-START-007: After choosing a color, `gbiv start` shall load extensions and fire `before_start` with the untrimmed argument and the chosen color, and if any handler vetoes, exit `1` without writing GBIV.md (EXT-EVT-004).
- [ ] FL-START-008: When a `before_start` handler names the entry (EXT-EVT-008), `gbiv start` shall write that text as the entry's text; otherwise it shall write `<task>` trimmed.
- [ ] FL-START-009: When at least one extension is enabled, no handler names the entry, and `<task>` (trimmed) contains no whitespace, `gbiv start` shall print a warning that no extension recognized it and it was added as free text.
- [ ] FL-START-010: When the entry's text is determined (FL-START-008) and passes FL-START-013, `gbiv start` shall append it with `append_gbiv_entry` (FL-MUTATE-012), then print `started on <color>: <text>` to stdout, then fire `after_start`, and exit `0`.
- [ ] FL-START-011: If writing GBIV.md fails after `before_start` handlers ran, then `gbiv start` shall exit `1` with an error stating that extensions already ran and their changes may need undoing, and shall fire no `after_start` event.
- [ ] FL-START-012: `gbiv start` shall not create, check out, reset or otherwise modify any worktree or branch.
- [ ] FL-START-013: If the entry's text, once named, starts with a bracketed text tag that an existing entry carries (FL-TAG-001), or equals an existing entry's text case-insensitively, then `gbiv start` shall exit `1` with `<tag or text> is already on <color>` and write nothing.
- [ ] FL-START-014: `gbiv start` shall perform its checks in this order: parse config files, validate `<task>` (FL-START-002), choose the color (FL-START-003 to FL-START-005), check for a duplicate argument (FL-START-006), validate `[extensions]` and load extensions, fire `before_start` (FL-START-007), check the named text (FL-START-013), write GBIV.md.

## Done Command

- [ ] FL-DONE-001: The system shall provide a top-level `gbiv done [<color|tag>]` command.
- [ ] FL-DONE-002: When the `gbiv done` argument exactly matches (case-sensitively) a name in the active palette, the system shall resolve it to the entry with that color tag.
- [ ] FL-DONE-003: When the `gbiv done` argument is not a palette name, the system shall resolve it to the entry that carries the text tag `[<argument>]` (FL-TAG-001) or whose whole text equals the argument case-insensitively.
- [ ] FL-DONE-004: When `gbiv done` is invoked without an argument, the system shall infer the color from the current working directory via `infer_color_from_path` and resolve it as in FL-DONE-002, exiting `1` with an error if inference fails.
- [ ] FL-DONE-005: If no entry matches the resolved color or tag, then `gbiv done` shall exit `1` with `no GBIV.md entry for <color>` or `no GBIV.md entry carries [<tag>]` and change nothing.
- [ ] FL-DONE-006: If more than one entry matches the resolved color or tag, then `gbiv done` shall exit `1` with an error listing the matching entries and change nothing.
- [ ] FL-DONE-007: When the entry is resolved, `gbiv done` shall load extensions and fire `before_done` with the entry's color and text, and if any handler vetoes, exit `1` leaving the entry in place (EXT-EVT-004).
- [ ] FL-DONE-008: When no handler vetoes, `gbiv done` shall remove the entry with `remove_gbiv_entry` (FL-MUTATE-016), then print `done <color>: <text>` to stdout, then fire `after_done`, and exit `0`.
- [ ] FL-DONE-009: `gbiv done` shall not reset, check out or otherwise modify any worktree or branch.
- [ ] FL-DONE-010: Before resolving the `gbiv done` argument, the system shall remove one pair of surrounding square brackets if present, so `gbiv done [gbv-1ht]` behaves like `gbiv done gbv-1ht`.
- [ ] FL-DONE-011: When the `gbiv done` argument is both a palette name and a text tag carried by some entry, the system shall resolve it as the color (FL-DONE-002).
