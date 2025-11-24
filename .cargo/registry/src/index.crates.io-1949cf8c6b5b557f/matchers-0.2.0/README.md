# matchers

Regular expression matching on Rust streams.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]
[![CI][ci-badge]][ci-url]

[crates-badge]: https://img.shields.io/crates/v/matchers.svg
[crates-url]: https://crates.io/crates/matchers
[docs-badge]: https://docs.rs/matchers/badge.svg
[docs-url]: https://docs.rs/matchers/0.2.0
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[ci-badge]: https://github.com/hawkw/matchers/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/hawkw/matchers/actions/workflows/ci.yml

## Overview

The [`regex`] crate implements regular expression matching on strings and byte
arrays. However, in order to match the output of implementations of `fmt::Debug`
and `fmt::Display`, or by any code which writes to an instance of `fmt::Write`
or `io::Write`, it is necessary to first allocate a buffer, write to that
buffer, and then match the buffer against a regex.

In cases where it is not necessary to extract substrings, but only to test whether
or not output matches a regex, it is not strictly necessary to allocate and
write this output to a buffer. This crate provides a simple interface on top of
the lower-level [`regex-automata`] library that implements `fmt::Write` and
`io::Write` for regex patterns. This may be used to test whether streaming
output matches a pattern without buffering that output.

Users who need to extract substrings based on a pattern or who already have
buffered data should probably use the [`regex`] crate instead.

[`regex`]: https://crates.io/crates/regex
[`regex-automata`]: https://crates.io/crates/regex-automata
