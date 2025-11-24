# idna_adapter

This crate abstracts over a Unicode back end for the [`idna`](https://docs.rs/crate/idna/latest) crate.

To work around the lack of [`global-features`](https://internals.rust-lang.org/t/pre-rfc-mutually-excusive-global-features/19618) in Cargo, this crate allows the top level `Cargo.lock` to choose an alternative Unicode back end for the `idna` crate by pinning a version of this crate.

`idna` depends on version 1 of this crate. The version stream 1.2.x uses ICU4X, the version stream 1.1.x uses unicode-rs, and the version stream 1.0.x has a stub implementation without an actual Unicode back end.

It is generally a good idea to refer to the [README of the latest version](https://docs.rs/crate/idna_adapter/latest) instead of the guidance below for up-to-date information about what options are available.

## ICU4X as the default

If you take no action, Cargo will choose the 1.2.x version stream i.e. latest ICU4X (2.0 as of writing).

### Opting to use ICU4X 1.x

To choose ICU4X 1.x, run `cargo update -p idna_adapter --precise 1.2.0` in the top-level directory of your application.

This may make sense if your application has other ways of depending on ICU4X 1.x, and you are not ready to update those dependencies to ICU4X 2.0 just yet.

## Opting to use unicode-rs

To choose unicode-rs, run `cargo update -p idna_adapter --precise 1.1.0` in the top-level directory of your application.

Compared to ICU4X, this makes build times faster, MSRV lower, binary size larger, and run-time performance slower.

## Turning off IDNA support

Since the ability to turn off actual IDNA processing has been requested again and again, an option to have no Unicode back end is provided. Choosing this option obviously breaks the `idna` crate in the sense that it cannot provide a proper implementation of UTS 46 without any Unicode data. Choosing this option makes your application reject non-ASCII domain name inputs and will fail to enforce the UTS 46 requirements on domain names that have labels in the Punycode form.

Using this option is not recommended, but to make the `idna` crate not actually support IDNA, run `cargo update -p idna_adapter --precise 1.0.0` in the top-level directory of your application.

## License

Apache-2.0 OR MIT
