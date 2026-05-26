# `libsqlite3-sys` SQLite Source Override

This directory is based on the crates.io `libsqlite3-sys` `0.30.1` package.
SQLx `0.8.6` requires the `0.30.x` Rust API, so Codex retains that package
interface rather than adopting a prerelease SQLx dependency.

The `sqlite3/` directory is copied verbatim from the upstream `rusqlite`
repository at commit
[`7bd509863f304a40ba6be1c1e3ad70a221d50490`](https://github.com/rusqlite/rusqlite/commit/7bd509863f304a40ba6be1c1e3ad70a221d50490),
which updates the bundled SQLite amalgamation to official SQLite `3.51.3`.
SQLite `3.51.3` contains the fix for the WAL-reset corruption bug affecting
WAL-mode databases through SQLite `3.51.2`.

The `codex-state` runtime test `linked_sqlite_has_wal_reset_bug_fix` queries
`sqlite_version()` from the actually linked library and rejects vulnerable
SQLite releases. Remove this override once a stable SQLx dependency line used
by Codex bundles a SQLite release accepted by that guard.
