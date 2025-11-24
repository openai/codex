/*!
A module with low-level architecture dependent routines.

These routines are useful as primitives for tasks not covered by the higher
level crate API.
*/

pub mod all;
pub(crate) mod generic;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm32;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;
