#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(nightly, feature(never_type))]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Unwrap;

#[derive(Unwrap)]
enum Either<TLeft, TRight> {
    Left(TLeft),
    Right(TRight),
}

#[derive(Unwrap)]
#[derive(Debug)]
#[unwrap(ref, ref_mut)]
enum Maybe<T> {
    Nothing,
    Just(T),
}

#[derive(Unwrap)]
enum Color {
    Rgb(u8, u8, u8),
    Cmyk(u8, u8, u8, u8),
}

/// With lifetime
#[derive(Unwrap)]
enum Nonsense<'a, T> {
    Ref(&'a T),
    NoRef,
    #[unwrap(ignore)]
    NoRefIgnored,
}

#[derive(Unwrap)]
enum WithConstraints<T>
where
    T: Copy,
{
    One(T),
    Two,
}

#[derive(Unwrap)]
enum KitchenSink<'a, 'b, T1: Copy, T2: Clone>
where
    T2: Into<T1> + 'b,
{
    Left(&'a T1),
    Right(&'b T2),
    OwnBoth(T1, T2),
    Empty,
    NeverMind(),
    NothingToSeeHere(),
}

/// Single variant enum
#[derive(Unwrap)]
enum Single {
    Value(i32),
}

#[derive(Unwrap)]
#[derive(Debug, PartialEq)]
#[unwrap(ref, ref_mut)]
enum Tuple<T> {
    None,
    Single(T),
    Double(T, T),
    Triple(T, T, T),
}

#[test]
pub fn test_unwrap() {
    assert!(matches!(Maybe::<()>::Nothing.unwrap_nothing(), ()));
    assert_eq!(Maybe::Just(1).unwrap_just(), 1);

    assert_eq!(Maybe::Just(42).unwrap_just_ref(), &42);
    assert_eq!(Maybe::Just(42).unwrap_just_mut(), &mut 42);
}

#[test]
#[should_panic]
pub fn test_unwrap_panic_1() {
    Maybe::<()>::Nothing.unwrap_just();
}

#[test]
#[should_panic]
pub fn test_unwrap_panic_2() {
    Maybe::Just(2).unwrap_nothing();
}

#[test]
#[should_panic]
pub fn test_unwrap_ref_panic() {
    Maybe::Just(2).unwrap_nothing_ref();
}

#[test]
pub fn test_unwrap_mut_1() {
    let mut value = Tuple::Double(1, 12);

    let (a, b) = value.unwrap_double_mut();
    *a = 9;
    *b = 10;

    assert_eq!(value, Tuple::Double(9, 10))
}

#[test]
pub fn test_unwrap_mut_2() {
    let mut value = Tuple::Single(128);

    let x = value.unwrap_single_mut();
    *x *= 2;

    assert_eq!(value, Tuple::Single(256));
}

#[cfg(nightly)]
mod never {
    use super::*;

    #[derive(Unwrap)]
    enum Enum {
        Tuple(!),
        TupleMulti(i32, !),
    }
}
