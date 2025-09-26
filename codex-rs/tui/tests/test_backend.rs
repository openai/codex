#![cfg(feature = "vt100-tests")]

#[path = "../src/test_backend.rs"]
mod inner;

pub use inner::VT100Backend;
