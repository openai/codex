//! A module that contains the implementations for optional features. For example `serde` support

#[cfg(feature = "arbitrary")]
mod arbitrary;
#[cfg(feature = "borsh")]
mod borsh;
#[cfg(feature = "bytes")]
mod bytes;
#[cfg(feature = "diesel")]
mod diesel;
#[cfg(feature = "markup")]
mod markup;
#[cfg(feature = "proptest")]
mod proptest;
#[cfg(feature = "quickcheck")]
mod quickcheck;
#[cfg(feature = "rkyv")]
mod rkyv;
#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "smallvec")]
mod smallvec;
#[cfg(feature = "sqlx")]
mod sqlx;
