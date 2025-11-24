# utf8_iter

[![crates.io](https://img.shields.io/crates/v/utf8_iter.svg)](https://crates.io/crates/utf8_iter)
[![docs.rs](https://docs.rs/utf8_iter/badge.svg)](https://docs.rs/utf8_iter/)

utf8_iter provides iteration by `char` over potentially-invalid UTF-8 `&[u8]`
such that UTF-8 errors are handled according to the WHATWG Encoding Standard.

Iteration by `Result<char,Utf8CharsError>` is provided as an alternative that
distinguishes UTF-8 errors from U+FFFD appearing in the input.

An implementation of `char_indices()` analogous to the same-name method on
`str` is also provided.

Key parts of the code are copypaste from the UTF-8 to UTF-16 conversion code
in `encoding_rs`, which was optimized for speed in the case of valid input.
The implementation here uses the structure that was found to be fast in the
`encoding_rs` context but the structure hasn't been benchmarked in this
context.

This is a `no_std` crate.

## Licensing

TL;DR: `Apache-2.0 OR MIT`

Please see the file named
[COPYRIGHT](https://github.com/hsivonen/utf8_iter/blob/master/COPYRIGHT).

## Documentation

Generated [API documentation](https://docs.rs/utf8_iter/) is available
online.

## Release Notes

### 1.0.4

* Add iteration by `Result<char,Utf8CharsError>`.

### 1.0.3

* Fix an error in documentation.

### 1.0.2

* `char_indices()` implementation.

### 1.0.1

* `as_slice()` method.
* Implement `DoubleEndedIterator`

### 1.0.0

The initial release.
