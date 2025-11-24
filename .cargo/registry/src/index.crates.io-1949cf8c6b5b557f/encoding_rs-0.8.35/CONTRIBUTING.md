If you send a pull request / patch, please observe the following.

## Licensing

Since this crate is dual-licensed,
[section 5 of the Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0#contributions)
is considered to apply in the sense of Contributions being automatically
under the Apache License 2.0 or MIT dual license (see the `COPYRIGHT` file).
That is, by the act of offering a Contribution, you place your Contribution
under the Apache License 2.0 or MIT dual license stated in the `COPYRIGHT`
file. Please do not contribute if you aren't willing or allowed to license your
contributions in this manner.

You are encouraged to dedicate test code that you contribute to the Public
Domain using the CC0 dedication. If you contribute test code that is not
dedicated to the Public Domain, please be sure not to put it in a part of
source code that the comments designate as being dedicated to the Public
Domain.

## Copyright Notices

If you require the addition of your copyright notice, it's up to you to edit in
your notice as part of your Contribution. Not adding a copyright notice is
taken as a waiver of copyright notice.

## No Encodings Beyond The Encoding Standard

Please do not contribute implementations of encodings that are not specified
in the [Encoding Standard](https://encoding.spec.whatwg.org/).

For example, an implementation of UTF-7 is explicitly out of scope for this
crate and is, therefore, provided by the [`charset`](https://crates.io/crates/charset)
crate instead. For single-byte DOS encodings, please see the
[`oem_cp`](https://crates.io/crates/oem_cp) crate.

## Compatibility with Stable Rust

Please ensure that your Contribution compiles with the latest stable-channel
rustc.

## rustfmt

The `rustfmt` version used for this code is `rustfmt-nightly`. Please either
use that version or avoid using `rustfmt` (so as not to reformat all the code).

## Unit tests

Please ensure that `cargo test` succeeds.
