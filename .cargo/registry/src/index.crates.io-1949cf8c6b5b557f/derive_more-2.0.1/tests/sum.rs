#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::Sum;

#[derive(Sum)]
struct MyInts(i32, i64);

// `Add` implementation is required for `Sum`.
impl ::core::ops::Add for MyInts {
    type Output = MyInts;
    #[inline]
    fn add(self, rhs: MyInts) -> MyInts {
        MyInts(self.0.add(rhs.0), self.1.add(rhs.1))
    }
}

#[derive(Sum)]
struct Point2D {
    x: i32,
    y: i32,
}

// `Add` implementation is required for `Sum`.
impl ::core::ops::Add for Point2D {
    type Output = Point2D;
    #[inline]
    fn add(self, rhs: Point2D) -> Point2D {
        Point2D {
            x: self.x.add(rhs.x),
            y: self.y.add(rhs.y),
        }
    }
}
