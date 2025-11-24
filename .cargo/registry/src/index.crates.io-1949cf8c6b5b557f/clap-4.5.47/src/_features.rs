//! ## Documentation: Feature Flags
//!
//! Available [compile-time feature flags](https://doc.rust-lang.org/cargo/reference/features.html#dependency-features)
//!
//! #### Default Features
//!
//! * `std`: _Not Currently Used._ Placeholder for supporting `no_std` environments in a backwards compatible manner.
//! * `color`: Turns on terminal styling of help and error messages.  See
//!   [`Command::styles`][crate::Command::styles] to customize this.
//! * `help`: Auto-generate help output
//! * `usage`: Auto-generate usage
//! * `error-context`: Include contextual information for errors (which arg failed, etc)
//! * `suggestions`: Turns on the `Did you mean '--myoption'?` feature for when users make typos.
//!
//! #### Optional features
//!
//! * `deprecated`: Guided experience to prepare for next breaking release (at different stages of development, this may become default)
//! * `derive`: Enables the custom derive (i.e. `#[derive(Parser)]`). Without this you must use one of the other methods of creating a `clap` CLI listed above.
//! * `cargo`: Turns on macros that read values from [`CARGO_*` environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates).
//! * `env`: Turns on the usage of environment variables during parsing.
//! * `unicode`: Turns on support for unicode characters (including emoji) in arguments and help messages.
//! * ``wrap_help``: Turns on the help text wrapping feature, based on the terminal size.
//! * `string`: Allow runtime generated strings (e.g. with [`Str`][crate::builder::Str]).
//!
//! #### Experimental features
//!
//! **Warning:** These may contain breaking changes between minor releases.
//!
//! * `unstable-v5`: Preview features which will be stable on the v5.0 release
