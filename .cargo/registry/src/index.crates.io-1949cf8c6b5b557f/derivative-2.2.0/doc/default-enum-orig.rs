# #![no_implicit_prelude]
# extern crate core;
# use core::default::Default;
# use Option::None;
#
pub enum Option<T> {
    /// No value
    None,
    /// Some value `T`
    Some(T),
}

impl<T> Default for Option<T> {
    /// Returns None.
    #[inline]
    fn default() -> Option<T> {
        None
    }
}
