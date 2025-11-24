#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::needless_doctest_main, clippy::partialeq_ne_impl)]

#[cfg(feature = "alloc")]
pub use self::slice::SliceExt;

pub mod alloc;

#[cfg(feature = "alloc")]
pub mod boxed;

#[cfg(feature = "alloc")]
mod raw_vec;

#[cfg(feature = "alloc")]
pub mod vec;

#[cfg(feature = "alloc")]
mod macros;

#[cfg(feature = "alloc")]
mod slice;

#[cfg(feature = "alloc")]
mod unique;

/// Allows turning a [`Box<T: Sized, A>`][boxed::Box] into a [`Box<U: ?Sized, A>`][boxed::Box] where `T` can be unsizing-coerced into a `U`.
///
/// This is the only way to create an `allocator_api2::boxed::Box` of an unsized type on stable.
///
/// With the standard library's `alloc::boxed::Box`, this is done automatically using the unstable unsize traits, but this crate's Box
/// can't take advantage of that machinery on stable. So, we need to use type inference and the fact that you *can*
/// still coerce the inner pointer of a box to get the compiler to help us unsize it using this macro.
///
/// # Example
///
/// ```
/// use allocator_api2::unsize_box;
/// use allocator_api2::boxed::Box;
/// use core::any::Any;
///
/// let sized_box: Box<u64> = Box::new(0);
/// let unsized_box: Box<dyn Any> = unsize_box!(sized_box);
/// ```
#[macro_export]
#[cfg(feature = "alloc")]
macro_rules! unsize_box {( $boxed:expr $(,)? ) => ({
    let (ptr, allocator) = ::allocator_api2::boxed::Box::into_raw_with_allocator($boxed);
    // we don't want to allow casting to arbitrary type U, but we do want to allow unsize coercion to happen.
    // that's exactly what's happening here -- this is *not* a pointer cast ptr as *mut _, but the compiler
    // *will* allow an unsizing coercion to happen into the `ptr` place, if one is available. And we use _ so that the user can
    // fill in what they want the unsized type to be by annotating the type of the variable this macro will
    // assign its result to.
    let ptr: *mut _ = ptr;
    // SAFETY: see above for why ptr's type can only be something that can be safely coerced.
    // also, ptr just came from a properly allocated box in the same allocator.
    unsafe {
        ::allocator_api2::boxed::Box::from_raw_in(ptr, allocator)
    }
})}

#[cfg(feature = "alloc")]
pub mod collections {
    pub use super::raw_vec::{TryReserveError, TryReserveErrorKind};
}

#[cfg(feature = "alloc")]
#[track_caller]
#[inline(always)]
#[cfg(debug_assertions)]
unsafe fn assume(v: bool) {
    if !v {
        core::unreachable!()
    }
}

#[cfg(feature = "alloc")]
#[track_caller]
#[inline(always)]
#[cfg(not(debug_assertions))]
unsafe fn assume(v: bool) {
    if !v {
        unsafe {
            core::hint::unreachable_unchecked();
        }
    }
}

#[cfg(feature = "alloc")]
#[inline(always)]
fn addr<T>(x: *const T) -> usize {
    #[allow(clippy::useless_transmute, clippy::transmutes_expressible_as_ptr_casts)]
    unsafe {
        core::mem::transmute(x)
    }
}

#[cfg(feature = "alloc")]
#[inline(always)]
fn invalid_mut<T>(addr: usize) -> *mut T {
    #[allow(clippy::useless_transmute, clippy::transmutes_expressible_as_ptr_casts)]
    unsafe {
        core::mem::transmute(addr)
    }
}
