//! # Rustls - a modern TLS library
//!
//! Rustls is a TLS library that aims to provide a good level of cryptographic security,
//! requires no configuration to achieve that security, and provides no unsafe features or
//! obsolete cryptography by default.
//!
//! Rustls implements TLS1.2 and TLS1.3 for both clients and servers. See [the full
//! list of protocol features](manual::_04_features).
//!
//! ### Platform support
//!
//! While Rustls itself is platform independent, by default it uses [`aws-lc-rs`] for implementing
//! the cryptography in TLS.  See [the aws-lc-rs FAQ][aws-lc-rs-platforms-faq] for more details of the
//! platform/architecture support constraints in aws-lc-rs.
//!
//! [`ring`] is also available via the `ring` crate feature: see
//! [the supported `ring` target platforms][ring-target-platforms].
//!
//! By providing a custom instance of the [`crypto::CryptoProvider`] struct, you
//! can replace all cryptography dependencies of rustls.  This is a route to being portable
//! to a wider set of architectures and environments, or compliance requirements.  See the
//! [`crypto::CryptoProvider`] documentation for more details.
//!
//! Specifying `default-features = false` when depending on rustls will remove the implicit
//! dependency on aws-lc-rs.
//!
//! Rustls requires Rust 1.71 or later. It has an optional dependency on zlib-rs which requires 1.75 or later.
//!
//! [ring-target-platforms]: https://github.com/briansmith/ring/blob/2e8363b433fa3b3962c877d9ed2e9145612f3160/include/ring-core/target.h#L18-L64
//! [`crypto::CryptoProvider`]: crate::crypto::CryptoProvider
//! [`ring`]: https://crates.io/crates/ring
//! [aws-lc-rs-platforms-faq]: https://aws.github.io/aws-lc-rs/faq.html#can-i-run-aws-lc-rs-on-x-platform-or-architecture
//! [`aws-lc-rs`]: https://crates.io/crates/aws-lc-rs
//!
//! ### Cryptography providers
//!
//! Since Rustls 0.22 it has been possible to choose the provider of the cryptographic primitives
//! that Rustls uses. This may be appealing if you have specific platform, compliance or feature
//! requirements that aren't met by the default provider, [`aws-lc-rs`].
//!
//! Users that wish to customize the provider in use can do so when constructing `ClientConfig`
//! and `ServerConfig` instances using the `with_crypto_provider` method on the respective config
//! builder types. See the [`crypto::CryptoProvider`] documentation for more details.
//!
//! #### Built-in providers
//!
//! Rustls ships with two built-in providers controlled by associated crate features:
//!
//!   * [`aws-lc-rs`] - enabled by default, available with the `aws_lc_rs` crate feature enabled.
//!   * [`ring`] - available with the `ring` crate feature enabled.
//!
//! See the documentation for [`crypto::CryptoProvider`] for details on how providers are
//! selected.
//!
//! #### Third-party providers
//!
//! The community has also started developing third-party providers for Rustls:
//!
//!   * [`boring-rustls-provider`] - a work-in-progress provider that uses [`boringssl`] for
//!     cryptography.
//!   * [`rustls-graviola`] - a provider that uses [`graviola`] for cryptography.
//!   * [`rustls-mbedtls-provider`] - a provider that uses [`mbedtls`] for cryptography.
//!   * [`rustls-openssl`] - a provider that uses [OpenSSL] for cryptography.
//!   * [`rustls-rustcrypto`] - an experimental provider that uses the crypto primitives
//!     from [`RustCrypto`] for cryptography.
//!   * [`rustls-symcrypt`] - a provider that uses Microsoft's [SymCrypt] library.
//!   * [`rustls-wolfcrypt-provider`] - a work-in-progress provider that uses [`wolfCrypt`] for cryptography.
//!
//! [`rustls-graviola`]: https://crates.io/crates/rustls-graviola
//! [`graviola`]: https://github.com/ctz/graviola
//! [`rustls-mbedtls-provider`]: https://github.com/fortanix/rustls-mbedtls-provider
//! [`mbedtls`]: https://github.com/Mbed-TLS/mbedtls
//! [`rustls-openssl`]: https://github.com/tofay/rustls-openssl
//! [OpenSSL]: https://openssl-library.org/
//! [`rustls-symcrypt`]: https://github.com/microsoft/rustls-symcrypt
//! [SymCrypt]: https://github.com/microsoft/SymCrypt
//! [`boring-rustls-provider`]: https://github.com/janrueth/boring-rustls-provider
//! [`boringssl`]: https://github.com/google/boringssl
//! [`rustls-rustcrypto`]: https://github.com/RustCrypto/rustls-rustcrypto
//! [`RustCrypto`]: https://github.com/RustCrypto
//! [`rustls-wolfcrypt-provider`]: https://github.com/wolfSSL/rustls-wolfcrypt-provider
//! [`wolfCrypt`]: https://www.wolfssl.com/products/wolfcrypt
//!
//! #### Custom provider
//!
//! We also provide a simple example of writing your own provider in the [custom provider example].
//! This example implements a minimal provider using parts of the [`RustCrypto`] ecosystem.
//!
//! See the [Making a custom CryptoProvider] section of the documentation for more information
//! on this topic.
//!
//! [custom provider example]: https://github.com/rustls/rustls/tree/main/provider-example/
//! [`RustCrypto`]: https://github.com/RustCrypto
//! [Making a custom CryptoProvider]: https://docs.rs/rustls/latest/rustls/crypto/struct.CryptoProvider.html#making-a-custom-cryptoprovider
//!
//! ## Design overview
//!
//! Rustls is a low-level library. If your goal is to make HTTPS connections you may prefer
//! to use a library built on top of Rustls like [hyper] or [ureq].
//!
//! [hyper]: https://crates.io/crates/hyper
//! [ureq]: https://crates.io/crates/ureq
//!
//! ### Rustls does not take care of network IO
//! It doesn't make or accept TCP connections, or do DNS, or read or write files.
//!
//! Our [examples] directory contains demos that show how to handle I/O using the
//! [`stream::Stream`] helper, as well as more complex asynchronous I/O using [`mio`].
//! If you're already using Tokio for an async runtime you may prefer to use [`tokio-rustls`] instead
//! of interacting with rustls directly.
//!
//! [examples]: https://github.com/rustls/rustls/tree/main/examples
//! [`tokio-rustls`]: https://github.com/rustls/tokio-rustls
//!
//! ### Rustls provides encrypted pipes
//! These are the [`ServerConnection`] and [`ClientConnection`] types.  You supply raw TLS traffic
//! on the left (via the [`read_tls()`] and [`write_tls()`] methods) and then read/write the
//! plaintext on the right:
//!
//! [`read_tls()`]: Connection::read_tls
//! [`write_tls()`]: Connection::read_tls
//!
//! ```text
//!          TLS                                   Plaintext
//!          ===                                   =========
//!     read_tls()      +-----------------------+      reader() as io::Read
//!                     |                       |
//!           +--------->   ClientConnection    +--------->
//!                     |          or           |
//!           <---------+   ServerConnection    <---------+
//!                     |                       |
//!     write_tls()     +-----------------------+      writer() as io::Write
//! ```
//!
//! ### Rustls takes care of server certificate verification
//! You do not need to provide anything other than a set of root certificates to trust.
//! Certificate verification cannot be turned off or disabled in the main API.
//!
//! ## Getting started
//! This is the minimum you need to do to make a TLS client connection.
//!
//! First we load some root certificates.  These are used to authenticate the server.
//! The simplest way is to depend on the [`webpki_roots`] crate which contains
//! the Mozilla set of root certificates.
//!
//! ```rust,no_run
//! # #[cfg(feature = "aws-lc-rs")] {
//! let root_store = rustls::RootCertStore::from_iter(
//!     webpki_roots::TLS_SERVER_ROOTS
//!         .iter()
//!         .cloned(),
//! );
//! # }
//! ```
//!
//! [`webpki_roots`]: https://crates.io/crates/webpki-roots
//!
//! Next, we make a `ClientConfig`.  You're likely to make one of these per process,
//! and use it for all connections made by that process.
//!
//! ```rust,no_run
//! # #[cfg(feature = "aws_lc_rs")] {
//! # let root_store: rustls::RootCertStore = panic!();
//! let config = rustls::ClientConfig::builder()
//!     .with_root_certificates(root_store)
//!     .with_no_client_auth();
//! # }
//! ```
//!
//! Now we can make a connection.  You need to provide the server's hostname so we
//! know what to expect to find in the server's certificate.
//!
//! ```rust
//! # #[cfg(feature = "aws_lc_rs")] {
//! # use rustls;
//! # use webpki;
//! # use std::sync::Arc;
//! # rustls::crypto::aws_lc_rs::default_provider().install_default();
//! # let root_store = rustls::RootCertStore::from_iter(
//! #  webpki_roots::TLS_SERVER_ROOTS
//! #      .iter()
//! #      .cloned(),
//! # );
//! # let config = rustls::ClientConfig::builder()
//! #     .with_root_certificates(root_store)
//! #     .with_no_client_auth();
//! let rc_config = Arc::new(config);
//! let example_com = "example.com".try_into().unwrap();
//! let mut client = rustls::ClientConnection::new(rc_config, example_com);
//! # }
//! ```
//!
//! Now you should do appropriate IO for the `client` object.  If `client.wants_read()` yields
//! true, you should call `client.read_tls()` when the underlying connection has data.
//! Likewise, if `client.wants_write()` yields true, you should call `client.write_tls()`
//! when the underlying connection is able to send data.  You should continue doing this
//! as long as the connection is valid.
//!
//! The return types of `read_tls()` and `write_tls()` only tell you if the IO worked.  No
//! parsing or processing of the TLS messages is done.  After each `read_tls()` you should
//! therefore call `client.process_new_packets()` which parses and processes the messages.
//! Any error returned from `process_new_packets` is fatal to the connection, and will tell you
//! why.  For example, if the server's certificate is expired `process_new_packets` will
//! return `Err(InvalidCertificate(Expired))`.  From this point on,
//! `process_new_packets` will not do any new work and will return that error continually.
//!
//! You can extract newly received data by calling `client.reader()` (which implements the
//! `io::Read` trait).  You can send data to the peer by calling `client.writer()` (which
//! implements `io::Write` trait).  Note that `client.writer().write()` buffers data you
//! send if the TLS connection is not yet established: this is useful for writing (say) a
//! HTTP request, but this is buffered so avoid large amounts of data.
//!
//! The following code uses a fictional socket IO API for illustration, and does not handle
//! errors.
//!
//! ```rust,no_run
//! # #[cfg(feature = "aws_lc_rs")] {
//! # let mut client = rustls::ClientConnection::new(panic!(), panic!()).unwrap();
//! # struct Socket { }
//! # impl Socket {
//! #   fn ready_for_write(&self) -> bool { false }
//! #   fn ready_for_read(&self) -> bool { false }
//! #   fn wait_for_something_to_happen(&self) { }
//! # }
//! #
//! # use std::io::{Read, Write, Result};
//! # impl Read for Socket {
//! #   fn read(&mut self, buf: &mut [u8]) -> Result<usize> { panic!() }
//! # }
//! # impl Write for Socket {
//! #   fn write(&mut self, buf: &[u8]) -> Result<usize> { panic!() }
//! #   fn flush(&mut self) -> Result<()> { panic!() }
//! # }
//! #
//! # fn connect(_address: &str, _port: u16) -> Socket {
//! #   panic!();
//! # }
//! use std::io;
//! use rustls::Connection;
//!
//! client.writer().write(b"GET / HTTP/1.0\r\n\r\n").unwrap();
//! let mut socket = connect("example.com", 443);
//! loop {
//!   if client.wants_read() && socket.ready_for_read() {
//!     client.read_tls(&mut socket).unwrap();
//!     client.process_new_packets().unwrap();
//!
//!     let mut plaintext = Vec::new();
//!     client.reader().read_to_end(&mut plaintext).unwrap();
//!     io::stdout().write(&plaintext).unwrap();
//!   }
//!
//!   if client.wants_write() && socket.ready_for_write() {
//!     client.write_tls(&mut socket).unwrap();
//!   }
//!
//!   socket.wait_for_something_to_happen();
//! }
//! # }
//! ```
//!
//! # Examples
//!
//! You can find several client and server examples of varying complexity in the [examples]
//! directory, including [`tlsserver-mio`](https://github.com/rustls/rustls/blob/main/examples/src/bin/tlsserver-mio.rs)
//! and [`tlsclient-mio`](https://github.com/rustls/rustls/blob/main/examples/src/bin/tlsclient-mio.rs)
//! \- full worked examples using [`mio`].
//!
//! [`mio`]: https://docs.rs/mio/latest/mio/
//!
//! # Manual
//!
//! The [rustls manual](crate::manual) explains design decisions and includes how-to guidance.
//!
//! # Crate features
//! Here's a list of what features are exposed by the rustls crate and what
//! they mean.
//!
//! - `std` (enabled by default): enable the high-level (buffered) Connection API and other functionality
//!   which relies on the `std` library.
//!
//! - `aws_lc_rs` (enabled by default): makes the rustls crate depend on the [`aws-lc-rs`] crate.
//!   Use `rustls::crypto::aws_lc_rs::default_provider().install_default()` to
//!   use it as the default `CryptoProvider`, or provide it explicitly
//!   when making a `ClientConfig` or `ServerConfig`.
//!
//!   Note that aws-lc-rs has additional build-time dependencies like cmake.
//!   See [the documentation](https://aws.github.io/aws-lc-rs/requirements/index.html) for details.
//!
//! - `ring`: makes the rustls crate depend on the *ring* crate for cryptography.
//!   Use `rustls::crypto::ring::default_provider().install_default()` to
//!   use it as the default `CryptoProvider`, or provide it explicitly
//!   when making a `ClientConfig` or `ServerConfig`.
//!
//! - `fips`: enable support for FIPS140-3-approved cryptography, via the [`aws-lc-rs`] crate.
//!   This feature enables the `aws_lc_rs` crate feature, which makes the rustls crate depend
//!   on [aws-lc-rs](https://github.com/aws/aws-lc-rs).  It also changes the default
//!   for [`ServerConfig::require_ems`] and [`ClientConfig::require_ems`].
//!
//!   See [manual::_06_fips] for more details.
//!
//! - `prefer-post-quantum` (enabled by default): for the [`aws-lc-rs`]-backed provider,
//!   prioritizes post-quantum secure key exchange by default (using X25519MLKEM768).
//!   This feature merely alters the order of `rustls::crypto::aws_lc_rs::DEFAULT_KX_GROUPS`.
//!   See [the manual][x25519mlkem768-manual] for more details.
//!
//! - `custom-provider`: disables implicit use of built-in providers (`aws-lc-rs` or `ring`). This forces
//!   applications to manually install one, for instance, when using a custom `CryptoProvider`.
//!
//! - `tls12` (enabled by default): enable support for TLS version 1.2. Note that, due to the
//!   additive nature of Cargo features and because it is enabled by default, other crates
//!   in your dependency graph could re-enable it for your application. If you want to disable
//!   TLS 1.2 for security reasons, consider explicitly enabling TLS 1.3 only in the config
//!   builder API.
//!
//! - `logging` (enabled by default): make the rustls crate depend on the `log` crate.
//!   rustls outputs interesting protocol-level messages at `trace!` and `debug!` level,
//!   and protocol-level errors at `warn!` and `error!` level.  The log messages do not
//!   contain secret key data, and so are safe to archive without affecting session security.
//!
//! - `read_buf`: when building with Rust Nightly, adds support for the unstable
//!   `std::io::ReadBuf` and related APIs. This reduces costs from initializing
//!   buffers. Will do nothing on non-Nightly releases.
//!
//! - `brotli`: uses the `brotli` crate for RFC8879 certificate compression support.
//!
//! - `zlib`: uses the `zlib-rs` crate for RFC8879 certificate compression support.
//!
//! [x25519mlkem768-manual]: manual::_05_defaults#about-the-post-quantum-secure-key-exchange-x25519mlkem768

// Require docs for public APIs, deny unsafe code, etc.
#![forbid(unsafe_code, unused_must_use)]
#![cfg_attr(not(any(read_buf, bench, coverage_nightly)), forbid(unstable_features))]
#![warn(
    clippy::alloc_instead_of_core,
    clippy::manual_let_else,
    clippy::std_instead_of_core,
    clippy::use_self,
    clippy::upper_case_acronyms,
    elided_lifetimes_in_paths,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_import_braces,
    unused_extern_crates,
    unused_qualifications
)]
// Relax these clippy lints:
// - ptr_arg: this triggers on references to type aliases that are Vec
//   underneath.
// - too_many_arguments: some things just need a lot of state, wrapping it
//   doesn't necessarily make it easier to follow what's going on
// - new_ret_no_self: we sometimes return `Arc<Self>`, which seems fine
// - single_component_path_imports: our top-level `use log` import causes
//   a false positive, https://github.com/rust-lang/rust-clippy/issues/5210
// - new_without_default: for internal constructors, the indirection is not
//   helpful
#![allow(
    clippy::too_many_arguments,
    clippy::new_ret_no_self,
    clippy::ptr_arg,
    clippy::single_component_path_imports,
    clippy::new_without_default
)]
// Enable documentation for all features on docs.rs
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
// Enable coverage() attr for nightly coverage builds, see
// <https://github.com/rust-lang/rust/issues/84605>
// (`coverage_nightly` is a cfg set by `cargo-llvm-cov`)
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
// XXX: Because of https://github.com/rust-lang/rust/issues/54726, we cannot
// write `#![rustversion::attr(nightly, feature(read_buf))]` here. Instead,
// build.rs set `read_buf` for (only) Rust Nightly to get the same effect.
//
// All the other conditional logic in the crate could use
// `#[rustversion::nightly]` instead of `#[cfg(read_buf)]`; `#[cfg(read_buf)]`
// is used to avoid needing `rustversion` to be compiled twice during
// cross-compiling.
#![cfg_attr(read_buf, feature(read_buf))]
#![cfg_attr(read_buf, feature(core_io_borrowed_buf))]
#![cfg_attr(bench, feature(test))]
#![no_std]

extern crate alloc;
// This `extern crate` plus the `#![no_std]` attribute changes the default prelude from
// `std::prelude` to `core::prelude`. That forces one to _explicitly_ import (`use`) everything that
// is in `std::prelude` but not in `core::prelude`. This helps maintain no-std support as even
// developers that are not interested in, or aware of, no-std support and / or that never run
// `cargo build --no-default-features` locally will get errors when they rely on `std::prelude` API.
#[cfg(any(feature = "std", test))]
extern crate std;

#[cfg(doc)]
use crate::crypto::CryptoProvider;

// Import `test` sysroot crate for `Bencher` definitions.
#[cfg(bench)]
#[allow(unused_extern_crates)]
extern crate test;

// log for logging (optional).
#[cfg(feature = "logging")]
use log;

#[cfg(not(feature = "logging"))]
mod log {
    macro_rules! trace    ( ($($tt:tt)*) => {{}} );
    macro_rules! debug    ( ($($tt:tt)*) => {{}} );
    macro_rules! error    ( ($($tt:tt)*) => {{}} );
    macro_rules! _warn    ( ($($tt:tt)*) => {{}} );
    pub(crate) use {_warn as warn, debug, error, trace};
}

#[cfg(test)]
#[macro_use]
mod test_macros;

/// This internal `sync` module aliases the `Arc` implementation to allow downstream forks
/// of rustls targeting architectures without atomic pointers to replace the implementation
/// with another implementation such as `portable_atomic_util::Arc` in one central location.
mod sync {
    #[allow(clippy::disallowed_types)]
    pub(crate) type Arc<T> = alloc::sync::Arc<T>;
    #[allow(clippy::disallowed_types)]
    pub(crate) type Weak<T> = alloc::sync::Weak<T>;
}

#[macro_use]
mod msgs;
mod common_state;
pub mod compress;
mod conn;
/// Crypto provider interface.
pub mod crypto;
mod error;
mod hash_hs;
#[cfg(any(feature = "std", feature = "hashbrown"))]
mod limited_cache;
mod rand;
mod record_layer;
#[cfg(feature = "std")]
mod stream;
#[cfg(feature = "tls12")]
mod tls12;
mod tls13;
mod vecbuf;
mod verify;
#[cfg(test)]
mod verifybench;
mod x509;
#[macro_use]
mod check;
#[cfg(feature = "logging")]
mod bs_debug;
mod builder;
mod enums;
mod key_log;
#[cfg(feature = "std")]
mod key_log_file;
mod suites;
mod versions;
mod webpki;

/// Internal classes that are used in integration tests.
/// The contents of this section DO NOT form part of the stable interface.
#[allow(missing_docs)]
#[doc(hidden)]
pub mod internal {
    /// Low-level TLS message parsing and encoding functions.
    pub mod msgs {
        pub mod base {
            pub use crate::msgs::base::{Payload, PayloadU16};
        }
        pub mod codec {
            pub use crate::msgs::codec::{Codec, Reader};
        }
        pub mod enums {
            pub use crate::msgs::enums::{
                AlertLevel, EchVersion, ExtensionType, HpkeAead, HpkeKdf, HpkeKem,
            };
        }
        pub mod fragmenter {
            pub use crate::msgs::fragmenter::MessageFragmenter;
        }
        pub mod handshake {
            pub use crate::msgs::handshake::{
                EchConfigContents, EchConfigPayload, HpkeKeyConfig, HpkeSymmetricCipherSuite,
            };
        }
        pub mod message {
            pub use crate::msgs::message::{
                Message, MessagePayload, OutboundOpaqueMessage, PlainMessage,
            };
        }
        pub mod persist {
            pub use crate::msgs::persist::ServerSessionValue;
        }
    }

    pub use crate::tls13::key_schedule::{derive_traffic_iv, derive_traffic_key};

    pub mod fuzzing {
        pub use crate::msgs::deframer::fuzz_deframer;
    }
}

/// Unbuffered connection API
///
/// This is an alternative to the [`crate::ConnectionCommon`] API that does not internally buffer
/// TLS nor plaintext data. Instead those buffers are managed by the API user so they have
/// control over when and how to allocate, resize and dispose of them.
///
/// This API is lower level than the `ConnectionCommon` API and is built around a state machine
/// interface where the API user must handle each state to advance and complete the
/// handshake process.
///
/// Like the `ConnectionCommon` API, no IO happens internally so all IO must be handled by the API
/// user. Unlike the `ConnectionCommon` API, this API does not make use of the [`std::io::Read`] and
/// [`std::io::Write`] traits so it's usable in no-std context.
///
/// The entry points into this API are [`crate::client::UnbufferedClientConnection::new`],
/// [`crate::server::UnbufferedServerConnection::new`] and
/// [`unbuffered::UnbufferedConnectionCommon::process_tls_records`]. The state machine API is
/// documented in [`unbuffered::ConnectionState`].
///
/// # Examples
///
/// [`unbuffered-client`] and [`unbuffered-server`] are examples that fully exercise the API in
/// std, non-async context.
///
/// [`unbuffered-client`]: https://github.com/rustls/rustls/blob/main/examples/src/bin/unbuffered-client.rs
/// [`unbuffered-server`]: https://github.com/rustls/rustls/blob/main/examples/src/bin/unbuffered-server.rs
pub mod unbuffered {
    pub use crate::conn::UnbufferedConnectionCommon;
    pub use crate::conn::unbuffered::{
        AppDataRecord, ConnectionState, EncodeError, EncodeTlsData, EncryptError,
        InsufficientSizeError, ReadEarlyData, ReadTraffic, TransmitTlsData, UnbufferedStatus,
        WriteTraffic,
    };
}

// The public interface is:
pub use crate::builder::{ConfigBuilder, ConfigSide, WantsVerifier, WantsVersions};
pub use crate::common_state::{CommonState, HandshakeKind, IoState, Side};
#[cfg(feature = "std")]
pub use crate::conn::{Connection, Reader, Writer};
pub use crate::conn::{ConnectionCommon, SideData, kernel};
pub use crate::enums::{
    AlertDescription, CertificateCompressionAlgorithm, CipherSuite, ContentType, HandshakeType,
    ProtocolVersion, SignatureAlgorithm, SignatureScheme,
};
pub use crate::error::{
    CertRevocationListError, CertificateError, EncryptedClientHelloError, Error,
    ExtendedKeyPurpose, InconsistentKeys, InvalidMessage, OtherError, PeerIncompatible,
    PeerMisbehaved,
};
pub use crate::key_log::{KeyLog, NoKeyLog};
#[cfg(feature = "std")]
pub use crate::key_log_file::KeyLogFile;
pub use crate::msgs::enums::NamedGroup;
pub use crate::msgs::ffdhe_groups;
pub use crate::msgs::handshake::DistinguishedName;
#[cfg(feature = "std")]
pub use crate::stream::{Stream, StreamOwned};
pub use crate::suites::{
    CipherSuiteCommon, ConnectionTrafficSecrets, ExtractedSecrets, SupportedCipherSuite,
};
#[cfg(feature = "std")]
pub use crate::ticketer::TicketRotator;
#[cfg(any(feature = "std", feature = "hashbrown"))] // < XXX: incorrect feature gate
pub use crate::ticketer::TicketSwitcher;
#[cfg(feature = "tls12")]
pub use crate::tls12::Tls12CipherSuite;
pub use crate::tls13::Tls13CipherSuite;
pub use crate::verify::DigitallySignedStruct;
pub use crate::versions::{ALL_VERSIONS, DEFAULT_VERSIONS, SupportedProtocolVersion};
pub use crate::webpki::RootCertStore;

/// Items for use in a client.
pub mod client {
    pub(super) mod builder;
    mod client_conn;
    mod common;
    mod ech;
    pub(super) mod handy;
    mod hs;
    #[cfg(test)]
    mod test;
    #[cfg(feature = "tls12")]
    mod tls12;
    mod tls13;

    pub use builder::WantsClientCert;
    pub use client_conn::{
        ClientConfig, ClientConnectionData, ClientSessionStore, EarlyDataError, ResolvesClientCert,
        Resumption, Tls12Resumption, UnbufferedClientConnection,
    };
    #[cfg(feature = "std")]
    pub use client_conn::{ClientConnection, WriteEarlyData};
    pub use ech::{EchConfig, EchGreaseConfig, EchMode, EchStatus};
    pub use handy::AlwaysResolvesClientRawPublicKeys;
    #[cfg(any(feature = "std", feature = "hashbrown"))]
    pub use handy::ClientSessionMemoryCache;

    /// Dangerous configuration that should be audited and used with extreme care.
    pub mod danger {
        pub use super::builder::danger::DangerousClientConfigBuilder;
        pub use super::client_conn::danger::DangerousClientConfig;
        pub use crate::verify::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    }

    pub use crate::msgs::persist::{Tls12ClientSessionValue, Tls13ClientSessionValue};
    pub use crate::webpki::{
        ServerCertVerifierBuilder, VerifierBuilderError, WebPkiServerVerifier,
        verify_server_cert_signed_by_trust_anchor, verify_server_name,
    };
}

pub use client::ClientConfig;
#[cfg(feature = "std")]
pub use client::ClientConnection;

/// Items for use in a server.
pub mod server {
    pub(crate) mod builder;
    mod common;
    pub(crate) mod handy;
    mod hs;
    mod server_conn;
    #[cfg(test)]
    mod test;
    #[cfg(feature = "tls12")]
    mod tls12;
    mod tls13;

    pub use builder::WantsServerCert;
    #[cfg(any(feature = "std", feature = "hashbrown"))]
    pub use handy::ResolvesServerCertUsingSni;
    #[cfg(any(feature = "std", feature = "hashbrown"))]
    pub use handy::ServerSessionMemoryCache;
    pub use handy::{AlwaysResolvesServerRawPublicKeys, NoServerSessionStorage};
    pub use server_conn::{
        Accepted, ClientHello, ProducesTickets, ResolvesServerCert, ServerConfig,
        ServerConnectionData, StoresServerSessions, UnbufferedServerConnection,
    };
    #[cfg(feature = "std")]
    pub use server_conn::{AcceptedAlert, Acceptor, ReadEarlyData, ServerConnection};

    pub use crate::enums::CertificateType;
    pub use crate::verify::NoClientAuth;
    pub use crate::webpki::{
        ClientCertVerifierBuilder, ParsedCertificate, VerifierBuilderError, WebPkiClientVerifier,
    };

    /// Dangerous configuration that should be audited and used with extreme care.
    pub mod danger {
        pub use crate::verify::{ClientCertVerified, ClientCertVerifier};
    }
}

pub use server::ServerConfig;
#[cfg(feature = "std")]
pub use server::ServerConnection;

/// All defined protocol versions appear in this module.
///
/// ALL_VERSIONS is a provided as an array of all of these values.
pub mod version {
    #[cfg(feature = "tls12")]
    pub use crate::versions::TLS12;
    pub use crate::versions::TLS13;
}

/// Re-exports the contents of the [rustls-pki-types](https://docs.rs/rustls-pki-types) crate for easy access
pub mod pki_types {
    #[doc(no_inline)]
    pub use pki_types::*;
}

/// Message signing interfaces.
pub mod sign {
    pub use crate::crypto::signer::{CertifiedKey, Signer, SigningKey, SingleCertAndKey};
}

/// APIs for implementing QUIC TLS
pub mod quic;

#[cfg(any(feature = "std", feature = "hashbrown"))] // < XXX: incorrect feature gate
/// APIs for implementing TLS tickets
pub mod ticketer;

/// This is the rustls manual.
pub mod manual;

pub mod time_provider;

/// APIs abstracting over locking primitives.
pub mod lock;

/// Polyfills for features that are not yet stabilized or available with current MSRV.
pub(crate) mod polyfill;

#[cfg(any(feature = "std", feature = "hashbrown"))]
mod hash_map {
    #[cfg(feature = "std")]
    pub(crate) use std::collections::HashMap;
    #[cfg(feature = "std")]
    pub(crate) use std::collections::hash_map::Entry;

    #[cfg(all(not(feature = "std"), feature = "hashbrown"))]
    pub(crate) use hashbrown::HashMap;
    #[cfg(all(not(feature = "std"), feature = "hashbrown"))]
    pub(crate) use hashbrown::hash_map::Entry;
}
