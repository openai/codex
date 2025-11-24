#![allow(bad_style)]
#![allow(clippy::clone_on_copy)]

#[cfg(feature = "serde")]
use serde_test::{assert_tokens, Token};
use std::iter::FromIterator;
use tinyvec::*;

#[test]
fn test_a_vec() {
  let mut expected: ArrayVec<[i32; 4]> = Default::default();
  expected.push(1);
  expected.push(2);
  expected.push(3);

  let actual = array_vec!(1, 2, 3);

  assert_eq!(expected, actual);

  assert_eq!(array_vec![0u8; 4], array_vec!(0u8, 0u8, 0u8, 0u8));
  assert_eq!(array_vec![0u8; 4], array_vec!([u8; 4] => 0, 0, 0, 0));
  assert_eq!(array_vec![0; 4], array_vec!(0, 0, 0, 0));
  assert_eq!(array_vec![0; 4], array_vec!([u8; 4] => 0, 0, 0, 0));

  let expected2 = array_vec![1.1; 3];
  let actual2 = array_vec!([f32; 3] => 1.1, 1.1, 1.1);
  assert_eq!(expected2, actual2);
}

#[test]
fn ArrayVec_push_pop() {
  let mut av: ArrayVec<[i32; 4]> = Default::default();
  assert_eq!(av.len(), 0);
  assert_eq!(av.pop(), None);

  av.push(10_i32);
  assert_eq!(av.len(), 1);
  assert_eq!(av[0], 10);
  assert_eq!(av.pop(), Some(10));
  assert_eq!(av.len(), 0);
  assert_eq!(av.pop(), None);

  av.push(10);
  av.push(11);
  av.push(12);
  av.push(13);
  assert_eq!(av[0], 10);
  assert_eq!(av[1], 11);
  assert_eq!(av[2], 12);
  assert_eq!(av[3], 13);
  assert_eq!(av.len(), 4);
  assert_eq!(av.pop(), Some(13));
  assert_eq!(av.len(), 3);
  assert_eq!(av.pop(), Some(12));
  assert_eq!(av.len(), 2);
  assert_eq!(av.pop(), Some(11));
  assert_eq!(av.len(), 1);
  assert_eq!(av.pop(), Some(10));
  assert_eq!(av.len(), 0);
  assert_eq!(av.pop(), None);
}

#[test]
#[should_panic]
fn ArrayVec_push_overflow() {
  let mut av: ArrayVec<[i32; 0]> = Default::default();
  av.push(7);
}

#[test]
fn ArrayVec_formatting() {
  // check that we get the comma placement correct

  let mut av: ArrayVec<[i32; 4]> = Default::default();
  assert_eq!(format!("{:?}", av), "[]");
  av.push(10);
  assert_eq!(format!("{:?}", av), "[10]");
  av.push(11);
  assert_eq!(format!("{:?}", av), "[10, 11]");
  av.push(12);
  assert_eq!(format!("{:?}", av), "[10, 11, 12]");

  // below here just asserts that the impls exist.

  //
  let av: ArrayVec<[i32; 4]> = Default::default();
  assert_eq!(format!("{:b}", av), "[]");
  assert_eq!(format!("{:o}", av), "[]");
  assert_eq!(format!("{:x}", av), "[]");
  assert_eq!(format!("{:X}", av), "[]");
  assert_eq!(format!("{}", av), "[]");
  //
  let av: ArrayVec<[f32; 4]> = Default::default();
  assert_eq!(format!("{:e}", av), "[]");
  assert_eq!(format!("{:E}", av), "[]");
  //
  let av: ArrayVec<[&'static str; 4]> = Default::default();
  assert_eq!(format!("{:p}", av), "[]");
}

#[test]
fn ArrayVec_iteration() {
  let av = array_vec!([i32; 4] => 10, 11, 12, 13);

  let mut i = av.into_iter();
  assert_eq!(i.next(), Some(10));
  assert_eq!(i.next(), Some(11));
  assert_eq!(i.next(), Some(12));
  assert_eq!(i.next(), Some(13));
  assert_eq!(i.next(), None);

  let av = array_vec!([i32; 4] => 10, 11, 12, 13);

  let mut av2: ArrayVec<[i32; 4]> = av.clone().into_iter().collect();
  assert_eq!(av, av2);

  // IntoIterator for &mut ArrayVec
  for x in &mut av2 {
    *x = -*x;
  }

  // IntoIterator for &ArrayVec
  assert!(av.iter().zip(&av2).all(|(&a, &b)| a == -b));
}

#[test]
fn ArrayVec_append() {
  let mut av = array_vec!([i32; 8] => 1, 2, 3);
  let mut av2 = array_vec!([i32; 8] => 4, 5, 6);
  //
  av.append(&mut av2);
  assert_eq!(av.as_slice(), &[1_i32, 2, 3, 4, 5, 6]);
  assert_eq!(av2.as_slice(), &[]);
}

#[test]
fn ArrayVec_remove() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);
  assert_eq!(av.remove(1), 2);
  assert_eq!(&av[..], &[1, 3][..]);
}

#[test]
#[should_panic]
fn ArrayVec_remove_invalid() {
  let mut av: ArrayVec<[i32; 1]> = Default::default();
  av.push(1);
  av.remove(1);
}

#[test]
fn ArrayVec_swap_remove() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);
  av.push(4);
  assert_eq!(av.swap_remove(3), 4);
  assert_eq!(&av[..], &[1, 2, 3][..]);
  assert_eq!(av.swap_remove(0), 1);
  assert_eq!(&av[..], &[3, 2][..]);
  assert_eq!(av.swap_remove(0), 3);
  assert_eq!(&av[..], &[2][..]);
  assert_eq!(av.swap_remove(0), 2);
  assert_eq!(&av[..], &[][..]);
}

#[test]
fn ArrayVec_drain() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);

  assert_eq!(Vec::from_iter(av.clone().drain(..)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().drain(..2)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().drain(..3)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().drain(..=1)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().drain(..=2)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().drain(0..)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().drain(1..)), vec![2, 3]);

  assert_eq!(Vec::from_iter(av.clone().drain(0..2)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().drain(0..3)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().drain(1..2)), vec![2]);
  assert_eq!(Vec::from_iter(av.clone().drain(1..3)), vec![2, 3]);

  assert_eq!(Vec::from_iter(av.clone().drain(0..=1)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().drain(0..=2)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().drain(1..=1)), vec![2]);
  assert_eq!(Vec::from_iter(av.clone().drain(1..=2)), vec![2, 3]);
}

#[test]
fn ArrayVec_splice() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);

  // splice returns the same things as drain
  assert_eq!(Vec::from_iter(av.clone().splice(.., None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().splice(..2, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().splice(..3, None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().splice(..=1, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().splice(..=2, None)), vec![1, 2, 3]);

  assert_eq!(Vec::from_iter(av.clone().splice(0.., None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().splice(1.., None)), vec![2, 3]);

  assert_eq!(Vec::from_iter(av.clone().splice(0..2, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().splice(0..3, None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().splice(1..2, None)), vec![2]);
  assert_eq!(Vec::from_iter(av.clone().splice(1..3, None)), vec![2, 3]);

  assert_eq!(Vec::from_iter(av.clone().splice(0..=1, None)), vec![1, 2]);
  assert_eq!(Vec::from_iter(av.clone().splice(0..=2, None)), vec![1, 2, 3]);
  assert_eq!(Vec::from_iter(av.clone().splice(1..=1, None)), vec![2]);
  assert_eq!(Vec::from_iter(av.clone().splice(1..=2, None)), vec![2, 3]);

  // splice removes the same things as drain
  let mut av2 = av.clone();
  av2.splice(.., None);
  assert_eq!(av2, array_vec![]);

  let mut av2 = av.clone();
  av2.splice(..2, None);
  assert_eq!(av2, array_vec![3]);

  let mut av2 = av.clone();
  av2.splice(..3, None);
  assert_eq!(av2, array_vec![]);

  let mut av2 = av.clone();
  av2.splice(..=1, None);
  assert_eq!(av2, array_vec![3]);
  let mut av2 = av.clone();
  av2.splice(..=2, None);
  assert_eq!(av2, array_vec![]);

  let mut av2 = av.clone();
  av2.splice(0.., None);
  assert_eq!(av2, array_vec![]);
  let mut av2 = av.clone();
  av2.splice(1.., None);
  assert_eq!(av2, array_vec![1]);

  let mut av2 = av.clone();
  av2.splice(0..2, None);
  assert_eq!(av2, array_vec![3]);

  let mut av2 = av.clone();
  av2.splice(0..3, None);
  assert_eq!(av2, array_vec![]);
  let mut av2 = av.clone();
  av2.splice(1..2, None);
  assert_eq!(av2, array_vec![1, 3]);

  let mut av2 = av.clone();
  av2.splice(1..3, None);
  assert_eq!(av2, array_vec![1]);

  let mut av2 = av.clone();
  av2.splice(0..=1, None);
  assert_eq!(av2, array_vec![3]);

  let mut av2 = av.clone();
  av2.splice(0..=2, None);
  assert_eq!(av2, array_vec![]);

  let mut av2 = av.clone();
  av2.splice(1..=1, None);
  assert_eq!(av2, array_vec![1, 3]);

  let mut av2 = av.clone();
  av2.splice(1..=2, None);
  assert_eq!(av2, array_vec![1]);

  // splice adds the elements correctly
  let mut av2 = av.clone();
  av2.splice(.., 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(..2, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(..3, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(..=1, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(..=2, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(0.., 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(1.., 4..=6);
  assert_eq!(av2, array_vec![1, 4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(0..2, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(0..3, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(1..2, 4..=6);
  assert_eq!(av2, array_vec![1, 4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(1..3, 4..=6);
  assert_eq!(av2, array_vec![1, 4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(0..=1, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(0..=2, 4..=6);
  assert_eq!(av2, array_vec![4, 5, 6]);

  let mut av2 = av.clone();
  av2.splice(1..=1, 4..=6);
  assert_eq!(av2, array_vec![1, 4, 5, 6, 3]);

  let mut av2 = av.clone();
  av2.splice(1..=2, 4..=6);
  assert_eq!(av2, array_vec![1, 4, 5, 6]);

  // splice adds the elements correctly when the replacement is smaller
  let mut av2 = av.clone();
  av2.splice(.., Some(4));
  assert_eq!(av2, array_vec![4]);

  let mut av2 = av.clone();
  av2.splice(..2, Some(4));
  assert_eq!(av2, array_vec![4, 3]);

  let mut av2 = av.clone();
  av2.splice(1.., Some(4));
  assert_eq!(av2, array_vec![1, 4]);

  let mut av2 = av.clone();
  av2.splice(1..=1, Some(4));
  assert_eq!(av2, array_vec![1, 4, 3]);
}

#[test]
fn iter_last_nth() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);
  av.push(4);
  assert_eq!(av.len(), 4);
  let mut iter = av.into_iter();
  assert_eq!(iter.next(), Some(1));
  assert_eq!(iter.next(), Some(2));
  assert_eq!(iter.next(), Some(3));
  assert_eq!(iter.next(), Some(4));
  assert_eq!(iter.next(), None);
  assert_eq!(iter.last(), None);

  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);

  assert_eq!(av.into_iter().next(), Some(1));
}

#[test]
#[cfg(feature = "rustc_1_40")]
fn reviter() {
  let mut av: ArrayVec<[i32; 10]> = Default::default();
  av.push(1);
  av.push(2);
  av.push(3);
  av.push(4);

  let mut iter = av.into_iter();

  assert_eq!(iter.next(), Some(1));
  assert_eq!(iter.next_back(), Some(4));
  assert_eq!(iter.next(), Some(2));
  assert_eq!(iter.next_back(), Some(3));
  assert_eq!(iter.next(), None);
  assert_eq!(iter.next_back(), None);

  let mut av: ArrayVec<[i32; 32]> = Default::default();
  av.extend(0..32);

  let mut iter = av.into_iter();

  assert_eq!(iter.nth_back(0), Some(31));
  assert_eq!(iter.nth_back(2), Some(28));
  assert_eq!(iter.nth_back(0), Some(27));
  assert_eq!(iter.nth_back(99), None);
  assert_eq!(iter.nth_back(99), None);
}

#[cfg(feature = "serde")]
#[test]
fn ArrayVec_ser_de_empty() {
  let tv: ArrayVec<[i32; 0]> = Default::default();

  assert_tokens(&tv, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
}

#[cfg(feature = "serde")]
#[test]
fn ArrayVec_ser_de() {
  let mut tv: ArrayVec<[i32; 4]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);
  tv.push(4);

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
fn ArrayVec_borsh_de_empty() {
  let tv: ArrayVec<[i32; 0]> = Default::default();
  let buffer = borsh::to_vec(&tv).unwrap();
  let des: ArrayVec<[i32; 0]> = borsh::from_slice(&buffer).unwrap();
  assert_eq!(tv, des);
}

#[cfg(feature = "borsh")]
#[test]
fn ArrayVec_borsh_de() {
  let mut tv: ArrayVec<[i32; 4]> = Default::default();
  tv.push(1);
  tv.push(2);
  tv.push(3);
  tv.push(4);
  let buffer = borsh::to_vec(&tv).unwrap();
  let des: ArrayVec<[i32; 4]> = borsh::from_slice(&buffer).unwrap();
  assert_eq!(tv, des);
}

#[test]
fn ArrayVec_try_from_slice() {
  use std::convert::TryFrom;

  let nums = [1, 2, 3, 4];

  let empty: Result<ArrayVec<[i32; 2]>, _> = ArrayVec::try_from(&nums[..0]);
  assert!(empty.is_ok());
  assert_eq!(empty.unwrap().as_slice(), &[]);

  let fits: Result<ArrayVec<[i32; 2]>, _> = ArrayVec::try_from(&nums[..2]);
  assert!(fits.is_ok());
  assert_eq!(fits.unwrap().as_slice(), &[1, 2]);

  let does_not_fit: Result<ArrayVec<[i32; 2]>, _> =
    ArrayVec::try_from(&nums[..4]);
  assert!(does_not_fit.is_err());
}

#[test]
fn ArrayVec_pretty_debug() {
  let arr: [i32; 3] = [1, 2, 3];
  let expect = format!("{:#?}", arr);

  let arr: ArrayVec<[i32; 3]> = array_vec![1, 2, 3];
  let got = format!("{:#?}", arr);

  assert_eq!(got, expect);
}
