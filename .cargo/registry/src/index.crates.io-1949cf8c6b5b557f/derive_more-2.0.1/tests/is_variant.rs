#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(nightly, feature(never_type))]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::IsVariant;

#[derive(IsVariant)]
enum Either<TLeft, TRight> {
    Left(TLeft),
    Right(TRight),
}

const _: () = {
    let either: Either<u8, i16> = Either::Right(7);
    assert!(either.is_right());
    assert!(!either.is_left());

    let either: Either<u8, i16> = Either::Left(7);
    assert!(!either.is_right());
    assert!(either.is_left());
};

#[derive(IsVariant)]
enum Maybe<T> {
    Nothing,
    Just(T),
}

const _: () = {
    let maybe: Maybe<u8> = Maybe::Just(7);
    assert!(maybe.is_just());
    assert!(!maybe.is_nothing());

    let maybe: Maybe<u8> = Maybe::Nothing;
    assert!(!maybe.is_just());
    assert!(maybe.is_nothing());
};

#[test]
pub fn test_is_variant() {
    assert!(Maybe::<()>::Nothing.is_nothing());
    assert!(!Maybe::<()>::Nothing.is_just());
}

#[derive(IsVariant)]
enum Color {
    Rgb(u8, u8, u8),
    Cmyk { c: u8, m: u8, y: u8, k: u8 },
}

const _: () = {
    let color = Color::Rgb(0, 0, 0);
    assert!(color.is_rgb());
    assert!(!color.is_cmyk());

    let color = Color::Cmyk {
        c: 0,
        m: 0,
        y: 0,
        k: 0,
    };
    assert!(!color.is_rgb());
    assert!(color.is_cmyk());
};

#[derive(IsVariant)]
enum Nonsense<'a, T> {
    Ref(&'a T),
    NoRef,
    #[is_variant(ignore)]
    NoRefIgnored,
}

const _: () = {
    let nonsense: Nonsense<u8> = Nonsense::Ref(&7);
    assert!(nonsense.is_ref());
    assert!(!nonsense.is_no_ref());

    let nonsense: Nonsense<u8> = Nonsense::NoRef;
    assert!(!nonsense.is_ref());
    assert!(nonsense.is_no_ref());
};

#[derive(IsVariant)]
enum WithConstraints<T>
where
    T: Copy,
{
    One(T),
    Two,
}

const _: () = {
    let wc: WithConstraints<u8> = WithConstraints::One(1);
    assert!(wc.is_one());
    assert!(!wc.is_two());

    let wc: WithConstraints<u8> = WithConstraints::Two;
    assert!(!wc.is_one());
    assert!(wc.is_two());
};

#[derive(IsVariant)]
enum KitchenSink<'a, 'b, T1: Copy, T2: Clone>
where
    T2: Into<T1> + 'b,
{
    Left(&'a T1),
    Right(&'b T2),
    OwnBoth { left: T1, right: T2 },
    Empty,
    NeverMind(),
    NothingToSeeHere {},
}

const _: () = {
    let ks: KitchenSink<u16, u8> = KitchenSink::Left(&1);
    assert!(ks.is_left());
    assert!(!ks.is_right());
    assert!(!ks.is_own_both());
    assert!(!ks.is_empty());
    assert!(!ks.is_never_mind());
    assert!(!ks.is_nothing_to_see_here());

    let ks: KitchenSink<u16, u8> = KitchenSink::Right(&1);
    assert!(!ks.is_left());
    assert!(ks.is_right());
    assert!(!ks.is_own_both());
    assert!(!ks.is_empty());
    assert!(!ks.is_never_mind());
    assert!(!ks.is_nothing_to_see_here());

    let ks: KitchenSink<u16, u8> = KitchenSink::OwnBoth { left: 1, right: 2 };
    assert!(!ks.is_left());
    assert!(!ks.is_right());
    assert!(ks.is_own_both());
    assert!(!ks.is_empty());
    assert!(!ks.is_never_mind());
    assert!(!ks.is_nothing_to_see_here());

    let ks: KitchenSink<u16, u8> = KitchenSink::Empty;
    assert!(!ks.is_left());
    assert!(!ks.is_right());
    assert!(!ks.is_own_both());
    assert!(ks.is_empty());
    assert!(!ks.is_never_mind());
    assert!(!ks.is_nothing_to_see_here());

    let ks: KitchenSink<u16, u8> = KitchenSink::NeverMind();
    assert!(!ks.is_left());
    assert!(!ks.is_right());
    assert!(!ks.is_own_both());
    assert!(!ks.is_empty());
    assert!(ks.is_never_mind());
    assert!(!ks.is_nothing_to_see_here());

    let ks: KitchenSink<u16, u8> = KitchenSink::NothingToSeeHere {};
    assert!(!ks.is_left());
    assert!(!ks.is_right());
    assert!(!ks.is_own_both());
    assert!(!ks.is_empty());
    assert!(!ks.is_never_mind());
    assert!(ks.is_nothing_to_see_here());
};

#[cfg(nightly)]
mod never {
    use super::*;

    #[derive(IsVariant)]
    enum Enum {
        Tuple(!),
        Struct { field: ! },
        TupleMulti(i32, !),
        StructMulti { field: !, other: i32 },
    }
}
