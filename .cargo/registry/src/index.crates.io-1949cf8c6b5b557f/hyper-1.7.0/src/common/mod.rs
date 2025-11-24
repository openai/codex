#[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
pub(crate) mod buf;
#[cfg(all(feature = "server", any(feature = "http1", feature = "http2")))]
pub(crate) mod date;
#[cfg(all(feature = "client", feature = "http2"))]
pub(crate) mod either;
#[cfg(any(
    all(feature = "client", any(feature = "http1", feature = "http2")),
    all(feature = "server", feature = "http1"),
))]
pub(crate) mod future;
pub(crate) mod io;
#[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
pub(crate) mod task;
#[cfg(any(
    all(feature = "server", feature = "http1"),
    all(any(feature = "client", feature = "server"), feature = "http2"),
))]
pub(crate) mod time;
#[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
pub(crate) mod watch;
