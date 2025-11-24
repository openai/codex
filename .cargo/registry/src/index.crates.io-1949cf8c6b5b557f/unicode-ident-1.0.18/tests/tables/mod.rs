#![allow(clippy::module_inception)]

#[allow(clippy::redundant_static_lifetimes)]
#[rustfmt::skip]
mod tables;

pub(crate) use self::tables::*;
