//! The `stream_select` macro.

#[allow(unreachable_pub)]
#[doc(hidden)]
pub use futures_macro::stream_select_internal;

#[allow(clippy::too_long_first_doc_paragraph)]
/// Combines several streams, all producing the same `Item` type, into one stream.
/// This is similar to `select_all` but does not require the streams to all be the same type.
/// It also keeps the streams inline, and does not require `Box<dyn Stream>`s to be allocated.
/// Streams passed to this macro must be `Unpin`.
///
/// If multiple streams are ready, one will be pseudo randomly selected at runtime.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::{stream, StreamExt, stream_select};
/// let endless_ints = |i| stream::iter(vec![i].into_iter().cycle()).fuse();
///
/// let mut endless_numbers = stream_select!(endless_ints(1i32), endless_ints(2), endless_ints(3));
/// match endless_numbers.next().await {
///     Some(1) => println!("Got a 1"),
///     Some(2) => println!("Got a 2"),
///     Some(3) => println!("Got a 3"),
///     _ => unreachable!(),
/// }
/// # });
/// ```
#[macro_export]
macro_rules! stream_select {
    ($($tokens:tt)*) => {{
        use $crate::__private as __futures_crate;
        $crate::stream_select_internal! {
            $( $tokens )*
        }
    }}
}
