Minimal SQLite recovery sources vendored from upstream SQLite:

- `ext/recover/sqlite3recover.c`
- `ext/recover/sqlite3recover.h`
- `ext/recover/dbdata.c`

`sqlite3.h` is a comment-stripped copy from the `libsqlite3-sys` 0.37.0
bundled SQLite 3.51.3 source (`SQLITE_SOURCE_ID`
`2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618`)
so Cargo and Bazel compile these extension files with matching public SQLite
declarations without adding a build-dependency that would compile SQLite a
second time.

These files implement SQLite's recover extension without invoking the
`sqlite3` command-line shell. They are compiled into `codex-state` and link
against the same `libsqlite3-sys` library that SQLx uses.

The recovery API is not exposed by `libsqlite3-sys`, and no published Rust
crate in the dependency graph provides this extension without also building a
second SQLite copy. When updating `libsqlite3-sys`, refresh these files from
the matching SQLite source snapshot and keep the vendored set limited to the
recover/dbdata extension sources plus the trimmed public header.
