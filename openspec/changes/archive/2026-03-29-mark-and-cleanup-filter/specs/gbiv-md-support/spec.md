## MODIFIED Requirements

### Requirement: GBIV.md parsing supports optional status tag
The parser SHALL recognize an optional second bracket tag after the color tag as a status value. The format is `- [color] [status] Description`. Valid status values are `in-progress` and `done`. Unrecognized second bracket values SHALL be treated as part of the description (not a status).

#### Scenario: Parse entry with status tag
- **WHEN** GBIV.md contains `- [red] [done] Fix critical bug`
- **THEN** the parser returns a feature with tag `red`, status `done`, and description `Fix critical bug`

#### Scenario: Parse entry with in-progress status
- **WHEN** GBIV.md contains `- [red] [in-progress] Fix critical bug`
- **THEN** the parser returns a feature with tag `red`, status `in-progress`, and description `Fix critical bug`

#### Scenario: Parse entry without status tag
- **WHEN** GBIV.md contains `- [red] Fix critical bug`
- **THEN** the parser returns a feature with tag `red`, status `None`, and description `Fix critical bug`

#### Scenario: Unrecognized second bracket is not a status
- **WHEN** GBIV.md contains `- [red] [wip] Fix critical bug`
- **THEN** the parser returns a feature with tag `red`, status `None`, and description `[wip] Fix critical bug`

#### Scenario: Backward compatibility with no tags
- **WHEN** GBIV.md contains `- Fix critical bug`
- **THEN** the parser returns a feature with tag `None`, status `None`, and description `Fix critical bug`

### Requirement: GBIV.md supports setting status on an entry
The system SHALL provide a function to add or replace the status tag on a GBIV.md feature entry identified by its color tag.

#### Scenario: Add status where none exists
- **WHEN** the function is called with color `red` and status `done` and GBIV.md contains `- [red] Fix critical bug`
- **THEN** GBIV.md is updated to `- [red] [done] Fix critical bug`

#### Scenario: Replace existing status
- **WHEN** the function is called with color `red` and status `done` and GBIV.md contains `- [red] [in-progress] Fix critical bug`
- **THEN** GBIV.md is updated to `- [red] [done] Fix critical bug`

#### Scenario: Multiple entries with same color
- **WHEN** GBIV.md contains multiple `[red]` entries and the function is called with color `red`
- **THEN** all `[red]` entries are updated with the new status
