### Requirement: Parse feature lines
The parser SHALL treat any line beginning with `"- "` (dash followed by a space) as a feature entry. The remainder of the line is the feature text. If the feature text begins with a bracketed tag (e.g., `[red]`), the tag SHALL be extracted and stored separately; the remaining text after the tag and any leading whitespace is the feature description.

#### Scenario: Plain feature line
- **WHEN** a line is `"- My feature"`
- **THEN** a feature is produced with no tag and description `"My feature"`

#### Scenario: Tagged feature line
- **WHEN** a line is `"- [red] My feature"`
- **THEN** a feature is produced with tag `"red"` and description `"My feature"`

#### Scenario: Unrecognized tag is preserved
- **WHEN** a line is `"- [purple] My feature"`
- **THEN** a feature is produced with tag `"purple"` and description `"My feature"` (no error)

### Requirement: Parse notes for features
Any line that follows a feature line and does NOT begin with `"- "` and is not the stop marker SHALL be treated as a note belonging to the most recent feature.

#### Scenario: Single note line
- **WHEN** a feature line is followed by a line `"  Extra context"`
- **THEN** `"  Extra context"` is stored as a note on that feature

#### Scenario: Multiple note lines
- **WHEN** two consecutive non-feature lines follow a feature
- **THEN** both lines are stored as notes on that feature, in order

#### Scenario: Note before any feature is ignored
- **WHEN** a non-feature line appears before the first feature in the file
- **THEN** that line is silently ignored

### Requirement: Stop marker terminates parsing
When a line consisting solely of `"---"` is encountered, the parser SHALL stop processing and discard all remaining content.

#### Scenario: Stop marker encountered
- **WHEN** a line `"---"` appears after some features
- **THEN** features above the marker are returned and everything below is ignored

#### Scenario: No stop marker
- **WHEN** the file has no `"---"` line
- **THEN** all features in the file are returned

### Requirement: Missing file returns empty result
If `GBIV.md` does not exist at the expected path, the parser SHALL return an empty list without error.

#### Scenario: File absent
- **WHEN** `GBIV.md` does not exist in the gbiv root
- **THEN** an empty feature list is returned and no error is emitted

### Requirement: Empty file returns empty result
If `GBIV.md` exists but contains no feature lines (e.g., only blank lines or notes before any feature), the parser SHALL return an empty list.

#### Scenario: File with no feature lines
- **WHEN** `GBIV.md` contains only blank lines
- **THEN** an empty feature list is returned
