#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use derive_more::TryInto;

// Ensure that the `TryInto` macro is hygienic and doesn't break when `Result`
// has been redefined.
type Result = ();

#[derive(TryInto, Clone, Copy, Debug, Eq, PartialEq)]
#[try_into(owned, ref, ref_mut)]
enum MixedInts {
    SmallInt(i32),
    NamedBigInt {
        int: i64,
    },
    UnsignedWithIgnoredField(#[try_into(ignore)] bool, i64),
    NamedUnsignedWithIgnoredField {
        #[try_into(ignore)]
        useless: bool,
        x: i64,
    },
    TwoSmallInts(i32, i32),
    NamedBigInts {
        x: i64,
        y: i64,
    },
    Unsigned(u32),
    NamedUnsigned {
        x: u32,
    },
    Unit,
    #[try_into(ignore)]
    Unit2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Wrapper<'a, const Y: usize, U>(&'a [U; Y]);

enum Foo<'lt: 'static, T: Clone, const X: usize> {
    X(Wrapper<'lt, X, T>),
}

enum EnumWithError {
    Error,
}

/// Making sure that `TryInto` does not trigger an ambiguous associated item error for `Error`.
#[derive(TryInto)]
enum EnumIntoEnumWithError {
    Foo(EnumWithError),
}

#[test]
fn test_try_into() {
    let mut i = MixedInts::SmallInt(42);
    assert_eq!(42i32, i.try_into().unwrap());
    assert_eq!(&42i32, <_ as TryInto<&i32>>::try_into(&i).unwrap());
    assert_eq!(
        &mut 42i32,
        <_ as TryInto<&mut i32>>::try_into(&mut i).unwrap()
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(i64::try_from(i).unwrap_err().input, MixedInts::SmallInt(42));
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(
        u32::try_from(i).unwrap_err().to_string(),
        "Only Unsigned, NamedUnsigned can be converted to u32"
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let mut i = MixedInts::NamedBigInt { int: 42 };
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(42i64, i.try_into().unwrap());
    assert_eq!(&42i64, <_ as TryInto<&i64>>::try_into(&i).unwrap());
    assert_eq!(
        &mut 42i64,
        <_ as TryInto<&mut i64>>::try_into(&mut i).unwrap()
    );
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(
        u32::try_from(i).unwrap_err().to_string(),
        "Only Unsigned, NamedUnsigned can be converted to u32"
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let mut i = MixedInts::TwoSmallInts(42, 64);
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!((42i32, 64i32), i.try_into().unwrap());
    assert_eq!((&42i32, &64i32), (&i).try_into().unwrap());
    assert_eq!((&mut 42i32, &mut 64i32), (&mut i).try_into().unwrap());
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(
        u32::try_from(i).unwrap_err().to_string(),
        "Only Unsigned, NamedUnsigned can be converted to u32"
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let mut i = MixedInts::NamedBigInts { x: 42, y: 64 };
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!((42i64, 64i64), i.try_into().unwrap());
    assert_eq!((&42i64, &64i64), (&i).try_into().unwrap());
    assert_eq!((&mut 42i64, &mut 64i64), (&mut i).try_into().unwrap());
    assert_eq!(
        u32::try_from(i).unwrap_err().to_string(),
        "Only Unsigned, NamedUnsigned can be converted to u32"
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let mut i = MixedInts::Unsigned(42);
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(42u32, i.try_into().unwrap());
    assert_eq!(&42u32, <_ as TryInto<&u32>>::try_into(&i).unwrap());
    assert_eq!(
        &mut 42u32,
        <_ as TryInto<&mut u32>>::try_into(&mut i).unwrap()
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let mut i = MixedInts::NamedUnsigned { x: 42 };
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(42u32, i.try_into().unwrap());
    assert_eq!(&42u32, <_ as TryInto<&u32>>::try_into(&i).unwrap());
    assert_eq!(
        &mut 42u32,
        <_ as TryInto<&mut u32>>::try_into(&mut i).unwrap()
    );
    assert_eq!(
        <()>::try_from(i).unwrap_err().to_string(),
        "Only Unit can be converted to ()"
    );

    let i = MixedInts::Unit;
    assert_eq!(
        i32::try_from(i).unwrap_err().to_string(),
        "Only SmallInt can be converted to i32"
    );
    assert_eq!(
        i64::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInt, UnsignedWithIgnoredField, NamedUnsignedWithIgnoredField can be converted to i64"
    );
    assert_eq!(
        <(i32, i32)>::try_from(i).unwrap_err().to_string(),
        "Only TwoSmallInts can be converted to (i32, i32)"
    );
    assert_eq!(
        <(i64, i64)>::try_from(i).unwrap_err().to_string(),
        "Only NamedBigInts can be converted to (i64, i64)"
    );
    assert_eq!(
        u32::try_from(i).unwrap_err().to_string(),
        "Only Unsigned, NamedUnsigned can be converted to u32"
    );
    assert!(matches!(i.try_into().unwrap(), ()));
}
