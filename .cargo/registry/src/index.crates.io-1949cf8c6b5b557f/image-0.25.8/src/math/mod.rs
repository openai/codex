//! Mathematical helper functions and types.
mod rect;
mod utils;

pub use self::rect::Rect;
pub(crate) use utils::multiply_accumulate;
pub(super) use utils::resize_dimensions;
