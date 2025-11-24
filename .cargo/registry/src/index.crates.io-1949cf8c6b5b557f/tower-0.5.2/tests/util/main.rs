#![cfg(feature = "util")]
#![allow(clippy::type_complexity)]

mod call_all;
mod oneshot;
mod service_fn;
#[path = "../support.rs"]
pub(crate) mod support;
