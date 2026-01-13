//! Crypto primitives and dependencies used by rama.
//!
//! This includes but is not limited to:
//! - Certificates
//! - Javascript object signing and encryption (JOSE): JWS, JWK, JWE...
//! - Public and private keys
//! - Signing
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

pub mod jose;

pub mod dep {
    //! Dependencies for rama crypto modules.
    //!
    //! Exported for your convenience

    pub mod aws_lc_rs {
        //! Re-export of the [`aws-lc-rs`] crate.
        //!
        //! [`aws-lc-rs`]: https://docs.rs/aws-lc-rs

        #[doc(inline)]
        pub use aws_lc_rs::*;
    }

    pub mod rcgen {
        //! Re-export of the [`rcgen`] crate.
        //!
        //! [`rcgen`]: https://docs.rs/rcgen

        #[doc(inline)]
        pub use rcgen::*;
    }

    pub mod pki_types {
        //! Re-export of the [`rustls-pki-types`] crate.
        //!
        //! [`rustls_pki_types`]: https://docs.rs/rustls-pki-types

        #[doc(inline)]
        pub use rustls_pki_types::*;
    }

    pub mod x509_parser {
        //! Re-export of the [`x509_parser`] crate.
        //!
        //! [`x509_parser`]: https://docs.rs/x509_parser

        #[doc(inline)]
        pub use x509_parser::*;
    }
}
