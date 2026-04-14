#![deny(clippy::print_stdout, clippy::print_stderr)]

#[cfg(not(target_arch = "wasm32"))]
include!("native.rs");

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
