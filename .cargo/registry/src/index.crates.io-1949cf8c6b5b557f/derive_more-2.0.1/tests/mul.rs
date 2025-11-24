#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Mul;

#[derive(Mul)]
struct MyInt(i32);

#[derive(Mul)]
struct MyInts(i32, i32);

#[derive(Mul)]
struct Point1D {
    x: i32,
}

#[derive(Mul)]
struct Point2D {
    x: i32,
    y: i32,
}
