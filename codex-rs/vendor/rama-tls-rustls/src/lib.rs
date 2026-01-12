//! TLS implementations for Rama using rustls.
//!
//! # Rama
//!
//! Crate used by the end-user `rama` crate and `rama` crate authors alike.
//!
//! Learn more about `rama`:
//!
//! - Github: <https://github.com/plabayo/rama>
//! - Book: <https://ramaproxy.org/book/>

#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/plabayo/rama/main/docs/img/old_logo.png"
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/plabayo/rama/main/docs/img/old_logo.png")]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

pub mod client;
pub mod server;
pub mod verify;

pub mod key_log;

mod type_conversion;

use rama_utils::macros::enums::rama_from_into_traits;
rama_from_into_traits!();

pub mod types {
    //! common tls types
    #[doc(inline)]
    pub use ::rama_net::tls::{
        ApplicationProtocol, CipherSuite, CompressionAlgorithm, ECPointFormat, ExtensionId,
        ProtocolVersion, SecureTransport, SignatureScheme, SupportedGroup, TlsTunnel, client,
    };
}

pub mod dep {
    //! Dependencies for rama rustls modules.
    //!
    //! Exported for your convenience.

    pub mod pki_types {
        //! Re-export of the [`pki-types`] crate.
        //!
        //! [`pki-types`]: https://docs.rs/rustls-pki-types

        #[doc(inline)]
        pub use rustls_pki_types::*;
    }

    pub mod pemfile {
        //! Re-export of the [`rustls-pemfile`] crate.
        //!
        //! A basic parser for .pem files containing cryptographic keys and certificates.
        //!
        //! [`rustls-pemfile`]: https://docs.rs/rustls-pemfile
        //!
        //! NOTE: Compatibility shim backed by `rustls-pki-types`.
        use rustls_pki_types::pem::{self, PemObject};
        use rustls_pki_types::{
            CertificateDer, CertificateRevocationListDer, CertificateSigningRequestDer,
            PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer,
            SubjectPublicKeyInfoDer,
        };
        use std::io;

        fn map_pem_err(err: pem::Error) -> io::Error {
            match err {
                pem::Error::Io(err) => err,
                err => io::Error::new(io::ErrorKind::InvalidData, err),
            }
        }

        fn map_result<T>(result: Result<T, pem::Error>) -> Result<T, io::Error> {
            result.map_err(map_pem_err)
        }

        /// Return an iterator over certificates from `rd`.
        pub fn certs(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<CertificateDer<'static>, io::Error>> + '_ {
            CertificateDer::pem_reader_iter(rd).map(map_result)
        }

        /// Return the first private key found in `rd`.
        pub fn private_key(
            rd: &mut dyn io::BufRead,
        ) -> Result<Option<PrivateKeyDer<'static>>, io::Error> {
            match PrivateKeyDer::from_pem_reader(rd) {
                Ok(key) => Ok(Some(key)),
                Err(pem::Error::NoItemsFound) => Ok(None),
                Err(err) => Err(map_pem_err(err)),
            }
        }

        /// Return the first certificate signing request (CSR) found in `rd`.
        pub fn csr(
            rd: &mut dyn io::BufRead,
        ) -> Result<Option<CertificateSigningRequestDer<'static>>, io::Error> {
            match CertificateSigningRequestDer::from_pem_reader(rd) {
                Ok(csr) => Ok(Some(csr)),
                Err(pem::Error::NoItemsFound) => Ok(None),
                Err(err) => Err(map_pem_err(err)),
            }
        }

        /// Return an iterator over certificate revocation lists (CRLs) from `rd`.
        pub fn crls(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<CertificateRevocationListDer<'static>, io::Error>> + '_ {
            CertificateRevocationListDer::pem_reader_iter(rd).map(map_result)
        }

        /// Return an iterator over RSA private keys from `rd`.
        pub fn rsa_private_keys(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<PrivatePkcs1KeyDer<'static>, io::Error>> + '_ {
            PrivatePkcs1KeyDer::pem_reader_iter(rd).map(map_result)
        }

        /// Return an iterator over PKCS8-encoded private keys from `rd`.
        pub fn pkcs8_private_keys(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<PrivatePkcs8KeyDer<'static>, io::Error>> + '_ {
            PrivatePkcs8KeyDer::pem_reader_iter(rd).map(map_result)
        }

        /// Return an iterator over SEC1-encoded EC private keys from `rd`.
        pub fn ec_private_keys(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<PrivateSec1KeyDer<'static>, io::Error>> + '_ {
            PrivateSec1KeyDer::pem_reader_iter(rd).map(map_result)
        }

        /// Return an iterator over SPKI-encoded public keys from `rd`.
        pub fn public_keys(
            rd: &mut dyn io::BufRead,
        ) -> impl Iterator<Item = Result<SubjectPublicKeyInfoDer<'static>, io::Error>> + '_ {
            SubjectPublicKeyInfoDer::pem_reader_iter(rd).map(map_result)
        }
    }

    pub mod native_certs {
        //! Re-export of the [`rustls-native-certs`] crate.
        //!
        //! rustls-native-certs allows rustls to use the platform's native certificate
        //! store when operating as a TLS client.
        //!
        //! [`rustls-native-certs`]: https://docs.rs/rustls-native-certs
        #[doc(inline)]
        pub use rustls_native_certs::*;
    }

    pub mod rcgen {
        //! Re-export of the [`rcgen`] crate.
        //!
        //! [`rcgen`]: https://docs.rs/rcgen

        #[doc(inline)]
        pub use rcgen::*;
    }

    pub mod rustls {
        //! Re-export of the [`rustls`] and  [`tokio-rustls`] crates.
        //!
        //! To facilitate the use of `rustls` types in API's such as [`TlsAcceptorLayer`].
        //!
        //! [`rustls`]: https://docs.rs/rustls
        //! [`tokio-rustls`]: https://docs.rs/tokio-rustls
        //! [`TlsAcceptorLayer`]: crate::rustls::server::TlsAcceptorLayer

        #[doc(inline)]
        pub use rustls::*;

        pub mod client {
            //! Re-export of client module of the [`rustls`] and [`tokio-rustls`] crates.
            //!
            //! [`rustls`]: https://docs.rs/rustls
            //! [`tokio-rustls`]: https://docs.rs/tokio-rustls

            #[doc(inline)]
            pub use rustls::client::*;
            #[doc(inline)]
            pub use tokio_rustls::client::TlsStream;
        }

        pub mod server {
            //! Re-export of server module of the [`rustls`] and [`tokio-rustls`] crates.
            //!
            //! [`rustls`]: https://docs.rs/rustls
            //! [`tokio-rustls`]: https://docs.rs/tokio-rustls

            #[doc(inline)]
            pub use rustls::server::*;
            #[doc(inline)]
            pub use tokio_rustls::server::TlsStream;
        }
    }

    pub mod tokio_rustls {
        //! Full Re-export of the [`tokio-rustls`] crate.
        //!
        //! [`tokio-rustls`]: https://docs.rs/tokio-rustls
        #[doc(inline)]
        pub use tokio_rustls::*;
    }

    pub mod webpki_roots {
        //! Re-export of the [`webpki-roots`] provides.
        //!
        //! This module provides a function to load the Mozilla root CA store.
        //!
        //! This module is inspired by <certifi.io> and uses the data provided by
        //! [the Common CA Database (CCADB)](https://www.ccadb.org/). The underlying data is used via
        //! [the CCADB Data Usage Terms](https://www.ccadb.org/rootstores/usage#ccadb-data-usage-terms).
        //!
        //! The data in this crate is a derived work of the CCADB data. See copy of LICENSE at
        //! <https://github.com/plabayo/rama/blob/main/docs/thirdparty/licenses/rustls-webpki-roots>.
        //!
        //! [`webpki-roots`]: https://docs.rs/webpki-roots
        #[doc(inline)]
        pub use webpki_roots::*;
    }
}
