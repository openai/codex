[![Build Status](https://travis-ci.org/rust-lang/ena.svg?branch=master)](https://travis-ci.org/rust-lang/ena)

An implementation of union-find in Rust; extracted from (and used by)
rustc.

### Name

The name "ena" comes from the Greek word for "one".

### Features

By default, you just get the union-find implementation. You can also
opt-in to the following experimental features:

- `bench`: use to run benchmarks (`cargo bench --features bench`)

### License

Like rustc itself, this code is dual-licensed under the MIT and Apache
licenses. Pull requests, comments, and other contributions are assumed
to imply consent to those terms. Moreover, it is understood that any
changes here may well be used in rustc itself under the same terms.
