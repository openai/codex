# OAuth2

<a href="https://crates.io/crates/oauth2"><img src="https://img.shields.io/crates/v/oauth2.svg"></a>
[![Build Status](https://github.com/ramosbugs/oauth2-rs/actions/workflows/main.yml/badge.svg)](https://github.com/ramosbugs/oauth2-rs/actions/workflows/main.yml)

An extensible, strongly-typed implementation of OAuth2
([RFC 6749](https://tools.ietf.org/html/rfc6749)).

Documentation is available on [docs.rs](https://docs.rs/oauth2). Release notes are available on [GitHub](https://github.com/ramosbugs/oauth2-rs/releases).

For authentication (e.g., single sign-on or social login) purposes, consider using the
[`openidconnect`](https://github.com/ramosbugs/openidconnect-rs) crate, which is built on top of
this one.

## Minimum Supported Rust Version (MSRV)

The MSRV for *5.0* and newer releases of this crate is Rust **1.65**.

The MSRV for *4.x* releases of this crate is Rust 1.45.

Beginning with the 5.0.0 release, this crate will maintain a policy of supporting
Rust releases going back at least 6 months. Changes that break compatibility with Rust releases
older than 6 months will no longer be considered SemVer breaking changes and will not result in a
new major version number for this crate. MSRV changes will coincide with minor version updates
and will not happen in patch releases.
