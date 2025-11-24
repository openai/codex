//! An internal module for grouping all forms of image buffers.
pub(crate) mod buffer;
#[cfg(feature = "rayon")]
pub(crate) mod buffer_par;
pub(crate) mod dynimage;
pub(crate) mod generic_image;
// Public as we re-export the whole module including its documentation.
pub mod flat;
pub(crate) mod sub_image;
