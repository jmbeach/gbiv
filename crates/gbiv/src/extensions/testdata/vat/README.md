# vat extension test fixtures

`start/` and `done/` are copied from the public repo
[jmbeach/vat](https://github.com/jmbeach/vat), `tests/fixtures/commands/start/`
and `tests/fixtures/commands/done/` (vat's own golden-test inputs), for gbv-1ht.
They are test data, kept byte for byte as upstream ships them, except that each
directory gains a `vat.toml` (`[project]` / `id = "vat"`): upstream's start/done
fixtures ship without one, but the extension needs `project.id` to decide which
IDs belong to the backlog.

Used by `crates/gbiv/src/extensions/vat_tests.rs`.
