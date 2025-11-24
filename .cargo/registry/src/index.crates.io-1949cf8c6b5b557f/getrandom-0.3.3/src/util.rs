#![allow(dead_code)]
use crate::Error;
use core::{mem::MaybeUninit, ptr, slice};

/// Polyfill for `maybe_uninit_slice` feature's
/// `MaybeUninit::slice_assume_init_mut`. Every element of `slice` must have
/// been initialized.
#[inline(always)]
#[allow(unused_unsafe)] // TODO(MSRV 1.65): Remove this.
pub unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    let ptr = ptr_from_mut::<[MaybeUninit<T>]>(slice) as *mut [T];
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    unsafe { &mut *ptr }
}

#[inline]
pub fn uninit_slice_fill_zero(slice: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    unsafe { ptr::write_bytes(slice.as_mut_ptr(), 0, slice.len()) };
    unsafe { slice_assume_init_mut(slice) }
}

#[inline(always)]
pub fn slice_as_uninit<T>(slice: &[T]) -> &[MaybeUninit<T>] {
    let ptr = ptr_from_ref::<[T]>(slice) as *const [MaybeUninit<T>];
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    unsafe { &*ptr }
}

/// View an mutable initialized array as potentially-uninitialized.
///
/// This is unsafe because it allows assigning uninitialized values into
/// `slice`, which would be undefined behavior.
#[inline(always)]
#[allow(unused_unsafe)] // TODO(MSRV 1.65): Remove this.
pub unsafe fn slice_as_uninit_mut<T>(slice: &mut [T]) -> &mut [MaybeUninit<T>] {
    let ptr = ptr_from_mut::<[T]>(slice) as *mut [MaybeUninit<T>];
    // SAFETY: `MaybeUninit<T>` is guaranteed to be layout-compatible with `T`.
    unsafe { &mut *ptr }
}

// TODO: MSRV(1.76.0): Replace with `core::ptr::from_mut`.
fn ptr_from_mut<T: ?Sized>(r: &mut T) -> *mut T {
    r
}

// TODO: MSRV(1.76.0): Replace with `core::ptr::from_ref`.
fn ptr_from_ref<T: ?Sized>(r: &T) -> *const T {
    r
}

/// Default implementation of `inner_u32` on top of `fill_uninit`
#[inline]
pub fn inner_u32() -> Result<u32, Error> {
    let mut res = MaybeUninit::<u32>::uninit();
    // SAFETY: the created slice has the same size as `res`
    let dst = unsafe {
        let p: *mut MaybeUninit<u8> = res.as_mut_ptr().cast();
        slice::from_raw_parts_mut(p, core::mem::size_of::<u32>())
    };
    crate::fill_uninit(dst)?;
    // SAFETY: `dst` has been fully initialized by `imp::fill_inner`
    // since it returned `Ok`.
    Ok(unsafe { res.assume_init() })
}

/// Default implementation of `inner_u64` on top of `fill_uninit`
#[inline]
pub fn inner_u64() -> Result<u64, Error> {
    let mut res = MaybeUninit::<u64>::uninit();
    // SAFETY: the created slice has the same size as `res`
    let dst = unsafe {
        let p: *mut MaybeUninit<u8> = res.as_mut_ptr().cast();
        slice::from_raw_parts_mut(p, core::mem::size_of::<u64>())
    };
    crate::fill_uninit(dst)?;
    // SAFETY: `dst` has been fully initialized by `imp::fill_inner`
    // since it returned `Ok`.
    Ok(unsafe { res.assume_init() })
}

/// Truncates `u64` and returns the lower 32 bits as `u32`
pub(crate) fn truncate(val: u64) -> u32 {
    u32::try_from(val & u64::from(u32::MAX)).expect("The higher 32 bits are masked")
}
