#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Add;

#[derive(Add)]
struct MyInts(i32, i32);

#[derive(Add)]
struct Point2D {
    x: i32,
    y: i32,
}

#[derive(Add)]
enum MixedInts {
    SmallInt(i32),
    BigInt(i64),
    TwoSmallInts(i32, i32),
    NamedSmallInts { x: i32, y: i32 },
    UnsignedOne(u32),
    UnsignedTwo(u32),
    Unit,
}
