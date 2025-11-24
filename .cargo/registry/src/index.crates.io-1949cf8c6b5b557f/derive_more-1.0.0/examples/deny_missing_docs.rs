//! Some docs

#![deny(missing_docs)]
#![allow(dead_code)] // for illustration purposes

use derive_more::{
    Add, AddAssign, Constructor, Deref, DerefMut, Display, From, FromStr, Index,
    IndexMut, Into, IsVariant, Mul, MulAssign, Not, TryInto,
};

fn main() {}

/// Some docs
#[derive(
    Add,
    AddAssign,
    Constructor,
    Display,
    From,
    FromStr,
    Into,
    Mul,
    MulAssign,
    Not
)]
pub struct MyInt(i32);

/// Some docs
#[derive(Deref, DerefMut)]
pub struct MyBoxedInt(Box<i32>);

/// Some docs
#[derive(Index, IndexMut)]
pub struct MyVec(Vec<i32>);

/// Some docs
#[derive(Clone, Copy, TryInto)]
#[derive(IsVariant)]
enum MixedInts {
    SmallInt(i32),
    NamedBigInt { int: i64 },
}
