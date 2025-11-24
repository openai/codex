#![allow(clippy::unnecessary_cast)]
#![allow(clippy::manual_slice_size_calculation)]

use core::{
  mem::size_of,
  num::{NonZeroU32, NonZeroU8},
};

use bytemuck::{checked::CheckedCastError, *};

#[test]
fn test_try_cast_slice() {
  // some align4 data
  let nonzero_u32_slice: &[NonZeroU32] = &[
    NonZeroU32::new(4).unwrap(),
    NonZeroU32::new(5).unwrap(),
    NonZeroU32::new(6).unwrap(),
  ];

  // contains bytes with invalid bitpattern for NonZeroU8
  assert_eq!(
    checked::try_cast_slice::<NonZeroU32, NonZeroU8>(nonzero_u32_slice),
    Err(CheckedCastError::InvalidBitPattern)
  );

  // the same data as align1
  let the_bytes: &[u8] = checked::try_cast_slice(nonzero_u32_slice).unwrap();

  assert_eq!(
    nonzero_u32_slice.as_ptr() as *const NonZeroU32 as usize,
    the_bytes.as_ptr() as *const u8 as usize
  );
  assert_eq!(
    nonzero_u32_slice.len() * size_of::<NonZeroU32>(),
    the_bytes.len() * size_of::<u8>()
  );

  // by taking one byte off the front, we're definitely mis-aligned for
  // NonZeroU32.
  let mis_aligned_bytes = &the_bytes[1..];
  assert_eq!(
    checked::try_cast_slice::<u8, NonZeroU32>(mis_aligned_bytes),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );

  // by taking one byte off the end, we're aligned but would have slop bytes for
  // NonZeroU32
  let the_bytes_len_minus1 = the_bytes.len() - 1;
  let slop_bytes = &the_bytes[..the_bytes_len_minus1];
  assert_eq!(
    checked::try_cast_slice::<u8, NonZeroU32>(slop_bytes),
    Err(CheckedCastError::PodCastError(PodCastError::OutputSliceWouldHaveSlop))
  );

  // if we don't mess with it we can up-alignment cast
  checked::try_cast_slice::<u8, NonZeroU32>(the_bytes).unwrap();
}

#[test]
fn test_try_cast_slice_mut() {
  // some align4 data
  let u32_slice: &mut [u32] = &mut [4, 5, 6];

  // contains bytes with invalid bitpattern for NonZeroU8
  assert_eq!(
    checked::try_cast_slice_mut::<u32, NonZeroU8>(u32_slice),
    Err(CheckedCastError::InvalidBitPattern)
  );

  // some align4 data
  let u32_slice: &mut [u32] = &mut [0x4444_4444, 0x5555_5555, 0x6666_6666];
  let u32_len = u32_slice.len();
  let u32_ptr = u32_slice.as_ptr();

  // the same data as align1, nonzero bytes
  let the_nonzero_bytes: &mut [NonZeroU8] =
    checked::try_cast_slice_mut(u32_slice).unwrap();
  let the_nonzero_bytes_len = the_nonzero_bytes.len();
  let the_nonzero_bytes_ptr = the_nonzero_bytes.as_ptr();

  assert_eq!(
    u32_ptr as *const u32 as usize,
    the_nonzero_bytes_ptr as *const NonZeroU8 as usize
  );
  assert_eq!(
    u32_len * size_of::<u32>(),
    the_nonzero_bytes_len * size_of::<NonZeroU8>()
  );

  // the same data as align1
  let the_bytes: &mut [u8] = checked::try_cast_slice_mut(u32_slice).unwrap();
  let the_bytes_len = the_bytes.len();
  let the_bytes_ptr = the_bytes.as_ptr();

  assert_eq!(
    u32_ptr as *const u32 as usize,
    the_bytes_ptr as *const u8 as usize
  );
  assert_eq!(
    u32_len * size_of::<u32>(),
    the_bytes_len * size_of::<NonZeroU8>()
  );

  // by taking one byte off the front, we're definitely mis-aligned for u32.
  let mis_aligned_bytes = &mut the_bytes[1..];
  assert_eq!(
    checked::try_cast_slice_mut::<u8, NonZeroU32>(mis_aligned_bytes),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );

  // by taking one byte off the end, we're aligned but would have slop bytes for
  // NonZeroU32
  let the_bytes_len_minus1 = the_bytes.len() - 1;
  let slop_bytes = &mut the_bytes[..the_bytes_len_minus1];
  assert_eq!(
    checked::try_cast_slice_mut::<u8, NonZeroU32>(slop_bytes),
    Err(CheckedCastError::PodCastError(PodCastError::OutputSliceWouldHaveSlop))
  );

  // if we don't mess with it we can up-alignment cast, since there are no
  // zeroes in the original slice
  checked::try_cast_slice_mut::<u8, NonZeroU32>(the_bytes).unwrap();
}

#[test]
fn test_types() {
  let _: NonZeroU32 = checked::cast(1.0_f32);
  let _: &mut NonZeroU32 = checked::cast_mut(&mut 1.0_f32);
  let _: &NonZeroU32 = checked::cast_ref(&1.0_f32);
  let _: &[NonZeroU32] = checked::cast_slice(&[1.0_f32]);
  let _: &mut [NonZeroU32] = checked::cast_slice_mut(&mut [1.0_f32]);
  //
  let _: Result<NonZeroU32, CheckedCastError> = checked::try_cast(1.0_f32);
  let _: Result<&mut NonZeroU32, CheckedCastError> =
    checked::try_cast_mut(&mut 1.0_f32);
  let _: Result<&NonZeroU32, CheckedCastError> =
    checked::try_cast_ref(&1.0_f32);
  let _: Result<&[NonZeroU32], CheckedCastError> =
    checked::try_cast_slice(&[1.0_f32]);
  let _: Result<&mut [NonZeroU32], CheckedCastError> =
    checked::try_cast_slice_mut(&mut [1.0_f32]);
}

#[test]
fn test_try_pod_read_unaligned() {
  let u32s = [0xaabbccdd, 0x11223344_u32];
  let bytes = bytemuck::checked::cast_slice::<u32, u8>(&u32s);

  #[cfg(target_endian = "big")]
  assert_eq!(
    checked::try_pod_read_unaligned::<NonZeroU32>(&bytes[1..5]),
    Ok(NonZeroU32::new(0xbbccdd11).unwrap())
  );
  #[cfg(target_endian = "little")]
  assert_eq!(
    checked::try_pod_read_unaligned::<NonZeroU32>(&bytes[1..5]),
    Ok(NonZeroU32::new(0x44aabbcc).unwrap())
  );

  let u32s = [0; 2];
  let bytes = bytemuck::checked::cast_slice::<u32, u8>(&u32s);

  assert_eq!(
    checked::try_pod_read_unaligned::<NonZeroU32>(&bytes[1..5]),
    Err(CheckedCastError::InvalidBitPattern)
  );
}

#[test]
fn test_try_from_bytes() {
  let nonzero_u32s = [
    NonZeroU32::new(0xaabbccdd).unwrap(),
    NonZeroU32::new(0x11223344).unwrap(),
  ];
  let bytes = bytemuck::checked::cast_slice::<NonZeroU32, u8>(&nonzero_u32s);
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..4]),
    Ok(&nonzero_u32s[0])
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..5]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..3]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[1..5]),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );

  let zero_u32s = [0, 0x11223344_u32];
  let bytes = bytemuck::checked::cast_slice::<u32, u8>(&zero_u32s);
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..4]),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[4..]),
    Ok(&NonZeroU32::new(zero_u32s[1]).unwrap())
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..5]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[..3]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[1..5]),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );
}

#[test]
fn test_try_from_bytes_mut() {
  let a = 0xaabbccdd_u32;
  let b = 0x11223344_u32;
  let mut u32s = [a, b];
  let bytes = bytemuck::checked::cast_slice_mut::<u32, u8>(&mut u32s);
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..4]),
    Ok(&mut NonZeroU32::new(a).unwrap())
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[4..]),
    Ok(&mut NonZeroU32::new(b).unwrap())
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..5]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..3]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[1..5]),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );

  let mut u32s = [0, b];
  let bytes = bytemuck::checked::cast_slice_mut::<u32, u8>(&mut u32s);
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..4]),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[4..]),
    Ok(&mut NonZeroU32::new(b).unwrap())
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..5]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes_mut::<NonZeroU32>(&mut bytes[..3]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
  assert_eq!(
    checked::try_from_bytes::<NonZeroU32>(&bytes[1..5]),
    Err(CheckedCastError::PodCastError(
      PodCastError::TargetAlignmentGreaterAndInputNotAligned
    ))
  );
}

#[test]
fn test_from_bytes() {
  let abcd = 0xaabbccdd_u32;
  let aligned_bytes = bytemuck::bytes_of(&abcd);
  assert_eq!(
    checked::from_bytes::<NonZeroU32>(aligned_bytes),
    &NonZeroU32::new(abcd).unwrap()
  );
  assert!(core::ptr::eq(
    checked::from_bytes(aligned_bytes) as *const NonZeroU32 as *const u32,
    &abcd
  ));
}

#[test]
fn test_from_bytes_mut() {
  let mut a = 0xaabbccdd_u32;
  let a_addr = &a as *const _ as usize;
  let aligned_bytes = bytemuck::bytes_of_mut(&mut a);
  assert_eq!(
    *checked::from_bytes_mut::<NonZeroU32>(aligned_bytes),
    NonZeroU32::new(0xaabbccdd).unwrap()
  );
  assert_eq!(
    checked::from_bytes_mut::<NonZeroU32>(aligned_bytes) as *const NonZeroU32
      as usize,
    a_addr
  );
}

// like #[should_panic], but can be a part of another test, instead of requiring
// it to be it's own test.
macro_rules! should_panic {
  ($ex:expr) => {
    assert!(
      std::panic::catch_unwind(|| {
        let _ = $ex;
      })
      .is_err(),
      concat!("should have panicked: `", stringify!($ex), "`")
    );
  };
}

#[test]
fn test_panics() {
  should_panic!(checked::cast::<u32, NonZeroU32>(0));
  should_panic!(checked::cast_ref::<u32, NonZeroU32>(&0));
  should_panic!(checked::cast_mut::<u32, NonZeroU32>(&mut 0));
  should_panic!(checked::cast_slice::<u8, NonZeroU32>(&[1u8, 2u8]));
  should_panic!(checked::cast_slice_mut::<u8, NonZeroU32>(&mut [1u8, 2u8]));
  should_panic!(checked::from_bytes::<NonZeroU32>(&[1u8, 2]));
  should_panic!(checked::from_bytes::<NonZeroU32>(&[1u8, 2, 3, 4, 5]));
  should_panic!(checked::from_bytes_mut::<NonZeroU32>(&mut [1u8, 2]));
  should_panic!(checked::from_bytes_mut::<NonZeroU32>(&mut [1u8, 2, 3, 4, 5]));
  // use cast_slice on some u32s to get some align>=4 bytes, so we can know
  // we'll give from_bytes unaligned ones.
  let aligned_bytes = bytemuck::cast_slice::<u32, u8>(&[0, 0]);
  should_panic!(checked::from_bytes::<NonZeroU32>(aligned_bytes));
  should_panic!(checked::from_bytes::<NonZeroU32>(&aligned_bytes[1..5]));
  should_panic!(checked::pod_read_unaligned::<NonZeroU32>(
    &aligned_bytes[1..5]
  ));
}

#[test]
fn test_char() {
  assert_eq!(checked::try_cast::<u32, char>(0), Ok('\0'));
  assert_eq!(checked::try_cast::<u32, char>(0xd7ff), Ok('\u{d7ff}'));
  assert_eq!(
    checked::try_cast::<u32, char>(0xd800),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(
    checked::try_cast::<u32, char>(0xdfff),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(checked::try_cast::<u32, char>(0xe000), Ok('\u{e000}'));
  assert_eq!(checked::try_cast::<u32, char>(0x10ffff), Ok('\u{10ffff}'));
  assert_eq!(
    checked::try_cast::<u32, char>(0x110000),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(
    checked::try_cast::<u32, char>(-1i32 as u32),
    Err(CheckedCastError::InvalidBitPattern)
  );
}

#[test]
fn test_bool() {
  assert_eq!(checked::try_cast::<u8, bool>(0), Ok(false));
  assert_eq!(checked::try_cast::<u8, bool>(1), Ok(true));
  for i in 2..=255 {
    assert_eq!(
      checked::try_cast::<u8, bool>(i),
      Err(CheckedCastError::InvalidBitPattern)
    );
  }

  assert_eq!(checked::try_from_bytes::<bool>(&[1]), Ok(&true));
  assert_eq!(
    checked::try_from_bytes::<bool>(&[3]),
    Err(CheckedCastError::InvalidBitPattern)
  );
  assert_eq!(
    checked::try_from_bytes::<bool>(&[0, 1]),
    Err(CheckedCastError::PodCastError(PodCastError::SizeMismatch))
  );
}

#[test]
fn test_all_nonzero() {
  use core::num::*;
  macro_rules! test_nonzero {
    ($nonzero:ty: $primitive:ty) => {
      assert_eq!(
        checked::try_cast::<$primitive, $nonzero>(0),
        Err(CheckedCastError::InvalidBitPattern)
      );
      assert_eq!(
        checked::try_cast::<$primitive, $nonzero>(1),
        Ok(<$nonzero>::new(1).unwrap())
      );
    };
  }

  test_nonzero!(NonZeroU8: u8);
  test_nonzero!(NonZeroI8: i8);
  test_nonzero!(NonZeroU16: u16);
  test_nonzero!(NonZeroI16: i16);
  test_nonzero!(NonZeroU32: u32);
  test_nonzero!(NonZeroI32: i32);
  test_nonzero!(NonZeroU64: u64);
  test_nonzero!(NonZeroI64: i64);
  test_nonzero!(NonZeroU128: u128);
  test_nonzero!(NonZeroI128: i128);
  test_nonzero!(NonZeroUsize: usize);
  test_nonzero!(NonZeroIsize: isize);
}
