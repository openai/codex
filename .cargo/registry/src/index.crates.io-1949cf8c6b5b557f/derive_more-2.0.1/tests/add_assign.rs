#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::AddAssign;

#[derive(AddAssign)]
struct MyInts(i32, i32);

#[derive(AddAssign)]
struct Point2D {
    x: i32,
    y: i32,
}
