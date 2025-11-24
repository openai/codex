//! Core primitives for `tracing`.
//!
//! [`tracing`] is a framework for instrumenting Rust programs to collect
//! structured, event-based diagnostic information. This crate defines the core
//! primitives of `tracing`.
//!
//! This crate provides:
//!
//! * [`span::Id`] identifies a span within the execution of a program.
//!
//! * [`Event`] represents a single event within a trace.
//!
//! * [`Subscriber`], the trait implemented to collect trace data.
//!
//! * [`Metadata`] and [`Callsite`] provide information describing spans and
//!   `Event`s.
//!
//! * [`Field`], [`FieldSet`], [`Value`], and [`ValueSet`] represent the
//!   structured data attached to a span.
//!
//! * [`Dispatch`] allows spans and events to be dispatched to `Subscriber`s.
//!
//! In addition, it defines the global callsite registry and per-thread current
//! dispatcher which other components of the tracing system rely on.
//!
//! *Compiler support: [requires `rustc` 1.65+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! ## Usage
//!
//! Application authors will typically not use this crate directly. Instead,
//! they will use the [`tracing`] crate, which provides a much more
//! fully-featured API. However, this crate's API will change very infrequently,
//! so it may be used when dependencies must be very stable.
//!
//! `Subscriber` implementations may depend on `tracing-core` rather than
//! `tracing`, as the additional APIs provided by `tracing` are primarily useful
//! for instrumenting libraries and applications, and are generally not
//! necessary for `Subscriber` implementations.
//!
//! The [`tokio-rs/tracing`] repository contains less stable crates designed to
//! be used with the `tracing` ecosystem. It includes a collection of
//! `Subscriber` implementations, as well as utility and adapter crates.
//!
//! ## Crate Feature Flags
//!
//! The following crate [feature flags] are available:
//!
//! * `std`: Depend on the Rust standard library (enabled by default).
//!
//!   `no_std` users may disable this feature with `default-features = false`:
//!
//!   ```toml
//!   [dependencies]
//!   tracing-core = { version = "0.1.22", default-features = false }
//!   ```
//!
//!   **Note**:`tracing-core`'s `no_std` support requires `liballoc`.
//!
//! ### Unstable Features
//!
//! These feature flags enable **unstable** features. The public API may break in 0.1.x
//! releases. To enable these features, the `--cfg tracing_unstable` must be passed to
//! `rustc` when compiling.
//!
//! The following unstable feature flags are currently available:
//!
//! * `valuable`: Enables support for recording [field values] using the
//!   [`valuable`] crate.
//!
//! #### Enabling Unstable Features
//!
//! The easiest way to set the `tracing_unstable` cfg is to use the `RUSTFLAGS`
//! env variable when running `cargo` commands:
//!
//! ```shell
//! RUSTFLAGS="--cfg tracing_unstable" cargo build
//! ```
//! Alternatively, the following can be added to the `.cargo/config` file in a
//! project to automatically enable the cfg flag for that project:
//!
//! ```toml
//! [build]
//! rustflags = ["--cfg", "tracing_unstable"]
//! ```
//!
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section
//! [field values]: crate::field
//! [`valuable`]: https://crates.io/crates/valuable
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.65. The current Tracing version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! Tracing follows the same compiler support policies as the rest of the Tokio
//! project. The current stable Rust compiler and the three most recent minor
//! versions before it will always be supported. For example, if the current
//! stable compiler version is 1.69, the minimum supported version will not be
//! increased past 1.66, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
//!
//!
//! [`span::Id`]: span::Id
//! [`Event`]: event::Event
//! [`Subscriber`]: subscriber::Subscriber
//! [`Metadata`]: metadata::Metadata
//! [`Callsite`]: callsite::Callsite
//! [`Field`]: field::Field
//! [`FieldSet`]: field::FieldSet
//! [`Value`]: field::Value
//! [`ValueSet`]: field::ValueSet
//! [`Dispatch`]: dispatcher::Dispatch
//! [`tokio-rs/tracing`]: https://github.com/tokio-rs/tracing
//! [`tracing`]: https://crates.io/crates/tracing
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/main/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg), deny(rustdoc::broken_intra_doc_links))]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_interfaces,
    private_bounds,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[doc(hidden)]
pub mod __macro_support {
    // Re-export the `core` functions that are used in macros. This allows
    // a crate to be named `core` and avoid name clashes.
    // See here: https://github.com/tokio-rs/tracing/issues/2761
    pub use core::{file, line, module_path, option::Option};
}

/// Statically constructs an [`Identifier`] for the provided [`Callsite`].
///
/// This may be used in contexts such as static initializers.
///
/// For example:
/// ```rust
/// use tracing_core::{callsite, identify_callsite};
/// # use tracing_core::{Metadata, subscriber::Interest};
/// # fn main() {
/// pub struct MyCallsite {
///    // ...
/// }
/// impl callsite::Callsite for MyCallsite {
/// # fn set_interest(&self, _: Interest) { unimplemented!() }
/// # fn metadata(&self) -> &Metadata { unimplemented!() }
///     // ...
/// }
///
/// static CALLSITE: MyCallsite = MyCallsite {
///     // ...
/// };
///
/// static CALLSITE_ID: callsite::Identifier = identify_callsite!(&CALLSITE);
/// # }
/// ```
///
/// [`Identifier`]: callsite::Identifier
/// [`Callsite`]: callsite::Callsite
#[macro_export]
macro_rules! identify_callsite {
    ($callsite:expr) => {
        $crate::callsite::Identifier($callsite)
    };
}

/// Statically constructs new span [metadata].
///
/// /// For example:
/// ```rust
/// # use tracing_core::{callsite::Callsite, subscriber::Interest};
/// use tracing_core::metadata;
/// use tracing_core::metadata::{Kind, Level, Metadata};
/// # fn main() {
/// # pub struct MyCallsite { }
/// # impl Callsite for MyCallsite {
/// # fn set_interest(&self, _: Interest) { unimplemented!() }
/// # fn metadata(&self) -> &Metadata { unimplemented!() }
/// # }
/// #
/// static FOO_CALLSITE: MyCallsite = MyCallsite {
///     // ...
/// };
///
/// static FOO_METADATA: Metadata = metadata!{
///     name: "foo",
///     target: module_path!(),
///     level: Level::DEBUG,
///     fields: &["bar", "baz"],
///     callsite: &FOO_CALLSITE,
///     kind: Kind::SPAN,
/// };
/// # }
/// ```
///
/// [metadata]: metadata::Metadata
/// [`Metadata::new`]: metadata::Metadata::new
#[macro_export]
macro_rules! metadata {
    (
        name: $name:expr,
        target: $target:expr,
        level: $level:expr,
        fields: $fields:expr,
        callsite: $callsite:expr,
        kind: $kind:expr
    ) => {
        $crate::metadata! {
            name: $name,
            target: $target,
            level: $level,
            fields: $fields,
            callsite: $callsite,
            kind: $kind,
        }
    };
    (
        name: $name:expr,
        target: $target:expr,
        level: $level:expr,
        fields: $fields:expr,
        callsite: $callsite:expr,
        kind: $kind:expr,
    ) => {
        $crate::metadata::Metadata::new(
            $name,
            $target,
            $level,
            $crate::__macro_support::Option::Some($crate::__macro_support::file!()),
            $crate::__macro_support::Option::Some($crate::__macro_support::line!()),
            $crate::__macro_support::Option::Some($crate::__macro_support::module_path!()),
            $crate::field::FieldSet::new($fields, $crate::identify_callsite!($callsite)),
            $kind,
        )
    };
}

pub(crate) mod lazy;

// Trimmed-down vendored version of spin 0.5.2 (0387621)
// Dependency of no_std lazy_static, not required in a std build
#[cfg(not(feature = "std"))]
pub(crate) mod spin;

#[cfg(not(feature = "std"))]
#[doc(hidden)]
pub type Once = self::spin::Once<()>;

#[cfg(feature = "std")]
pub use stdlib::sync::Once;

pub mod callsite;
pub mod dispatcher;
pub mod event;
pub mod field;
pub mod metadata;
mod parent;
pub mod span;
pub(crate) mod stdlib;
pub mod subscriber;

#[doc(inline)]
pub use self::{
    callsite::Callsite,
    dispatcher::Dispatch,
    event::Event,
    field::Field,
    metadata::{Level, LevelFilter, Metadata},
    subscriber::Subscriber,
};

pub use self::{metadata::Kind, subscriber::Interest};

mod sealed {
    pub trait Sealed {}
}
