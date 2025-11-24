#[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
mod compat;
mod rewind;

#[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
pub(crate) use self::compat::Compat;
pub(crate) use self::rewind::Rewind;
