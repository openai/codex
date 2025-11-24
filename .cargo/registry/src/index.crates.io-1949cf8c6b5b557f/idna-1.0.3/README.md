# `idna`

IDNA library for Rust implementing [UTS 46: Unicode IDNA Compatibility Processing](https://www.unicode.org/reports/tr46/) as parametrized by the [WHATWG URL Standard](https://url.spec.whatwg.org/#idna).

## What it does

* An implementation of UTS 46 is provided, with configurable ASCII deny list (e.g. STD3 or WHATWG rules).
* A callback mechanism is provided for pluggable logic for deciding if a label is deemed potentially too misleading to render as Unicode in a user interface.
* Errors are marked as U+FFFD REPLACEMENT CHARACTERs in Unicode output so that locations of errors may be illustrated to the user.

## What it does not do

* There is no default/sample policy provided for the callback mechanism mentioned above.
* Only UTS 46 is implemented: There is no API to request strictly IDNA 2008 only or strictly IDNA 2003 only.
* There is no API for categorizing errors beyond there being an error.
* Checks that are configurable in UTS 46 but that the WHATWG URL Standard always set a particular way (regardless of the _beStrict_ flag in the URL Standard) cannot be configured (with the exception of the old deprecated API supporting transitional processing).

## Usage

Apps that need to prepare a hostname for usage in protocols are likely to only need the top-level function `domain_to_ascii_cow` with `AsciiDenyList::URL` as the second argument. Note that this rejects IPv6 addresses, so before this, you need to check if the first byte of the input is `b'['` and, if it is, treat the input as an IPv6 address instead.

Apps that need to display host names to the user should use `uts46::Uts46::to_user_interface`. The _ToUnicode_ operation is rarely appropriate for direct application usage.

## Cargo features

* `alloc` - For future proofing. Currently always required. Currently, the crate internal may allocate heap but for typical inputs do not allocate on the heap (apart from the output `String` when applicable).
* `compiled_data` - For future proofing. Currently always required. (Passed through to ICU4X.)
* `std` - Adds `impl std::error::Error for Errors {}` (and implies `alloc`).
* By default, all of the above are enabled.

## Alternative Unicode back ends

By default, `idna` uses [ICU4X](https://github.com/unicode-org/icu4x/) as its Unicode back end. If you wish to opt for different tradeoffs between correctness, run-time performance, binary size, compile time, and MSRV, please see the [README of the latest version of the `idna_adapter` crate](https://docs.rs/crate/idna_adapter/latest) for how to opt into a different Unicode back end.

## Breaking changes since 0.5.0

* Stricter IDNA 2008 restrictions are no longer supported. Attempting to enable them panics immediately. UTS 46 allows all the names that IDNA 2008 allows, and when transitional processing is disabled, they resolve the same way. There are additional names that IDNA 2008 disallows but UTS 46 maps to names that IDNA 2008 allows (notably, input is mapped to fold-case output). UTS 46 also allows symbols that were allowed in IDNA 2003 as well as newer symbols that are allowed according to the same principle. (Earlier versions of this crate allowed rejecting such symbols. Rejecting characters that UTS 46 maps to IDNA 2008-permitted characters wasn't supported in earlier versions, either.)
* `domain_to_ascii_strict` now performs the _CheckHyphens_ check (matching previous documentation).
* The ContextJ rules are now implemented and always enabled, even when using the old deprecated API, so input that fails those rules is rejected.
* The `Idna::to_ascii_inner` method has been removed. It didn't make sense as a public method, since callers were unable to figure out if there were errors. (A GitHub search found no callers for this method.)
* Punycode labels whose decoding does not yield any non-ASCII characters are now treated as being in error.
* When turning off default cargo features, the cargo feature `compiled_data` needs to be explicitly enabled.
