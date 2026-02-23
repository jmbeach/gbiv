## ADDED Requirements

### Requirement: Display GBIV.md features in status output
After displaying the worktree rows, `gbiv status` SHALL display a GBIV.md section when the file is present and contains at least one feature. The section SHALL be separated from the worktree output by a blank line and a dim header line.

#### Scenario: GBIV.md present with features
- **WHEN** `GBIV.md` exists and contains at least one feature
- **THEN** `gbiv status` prints a blank line, a dim `"GBIV.md"` header, then one row per feature

#### Scenario: GBIV.md absent
- **WHEN** `GBIV.md` does not exist in the gbiv root
- **THEN** `gbiv status` output is identical to current behavior (no GBIV.md section)

#### Scenario: GBIV.md present but empty
- **WHEN** `GBIV.md` exists but contains no features
- **THEN** no GBIV.md section is printed

### Requirement: Feature rows show tag and description
Each feature row SHALL display the tag (if present) colored with the corresponding ANSI color, followed by the feature description. Untagged features SHALL be displayed with a dim `"backlog"` label in place of the tag.

#### Scenario: Tagged feature row
- **WHEN** a feature has tag `"red"` and description `"My feature"`
- **THEN** the row displays the word `"red"` in red ANSI color followed by `"My feature"`

#### Scenario: Untagged feature row
- **WHEN** a feature has no tag and description `"Backlog item"`
- **THEN** the row displays a dim `"backlog"` label followed by `"Backlog item"`
