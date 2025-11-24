//! Modules providing SSL/TLS implementations. For backwards compatibility, OpenSSL is the default
//! implementation, but Rustls is highly recommended as a pure Rust alternative.
//!
//! In order to simplify the swappable implementations these SSL/TLS modules adhere to an implicit
//! trait contract and specific implementations are re-exported as [`SslContextImpl`] and [`SslStream`].
//! The concrete type of these aliases will depend on which module you enable in `Cargo.toml`.

#[cfg(feature = "ssl-openssl")]
pub(crate) mod openssl;
#[cfg(feature = "ssl-openssl")]
pub(crate) use self::openssl::OpenSslContext as SslContextImpl;
#[cfg(feature = "ssl-openssl")]
pub(crate) use self::openssl::SplitOpenSslStream as SslStream;

#[cfg(feature = "ssl-rustls")]
pub(crate) mod rustls;
#[cfg(feature = "ssl-rustls")]
pub(crate) use self::rustls::RustlsContext as SslContextImpl;
#[cfg(feature = "ssl-rustls")]
pub(crate) use self::rustls::RustlsStream as SslStream;
