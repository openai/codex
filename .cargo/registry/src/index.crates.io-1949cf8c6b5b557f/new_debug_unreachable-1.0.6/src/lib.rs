#![no_std]

//! `panic!()` in debug builds, optimization hint in release.

#[doc(hidden)]
pub mod _internal {
    pub use core::hint::unreachable_unchecked;
}

#[macro_export]
/// `panic!()` in debug builds, optimization hint in release.
///
/// This is equivalent to [`core::unreachable`] in debug builds,
/// and [`core::hint::unreachable_unchecked`] in release builds.
///
/// Example:
///
/// ```
/// use debug_unreachable::debug_unreachable;
///
/// if 0 > 100 {
///     // Can't happen!
///     unsafe { debug_unreachable!() }
/// } else {
///     println!("Good, 0 <= 100.");
/// }
/// ```
macro_rules! debug_unreachable {
    () => {
        debug_unreachable!("entered unreachable code")
    };
    ($e:expr) => {
        if cfg!(debug_assertions) {
            unreachable!($e)
        } else {
            $crate::_internal::unreachable_unchecked()
        }
    };
}
