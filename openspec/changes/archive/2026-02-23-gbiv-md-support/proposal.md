## Why

Users want a lightweight way to maintain a feature backlog in a `GBIV.md` file alongside their worktrees, without being forced into any rigid structure. Currently there is no tooling to read this file, so features defined there are invisible to `gbiv status`.

## What Changes

- Add parsing of a `GBIV.md` file in the repository root
- Surface parsed features (including untagged backlog items) in `gbiv status` as informational output
- Support a flexible, human-friendly file format with color tags, inline notes, and a stop marker

## Capabilities

### New Capabilities
- `gbiv-md-support`: Parse a `GBIV.md` file and display its features in `gbiv status`. The format uses `- ` to denote features, optional `[color]` tags, sub-line notes, and `---` to stop analysis.

### Modified Capabilities
- `status`: `gbiv status` will now include GBIV.md features when the file is present.

## Impact

- New parser module for GBIV.md format
- `gbiv status` output extended to show GBIV.md features when the file exists
- No breaking changes — the file is optional and status falls back gracefully if absent
