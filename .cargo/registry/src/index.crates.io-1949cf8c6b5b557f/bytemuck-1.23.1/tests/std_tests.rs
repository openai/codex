#![allow(clippy::uninlined_format_args)]
#![allow(unused_imports)]
//! The integration tests seem to always have `std` linked, so things that would
//! depend on that can go here.

use bytemuck::*;
use core::num::NonZeroU8;

#[test]
fn test_transparent_vtabled() {
  use core::fmt::Display;

  #[repr(transparent)]
  struct DisplayTraitObj(dyn Display);

  unsafe impl TransparentWrapper<dyn Display> for DisplayTraitObj {}

  impl Display for DisplayTraitObj {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
      self.0.fmt(f)
    }
  }

  let v = DisplayTraitObj::wrap_ref(&5i32);
  let s = format!("{}", v);
  assert_eq!(s, "5");

  let mut x = 100i32;
  let v_mut = DisplayTraitObj::wrap_mut(&mut x);
  let s = format!("{}", v_mut);
  assert_eq!(s, "100");
}

#[test]
#[cfg(feature = "extern_crate_alloc")]
fn test_large_box_alloc() {
  type SuperPage = [[u8; 4096]; 4096];
  let _: Box<SuperPage> = try_zeroed_box().unwrap();
}

#[test]
#[cfg(feature = "extern_crate_alloc")]
fn test_zero_sized_box_alloc() {
  #[repr(align(4096))]
  struct Empty;
  unsafe impl Zeroable for Empty {}
  let _: Box<Empty> = try_zeroed_box().unwrap();
}

#[test]
#[cfg(feature = "extern_crate_alloc")]
fn test_try_from_box_bytes() {
  // Different layout: target alignment is greater than source alignment.
  assert_eq!(
    try_from_box_bytes::<u32>(Box::new([0u8; 4]).into()).map_err(|(x, _)| x),
    Err(PodCastError::AlignmentMismatch)
  );

  // Different layout: target alignment is less than source alignment.
  assert_eq!(
    try_from_box_bytes::<u32>(Box::new(0u64).into()).map_err(|(x, _)| x),
    Err(PodCastError::AlignmentMismatch)
  );

  // Different layout: target size is greater than source size.
  assert_eq!(
    try_from_box_bytes::<[u32; 2]>(Box::new(0u32).into()).map_err(|(x, _)| x),
    Err(PodCastError::SizeMismatch)
  );

  // Different layout: target size is less than source size.
  assert_eq!(
    try_from_box_bytes::<u32>(Box::new([0u32; 2]).into()).map_err(|(x, _)| x),
    Err(PodCastError::SizeMismatch)
  );

  // Round trip: alignment is equal to size.
  assert_eq!(*from_box_bytes::<u32>(Box::new(1000u32).into()), 1000u32);

  // Round trip: alignment is divider of size.
  assert_eq!(&*from_box_bytes::<[u8; 5]>(Box::new(*b"hello").into()), b"hello");

  // It's ok for T to have uninitialized bytes.
  #[cfg(feature = "derive")]
  {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, AnyBitPattern)]
    struct Foo(u8, u16);
    assert_eq!(
      *from_box_bytes::<Foo>(Box::new([0xc5c5u16; 2]).into()),
      Foo(0xc5u8, 0xc5c5u16)
    );
  }
}

#[test]
#[cfg(feature = "extern_crate_alloc")]
fn test_box_bytes_of() {
  assert_eq!(&*box_bytes_of(Box::new(*b"hello")), b"hello");

  #[cfg(target_endian = "big")]
  assert_eq!(&*box_bytes_of(Box::new(0x12345678)), b"\x12\x34\x56\x78");
  #[cfg(target_endian = "little")]
  assert_eq!(&*box_bytes_of(Box::new(0x12345678)), b"\x78\x56\x34\x12");

  // It's ok for T to have invalid bit patterns.
  assert_eq!(&*box_bytes_of(Box::new(NonZeroU8::new(0xc5))), b"\xc5");
}
