//! Runtime utilities

#[cfg(feature = "client-legacy")]
mod io;
#[cfg(feature = "client-legacy")]
pub(crate) use self::io::{read, write_all};

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "tokio")]
pub use self::tokio::{TokioExecutor, TokioIo, TokioTimer};
