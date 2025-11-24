//! [`Stream<Item = Request>`][stream] + [`Service<Request>`] => [`Stream<Item = Response>`][stream].
//!
//! [`Service<Request>`]: crate::Service
//! [stream]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html

mod common;
mod ordered;
mod unordered;

#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::{ordered::CallAll, unordered::CallAllUnordered};
