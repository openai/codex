//! Bindings to OpenSSL
//!
//! This crate provides a safe interface to the popular OpenSSL cryptography library. OpenSSL versions 1.0.1 through
//! 3.x.x and LibreSSL versions 2.5 through 3.7.x are supported.
//!
//! # Building
//!
//! Both OpenSSL libraries and headers are required to build this crate. There are multiple options available to locate
//! OpenSSL.
//!
//! ## Vendored
//!
//! If the `vendored` Cargo feature is enabled, the `openssl-src` crate will be used to compile and statically link to
//! a copy of OpenSSL. The build process requires a C compiler, perl (and perl-core), and make. The OpenSSL version will generally track
//! the newest OpenSSL release, and changes to the version are *not* considered breaking changes.
//!
//! ```toml
//! [dependencies]
//! openssl = { version = "0.10", features = ["vendored"] }
//! ```
//!
//! The vendored copy will be configured to automatically find a configuration and root certificates at `/usr/local/ssl`.
//! This path can be overridden with an environment variable (see the manual section below).
//! Alternatively, the `openssl-probe` crate can be used to find root certificates at runtime.
//!
//! ## Automatic
//!
//! The `openssl-sys` crate will automatically detect OpenSSL installations via Homebrew on macOS and vcpkg on Windows.
//! Additionally, it will use `pkg-config` on Unix-like systems to find the system installation.
//!
//! ```not_rust
//! # macOS (Homebrew)
//! $ brew install openssl@3
//!
//! # macOS (MacPorts)
//! $ sudo port install openssl
//!
//! # macOS (pkgsrc)
//! $ sudo pkgin install openssl
//!
//! # Arch Linux
//! $ sudo pacman -S pkgconf openssl
//!
//! # Debian and Ubuntu
//! $ sudo apt-get install pkg-config libssl-dev
//!
//! # Fedora
//! $ sudo dnf install pkgconf perl-FindBin perl-IPC-Cmd openssl-devel
//!
//! # Alpine Linux
//! $ apk add pkgconf openssl-dev
//!
//! # openSUSE
//! $ sudo zypper in libopenssl-devel
//! ```
//!
//! ## Manual
//!
//! A set of environment variables can be used to point `openssl-sys` towards an OpenSSL installation. They will
//! override the automatic detection logic.
//!
//! * `OPENSSL_DIR` - If specified, the directory of an OpenSSL installation. The directory should contain `lib` and
//!   `include` subdirectories containing the libraries and headers respectively.
//! * `OPENSSL_LIB_DIR` and `OPENSSL_INCLUDE_DIR` - If specified, the directories containing the OpenSSL libraries and
//!   headers respectively. This can be used if the OpenSSL installation is split in a nonstandard directory layout.
//! * `OPENSSL_STATIC` - If set, the crate will statically link to OpenSSL rather than dynamically link.
//! * `OPENSSL_LIBS` - If set, a `:`-separated list of library names to link to (e.g. `ssl:crypto`). This can be used
//!   if nonstandard library names were used for whatever reason.
//! * `OPENSSL_NO_VENDOR` - If set, always find OpenSSL in the system, even if the `vendored` feature is enabled.
//!
//! If the `vendored` Cargo feature is enabled, the following environment variable can also be used to further configure
//! the OpenSSL build.
//!
//! * `OPENSSL_CONFIG_DIR` - If set, the copy of OpenSSL built by the `openssl-src` crate will be configured to look for
//!   configuration files and root certificates in this directory.
//!
//! Additionally, these variables can be prefixed with the upper-cased target architecture (e.g.
//!     `X86_64_UNKNOWN_LINUX_GNU_OPENSSL_DIR`), which can be useful when cross compiling.
//!
//! # Feature Detection
//!
//! APIs have been added to and removed from the various supported OpenSSL versions, and this library exposes the
//! functionality available in the version being linked against. This means that methods, constants, and even modules
//! will be present when building against one version of OpenSSL but not when building against another! APIs will
//! document any version-specific availability restrictions.
//!
//! A build script can be used to detect the OpenSSL or LibreSSL version at compile time if needed. The `openssl-sys`
//! crate propagates the version via the `DEP_OPENSSL_VERSION_NUMBER` and `DEP_OPENSSL_LIBRESSL_VERSION_NUMBER`
//! environment variables to build scripts. The version format is a hex-encoding of the OpenSSL release version:
//! `0xMNNFFPPS`. For example, version 1.0.2g's encoding is `0x1_00_02_07_0`.
//!
//! For example, let's say we want to adjust the TLSv1.3 cipher suites used by a client, but also want to compile
//! against OpenSSL versions that don't support TLSv1.3:
//!
//! Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! openssl-sys = "0.9"
//! openssl = "0.10"
//! ```
//!
//! build.rs:
//!
//! ```
//! use std::env;
//!
//! fn main() {
//!     if let Ok(v) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
//!         let version = u64::from_str_radix(&v, 16).unwrap();
//!
//!         if version >= 0x1_01_01_00_0 {
//!             println!("cargo:rustc-cfg=openssl111");
//!         }
//!     }
//! }
//! ```
//!
//! lib.rs:
//!
//! ```
//! use openssl::ssl::{SslConnector, SslMethod};
//!
//! let mut ctx = SslConnector::builder(SslMethod::tls()).unwrap();
//!
//! // set_ciphersuites was added in OpenSSL 1.1.1, so we can only call it when linking against that version
//! #[cfg(openssl111)]
//! ctx.set_ciphersuites("TLS_AES_256_GCM_SHA384:TLS_AES_128_GCM_SHA256").unwrap();
//! ```
#![doc(html_root_url = "https://docs.rs/openssl/0.10")]
#![warn(rust_2018_idioms)]
#![allow(clippy::uninlined_format_args, clippy::needless_doctest_main)]

#[doc(inline)]
pub use ffi::init;

use libc::c_int;
#[cfg(ossl300)]
use libc::c_long;

use crate::error::ErrorStack;

#[macro_use]
mod macros;

mod bio;
#[macro_use]
mod util;
pub mod aes;
pub mod asn1;
pub mod base64;
pub mod bn;
pub mod cipher;
pub mod cipher_ctx;
#[cfg(all(not(libressl), not(osslconf = "OPENSSL_NO_CMS")))]
pub mod cms;
pub mod conf;
pub mod derive;
pub mod dh;
pub mod dsa;
pub mod ec;
pub mod ecdsa;
pub mod encrypt;
#[cfg(not(any(boringssl, awslc)))]
pub mod envelope;
pub mod error;
pub mod ex_data;
#[cfg(not(any(libressl, ossl300)))]
pub mod fips;
pub mod hash;
pub mod kdf;
#[cfg(ossl300)]
pub mod lib_ctx;
pub mod md;
pub mod md_ctx;
pub mod memcmp;
pub mod nid;
#[cfg(not(osslconf = "OPENSSL_NO_OCSP"))]
pub mod ocsp;
pub mod pkcs12;
pub mod pkcs5;
#[cfg(not(any(boringssl, awslc)))]
pub mod pkcs7;
pub mod pkey;
pub mod pkey_ctx;
#[cfg(ossl300)]
pub mod provider;
pub mod rand;
pub mod rsa;
pub mod sha;
pub mod sign;
pub mod srtp;
pub mod ssl;
pub mod stack;
pub mod string;
pub mod symm;
pub mod version;
pub mod x509;

#[cfg(any(boringssl, awslc))]
type LenType = libc::size_t;
#[cfg(not(any(boringssl, awslc)))]
type LenType = libc::c_int;

#[cfg(any(boringssl, awslc))]
type SLenType = libc::ssize_t;
#[cfg(not(any(boringssl, awslc)))]
type SLenType = libc::c_int;

#[inline]
fn cvt_p<T>(r: *mut T) -> Result<*mut T, ErrorStack> {
    if r.is_null() {
        Err(ErrorStack::get())
    } else {
        Ok(r)
    }
}

#[inline]
fn cvt_p_const<T>(r: *const T) -> Result<*const T, ErrorStack> {
    if r.is_null() {
        Err(ErrorStack::get())
    } else {
        Ok(r)
    }
}

#[inline]
fn cvt(r: c_int) -> Result<c_int, ErrorStack> {
    if r <= 0 {
        Err(ErrorStack::get())
    } else {
        Ok(r)
    }
}

// cvt_long is currently only used in functions that require openssl >= 3.0.0,
// so this cfg statement is used to avoid "unused function" errors when
// compiling with openssl < 3.0.0
#[inline]
#[cfg(ossl300)]
fn cvt_long(r: c_long) -> Result<c_long, ErrorStack> {
    if r <= 0 {
        Err(ErrorStack::get())
    } else {
        Ok(r)
    }
}

#[inline]
fn cvt_n(r: c_int) -> Result<c_int, ErrorStack> {
    if r < 0 {
        Err(ErrorStack::get())
    } else {
        Ok(r)
    }
}
