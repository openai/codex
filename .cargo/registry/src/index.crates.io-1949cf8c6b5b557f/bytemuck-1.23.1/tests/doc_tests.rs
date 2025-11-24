#![allow(clippy::disallowed_names)]
#![allow(dead_code)]

//! Cargo miri doesn't run doctests yet, so we duplicate these here. It's
//! probably not that important to sweat keeping these perfectly up to date, but
//! we should try to catch the cases where the primary tests are doctests.
use bytemuck::*;

// Miri doesn't run on doctests, so... copypaste to the rescue.
#[test]
fn test_transparent_slice() {
  #[repr(transparent)]
  struct Slice<T>([T]);

  unsafe impl<T> TransparentWrapper<[T]> for Slice<T> {}

  let s = Slice::wrap_ref(&[1u32, 2, 3]);
  assert_eq!(&s.0, &[1, 2, 3]);

  let mut buf = [1, 2, 3u8];
  let _sm = Slice::wrap_mut(&mut buf);
}

#[test]
fn test_transparent_basic() {
  #[derive(Default)]
  struct SomeStruct(u32);

  #[repr(transparent)]
  struct MyWrapper(SomeStruct);

  unsafe impl TransparentWrapper<SomeStruct> for MyWrapper {}

  // interpret a reference to &SomeStruct as a &MyWrapper
  let thing = SomeStruct::default();
  let wrapped_ref: &MyWrapper = MyWrapper::wrap_ref(&thing);

  // Works with &mut too.
  let mut mut_thing = SomeStruct::default();
  let wrapped_mut: &mut MyWrapper = MyWrapper::wrap_mut(&mut mut_thing);
  let _ = (wrapped_ref, wrapped_mut);
}

// Work around miri not running doctests
#[test]
fn test_contiguous_doc() {
  #[repr(u8)]
  #[derive(Debug, Copy, Clone, PartialEq)]
  enum Foo {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
  }
  unsafe impl Contiguous for Foo {
    type Int = u8;
    const MIN_VALUE: u8 = Foo::A as u8;
    const MAX_VALUE: u8 = Foo::E as u8;
  }

  assert_eq!(Foo::from_integer(3).unwrap(), Foo::D);
  assert_eq!(Foo::from_integer(8), None);
  assert_eq!(Foo::C.into_integer(), 2);
  assert_eq!(Foo::B.into_integer(), Foo::B as u8);
}

#[test]
fn test_offsetof_vertex() {
  #[repr(C)]
  struct Vertex {
    pos: [f32; 2],
    uv: [u16; 2],
    color: [u8; 4],
  }
  unsafe impl Zeroable for Vertex {}

  let pos = offset_of!(Zeroable::zeroed(), Vertex, pos);
  let uv = offset_of!(Zeroable::zeroed(), Vertex, uv);
  let color = offset_of!(Zeroable::zeroed(), Vertex, color);

  assert_eq!(pos, 0);
  assert_eq!(uv, 8);
  assert_eq!(color, 12);
}

#[test]
fn test_offsetof_nonpod() {
  #[derive(Default)]
  struct Foo {
    a: u8,
    b: &'static str,
    c: i32,
  }

  let a_offset = offset_of!(Default::default(), Foo, a);
  let b_offset = offset_of!(Default::default(), Foo, b);
  let c_offset = offset_of!(Default::default(), Foo, c);

  assert_ne!(a_offset, b_offset);
  assert_ne!(b_offset, c_offset);
  // We can't check against hardcoded values for a repr(Rust) type,
  // but prove to ourself this way.

  let foo = Foo::default();
  // Note: offsets are in bytes.
  let as_bytes = &foo as *const _ as *const u8;

  // We're using wrapping_offset here because it's not worth
  // the unsafe block, but it would be valid to use `add` instead,
  // as it cannot overflow.
  assert_eq!(
    &foo.a as *const _ as usize,
    as_bytes.wrapping_add(a_offset) as usize
  );
  assert_eq!(
    &foo.b as *const _ as usize,
    as_bytes.wrapping_add(b_offset) as usize
  );
  assert_eq!(
    &foo.c as *const _ as usize,
    as_bytes.wrapping_add(c_offset) as usize
  );
}
