#![allow(dead_code)]
use core::{mem::MaybeUninit, ptr};

/// Polyfill for `maybe_uninit_slice` feature's
/// `MaybeUninit::slice_assume_init_mut`. Every element of `slice` must have
/// been initialized.
#[inline(always)]
pub unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    &mut *(slice as *mut [MaybeUninit<T>] as *mut [T])
}

#[inline]
pub fn uninit_slice_fill_zero(slice: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    unsafe { ptr::write_bytes(slice.as_mut_ptr(), 0, slice.len()) };
    unsafe { slice_assume_init_mut(slice) }
}

#[inline(always)]
pub fn slice_as_uninit<T>(slice: &[T]) -> &[MaybeUninit<T>] {
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    // There is no risk of writing a `MaybeUninit<T>` into the result since
    // the result isn't mutable.
    unsafe { &*(slice as *const [T] as *const [MaybeUninit<T>]) }
}

/// View an mutable initialized array as potentially-uninitialized.
///
/// This is unsafe because it allows assigning uninitialized values into
/// `slice`, which would be undefined behavior.
#[inline(always)]
pub unsafe fn slice_as_uninit_mut<T>(slice: &mut [T]) -> &mut [MaybeUninit<T>] {
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    &mut *(slice as *mut [T] as *mut [MaybeUninit<T>])
}
