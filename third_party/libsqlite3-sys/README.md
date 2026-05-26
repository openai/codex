# Rusqlite

[![Latest Version](https://img.shields.io/crates/v/rusqlite.svg)](https://crates.io/crates/rusqlite)
[![Documentation](https://docs.rs/rusqlite/badge.svg)](https://docs.rs/rusqlite)
[![Build Status (GitHub)](https://github.com/rusqlite/rusqlite/workflows/CI/badge.svg)](https://github.com/rusqlite/rusqlite/actions)
[![Build Status (AppVeyor)](https://ci.appveyor.com/api/projects/status/github/rusqlite/rusqlite?branch=master&svg=true)](https://ci.appveyor.com/project/rusqlite/rusqlite)
[![Code Coverage](https://codecov.io/gh/rusqlite/rusqlite/branch/master/graph/badge.svg)](https://codecov.io/gh/rusqlite/rusqlite)
[![Dependency Status](https://deps.rs/repo/github/rusqlite/rusqlite/status.svg)](https://deps.rs/repo/github/rusqlite/rusqlite)
[![Discord Chat](https://img.shields.io/discord/927966344266256434.svg?logo=discord)](https://discord.gg/nFYfGPB8g4)

Rusqlite is an ergonomic wrapper for using SQLite from Rust.

Historically, the API was based on the one from [`rust-postgres`](https://github.com/sfackler/rust-postgres). However, the two have diverged in many ways, and no compatibility between the two is intended.

## Usage

In your Cargo.toml:

```toml
[dependencies]
# `bundled` causes us to automatically compile and link in an up to date
# version of SQLite for you. This avoids many common build issues, and
# avoids depending on the version of SQLite on the users system (or your
# system), which may be old or missing. It's the right choice for most
# programs that control their own SQLite databases.
#
# That said, it's not ideal for all scenarios and in particular, generic
# libraries built around `rusqlite` should probably not enable it, which
# is why it is not a default feature -- it could become hard to disable.
rusqlite = { version = "0.32.0", features = ["bundled"] }
```

Simple example usage:

```rust
use rusqlite::{Connection, Result};

#[derive(Debug)]
struct Person {
    id: i32,
    name: String,
    data: Option<Vec<u8>>,
}

fn main() -> Result<()> {
    let conn = Connection::open_in_memory()?;

    conn.execute(
        "CREATE TABLE person (
            id    INTEGER PRIMARY KEY,
            name  TEXT NOT NULL,
            data  BLOB
        )",
        (), // empty list of parameters.
    )?;
    let me = Person {
        id: 0,
        name: "Steven".to_string(),
        data: None,
    };
    conn.execute(
        "INSERT INTO person (name, data) VALUES (?1, ?2)",
        (&me.name, &me.data),
    )?;

    let mut stmt = conn.prepare("SELECT id, name, data FROM person")?;
    let person_iter = stmt.query_map([], |row| {
        Ok(Person {
            id: row.get(0)?,
            name: row.get(1)?,
            data: row.get(2)?,
        })
    })?;

    for person in person_iter {
        println!("Found person {:?}", person.unwrap());
    }
    Ok(())
}
```

### Supported SQLite Versions

The base `rusqlite` package supports SQLite version 3.14.0 or newer. If you need
support for older versions, please file an issue. Some cargo features require a
newer SQLite version; see details below.

### Optional Features

Rusqlite provides several features that are behind [Cargo
features](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section). They are:

* [`load_extension`](https://docs.rs/rusqlite/~0/rusqlite/struct.LoadExtensionGuard.html)
  allows loading dynamic library-based SQLite extensions.
* `loadable_extension` to program [loadable extension](https://sqlite.org/loadext.html) in Rust.
* [`backup`](https://docs.rs/rusqlite/~0/rusqlite/backup/index.html)
  allows use of SQLite's online backup API.
* [`functions`](https://docs.rs/rusqlite/~0/rusqlite/functions/index.html)
  allows you to load Rust closures into SQLite connections for use in queries.
* `window` for [window function](https://www.sqlite.org/windowfunctions.html) support (`fun(...) OVER ...`). (Implies `functions`.)
* [`trace`](https://docs.rs/rusqlite/~0/rusqlite/trace/index.html)
  allows hooks into SQLite's tracing and profiling APIs.
* [`blob`](https://docs.rs/rusqlite/~0/rusqlite/blob/index.html)
  gives `std::io::{Read, Write, Seek}` access to SQL BLOBs.
* [`limits`](https://docs.rs/rusqlite/~0/rusqlite/struct.Connection.html#method.limit)
  allows you to set and retrieve SQLite's per connection limits.
* `chrono` implements [`FromSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.FromSql.html)
  and [`ToSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.ToSql.html) for various
  types from the [`chrono` crate](https://crates.io/crates/chrono).
* `serde_json` implements [`FromSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.FromSql.html)
  and [`ToSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.ToSql.html) for the
  `Value` type from the [`serde_json` crate](https://crates.io/crates/serde_json).
* `time` implements [`FromSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.FromSql.html)
  and [`ToSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.ToSql.html) for various
  types from the [`time` crate](https://crates.io/crates/time).
* `url` implements [`FromSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.FromSql.html)
  and [`ToSql`](https://docs.rs/rusqlite/~0/rusqlite/types/trait.ToSql.html) for the
  `Url` type from the [`url` crate](https://crates.io/crates/url).
* `bundled` uses a bundled version of SQLite.  This is a good option for cases where linking to SQLite is complicated, such as Windows.
* `sqlcipher` looks for the SQLCipher library to link against instead of SQLite. This feature overrides `bundled`.
* `bundled-sqlcipher` uses a bundled version of SQLCipher. This searches for and links against a system-installed crypto library to provide the crypto implementation.
* `bundled-sqlcipher-vendored-openssl` allows using bundled-sqlcipher with a vendored version of OpenSSL (via the `openssl-sys` crate) as the crypto provider.
  - As the name implies this depends on the `bundled-sqlcipher` feature, and automatically turns it on.
  - If turned on, this uses the [`openssl-sys`](https://crates.io/crates/openssl-sys) crate, with the `vendored` feature enabled in order to build and bundle the OpenSSL crypto library.
* `hooks` for [Commit, Rollback](http://sqlite.org/c3ref/commit_hook.html) and [Data Change](http://sqlite.org/c3ref/update_hook.html) notification callbacks.
* `preupdate_hook` for [preupdate](https://sqlite.org/c3ref/preupdate_count.html) notification callbacks. (Implies `hooks`.)
* `unlock_notify` for [Unlock](https://sqlite.org/unlock_notify.html) notification.
* `vtab` for [virtual table](https://sqlite.org/vtab.html) support (allows you to write virtual table implementations in Rust). Currently, only read-only virtual tables are supported.
* `series` exposes [`generate_series(...)`](https://www.sqlite.org/series.html) Table-Valued Function. (Implies `vtab`.)
* [`csvtab`](https://sqlite.org/csv.html), CSV virtual table written in Rust. (Implies `vtab`.)
* [`array`](https://sqlite.org/carray.html), The `rarray()` Table-Valued Function. (Implies `vtab`.)
* `i128_blob` allows storing values of type `i128` type in SQLite databases. Internally, the data is stored as a 16 byte big-endian blob, with the most significant bit flipped, which allows ordering and comparison between different blobs storing i128s to work as expected.
* `uuid` allows storing and retrieving `Uuid` values from the [`uuid`](https://docs.rs/uuid/) crate using blobs.
* [`session`](https://sqlite.org/sessionintro.html), Session module extension. Requires `buildtime_bindgen` feature. (Implies `hooks`.)
* `extra_check` fail when a query passed to execute is readonly or has a column count > 0.
* `column_decltype` provides `columns()` method for Statements and Rows; omit if linking to a version of SQLite/SQLCipher compiled with `-DSQLITE_OMIT_DECLTYPE`.
* `collation` exposes [`sqlite3_create_collation_v2`](https://sqlite.org/c3ref/create_collation.html).
* `serialize` exposes [`sqlite3_serialize`](http://sqlite.org/c3ref/serialize.html) (3.23.0).

## Notes on building rusqlite and libsqlite3-sys

`libsqlite3-sys` is a separate crate from `rusqlite` that provides the Rust
declarations for SQLite's C API. By default, `libsqlite3-sys` attempts to find a SQLite library that already exists on your system using pkg-config, or a
[Vcpkg](https://github.com/Microsoft/vcpkg) installation for MSVC ABI builds.

You can adjust this behavior in a number of ways:

* If you use the `bundled`, `bundled-sqlcipher`, or `bundled-sqlcipher-vendored-openssl` features, `libsqlite3-sys` will use the
  [cc](https://crates.io/crates/cc) crate to compile SQLite or SQLCipher from source and
  link against that. This source is embedded in the `libsqlite3-sys` crate and
  is currently SQLite 3.46.0 (as of `rusqlite` 0.32.0 / `libsqlite3-sys`
  0.30.0).  This is probably the simplest solution to any build problems. You can enable this by adding the following in your `Cargo.toml` file:
  ```toml
  [dependencies.rusqlite]
  version = "0.32.0"
  features = ["bundled"]
  ```
* When using any of the `bundled` features, the build script will honor `SQLITE_MAX_VARIABLE_NUMBER` and `SQLITE_MAX_EXPR_DEPTH` variables. It will also honor a `LIBSQLITE3_FLAGS` variable, which can have a format like `"-USQLITE_ALPHA -DSQLITE_BETA SQLITE_GAMMA ..."`. That would disable the `SQLITE_ALPHA` flag, and set the `SQLITE_BETA` and `SQLITE_GAMMA` flags. (The initial `-D` can be omitted, as on the last one.)
* When using `bundled-sqlcipher` (and not also using `bundled-sqlcipher-vendored-openssl`), `libsqlite3-sys` will need to
  link against crypto libraries on the system. If the build script can find a `libcrypto` from OpenSSL or LibreSSL (it will consult `OPENSSL_LIB_DIR`/`OPENSSL_INCLUDE_DIR` and `OPENSSL_DIR` environment variables), it will use that. If building on and for Macs, and none of those variables are set, it will use the system's SecurityFramework instead.

* When linking against a SQLite (or SQLCipher) library already on the system (so *not* using any of the `bundled` features), you can set the `SQLITE3_LIB_DIR` (or `SQLCIPHER_LIB_DIR`) environment variable to point to a directory containing the library. You can also set the `SQLITE3_INCLUDE_DIR` (or `SQLCIPHER_INCLUDE_DIR`) variable to point to the directory containing `sqlite3.h`.
* Installing the sqlite3 development packages will usually be all that is required, but
  the build helpers for [pkg-config](https://github.com/alexcrichton/pkg-config-rs)
  and [vcpkg](https://github.com/mcgoo/vcpkg-rs) have some additional configuration
  options. The default when using vcpkg is to dynamically link,
  which must be enabled by setting `VCPKGRS_DYNAMIC=1` environment variable before build.
  `vcpkg install sqlite3:x64-windows` will install the required library.
* When linking against a SQLite (or SQLCipher) library already on the system, you can set the `SQLITE3_STATIC` (or `SQLCIPHER_STATIC`) environment variable to 1 to request that the library be statically instead of dynamically linked.


### Binding generation

We use [bindgen](https://crates.io/crates/bindgen) to generate the Rust
declarations from SQLite's C header file. `bindgen`
[recommends](https://github.com/servo/rust-bindgen#library-usage-with-buildrs)
running this as part of the build process of libraries that used this. We tried
this briefly (`rusqlite` 0.10.0, specifically), but it had some annoyances:

* The build time for `libsqlite3-sys` (and therefore `rusqlite`) increased
  dramatically.
* Running `bindgen` requires a relatively-recent version of Clang, which many
  systems do not have installed by default.
* Running `bindgen` also requires the SQLite header file to be present.

As of `rusqlite` 0.10.1, we avoid running `bindgen` at build-time by shipping
pregenerated bindings for several versions of SQLite. When compiling
`rusqlite`, we use your selected Cargo features to pick the bindings for the
minimum SQLite version that supports your chosen features. If you are using
`libsqlite3-sys` directly, you can use the same features to choose which
pregenerated bindings are chosen:

* `min_sqlite_version_3_14_0` - SQLite 3.14.0 bindings (this is the default)

If you use any of the `bundled` features, you will get pregenerated bindings for the
bundled version of SQLite/SQLCipher. If you need other specific pregenerated binding
versions, please file an issue. If you want to run `bindgen` at buildtime to
produce your own bindings, use the `buildtime_bindgen` Cargo feature.

If you enable the `modern_sqlite` feature, we'll use the bindings we would have
included with the bundled build. You generally should have `buildtime_bindgen`
enabled if you turn this on, as otherwise you'll need to keep the version of
SQLite you link with in sync with what rusqlite would have bundled, (usually the
most recent release of SQLite). Failing to do this will cause a runtime error.

## Contributing

Rusqlite has many features, and many of them impact the build configuration in
incompatible ways. This is unfortunate, and makes testing changes hard.

To help here: you generally should ensure that you run tests/lint for
`--features bundled`, and `--features "bundled-full session buildtime_bindgen"`.

If running bindgen is problematic for you, `--features bundled-full` enables
bundled and all features which don't require binding generation, and can be used
instead.

### Checklist

- Run `cargo fmt` to ensure your Rust code is correctly formatted.
- Ensure `cargo clippy --workspace --features bundled` passes without warnings.
- Ensure `cargo clippy --workspace --features "bundled-full session buildtime_bindgen"` passes without warnings.
- Ensure `cargo test --workspace --features bundled` reports no failures.
- Ensure `cargo test --workspace --features "bundled-full session buildtime_bindgen"` reports no failures.

## Author

Rusqlite is the product of hard work by a number of people. A list is available
here: https://github.com/rusqlite/rusqlite/graphs/contributors

## Community

Feel free to join the [Rusqlite Discord Server](https://discord.gg/nFYfGPB8g4) to discuss or get help with `rusqlite` or `libsqlite3-sys`.

## License

Rusqlite and libsqlite3-sys are available under the MIT license. See the LICENSE file for more info.

### Licenses of Bundled Software

Depending on the set of enabled cargo `features`, rusqlite and libsqlite3-sys will also bundle other libraries, which have their own licensing terms:

- If `--features=bundled-sqlcipher` is enabled, the vendored source of [SQLcipher](https://github.com/sqlcipher/sqlcipher) will be compiled and statically linked in. SQLcipher is distributed under a BSD-style license, as described [here](libsqlite3-sys/sqlcipher/LICENSE).

- If `--features=bundled` is enabled, the vendored source of SQLite will be compiled and linked in. SQLite is in the public domain, as described [here](https://www.sqlite.org/copyright.html).

Both of these are quite permissive, have no bearing on the license of the code in `rusqlite` or `libsqlite3-sys` themselves, and can be entirely ignored if you do not use the feature in question.
