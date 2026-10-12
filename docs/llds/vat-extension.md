# vat Extension

**Created**: 2026-10-12
**Status**: Draft (greenfield)

## Context and Design Philosophy

The vat extension is the bundled, reference extension: it makes
`gbiv start <vat-id>` claim the task in the vat backlog and `gbiv done <vat-id>`
complete it. It is also the worked example extension authors copy from, so it
uses only the public host API (`docs/llds/extension-runtime.md`) and nothing
gbiv-internal.

It is written in Lua and operates directly on vat's plain-markdown files, with no
dependency on the `vat` binary. That makes it the third implementation of vat's
format, beside the `vat-cli` crate and the vat skill. To keep the three from
drifting, it implements only what `start` and `done` need, and its tests run
against fixtures copied from vat's own golden tests.

Source of truth for vat's format and semantics: the vat skill
(`.claude/skills/vat/SKILL.md`) and `jmbeach/vat`. Where this document
summarizes them, they win on any disagreement.

## Files

Bundled at `extensions/vat/` in the gbiv repo, compiled into the binary, and
copied out by `gbiv install-extension vat`:

```
extensions/vat/
  extension.toml   # name = "vat", api_version = 1, capabilities = ["fs", "git", "env"]
  init.lua         # hook table; wires the modules below together
  backlog.lua      # parse / edit / serialize backlog.md (start + done subset)
  claim.lua        # nested-repo git claim loop
```

## Configuration

```toml
[extensions.vat]
paths = ["backlog", "~/.config/vat"]   # default grants written by install-extension
env = ["XDG_CONFIG_HOME"]              # so vat's user config is found where vat looks
backlog = "backlog"                    # backlog directory (default "backlog")
user = "jared"                         # optional; overrides vat's user config
```

- `backlog` resolves like any grant path (against the main repo). It must lie
  inside a granted path.
- The claiming user's name comes from `user` if set, otherwise from vat's user
  config (`$XDG_CONFIG_HOME/vat/config.toml`, else `~/.config/vat/config.toml`,
  key `user.name`, parsed with `gbiv.toml.decode`). If `$XDG_CONFIG_HOME` points
  outside the granted paths, the read fails and the extension falls back to
  requiring `user`. If neither yields a name, `before_start` on a vat ID vetoes
  with vat's own message: `set user.name first: vat config set user.name <name>`.

## Recognizing a vat ID

The extension only acts on arguments and entries that are vat IDs for *this*
backlog. It reads `project.id` from `<backlog>/vat.toml`.

An argument is a **candidate** when, after trimming, the *whole* argument matches
vat's ID shape: `<3 Crockford base32 chars>-<3 Crockford base32 chars>`,
case-insensitive (`"gbv-1ht fix"` and `"[gbv-1ht]"` are not candidates). It is
**this backlog's** when its prefix equals `project.id`. If `vat.toml` is missing
or invalid, every candidate is vetoed with vat's init-pointer message, since
ownership can't be decided; non-candidates still pass through.

| `gbiv start <arg>` where `arg` is… | Extension does |
|---|---|
| not a candidate (e.g. `"Fix login bug"`) | returns `nil`; the entry is free text |
| a candidate with another prefix | returns `nil` (another extension may own it) |
| this backlog's ID, not in `backlog.md` | vetoes: `unknown id: <id>` |
| this backlog's ID, `backlog.md` missing or holding the ID twice | vetoes with vat's message |
| this backlog's ID, present | claims it (below) and names the entry |

IDs are normalized to lowercase before lookup, in the returned text, and when
reading the ID from `ctx.text` in `before_done` (so a hand-written `[GBV-1HT]` entry still
matches). User names in `[by:]` are compared case-sensitively, as vat does.

Like `vat start`, claiming a bullet that has `[blocked-by:...]` is allowed.

## `before_start` — claim

Mirrors `vat start` semantics, with one deliberate difference for tasks the
user already holds:

1. Read `user` (see Configuration).
2. Locate the bullet by `[id]` in the parsed region of `backlog.md`.
3. If the bullet has `[by:<name>]` with **this user's name**, the task is already
   ours: make no change and go to step 5. (`vat start` would refuse here; gbiv
   allows it so a task claimed earlier with `vat start` can still be put on a
   color.)
4. If it has `[by:<other>]` or a bare `[in-progress]`, veto:
   `<id> already claimed by <other>` (or `<id> already in progress`). Otherwise
   insert `[in-progress]` and `[by:<user>]` in canonical marker position and write
   the file (or run the claim loop; see Nested-repo backlogs).
5. Return `{ text = <text> }`: the bullet line after `- `, as written after the
   claim, with `[in-progress]` and `[by:...]` removed (redundant once the task is
   on a color) and the pointer ` (see ./items/<id>.md)` rewritten relative to the
   main repo (` (see backlog/items/<id>.md)`), the same way the `gbiv-sync` skill
   copies bullets. Every other tag stays: `[gbv-1ht] [blocked-by:gbv-x2v] Title`.
   The leading `[<id>]` is what lets `gbiv done <id>` find the entry later.

## `before_done` — complete

Mirrors `vat done`:

1. Find this backlog's ID in `ctx.text`: the first bracketed tag that is a
   candidate ID with this backlog's prefix. None → do nothing (the entry isn't a
   vat task). Like `vat done`, this completes the task whoever claimed it, or if
   nobody did.
2. Locate the bullet. If it's absent and the ID is in `.used-ids`, the task was
   already completed outside gbiv: do nothing, so the entry can still come off
   its color. If it's absent and not in `.used-ids` (or `backlog.md` is missing),
   veto with `unknown id: <id>`, as `vat done` does. An ID on two bullets vetoes
   with vat's message.
3. Remove the bullet line (dropping one adjacent blank line if removal would leave
   a double blank), delete `items/<id>.md` if present, append `<id>` to
   `.used-ids` if absent, and strip `[blocked-by:<id>]` from every remaining
   bullet.
4. Write the files (or run the claim loop), in this order: append to
   `.used-ids`, then rewrite `backlog.md`, then delete the item file. A failure
   partway leaves the ID retired rather than reusable.

`after_start` and `after_done` are not implemented.

## Backlog Handling (`backlog.lua`)

A subset of vat's format, enough for `start` and `done`:

- Normalize CRLF to `\n` on read; every write ends with a single `\n`.
- Optional YAML frontmatter (lines of `---`); if `version` > 1, veto with vat's
  upgrade message. Unknown frontmatter is preserved verbatim.
- The body splits at the first standalone `---` line into a parsed region and a
  freeform region; the freeform region is preserved byte for byte.
- A bullet is a column-0 line starting with `- `. Markers are matched left to
  right: `[<id>]`, `[in-progress]`, `[by:<name>]`, `[blocked-by:<id>]`; the first
  bracketed token that isn't a known marker starts the title. Other markers vat
  writes (for example `[parent:<id>]`, seen in gbiv's own backlog) are preserved
  in place as part of the title text.
- Only the bullets being changed are reserialized; every other line is written
  back exactly as read.

The extension never assigns IDs, extracts notes or reorders markers on lines it
doesn't touch; those are `vat sync`'s job.

## Nested-repo Backlogs (`claim.lua`)

When `<backlog>/.git` exists (a directory, or a file for a submodule or
worktree), the backlog is its own git repository, as gbiv's own backlog is.
The extension then follows the vat skill's atomic claim loop exactly, using
`gbiv.git`:

1. **Guard, once.** `git status --porcelain`, `git fetch`, then
   `git merge-base --is-ancestor HEAD @{u}`.
   - Clean and synced → run the loop, reusing this fetch as the first refresh.
   - Dirty, or ahead of upstream → edit the files locally, don't commit or push,
     and warn: `<id>: not pushed; backlog has uncommitted changes` (or
     `unpushed local commits`).
   - No upstream, or `fetch` fails → veto with git's raw error.
2. **Loop**, at most 5 attempts: `git fetch` (skipped on the first round,
   which reuses the guard's fetch), `git reset --hard @{u}`, re-read the files,
   check the terminal precondition (below), apply the change, commit with the
   fixed message `vat start <id>` / `vat done <id>`, and `git push`.
   - Push succeeds → done.
   - Push rejected (git's stderr contains `[rejected]`, `non-fast-forward` or
     `fetch first`; the runtime runs git with `LC_ALL=C`) → `git reset --hard
     @{u}`, sleep 0.5 × 2^(attempt−1) s plus up to 0.5 s of jitter, and retry.
   - Any other push failure → veto with git's raw error.
   - 5 rejections → veto: `lost race claiming <id>`.
   - Any other failure inside the loop (a failed `git commit` from a hook or
     missing identity, a `fetch` error, the handler running out of time) →
     `git reset --hard @{u}` so no local commit is left behind, then veto with
     git's raw error.
3. **Terminal preconditions on fresh state:** for `start`, already claimed by
   someone else → veto (`lost claim: <id> already claimed by <name>`); already
   claimed by this user → success with no commit. For `done`, absent and in
   `.used-ids` → success with no commit; absent and not in `.used-ids` → veto
   `unknown id: <id>`. Both match the vat skill.

A non-nested backlog (no `.git` inside it) is edited in place with no git at all,
exactly like `vat` itself: committing is left to the user.

## Testing Approach

- Unit tests of `backlog.lua` run in the gbiv test suite through the real
  runtime, against fixtures copied from `jmbeach/vat`'s
  `tests/fixtures/commands/{start,done}/` (copied into gbiv's test fixtures,
  with their origin noted).
- The claim loop is tested against a temporary bare remote with two clones, one
  of which pushes first, to force a rejection.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Implementation | Lua on vat's files | Call the `vat` binary; depend on a vat Rust crate | Extensions are Lua (HLD); vat's logic is small. Calling `vat` would need process exec in the sandbox, and vat-cli has no library target or JSON output. |
| Format coverage | Only what `start`/`done` touch | Full vat (sync, block, init…) | Smallest possible third copy of the format; less drift. |
| Already claimed by this user | Accept, no change | Refuse like `vat start` | A task claimed earlier with `vat start` should still go on a color. |
| `done` on an ID missing from the backlog | Veto `unknown id`, unless the ID is in `.used-ids` (already done) | Always warn and proceed | Match vat. The `.used-ids` case keeps a task completed outside gbiv from getting stuck on its color. |
| Entry text | The bullet's tags and title, minus `[in-progress]` / `[by:]` | Title only | Keeps vat's other tags (`[blocked-by:]`, `[parent:]`) visible on the color, as `gbiv-sync` does. |
| Claim loop | Copy of the vat skill's first-push-wins loop | Plain commit + push; no git | Matches the skill exactly, so concurrent agents claiming through gbiv and through the skill behave the same. |
| Default grants | `backlog` and `~/.config/vat` | Ask on install | Covers the common layout and the user name; editable in config. |

## Open Questions & Future Decisions

### Deferred
1. Whether `vat-cli` should ship these same fixtures as a reusable conformance
   suite, so all three implementations test against one source.

## References

- `docs/llds/extension-runtime.md` — host API and events
- `.claude/skills/vat/SKILL.md` — vat format and the atomic claim loop
- `jmbeach/vat` — `vat-cli`, golden fixtures under `tests/fixtures/commands/`
