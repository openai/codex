//! [`Layer`]s that control which spans and events are enabled by the wrapped
//! subscriber.
//!
//! This module contains a number of types that provide implementations of
//! various strategies for filtering which spans and events are enabled. For
//! details on filtering spans and events using [`Layer`]s, see the
//! [`layer` module's documentation].
//!
//! [`layer` module's documentation]: crate::layer#filtering-with-layers
//! [`Layer`]: crate::layer
mod filter_fn;

feature! {
    #![all(feature = "env-filter", feature = "std")]
    mod env;
    pub use self::env::*;
}

feature! {
    #![all(feature = "registry", feature = "std")]
    mod layer_filters;
    pub use self::layer_filters::*;
}

mod level;

pub use self::filter_fn::*;
pub use self::level::{LevelFilter, ParseError as LevelParseError};

#[cfg(not(all(feature = "registry", feature = "std")))]
pub(crate) use self::has_plf_stubs::*;

feature! {
    #![any(feature = "std", feature = "alloc")]
    pub mod targets;
    pub use self::targets::Targets;

    mod directive;
    pub use self::directive::ParseError;
}

/// Stub implementations of the per-layer-filter detection functions for when the
/// `registry` feature is disabled.
#[cfg(not(all(feature = "registry", feature = "std")))]
mod has_plf_stubs {
    pub(crate) fn is_plf_downcast_marker(_: core::any::TypeId) -> bool {
        false
    }

    /// Does a type implementing `Subscriber` contain any per-layer filters?
    pub(crate) fn subscriber_has_plf<S>(_: &S) -> bool
    where
        S: tracing_core::Subscriber,
    {
        false
    }

    /// Does a type implementing `Layer` contain any per-layer filters?
    pub(crate) fn layer_has_plf<L, S>(_: &L) -> bool
    where
        L: crate::Layer<S>,
        S: tracing_core::Subscriber,
    {
        false
    }
}
