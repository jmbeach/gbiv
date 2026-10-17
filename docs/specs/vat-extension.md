# vat Extension

Specs for the bundled vat extension: the Lua extension that claims a vat task on
`gbiv start` and completes it on `gbiv done`. It runs inside the extension
runtime (`docs/specs/extension-runtime.md`) and uses only its public host API.
vat's own format and semantics are defined by the vat skill
(`.claude/skills/vat/SKILL.md`) and `jmbeach/vat`; these specs cover only what
the extension does with them.

**Component LLD**: `docs/llds/vat-extension.md`

## Packaging and Configuration

- [ ] **VATX-PKG-001**: The vat extension shall ship as `extensions/vat/` in the gbiv repository, with a manifest declaring `name = "vat"`, `api_version = 1` and capabilities `fs`, `git` and `env`.
- [ ] **VATX-PKG-002**: The vat extension shall define `before_start` and `before_done` handlers and no `after_start` or `after_done` handlers.
- [ ] **VATX-CFG-001**: The vat extension shall locate the backlog directory from its `backlog` option, defaulting to `backlog`, resolved like a grant path (relative to the main repo).
- [ ] **VATX-CFG-002**: When the vat extension needs the claiming user's name, it shall use its `user` option if set, otherwise `user.name` from vat's user config file (`$XDG_CONFIG_HOME/vat/config.toml` if `XDG_CONFIG_HOME` is readable and set, else `~/.config/vat/config.toml`).
- [ ] **VATX-CFG-003**: If no user name can be determined when claiming, then the vat extension shall veto with `set user.name first: vat config set user.name <name>`.

## Recognizing vat IDs

- [ ] **VATX-ID-001**: When the vat extension receives a `gbiv start` argument, it shall treat the argument as a candidate vat ID only if, after trimming, the whole argument matches `<3 Crockford base32 chars>-<3 Crockford base32 chars>`, case-insensitively.
- [ ] **VATX-ID-002**: The vat extension shall treat a candidate ID as belonging to this backlog only when its prefix equals `project.id` from `<backlog>/vat.toml`, compared case-insensitively.
- [ ] **VATX-ID-003**: When the `gbiv start` argument is not a candidate, or is a candidate with another prefix, the vat extension's `before_start` shall return `nil` and change nothing.
- [ ] **VATX-ID-004**: If `<backlog>/vat.toml` is missing or has no valid `project.id`, then the vat extension shall veto any candidate argument with vat's init-pointer message, and shall still return `nil` for non-candidate arguments.
- [ ] **VATX-ID-005**: The vat extension shall normalize IDs to lowercase for lookup, in the entry text it returns, and when reading an ID from `ctx.text`.

## `before_start` (claim)

- [ ] **VATX-START-001**: If this backlog's ID is not on any bullet in `backlog.md`, or `backlog.md` is missing, then `before_start` shall veto with `unknown id: <id>`.
- [ ] **VATX-START-002**: If this backlog's ID is on more than one bullet, then `before_start` shall veto with vat's duplicate-ID message.
- [ ] **VATX-START-003**: When the bullet has neither `[in-progress]` nor `[by:...]`, `before_start` shall insert `[in-progress]` and `[by:<user>]` in vat's canonical marker position and write the backlog, including when the bullet has `[blocked-by:...]`.
- [ ] **VATX-START-004**: When the bullet already has `[by:<user>]` naming the claiming user (compared case-sensitively), `before_start` shall leave the backlog unchanged and still name the entry.
- [ ] **VATX-START-005**: If the bullet has `[by:<other>]` naming a different user, or `[in-progress]` without any `[by:...]`, then `before_start` shall veto with `<id> already claimed by <other>` or `<id> already in progress`.
- [ ] **VATX-START-006**: After a successful claim (or VATX-START-004), `before_start` shall return `{ text = ... }` where the text is the bullet line after `- `, starting with the `[<id>]` tag, with `[in-progress]` and `[by:...]` removed and a trailing ` (see ./items/<id>.md)` rewritten to ` (see <backlog>/items/<id>.md)`, keeping every other tag and the `[<id>]` tag.

## `before_done` (complete)

- [ ] **VATX-DONE-001**: When `ctx.text` contains no bracketed tag that is this backlog's ID, `before_done` shall return and change nothing.
- [ ] **VATX-DONE-002**: When `ctx.text` contains this backlog's ID as a bracketed tag, `before_done` shall use the first such tag as the task ID.
- [ ] **VATX-DONE-003**: When the task's bullet exists, `before_done` shall complete it regardless of who claimed it: append the ID to `.used-ids` if absent, then rewrite `backlog.md` with the bullet and any note lines under it removed (dropping one adjacent blank line if removal would leave a double blank) and `[blocked-by:<id>]` stripped from every other bullet, then delete `items/<id>.md` if it exists.
- [ ] **VATX-DONE-004**: When the task's bullet is absent and its ID is in `.used-ids`, `before_done` shall change nothing and not veto.
- [ ] **VATX-DONE-005**: If the task's bullet is absent (including when `backlog.md` is missing) and its ID is not in `.used-ids`, then `before_done` shall veto with `unknown id: <id>`; when the ID is in `.used-ids`, VATX-DONE-004 applies even if `backlog.md` is missing.
- [ ] **VATX-DONE-006**: If the task's ID is on more than one bullet, then `before_done` shall veto with vat's duplicate-ID message.
- [ ] **VATX-DONE-007**: If `<backlog>/vat.toml` is missing or has no valid `project.id`, then `before_done` shall veto with vat's init-pointer message when `ctx.text` contains a bracketed tag matching the vat ID shape, and otherwise return and change nothing.

## Backlog File Handling

- [ ] **VATX-FMT-001**: When reading `backlog.md`, the vat extension shall normalize CRLF line endings to LF, and every file it writes shall end with exactly one `\n`.
- [ ] **VATX-FMT-002**: If `backlog.md` frontmatter declares a `version` greater than `1`, then the vat extension shall veto with vat's upgrade message and write nothing.
- [ ] **VATX-FMT-003**: The vat extension shall preserve frontmatter, preamble, the freeform region after the first standalone `---` line, and every bullet it does not change, byte for byte (after VATX-FMT-001's line-ending normalization).
- [ ] **VATX-FMT-004**: When reading a bullet's markers, the vat extension shall match `[<id>]`, `[in-progress]`, `[by:<name>]` and `[blocked-by:<id>]` left to right and treat the first other bracketed token, and everything after it, as the title, preserving markers it does not know (such as `[parent:<id>]`) in place.

## Nested-repo Backlogs

- [ ] **VATX-GIT-001**: When `<backlog>/.git` does not exist, the vat extension shall edit the backlog files in place and run no git commands.
- [ ] **VATX-GIT-002**: When `<backlog>/.git` exists, before changing anything the vat extension shall run `git status --porcelain`, `git fetch` and `git merge-base --is-ancestor HEAD @{u}` in the backlog directory.
- [ ] **VATX-GIT-003**: If the nested backlog has no upstream, or the guard's `git fetch` fails, then the vat extension shall veto with git's error output and change nothing.
- [ ] **VATX-GIT-004**: When the nested backlog's working tree is dirty or `HEAD` is ahead of upstream, the vat extension shall edit the files without committing or pushing and warn `<id>: not pushed; backlog has uncommitted changes` or `<id>: not pushed; backlog has unpushed local commits`.
- [ ] **VATX-GIT-005**: When the nested backlog is clean and in sync, the vat extension shall loop up to 5 attempts of: `git fetch` (skipped on the first attempt), `git reset --hard @{u}`, re-read the files, re-check the outcome on fresh state, apply the change, `git commit -m "vat start <id>"` or `"vat done <id>"`, and `git push`.
- [ ] **VATX-GIT-006**: When a push in the claim loop is rejected (git's stderr contains `[rejected]`, `non-fast-forward` or `fetch first`), the vat extension shall run `git reset --hard @{u}`, sleep 0.5 × 2^(attempt−1) seconds plus up to 0.5 seconds of random jitter, and retry.
- [ ] **VATX-GIT-007**: If the claim loop's push is rejected on all 5 attempts, then the vat extension shall veto with `lost race claiming <id>`.
- [ ] **VATX-GIT-008**: If any other git command in the claim loop fails, or the handler's time runs out during the loop, then the vat extension shall run `git reset --hard @{u}` (when an upstream exists) so no local commit remains, and veto with git's error output.
- [ ] **VATX-GIT-009**: When the fresh state inside the claim loop shows the task claimed by another user (for `start`), the vat extension shall veto with `lost claim: <id> already claimed by <name>`; when it shows the task claimed by this user (for `start`) or absent and in `.used-ids` (for `done`), it shall succeed without committing.

## References

- LLD: `docs/llds/vat-extension.md`
- Companion: `docs/specs/extension-runtime.md` (events, host API)
- External: `.claude/skills/vat/SKILL.md` (vat format, atomic claim loop); `jmbeach/vat` golden fixtures
