//! Import of the unstable private `Content` type from `serde`.
//!
//! <https://github.com/serde-rs/serde/blob/55a7cedd737278a9d75a2efd038c6f38b8c38bd6/serde/src/private/ser.rs#L338-L997>

#![cfg(not(tarpaulin_include))]

pub(crate) mod de;
pub(crate) mod ser;
