// Copyright 2025 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

#![cfg(feature = "derive")] // Required for derives on `SliceDst`
#![allow(dead_code)]

//! Our UI test framework, built on the `trybuild` crate, does not support
//! testing for post-monomorphization errors. Instead, we use doctests, which
//! are able to test for post-monomorphization errors.

use crate::*;

#[derive(KnownLayout, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
#[allow(missing_debug_implementations, missing_copy_implementations)]
pub struct SliceDst<T, U> {
    pub t: T,
    pub u: [U],
}

#[allow(clippy::must_use_candidate, clippy::missing_inline_in_public_items, clippy::todo)]
impl<T: FromBytes + IntoBytes, U: FromBytes + IntoBytes> SliceDst<T, U> {
    pub fn new() -> &'static SliceDst<T, U> {
        todo!()
    }

    pub fn new_mut() -> &'static mut SliceDst<T, U> {
        todo!()
    }
}

/// We require that the alignment of the destination type is not larger than the
/// alignment of the source type.
///
/// ```compile_fail,E0080
/// let increase_alignment: &u16 = zerocopy::transmute_ref!(&[0u8; 2]);
/// ```
///
/// ```compile_fail,E0080
/// let mut src = [0u8; 2];
/// let increase_alignment: &mut u16 = zerocopy::transmute_mut!(&mut src);
/// ```
enum TransmuteRefMutAlignmentIncrease {}

/// We require that the size of the destination type is not larger than the size
/// of the source type.
///
/// ```compile_fail,E0080
/// let increase_size: &[u8; 2] = zerocopy::transmute_ref!(&0u8);
/// ```
///
/// ```compile_fail,E0080
/// let mut src = 0u8;
/// let increase_size: &mut [u8; 2] = zerocopy::transmute_mut!(&mut src);
/// ```
enum TransmuteRefMutSizeIncrease {}

/// We require that the size of the destination type is not smaller than the
/// size of the source type.
///
/// ```compile_fail,E0080
/// let decrease_size: &u8 = zerocopy::transmute_ref!(&[0u8; 2]);
/// ```
///
/// ```compile_fail,E0080
/// let mut src = [0u8; 2];
/// let decrease_size: &mut u8 = zerocopy::transmute_mut!(&mut src);
/// ```
enum TransmuteRefMutSizeDecrease {}

/// It's not possible in the general case to increase the trailing slice offset
/// during a reference transmutation - some pointer metadata values would not be
/// supportable, and so such a transmutation would be fallible.
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &SliceDst<u8, u8> = SliceDst::new();
/// let increase_offset: &SliceDst<[u8; 2], u8> = zerocopy::transmute_ref!(src);
/// ```
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &mut SliceDst<u8, u8> = SliceDst::new_mut();
/// let increase_offset: &mut SliceDst<[u8; 2], u8> = zerocopy::transmute_mut!(src);
/// ```
enum TransmuteRefMutDstOffsetIncrease {}

/// Reference transmutes are not possible when the difference between the source
/// and destination types' trailing slice offsets is not a multiple of the
/// destination type's trailing slice element size.
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &SliceDst<[u8; 3], [u8; 2]> = SliceDst::new();
/// let _: &SliceDst<[u8; 2], [u8; 2]> = zerocopy::transmute_ref!(src);
/// ```
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &mut SliceDst<[u8; 3], [u8; 2]> = SliceDst::new_mut();
/// let _: &mut SliceDst<[u8; 2], [u8; 2]> = zerocopy::transmute_mut!(src);
/// ```
enum TransmuteRefMutDstOffsetNotMultiple {}

/// Reference transmutes are not possible when the source's trailing slice
/// element size is not a multiple of the destination's.
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &SliceDst<(), [u8; 3]> = SliceDst::new();
/// let _: &SliceDst<(), [u8; 2]> = zerocopy::transmute_ref!(src);
/// ```
///
/// ```compile_fail,E0080
/// use zerocopy::doctests::SliceDst;
/// let src: &mut SliceDst<(), [u8; 3]> = SliceDst::new_mut();
/// let _: &mut SliceDst<(), [u8; 2]> = zerocopy::transmute_mut!(src);
/// ```
enum TransmuteRefMutDstElemSizeNotMultiple {}
