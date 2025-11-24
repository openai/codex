#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::ToString as _, vec::Vec};

use derive_more::{
    Add, AddAssign, Binary, BitAnd, BitOr, BitXor, Constructor, Deref, DerefMut,
    Display, Div, From, FromStr, Index, IndexMut, Into, IntoIterator, Mul, MulAssign,
    Neg, Not, Octal, Product, Rem, Shl, Shr, Sub, Sum,
};

#[derive(From)]
#[derive(Into)]
#[derive(Constructor)]
#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Add)]
#[derive(Mul)]
#[derive(Neg)]
#[derive(AddAssign)]
#[derive(MulAssign)]
#[derive(FromStr)]
#[derive(Display)]
#[derive(Octal)]
#[derive(Binary)]
#[derive(Deref, DerefMut)]
#[into(owned, ref, ref_mut)]
struct MyInt(i32);

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Add)]
#[derive(Sum)]
#[derive(Mul)]
#[derive(MulAssign)]
#[derive(Product)]
#[mul(forward)]
#[mul_assign(forward)]
struct MyInt2(i32);

#[derive(Debug, Eq, PartialEq)]
#[derive(Index, IndexMut)]
#[derive(Deref, DerefMut)]
#[derive(IntoIterator)]
#[deref(forward)]
#[deref_mut(forward)]
#[into_iterator(owned, ref, ref_mut)]
struct MyVec(Vec<i32>);

#[derive(Debug, Eq, PartialEq)]
#[derive(Deref, DerefMut)]
#[deref(forward)]
#[deref_mut(forward)]
struct MyBoxedInt(Box<i32>);

#[derive(Debug, Eq, PartialEq)]
#[derive(Not)]
#[derive(From)]
struct MyBool(bool);

#[derive(From)]
#[derive(Into)]
#[derive(Constructor)]
#[derive(Add)]
#[derive(Debug, Eq, PartialEq)]
#[derive(Mul)]
#[derive(AddAssign)]
struct MyUInt(u64, u64);

#[derive(From)]
#[derive(Into)]
#[derive(Constructor)]
#[derive(FromStr)]
#[derive(Debug, Eq, PartialEq)]
#[derive(Display)]
struct SimpleStruct {
    int1: u64,
}

#[derive(From)]
#[derive(Constructor)]
#[derive(Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shr, Shl)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[derive(Into)]
#[derive(AddAssign)]
#[into(owned, ref, ref_mut)]
struct NormalStruct {
    int1: u64,
    int2: u64,
}

#[derive(From)]
#[derive(Debug, Eq, PartialEq)]
struct NestedInt(MyInt);

#[derive(Debug, Eq, PartialEq)]
#[derive(From)]
#[derive(Add, Sub)]
enum SimpleMyIntEnum {
    Int(i32),
    #[from(ignore)]
    _UnsignedOne(u32),
    _UnsignedTwo(u32),
}
#[derive(Debug, Eq, PartialEq)]
#[derive(From)]
#[derive(Neg)]
enum SimpleSignedIntEnum {
    Int(i32),
    Int2(i16),
}

#[derive(Debug, Eq, PartialEq)]
#[derive(From)]
#[derive(Add, Sub)]
#[derive(Neg)]
enum SimpleEnum {
    Int(i32),
    #[from(ignore)]
    _Ints(i32, i32),
    LabeledInts {
        a: i32,
        b: i32,
    },
    _SomeUnit,
}

#[derive(Debug, Eq, PartialEq)]
#[derive(From)]
#[derive(Add, Sub)]
enum MyIntEnum {
    SmallInt(i32),
    BigInt(i64),
    TwoInts(i32, i32),
    Point2D {
        x: i64,
        y: i64,
    },
    #[from(ignore)]
    _UnsignedOne(u32),
    _UnsignedTwo(u32),
    #[from(ignore)]
    _Uints1(u64, u64),
    _Uints2 {
        x: u64,
        y: u64,
    },
}

#[derive(Debug, Eq, PartialEq)]
#[derive(Add, Mul)]
struct DoubleUInt(u32, u32);

#[derive(Debug, Eq, PartialEq)]
#[derive(Add, Mul)]
struct DoubleUIntStruct {
    x: u32,
    y: u32,
}

#[derive(Debug, Eq, PartialEq)]
#[derive(From, Into, Constructor)]
struct Unit;

// Tests that we can forward to a path
// containing `$crate`
macro_rules! use_dollar_crate {
    () => {
        struct Foo;
        #[derive(From)]
        enum Bar {
            First(#[from(forward)] $crate::Foo),
        }
    };
}

use_dollar_crate!();

#[test]
fn main() {
    let mut myint: MyInt = 5.into();
    let _: SimpleMyIntEnum = 5i32.into();
    let _: MyIntEnum = 5i32.into();
    let _: MyIntEnum = 6i64.into();
    let _: MyIntEnum = (5i32, 8i32).into();
    let _: MyIntEnum = (5i64, 8i64).into();

    let int_ref: &i32 = (&myint).into();
    assert_eq!(int_ref, &5);

    let int_ref_mut: &mut i32 = (&mut myint).into();
    assert_eq!(int_ref_mut, &mut 5);

    let mut myint: MyInt = 5.into();
    let _: Unit = ().into();
    assert!(matches!(Unit.into(), ()));
    assert_eq!(Unit, Unit::new());
    assert_eq!(MyInt(5), 5.into());
    assert_eq!(Ok(MyInt(5)), "5".parse());
    assert_eq!(5, MyInt(5).into());
    assert_eq!(MyInt(5), MyInt::new(5));
    assert_eq!(-MyInt(5), (-5).into());
    assert_eq!("30", MyInt(30).to_string());
    assert_eq!("36", format!("{:o}", MyInt(30)));
    assert_eq!("100", format!("{:b}", MyInt(4)));
    assert_eq!(!MyBool(true), false.into());
    assert_eq!(MyIntEnum::SmallInt(5), 5.into());

    assert_eq!(SimpleStruct { int1: 5 }, 5.into());
    assert_eq!(5u64, SimpleStruct { int1: 5 }.into());
    assert_eq!(Ok(SimpleStruct { int1: 5 }), "5".parse());
    assert_eq!("5", SimpleStruct { int1: 5 }.to_string());
    assert_eq!(NormalStruct { int1: 5, int2: 6 }, (5, 6).into());
    assert_eq!(SimpleStruct { int1: 5 }, SimpleStruct::new(5));
    assert_eq!(NormalStruct { int1: 5, int2: 6 }, NormalStruct::new(5, 6));
    assert_eq!((5, 6), NormalStruct::new(5, 6).into());
    let mut norm_struct = NormalStruct::new(5, 6);
    let uints_ref: (&u64, &u64) = (&norm_struct).into();
    assert_eq!((&5, &6), uints_ref);
    let uints_ref_mut: (&mut u64, &mut u64) = (&mut norm_struct).into();
    assert_eq!((&mut 5, &mut 6), uints_ref_mut);

    assert_eq!(MyInt(4) + MyInt(1), 5.into());
    myint += MyInt(3);
    assert_eq!(myint, 8.into());
    myint *= 5;
    assert_eq!(myint, 40.into());
    assert_eq!(MyInt(4) + MyInt(1), 5.into());
    assert_eq!(MyUInt(4, 5) + MyUInt(1, 2), MyUInt(5, 7));
    assert_eq!(MyUInt(4, 5), MyUInt::new(4, 5));
    assert_eq!((4, 5), MyUInt(4, 5).into());
    let mut s1 = NormalStruct { int1: 1, int2: 2 };
    let s2 = NormalStruct { int1: 2, int2: 3 };
    let s3 = NormalStruct { int1: 3, int2: 5 };
    assert_eq!(s1 + s2, s3);
    assert_eq!(s3 - s2, s1);
    s1 += s2;
    assert_eq!(s1, s3);

    assert_eq!((SimpleMyIntEnum::Int(6) + 5.into()).unwrap(), 11.into());
    assert_eq!((SimpleMyIntEnum::Int(6) - 5.into()).unwrap(), 1.into());
    assert_eq!((SimpleMyIntEnum::Int(6) - 5.into()).unwrap(), 1.into());
    assert_eq!(-SimpleSignedIntEnum::Int(6), (-6i32).into());
    assert_eq!(
        (SimpleEnum::LabeledInts { a: 6, b: 5 }
            + SimpleEnum::LabeledInts { a: 1, b: 4 })
        .unwrap(),
        SimpleEnum::LabeledInts { a: 7, b: 9 }
    );

    let _ = (MyIntEnum::SmallInt(5) + 6.into()).unwrap();
    assert_eq!((-SimpleEnum::Int(5)).unwrap(), (-5).into());

    assert_eq!(MyInt(50), MyInt(5) * 10);
    assert_eq!(DoubleUInt(5, 6) * 10, DoubleUInt(50, 60));
    assert_eq!(
        DoubleUIntStruct { x: 5, y: 6 } * 10,
        DoubleUIntStruct { x: 50, y: 60 }
    );

    let mut myint = MyInt(5);
    assert_eq!(5, *myint);
    *myint = 7;
    assert_eq!(MyInt(7), myint);

    let mut my_vec = MyVec(vec![5, 8]);
    assert_eq!(5, my_vec[0]);
    assert_eq!(8, my_vec[1]);
    my_vec[0] = 20;
    assert_eq!(20, my_vec[0]);
    assert_eq!((&my_vec).into_iter().next(), Some(&20));
    assert_eq!((&mut my_vec).into_iter().next(), Some(&mut 20));
    assert_eq!(my_vec.into_iter().next(), Some(20));

    let int_vec = vec![MyInt2(2), MyInt2(3)];
    assert_eq!(MyInt2(5), int_vec.clone().into_iter().sum());
    assert_eq!(MyInt2(6), int_vec.clone().into_iter().product());
    let mut myint2 = MyInt2(8);
    myint2 *= MyInt2(4);
    assert_eq!(MyInt2(32), myint2);

    let mut boxed = MyBoxedInt(Box::new(5));
    assert_eq!(5, *boxed);
    *boxed = 7;
    assert_eq!(MyBoxedInt(Box::new(7)), boxed)
}
