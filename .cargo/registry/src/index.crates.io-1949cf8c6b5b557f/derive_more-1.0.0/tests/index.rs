#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use derive_more::Index;

#[derive(Index)]
struct MyVec(Vec<i32>);

#[derive(Index)]
struct Numbers {
    #[index]
    numbers: Vec<i32>,
    useless: bool,
}
