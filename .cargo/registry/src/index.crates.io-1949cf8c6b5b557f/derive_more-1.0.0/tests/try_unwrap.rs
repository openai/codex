#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use derive_more::TryUnwrap;

#[derive(TryUnwrap)]
enum Either<TLeft, TRight> {
    Left(TLeft),
    Right(TRight),
}

#[derive(TryUnwrap)]
#[derive(Debug, PartialEq)]
#[try_unwrap(ref, ref_mut)]
enum Maybe<T> {
    Nothing,
    Just(T),
}

#[derive(TryUnwrap)]
enum Color {
    Rgb(u8, u8, u8),
    Cmyk(u8, u8, u8, u8),
}

/// With lifetime
#[derive(TryUnwrap)]
enum Nonsense<'a, T> {
    Ref(&'a T),
    NoRef,
    #[try_unwrap(ignore)]
    NoRefIgnored,
}

#[derive(TryUnwrap)]
enum WithConstraints<T>
where
    T: Copy,
{
    One(T),
    Two,
}

#[derive(TryUnwrap)]
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
#[derive(TryUnwrap)]
enum Single {
    Value(i32),
}

#[derive(TryUnwrap)]
#[derive(Debug, PartialEq)]
#[try_unwrap(ref, ref_mut)]
enum Tuple<T> {
    None,
    Single(T),
    Double(T, T),
    Triple(T, T, T),
}

#[test]
pub fn test_try_unwrap() {
    assert_eq!(Maybe::<()>::Nothing.try_unwrap_nothing().ok(), Some(()));
    assert_eq!(Maybe::Just(1).try_unwrap_just_ref().ok(), Some(&1));
    assert_eq!(Maybe::Just(42).try_unwrap_just_mut().ok(), Some(&mut 42));

    assert_eq!(
        Maybe::<()>::Nothing.try_unwrap_just().map_err(|e| e.input),
        Err(Maybe::<()>::Nothing),
    );
    assert_eq!(
        Maybe::Just(1).try_unwrap_nothing_ref().map_err(|e| e.input),
        Err(&Maybe::Just(1)),
    );
    assert_eq!(
        Maybe::Just(42)
            .try_unwrap_nothing_mut()
            .map_err(|e| e.to_string()),
        Err(
            "Attempt to call `Maybe::try_unwrap_nothing_mut()` on a `Maybe::Just` value"
                .to_string()
        ),
    );
}

#[test]
pub fn test_try_unwrap_mut_1() {
    let mut value = Tuple::Double(1, 12);

    if let Ok((a, b)) = value.try_unwrap_double_mut() {
        *a = 9;
        *b = 10;
    }

    assert_eq!(value, Tuple::Double(9, 10));
}

#[test]
pub fn test_try_unwrap_mut_2() {
    let mut value = Tuple::Single(128);

    if let Ok(x) = value.try_unwrap_single_mut() {
        *x *= 2;
    }

    if let Err(e) = value.try_unwrap_none_mut() {
        let x = *e.input.try_unwrap_single_ref().unwrap_or(&0);
        *e.input = Tuple::Double(x - 1, x);
    }

    assert_eq!(value, Tuple::Double(255, 256));
}
