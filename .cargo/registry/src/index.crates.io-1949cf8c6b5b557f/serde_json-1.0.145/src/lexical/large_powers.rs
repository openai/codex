// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Precalculated large powers for limbs.

#[cfg(fast_arithmetic = "32")]
pub(crate) use super::large_powers32::*;

#[cfg(fast_arithmetic = "64")]
pub(crate) use super::large_powers64::*;
