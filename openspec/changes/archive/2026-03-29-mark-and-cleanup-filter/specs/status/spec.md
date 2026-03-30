## MODIFIED Requirements

### Requirement: Display GBIV.md features in status output
After displaying the worktree rows, `gbiv status` SHALL display a GBIV.md section when the file is present and contains at least one feature. The section SHALL be separated from the worktree output by a blank line and a dim header line.

Each worktree row SHALL include the worktree's lifecycle status (`in-progress`, `done`) when the corresponding GBIV.md entry has a status tag. The status SHALL be displayed after the color name (e.g., `red [done]  my-feature-branch ...`).

#### Scenario: GBIV.md present with features
- **WHEN** `GBIV.md` exists and contains at least one feature
- **THEN** `gbiv status` prints a blank line, a dim `"GBIV.md"` header, then one row per feature

#### Scenario: GBIV.md absent
- **WHEN** `GBIV.md` does not exist in the gbiv root
- **THEN** `gbiv status` output is identical to current behavior (no GBIV.md section)

#### Scenario: GBIV.md present but empty
- **WHEN** `GBIV.md` exists but contains no features
- **THEN** no GBIV.md section is printed

#### Scenario: Worktree with in-progress status
- **WHEN** a color worktree's GBIV.md entry has `[in-progress]` status tag
- **THEN** the worktree row displays `[in-progress]` after the color name

#### Scenario: Worktree with done status
- **WHEN** a color worktree's GBIV.md entry has `[done]` status tag
- **THEN** the worktree row displays `[done]` after the color name

#### Scenario: Worktree with no status
- **WHEN** a color worktree's GBIV.md entry has no status tag
- **THEN** the worktree row displays no status indicator (same as current behavior)
