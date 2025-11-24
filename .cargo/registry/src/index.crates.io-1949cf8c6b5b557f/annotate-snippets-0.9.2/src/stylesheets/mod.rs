//! List of stylesheets
//!
//! The list depends on what optional dependencies the crate has been
//! compiled with.
//!
//! By default the `no_color` is available. If the crate gets compiled
//! with `ansi_term`, the `color` stylesheet is added.

#[cfg(feature = "color")]
pub mod color;
pub mod no_color;
