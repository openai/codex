#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]

//! Asynchronous stream of elements.
//!
//! Provides two macros, `stream!` and `try_stream!`, allowing the caller to
//! define asynchronous streams of elements. These are implemented using `async`
//! & `await` notation. This crate works without unstable features.
//!
//! The `stream!` macro returns an anonymous type implementing the [`Stream`]
//! trait. The `Item` associated type is the type of the values yielded from the
//! stream. The `try_stream!` also returns an anonymous type implementing the
//! [`Stream`] trait, but the `Item` associated type is `Result<T, Error>`. The
//! `try_stream!` macro supports using `?` notation as part of the
//! implementation.
//!
//! # Usage
//!
//! A basic stream yielding numbers. Values are yielded using the `yield`
//! keyword. The stream block must return `()`.
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     };
//!
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! Streams may be returned by using `impl Stream<Item = T>`:
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_core::stream::Stream;
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = zero_to_three();
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! Streams may be implemented in terms of other streams - `async-stream` provides `for await`
//! syntax to assist with this:
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_core::stream::Stream;
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     }
//! }
//!
//! fn double<S: Stream<Item = u32>>(input: S)
//!     -> impl Stream<Item = u32>
//! {
//!     stream! {
//!         for await value in input {
//!             yield value * 2;
//!         }
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = double(zero_to_three());
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! Rust try notation (`?`) can be used with the `try_stream!` macro. The `Item`
//! of the returned stream is `Result` with `Ok` being the value yielded and
//! `Err` the error type returned by `?`.
//!
//! ```rust
//! use tokio::net::{TcpListener, TcpStream};
//!
//! use async_stream::try_stream;
//! use futures_core::stream::Stream;
//!
//! use std::io;
//! use std::net::SocketAddr;
//!
//! fn bind_and_accept(addr: SocketAddr)
//!     -> impl Stream<Item = io::Result<TcpStream>>
//! {
//!     try_stream! {
//!         let mut listener = TcpListener::bind(addr).await?;
//!
//!         loop {
//!             let (stream, addr) = listener.accept().await?;
//!             println!("received on {:?}", addr);
//!             yield stream;
//!         }
//!     }
//! }
//! ```
//!
//! # Implementation
//!
//! The `stream!` and `try_stream!` macros are implemented using proc macros.
//! The macro searches the syntax tree for instances of `yield $expr` and
//! transforms them into `sender.send($expr).await`.
//!
//! The stream uses a lightweight sender to send values from the stream
//! implementation to the caller. When entering the stream, an `Option<T>` is
//! stored on the stack. A pointer to the cell is stored in a thread local and
//! `poll` is called on the async block. When `poll` returns.
//! `sender.send(value)` stores the value that cell and yields back to the
//! caller.
//!
//! [`Stream`]: https://docs.rs/futures-core/*/futures_core/stream/trait.Stream.html

mod async_stream;
mod next;
mod yielder;

/// Asynchronous stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```
/// use async_stream::stream;
///
/// use futures_util::pin_mut;
/// use futures_util::stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let s = stream! {
///         for i in 0..3 {
///             yield i;
///         }
///     };
///
///     pin_mut!(s); // needed for iteration
///
///     while let Some(value) = s.next().await {
///         println!("got {}", value);
///     }
/// }
/// ```
#[macro_export]
macro_rules! stream {
    ($($tt:tt)*) => {
        $crate::__private::stream_inner!(($crate) $($tt)*)
    }
}

/// Asynchronous fallible stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```
/// use tokio::net::{TcpListener, TcpStream};
///
/// use async_stream::try_stream;
/// use futures_core::stream::Stream;
///
/// use std::io;
/// use std::net::SocketAddr;
///
/// fn bind_and_accept(addr: SocketAddr)
///     -> impl Stream<Item = io::Result<TcpStream>>
/// {
///     try_stream! {
///         let mut listener = TcpListener::bind(addr).await?;
///
///         loop {
///             let (stream, addr) = listener.accept().await?;
///             println!("received on {:?}", addr);
///             yield stream;
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! try_stream {
    ($($tt:tt)*) => {
        $crate::__private::try_stream_inner!(($crate) $($tt)*)
    }
}

// Not public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::async_stream::AsyncStream;
    pub use crate::next::next;
    pub use async_stream_impl::{stream_inner, try_stream_inner};
    pub mod yielder {
        pub use crate::yielder::pair;
    }
}
