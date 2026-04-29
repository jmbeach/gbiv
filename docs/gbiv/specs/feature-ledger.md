# Feature Ledger

Specs for GBIV.md parsing, mutation, and the mark command.

**Component LLD**: `docs/gbiv/llds/feature-ledger.md`

## Parsing

- [x] FL-PARSE-001: When a GBIV.md file contains a line starting with `- `, the system shall treat that line as a feature entry.
- [x] FL-PARSE-002: When a feature line starts with `- [`, the system shall extract the content between the first `[` and `]` as the feature's tag.
- [x] FL-PARSE-003: When a feature line has a tag and is followed by a second bracketed token containing `done`, the system shall set the feature's status to `"done"`.
- [x] FL-PARSE-004: When a feature line has a tag and is followed by a second bracketed token containing `in-progress`, the system shall set the feature's status to `"in-progress"`.
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

- [x] FL-MUTATE-001: When `set_gbiv_feature_status` is called with a color and `Some(status)`, the system shall find the first entry whose tag matches the color and insert or replace the status bracket.
- [x] FL-MUTATE-002: When `set_gbiv_feature_status` is called with a color and `None`, the system shall find the first entry whose tag matches the color and remove any existing status bracket.
- [x] FL-MUTATE-003: When `set_gbiv_feature_status` is called with a color that has no matching entry, the system shall return an error containing the color name.
- [x] FL-MUTATE-004: When `set_gbiv_feature_status` modifies an entry, the system shall preserve all note lines attached to that entry.
- [x] FL-MUTATE-005: When `set_gbiv_feature_status` modifies an entry, the system shall preserve the original trailing newline of the file.
- [x] FL-MUTATE-006: When `set_gbiv_feature_status` encounters entries past a `---` separator, the system shall not modify them even if they match the color.
- [x] FL-MUTATE-007: When `remove_gbiv_features_by_tag` is called with a tag, the system shall remove all entries matching that tag and their attached notes.
- [x] FL-MUTATE-008: When `remove_gbiv_features_by_tag` removes entries, the system shall preserve the `---` separator and all content after it.
- [x] FL-MUTATE-009: When `remove_gbiv_features_by_tag` removes an entry that was followed by a blank line, the system shall also remove that blank line to avoid stray whitespace.
- [x] FL-MUTATE-010: When `remove_gbiv_features_by_tag` is called on a file with no matching entries, the system shall leave the file unchanged.
- [x] FL-MUTATE-011: When `remove_gbiv_features_by_tag` is called on a missing file, the system shall return Ok without error.

## Mark Command

- [x] FL-MARK-001: When `gbiv mark --done <color>` is invoked, the system shall set the status of the matching GBIV.md entry to `[done]`.
- [x] FL-MARK-002: When `gbiv mark --in-progress <color>` is invoked, the system shall set the status of the matching GBIV.md entry to `[in-progress]`.
- [x] FL-MARK-003: When `gbiv mark --unset <color>` is invoked, the system shall remove the status bracket from the matching GBIV.md entry.
- [x] FL-MARK-004: When `gbiv mark` is invoked without a color argument, the system shall infer the color from the current working directory via `infer_color_from_path`.
- [x] FL-MARK-005: When `gbiv mark` cannot determine a color (no argument and inference fails), the system shall return an error mentioning inability to infer color.
- [x] FL-MARK-006: When `gbiv mark --done` is invoked and no GBIV.md entry exists for the color, the system shall return an error.
- [x] FL-MARK-007: When `gbiv mark --in-progress` is invoked and no GBIV.md entry exists for the color, the system shall return an error.
- [x] FL-MARK-008: When `gbiv mark --unset` is invoked and no GBIV.md entry exists for the color, the system shall succeed as a no-op.
- [x] FL-MARK-009: When `gbiv mark --unset` is invoked on an entry that already has no status, the system shall succeed as a no-op.
- [x] FL-MARK-010: When `gbiv mark --done` replaces an existing `[in-progress]` status, the system shall produce an entry with only the `[done]` bracket.
- [x] FL-MARK-011: When a positional argument matches a status keyword (`done`, `in-progress`, `unset`), the system shall reject it with a hint to use the flag form instead.
