#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(nightly, feature(never_type))]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Not;

#[derive(Not)]
struct MyInts(i32, i32);

#[derive(Not)]
struct Point2D {
    x: i32,
    y: i32,
}

#[derive(Not)]
enum MixedInts {
    SmallInt(i32),
    BigInt(i64),
    TwoSmallInts(i32, i32),
    NamedSmallInts { x: i32, y: i32 },
    UnsignedOne(u32),
    UnsignedTwo(u32),
}

#[derive(Not)]
enum EnumWithUnit {
    SmallInt(i32),
    Unit,
}

#[cfg(nightly)]
mod never {
    use super::*;

    #[derive(Not)]
    struct Tuple(!);

    #[derive(Not)]
    struct Struct {
        field: !,
    }

    #[derive(Not)]
    struct TupleMulti(i32, !);

    #[derive(Not)]
    struct StructMulti {
        field: !,
        other: i32,
    }

    #[derive(Not)]
    enum Enum {
        Tuple(!),
        Struct { field: ! },
        TupleMulti(i32, !),
        StructMulti { field: !, other: i32 },
    }
}
