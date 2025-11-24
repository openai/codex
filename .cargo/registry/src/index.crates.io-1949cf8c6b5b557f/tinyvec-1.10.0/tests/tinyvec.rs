#![cfg(feature = "alloc")]
#![allow(bad_style)]
#![allow(clippy::redundant_clone)]

#[cfg(feature = "serde")]
use serde_test::{assert_tokens, Token};
use std::iter::FromIterator;
use tinyvec::*;

#[test]
fn TinyVec_swap_remove() {
  let mut tv: TinyVec<[i32; 10]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);
  tv.push(4);
  assert_eq!(tv.swap_remove(3), 4);
  assert_eq!(&tv[..], &[1, 2, 3][..]);
  assert_eq!(tv.swap_remove(0), 1);
  assert_eq!(&tv[..], &[3, 2][..]);
  assert_eq!(tv.swap_remove(0), 3);
  assert_eq!(&tv[..], &[2][..]);
  assert_eq!(tv.swap_remove(0), 2);
  assert_eq!(&tv[..], &[][..]);
}

#[test]
fn TinyVec_capacity() {
  let mut tv: TinyVec<[i32; 1]> = Default::default();
  assert_eq!(tv.capacity(), 1);
  tv.move_to_the_heap();
  tv.extend_from_slice(&[1, 2, 3, 4]);
  assert_eq!(tv.capacity(), 4);
}

#[test]
fn TinyVec_drain() {
  let mut tv: TinyVec<[i32; 10]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);

  assert_eq!(Vec::from_iter(tv.clone().drain(..)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().drain(..2)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(..3)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().drain(..=1)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(..=2)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().drain(0..)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().drain(1..)), vec![2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().drain(0..2)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(0..3)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().drain(1..2)), vec![2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(1..3)), vec![2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().drain(0..=1)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(0..=2)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().drain(1..=1)), vec![2]);
  assert_eq!(Vec::from_iter(tv.clone().drain(1..=2)), vec![2, 3]);
}

#[test]
fn TinyVec_splice() {
  let mut tv: TinyVec<[i32; 10]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);

  // splice returns the same things as drain
  assert_eq!(Vec::from_iter(tv.clone().splice(.., None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().splice(..2, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(..3, None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().splice(..=1, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(..=2, None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().splice(0.., None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().splice(1.., None)), vec![2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().splice(0..2, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(0..3, None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().splice(1..2, None)), vec![2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(1..3, None)), vec![2, 3]);

  assert_eq!(Vec::from_iter(tv.clone().splice(0..=1, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(0..=2, None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(tv.clone().splice(1..=1, None)), vec![2]);
  assert_eq!(Vec::from_iter(tv.clone().splice(1..=2, None)), vec![2, 3]);

  // splice removes the same things as drain
  let mut tv2 = tv.clone();
  tv2.splice(.., None);
  assert_eq!(tv2, tiny_vec![]);

  let mut tv2 = tv.clone();
  tv2.splice(..2, None);
  assert_eq!(tv2, tiny_vec![3]);

  let mut tv2 = tv.clone();
  tv2.splice(..3, None);
  assert_eq!(tv2, tiny_vec![]);

  let mut tv2 = tv.clone();
  tv2.splice(..=1, None);
  assert_eq!(tv2, tiny_vec![3]);
  let mut tv2 = tv.clone();
  tv2.splice(..=2, None);
  assert_eq!(tv2, tiny_vec![]);

  let mut tv2 = tv.clone();
  tv2.splice(0.., None);
  assert_eq!(tv2, tiny_vec![]);
  let mut tv2 = tv.clone();
  tv2.splice(1.., None);
  assert_eq!(tv2, tiny_vec![1]);

  let mut tv2 = tv.clone();
  tv2.splice(0..2, None);
  assert_eq!(tv2, tiny_vec![3]);

  let mut tv2 = tv.clone();
  tv2.splice(0..3, None);
  assert_eq!(tv2, tiny_vec![]);
  let mut tv2 = tv.clone();
  tv2.splice(1..2, None);
  assert_eq!(tv2, tiny_vec![1, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(1..3, None);
  assert_eq!(tv2, tiny_vec![1]);

  let mut tv2 = tv.clone();
  tv2.splice(0..=1, None);
  assert_eq!(tv2, tiny_vec![3]);

  let mut tv2 = tv.clone();
  tv2.splice(0..=2, None);
  assert_eq!(tv2, tiny_vec![]);

  let mut tv2 = tv.clone();
  tv2.splice(1..=1, None);
  assert_eq!(tv2, tiny_vec![1, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(1..=2, None);
  assert_eq!(tv2, tiny_vec![1]);

  // splice adds the elements correctly
  let mut tv2 = tv.clone();
  tv2.splice(.., 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(..2, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(..3, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(..=1, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(..=2, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(0.., 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(1.., 4..=6);
  assert_eq!(tv2, tiny_vec![1, 4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(0..2, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(0..3, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(1..2, 4..=6);
  assert_eq!(tv2, tiny_vec![1, 4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(1..3, 4..=6);
  assert_eq!(tv2, tiny_vec![1, 4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(0..=1, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(0..=2, 4..=6);
  assert_eq!(tv2, tiny_vec![4, 5, 6]);

  let mut tv2 = tv.clone();
  tv2.splice(1..=1, 4..=6);
  assert_eq!(tv2, tiny_vec![1, 4, 5, 6, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(1..=2, 4..=6);
  assert_eq!(tv2, tiny_vec![1, 4, 5, 6]);

  // splice adds the elements correctly when the replacement is smaller
  let mut tv2 = tv.clone();
  tv2.splice(.., Some(4));
  assert_eq!(tv2, tiny_vec![4]);

  let mut tv2 = tv.clone();
  tv2.splice(..2, Some(4));
  assert_eq!(tv2, tiny_vec![4, 3]);

  let mut tv2 = tv.clone();
  tv2.splice(1.., Some(4));
  assert_eq!(tv2, tiny_vec![1, 4]);

  let mut tv2 = tv.clone();
  tv2.splice(1..=1, Some(4));
  assert_eq!(tv2, tiny_vec![1, 4, 3]);
}

#[test]
fn TinyVec_resize() {
  let mut tv: TinyVec<[i32; 10]> = Default::default();
  tv.resize(20, 5);
  assert_eq!(&tv[..], &[5; 20]);
}

#[test]
fn TinyVec_from_slice_impl() {
  let bigger_slice: [u8; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  let tinyvec: TinyVec<[u8; 10]> = TinyVec::Heap((&bigger_slice[..]).into());
  assert_eq!(TinyVec::from(&bigger_slice[..]), tinyvec);

  let smaller_slice: [u8; 5] = [0, 1, 2, 3, 4];
  let tinyvec: TinyVec<[u8; 10]> = TinyVec::Inline(ArrayVec::from_array_len(
    [0, 1, 2, 3, 4, 0, 0, 0, 0, 0],
    5,
  ));
  assert_eq!(TinyVec::from(&smaller_slice[..]), tinyvec);

  let same_size: [u8; 4] = [0, 1, 2, 3];
  let tinyvec: TinyVec<[u8; 4]> =
    TinyVec::Inline(ArrayVec::from_array_len(same_size, 4));
  assert_eq!(TinyVec::from(&same_size[..]), tinyvec);
}

#[test]
fn TinyVec_from_array() {
  let array = [9, 8, 7, 6, 5, 4, 3, 2, 1];
  let tv = TinyVec::from(array);
  assert_eq!(&array, &tv[..]);
}

#[test]
fn TinyVec_macro() {
  let mut expected: TinyVec<[i32; 4]> = Default::default();
  expected.push(1);
  expected.push(2);
  expected.push(3);

  let actual = tiny_vec!(1, 2, 3);

  assert_eq!(expected, actual);

  assert_eq!(tiny_vec![0u8; 4], tiny_vec!(0u8, 0u8, 0u8, 0u8));
  assert_eq!(tiny_vec![0u8; 4], tiny_vec!([u8; 4] => 0, 0, 0, 0));
  assert_eq!(tiny_vec![0; 4], tiny_vec!(0, 0, 0, 0));
  assert_eq!(tiny_vec![0; 4], tiny_vec!([u8; 4] => 0, 0, 0, 0));

  let expected2 = tiny_vec![1.1; 3];
  let actual2 = tiny_vec!([f32; 3] => 1.1, 1.1, 1.1);
  assert_eq!(expected2, actual2);
}

#[test]
fn TinyVec_macro_non_copy() {
  // must use a variable here to avoid macro shenanigans
  let s = String::new();
  let _: TinyVec<[String; 10]> = tiny_vec!([String; 10] => s);
}

#[test]
fn TinyVec_reserve() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert_eq!(tv.capacity(), 4);
  tv.extend_from_slice(&[1, 2]);
  assert_eq!(tv.capacity(), 4);
  tv.reserve(2);
  assert_eq!(tv.capacity(), 4);
  tv.reserve(4);
  assert!(tv.capacity() >= 6);
  tv.extend_from_slice(&[3, 4, 5, 6]);
  tv.reserve(4);
  assert!(tv.capacity() >= 10);
}

#[cfg(feature = "rustc_1_57")]
#[test]
fn TinyVec_try_reserve() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert_eq!(tv.capacity(), 4);
  tv.extend_from_slice(&[1, 2]);
  assert_eq!(tv.capacity(), 4);
  assert!(tv.try_reserve(2).is_ok());
  assert_eq!(tv.capacity(), 4);
  assert!(tv.try_reserve(4).is_ok());
  assert!(tv.capacity() >= 6);
  tv.extend_from_slice(&[3, 4, 5, 6]);
  assert!(tv.try_reserve(4).is_ok());
  assert!(tv.capacity() >= 10);
}

#[test]
fn TinyVec_reserve_exact() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert_eq!(tv.capacity(), 4);

  tv.extend_from_slice(&[1, 2]);
  assert_eq!(tv.capacity(), 4);
  tv.reserve_exact(2);
  assert_eq!(tv.capacity(), 4);
  tv.reserve_exact(4);
  assert!(tv.capacity() >= 6);
  tv.extend_from_slice(&[3, 4, 5, 6]);
  tv.reserve_exact(4);
  assert!(tv.capacity() >= 10);
}

#[cfg(feature = "rustc_1_57")]
#[test]
fn TinyVec_try_reserve_exact() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert_eq!(tv.capacity(), 4);

  tv.extend_from_slice(&[1, 2]);
  assert_eq!(tv.capacity(), 4);
  assert!(tv.try_reserve_exact(2).is_ok());
  assert_eq!(tv.capacity(), 4);
  assert!(tv.try_reserve_exact(4).is_ok());
  assert!(tv.capacity() >= 6);
  tv.extend_from_slice(&[3, 4, 5, 6]);
  assert!(tv.try_reserve_exact(4).is_ok());
  assert!(tv.capacity() >= 10);
}

#[test]
fn TinyVec_move_to_heap_and_shrink() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert!(tv.is_inline());
  tv.move_to_the_heap();
  assert!(tv.is_heap());
  assert_eq!(tv.capacity(), 0);

  tv.push(1);
  tv.shrink_to_fit();
  assert!(tv.is_inline());
  assert_eq!(tv.capacity(), 4);

  tv.move_to_the_heap_and_reserve(3);
  assert!(tv.is_heap());
  assert_eq!(tv.capacity(), 4);
  tv.extend(2..=4);
  assert_eq!(tv.capacity(), 4);
  assert_eq!(tv.as_slice(), [1, 2, 3, 4]);
}

#[cfg(feature = "rustc_1_57")]
#[test]
fn TinyVec_try_move_to_heap_and_shrink() {
  let mut tv: TinyVec<[i32; 4]> = Default::default();
  assert!(tv.is_inline());
  assert!(tv.try_move_to_the_heap().is_ok());
  assert!(tv.is_heap());
  assert_eq!(tv.capacity(), 0);

  assert!(tv.try_reserve_exact(1).is_ok());
  assert_eq!(tv.capacity(), 1);
  tv.push(1);
  tv.shrink_to_fit();
  assert!(tv.is_inline());
  assert_eq!(tv.capacity(), 4);

  assert!(tv.try_move_to_the_heap_and_reserve(3).is_ok());
  assert!(tv.is_heap());
  assert_eq!(tv.capacity(), 4);
  tv.extend(2..=4);
  assert_eq!(tv.capacity(), 4);
  assert_eq!(tv.as_slice(), [1, 2, 3, 4]);
}

#[cfg(feature = "serde")]
#[test]
fn TinyVec_ser_de_empty() {
  let tv: TinyVec<[i32; 0]> = tiny_vec![];

  assert_tokens(&tv, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
}

#[cfg(feature = "serde")]
#[test]
fn TinyVec_ser_de() {
  let tv: TinyVec<[i32; 4]> = tiny_vec![1, 2, 3, 4];

  assert_tokens(
    &tv,
    &[
      Token::Seq { len: Some(4) },
      Token::I32(1),
      Token::I32(2),
      Token::I32(3),
      Token::I32(4),
      Token::SeqEnd,
    ],
  );
}

#[cfg(feature = "serde")]
#[test]
fn TinyVec_ser_de_heap() {
  let mut tv: TinyVec<[i32; 4]> = tiny_vec![1, 2, 3, 4];
  tv.move_to_the_heap();

  assert_tokens(
    &tv,
    &[
      Token::Seq { len: Some(4) },
      Token::I32(1),
      Token::I32(2),
      Token::I32(3),
      Token::I32(4),
      Token::SeqEnd,
    ],
  );
}

#[cfg(feature = "borsh")]
#[test]
fn TinyVec_borsh_de_empty() {
  let tv: ArrayVec<[i32; 0]> = Default::default();
  let buffer = borsh::to_vec(&tv).unwrap();
  let des: ArrayVec<[i32; 0]> = borsh::from_slice(&buffer).unwrap();
  assert_eq!(tv, des);
}

#[cfg(feature = "borsh")]
#[test]
fn TinyVec_borsh_de() {
  let mut tv: ArrayVec<[i32; 4]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);
  tv.push(4);
  let buffer = borsh::to_vec(&tv).unwrap();
  let des: ArrayVec<[i32; 4]> = borsh::from_slice(&buffer).unwrap();
  assert_eq!(tv, des);
}

#[cfg(feature = "borsh")]
#[test]
fn TinyVec_borsh_de_heap() {
  let mut tv: TinyVec<[i32; 4]> = tiny_vec![1, 2, 3, 4];
  tv.move_to_the_heap();

  let buffer = borsh::to_vec(&tv).unwrap();
  let des: TinyVec<[i32; 4]> = borsh::from_slice(&buffer).unwrap();
  assert_eq!(tv, des);
}

#[test]
fn TinyVec_pretty_debug() {
  let tv: TinyVec<[i32; 6]> = tiny_vec![1, 2, 3];
  let s = format!("{:#?}", tv);
  let expected = format!("{:#?}", tv.as_slice());

  assert_eq!(s, expected);
}

#[cfg(feature = "std")]
#[test]
fn TinyVec_std_io_write() {
  use std::io::Write;
  let mut tv: TinyVec<[u8; 3]> = TinyVec::new();

  tv.write_all(b"foo").ok();
  assert!(tv.is_inline());
  assert_eq!(tv, tiny_vec![b'f', b'o', b'o']);

  tv.write_all(b"bar").ok();
  assert!(tv.is_heap());
  assert_eq!(tv, tiny_vec![b'f', b'o', b'o', b'b', b'a', b'r']);
}
