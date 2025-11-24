#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Constructor;

#[derive(Constructor)]
struct EmptyTuple();

const EMPTY_TUPLE: EmptyTuple = EmptyTuple::new();

#[derive(Constructor)]
struct EmptyStruct {}

const EMPTY_STRUCT: EmptyStruct = EmptyStruct::new();

#[derive(Constructor)]
struct EmptyUnit;

const EMPTY_UNIT: EmptyUnit = EmptyUnit::new();

#[derive(Constructor)]
struct MyInts(i32, i32);

const MY_INTS: MyInts = MyInts::new(1, 2);

#[derive(Constructor)]
struct Point2D {
    x: i32,
    y: i32,
}

const POINT_2D: Point2D = Point2D::new(-4, 7);
