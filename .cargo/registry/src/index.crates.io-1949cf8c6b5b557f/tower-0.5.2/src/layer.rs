//! A collection of [`Layer`] based tower services
//!
//! [`Layer`]: crate::Layer

pub use tower_layer::{layer_fn, Layer, LayerFn};

/// Utilities for combining layers
///
/// [`Identity`]: crate::layer::util::Identity
/// [`Layer`]: crate::Layer
/// [`Stack`]: crate::layer::util::Stack
pub mod util {
    pub use tower_layer::{Identity, Stack};
}
