Vendored from upstream SQLite Fossil trunk:

- `ext/recover/sqlite3recover.c`
- `ext/recover/sqlite3recover.h`
- `ext/recover/dbdata.c`

`sqlite3.h` is copied from the `libsqlite3-sys` 0.30.1 bundled SQLite
source so Cargo and Bazel compile these extension files with matching public
SQLite declarations without adding a build-dependency that would compile
SQLite a second time.

These files implement SQLite's recover extension without invoking the
`sqlite3` command-line shell. They are compiled into `codex-state` and link
against the same `libsqlite3-sys` library that SQLx uses.
