// The code in this module is derived from the `lexical` crate by @Alexhuszagh
// which the author condensed into this minimal subset for use in serde_json.
// For the serde_json use case we care more about reliably round tripping all
// possible floating point values than about parsing any arbitrarily long string
// of digits with perfect accuracy, as the latter would take a high cost in
// compile time and performance.
//
// Dual licensed as MIT and Apache 2.0 just like the rest of serde_json, but
// copyright Alexander Huszagh.

//! Fast, minimal float-parsing algorithm.

// MODULES
pub(crate) mod algorithm;
mod bhcomp;
mod bignum;
mod cached;
mod cached_float80;
mod digit;
mod errors;
pub(crate) mod exponent;
pub(crate) mod float;
mod large_powers;
pub(crate) mod math;
pub(crate) mod num;
pub(crate) mod parse;
pub(crate) mod rounding;
mod shift;
mod small_powers;

#[cfg(fast_arithmetic = "32")]
mod large_powers32;

#[cfg(fast_arithmetic = "64")]
mod large_powers64;

// API
pub use self::parse::{parse_concise_float, parse_truncated_float};
