//! Tools for converting file format structures to and from bytes.
//!
//! This module should be replaced once rust provides safe transmutes.

// This module provides functions for both read and write features.
#![cfg_attr(
    not(all(feature = "read_core", feature = "write_core")),
    allow(dead_code)
)]

use core::{mem, result, slice};

type Result<T> = result::Result<T, ()>;

/// A trait for types that can safely be converted from and to byte slices.
///
/// # Safety
/// A type that is `Pod` must:
/// - be `#[repr(C)]` or `#[repr(transparent)]`
/// - have no invalid byte values
/// - have no padding
pub unsafe trait Pod: Copy + 'static {}

/// Cast the head of a byte slice to a `Pod` type.
///
/// Returns the type and the tail of the byte slice.
///
/// Returns an error if the byte slice is too short or the alignment is invalid.
#[inline]
pub fn from_bytes<T: Pod>(data: &[u8]) -> Result<(&T, &[u8])> {
    let size = mem::size_of::<T>();
    let tail = data.get(size..).ok_or(())?;
    let ptr = data.as_ptr();
    if (ptr as usize) % mem::align_of::<T>() != 0 {
        return Err(());
    }
    // Safety:
    // The alignment and size are checked by this function.
    // The Pod trait ensures the type is valid to cast from bytes.
    let val = unsafe { &*ptr.cast() };
    Ok((val, tail))
}

/// Cast the head of a mutable byte slice to a `Pod` type.
///
/// Returns the type and the tail of the byte slice.
///
/// Returns an error if the byte slice is too short or the alignment is invalid.
#[inline]
pub fn from_bytes_mut<T: Pod>(data: &mut [u8]) -> Result<(&mut T, &mut [u8])> {
    let size = mem::size_of::<T>();
    if size > data.len() {
        return Err(());
    }
    let (data, tail) = data.split_at_mut(size);
    let ptr = data.as_mut_ptr();
    if (ptr as usize) % mem::align_of::<T>() != 0 {
        return Err(());
    }
    // Safety:
    // The alignment and size are checked by this function.
    // The Pod trait ensures the type is valid to cast from bytes.
    let val = unsafe { &mut *ptr.cast() };
    Ok((val, tail))
}

/// Cast the head of a byte slice to a slice of a `Pod` type.
///
/// Returns the type slice and the tail of the byte slice.
///
/// Returns an error if the byte slice is too short or the alignment is invalid.
#[inline]
pub fn slice_from_bytes<T: Pod>(data: &[u8], count: usize) -> Result<(&[T], &[u8])> {
    let size = count.checked_mul(mem::size_of::<T>()).ok_or(())?;
    let tail = data.get(size..).ok_or(())?;
    let ptr = data.as_ptr();
    if (ptr as usize) % mem::align_of::<T>() != 0 {
        return Err(());
    }
    // Safety:
    // The alignment and size are checked by this function.
    // The Pod trait ensures the type is valid to cast from bytes.
    let slice = unsafe { slice::from_raw_parts(ptr.cast(), count) };
    Ok((slice, tail))
}

/// Cast the head of a mutable byte slice to a slice of a `Pod` type.
///
/// Returns the type slice and the tail of the byte slice.
///
/// Returns an error if the byte slice is too short or the alignment is invalid.
#[inline]
pub fn slice_from_bytes_mut<T: Pod>(
    data: &mut [u8],
    count: usize,
) -> Result<(&mut [T], &mut [u8])> {
    let size = count.checked_mul(mem::size_of::<T>()).ok_or(())?;
    if size > data.len() {
        return Err(());
    }
    let (data, tail) = data.split_at_mut(size);
    let ptr = data.as_mut_ptr();
    if (ptr as usize) % mem::align_of::<T>() != 0 {
        return Err(());
    }
    // Safety:
    // The alignment and size are checked by this function.
    // The Pod trait ensures the type is valid to cast from bytes.
    let slice = unsafe { slice::from_raw_parts_mut(ptr.cast(), count) };
    Ok((slice, tail))
}

/// Cast all of a byte slice to a slice of a `Pod` type.
///
/// Returns the type slice.
///
/// Returns an error if the size of the byte slice is not an exact multiple
/// of the type size, or the alignment is invalid.
#[inline]
pub fn slice_from_all_bytes<T: Pod>(data: &[u8]) -> Result<&[T]> {
    let count = data.len() / mem::size_of::<T>();
    let (slice, tail) = slice_from_bytes(data, count)?;
    if !tail.is_empty() {
        return Err(());
    }
    Ok(slice)
}

/// Cast all of a byte slice to a slice of a `Pod` type.
///
/// Returns the type slice.
///
/// Returns an error if the size of the byte slice is not an exact multiple
/// of the type size, or the alignment is invalid.
#[inline]
pub fn slice_from_all_bytes_mut<T: Pod>(data: &mut [u8]) -> Result<&mut [T]> {
    let count = data.len() / mem::size_of::<T>();
    let (slice, tail) = slice_from_bytes_mut(data, count)?;
    if !tail.is_empty() {
        return Err(());
    }
    Ok(slice)
}

/// Cast a `Pod` type to a byte slice.
#[inline]
pub fn bytes_of<T: Pod>(val: &T) -> &[u8] {
    let size = mem::size_of::<T>();
    // Safety:
    // Any alignment is allowed.
    // The size is determined in this function.
    // The Pod trait ensures the type is valid to cast to bytes.
    unsafe { slice::from_raw_parts(slice::from_ref(val).as_ptr().cast(), size) }
}

/// Cast a `Pod` type to a mutable byte slice.
#[inline]
pub fn bytes_of_mut<T: Pod>(val: &mut T) -> &mut [u8] {
    let size = mem::size_of::<T>();
    // Safety:
    // Any alignment is allowed.
    // The size is determined in this function.
    // The Pod trait ensures the type is valid to cast to bytes.
    unsafe { slice::from_raw_parts_mut(slice::from_mut(val).as_mut_ptr().cast(), size) }
}

/// Cast a slice of a `Pod` type to a byte slice.
#[inline]
pub fn bytes_of_slice<T: Pod>(val: &[T]) -> &[u8] {
    let size = val.len().wrapping_mul(mem::size_of::<T>());
    // Safety:
    // Any alignment is allowed.
    // The size is determined in this function.
    // The Pod trait ensures the type is valid to cast to bytes.
    unsafe { slice::from_raw_parts(val.as_ptr().cast(), size) }
}

/// Cast a slice of a `Pod` type to a mutable byte slice.
#[inline]
pub fn bytes_of_slice_mut<T: Pod>(val: &mut [T]) -> &mut [u8] {
    let size = val.len().wrapping_mul(mem::size_of::<T>());
    // Safety:
    // Any alignment is allowed.
    // The size is determined in this function.
    // The Pod trait ensures the type is valid to cast to bytes.
    unsafe { slice::from_raw_parts_mut(val.as_mut_ptr().cast(), size) }
}

macro_rules! unsafe_impl_pod {
    ($($struct_name:ident),+ $(,)?) => {
        $(
            unsafe impl Pod for $struct_name { }
        )+
    }
}

unsafe_impl_pod!(u8, u16, u32, u64);

unsafe impl<const N: usize, T: Pod> Pod for [T; N] {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single() {
        let x = u32::to_be(0x0123_4567);
        let mut x_mut = x;
        let bytes = bytes_of(&x);
        let bytes_mut = bytes_of_mut(&mut x_mut);
        assert_eq!(bytes, [0x01, 0x23, 0x45, 0x67]);
        assert_eq!(bytes, bytes_mut);

        let x16 = [u16::to_be(0x0123), u16::to_be(0x4567)];

        let (y, tail) = from_bytes::<u32>(bytes).unwrap();
        let (y_mut, tail_mut) = from_bytes_mut::<u32>(bytes_mut).unwrap();
        assert_eq!(*y, x);
        assert_eq!(y, y_mut);
        assert_eq!(tail, &[]);
        assert_eq!(tail, tail_mut);

        let (y, tail) = from_bytes::<u16>(bytes).unwrap();
        let (y_mut, tail_mut) = from_bytes_mut::<u16>(bytes_mut).unwrap();
        assert_eq!(*y, x16[0]);
        assert_eq!(y, y_mut);
        assert_eq!(tail, &bytes[2..]);
        assert_eq!(tail, tail_mut);

        let (y, tail) = from_bytes::<u16>(&bytes[2..]).unwrap();
        let (y_mut, tail_mut) = from_bytes_mut::<u16>(&mut bytes_mut[2..]).unwrap();
        assert_eq!(*y, x16[1]);
        assert_eq!(y, y_mut);
        assert_eq!(tail, &[]);
        assert_eq!(tail, tail_mut);

        assert_eq!(from_bytes::<u16>(&bytes[1..]), Err(()));
        assert_eq!(from_bytes::<u16>(&bytes[3..]), Err(()));
        assert_eq!(from_bytes::<u16>(&bytes[4..]), Err(()));
        assert_eq!(from_bytes_mut::<u16>(&mut bytes_mut[1..]), Err(()));
        assert_eq!(from_bytes_mut::<u16>(&mut bytes_mut[3..]), Err(()));
        assert_eq!(from_bytes_mut::<u16>(&mut bytes_mut[4..]), Err(()));
    }

    #[test]
    fn slice() {
        let x = [
            u16::to_be(0x0123),
            u16::to_be(0x4567),
            u16::to_be(0x89ab),
            u16::to_be(0xcdef),
        ];
        let mut x_mut = x;

        let bytes = bytes_of_slice(&x);
        let bytes_mut = bytes_of_slice_mut(&mut x_mut);
        assert_eq!(bytes, [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        assert_eq!(bytes, bytes_mut);

        let (y, tail) = slice_from_bytes::<u16>(bytes, 4).unwrap();
        let (y_mut, tail_mut) = slice_from_bytes_mut::<u16>(bytes_mut, 4).unwrap();
        assert_eq!(y, x);
        assert_eq!(y, y_mut);
        assert_eq!(tail, &[]);
        assert_eq!(tail, tail_mut);

        let (y, tail) = slice_from_bytes::<u16>(&bytes[2..], 2).unwrap();
        let (y_mut, tail_mut) = slice_from_bytes_mut::<u16>(&mut bytes_mut[2..], 2).unwrap();
        assert_eq!(y, &x[1..3]);
        assert_eq!(y, y_mut);
        assert_eq!(tail, &bytes[6..]);
        assert_eq!(tail, tail_mut);

        assert_eq!(slice_from_bytes::<u16>(bytes, 5), Err(()));
        assert_eq!(slice_from_bytes::<u16>(&bytes[2..], 4), Err(()));
        assert_eq!(slice_from_bytes::<u16>(&bytes[1..], 2), Err(()));
        assert_eq!(slice_from_bytes_mut::<u16>(bytes_mut, 5), Err(()));
        assert_eq!(slice_from_bytes_mut::<u16>(&mut bytes_mut[2..], 4), Err(()));
        assert_eq!(slice_from_bytes_mut::<u16>(&mut bytes_mut[1..], 2), Err(()));
    }
}
