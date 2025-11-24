# encoding_rs

[![Build Status](https://travis-ci.org/hsivonen/encoding_rs.svg?branch=master)](https://travis-ci.org/hsivonen/encoding_rs)
[![crates.io](https://img.shields.io/crates/v/encoding_rs.svg)](https://crates.io/crates/encoding_rs)
[![docs.rs](https://docs.rs/encoding_rs/badge.svg)](https://docs.rs/encoding_rs/)

encoding_rs an implementation of the (non-JavaScript parts of) the
[Encoding Standard](https://encoding.spec.whatwg.org/) written in Rust.

The Encoding Standard defines the Web-compatible set of character encodings,
which means this crate can be used to decode Web content. encoding_rs is
used in Gecko starting with Firefox 56. Due to the notable overlap between
the legacy encodings on the Web and the legacy encodings used on Windows,
this crate may be of use for non-Web-related situations as well; see below
for links to adjacent crates.

Additionally, the `mem` module provides various operations for dealing with
in-RAM text (as opposed to data that's coming from or going to an IO boundary).
The `mem` module is a module instead of a separate crate due to internal
implementation detail efficiencies.

## Functionality

Due to the Gecko use case, encoding_rs supports decoding to and encoding from
UTF-16 in addition to supporting the usual Rust use case of decoding to and
encoding from UTF-8. Additionally, the API has been designed to be FFI-friendly
to accommodate the C++ side of Gecko.

Specifically, encoding_rs does the following:

* Decodes a stream of bytes in an Encoding Standard-defined character encoding
  into valid aligned native-endian in-RAM UTF-16 (units of `u16` / `char16_t`).
* Encodes a stream of potentially-invalid aligned native-endian in-RAM UTF-16
  (units of `u16` / `char16_t`) into a sequence of bytes in an Encoding
  Standard-defined character encoding as if the lone surrogates had been
  replaced with the REPLACEMENT CHARACTER before performing the encode.
  (Gecko's UTF-16 is potentially invalid.)
* Decodes a stream of bytes in an Encoding Standard-defined character
  encoding into valid UTF-8.
* Encodes a stream of valid UTF-8 into a sequence of bytes in an Encoding
  Standard-defined character encoding. (Rust's UTF-8 is guaranteed-valid.)
* Does the above in streaming (input and output split across multiple
  buffers) and non-streaming (whole input in a single buffer and whole
  output in a single buffer) variants.
* Avoids copying (borrows) when possible in the non-streaming cases when
  decoding to or encoding from UTF-8.
* Resolves textual labels that identify character encodings in
  protocol text into type-safe objects representing the those encodings
  conceptually.
* Maps the type-safe encoding objects onto strings suitable for
  returning from `document.characterSet`.
* Validates UTF-8 (in common instruction set scenarios a bit faster for Web
  workloads than the standard library; hopefully will get upstreamed some
  day) and ASCII.

Additionally, `encoding_rs::mem` does the following:

* Checks if a byte buffer contains only ASCII.
* Checks if a potentially-invalid UTF-16 buffer contains only Basic Latin (ASCII).
* Checks if a valid UTF-8, potentially-invalid UTF-8 or potentially-invalid UTF-16
  buffer contains only Latin1 code points (below U+0100).
* Checks if a valid UTF-8, potentially-invalid UTF-8 or potentially-invalid UTF-16
  buffer or a code point or a UTF-16 code unit can trigger right-to-left behavior
  (suitable for checking if the Unicode Bidirectional Algorithm can be optimized
  out).
* Combined versions of the above two checks.
* Converts valid UTF-8, potentially-invalid UTF-8 and Latin1 to UTF-16.
* Converts potentially-invalid UTF-16 and Latin1 to UTF-8.
* Converts UTF-8 and UTF-16 to Latin1 (if in range).
* Finds the first invalid code unit in a buffer of potentially-invalid UTF-16.
* Makes a mutable buffer of potential-invalid UTF-16 contain valid UTF-16.
* Copies ASCII from one buffer to another up to the first non-ASCII byte.
* Converts ASCII to UTF-16 up to the first non-ASCII byte.
* Converts UTF-16 to ASCII up to the first non-Basic Latin code unit.

## Integration with `std::io`

Notably, the above feature list doesn't include the capability to wrap
a `std::io::Read`, decode it into UTF-8 and presenting the result via
`std::io::Read`. The [`encoding_rs_io`](https://crates.io/crates/encoding_rs_io)
crate provides that capability.

## `no_std` Environment

The crate works in a `no_std` environment. By default, the `alloc` feature,
which assumes that an allocator is present is enabled. For a no-allocator
environment, the default features (i.e. `alloc`) can be turned off. This
makes the part of the API that returns `Vec`/`String`/`Cow` unavailable.

## Decoding Email

For decoding character encodings that occur in email, use the
[`charset`](https://crates.io/crates/charset) crate instead of using this
one directly. (It wraps this crate and adds UTF-7 decoding.)

## Windows Code Page Identifier Mappings

For mappings to and from Windows code page identifiers, use the
[`codepage`](https://crates.io/crates/codepage) crate.

## DOS Encodings

This crate does not support single-byte DOS encodings that aren't required by
the Web Platform, but the [`oem_cp`](https://crates.io/crates/oem_cp) crate does.

## Preparing Text for the Encoders

Normalizing text into Unicode Normalization Form C prior to encoding text into
a legacy encoding minimizes unmappable characters. Text can be normalized to
Unicode Normalization Form C using the
[`icu_normalizer`](https://crates.io/crates/icu_normalizer) crate.

The exception is windows-1258, which after normalizing to Unicode Normalization
Form C requires tone marks to be decomposed in order to minimize unmappable
characters. Vietnamese tone marks can be decomposed using the
[`detone`](https://crates.io/crates/detone) crate.

## Licensing

TL;DR: `(Apache-2.0 OR MIT) AND BSD-3-Clause` for the code and data combination.

Please see the file named
[COPYRIGHT](https://github.com/hsivonen/encoding_rs/blob/master/COPYRIGHT).

The non-test code that isn't generated from the WHATWG data in this crate is
under Apache-2.0 OR MIT. Test code is under CC0.

This crate contains code/data generated from WHATWG-supplied data. The WHATWG
upstream changed its license for portions of specs incorporated into source code
from CC0 to BSD-3-Clause between the initial release of this crate and the present
version of this crate. The in-source licensing legends have been updated for the
parts of the generated code that have changed since the upstream license change.

## Documentation

Generated [API documentation](https://docs.rs/encoding_rs/) is available
online.

There is a [long-form write-up](https://hsivonen.fi/encoding_rs/) about the
design and internals of the crate.

## C and C++ bindings

An FFI layer for encoding_rs is available as a
[separate crate](https://github.com/hsivonen/encoding_c). The crate comes
with a [demo C++ wrapper](https://github.com/hsivonen/encoding_c/blob/master/include/encoding_rs_cpp.h)
using the C++ standard library and [GSL](https://github.com/Microsoft/GSL/) types.

The bindings for the `mem` module are in the
[encoding_c_mem crate](https://github.com/hsivonen/encoding_c_mem).

For the Gecko context, there's a
[C++ wrapper using the MFBT/XPCOM types](https://searchfox.org/mozilla-central/source/intl/Encoding.h#100).

There's a [write-up](https://hsivonen.fi/modern-cpp-in-rust/) about the C++
wrappers.

## Sample programs

* [Rust](https://github.com/hsivonen/recode_rs)
* [C](https://github.com/hsivonen/recode_c)
* [C++](https://github.com/hsivonen/recode_cpp)

## Optional features

There are currently these optional cargo features:

### `simd-accel`

Enables SIMD acceleration using the nightly-dependent `portable_simd` standard
library feature.

This is an opt-in feature, because enabling this feature _opts out_ of Rust's
guarantees of future compilers compiling old code (aka. "stability story").

Currently, this has not been tested to be an improvement except for these
targets and enabling the `simd-accel` feature is expected to break the build
on other targets:

* x86_64
* i686
* aarch64
* thumbv7neon

If you use nightly Rust, you use targets whose first component is one of the
above, and you are prepared _to have to revise your configuration when updating
Rust_, you should enable this feature. Otherwise, please _do not_ enable this
feature.

Used by Firefox.

### `serde`

Enables support for serializing and deserializing `&'static Encoding`-typed
struct fields using [Serde][1].

[1]: https://serde.rs/

Not used by Firefox.

### `fast-legacy-encode`

A catch-all option for enabling the fastest legacy encode options. _Does not
affect decode speed or UTF-8 encode speed._

At present, this option is equivalent to enabling the following options:
 * `fast-hangul-encode`
 * `fast-hanja-encode`
 * `fast-kanji-encode`
 * `fast-gb-hanzi-encode`
 * `fast-big5-hanzi-encode`

Adds 176 KB to the binary size.

Not used by Firefox.

### `fast-hangul-encode`

Changes encoding precomposed Hangul syllables into EUC-KR from binary
search over the decode-optimized tables to lookup by index making Korean
plain-text encode about 4 times as fast as without this option.

Adds 20 KB to the binary size.

Does _not_ affect decode speed.

Not used by Firefox.

### `fast-hanja-encode`

Changes encoding of Hanja into EUC-KR from linear search over the
decode-optimized table to lookup by index. Since Hanja is practically absent
in modern Korean text, this option doesn't affect perfomance in the common
case and mainly makes sense if you want to make your application resilient
agaist denial of service by someone intentionally feeding it a lot of Hanja
to encode into EUC-KR.

Adds 40 KB to the binary size.

Does _not_ affect decode speed.

Not used by Firefox.

### `fast-kanji-encode`

Changes encoding of Kanji into Shift_JIS, EUC-JP and ISO-2022-JP from linear
search over the decode-optimized tables to lookup by index making Japanese
plain-text encode to legacy encodings 30 to 50 times as fast as without this
option (about 2 times as fast as with `less-slow-kanji-encode`).

Takes precedence over `less-slow-kanji-encode`.

Adds 36 KB to the binary size (24 KB compared to `less-slow-kanji-encode`).

Does _not_ affect decode speed.

Not used by Firefox.

### `less-slow-kanji-encode`

Makes JIS X 0208 Level 1 Kanji (the most common Kanji in Shift_JIS, EUC-JP and
ISO-2022-JP) encode less slow (binary search instead of linear search) making
Japanese plain-text encode to legacy encodings 14 to 23 times as fast as
without this option.

Adds 12 KB to the binary size.

Does _not_ affect decode speed.

Not used by Firefox.

### `fast-gb-hanzi-encode`

Changes encoding of Hanzi in the CJK Unified Ideographs block into GBK and
gb18030 from linear search over a part the decode-optimized tables followed
by a binary search over another part of the decode-optimized tables to lookup
by index making Simplified Chinese plain-text encode to the legacy encodings
100 to 110 times as fast as without this option (about 2.5 times as fast as
with `less-slow-gb-hanzi-encode`).

Takes precedence over `less-slow-gb-hanzi-encode`.

Adds 36 KB to the binary size (24 KB compared to `less-slow-gb-hanzi-encode`).

Does _not_ affect decode speed.

Not used by Firefox.

### `less-slow-gb-hanzi-encode`

Makes GB2312 Level 1 Hanzi (the most common Hanzi in gb18030 and GBK) encode
less slow (binary search instead of linear search) making Simplified Chinese
plain-text encode to the legacy encodings about 40 times as fast as without
this option.

Adds 12 KB to the binary size.

Does _not_ affect decode speed.

Not used by Firefox.

### `fast-big5-hanzi-encode`

Changes encoding of Hanzi in the CJK Unified Ideographs block into Big5 from
linear search over a part the decode-optimized tables to lookup by index
making Traditional Chinese plain-text encode to Big5 105 to 125 times as fast
as without this option (about 3 times as fast as with
`less-slow-big5-hanzi-encode`).

Takes precedence over `less-slow-big5-hanzi-encode`.

Adds 40 KB to the binary size (20 KB compared to `less-slow-big5-hanzi-encode`).

Does _not_ affect decode speed.

Not used by Firefox.

### `less-slow-big5-hanzi-encode`

Makes Big5 Level 1 Hanzi (the most common Hanzi in Big5) encode less slow
(binary search instead of linear search) making Traditional Chinese
plain-text encode to Big5 about 36 times as fast as without this option.

Adds 20 KB to the binary size.

Does _not_ affect decode speed.

Not used by Firefox.

## Performance goals

For decoding to UTF-16, the goal is to perform at least as well as Gecko's old
uconv. For decoding to UTF-8, the goal is to perform at least as well as
rust-encoding. These goals have been achieved.

Encoding to UTF-8 should be fast. (UTF-8 to UTF-8 encode should be equivalent
to `memcpy` and UTF-16 to UTF-8 should be fast.)

Speed is a non-goal when encoding to legacy encodings. By default, encoding to
legacy encodings should not be optimized for speed at the expense of code size
as long as form submission and URL parsing in Gecko don't become noticeably
too slow in real-world use.

In the interest of binary size, by default, encoding_rs does not have
encode-specific data tables beyond 32 bits of encode-specific data for each
single-byte encoding. Therefore, encoders search the decode-optimized data
tables. This is a linear search in most cases. As a result, by default, encode
to legacy encodings varies from slow to extremely slow relative to other
libraries. Still, with realistic work loads, this seemed fast enough not to be
user-visibly slow on Raspberry Pi 3 (which stood in for a phone for testing)
in the Web-exposed encoder use cases.

See the cargo features above for optionally making CJK legacy encode fast.

A framework for measuring performance is [available separately][2].

[2]: https://github.com/hsivonen/encoding_bench/

## Rust Version Compatibility

It is a goal to support the latest stable Rust, the latest nightly Rust and
the version of Rust that's used for Firefox Nightly.

At this time, there is no firm commitment to support a version older than
what's required by Firefox, and there is no commitment to treat MSRV changes
as semver-breaking, because this crate depends on `cfg-if`, which doesn't
appear to treat MSRV changes as semver-breaking, so it would be useless for
this crate to treat MSRV changes as semver-breaking.

As of 2024-04-04, MSRV appears to be Rust 1.36.0 for using the crate and
1.42.0 for doc tests to pass without errors about the global allocator.
With the `simd-accel` feature, the MSRV is even higher.

## Compatibility with rust-encoding

A compatibility layer that implements the rust-encoding API on top of
encoding_rs is
[provided as a separate crate](https://github.com/hsivonen/encoding_rs_compat)
(cannot be uploaded to crates.io). The compatibility layer was originally
written with the assuption that Firefox would need it, but it is not currently
used in Firefox.

## Regenerating Generated Code

To regenerate the generated code:

 * Have Python 2 installed.
 * Clone [`https://github.com/hsivonen/encoding_c`](https://github.com/hsivonen/encoding_c)
   next to the `encoding_rs` directory.
 * Clone [`https://github.com/hsivonen/codepage`](https://github.com/hsivonen/codepage)
   next to the `encoding_rs` directory.
 * Clone [`https://github.com/whatwg/encoding`](https://github.com/whatwg/encoding)
   next to the `encoding_rs` directory.
 * Checkout revision `1d519bf8e5555cef64cf3a712485f41cd1a6a990` of the `encoding` repo.
   (Note: `f381389` was the revision of `encoding` used from before the `encoding` repo
   license change.)
 * With the `encoding_rs` directory as the working directory, run
   `python generate-encoding-data.py`.

## Roadmap

- [x] Design the low-level API.
- [x] Provide Rust-only convenience features.
- [x] Provide an stl/gsl-flavored C++ API.
- [x] Implement all decoders and encoders.
- [x] Add unit tests for all decoders and encoders.
- [x] Finish BOM sniffing variants in Rust-only convenience features.
- [x] Document the API.
- [x] Publish the crate on crates.io.
- [x] Create a solution for measuring performance.
- [x] Accelerate ASCII conversions using SSE2 on x86.
- [x] Accelerate ASCII conversions using ALU register-sized operations on
      non-x86 architectures (process an `usize` instead of `u8` at a time).
- [x] Split FFI into a separate crate so that the FFI doesn't interfere with
      LTO in pure-Rust usage.
- [x] Compress CJK indices by making use of sequential code points as well
      as Unicode-ordered parts of indices.
- [x] Make lookups by label or name use binary search that searches from the
      end of the label/name to the start.
- [x] Make labels with non-ASCII bytes fail fast.
- [ ] ~Parallelize UTF-8 validation using [Rayon](https://github.com/nikomatsakis/rayon).~
      (This turned out to be a pessimization in the ASCII case due to memory bandwidth reasons.)
- [x] Provide an XPCOM/MFBT-flavored C++ API.
- [x] Investigate accelerating single-byte encode with a single fast-tracked
      range per encoding.
- [x] Replace uconv with encoding_rs in Gecko.
- [x] Implement the rust-encoding API in terms of encoding_rs.
- [x] Add SIMD acceleration for Aarch64.
- [x] Investigate the use of NEON on 32-bit ARM.
- [ ] ~Investigate Björn Höhrmann's lookup table acceleration for UTF-8 as
      adapted to Rust in rust-encoding.~
- [x] Add actually fast CJK encode options.
- [ ] ~Investigate [Bob Steagall's lookup table acceleration for UTF-8](https://github.com/BobSteagall/CppNow2018/blob/master/FastConversionFromUTF-8/Fast%20Conversion%20From%20UTF-8%20with%20C%2B%2B%2C%20DFAs%2C%20and%20SSE%20Intrinsics%20-%20Bob%20Steagall%20-%20C%2B%2BNow%202018.pdf).~
- [x] Provide a build mode that works without `alloc` (with lesser API surface).
- [x] Migrate to `std::simd` ~once it is stable and declare 1.0.~
- [ ] Migrate `unsafe` slice access by larger types than `u8`/`u16` to `align_to`.

## Release Notes

### 0.8.35

* Implement changes for GB18030-2022. (Intentionally not treated as a semver break in practice even if this could be argued to be a breaking change in theory.)

### 0.8.34

* Use the `portable_simd` nightly feature of the standard library instead of the `packed_simd` crate. Only affects the `simd-accel` optional nightly feature.
* Internal documentation improvements and minor code improvements around `unsafe`.
* Added `rust-version` to `Cargo.toml`.

### 0.8.33

* Use `packed_simd` instead of `packed_simd_2` again now that updates are back under the `packed_simd` name. Only affects the `simd-accel` optional nightly feature.

### 0.8.32

* Removed `build.rs`. (This removal should resolve false positives reported by some antivirus products. This may break some build configurations that have opted out of Rust's guarantees against future build breakage.)
* Internal change to what API is used for reinterpreting the lane configuration of SIMD vectors.
* Documentation improvements.

### 0.8.31

* Use SPDX with parentheses now that crates.io supports parentheses.

### 0.8.30

* Update the licensing information to take into account the WHATWG data license change.

### 0.8.29

* Make the parts that use an allocator optional.

### 0.8.28

* Fix error in Serde support introduced as part of `no_std` support.

### 0.8.27

* Make the crate works in a `no_std` environment (with `alloc`).

### 0.8.26

* Fix oversights in edition 2018 migration that broke the `simd-accel` feature.

### 0.8.25

* Do pointer alignment checks in a way where intermediate steps aren't defined to be Undefined Behavior.
* Update the `packed_simd` dependency to `packed_simd_2`.
* Update the `cfg-if` dependency to 1.0.
* Address warnings that have been introduced by newer Rust versions along the way.
* Update to edition 2018, since even prior to 1.0 `cfg-if` updated to edition 2018 without a semver break.

### 0.8.24

* Avoid computing an intermediate (not dereferenced) pointer value in a manner designated as Undefined Behavior when computing pointer alignment.

### 0.8.23

* Remove year from copyright notices. (No features or bug fixes.)

### 0.8.22

* Formatting fix and new unit test. (No features or bug fixes.)

### 0.8.21

* Fixed a panic with invalid UTF-16[BE|LE] input at the end of the stream.

### 0.8.20

* Make `Decoder::latin1_byte_compatible_up_to` return `None` in more
  cases to make the method actually useful. While this could be argued
  to be a breaking change due to the bug fix changing semantics, it does
  not break callers that had to handle the `None` case in a reasonable
  way anyway.

### 0.8.19

* Removed a bunch of bound checks in `convert_str_to_utf16`.
* Added `mem::convert_utf8_to_utf16_without_replacement`.

### 0.8.18

* Added `mem::utf8_latin1_up_to` and `mem::str_latin1_up_to`.
* Added `Decoder::latin1_byte_compatible_up_to`.

### 0.8.17

* Update `bincode` (dev dependency) version requirement to 1.0.

### 0.8.16

* Switch from the `simd` crate to `packed_simd`.

### 0.8.15

* Adjust documentation for `simd-accel` (README-only release).

### 0.8.14

* Made UTF-16 to UTF-8 encode conversion fill the output buffer as
  closely as possible.

### 0.8.13

* Made the UTF-8 to UTF-16 decoder compare the number of code units written
  with the length of the right slice (the output slice) to fix a panic
  introduced in 0.8.11.

### 0.8.12

* Removed the `clippy::` prefix from clippy lint names.

### 0.8.11

* Changed minimum Rust requirement to 1.29.0 (for the ability to refer
  to the interior of a `static` when defining another `static`).
* Explicitly aligned the lookup tables for single-byte encodings and
  UTF-8 to cache lines in the hope of freeing up one cache line for
  other data. (Perhaps the tables were already aligned and this is
  placebo.)
* Added 32 bits of encode-oriented data for each single-byte encoding.
  The change was performance-neutral for non-Latin1-ish Latin legacy
  encodings, improved Latin1-ish and Arabic legacy encode speed
  somewhat (new speed is 2.4x the old speed for German, 2.3x for
  Arabic, 1.7x for Portuguese and 1.4x for French) and improved
  non-Latin1, non-Arabic legacy single-byte encode a lot (7.2x for
  Thai, 6x for Greek, 5x for Russian, 4x for Hebrew).
* Added compile-time options for fast CJK legacy encode options (at
  the cost of binary size (up to 176 KB) and run-time memory usage).
  These options still retain the overall code structure instead of
  rewriting the CJK encoders totally, so the speed isn't as good as
  what could be achieved by using even more memory / making the
  binary even langer.
* Made UTF-8 decode and validation faster.
* Added method `is_single_byte()` on `Encoding`.
* Added `mem::decode_latin1()` and `mem::encode_latin1_lossy()`.

### 0.8.10

* Disabled a unit test that tests a panic condition when the assertion
  being tested is disabled.

### 0.8.9

* Made `--features simd-accel` work with stable-channel compiler to
  simplify the Firefox build system.

### 0.8.8

* Made the `is_foo_bidi()` not treat U+FEFF (ZERO WIDTH NO-BREAK SPACE
  aka. BYTE ORDER MARK) as right-to-left.
* Made the `is_foo_bidi()` functions report `true` if the input contains
  Hebrew presentations forms (which are right-to-left but not in a
  right-to-left-roadmapped block).

### 0.8.7

* Fixed a panic in the UTF-16LE/UTF-16BE decoder when decoding to UTF-8.

### 0.8.6

* Temporarily removed the debug assertion added in version 0.8.5 from
  `convert_utf16_to_latin1_lossy`.

### 0.8.5

* If debug assertions are enabled but fuzzing isn't enabled, lossy conversions
  to Latin1 in the `mem` module assert that the input is in the range
  U+0000...U+00FF (inclusive).
* In the `mem` module provide conversions from Latin1 and UTF-16 to UTF-8
  that can deal with insufficient output space. The idea is to use them
  first with an allocation rounded up to jemalloc bucket size and do the
  worst-case allocation only if the jemalloc rounding up was insufficient
  as the first guess.

### 0.8.4

* Fix SSE2-specific, `simd-accel`-specific memory corruption introduced in
  version 0.8.1 in conversions between UTF-16 and Latin1 in the `mem` module.

### 0.8.3

* Removed an `#[inline(never)]` annotation that was not meant for release.

### 0.8.2

* Made non-ASCII UTF-16 to UTF-8 encode faster by manually omitting bound
  checks and manually adding branch prediction annotations.

### 0.8.1

* Tweaked loop unrolling and memory alignment for SSE2 conversions between
  UTF-16 and Latin1 in the `mem` module to increase the performance when
  converting long buffers.

### 0.8.0

* Changed the minimum supported version of Rust to 1.21.0 (semver breaking
  change).
* Flipped around the defaults vs. optional features for controlling the size
  vs. speed trade-off for Kanji and Hanzi legacy encode (semver breaking
  change).
* Added NEON support on ARMv7.
* SIMD-accelerated x-user-defined to UTF-16 decode.
* Made UTF-16LE and UTF-16BE decode a lot faster (including SIMD
  acceleration).

### 0.7.2

* Add the `mem` module.
* Refactor SIMD code which can affect performance outside the `mem`
  module.

### 0.7.1

* When encoding from invalid UTF-16, correctly handle U+DC00 followed by
  another low surrogate.

### 0.7.0

* [Make `replacement` a label of the replacement
  encoding.](https://github.com/whatwg/encoding/issues/70) (Spec change.)
* Remove `Encoding::for_name()`. (`Encoding::for_label(foo).unwrap()` is
  now close enough after the above label change.)
* Remove the `parallel-utf8` cargo feature.
* Add optional Serde support for `&'static Encoding`.
* Performance tweaks for ASCII handling.
* Performance tweaks for UTF-8 validation.
* SIMD support on aarch64.

### 0.6.11

* Make `Encoder::has_pending_state()` public.
* Update the `simd` crate dependency to 0.2.0.

### 0.6.10

* Reserve enough space for NCRs when encoding to ISO-2022-JP.
* Correct max length calculations for multibyte decoders.
* Correct max length calculations before BOM sniffing has been
  performed.
* Correctly calculate max length when encoding from UTF-16 to GBK.

### 0.6.9

* [Don't prepend anything when gb18030 range decode
  fails](https://github.com/whatwg/encoding/issues/110). (Spec change.)

### 0.6.8

* Correcly handle the case where the first buffer contains potentially
  partial BOM and the next buffer is the last buffer.
* Decode byte `7F` correctly in ISO-2022-JP.
* Make UTF-16 to UTF-8 encode write closer to the end of the buffer.
* Implement `Hash` for `Encoding`.

### 0.6.7

* [Map half-width katakana to full-width katana in ISO-2022-JP
  encoder](https://github.com/whatwg/encoding/issues/105). (Spec change.)
* Give `InputEmpty` correct precedence over `OutputFull` when encoding
  with replacement and the output buffer passed in is too short or the
  remaining space in the output buffer is too small after a replacement.

### 0.6.6

* Correct max length calculation when a partial BOM prefix is part of
  the decoder's state.

### 0.6.5

* Correct max length calculation in various encoders.
* Correct max length calculation in the UTF-16 decoder.
* Derive `PartialEq` and `Eq` for the `CoderResult`, `DecoderResult`
  and `EncoderResult` types.

### 0.6.4

* Avoid panic when encoding with replacement and the destination buffer is
  too short to hold one numeric character reference.

### 0.6.3

* Add support for 32-bit big-endian hosts. (For real this time.)

### 0.6.2

* Fix a panic from subslicing with bad indices in
  `Encoder::encode_from_utf16`. (Due to an oversight, it lacked the fix that
  `Encoder::encode_from_utf8` already had.)
* Micro-optimize error status accumulation in non-streaming case.

### 0.6.1

* Avoid panic near integer overflow in a case that's unlikely to actually
  happen.
* Address Clippy lints.

### 0.6.0

* Make the methods for computing worst-case buffer size requirements check
  for integer overflow.
* Upgrade rayon to 0.7.0.

### 0.5.1

* Reorder methods for better documentation readability.
* Add support for big-endian hosts. (Only 64-bit case actually tested.)
* Optimize the ALU (non-SIMD) case for 32-bit ARM instead of x86_64.

### 0.5.0

* Avoid allocating an excessively long buffers in non-streaming decode.
* Fix the behavior of ISO-2022-JP and replacement decoders near the end of the
  output buffer.
* Annotate the result structs with `#[must_use]`.

### 0.4.0

* Split FFI into a separate crate.
* Performance tweaks.
* CJK binary size and encoding performance changes.
* Parallelize UTF-8 validation in the case of long buffers (with optional
  feature `parallel-utf8`).
* Borrow even with ISO-2022-JP when possible.

### 0.3.2

* Fix moving pointers to alignment in ALU-based ASCII acceleration.
* Fix errors in documentation and improve documentation.

### 0.3.1

* Fix UTF-8 to UTF-16 decode for byte sequences beginning with 0xEE.
* Make UTF-8 to UTF-8 decode SSE2-accelerated when feature `simd-accel` is used.
* When decoding and encoding ASCII-only input from or to an ASCII-compatible
  encoding using the non-streaming API, return a borrow of the input.
* Make encode from UTF-16 to UTF-8 faster.

### 0.3

* Change the references to the instances of `Encoding` from `const` to `static`
  to make the referents unique across crates that use the refernces.
* Introduce non-reference-typed `FOO_INIT` instances of `Encoding` to allow
  foreign crates to initialize `static` arrays with references to `Encoding`
  instances even under Rust's constraints that prohibit the initialization of
  `&'static Encoding`-typed array items with `&'static Encoding`-typed
  `statics`.
* Document that the above two points will be reverted if Rust changes `const`
  to work so that cross-crate usage keeps the referents unique.
* Return `Cow`s from Rust-only non-streaming methods for encode and decode.
* `Encoding::for_bom()` returns the length of the BOM.
* ASCII-accelerated conversions for encodings other than UTF-16LE, UTF-16BE,
  ISO-2022-JP and x-user-defined.
* Add SSE2 acceleration behind the `simd-accel` feature flag. (Requires
  nightly Rust.)
* Fix panic with long bogus labels.
* Map [0xCA to U+05BA in windows-1255](https://github.com/whatwg/encoding/issues/73).
  (Spec change.)
* Correct the [end of the Shift_JIS EUDC range](https://github.com/whatwg/encoding/issues/53).
  (Spec change.)

### 0.2.4

* Polish FFI documentation.

### 0.2.3

* Fix UTF-16 to UTF-8 encode.

### 0.2.2

* Add `Encoder.encode_from_utf8_to_vec_without_replacement()`.

### 0.2.1

* Add `Encoding.is_ascii_compatible()`.

* Add `Encoding::for_bom()`.

* Make `==` for `Encoding` use name comparison instead of pointer comparison,
  because uses of the encoding constants in different crates result in
  different addresses and the constant cannot be turned into statics without
  breaking other things.

### 0.2.0

The initial release.
