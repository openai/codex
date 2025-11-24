#![allow(clippy::disallowed_names)]
use bytemuck::{offset_of, Zeroable};

#[test]
fn test_offset_of_vertex() {
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
fn test_offset_of_foo() {
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

  // we're using wrapping_offset here because it's not worth
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
