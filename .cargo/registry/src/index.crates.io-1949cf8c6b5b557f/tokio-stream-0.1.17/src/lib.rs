#![allow(unknown_lints, unexpected_cfgs)]
#![allow(
    clippy::cognitive_complexity,
    clippy::large_enum_variant,
    clippy::needless_doctest_main
)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

//! Stream utilities for Tokio.
//!
//! A `Stream` is an asynchronous sequence of values. It can be thought of as
//! an asynchronous version of the standard library's `Iterator` trait.
//!
//! This crate provides helpers to work with them. For examples of usage and a more in-depth
//! description of streams you can also refer to the [streams
//! tutorial](https://tokio.rs/tokio/tutorial/streams) on the tokio website.
//!
//! # Iterating over a Stream
//!
//! Due to similarities with the standard library's `Iterator` trait, some new
//! users may assume that they can use `for in` syntax to iterate over a
//! `Stream`, but this is unfortunately not possible. Instead, you can use a
//! `while let` loop as follows:
//!
//! ```rust
//! use tokio_stream::{self as stream, StreamExt};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut stream = stream::iter(vec![0, 1, 2]);
//!
//!     while let Some(value) = stream.next().await {
//!         println!("Got {}", value);
//!     }
//! }
//! ```
//!
//! # Returning a Stream from a function
//!
//! A common way to stream values from a function is to pass in the sender
//! half of a channel and use the receiver as the stream. This requires awaiting
//! both futures to ensure progress is made. Another alternative is the
//! [async-stream] crate, which contains macros that provide a `yield` keyword
//! and allow you to return an `impl Stream`.
//!
//! [async-stream]: https://docs.rs/async-stream
//!
//! # Conversion to and from `AsyncRead`/`AsyncWrite`
//!
//! It is often desirable to convert a `Stream` into an [`AsyncRead`],
//! especially when dealing with plaintext formats streamed over the network.
//! The opposite conversion from an [`AsyncRead`] into a `Stream` is also
//! another commonly required feature. To enable these conversions,
//! [`tokio-util`] provides the [`StreamReader`] and [`ReaderStream`]
//! types when the io feature is enabled.
//!
//! [`tokio-util`]: https://docs.rs/tokio-util/latest/tokio_util/codec/index.html
//! [`tokio::io`]: https://docs.rs/tokio/latest/tokio/io/index.html
//! [`AsyncRead`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncRead.html
//! [`AsyncWrite`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html
//! [`ReaderStream`]: https://docs.rs/tokio-util/latest/tokio_util/io/struct.ReaderStream.html
//! [`StreamReader`]: https://docs.rs/tokio-util/latest/tokio_util/io/struct.StreamReader.html

#[macro_use]
mod macros;

pub mod wrappers;

mod stream_ext;
pub use stream_ext::{collect::FromStream, StreamExt};
/// Adapters for [`Stream`]s created by methods in [`StreamExt`].
pub mod adapters {
    pub use crate::stream_ext::{
        Chain, Filter, FilterMap, Fuse, Map, MapWhile, Merge, Peekable, Skip, SkipWhile, Take,
        TakeWhile, Then,
    };
    cfg_time! {
        pub use crate::stream_ext::{ChunksTimeout, Timeout, TimeoutRepeating};
    }
}

cfg_time! {
    #[deprecated = "Import those symbols from adapters instead"]
    #[doc(hidden)]
    pub use stream_ext::timeout::Timeout;
    pub use stream_ext::timeout::Elapsed;
}

mod empty;
pub use empty::{empty, Empty};

mod iter;
pub use iter::{iter, Iter};

mod once;
pub use once::{once, Once};

mod pending;
pub use pending::{pending, Pending};

mod stream_map;
pub use stream_map::StreamMap;

mod stream_close;
pub use stream_close::StreamNotifyClose;

#[doc(no_inline)]
pub use futures_core::Stream;
