# webpki-roots
This is a crate containing Mozilla's root certificates for use with
the [webpki](https://github.com/rustls/webpki) or
[rustls](https://github.com/rustls/rustls) crates.

This crate is inspired by [certifi.io](https://certifi.io/en/latest/) and
uses the data provided by the [Common CA Database (CCADB)](https://www.ccadb.org/).

[![webpki-roots](https://github.com/rustls/webpki-roots/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/rustls/webpki-roots/actions/workflows/build.yml)
[![Crate](https://img.shields.io/crates/v/webpki-roots.svg)](https://crates.io/crates/webpki-roots)

# License
The underlying data is used via the [CCADB Data Usage Terms](https://www.ccadb.org/rootstores/usage#ccadb-data-usage-terms) (see [`LICENSE`](LICENSE)).
The data in this crate is a derived work of the CCADB data.

# Regenerating sources
Sources are generated in an integration test, in `tests/codegen.rs`. The test
will fail if the sources are out of date relative to upstream, and update
`src/lib.rs` if so. The code is generated in deterministic order so changes
to the source should only result from upstream changes.
