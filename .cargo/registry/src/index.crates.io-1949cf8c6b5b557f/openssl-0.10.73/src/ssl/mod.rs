//! SSL/TLS support.
//!
//! `SslConnector` and `SslAcceptor` should be used in most cases - they handle
//! configuration of the OpenSSL primitives for you.
//!
//! # Examples
//!
//! To connect as a client to a remote server:
//!
//! ```no_run
//! use openssl::ssl::{SslMethod, SslConnector};
//! use std::io::{Read, Write};
//! use std::net::TcpStream;
//!
//! let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();
//!
//! let stream = TcpStream::connect("google.com:443").unwrap();
//! let mut stream = connector.connect("google.com", stream).unwrap();
//!
//! stream.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
//! let mut res = vec![];
//! stream.read_to_end(&mut res).unwrap();
//! println!("{}", String::from_utf8_lossy(&res));
//! ```
//!
//! To accept connections as a server from remote clients:
//!
//! ```no_run
//! use openssl::ssl::{SslMethod, SslAcceptor, SslStream, SslFiletype};
//! use std::net::{TcpListener, TcpStream};
//! use std::sync::Arc;
//! use std::thread;
//!
//!
//! let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
//! acceptor.set_private_key_file("key.pem", SslFiletype::PEM).unwrap();
//! acceptor.set_certificate_chain_file("certs.pem").unwrap();
//! acceptor.check_private_key().unwrap();
//! let acceptor = Arc::new(acceptor.build());
//!
//! let listener = TcpListener::bind("0.0.0.0:8443").unwrap();
//!
//! fn handle_client(stream: SslStream<TcpStream>) {
//!     // ...
//! }
//!
//! for stream in listener.incoming() {
//!     match stream {
//!         Ok(stream) => {
//!             let acceptor = acceptor.clone();
//!             thread::spawn(move || {
//!                 let stream = acceptor.accept(stream).unwrap();
//!                 handle_client(stream);
//!             });
//!         }
//!         Err(e) => { /* connection failed */ }
//!     }
//! }
//! ```
#[cfg(ossl300)]
use crate::cvt_long;
use crate::dh::{Dh, DhRef};
#[cfg(all(ossl101, not(ossl110)))]
use crate::ec::EcKey;
use crate::ec::EcKeyRef;
use crate::error::ErrorStack;
use crate::ex_data::Index;
#[cfg(ossl111)]
use crate::hash::MessageDigest;
#[cfg(any(ossl110, libressl270))]
use crate::nid::Nid;
use crate::pkey::{HasPrivate, PKeyRef, Params, Private};
#[cfg(ossl300)]
use crate::pkey::{PKey, Public};
use crate::srtp::{SrtpProtectionProfile, SrtpProtectionProfileRef};
use crate::ssl::bio::BioMethod;
use crate::ssl::callbacks::*;
use crate::ssl::error::InnerError;
use crate::stack::{Stack, StackRef, Stackable};
use crate::util;
use crate::util::{ForeignTypeExt, ForeignTypeRefExt};
use crate::x509::store::{X509Store, X509StoreBuilderRef, X509StoreRef};
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
use crate::x509::verify::X509VerifyParamRef;
use crate::x509::{X509Name, X509Ref, X509StoreContextRef, X509VerifyResult, X509};
use crate::{cvt, cvt_n, cvt_p, init};
use bitflags::bitflags;
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef, Opaque};
use libc::{c_char, c_int, c_long, c_uchar, c_uint, c_void};
use once_cell::sync::{Lazy, OnceCell};
use openssl_macros::corresponds;
use std::any::TypeId;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::panic::resume_unwind;
use std::path::Path;
use std::ptr;
use std::str;
use std::sync::{Arc, Mutex};

pub use crate::ssl::connector::{
    ConnectConfiguration, SslAcceptor, SslAcceptorBuilder, SslConnector, SslConnectorBuilder,
};
pub use crate::ssl::error::{Error, ErrorCode, HandshakeError};

mod bio;
mod callbacks;
mod connector;
mod error;
#[cfg(test)]
mod test;

/// Returns the OpenSSL name of a cipher corresponding to an RFC-standard cipher name.
///
/// If the cipher has no corresponding OpenSSL name, the string `(NONE)` is returned.
///
/// Requires OpenSSL 1.1.1 or newer.
#[corresponds(OPENSSL_cipher_name)]
#[cfg(ossl111)]
pub fn cipher_name(std_name: &str) -> &'static str {
    unsafe {
        ffi::init();

        let s = CString::new(std_name).unwrap();
        let ptr = ffi::OPENSSL_cipher_name(s.as_ptr());
        CStr::from_ptr(ptr).to_str().unwrap()
    }
}

cfg_if! {
    if #[cfg(ossl300)] {
        type SslOptionsRepr = u64;
    } else if #[cfg(any(boringssl, awslc))] {
        type SslOptionsRepr = u32;
    } else {
        type SslOptionsRepr = libc::c_ulong;
    }
}

bitflags! {
    /// Options controlling the behavior of an `SslContext`.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct SslOptions: SslOptionsRepr {
        /// Disables a countermeasure against an SSLv3/TLSv1.0 vulnerability affecting CBC ciphers.
        const DONT_INSERT_EMPTY_FRAGMENTS = ffi::SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS as SslOptionsRepr;

        /// A "reasonable default" set of options which enables compatibility flags.
        #[cfg(not(any(boringssl, awslc)))]
        const ALL = ffi::SSL_OP_ALL as SslOptionsRepr;

        /// Do not query the MTU.
        ///
        /// Only affects DTLS connections.
        const NO_QUERY_MTU = ffi::SSL_OP_NO_QUERY_MTU as SslOptionsRepr;

        /// Enables Cookie Exchange as described in [RFC 4347 Section 4.2.1].
        ///
        /// Only affects DTLS connections.
        ///
        /// [RFC 4347 Section 4.2.1]: https://tools.ietf.org/html/rfc4347#section-4.2.1
        #[cfg(not(any(boringssl, awslc)))]
        const COOKIE_EXCHANGE = ffi::SSL_OP_COOKIE_EXCHANGE as SslOptionsRepr;

        /// Disables the use of session tickets for session resumption.
        const NO_TICKET = ffi::SSL_OP_NO_TICKET as SslOptionsRepr;

        /// Always start a new session when performing a renegotiation on the server side.
        #[cfg(not(any(boringssl, awslc)))]
        const NO_SESSION_RESUMPTION_ON_RENEGOTIATION =
            ffi::SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION as SslOptionsRepr;

        /// Disables the use of TLS compression.
        #[cfg(not(any(boringssl, awslc)))]
        const NO_COMPRESSION = ffi::SSL_OP_NO_COMPRESSION as SslOptionsRepr;

        /// Allow legacy insecure renegotiation with servers or clients that do not support secure
        /// renegotiation.
        const ALLOW_UNSAFE_LEGACY_RENEGOTIATION =
            ffi::SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION as SslOptionsRepr;

        /// Creates a new key for each session when using ECDHE.
        ///
        /// This is always enabled in OpenSSL 1.1.0.
        const SINGLE_ECDH_USE = ffi::SSL_OP_SINGLE_ECDH_USE as SslOptionsRepr;

        /// Creates a new key for each session when using DHE.
        ///
        /// This is always enabled in OpenSSL 1.1.0.
        const SINGLE_DH_USE = ffi::SSL_OP_SINGLE_DH_USE as SslOptionsRepr;

        /// Use the server's preferences rather than the client's when selecting a cipher.
        ///
        /// This has no effect on the client side.
        const CIPHER_SERVER_PREFERENCE = ffi::SSL_OP_CIPHER_SERVER_PREFERENCE as SslOptionsRepr;

        /// Disables version rollback attach detection.
        const TLS_ROLLBACK_BUG = ffi::SSL_OP_TLS_ROLLBACK_BUG as SslOptionsRepr;

        /// Disables the use of SSLv2.
        const NO_SSLV2 = ffi::SSL_OP_NO_SSLv2 as SslOptionsRepr;

        /// Disables the use of SSLv3.
        const NO_SSLV3 = ffi::SSL_OP_NO_SSLv3 as SslOptionsRepr;

        /// Disables the use of TLSv1.0.
        const NO_TLSV1 = ffi::SSL_OP_NO_TLSv1 as SslOptionsRepr;

        /// Disables the use of TLSv1.1.
        const NO_TLSV1_1 = ffi::SSL_OP_NO_TLSv1_1 as SslOptionsRepr;

        /// Disables the use of TLSv1.2.
        const NO_TLSV1_2 = ffi::SSL_OP_NO_TLSv1_2 as SslOptionsRepr;

        /// Disables the use of TLSv1.3.
        ///
        /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
        #[cfg(any(boringssl, ossl111, libressl340, awslc))]
        const NO_TLSV1_3 = ffi::SSL_OP_NO_TLSv1_3 as SslOptionsRepr;

        /// Disables the use of DTLSv1.0
        ///
        /// Requires OpenSSL 1.0.2 or LibreSSL 3.3.2 or newer.
        #[cfg(any(boringssl, ossl102, ossl110, libressl332, awslc))]
        const NO_DTLSV1 = ffi::SSL_OP_NO_DTLSv1 as SslOptionsRepr;

        /// Disables the use of DTLSv1.2.
        ///
        /// Requires OpenSSL 1.0.2 or LibreSSL 3.3.2 or newer.
        #[cfg(any(boringssl, ossl102, ossl110, libressl332, awslc))]
        const NO_DTLSV1_2 = ffi::SSL_OP_NO_DTLSv1_2 as SslOptionsRepr;

        /// Disables the use of all (D)TLS protocol versions.
        ///
        /// This can be used as a mask when whitelisting protocol versions.
        ///
        /// Requires OpenSSL 1.0.2 or newer.
        ///
        /// # Examples
        ///
        /// Only support TLSv1.2:
        ///
        /// ```rust
        /// use openssl::ssl::SslOptions;
        ///
        /// let options = SslOptions::NO_SSL_MASK & !SslOptions::NO_TLSV1_2;
        /// ```
        #[cfg(any(ossl102, ossl110))]
        const NO_SSL_MASK = ffi::SSL_OP_NO_SSL_MASK as SslOptionsRepr;

        /// Disallow all renegotiation in TLSv1.2 and earlier.
        ///
        /// Requires OpenSSL 1.1.0h or newer.
        #[cfg(any(boringssl, ossl110h, awslc))]
        const NO_RENEGOTIATION = ffi::SSL_OP_NO_RENEGOTIATION as SslOptionsRepr;

        /// Enable TLSv1.3 Compatibility mode.
        ///
        /// Requires OpenSSL 1.1.1 or newer. This is on by default in 1.1.1, but a future version
        /// may have this disabled by default.
        #[cfg(ossl111)]
        const ENABLE_MIDDLEBOX_COMPAT = ffi::SSL_OP_ENABLE_MIDDLEBOX_COMPAT as SslOptionsRepr;

        /// Prioritize ChaCha ciphers when preferred by clients.
        ///
        /// Temporarily reprioritize ChaCha20-Poly1305 ciphers to the top of the server cipher list
        /// if a ChaCha20-Poly1305 cipher is at the top of the client cipher list. This helps those
        /// clients (e.g. mobile) use ChaCha20-Poly1305 if that cipher is anywhere in the server
        /// cipher list; but still allows other clients to use AES and other ciphers.
        ///
        /// Requires enable [`SslOptions::CIPHER_SERVER_PREFERENCE`].
        /// Requires OpenSSL 1.1.1 or newer.
        ///
        /// [`SslOptions::CIPHER_SERVER_PREFERENCE`]: struct.SslOptions.html#associatedconstant.CIPHER_SERVER_PREFERENCE
        #[cfg(ossl111)]
        const PRIORITIZE_CHACHA = ffi::SSL_OP_PRIORITIZE_CHACHA as SslOptionsRepr;
    }
}

bitflags! {
    /// Options controlling the behavior of an `SslContext`.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct SslMode: SslBitType {
        /// Enables "short writes".
        ///
        /// Normally, a write in OpenSSL will always write out all of the requested data, even if it
        /// requires more than one TLS record or write to the underlying stream. This option will
        /// cause a write to return after writing a single TLS record instead.
        const ENABLE_PARTIAL_WRITE = ffi::SSL_MODE_ENABLE_PARTIAL_WRITE;

        /// Disables a check that the data buffer has not moved between calls when operating in a
        /// non-blocking context.
        const ACCEPT_MOVING_WRITE_BUFFER = ffi::SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER;

        /// Enables automatic retries after TLS session events such as renegotiations or heartbeats.
        ///
        /// By default, OpenSSL will return a `WantRead` error after a renegotiation or heartbeat.
        /// This option will cause OpenSSL to automatically continue processing the requested
        /// operation instead.
        ///
        /// Note that `SslStream::read` and `SslStream::write` will automatically retry regardless
        /// of the state of this option. It only affects `SslStream::ssl_read` and
        /// `SslStream::ssl_write`.
        const AUTO_RETRY = ffi::SSL_MODE_AUTO_RETRY;

        /// Disables automatic chain building when verifying a peer's certificate.
        ///
        /// TLS peers are responsible for sending the entire certificate chain from the leaf to a
        /// trusted root, but some will incorrectly not do so. OpenSSL will try to build the chain
        /// out of certificates it knows of, and this option will disable that behavior.
        const NO_AUTO_CHAIN = ffi::SSL_MODE_NO_AUTO_CHAIN;

        /// Release memory buffers when the session does not need them.
        ///
        /// This saves ~34 KiB of memory for idle streams.
        const RELEASE_BUFFERS = ffi::SSL_MODE_RELEASE_BUFFERS;

        /// Sends the fake `TLS_FALLBACK_SCSV` cipher suite in the ClientHello message of a
        /// handshake.
        ///
        /// This should only be enabled if a client has failed to connect to a server which
        /// attempted to downgrade the protocol version of the session.
        ///
        /// Do not use this unless you know what you're doing!
        #[cfg(not(libressl))]
        const SEND_FALLBACK_SCSV = ffi::SSL_MODE_SEND_FALLBACK_SCSV;
    }
}

/// A type specifying the kind of protocol an `SslContext` will speak.
#[derive(Copy, Clone)]
pub struct SslMethod(*const ffi::SSL_METHOD);

impl SslMethod {
    /// Support all versions of the TLS protocol.
    #[corresponds(TLS_method)]
    pub fn tls() -> SslMethod {
        unsafe { SslMethod(TLS_method()) }
    }

    /// Support all versions of the DTLS protocol.
    #[corresponds(DTLS_method)]
    pub fn dtls() -> SslMethod {
        unsafe { SslMethod(DTLS_method()) }
    }

    /// Support all versions of the TLS protocol, explicitly as a client.
    #[corresponds(TLS_client_method)]
    pub fn tls_client() -> SslMethod {
        unsafe { SslMethod(TLS_client_method()) }
    }

    /// Support all versions of the TLS protocol, explicitly as a server.
    #[corresponds(TLS_server_method)]
    pub fn tls_server() -> SslMethod {
        unsafe { SslMethod(TLS_server_method()) }
    }

    /// Support all versions of the DTLS protocol, explicitly as a client.
    #[corresponds(DTLS_client_method)]
    #[cfg(any(boringssl, ossl110, libressl291, awslc))]
    pub fn dtls_client() -> SslMethod {
        unsafe { SslMethod(DTLS_client_method()) }
    }

    /// Support all versions of the DTLS protocol, explicitly as a server.
    #[corresponds(DTLS_server_method)]
    #[cfg(any(boringssl, ossl110, libressl291, awslc))]
    pub fn dtls_server() -> SslMethod {
        unsafe { SslMethod(DTLS_server_method()) }
    }

    /// Constructs an `SslMethod` from a pointer to the underlying OpenSSL value.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is valid.
    pub unsafe fn from_ptr(ptr: *const ffi::SSL_METHOD) -> SslMethod {
        SslMethod(ptr)
    }

    /// Returns a pointer to the underlying OpenSSL value.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_ptr(&self) -> *const ffi::SSL_METHOD {
        self.0
    }
}

unsafe impl Sync for SslMethod {}
unsafe impl Send for SslMethod {}

bitflags! {
    /// Options controlling the behavior of certificate verification.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct SslVerifyMode: i32 {
        /// Verifies that the peer's certificate is trusted.
        ///
        /// On the server side, this will cause OpenSSL to request a certificate from the client.
        const PEER = ffi::SSL_VERIFY_PEER;

        /// Disables verification of the peer's certificate.
        ///
        /// On the server side, this will cause OpenSSL to not request a certificate from the
        /// client. On the client side, the certificate will be checked for validity, but the
        /// negotiation will continue regardless of the result of that check.
        const NONE = ffi::SSL_VERIFY_NONE;

        /// On the server side, abort the handshake if the client did not send a certificate.
        ///
        /// This should be paired with `SSL_VERIFY_PEER`. It has no effect on the client side.
        const FAIL_IF_NO_PEER_CERT = ffi::SSL_VERIFY_FAIL_IF_NO_PEER_CERT;
    }
}

#[cfg(any(boringssl, awslc))]
type SslBitType = c_int;
#[cfg(not(any(boringssl, awslc)))]
type SslBitType = c_long;

#[cfg(any(boringssl, awslc))]
type SslTimeTy = u64;
#[cfg(not(any(boringssl, awslc)))]
type SslTimeTy = c_long;

bitflags! {
    /// Options controlling the behavior of session caching.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct SslSessionCacheMode: SslBitType {
        /// No session caching for the client or server takes place.
        const OFF = ffi::SSL_SESS_CACHE_OFF;

        /// Enable session caching on the client side.
        ///
        /// OpenSSL has no way of identifying the proper session to reuse automatically, so the
        /// application is responsible for setting it explicitly via [`SslRef::set_session`].
        ///
        /// [`SslRef::set_session`]: struct.SslRef.html#method.set_session
        const CLIENT = ffi::SSL_SESS_CACHE_CLIENT;

        /// Enable session caching on the server side.
        ///
        /// This is the default mode.
        const SERVER = ffi::SSL_SESS_CACHE_SERVER;

        /// Enable session caching on both the client and server side.
        const BOTH = ffi::SSL_SESS_CACHE_BOTH;

        /// Disable automatic removal of expired sessions from the session cache.
        const NO_AUTO_CLEAR = ffi::SSL_SESS_CACHE_NO_AUTO_CLEAR;

        /// Disable use of the internal session cache for session lookups.
        const NO_INTERNAL_LOOKUP = ffi::SSL_SESS_CACHE_NO_INTERNAL_LOOKUP;

        /// Disable use of the internal session cache for session storage.
        const NO_INTERNAL_STORE = ffi::SSL_SESS_CACHE_NO_INTERNAL_STORE;

        /// Disable use of the internal session cache for storage and lookup.
        const NO_INTERNAL = ffi::SSL_SESS_CACHE_NO_INTERNAL;
    }
}

#[cfg(ossl111)]
bitflags! {
    /// Which messages and under which conditions an extension should be added or expected.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct ExtensionContext: c_uint {
        /// This extension is only allowed in TLS
        const TLS_ONLY = ffi::SSL_EXT_TLS_ONLY;
        /// This extension is only allowed in DTLS
        const DTLS_ONLY = ffi::SSL_EXT_DTLS_ONLY;
        /// Some extensions may be allowed in DTLS but we don't implement them for it
        const TLS_IMPLEMENTATION_ONLY = ffi::SSL_EXT_TLS_IMPLEMENTATION_ONLY;
        /// Most extensions are not defined for SSLv3 but EXT_TYPE_renegotiate is
        const SSL3_ALLOWED = ffi::SSL_EXT_SSL3_ALLOWED;
        /// Extension is only defined for TLS1.2 and below
        const TLS1_2_AND_BELOW_ONLY = ffi::SSL_EXT_TLS1_2_AND_BELOW_ONLY;
        /// Extension is only defined for TLS1.3 and above
        const TLS1_3_ONLY = ffi::SSL_EXT_TLS1_3_ONLY;
        /// Ignore this extension during parsing if we are resuming
        const IGNORE_ON_RESUMPTION = ffi::SSL_EXT_IGNORE_ON_RESUMPTION;
        const CLIENT_HELLO = ffi::SSL_EXT_CLIENT_HELLO;
        /// Really means TLS1.2 or below
        const TLS1_2_SERVER_HELLO = ffi::SSL_EXT_TLS1_2_SERVER_HELLO;
        const TLS1_3_SERVER_HELLO = ffi::SSL_EXT_TLS1_3_SERVER_HELLO;
        const TLS1_3_ENCRYPTED_EXTENSIONS = ffi::SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS;
        const TLS1_3_HELLO_RETRY_REQUEST = ffi::SSL_EXT_TLS1_3_HELLO_RETRY_REQUEST;
        const TLS1_3_CERTIFICATE = ffi::SSL_EXT_TLS1_3_CERTIFICATE;
        const TLS1_3_NEW_SESSION_TICKET = ffi::SSL_EXT_TLS1_3_NEW_SESSION_TICKET;
        const TLS1_3_CERTIFICATE_REQUEST = ffi::SSL_EXT_TLS1_3_CERTIFICATE_REQUEST;
    }
}

/// An identifier of the format of a certificate or key file.
#[derive(Copy, Clone)]
pub struct SslFiletype(c_int);

impl SslFiletype {
    /// The PEM format.
    ///
    /// This corresponds to `SSL_FILETYPE_PEM`.
    pub const PEM: SslFiletype = SslFiletype(ffi::SSL_FILETYPE_PEM);

    /// The ASN1 format.
    ///
    /// This corresponds to `SSL_FILETYPE_ASN1`.
    pub const ASN1: SslFiletype = SslFiletype(ffi::SSL_FILETYPE_ASN1);

    /// Constructs an `SslFiletype` from a raw OpenSSL value.
    pub fn from_raw(raw: c_int) -> SslFiletype {
        SslFiletype(raw)
    }

    /// Returns the raw OpenSSL value represented by this type.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

/// An identifier of a certificate status type.
#[derive(Copy, Clone)]
pub struct StatusType(c_int);

impl StatusType {
    /// An OSCP status.
    pub const OCSP: StatusType = StatusType(ffi::TLSEXT_STATUSTYPE_ocsp);

    /// Constructs a `StatusType` from a raw OpenSSL value.
    pub fn from_raw(raw: c_int) -> StatusType {
        StatusType(raw)
    }

    /// Returns the raw OpenSSL value represented by this type.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

/// An identifier of a session name type.
#[derive(Copy, Clone)]
pub struct NameType(c_int);

impl NameType {
    /// A host name.
    pub const HOST_NAME: NameType = NameType(ffi::TLSEXT_NAMETYPE_host_name);

    /// Constructs a `StatusType` from a raw OpenSSL value.
    pub fn from_raw(raw: c_int) -> StatusType {
        StatusType(raw)
    }

    /// Returns the raw OpenSSL value represented by this type.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

static INDEXES: Lazy<Mutex<HashMap<TypeId, c_int>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static SSL_INDEXES: Lazy<Mutex<HashMap<TypeId, c_int>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static SESSION_CTX_INDEX: OnceCell<Index<Ssl, SslContext>> = OnceCell::new();

fn try_get_session_ctx_index() -> Result<&'static Index<Ssl, SslContext>, ErrorStack> {
    SESSION_CTX_INDEX.get_or_try_init(Ssl::new_ex_index)
}

unsafe extern "C" fn free_data_box<T>(
    _parent: *mut c_void,
    ptr: *mut c_void,
    _ad: *mut ffi::CRYPTO_EX_DATA,
    _idx: c_int,
    _argl: c_long,
    _argp: *mut c_void,
) {
    if !ptr.is_null() {
        let _ = Box::<T>::from_raw(ptr as *mut T);
    }
}

/// An error returned from the SNI callback.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SniError(c_int);

impl SniError {
    /// Abort the handshake with a fatal alert.
    pub const ALERT_FATAL: SniError = SniError(ffi::SSL_TLSEXT_ERR_ALERT_FATAL);

    /// Send a warning alert to the client and continue the handshake.
    pub const ALERT_WARNING: SniError = SniError(ffi::SSL_TLSEXT_ERR_ALERT_WARNING);

    pub const NOACK: SniError = SniError(ffi::SSL_TLSEXT_ERR_NOACK);
}

/// An SSL/TLS alert.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SslAlert(c_int);

impl SslAlert {
    /// Alert 112 - `unrecognized_name`.
    pub const UNRECOGNIZED_NAME: SslAlert = SslAlert(ffi::SSL_AD_UNRECOGNIZED_NAME);
    pub const ILLEGAL_PARAMETER: SslAlert = SslAlert(ffi::SSL_AD_ILLEGAL_PARAMETER);
    pub const DECODE_ERROR: SslAlert = SslAlert(ffi::SSL_AD_DECODE_ERROR);
}

/// An error returned from an ALPN selection callback.
///
/// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or LibreSSL 2.6.1 or newer.
#[cfg(any(ossl102, libressl261, boringssl, awslc))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AlpnError(c_int);

#[cfg(any(ossl102, libressl261, boringssl, awslc))]
impl AlpnError {
    /// Terminate the handshake with a fatal alert.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.0 or newer.
    #[cfg(any(ossl110, boringssl, awslc))]
    pub const ALERT_FATAL: AlpnError = AlpnError(ffi::SSL_TLSEXT_ERR_ALERT_FATAL);

    /// Do not select a protocol, but continue the handshake.
    pub const NOACK: AlpnError = AlpnError(ffi::SSL_TLSEXT_ERR_NOACK);
}

/// The result of a client hello callback.
///
/// Requires OpenSSL 1.1.1 or newer.
#[cfg(ossl111)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ClientHelloResponse(c_int);

#[cfg(ossl111)]
impl ClientHelloResponse {
    /// Continue the handshake.
    pub const SUCCESS: ClientHelloResponse = ClientHelloResponse(ffi::SSL_CLIENT_HELLO_SUCCESS);

    /// Return from the handshake with an `ErrorCode::WANT_CLIENT_HELLO_CB` error.
    pub const RETRY: ClientHelloResponse = ClientHelloResponse(ffi::SSL_CLIENT_HELLO_RETRY);
}

/// An SSL/TLS protocol version.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SslVersion(c_int);

impl SslVersion {
    /// SSLv3
    pub const SSL3: SslVersion = SslVersion(ffi::SSL3_VERSION);

    /// TLSv1.0
    pub const TLS1: SslVersion = SslVersion(ffi::TLS1_VERSION);

    /// TLSv1.1
    pub const TLS1_1: SslVersion = SslVersion(ffi::TLS1_1_VERSION);

    /// TLSv1.2
    pub const TLS1_2: SslVersion = SslVersion(ffi::TLS1_2_VERSION);

    /// TLSv1.3
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[cfg(any(ossl111, libressl340, boringssl, awslc))]
    pub const TLS1_3: SslVersion = SslVersion(ffi::TLS1_3_VERSION);

    /// DTLSv1.0
    ///
    /// DTLS 1.0 corresponds to TLS 1.1.
    pub const DTLS1: SslVersion = SslVersion(ffi::DTLS1_VERSION);

    /// DTLSv1.2
    ///
    /// DTLS 1.2 corresponds to TLS 1.2 to harmonize versions. There was never a DTLS 1.1.
    #[cfg(any(ossl102, libressl332, boringssl, awslc))]
    pub const DTLS1_2: SslVersion = SslVersion(ffi::DTLS1_2_VERSION);
}

cfg_if! {
    if #[cfg(any(boringssl, awslc))] {
        type SslCacheTy = i64;
        type SslCacheSize = libc::c_ulong;
        type MtuTy = u32;
        type SizeTy = usize;
    } else {
        type SslCacheTy = i64;
        type SslCacheSize = c_long;
        type MtuTy = c_long;
        type SizeTy = u32;
    }
}

/// A standard implementation of protocol selection for Application Layer Protocol Negotiation
/// (ALPN).
///
/// `server` should contain the server's list of supported protocols and `client` the client's. They
/// must both be in the ALPN wire format. See the documentation for
/// [`SslContextBuilder::set_alpn_protos`] for details.
///
/// It will select the first protocol supported by the server which is also supported by the client.
///
/// [`SslContextBuilder::set_alpn_protos`]: struct.SslContextBuilder.html#method.set_alpn_protos
#[corresponds(SSL_select_next_proto)]
pub fn select_next_proto<'a>(server: &'a [u8], client: &'a [u8]) -> Option<&'a [u8]> {
    unsafe {
        let mut out = ptr::null_mut();
        let mut outlen = 0;
        let r = ffi::SSL_select_next_proto(
            &mut out,
            &mut outlen,
            server.as_ptr(),
            server.len() as c_uint,
            client.as_ptr(),
            client.len() as c_uint,
        );
        if r == ffi::OPENSSL_NPN_NEGOTIATED {
            Some(util::from_raw_parts(out as *const u8, outlen as usize))
        } else {
            None
        }
    }
}

/// A builder for `SslContext`s.
pub struct SslContextBuilder(SslContext);

impl SslContextBuilder {
    /// Creates a new `SslContextBuilder`.
    #[corresponds(SSL_CTX_new)]
    pub fn new(method: SslMethod) -> Result<SslContextBuilder, ErrorStack> {
        unsafe {
            init();
            let ctx = cvt_p(ffi::SSL_CTX_new(method.as_ptr()))?;

            Ok(SslContextBuilder::from_ptr(ctx))
        }
    }

    /// Creates an `SslContextBuilder` from a pointer to a raw OpenSSL value.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and uniquely owned by the builder.
    pub unsafe fn from_ptr(ctx: *mut ffi::SSL_CTX) -> SslContextBuilder {
        SslContextBuilder(SslContext::from_ptr(ctx))
    }

    /// Returns a pointer to the raw OpenSSL value.
    pub fn as_ptr(&self) -> *mut ffi::SSL_CTX {
        self.0.as_ptr()
    }

    /// Configures the certificate verification method for new connections.
    #[corresponds(SSL_CTX_set_verify)]
    pub fn set_verify(&mut self, mode: SslVerifyMode) {
        unsafe {
            ffi::SSL_CTX_set_verify(self.as_ptr(), mode.bits() as c_int, None);
        }
    }

    /// Configures the certificate verification method for new connections and
    /// registers a verification callback.
    ///
    /// The callback is passed a boolean indicating if OpenSSL's internal verification succeeded as
    /// well as a reference to the `X509StoreContext` which can be used to examine the certificate
    /// chain. It should return a boolean indicating if verification succeeded.
    #[corresponds(SSL_CTX_set_verify)]
    pub fn set_verify_callback<F>(&mut self, mode: SslVerifyMode, verify: F)
    where
        F: Fn(bool, &mut X509StoreContextRef) -> bool + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), verify);
            ffi::SSL_CTX_set_verify(self.as_ptr(), mode.bits() as c_int, Some(raw_verify::<F>));
        }
    }

    /// Configures the server name indication (SNI) callback for new connections.
    ///
    /// SNI is used to allow a single server to handle requests for multiple domains, each of which
    /// has its own certificate chain and configuration.
    ///
    /// Obtain the server name with the `servername` method and then set the corresponding context
    /// with `set_ssl_context`
    #[corresponds(SSL_CTX_set_tlsext_servername_callback)]
    // FIXME tlsext prefix?
    pub fn set_servername_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &mut SslAlert) -> Result<(), SniError> + 'static + Sync + Send,
    {
        unsafe {
            // The SNI callback is somewhat unique in that the callback associated with the original
            // context associated with an SSL can be used even if the SSL's context has been swapped
            // out. When that happens, we wouldn't be able to look up the callback's state in the
            // context's ex data. Instead, pass the pointer directly as the servername arg. It's
            // still stored in ex data to manage the lifetime.
            let arg = self.set_ex_data_inner(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_tlsext_servername_arg(self.as_ptr(), arg);
            #[cfg(any(boringssl, awslc))]
            ffi::SSL_CTX_set_tlsext_servername_callback(self.as_ptr(), Some(raw_sni::<F>));
            #[cfg(not(any(boringssl, awslc)))]
            ffi::SSL_CTX_set_tlsext_servername_callback__fixed_rust(
                self.as_ptr(),
                Some(raw_sni::<F>),
            );
        }
    }

    /// Sets the certificate verification depth.
    ///
    /// If the peer's certificate chain is longer than this value, verification will fail.
    #[corresponds(SSL_CTX_set_verify_depth)]
    pub fn set_verify_depth(&mut self, depth: u32) {
        unsafe {
            ffi::SSL_CTX_set_verify_depth(self.as_ptr(), depth as c_int);
        }
    }

    /// Sets a custom certificate store for verifying peer certificates.
    ///
    /// Requires OpenSSL 1.0.2 or newer.
    #[corresponds(SSL_CTX_set0_verify_cert_store)]
    #[cfg(ossl102)]
    pub fn set_verify_cert_store(&mut self, cert_store: X509Store) -> Result<(), ErrorStack> {
        unsafe {
            let ptr = cert_store.as_ptr();
            cvt(ffi::SSL_CTX_set0_verify_cert_store(self.as_ptr(), ptr) as c_int)?;
            mem::forget(cert_store);

            Ok(())
        }
    }

    /// Replaces the context's certificate store.
    #[corresponds(SSL_CTX_set_cert_store)]
    pub fn set_cert_store(&mut self, cert_store: X509Store) {
        unsafe {
            ffi::SSL_CTX_set_cert_store(self.as_ptr(), cert_store.as_ptr());
            mem::forget(cert_store);
        }
    }

    /// Controls read ahead behavior.
    ///
    /// If enabled, OpenSSL will read as much data as is available from the underlying stream,
    /// instead of a single record at a time.
    ///
    /// It has no effect when used with DTLS.
    #[corresponds(SSL_CTX_set_read_ahead)]
    pub fn set_read_ahead(&mut self, read_ahead: bool) {
        unsafe {
            ffi::SSL_CTX_set_read_ahead(self.as_ptr(), read_ahead as SslBitType);
        }
    }

    /// Sets the mode used by the context, returning the previous mode.
    #[corresponds(SSL_CTX_set_mode)]
    pub fn set_mode(&mut self, mode: SslMode) -> SslMode {
        unsafe {
            let bits = ffi::SSL_CTX_set_mode(self.as_ptr(), mode.bits() as MtuTy) as SslBitType;
            SslMode::from_bits_retain(bits)
        }
    }

    /// Sets the parameters to be used during ephemeral Diffie-Hellman key exchange.
    #[corresponds(SSL_CTX_set_tmp_dh)]
    pub fn set_tmp_dh(&mut self, dh: &DhRef<Params>) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_set_tmp_dh(self.as_ptr(), dh.as_ptr()) as c_int).map(|_| ()) }
    }

    /// Sets the callback which will generate parameters to be used during ephemeral Diffie-Hellman
    /// key exchange.
    ///
    /// The callback is provided with a reference to the `Ssl` for the session, as well as a boolean
    /// indicating if the selected cipher is export-grade, and the key length. The export and key
    /// length options are archaic and should be ignored in almost all cases.
    #[corresponds(SSL_CTX_set_tmp_dh_callback)]
    pub fn set_tmp_dh_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, bool, u32) -> Result<Dh<Params>, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);

            #[cfg(not(any(boringssl, awslc)))]
            ffi::SSL_CTX_set_tmp_dh_callback__fixed_rust(self.as_ptr(), Some(raw_tmp_dh::<F>));
            #[cfg(any(boringssl, awslc))]
            ffi::SSL_CTX_set_tmp_dh_callback(self.as_ptr(), Some(raw_tmp_dh::<F>));
        }
    }

    /// Sets the parameters to be used during ephemeral elliptic curve Diffie-Hellman key exchange.
    #[corresponds(SSL_CTX_set_tmp_ecdh)]
    pub fn set_tmp_ecdh(&mut self, key: &EcKeyRef<Params>) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_set_tmp_ecdh(self.as_ptr(), key.as_ptr()) as c_int).map(|_| ()) }
    }

    /// Sets the callback which will generate parameters to be used during ephemeral elliptic curve
    /// Diffie-Hellman key exchange.
    ///
    /// The callback is provided with a reference to the `Ssl` for the session, as well as a boolean
    /// indicating if the selected cipher is export-grade, and the key length. The export and key
    /// length options are archaic and should be ignored in almost all cases.
    ///
    /// Requires OpenSSL 1.0.1 or 1.0.2.
    #[corresponds(SSL_CTX_set_tmp_ecdh_callback)]
    #[cfg(all(ossl101, not(ossl110)))]
    #[deprecated(note = "this function leaks memory and does not exist on newer OpenSSL versions")]
    pub fn set_tmp_ecdh_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, bool, u32) -> Result<EcKey<Params>, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_tmp_ecdh_callback__fixed_rust(self.as_ptr(), Some(raw_tmp_ecdh::<F>));
        }
    }

    /// Use the default locations of trusted certificates for verification.
    ///
    /// These locations are read from the `SSL_CERT_FILE` and `SSL_CERT_DIR` environment variables
    /// if present, or defaults specified at OpenSSL build time otherwise.
    #[corresponds(SSL_CTX_set_default_verify_paths)]
    pub fn set_default_verify_paths(&mut self) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_set_default_verify_paths(self.as_ptr())).map(|_| ()) }
    }

    /// Loads trusted root certificates from a file.
    ///
    /// The file should contain a sequence of PEM-formatted CA certificates.
    #[corresponds(SSL_CTX_load_verify_locations)]
    pub fn set_ca_file<P: AsRef<Path>>(&mut self, file: P) -> Result<(), ErrorStack> {
        self.load_verify_locations(Some(file.as_ref()), None)
    }

    /// Loads trusted root certificates from a file and/or a directory.
    #[corresponds(SSL_CTX_load_verify_locations)]
    pub fn load_verify_locations(
        &mut self,
        ca_file: Option<&Path>,
        ca_path: Option<&Path>,
    ) -> Result<(), ErrorStack> {
        let ca_file = ca_file.map(|p| CString::new(p.as_os_str().to_str().unwrap()).unwrap());
        let ca_path = ca_path.map(|p| CString::new(p.as_os_str().to_str().unwrap()).unwrap());
        unsafe {
            cvt(ffi::SSL_CTX_load_verify_locations(
                self.as_ptr(),
                ca_file.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                ca_path.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            ))
            .map(|_| ())
        }
    }

    /// Sets the list of CA names sent to the client.
    ///
    /// The CA certificates must still be added to the trust root - they are not automatically set
    /// as trusted by this method.
    #[corresponds(SSL_CTX_set_client_CA_list)]
    pub fn set_client_ca_list(&mut self, list: Stack<X509Name>) {
        unsafe {
            ffi::SSL_CTX_set_client_CA_list(self.as_ptr(), list.as_ptr());
            mem::forget(list);
        }
    }

    /// Add the provided CA certificate to the list sent by the server to the client when
    /// requesting client-side TLS authentication.
    #[corresponds(SSL_CTX_add_client_CA)]
    pub fn add_client_ca(&mut self, cacert: &X509Ref) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_add_client_CA(self.as_ptr(), cacert.as_ptr())).map(|_| ()) }
    }

    /// Set the context identifier for sessions.
    ///
    /// This value identifies the server's session cache to clients, telling them when they're
    /// able to reuse sessions. It should be set to a unique value per server, unless multiple
    /// servers share a session cache.
    ///
    /// This value should be set when using client certificates, or each request will fail its
    /// handshake and need to be restarted.
    #[corresponds(SSL_CTX_set_session_id_context)]
    pub fn set_session_id_context(&mut self, sid_ctx: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            assert!(sid_ctx.len() <= c_uint::MAX as usize);
            cvt(ffi::SSL_CTX_set_session_id_context(
                self.as_ptr(),
                sid_ctx.as_ptr(),
                sid_ctx.len() as SizeTy,
            ))
            .map(|_| ())
        }
    }

    /// Loads a leaf certificate from a file.
    ///
    /// Only a single certificate will be loaded - use `add_extra_chain_cert` to add the remainder
    /// of the certificate chain, or `set_certificate_chain_file` to load the entire chain from a
    /// single file.
    #[corresponds(SSL_CTX_use_certificate_file)]
    pub fn set_certificate_file<P: AsRef<Path>>(
        &mut self,
        file: P,
        file_type: SslFiletype,
    ) -> Result<(), ErrorStack> {
        let file = CString::new(file.as_ref().as_os_str().to_str().unwrap()).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_use_certificate_file(
                self.as_ptr(),
                file.as_ptr() as *const _,
                file_type.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Loads a certificate chain from a file.
    ///
    /// The file should contain a sequence of PEM-formatted certificates, the first being the leaf
    /// certificate, and the remainder forming the chain of certificates up to and including the
    /// trusted root certificate.
    #[corresponds(SSL_CTX_use_certificate_chain_file)]
    pub fn set_certificate_chain_file<P: AsRef<Path>>(
        &mut self,
        file: P,
    ) -> Result<(), ErrorStack> {
        let file = CString::new(file.as_ref().as_os_str().to_str().unwrap()).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_use_certificate_chain_file(
                self.as_ptr(),
                file.as_ptr() as *const _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the leaf certificate.
    ///
    /// Use `add_extra_chain_cert` to add the remainder of the certificate chain.
    #[corresponds(SSL_CTX_use_certificate)]
    pub fn set_certificate(&mut self, cert: &X509Ref) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_use_certificate(self.as_ptr(), cert.as_ptr())).map(|_| ()) }
    }

    /// Appends a certificate to the certificate chain.
    ///
    /// This chain should contain all certificates necessary to go from the certificate specified by
    /// `set_certificate` to a trusted root.
    #[corresponds(SSL_CTX_add_extra_chain_cert)]
    pub fn add_extra_chain_cert(&mut self, cert: X509) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_CTX_add_extra_chain_cert(self.as_ptr(), cert.as_ptr()) as c_int)?;
            mem::forget(cert);
            Ok(())
        }
    }

    /// Loads the private key from a file.
    #[corresponds(SSL_CTX_use_PrivateKey_file)]
    pub fn set_private_key_file<P: AsRef<Path>>(
        &mut self,
        file: P,
        file_type: SslFiletype,
    ) -> Result<(), ErrorStack> {
        let file = CString::new(file.as_ref().as_os_str().to_str().unwrap()).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_use_PrivateKey_file(
                self.as_ptr(),
                file.as_ptr() as *const _,
                file_type.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the private key.
    #[corresponds(SSL_CTX_use_PrivateKey)]
    pub fn set_private_key<T>(&mut self, key: &PKeyRef<T>) -> Result<(), ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe { cvt(ffi::SSL_CTX_use_PrivateKey(self.as_ptr(), key.as_ptr())).map(|_| ()) }
    }

    /// Sets the list of supported ciphers for protocols before TLSv1.3.
    ///
    /// The `set_ciphersuites` method controls the cipher suites for TLSv1.3.
    ///
    /// See [`ciphers`] for details on the format.
    ///
    /// [`ciphers`]: https://www.openssl.org/docs/manmaster/apps/ciphers.html
    #[corresponds(SSL_CTX_set_cipher_list)]
    pub fn set_cipher_list(&mut self, cipher_list: &str) -> Result<(), ErrorStack> {
        let cipher_list = CString::new(cipher_list).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_set_cipher_list(
                self.as_ptr(),
                cipher_list.as_ptr() as *const _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the list of supported ciphers for the TLSv1.3 protocol.
    ///
    /// The `set_cipher_list` method controls the cipher suites for protocols before TLSv1.3.
    ///
    /// The format consists of TLSv1.3 cipher suite names separated by `:` characters in order of
    /// preference.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_CTX_set_ciphersuites)]
    #[cfg(any(ossl111, libressl340))]
    pub fn set_ciphersuites(&mut self, cipher_list: &str) -> Result<(), ErrorStack> {
        let cipher_list = CString::new(cipher_list).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_set_ciphersuites(
                self.as_ptr(),
                cipher_list.as_ptr() as *const _,
            ))
            .map(|_| ())
        }
    }

    /// Enables ECDHE key exchange with an automatically chosen curve list.
    ///
    /// Requires OpenSSL 1.0.2.
    #[corresponds(SSL_CTX_set_ecdh_auto)]
    #[cfg(any(libressl, all(ossl102, not(ossl110))))]
    pub fn set_ecdh_auto(&mut self, onoff: bool) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_set_ecdh_auto(self.as_ptr(), onoff as c_int)).map(|_| ()) }
    }

    /// Sets the options used by the context, returning the old set.
    ///
    /// # Note
    ///
    /// This *enables* the specified options, but does not disable unspecified options. Use
    /// `clear_options` for that.
    #[corresponds(SSL_CTX_set_options)]
    pub fn set_options(&mut self, option: SslOptions) -> SslOptions {
        let bits =
            unsafe { ffi::SSL_CTX_set_options(self.as_ptr(), option.bits()) } as SslOptionsRepr;
        SslOptions::from_bits_retain(bits)
    }

    /// Returns the options used by the context.
    #[corresponds(SSL_CTX_get_options)]
    pub fn options(&self) -> SslOptions {
        let bits = unsafe { ffi::SSL_CTX_get_options(self.as_ptr()) } as SslOptionsRepr;
        SslOptions::from_bits_retain(bits)
    }

    /// Clears the options used by the context, returning the old set.
    #[corresponds(SSL_CTX_clear_options)]
    pub fn clear_options(&mut self, option: SslOptions) -> SslOptions {
        let bits =
            unsafe { ffi::SSL_CTX_clear_options(self.as_ptr(), option.bits()) } as SslOptionsRepr;
        SslOptions::from_bits_retain(bits)
    }

    /// Sets the minimum supported protocol version.
    ///
    /// A value of `None` will enable protocol versions down to the lowest version supported by
    /// OpenSSL.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.0 or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_CTX_set_min_proto_version)]
    #[cfg(any(ossl110, libressl261, boringssl, awslc))]
    pub fn set_min_proto_version(&mut self, version: Option<SslVersion>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_CTX_set_min_proto_version(
                self.as_ptr(),
                version.map_or(0, |v| v.0 as _),
            ))
            .map(|_| ())
        }
    }

    /// Sets the maximum supported protocol version.
    ///
    /// A value of `None` will enable protocol versions up to the highest version supported by
    /// OpenSSL.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.0 or or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_CTX_set_max_proto_version)]
    #[cfg(any(ossl110, libressl261, boringssl, awslc))]
    pub fn set_max_proto_version(&mut self, version: Option<SslVersion>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_CTX_set_max_proto_version(
                self.as_ptr(),
                version.map_or(0, |v| v.0 as _),
            ))
            .map(|_| ())
        }
    }

    /// Gets the minimum supported protocol version.
    ///
    /// A value of `None` indicates that all versions down to the lowest version supported by
    /// OpenSSL are enabled.
    ///
    /// Requires OpenSSL 1.1.0g or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_CTX_get_min_proto_version)]
    #[cfg(any(ossl110g, libressl270))]
    pub fn min_proto_version(&mut self) -> Option<SslVersion> {
        unsafe {
            let r = ffi::SSL_CTX_get_min_proto_version(self.as_ptr());
            if r == 0 {
                None
            } else {
                Some(SslVersion(r))
            }
        }
    }

    /// Gets the maximum supported protocol version.
    ///
    /// A value of `None` indicates that all versions up to the highest version supported by
    /// OpenSSL are enabled.
    ///
    /// Requires OpenSSL 1.1.0g or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_CTX_get_max_proto_version)]
    #[cfg(any(ossl110g, libressl270))]
    pub fn max_proto_version(&mut self) -> Option<SslVersion> {
        unsafe {
            let r = ffi::SSL_CTX_get_max_proto_version(self.as_ptr());
            if r == 0 {
                None
            } else {
                Some(SslVersion(r))
            }
        }
    }

    /// Sets the protocols to sent to the server for Application Layer Protocol Negotiation (ALPN).
    ///
    /// The input must be in ALPN "wire format". It consists of a sequence of supported protocol
    /// names prefixed by their byte length. For example, the protocol list consisting of `spdy/1`
    /// and `http/1.1` is encoded as `b"\x06spdy/1\x08http/1.1"`. The protocols are ordered by
    /// preference.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_CTX_set_alpn_protos)]
    #[cfg(any(ossl102, libressl261, boringssl, awslc))]
    pub fn set_alpn_protos(&mut self, protocols: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            assert!(protocols.len() <= c_uint::MAX as usize);
            let r = ffi::SSL_CTX_set_alpn_protos(
                self.as_ptr(),
                protocols.as_ptr(),
                protocols.len() as _,
            );
            // fun fact, SSL_CTX_set_alpn_protos has a reversed return code D:
            if r == 0 {
                Ok(())
            } else {
                Err(ErrorStack::get())
            }
        }
    }

    /// Enables the DTLS extension "use_srtp" as defined in RFC5764.
    #[corresponds(SSL_CTX_set_tlsext_use_srtp)]
    pub fn set_tlsext_use_srtp(&mut self, protocols: &str) -> Result<(), ErrorStack> {
        unsafe {
            let cstr = CString::new(protocols).unwrap();

            let r = ffi::SSL_CTX_set_tlsext_use_srtp(self.as_ptr(), cstr.as_ptr());
            // fun fact, set_tlsext_use_srtp has a reversed return code D:
            if r == 0 {
                Ok(())
            } else {
                Err(ErrorStack::get())
            }
        }
    }

    /// Sets the callback used by a server to select a protocol for Application Layer Protocol
    /// Negotiation (ALPN).
    ///
    /// The callback is provided with the client's protocol list in ALPN wire format. See the
    /// documentation for [`SslContextBuilder::set_alpn_protos`] for details. It should return one
    /// of those protocols on success. The [`select_next_proto`] function implements the standard
    /// protocol selection algorithm.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or LibreSSL 2.6.1 or newer.
    ///
    /// [`SslContextBuilder::set_alpn_protos`]: struct.SslContextBuilder.html#method.set_alpn_protos
    /// [`select_next_proto`]: fn.select_next_proto.html
    #[corresponds(SSL_CTX_set_alpn_select_cb)]
    #[cfg(any(ossl102, libressl261, boringssl, awslc))]
    pub fn set_alpn_select_callback<F>(&mut self, callback: F)
    where
        F: for<'a> Fn(&mut SslRef, &'a [u8]) -> Result<&'a [u8], AlpnError> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            #[cfg(not(any(boringssl, awslc)))]
            ffi::SSL_CTX_set_alpn_select_cb__fixed_rust(
                self.as_ptr(),
                Some(callbacks::raw_alpn_select::<F>),
                ptr::null_mut(),
            );
            #[cfg(any(boringssl, awslc))]
            ffi::SSL_CTX_set_alpn_select_cb(
                self.as_ptr(),
                Some(callbacks::raw_alpn_select::<F>),
                ptr::null_mut(),
            );
        }
    }

    /// Checks for consistency between the private key and certificate.
    #[corresponds(SSL_CTX_check_private_key)]
    pub fn check_private_key(&self) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_check_private_key(self.as_ptr())).map(|_| ()) }
    }

    /// Returns a shared reference to the context's certificate store.
    #[corresponds(SSL_CTX_get_cert_store)]
    pub fn cert_store(&self) -> &X509StoreBuilderRef {
        unsafe { X509StoreBuilderRef::from_ptr(ffi::SSL_CTX_get_cert_store(self.as_ptr())) }
    }

    /// Returns a mutable reference to the context's certificate store.
    #[corresponds(SSL_CTX_get_cert_store)]
    pub fn cert_store_mut(&mut self) -> &mut X509StoreBuilderRef {
        unsafe { X509StoreBuilderRef::from_ptr_mut(ffi::SSL_CTX_get_cert_store(self.as_ptr())) }
    }

    /// Returns a reference to the X509 verification configuration.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or newer.
    #[corresponds(SSL_CTX_get0_param)]
    #[cfg(any(ossl102, boringssl, libressl261, awslc))]
    pub fn verify_param(&self) -> &X509VerifyParamRef {
        unsafe { X509VerifyParamRef::from_ptr(ffi::SSL_CTX_get0_param(self.as_ptr())) }
    }

    /// Returns a mutable reference to the X509 verification configuration.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or newer.
    #[corresponds(SSL_CTX_get0_param)]
    #[cfg(any(ossl102, boringssl, libressl261, awslc))]
    pub fn verify_param_mut(&mut self) -> &mut X509VerifyParamRef {
        unsafe { X509VerifyParamRef::from_ptr_mut(ffi::SSL_CTX_get0_param(self.as_ptr())) }
    }

    /// Sets the callback dealing with OCSP stapling.
    ///
    /// On the client side, this callback is responsible for validating the OCSP status response
    /// returned by the server. The status may be retrieved with the `SslRef::ocsp_status` method.
    /// A response of `Ok(true)` indicates that the OCSP status is valid, and a response of
    /// `Ok(false)` indicates that the OCSP status is invalid and the handshake should be
    /// terminated.
    ///
    /// On the server side, this callback is responsible for setting the OCSP status response to be
    /// returned to clients. The status may be set with the `SslRef::set_ocsp_status` method. A
    /// response of `Ok(true)` indicates that the OCSP status should be returned to the client, and
    /// `Ok(false)` indicates that the status should not be returned to the client.
    #[corresponds(SSL_CTX_set_tlsext_status_cb)]
    pub fn set_status_callback<F>(&mut self, callback: F) -> Result<(), ErrorStack>
    where
        F: Fn(&mut SslRef) -> Result<bool, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            cvt(
                ffi::SSL_CTX_set_tlsext_status_cb(self.as_ptr(), Some(raw_tlsext_status::<F>))
                    as c_int,
            )
            .map(|_| ())
        }
    }

    /// Sets the callback for providing an identity and pre-shared key for a TLS-PSK client.
    ///
    /// The callback will be called with the SSL context, an identity hint if one was provided
    /// by the server, a mutable slice for each of the identity and pre-shared key bytes. The
    /// identity must be written as a null-terminated C string.
    #[corresponds(SSL_CTX_set_psk_client_callback)]
    #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
    pub fn set_psk_client_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, Option<&[u8]>, &mut [u8], &mut [u8]) -> Result<usize, ErrorStack>
            + 'static
            + Sync
            + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_psk_client_callback(self.as_ptr(), Some(raw_client_psk::<F>));
        }
    }

    #[deprecated(since = "0.10.10", note = "renamed to `set_psk_client_callback`")]
    #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
    pub fn set_psk_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, Option<&[u8]>, &mut [u8], &mut [u8]) -> Result<usize, ErrorStack>
            + 'static
            + Sync
            + Send,
    {
        self.set_psk_client_callback(callback)
    }

    /// Sets the callback for providing an identity and pre-shared key for a TLS-PSK server.
    ///
    /// The callback will be called with the SSL context, an identity provided by the client,
    /// and, a mutable slice for the pre-shared key bytes. The callback returns the number of
    /// bytes in the pre-shared key.
    #[corresponds(SSL_CTX_set_psk_server_callback)]
    #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
    pub fn set_psk_server_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, Option<&[u8]>, &mut [u8]) -> Result<usize, ErrorStack>
            + 'static
            + Sync
            + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_psk_server_callback(self.as_ptr(), Some(raw_server_psk::<F>));
        }
    }

    /// Sets the callback which is called when new sessions are negotiated.
    ///
    /// This can be used by clients to implement session caching. While in TLSv1.2 the session is
    /// available to access via [`SslRef::session`] immediately after the handshake completes, this
    /// is not the case for TLSv1.3. There, a session is not generally available immediately, and
    /// the server may provide multiple session tokens to the client over a single session. The new
    /// session callback is a portable way to deal with both cases.
    ///
    /// Note that session caching must be enabled for the callback to be invoked, and it defaults
    /// off for clients. [`set_session_cache_mode`] controls that behavior.
    ///
    /// [`SslRef::session`]: struct.SslRef.html#method.session
    /// [`set_session_cache_mode`]: #method.set_session_cache_mode
    #[corresponds(SSL_CTX_sess_set_new_cb)]
    pub fn set_new_session_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, SslSession) + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_sess_set_new_cb(self.as_ptr(), Some(callbacks::raw_new_session::<F>));
        }
    }

    /// Sets the callback which is called when sessions are removed from the context.
    ///
    /// Sessions can be removed because they have timed out or because they are considered faulty.
    #[corresponds(SSL_CTX_sess_set_remove_cb)]
    pub fn set_remove_session_callback<F>(&mut self, callback: F)
    where
        F: Fn(&SslContextRef, &SslSessionRef) + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_sess_set_remove_cb(
                self.as_ptr(),
                Some(callbacks::raw_remove_session::<F>),
            );
        }
    }

    /// Sets the callback which is called when a client proposed to resume a session but it was not
    /// found in the internal cache.
    ///
    /// The callback is passed a reference to the session ID provided by the client. It should
    /// return the session corresponding to that ID if available. This is only used for servers, not
    /// clients.
    ///
    /// # Safety
    ///
    /// The returned `SslSession` must not be associated with a different `SslContext`.
    #[corresponds(SSL_CTX_sess_set_get_cb)]
    pub unsafe fn set_get_session_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &[u8]) -> Option<SslSession> + 'static + Sync + Send,
    {
        self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
        ffi::SSL_CTX_sess_set_get_cb(self.as_ptr(), Some(callbacks::raw_get_session::<F>));
    }

    /// Sets the TLS key logging callback.
    ///
    /// The callback is invoked whenever TLS key material is generated, and is passed a line of NSS
    /// SSLKEYLOGFILE-formatted text. This can be used by tools like Wireshark to decrypt message
    /// traffic. The line does not contain a trailing newline.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CTX_set_keylog_callback)]
    #[cfg(any(ossl111, boringssl, awslc))]
    pub fn set_keylog_callback<F>(&mut self, callback: F)
    where
        F: Fn(&SslRef, &str) + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_keylog_callback(self.as_ptr(), Some(callbacks::raw_keylog::<F>));
        }
    }

    /// Sets the session caching mode use for connections made with the context.
    ///
    /// Returns the previous session caching mode.
    #[corresponds(SSL_CTX_set_session_cache_mode)]
    pub fn set_session_cache_mode(&mut self, mode: SslSessionCacheMode) -> SslSessionCacheMode {
        unsafe {
            let bits = ffi::SSL_CTX_set_session_cache_mode(self.as_ptr(), mode.bits());
            SslSessionCacheMode::from_bits_retain(bits)
        }
    }

    /// Sets the callback for generating an application cookie for TLS1.3
    /// stateless handshakes.
    ///
    /// The callback will be called with the SSL context and a slice into which the cookie
    /// should be written. The callback should return the number of bytes written.
    #[corresponds(SSL_CTX_set_stateless_cookie_generate_cb)]
    #[cfg(ossl111)]
    pub fn set_stateless_cookie_generate_cb<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &mut [u8]) -> Result<usize, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_stateless_cookie_generate_cb(
                self.as_ptr(),
                Some(raw_stateless_cookie_generate::<F>),
            );
        }
    }

    /// Sets the callback for verifying an application cookie for TLS1.3
    /// stateless handshakes.
    ///
    /// The callback will be called with the SSL context and the cookie supplied by the
    /// client. It should return true if and only if the cookie is valid.
    ///
    /// Note that the OpenSSL implementation independently verifies the integrity of
    /// application cookies using an HMAC before invoking the supplied callback.
    #[corresponds(SSL_CTX_set_stateless_cookie_verify_cb)]
    #[cfg(ossl111)]
    pub fn set_stateless_cookie_verify_cb<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &[u8]) -> bool + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_stateless_cookie_verify_cb(
                self.as_ptr(),
                Some(raw_stateless_cookie_verify::<F>),
            )
        }
    }

    /// Sets the callback for generating a DTLSv1 cookie
    ///
    /// The callback will be called with the SSL context and a slice into which the cookie
    /// should be written. The callback should return the number of bytes written.
    #[corresponds(SSL_CTX_set_cookie_generate_cb)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn set_cookie_generate_cb<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &mut [u8]) -> Result<usize, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_cookie_generate_cb(self.as_ptr(), Some(raw_cookie_generate::<F>));
        }
    }

    /// Sets the callback for verifying a DTLSv1 cookie
    ///
    /// The callback will be called with the SSL context and the cookie supplied by the
    /// client. It should return true if and only if the cookie is valid.
    #[corresponds(SSL_CTX_set_cookie_verify_cb)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn set_cookie_verify_cb<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &[u8]) -> bool + 'static + Sync + Send,
    {
        unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_cookie_verify_cb(self.as_ptr(), Some(raw_cookie_verify::<F>));
        }
    }

    /// Sets the extra data at the specified index.
    ///
    /// This can be used to provide data to callbacks registered with the context. Use the
    /// `SslContext::new_ex_index` method to create an `Index`.
    // FIXME should return a result
    #[corresponds(SSL_CTX_set_ex_data)]
    pub fn set_ex_data<T>(&mut self, index: Index<SslContext, T>, data: T) {
        self.set_ex_data_inner(index, data);
    }

    fn set_ex_data_inner<T>(&mut self, index: Index<SslContext, T>, data: T) -> *mut c_void {
        match self.ex_data_mut(index) {
            Some(v) => {
                *v = data;
                (v as *mut T).cast()
            }
            _ => unsafe {
                let data = Box::into_raw(Box::new(data)) as *mut c_void;
                ffi::SSL_CTX_set_ex_data(self.as_ptr(), index.as_raw(), data);
                data
            },
        }
    }

    fn ex_data_mut<T>(&mut self, index: Index<SslContext, T>) -> Option<&mut T> {
        unsafe {
            let data = ffi::SSL_CTX_get_ex_data(self.as_ptr(), index.as_raw());
            if data.is_null() {
                None
            } else {
                Some(&mut *data.cast())
            }
        }
    }

    /// Adds a custom extension for a TLS/DTLS client or server for all supported protocol versions.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CTX_add_custom_ext)]
    #[cfg(ossl111)]
    pub fn add_custom_ext<AddFn, ParseFn, T>(
        &mut self,
        ext_type: u16,
        context: ExtensionContext,
        add_cb: AddFn,
        parse_cb: ParseFn,
    ) -> Result<(), ErrorStack>
    where
        AddFn: Fn(
                &mut SslRef,
                ExtensionContext,
                Option<(usize, &X509Ref)>,
            ) -> Result<Option<T>, SslAlert>
            + 'static
            + Sync
            + Send,
        T: AsRef<[u8]> + 'static + Sync + Send,
        ParseFn: Fn(
                &mut SslRef,
                ExtensionContext,
                &[u8],
                Option<(usize, &X509Ref)>,
            ) -> Result<(), SslAlert>
            + 'static
            + Sync
            + Send,
    {
        let ret = unsafe {
            self.set_ex_data(SslContext::cached_ex_index::<AddFn>(), add_cb);
            self.set_ex_data(SslContext::cached_ex_index::<ParseFn>(), parse_cb);

            ffi::SSL_CTX_add_custom_ext(
                self.as_ptr(),
                ext_type as c_uint,
                context.bits(),
                Some(raw_custom_ext_add::<AddFn, T>),
                Some(raw_custom_ext_free::<T>),
                ptr::null_mut(),
                Some(raw_custom_ext_parse::<ParseFn>),
                ptr::null_mut(),
            )
        };
        if ret == 1 {
            Ok(())
        } else {
            Err(ErrorStack::get())
        }
    }

    /// Sets the maximum amount of early data that will be accepted on incoming connections.
    ///
    /// Defaults to 0.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_CTX_set_max_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn set_max_early_data(&mut self, bytes: u32) -> Result<(), ErrorStack> {
        if unsafe { ffi::SSL_CTX_set_max_early_data(self.as_ptr(), bytes) } == 1 {
            Ok(())
        } else {
            Err(ErrorStack::get())
        }
    }

    /// Sets a callback which will be invoked just after the client's hello message is received.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CTX_set_client_hello_cb)]
    #[cfg(ossl111)]
    pub fn set_client_hello_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, &mut SslAlert) -> Result<ClientHelloResponse, ErrorStack>
            + 'static
            + Sync
            + Send,
    {
        unsafe {
            let ptr = self.set_ex_data_inner(SslContext::cached_ex_index::<F>(), callback);
            ffi::SSL_CTX_set_client_hello_cb(
                self.as_ptr(),
                Some(callbacks::raw_client_hello::<F>),
                ptr,
            );
        }
    }

    /// Sets the context's session cache size limit, returning the previous limit.
    ///
    /// A value of 0 means that the cache size is unbounded.
    #[corresponds(SSL_CTX_sess_set_cache_size)]
    #[allow(clippy::useless_conversion)]
    pub fn set_session_cache_size(&mut self, size: i32) -> i64 {
        unsafe {
            ffi::SSL_CTX_sess_set_cache_size(self.as_ptr(), size as SslCacheSize) as SslCacheTy
        }
    }

    /// Sets the context's supported signature algorithms.
    ///
    /// Requires OpenSSL 1.0.2 or newer.
    #[corresponds(SSL_CTX_set1_sigalgs_list)]
    #[cfg(ossl102)]
    pub fn set_sigalgs_list(&mut self, sigalgs: &str) -> Result<(), ErrorStack> {
        let sigalgs = CString::new(sigalgs).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_set1_sigalgs_list(self.as_ptr(), sigalgs.as_ptr()) as c_int)
                .map(|_| ())
        }
    }

    /// Sets the context's supported elliptic curve groups.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.1 or LibreSSL 2.5.1 or newer.
    #[corresponds(SSL_CTX_set1_groups_list)]
    #[cfg(any(ossl111, boringssl, libressl251, awslc))]
    pub fn set_groups_list(&mut self, groups: &str) -> Result<(), ErrorStack> {
        let groups = CString::new(groups).unwrap();
        unsafe {
            cvt(ffi::SSL_CTX_set1_groups_list(self.as_ptr(), groups.as_ptr()) as c_int).map(|_| ())
        }
    }

    /// Sets the number of TLS 1.3 session tickets that will be sent to a client after a full
    /// handshake.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CTX_set_num_tickets)]
    #[cfg(ossl111)]
    pub fn set_num_tickets(&mut self, num_tickets: usize) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_CTX_set_num_tickets(self.as_ptr(), num_tickets)).map(|_| ()) }
    }

    /// Set the context's security level to a value between 0 and 5, inclusive.
    /// A security value of 0 allows allows all parameters and algorithms.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    #[corresponds(SSL_CTX_set_security_level)]
    #[cfg(any(ossl110, libressl360))]
    pub fn set_security_level(&mut self, level: u32) {
        unsafe { ffi::SSL_CTX_set_security_level(self.as_ptr(), level as c_int) }
    }

    /// Consumes the builder, returning a new `SslContext`.
    pub fn build(self) -> SslContext {
        self.0
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::SSL_CTX;
    fn drop = ffi::SSL_CTX_free;

    /// A context object for TLS streams.
    ///
    /// Applications commonly configure a single `SslContext` that is shared by all of its
    /// `SslStreams`.
    pub struct SslContext;

    /// Reference to [`SslContext`]
    ///
    /// [`SslContext`]: struct.SslContext.html
    pub struct SslContextRef;
}

impl Clone for SslContext {
    fn clone(&self) -> Self {
        (**self).to_owned()
    }
}

impl ToOwned for SslContextRef {
    type Owned = SslContext;

    fn to_owned(&self) -> Self::Owned {
        unsafe {
            SSL_CTX_up_ref(self.as_ptr());
            SslContext::from_ptr(self.as_ptr())
        }
    }
}

// TODO: add useful info here
impl fmt::Debug for SslContext {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "SslContext")
    }
}

impl SslContext {
    /// Creates a new builder object for an `SslContext`.
    pub fn builder(method: SslMethod) -> Result<SslContextBuilder, ErrorStack> {
        SslContextBuilder::new(method)
    }

    /// Returns a new extra data index.
    ///
    /// Each invocation of this function is guaranteed to return a distinct index. These can be used
    /// to store data in the context that can be retrieved later by callbacks, for example.
    #[corresponds(SSL_CTX_get_ex_new_index)]
    pub fn new_ex_index<T>() -> Result<Index<SslContext, T>, ErrorStack>
    where
        T: 'static + Sync + Send,
    {
        unsafe {
            ffi::init();
            #[cfg(any(boringssl, awslc))]
            let idx = cvt_n(get_new_idx(Some(free_data_box::<T>)))?;
            #[cfg(not(any(boringssl, awslc)))]
            let idx = cvt_n(get_new_idx(free_data_box::<T>))?;
            Ok(Index::from_raw(idx))
        }
    }

    // FIXME should return a result?
    fn cached_ex_index<T>() -> Index<SslContext, T>
    where
        T: 'static + Sync + Send,
    {
        unsafe {
            let idx = *INDEXES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .entry(TypeId::of::<T>())
                .or_insert_with(|| SslContext::new_ex_index::<T>().unwrap().as_raw());
            Index::from_raw(idx)
        }
    }
}

impl SslContextRef {
    /// Returns the certificate associated with this `SslContext`, if present.
    ///
    /// Requires OpenSSL 1.0.2 or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_CTX_get0_certificate)]
    #[cfg(any(ossl102, libressl270))]
    pub fn certificate(&self) -> Option<&X509Ref> {
        unsafe {
            let ptr = ffi::SSL_CTX_get0_certificate(self.as_ptr());
            X509Ref::from_const_ptr_opt(ptr)
        }
    }

    /// Returns the private key associated with this `SslContext`, if present.
    ///
    /// Requires OpenSSL 1.0.2 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_CTX_get0_privatekey)]
    #[cfg(any(ossl102, libressl340))]
    pub fn private_key(&self) -> Option<&PKeyRef<Private>> {
        unsafe {
            let ptr = ffi::SSL_CTX_get0_privatekey(self.as_ptr());
            PKeyRef::from_const_ptr_opt(ptr)
        }
    }

    /// Returns a shared reference to the certificate store used for verification.
    #[corresponds(SSL_CTX_get_cert_store)]
    pub fn cert_store(&self) -> &X509StoreRef {
        unsafe { X509StoreRef::from_ptr(ffi::SSL_CTX_get_cert_store(self.as_ptr())) }
    }

    /// Returns a shared reference to the stack of certificates making up the chain from the leaf.
    #[corresponds(SSL_CTX_get_extra_chain_certs)]
    pub fn extra_chain_certs(&self) -> &StackRef<X509> {
        unsafe {
            let mut chain = ptr::null_mut();
            ffi::SSL_CTX_get_extra_chain_certs(self.as_ptr(), &mut chain);
            StackRef::from_const_ptr_opt(chain).expect("extra chain certs must not be null")
        }
    }

    /// Returns a reference to the extra data at the specified index.
    #[corresponds(SSL_CTX_get_ex_data)]
    pub fn ex_data<T>(&self, index: Index<SslContext, T>) -> Option<&T> {
        unsafe {
            let data = ffi::SSL_CTX_get_ex_data(self.as_ptr(), index.as_raw());
            if data.is_null() {
                None
            } else {
                Some(&*(data as *const T))
            }
        }
    }

    /// Gets the maximum amount of early data that will be accepted on incoming connections.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_CTX_get_max_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn max_early_data(&self) -> u32 {
        unsafe { ffi::SSL_CTX_get_max_early_data(self.as_ptr()) }
    }

    /// Adds a session to the context's cache.
    ///
    /// Returns `true` if the session was successfully added to the cache, and `false` if it was already present.
    ///
    /// # Safety
    ///
    /// The caller of this method is responsible for ensuring that the session has never been used with another
    /// `SslContext` than this one.
    #[corresponds(SSL_CTX_add_session)]
    pub unsafe fn add_session(&self, session: &SslSessionRef) -> bool {
        ffi::SSL_CTX_add_session(self.as_ptr(), session.as_ptr()) != 0
    }

    /// Removes a session from the context's cache and marks it as non-resumable.
    ///
    /// Returns `true` if the session was successfully found and removed, and `false` otherwise.
    ///
    /// # Safety
    ///
    /// The caller of this method is responsible for ensuring that the session has never been used with another
    /// `SslContext` than this one.
    #[corresponds(SSL_CTX_remove_session)]
    pub unsafe fn remove_session(&self, session: &SslSessionRef) -> bool {
        ffi::SSL_CTX_remove_session(self.as_ptr(), session.as_ptr()) != 0
    }

    /// Returns the context's session cache size limit.
    ///
    /// A value of 0 means that the cache size is unbounded.
    #[corresponds(SSL_CTX_sess_get_cache_size)]
    #[allow(clippy::unnecessary_cast)]
    pub fn session_cache_size(&self) -> i64 {
        unsafe { ffi::SSL_CTX_sess_get_cache_size(self.as_ptr()) as i64 }
    }

    /// Returns the verify mode that was set on this context from [`SslContextBuilder::set_verify`].
    ///
    /// [`SslContextBuilder::set_verify`]: struct.SslContextBuilder.html#method.set_verify
    #[corresponds(SSL_CTX_get_verify_mode)]
    pub fn verify_mode(&self) -> SslVerifyMode {
        let mode = unsafe { ffi::SSL_CTX_get_verify_mode(self.as_ptr()) };
        SslVerifyMode::from_bits(mode).expect("SSL_CTX_get_verify_mode returned invalid mode")
    }

    /// Gets the number of TLS 1.3 session tickets that will be sent to a client after a full
    /// handshake.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CTX_get_num_tickets)]
    #[cfg(ossl111)]
    pub fn num_tickets(&self) -> usize {
        unsafe { ffi::SSL_CTX_get_num_tickets(self.as_ptr()) }
    }

    /// Get the context's security level, which controls the allowed parameters
    /// and algorithms.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    #[corresponds(SSL_CTX_get_security_level)]
    #[cfg(any(ossl110, libressl360))]
    pub fn security_level(&self) -> u32 {
        unsafe { ffi::SSL_CTX_get_security_level(self.as_ptr()) as u32 }
    }
}

/// Information about the state of a cipher.
pub struct CipherBits {
    /// The number of secret bits used for the cipher.
    pub secret: i32,

    /// The number of bits processed by the chosen algorithm.
    pub algorithm: i32,
}

/// Information about a cipher.
pub struct SslCipher(*mut ffi::SSL_CIPHER);

impl ForeignType for SslCipher {
    type CType = ffi::SSL_CIPHER;
    type Ref = SslCipherRef;

    #[inline]
    unsafe fn from_ptr(ptr: *mut ffi::SSL_CIPHER) -> SslCipher {
        SslCipher(ptr)
    }

    #[inline]
    fn as_ptr(&self) -> *mut ffi::SSL_CIPHER {
        self.0
    }
}

impl Stackable for SslCipher {
    type StackType = ffi::stack_st_SSL_CIPHER;
}

impl Deref for SslCipher {
    type Target = SslCipherRef;

    fn deref(&self) -> &SslCipherRef {
        unsafe { SslCipherRef::from_ptr(self.0) }
    }
}

impl DerefMut for SslCipher {
    fn deref_mut(&mut self) -> &mut SslCipherRef {
        unsafe { SslCipherRef::from_ptr_mut(self.0) }
    }
}

/// Reference to an [`SslCipher`].
///
/// [`SslCipher`]: struct.SslCipher.html
pub struct SslCipherRef(Opaque);

impl ForeignTypeRef for SslCipherRef {
    type CType = ffi::SSL_CIPHER;
}

impl SslCipherRef {
    /// Returns the name of the cipher.
    #[corresponds(SSL_CIPHER_get_name)]
    pub fn name(&self) -> &'static str {
        unsafe {
            let ptr = ffi::SSL_CIPHER_get_name(self.as_ptr());
            CStr::from_ptr(ptr).to_str().unwrap()
        }
    }

    /// Returns the RFC-standard name of the cipher, if one exists.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CIPHER_standard_name)]
    #[cfg(ossl111)]
    pub fn standard_name(&self) -> Option<&'static str> {
        unsafe {
            let ptr = ffi::SSL_CIPHER_standard_name(self.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_str().unwrap())
            }
        }
    }

    /// Returns the SSL/TLS protocol version that first defined the cipher.
    #[corresponds(SSL_CIPHER_get_version)]
    pub fn version(&self) -> &'static str {
        let version = unsafe {
            let ptr = ffi::SSL_CIPHER_get_version(self.as_ptr());
            CStr::from_ptr(ptr as *const _)
        };

        str::from_utf8(version.to_bytes()).unwrap()
    }

    /// Returns the number of bits used for the cipher.
    #[corresponds(SSL_CIPHER_get_bits)]
    #[allow(clippy::useless_conversion)]
    pub fn bits(&self) -> CipherBits {
        unsafe {
            let mut algo_bits = 0;
            let secret_bits = ffi::SSL_CIPHER_get_bits(self.as_ptr(), &mut algo_bits);
            CipherBits {
                secret: secret_bits.into(),
                algorithm: algo_bits.into(),
            }
        }
    }

    /// Returns a textual description of the cipher.
    #[corresponds(SSL_CIPHER_description)]
    pub fn description(&self) -> String {
        unsafe {
            // SSL_CIPHER_description requires a buffer of at least 128 bytes.
            let mut buf = [0; 128];
            let ptr = ffi::SSL_CIPHER_description(self.as_ptr(), buf.as_mut_ptr(), 128);
            String::from_utf8(CStr::from_ptr(ptr as *const _).to_bytes().to_vec()).unwrap()
        }
    }

    /// Returns the handshake digest of the cipher.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_CIPHER_get_handshake_digest)]
    #[cfg(ossl111)]
    pub fn handshake_digest(&self) -> Option<MessageDigest> {
        unsafe {
            let ptr = ffi::SSL_CIPHER_get_handshake_digest(self.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(MessageDigest::from_ptr(ptr))
            }
        }
    }

    /// Returns the NID corresponding to the cipher.
    ///
    /// Requires OpenSSL 1.1.0 or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_CIPHER_get_cipher_nid)]
    #[cfg(any(ossl110, libressl270))]
    pub fn cipher_nid(&self) -> Option<Nid> {
        let n = unsafe { ffi::SSL_CIPHER_get_cipher_nid(self.as_ptr()) };
        if n == 0 {
            None
        } else {
            Some(Nid::from_raw(n))
        }
    }
}

impl fmt::Debug for SslCipherRef {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}", self.name())
    }
}

/// A stack of selected ciphers, and a stack of selected signalling cipher suites
#[derive(Debug)]
pub struct CipherLists {
    pub suites: Stack<SslCipher>,
    pub signalling_suites: Stack<SslCipher>,
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::SSL_SESSION;
    fn drop = ffi::SSL_SESSION_free;

    /// An encoded SSL session.
    ///
    /// These can be cached to share sessions across connections.
    pub struct SslSession;

    /// Reference to [`SslSession`].
    ///
    /// [`SslSession`]: struct.SslSession.html
    pub struct SslSessionRef;
}

impl Clone for SslSession {
    fn clone(&self) -> SslSession {
        SslSessionRef::to_owned(self)
    }
}

impl SslSession {
    from_der! {
        /// Deserializes a DER-encoded session structure.
        #[corresponds(d2i_SSL_SESSION)]
        from_der,
        SslSession,
        ffi::d2i_SSL_SESSION
    }
}

impl ToOwned for SslSessionRef {
    type Owned = SslSession;

    fn to_owned(&self) -> SslSession {
        unsafe {
            SSL_SESSION_up_ref(self.as_ptr());
            SslSession(self.as_ptr())
        }
    }
}

impl SslSessionRef {
    /// Returns the SSL session ID.
    #[corresponds(SSL_SESSION_get_id)]
    pub fn id(&self) -> &[u8] {
        unsafe {
            let mut len = 0;
            let p = ffi::SSL_SESSION_get_id(self.as_ptr(), &mut len);
            #[allow(clippy::unnecessary_cast)]
            util::from_raw_parts(p as *const u8, len as usize)
        }
    }

    /// Returns the length of the master key.
    #[corresponds(SSL_SESSION_get_master_key)]
    pub fn master_key_len(&self) -> usize {
        unsafe { SSL_SESSION_get_master_key(self.as_ptr(), ptr::null_mut(), 0) }
    }

    /// Copies the master key into the provided buffer.
    ///
    /// Returns the number of bytes written, or the size of the master key if the buffer is empty.
    #[corresponds(SSL_SESSION_get_master_key)]
    pub fn master_key(&self, buf: &mut [u8]) -> usize {
        unsafe { SSL_SESSION_get_master_key(self.as_ptr(), buf.as_mut_ptr(), buf.len()) }
    }

    /// Gets the maximum amount of early data that can be sent on this session.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_SESSION_get_max_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn max_early_data(&self) -> u32 {
        unsafe { ffi::SSL_SESSION_get_max_early_data(self.as_ptr()) }
    }

    /// Returns the time at which the session was established, in seconds since the Unix epoch.
    #[corresponds(SSL_SESSION_get_time)]
    #[allow(clippy::useless_conversion)]
    pub fn time(&self) -> SslTimeTy {
        unsafe { ffi::SSL_SESSION_get_time(self.as_ptr()) }
    }

    /// Returns the sessions timeout, in seconds.
    ///
    /// A session older than this time should not be used for session resumption.
    #[corresponds(SSL_SESSION_get_timeout)]
    #[allow(clippy::useless_conversion)]
    pub fn timeout(&self) -> i64 {
        unsafe { ffi::SSL_SESSION_get_timeout(self.as_ptr()).into() }
    }

    /// Returns the session's TLS protocol version.
    ///
    /// Requires OpenSSL 1.1.0 or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_SESSION_get_protocol_version)]
    #[cfg(any(ossl110, libressl270))]
    pub fn protocol_version(&self) -> SslVersion {
        unsafe {
            let version = ffi::SSL_SESSION_get_protocol_version(self.as_ptr());
            SslVersion(version)
        }
    }

    to_der! {
        /// Serializes the session into a DER-encoded structure.
        #[corresponds(i2d_SSL_SESSION)]
        to_der,
        ffi::i2d_SSL_SESSION
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::SSL;
    fn drop = ffi::SSL_free;

    /// The state of an SSL/TLS session.
    ///
    /// `Ssl` objects are created from an [`SslContext`], which provides configuration defaults.
    /// These defaults can be overridden on a per-`Ssl` basis, however.
    ///
    /// [`SslContext`]: struct.SslContext.html
    pub struct Ssl;

    /// Reference to an [`Ssl`].
    ///
    /// [`Ssl`]: struct.Ssl.html
    pub struct SslRef;
}

impl fmt::Debug for Ssl {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, fmt)
    }
}

impl Ssl {
    /// Returns a new extra data index.
    ///
    /// Each invocation of this function is guaranteed to return a distinct index. These can be used
    /// to store data in the context that can be retrieved later by callbacks, for example.
    #[corresponds(SSL_get_ex_new_index)]
    pub fn new_ex_index<T>() -> Result<Index<Ssl, T>, ErrorStack>
    where
        T: 'static + Sync + Send,
    {
        unsafe {
            ffi::init();
            #[cfg(any(boringssl, awslc))]
            let idx = cvt_n(get_new_ssl_idx(Some(free_data_box::<T>)))?;
            #[cfg(not(any(boringssl, awslc)))]
            let idx = cvt_n(get_new_ssl_idx(free_data_box::<T>))?;
            Ok(Index::from_raw(idx))
        }
    }

    // FIXME should return a result?
    fn cached_ex_index<T>() -> Index<Ssl, T>
    where
        T: 'static + Sync + Send,
    {
        unsafe {
            let idx = *SSL_INDEXES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Ssl::new_ex_index::<T>().unwrap().as_raw());
            Index::from_raw(idx)
        }
    }

    /// Creates a new `Ssl`.
    #[corresponds(SSL_new)]
    pub fn new(ctx: &SslContextRef) -> Result<Ssl, ErrorStack> {
        let session_ctx_index = try_get_session_ctx_index()?;
        unsafe {
            let ptr = cvt_p(ffi::SSL_new(ctx.as_ptr()))?;
            let mut ssl = Ssl::from_ptr(ptr);
            ssl.set_ex_data(*session_ctx_index, ctx.to_owned());

            Ok(ssl)
        }
    }

    /// Initiates a client-side TLS handshake.
    /// # Warning
    ///
    /// OpenSSL's default configuration is insecure. It is highly recommended to use
    /// `SslConnector` rather than `Ssl` directly, as it manages that configuration.
    #[corresponds(SSL_connect)]
    #[allow(deprecated)]
    pub fn connect<S>(self, stream: S) -> Result<SslStream<S>, HandshakeError<S>>
    where
        S: Read + Write,
    {
        SslStreamBuilder::new(self, stream).connect()
    }

    /// Initiates a server-side TLS handshake.
    ///
    /// # Warning
    ///
    /// OpenSSL's default configuration is insecure. It is highly recommended to use
    /// `SslAcceptor` rather than `Ssl` directly, as it manages that configuration.
    #[corresponds(SSL_accept)]
    #[allow(deprecated)]
    pub fn accept<S>(self, stream: S) -> Result<SslStream<S>, HandshakeError<S>>
    where
        S: Read + Write,
    {
        SslStreamBuilder::new(self, stream).accept()
    }
}

impl fmt::Debug for SslRef {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Ssl")
            .field("state", &self.state_string_long())
            .field("verify_result", &self.verify_result())
            .finish()
    }
}

impl SslRef {
    fn get_raw_rbio(&self) -> *mut ffi::BIO {
        unsafe { ffi::SSL_get_rbio(self.as_ptr()) }
    }

    fn get_error(&self, ret: c_int) -> ErrorCode {
        unsafe { ErrorCode::from_raw(ffi::SSL_get_error(self.as_ptr(), ret)) }
    }

    /// Configure as an outgoing stream from a client.
    #[corresponds(SSL_set_connect_state)]
    pub fn set_connect_state(&mut self) {
        unsafe { ffi::SSL_set_connect_state(self.as_ptr()) }
    }

    /// Configure as an incoming stream to a server.
    #[corresponds(SSL_set_accept_state)]
    pub fn set_accept_state(&mut self) {
        unsafe { ffi::SSL_set_accept_state(self.as_ptr()) }
    }

    /// Like [`SslContextBuilder::set_verify`].
    ///
    /// [`SslContextBuilder::set_verify`]: struct.SslContextBuilder.html#method.set_verify
    #[corresponds(SSL_set_verify)]
    pub fn set_verify(&mut self, mode: SslVerifyMode) {
        unsafe { ffi::SSL_set_verify(self.as_ptr(), mode.bits() as c_int, None) }
    }

    /// Returns the verify mode that was set using `set_verify`.
    #[corresponds(SSL_set_verify_mode)]
    pub fn verify_mode(&self) -> SslVerifyMode {
        let mode = unsafe { ffi::SSL_get_verify_mode(self.as_ptr()) };
        SslVerifyMode::from_bits(mode).expect("SSL_get_verify_mode returned invalid mode")
    }

    /// Like [`SslContextBuilder::set_verify_callback`].
    ///
    /// [`SslContextBuilder::set_verify_callback`]: struct.SslContextBuilder.html#method.set_verify_callback
    #[corresponds(SSL_set_verify)]
    pub fn set_verify_callback<F>(&mut self, mode: SslVerifyMode, verify: F)
    where
        F: Fn(bool, &mut X509StoreContextRef) -> bool + 'static + Sync + Send,
    {
        unsafe {
            // this needs to be in an Arc since the callback can register a new callback!
            self.set_ex_data(Ssl::cached_ex_index(), Arc::new(verify));
            ffi::SSL_set_verify(
                self.as_ptr(),
                mode.bits() as c_int,
                Some(ssl_raw_verify::<F>),
            );
        }
    }

    /// Like [`SslContextBuilder::set_tmp_dh`].
    ///
    /// [`SslContextBuilder::set_tmp_dh`]: struct.SslContextBuilder.html#method.set_tmp_dh
    #[corresponds(SSL_set_tmp_dh)]
    pub fn set_tmp_dh(&mut self, dh: &DhRef<Params>) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_set_tmp_dh(self.as_ptr(), dh.as_ptr()) as c_int).map(|_| ()) }
    }

    /// Like [`SslContextBuilder::set_tmp_dh_callback`].
    ///
    /// [`SslContextBuilder::set_tmp_dh_callback`]: struct.SslContextBuilder.html#method.set_tmp_dh_callback
    #[corresponds(SSL_set_tmp_dh_callback)]
    pub fn set_tmp_dh_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, bool, u32) -> Result<Dh<Params>, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            // this needs to be in an Arc since the callback can register a new callback!
            self.set_ex_data(Ssl::cached_ex_index(), Arc::new(callback));
            #[cfg(any(boringssl, awslc))]
            ffi::SSL_set_tmp_dh_callback(self.as_ptr(), Some(raw_tmp_dh_ssl::<F>));
            #[cfg(not(any(boringssl, awslc)))]
            ffi::SSL_set_tmp_dh_callback__fixed_rust(self.as_ptr(), Some(raw_tmp_dh_ssl::<F>));
        }
    }

    /// Like [`SslContextBuilder::set_tmp_ecdh`].
    ///
    /// [`SslContextBuilder::set_tmp_ecdh`]: struct.SslContextBuilder.html#method.set_tmp_ecdh
    #[corresponds(SSL_set_tmp_ecdh)]
    pub fn set_tmp_ecdh(&mut self, key: &EcKeyRef<Params>) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_set_tmp_ecdh(self.as_ptr(), key.as_ptr()) as c_int).map(|_| ()) }
    }

    /// Like [`SslContextBuilder::set_tmp_ecdh_callback`].
    ///
    /// Requires OpenSSL 1.0.1 or 1.0.2.
    #[corresponds(SSL_set_tmp_ecdh_callback)]
    #[cfg(all(ossl101, not(ossl110)))]
    #[deprecated(note = "this function leaks memory and does not exist on newer OpenSSL versions")]
    pub fn set_tmp_ecdh_callback<F>(&mut self, callback: F)
    where
        F: Fn(&mut SslRef, bool, u32) -> Result<EcKey<Params>, ErrorStack> + 'static + Sync + Send,
    {
        unsafe {
            // this needs to be in an Arc since the callback can register a new callback!
            self.set_ex_data(Ssl::cached_ex_index(), Arc::new(callback));
            ffi::SSL_set_tmp_ecdh_callback__fixed_rust(self.as_ptr(), Some(raw_tmp_ecdh_ssl::<F>));
        }
    }

    /// Like [`SslContextBuilder::set_ecdh_auto`].
    ///
    /// Requires OpenSSL 1.0.2 or LibreSSL.
    ///
    /// [`SslContextBuilder::set_tmp_ecdh`]: struct.SslContextBuilder.html#method.set_tmp_ecdh
    #[corresponds(SSL_set_ecdh_auto)]
    #[cfg(any(all(ossl102, not(ossl110)), libressl))]
    pub fn set_ecdh_auto(&mut self, onoff: bool) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_set_ecdh_auto(self.as_ptr(), onoff as c_int)).map(|_| ()) }
    }

    /// Like [`SslContextBuilder::set_alpn_protos`].
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or LibreSSL 2.6.1 or newer.
    ///
    /// [`SslContextBuilder::set_alpn_protos`]: struct.SslContextBuilder.html#method.set_alpn_protos
    #[corresponds(SSL_set_alpn_protos)]
    #[cfg(any(ossl102, libressl261, boringssl, awslc))]
    pub fn set_alpn_protos(&mut self, protocols: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            assert!(protocols.len() <= c_uint::MAX as usize);
            let r =
                ffi::SSL_set_alpn_protos(self.as_ptr(), protocols.as_ptr(), protocols.len() as _);
            // fun fact, SSL_set_alpn_protos has a reversed return code D:
            if r == 0 {
                Ok(())
            } else {
                Err(ErrorStack::get())
            }
        }
    }

    /// Returns the current cipher if the session is active.
    #[corresponds(SSL_get_current_cipher)]
    pub fn current_cipher(&self) -> Option<&SslCipherRef> {
        unsafe {
            let ptr = ffi::SSL_get_current_cipher(self.as_ptr());

            SslCipherRef::from_const_ptr_opt(ptr)
        }
    }

    /// Returns a short string describing the state of the session.
    #[corresponds(SSL_state_string)]
    pub fn state_string(&self) -> &'static str {
        let state = unsafe {
            let ptr = ffi::SSL_state_string(self.as_ptr());
            CStr::from_ptr(ptr as *const _)
        };

        str::from_utf8(state.to_bytes()).unwrap()
    }

    /// Returns a longer string describing the state of the session.
    #[corresponds(SSL_state_string_long)]
    pub fn state_string_long(&self) -> &'static str {
        let state = unsafe {
            let ptr = ffi::SSL_state_string_long(self.as_ptr());
            CStr::from_ptr(ptr as *const _)
        };

        str::from_utf8(state.to_bytes()).unwrap()
    }

    /// Sets the host name to be sent to the server for Server Name Indication (SNI).
    ///
    /// It has no effect for a server-side connection.
    #[corresponds(SSL_set_tlsext_host_name)]
    pub fn set_hostname(&mut self, hostname: &str) -> Result<(), ErrorStack> {
        let cstr = CString::new(hostname).unwrap();
        unsafe {
            cvt(ffi::SSL_set_tlsext_host_name(self.as_ptr(), cstr.as_ptr() as *mut _) as c_int)
                .map(|_| ())
        }
    }

    /// Returns the peer's certificate, if present.
    #[corresponds(SSL_get_peer_certificate)]
    pub fn peer_certificate(&self) -> Option<X509> {
        unsafe {
            let ptr = SSL_get1_peer_certificate(self.as_ptr());
            X509::from_ptr_opt(ptr)
        }
    }

    /// Returns the certificate chain of the peer, if present.
    ///
    /// On the client side, the chain includes the leaf certificate, but on the server side it does
    /// not. Fun!
    #[corresponds(SSL_get_peer_cert_chain)]
    pub fn peer_cert_chain(&self) -> Option<&StackRef<X509>> {
        unsafe {
            let ptr = ffi::SSL_get_peer_cert_chain(self.as_ptr());
            StackRef::from_const_ptr_opt(ptr)
        }
    }

    /// Returns the verified certificate chain of the peer, including the leaf certificate.
    ///
    /// If verification was not successful (i.e. [`verify_result`] does not return
    /// [`X509VerifyResult::OK`]), this chain may be incomplete or invalid.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    ///
    /// [`verify_result`]: #method.verify_result
    /// [`X509VerifyResult::OK`]: ../x509/struct.X509VerifyResult.html#associatedconstant.OK
    #[corresponds(SSL_get0_verified_chain)]
    #[cfg(ossl110)]
    pub fn verified_chain(&self) -> Option<&StackRef<X509>> {
        unsafe {
            let ptr = ffi::SSL_get0_verified_chain(self.as_ptr());
            StackRef::from_const_ptr_opt(ptr)
        }
    }

    /// Like [`SslContext::certificate`].
    #[corresponds(SSL_get_certificate)]
    pub fn certificate(&self) -> Option<&X509Ref> {
        unsafe {
            let ptr = ffi::SSL_get_certificate(self.as_ptr());
            X509Ref::from_const_ptr_opt(ptr)
        }
    }

    /// Like [`SslContext::private_key`].
    ///
    /// [`SslContext::private_key`]: struct.SslContext.html#method.private_key
    #[corresponds(SSL_get_privatekey)]
    pub fn private_key(&self) -> Option<&PKeyRef<Private>> {
        unsafe {
            let ptr = ffi::SSL_get_privatekey(self.as_ptr());
            PKeyRef::from_const_ptr_opt(ptr)
        }
    }

    #[deprecated(since = "0.10.5", note = "renamed to `version_str`")]
    pub fn version(&self) -> &str {
        self.version_str()
    }

    /// Returns the protocol version of the session.
    #[corresponds(SSL_version)]
    pub fn version2(&self) -> Option<SslVersion> {
        unsafe {
            let r = ffi::SSL_version(self.as_ptr());
            if r == 0 {
                None
            } else {
                Some(SslVersion(r))
            }
        }
    }

    /// Returns a string describing the protocol version of the session.
    #[corresponds(SSL_get_version)]
    pub fn version_str(&self) -> &'static str {
        let version = unsafe {
            let ptr = ffi::SSL_get_version(self.as_ptr());
            CStr::from_ptr(ptr as *const _)
        };

        str::from_utf8(version.to_bytes()).unwrap()
    }

    /// Returns the protocol selected via Application Layer Protocol Negotiation (ALPN).
    ///
    /// The protocol's name is returned is an opaque sequence of bytes. It is up to the client
    /// to interpret it.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_get0_alpn_selected)]
    #[cfg(any(ossl102, libressl261, boringssl, awslc))]
    pub fn selected_alpn_protocol(&self) -> Option<&[u8]> {
        unsafe {
            let mut data: *const c_uchar = ptr::null();
            let mut len: c_uint = 0;
            // Get the negotiated protocol from the SSL instance.
            // `data` will point at a `c_uchar` array; `len` will contain the length of this array.
            ffi::SSL_get0_alpn_selected(self.as_ptr(), &mut data, &mut len);

            if data.is_null() {
                None
            } else {
                Some(util::from_raw_parts(data, len as usize))
            }
        }
    }

    /// Enables the DTLS extension "use_srtp" as defined in RFC5764.
    #[corresponds(SSL_set_tlsext_use_srtp)]
    pub fn set_tlsext_use_srtp(&mut self, protocols: &str) -> Result<(), ErrorStack> {
        unsafe {
            let cstr = CString::new(protocols).unwrap();

            let r = ffi::SSL_set_tlsext_use_srtp(self.as_ptr(), cstr.as_ptr());
            // fun fact, set_tlsext_use_srtp has a reversed return code D:
            if r == 0 {
                Ok(())
            } else {
                Err(ErrorStack::get())
            }
        }
    }

    /// Gets all SRTP profiles that are enabled for handshake via set_tlsext_use_srtp
    ///
    /// DTLS extension "use_srtp" as defined in RFC5764 has to be enabled.
    #[corresponds(SSL_get_srtp_profiles)]
    pub fn srtp_profiles(&self) -> Option<&StackRef<SrtpProtectionProfile>> {
        unsafe {
            let chain = ffi::SSL_get_srtp_profiles(self.as_ptr());

            StackRef::from_const_ptr_opt(chain)
        }
    }

    /// Gets the SRTP profile selected by handshake.
    ///
    /// DTLS extension "use_srtp" as defined in RFC5764 has to be enabled.
    #[corresponds(SSL_get_selected_srtp_profile)]
    pub fn selected_srtp_profile(&self) -> Option<&SrtpProtectionProfileRef> {
        unsafe {
            let profile = ffi::SSL_get_selected_srtp_profile(self.as_ptr());

            SrtpProtectionProfileRef::from_const_ptr_opt(profile)
        }
    }

    /// Returns the number of bytes remaining in the currently processed TLS record.
    ///
    /// If this is greater than 0, the next call to `read` will not call down to the underlying
    /// stream.
    #[corresponds(SSL_pending)]
    pub fn pending(&self) -> usize {
        unsafe { ffi::SSL_pending(self.as_ptr()) as usize }
    }

    /// Returns the servername sent by the client via Server Name Indication (SNI).
    ///
    /// It is only useful on the server side.
    ///
    /// # Note
    ///
    /// While the SNI specification requires that servernames be valid domain names (and therefore
    /// ASCII), OpenSSL does not enforce this restriction. If the servername provided by the client
    /// is not valid UTF-8, this function will return `None`. The `servername_raw` method returns
    /// the raw bytes and does not have this restriction.
    ///
    /// [`SSL_get_servername`]: https://www.openssl.org/docs/manmaster/man3/SSL_get_servername.html
    #[corresponds(SSL_get_servername)]
    // FIXME maybe rethink in 0.11?
    pub fn servername(&self, type_: NameType) -> Option<&str> {
        self.servername_raw(type_)
            .and_then(|b| str::from_utf8(b).ok())
    }

    /// Returns the servername sent by the client via Server Name Indication (SNI).
    ///
    /// It is only useful on the server side.
    ///
    /// # Note
    ///
    /// Unlike `servername`, this method does not require the name be valid UTF-8.
    #[corresponds(SSL_get_servername)]
    pub fn servername_raw(&self, type_: NameType) -> Option<&[u8]> {
        unsafe {
            let name = ffi::SSL_get_servername(self.as_ptr(), type_.0);
            if name.is_null() {
                None
            } else {
                Some(CStr::from_ptr(name as *const _).to_bytes())
            }
        }
    }

    /// Changes the context corresponding to the current connection.
    ///
    /// It is most commonly used in the Server Name Indication (SNI) callback.
    #[corresponds(SSL_set_SSL_CTX)]
    pub fn set_ssl_context(&mut self, ctx: &SslContextRef) -> Result<(), ErrorStack> {
        unsafe { cvt_p(ffi::SSL_set_SSL_CTX(self.as_ptr(), ctx.as_ptr())).map(|_| ()) }
    }

    /// Returns the context corresponding to the current connection.
    #[corresponds(SSL_get_SSL_CTX)]
    pub fn ssl_context(&self) -> &SslContextRef {
        unsafe {
            let ssl_ctx = ffi::SSL_get_SSL_CTX(self.as_ptr());
            SslContextRef::from_ptr(ssl_ctx)
        }
    }

    /// Returns a mutable reference to the X509 verification configuration.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.0.2 or newer.
    #[corresponds(SSL_get0_param)]
    #[cfg(any(ossl102, boringssl, libressl261, awslc))]
    pub fn param_mut(&mut self) -> &mut X509VerifyParamRef {
        unsafe { X509VerifyParamRef::from_ptr_mut(ffi::SSL_get0_param(self.as_ptr())) }
    }

    /// Returns the certificate verification result.
    #[corresponds(SSL_get_verify_result)]
    pub fn verify_result(&self) -> X509VerifyResult {
        unsafe { X509VerifyResult::from_raw(ffi::SSL_get_verify_result(self.as_ptr()) as c_int) }
    }

    /// Returns a shared reference to the SSL session.
    #[corresponds(SSL_get_session)]
    pub fn session(&self) -> Option<&SslSessionRef> {
        unsafe {
            let p = ffi::SSL_get_session(self.as_ptr());
            SslSessionRef::from_const_ptr_opt(p)
        }
    }

    /// Copies the `client_random` value sent by the client in the TLS handshake into a buffer.
    ///
    /// Returns the number of bytes copied, or if the buffer is empty, the size of the `client_random`
    /// value.
    ///
    /// Requires OpenSSL 1.1.0 or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_get_client_random)]
    #[cfg(any(ossl110, libressl270))]
    pub fn client_random(&self, buf: &mut [u8]) -> usize {
        unsafe {
            ffi::SSL_get_client_random(self.as_ptr(), buf.as_mut_ptr() as *mut c_uchar, buf.len())
        }
    }

    /// Copies the `server_random` value sent by the server in the TLS handshake into a buffer.
    ///
    /// Returns the number of bytes copied, or if the buffer is empty, the size of the `server_random`
    /// value.
    ///
    /// Requires OpenSSL 1.1.0 or LibreSSL 2.7.0 or newer.
    #[corresponds(SSL_get_server_random)]
    #[cfg(any(ossl110, libressl270))]
    pub fn server_random(&self, buf: &mut [u8]) -> usize {
        unsafe {
            ffi::SSL_get_server_random(self.as_ptr(), buf.as_mut_ptr() as *mut c_uchar, buf.len())
        }
    }

    /// Derives keying material for application use in accordance to RFC 5705.
    #[corresponds(SSL_export_keying_material)]
    pub fn export_keying_material(
        &self,
        out: &mut [u8],
        label: &str,
        context: Option<&[u8]>,
    ) -> Result<(), ErrorStack> {
        unsafe {
            let (context, contextlen, use_context) = match context {
                Some(context) => (context.as_ptr() as *const c_uchar, context.len(), 1),
                None => (ptr::null(), 0, 0),
            };
            cvt(ffi::SSL_export_keying_material(
                self.as_ptr(),
                out.as_mut_ptr() as *mut c_uchar,
                out.len(),
                label.as_ptr() as *const c_char,
                label.len(),
                context,
                contextlen,
                use_context,
            ))
            .map(|_| ())
        }
    }

    /// Derives keying material for application use in accordance to RFC 5705.
    ///
    /// This function is only usable with TLSv1.3, wherein there is no distinction between an empty context and no
    /// context. Therefore, unlike `export_keying_material`, `context` must always be supplied.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_export_keying_material_early)]
    #[cfg(ossl111)]
    pub fn export_keying_material_early(
        &self,
        out: &mut [u8],
        label: &str,
        context: &[u8],
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_export_keying_material_early(
                self.as_ptr(),
                out.as_mut_ptr() as *mut c_uchar,
                out.len(),
                label.as_ptr() as *const c_char,
                label.len(),
                context.as_ptr() as *const c_uchar,
                context.len(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the session to be used.
    ///
    /// This should be called before the handshake to attempt to reuse a previously established
    /// session. If the server is not willing to reuse the session, a new one will be transparently
    /// negotiated.
    ///
    /// # Safety
    ///
    /// The caller of this method is responsible for ensuring that the session is associated
    /// with the same `SslContext` as this `Ssl`.
    #[corresponds(SSL_set_session)]
    pub unsafe fn set_session(&mut self, session: &SslSessionRef) -> Result<(), ErrorStack> {
        cvt(ffi::SSL_set_session(self.as_ptr(), session.as_ptr())).map(|_| ())
    }

    /// Determines if the session provided to `set_session` was successfully reused.
    #[corresponds(SSL_session_reused)]
    pub fn session_reused(&self) -> bool {
        unsafe { ffi::SSL_session_reused(self.as_ptr()) != 0 }
    }

    /// Sets the status response a client wishes the server to reply with.
    #[corresponds(SSL_set_tlsext_status_type)]
    pub fn set_status_type(&mut self, type_: StatusType) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_set_tlsext_status_type(self.as_ptr(), type_.as_raw()) as c_int).map(|_| ())
        }
    }

    /// Determines if current session used Extended Master Secret
    ///
    /// Returns `None` if the handshake is still in-progress.
    #[corresponds(SSL_get_extms_support)]
    #[cfg(ossl110)]
    pub fn extms_support(&self) -> Option<bool> {
        unsafe {
            match ffi::SSL_get_extms_support(self.as_ptr()) {
                -1 => None,
                ret => Some(ret != 0),
            }
        }
    }

    /// Returns the server's OCSP response, if present.
    #[corresponds(SSL_get_tlsext_status_ocsp_resp)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn ocsp_status(&self) -> Option<&[u8]> {
        unsafe {
            let mut p = ptr::null_mut();
            let len = ffi::SSL_get_tlsext_status_ocsp_resp(self.as_ptr(), &mut p);

            if len < 0 {
                None
            } else {
                Some(util::from_raw_parts(p as *const u8, len as usize))
            }
        }
    }

    /// Sets the OCSP response to be returned to the client.
    #[corresponds(SSL_set_tlsext_status_oscp_resp)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn set_ocsp_status(&mut self, response: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            assert!(response.len() <= c_int::MAX as usize);
            let p = cvt_p(ffi::OPENSSL_malloc(response.len() as _))?;
            ptr::copy_nonoverlapping(response.as_ptr(), p as *mut u8, response.len());
            cvt(ffi::SSL_set_tlsext_status_ocsp_resp(
                self.as_ptr(),
                p as *mut c_uchar,
                response.len() as c_long,
            ) as c_int)
            .map(|_| ())
            .map_err(|e| {
                ffi::OPENSSL_free(p);
                e
            })
        }
    }

    /// Determines if this `Ssl` is configured for server-side or client-side use.
    #[corresponds(SSL_is_server)]
    pub fn is_server(&self) -> bool {
        unsafe { SSL_is_server(self.as_ptr()) != 0 }
    }

    /// Sets the extra data at the specified index.
    ///
    /// This can be used to provide data to callbacks registered with the context. Use the
    /// `Ssl::new_ex_index` method to create an `Index`.
    // FIXME should return a result
    #[corresponds(SSL_set_ex_data)]
    pub fn set_ex_data<T>(&mut self, index: Index<Ssl, T>, data: T) {
        match self.ex_data_mut(index) {
            Some(v) => *v = data,
            None => unsafe {
                let data = Box::new(data);
                ffi::SSL_set_ex_data(
                    self.as_ptr(),
                    index.as_raw(),
                    Box::into_raw(data) as *mut c_void,
                );
            },
        }
    }

    /// Returns a reference to the extra data at the specified index.
    #[corresponds(SSL_get_ex_data)]
    pub fn ex_data<T>(&self, index: Index<Ssl, T>) -> Option<&T> {
        unsafe {
            let data = ffi::SSL_get_ex_data(self.as_ptr(), index.as_raw());
            if data.is_null() {
                None
            } else {
                Some(&*(data as *const T))
            }
        }
    }

    /// Returns a mutable reference to the extra data at the specified index.
    #[corresponds(SSL_get_ex_data)]
    pub fn ex_data_mut<T>(&mut self, index: Index<Ssl, T>) -> Option<&mut T> {
        unsafe {
            let data = ffi::SSL_get_ex_data(self.as_ptr(), index.as_raw());
            if data.is_null() {
                None
            } else {
                Some(&mut *(data as *mut T))
            }
        }
    }

    /// Sets the maximum amount of early data that will be accepted on this connection.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_set_max_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn set_max_early_data(&mut self, bytes: u32) -> Result<(), ErrorStack> {
        if unsafe { ffi::SSL_set_max_early_data(self.as_ptr(), bytes) } == 1 {
            Ok(())
        } else {
            Err(ErrorStack::get())
        }
    }

    /// Gets the maximum amount of early data that can be sent on this connection.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_get_max_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn max_early_data(&self) -> u32 {
        unsafe { ffi::SSL_get_max_early_data(self.as_ptr()) }
    }

    /// Copies the contents of the last Finished message sent to the peer into the provided buffer.
    ///
    /// The total size of the message is returned, so this can be used to determine the size of the
    /// buffer required.
    #[corresponds(SSL_get_finished)]
    pub fn finished(&self, buf: &mut [u8]) -> usize {
        unsafe { ffi::SSL_get_finished(self.as_ptr(), buf.as_mut_ptr() as *mut c_void, buf.len()) }
    }

    /// Copies the contents of the last Finished message received from the peer into the provided
    /// buffer.
    ///
    /// The total size of the message is returned, so this can be used to determine the size of the
    /// buffer required.
    #[corresponds(SSL_get_peer_finished)]
    pub fn peer_finished(&self, buf: &mut [u8]) -> usize {
        unsafe {
            ffi::SSL_get_peer_finished(self.as_ptr(), buf.as_mut_ptr() as *mut c_void, buf.len())
        }
    }

    /// Determines if the initial handshake has been completed.
    #[corresponds(SSL_is_init_finished)]
    #[cfg(ossl110)]
    pub fn is_init_finished(&self) -> bool {
        unsafe { ffi::SSL_is_init_finished(self.as_ptr()) != 0 }
    }

    /// Determines if the client's hello message is in the SSLv2 format.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `false` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_isv2)]
    #[cfg(ossl111)]
    pub fn client_hello_isv2(&self) -> bool {
        unsafe { ffi::SSL_client_hello_isv2(self.as_ptr()) != 0 }
    }

    /// Returns the legacy version field of the client's hello message.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `None` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_get0_legacy_version)]
    #[cfg(ossl111)]
    pub fn client_hello_legacy_version(&self) -> Option<SslVersion> {
        unsafe {
            let version = ffi::SSL_client_hello_get0_legacy_version(self.as_ptr());
            if version == 0 {
                None
            } else {
                Some(SslVersion(version as c_int))
            }
        }
    }

    /// Returns the random field of the client's hello message.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `None` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_get0_random)]
    #[cfg(ossl111)]
    pub fn client_hello_random(&self) -> Option<&[u8]> {
        unsafe {
            let mut ptr = ptr::null();
            let len = ffi::SSL_client_hello_get0_random(self.as_ptr(), &mut ptr);
            if len == 0 {
                None
            } else {
                Some(util::from_raw_parts(ptr, len))
            }
        }
    }

    /// Returns the session ID field of the client's hello message.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `None` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_get0_session_id)]
    #[cfg(ossl111)]
    pub fn client_hello_session_id(&self) -> Option<&[u8]> {
        unsafe {
            let mut ptr = ptr::null();
            let len = ffi::SSL_client_hello_get0_session_id(self.as_ptr(), &mut ptr);
            if len == 0 {
                None
            } else {
                Some(util::from_raw_parts(ptr, len))
            }
        }
    }

    /// Returns the ciphers field of the client's hello message.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `None` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_get0_ciphers)]
    #[cfg(ossl111)]
    pub fn client_hello_ciphers(&self) -> Option<&[u8]> {
        unsafe {
            let mut ptr = ptr::null();
            let len = ffi::SSL_client_hello_get0_ciphers(self.as_ptr(), &mut ptr);
            if len == 0 {
                None
            } else {
                Some(util::from_raw_parts(ptr, len))
            }
        }
    }

    /// Decodes a slice of wire-format cipher suite specification bytes. Unsupported cipher suites
    /// are ignored.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_bytes_to_cipher_list)]
    #[cfg(ossl111)]
    pub fn bytes_to_cipher_list(
        &self,
        bytes: &[u8],
        isv2format: bool,
    ) -> Result<CipherLists, ErrorStack> {
        unsafe {
            let ptr = bytes.as_ptr();
            let len = bytes.len();
            let mut sk = ptr::null_mut();
            let mut scsvs = ptr::null_mut();
            let res = ffi::SSL_bytes_to_cipher_list(
                self.as_ptr(),
                ptr,
                len,
                isv2format as c_int,
                &mut sk,
                &mut scsvs,
            );
            if res == 1 {
                Ok(CipherLists {
                    suites: Stack::from_ptr(sk),
                    signalling_suites: Stack::from_ptr(scsvs),
                })
            } else {
                Err(ErrorStack::get())
            }
        }
    }

    /// Returns the compression methods field of the client's hello message.
    ///
    /// This can only be used inside of the client hello callback. Otherwise, `None` is returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_client_hello_get0_compression_methods)]
    #[cfg(ossl111)]
    pub fn client_hello_compression_methods(&self) -> Option<&[u8]> {
        unsafe {
            let mut ptr = ptr::null();
            let len = ffi::SSL_client_hello_get0_compression_methods(self.as_ptr(), &mut ptr);
            if len == 0 {
                None
            } else {
                Some(util::from_raw_parts(ptr, len))
            }
        }
    }

    /// Sets the MTU used for DTLS connections.
    #[corresponds(SSL_set_mtu)]
    pub fn set_mtu(&mut self, mtu: u32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_set_mtu(self.as_ptr(), mtu as MtuTy) as c_int).map(|_| ()) }
    }

    /// Returns the PSK identity hint used during connection setup.
    ///
    /// May return `None` if no PSK identity hint was used during the connection setup.
    #[corresponds(SSL_get_psk_identity_hint)]
    #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
    pub fn psk_identity_hint(&self) -> Option<&[u8]> {
        unsafe {
            let ptr = ffi::SSL_get_psk_identity_hint(self.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_bytes())
            }
        }
    }

    /// Returns the PSK identity used during connection setup.
    #[corresponds(SSL_get_psk_identity)]
    #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
    pub fn psk_identity(&self) -> Option<&[u8]> {
        unsafe {
            let ptr = ffi::SSL_get_psk_identity(self.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_bytes())
            }
        }
    }

    #[corresponds(SSL_add0_chain_cert)]
    #[cfg(ossl102)]
    pub fn add_chain_cert(&mut self, chain: X509) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_add0_chain_cert(self.as_ptr(), chain.as_ptr()) as c_int).map(|_| ())?;
            mem::forget(chain);
        }
        Ok(())
    }

    /// Sets a new default TLS/SSL method for SSL objects
    #[cfg(not(any(boringssl, awslc)))]
    pub fn set_method(&mut self, method: SslMethod) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_set_ssl_method(self.as_ptr(), method.as_ptr()))?;
        };
        Ok(())
    }

    /// Loads the private key from a file.
    #[corresponds(SSL_use_Private_Key_file)]
    pub fn set_private_key_file<P: AsRef<Path>>(
        &mut self,
        path: P,
        ssl_file_type: SslFiletype,
    ) -> Result<(), ErrorStack> {
        let p = path.as_ref().as_os_str().to_str().unwrap();
        let key_file = CString::new(p).unwrap();
        unsafe {
            cvt(ffi::SSL_use_PrivateKey_file(
                self.as_ptr(),
                key_file.as_ptr(),
                ssl_file_type.as_raw(),
            ))?;
        };
        Ok(())
    }

    /// Sets the private key.
    #[corresponds(SSL_use_PrivateKey)]
    pub fn set_private_key(&mut self, pkey: &PKeyRef<Private>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_use_PrivateKey(self.as_ptr(), pkey.as_ptr()))?;
        };
        Ok(())
    }

    /// Sets the certificate
    #[corresponds(SSL_use_certificate)]
    pub fn set_certificate(&mut self, cert: &X509Ref) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_use_certificate(self.as_ptr(), cert.as_ptr()))?;
        };
        Ok(())
    }

    /// Loads a certificate chain from a file.
    ///
    /// The file should contain a sequence of PEM-formatted certificates, the first being the leaf
    /// certificate, and the remainder forming the chain of certificates up to and including the
    /// trusted root certificate.
    #[corresponds(SSL_use_certificate_chain_file)]
    #[cfg(any(ossl110, libressl332))]
    pub fn set_certificate_chain_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<(), ErrorStack> {
        let p = path.as_ref().as_os_str().to_str().unwrap();
        let cert_file = CString::new(p).unwrap();
        unsafe {
            cvt(ffi::SSL_use_certificate_chain_file(
                self.as_ptr(),
                cert_file.as_ptr(),
            ))?;
        };
        Ok(())
    }

    /// Sets ca certificate that client trusted
    #[corresponds(SSL_add_client_CA)]
    pub fn add_client_ca(&mut self, cacert: &X509Ref) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_add_client_CA(self.as_ptr(), cacert.as_ptr()))?;
        };
        Ok(())
    }

    // Sets the list of CAs sent to the client when requesting a client certificate for the chosen ssl
    #[corresponds(SSL_set_client_CA_list)]
    pub fn set_client_ca_list(&mut self, list: Stack<X509Name>) {
        unsafe { ffi::SSL_set_client_CA_list(self.as_ptr(), list.as_ptr()) }
        mem::forget(list);
    }

    /// Sets the minimum supported protocol version.
    ///
    /// A value of `None` will enable protocol versions down to the lowest version supported by
    /// OpenSSL.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.0 or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_set_min_proto_version)]
    #[cfg(any(ossl110, libressl261, boringssl, awslc))]
    pub fn set_min_proto_version(&mut self, version: Option<SslVersion>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_set_min_proto_version(
                self.as_ptr(),
                version.map_or(0, |v| v.0 as _),
            ))
            .map(|_| ())
        }
    }

    /// Sets the maximum supported protocol version.
    ///
    /// A value of `None` will enable protocol versions up to the highest version supported by
    /// OpenSSL.
    ///
    /// Requires AWS-LC or BoringSSL or OpenSSL 1.1.0 or or LibreSSL 2.6.1 or newer.
    #[corresponds(SSL_set_max_proto_version)]
    #[cfg(any(ossl110, libressl261, boringssl, awslc))]
    pub fn set_max_proto_version(&mut self, version: Option<SslVersion>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_set_max_proto_version(
                self.as_ptr(),
                version.map_or(0, |v| v.0 as _),
            ))
            .map(|_| ())
        }
    }

    /// Sets the list of supported ciphers for the TLSv1.3 protocol.
    ///
    /// The `set_cipher_list` method controls the cipher suites for protocols before TLSv1.3.
    ///
    /// The format consists of TLSv1.3 cipher suite names separated by `:` characters in order of
    /// preference.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_set_ciphersuites)]
    #[cfg(any(ossl111, libressl340))]
    pub fn set_ciphersuites(&mut self, cipher_list: &str) -> Result<(), ErrorStack> {
        let cipher_list = CString::new(cipher_list).unwrap();
        unsafe {
            cvt(ffi::SSL_set_ciphersuites(
                self.as_ptr(),
                cipher_list.as_ptr() as *const _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the list of supported ciphers for protocols before TLSv1.3.
    ///
    /// The `set_ciphersuites` method controls the cipher suites for TLSv1.3.
    ///
    /// See [`ciphers`] for details on the format.
    ///
    /// [`ciphers`]: https://www.openssl.org/docs/manmaster/apps/ciphers.html
    #[corresponds(SSL_set_cipher_list)]
    pub fn set_cipher_list(&mut self, cipher_list: &str) -> Result<(), ErrorStack> {
        let cipher_list = CString::new(cipher_list).unwrap();
        unsafe {
            cvt(ffi::SSL_set_cipher_list(
                self.as_ptr(),
                cipher_list.as_ptr() as *const _,
            ))
            .map(|_| ())
        }
    }

    /// Set the certificate store used for certificate verification
    #[corresponds(SSL_set_cert_store)]
    #[cfg(ossl102)]
    pub fn set_verify_cert_store(&mut self, cert_store: X509Store) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::SSL_set0_verify_cert_store(self.as_ptr(), cert_store.as_ptr()) as c_int)?;
            mem::forget(cert_store);
            Ok(())
        }
    }

    /// Sets the number of TLS 1.3 session tickets that will be sent to a client after a full
    /// handshake.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_set_num_tickets)]
    #[cfg(ossl111)]
    pub fn set_num_tickets(&mut self, num_tickets: usize) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::SSL_set_num_tickets(self.as_ptr(), num_tickets)).map(|_| ()) }
    }

    /// Gets the number of TLS 1.3 session tickets that will be sent to a client after a full
    /// handshake.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(SSL_get_num_tickets)]
    #[cfg(ossl111)]
    pub fn num_tickets(&self) -> usize {
        unsafe { ffi::SSL_get_num_tickets(self.as_ptr()) }
    }

    /// Set the context's security level to a value between 0 and 5, inclusive.
    /// A security value of 0 allows allows all parameters and algorithms.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    #[corresponds(SSL_set_security_level)]
    #[cfg(any(ossl110, libressl360))]
    pub fn set_security_level(&mut self, level: u32) {
        unsafe { ffi::SSL_set_security_level(self.as_ptr(), level as c_int) }
    }

    /// Get the connection's security level, which controls the allowed parameters
    /// and algorithms.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    #[corresponds(SSL_get_security_level)]
    #[cfg(any(ossl110, libressl360))]
    pub fn security_level(&self) -> u32 {
        unsafe { ffi::SSL_get_security_level(self.as_ptr()) as u32 }
    }

    /// Get the temporary key provided by the peer that is used during key
    /// exchange.
    // We use an owned value because EVP_KEY free need to be called when it is
    // dropped
    #[corresponds(SSL_get_peer_tmp_key)]
    #[cfg(ossl300)]
    pub fn peer_tmp_key(&self) -> Result<PKey<Public>, ErrorStack> {
        unsafe {
            let mut key = ptr::null_mut();
            match cvt_long(ffi::SSL_get_peer_tmp_key(self.as_ptr(), &mut key)) {
                Ok(_) => Ok(PKey::<Public>::from_ptr(key)),
                Err(e) => Err(e),
            }
        }
    }

    /// Returns the temporary key from the local end of the connection that is
    /// used during key exchange.
    // We use an owned value because EVP_KEY free need to be called when it is
    // dropped
    #[corresponds(SSL_get_tmp_key)]
    #[cfg(ossl300)]
    pub fn tmp_key(&self) -> Result<PKey<Private>, ErrorStack> {
        unsafe {
            let mut key = ptr::null_mut();
            match cvt_long(ffi::SSL_get_tmp_key(self.as_ptr(), &mut key)) {
                Ok(_) => Ok(PKey::<Private>::from_ptr(key)),
                Err(e) => Err(e),
            }
        }
    }
}

/// An SSL stream midway through the handshake process.
#[derive(Debug)]
pub struct MidHandshakeSslStream<S> {
    stream: SslStream<S>,
    error: Error,
}

impl<S> MidHandshakeSslStream<S> {
    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    /// Returns a shared reference to the `Ssl` of the stream.
    pub fn ssl(&self) -> &SslRef {
        self.stream.ssl()
    }

    /// Returns the underlying error which interrupted this handshake.
    pub fn error(&self) -> &Error {
        &self.error
    }

    /// Consumes `self`, returning its error.
    pub fn into_error(self) -> Error {
        self.error
    }
}

impl<S> MidHandshakeSslStream<S>
where
    S: Read + Write,
{
    /// Restarts the handshake process.
    ///
    #[corresponds(SSL_do_handshake)]
    pub fn handshake(mut self) -> Result<SslStream<S>, HandshakeError<S>> {
        match self.stream.do_handshake() {
            Ok(()) => Ok(self.stream),
            Err(error) => {
                self.error = error;
                match self.error.code() {
                    ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                        Err(HandshakeError::WouldBlock(self))
                    }
                    _ => Err(HandshakeError::Failure(self)),
                }
            }
        }
    }
}

/// A TLS session over a stream.
pub struct SslStream<S> {
    ssl: ManuallyDrop<Ssl>,
    method: ManuallyDrop<BioMethod>,
    _p: PhantomData<S>,
}

impl<S> Drop for SslStream<S> {
    fn drop(&mut self) {
        // ssl holds a reference to method internally so it has to drop first
        unsafe {
            ManuallyDrop::drop(&mut self.ssl);
            ManuallyDrop::drop(&mut self.method);
        }
    }
}

impl<S> fmt::Debug for SslStream<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("SslStream")
            .field("stream", &self.get_ref())
            .field("ssl", &self.ssl())
            .finish()
    }
}

impl<S: Read + Write> SslStream<S> {
    /// Creates a new `SslStream`.
    ///
    /// This function performs no IO; the stream will not have performed any part of the handshake
    /// with the peer. If the `Ssl` was configured with [`SslRef::set_connect_state`] or
    /// [`SslRef::set_accept_state`], the handshake can be performed automatically during the first
    /// call to read or write. Otherwise the `connect` and `accept` methods can be used to
    /// explicitly perform the handshake.
    #[corresponds(SSL_set_bio)]
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        let (bio, method) = bio::new(stream)?;
        unsafe {
            ffi::SSL_set_bio(ssl.as_ptr(), bio, bio);
        }

        Ok(SslStream {
            ssl: ManuallyDrop::new(ssl),
            method: ManuallyDrop::new(method),
            _p: PhantomData,
        })
    }

    /// Constructs an `SslStream` from a pointer to the underlying OpenSSL `SSL` struct.
    ///
    /// This is useful if the handshake has already been completed elsewhere.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is valid.
    #[deprecated(
        since = "0.10.32",
        note = "use Ssl::from_ptr and SslStream::new instead"
    )]
    pub unsafe fn from_raw_parts(ssl: *mut ffi::SSL, stream: S) -> Self {
        let ssl = Ssl::from_ptr(ssl);
        Self::new(ssl, stream).unwrap()
    }

    /// Read application data transmitted by a client before handshake completion.
    ///
    /// Useful for reducing latency, but vulnerable to replay attacks. Call
    /// [`SslRef::set_accept_state`] first.
    ///
    /// Returns `Ok(0)` if all early data has been read.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_read_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn read_early_data(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut read = 0;
        let ret = unsafe {
            ffi::SSL_read_early_data(
                self.ssl.as_ptr(),
                buf.as_ptr() as *mut c_void,
                buf.len(),
                &mut read,
            )
        };
        match ret {
            ffi::SSL_READ_EARLY_DATA_ERROR => Err(self.make_error(ret)),
            ffi::SSL_READ_EARLY_DATA_SUCCESS => Ok(read),
            ffi::SSL_READ_EARLY_DATA_FINISH => Ok(0),
            _ => unreachable!(),
        }
    }

    /// Send data to the server without blocking on handshake completion.
    ///
    /// Useful for reducing latency, but vulnerable to replay attacks. Call
    /// [`SslRef::set_connect_state`] first.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_write_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn write_early_data(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let mut written = 0;
        let ret = unsafe {
            ffi::SSL_write_early_data(
                self.ssl.as_ptr(),
                buf.as_ptr() as *const c_void,
                buf.len(),
                &mut written,
            )
        };
        if ret > 0 {
            Ok(written)
        } else {
            Err(self.make_error(ret))
        }
    }

    /// Initiates a client-side TLS handshake.
    ///
    /// # Warning
    ///
    /// OpenSSL's default configuration is insecure. It is highly recommended to use
    /// `SslConnector` rather than `Ssl` directly, as it manages that configuration.
    #[corresponds(SSL_connect)]
    pub fn connect(&mut self) -> Result<(), Error> {
        let ret = unsafe { ffi::SSL_connect(self.ssl.as_ptr()) };
        if ret > 0 {
            Ok(())
        } else {
            Err(self.make_error(ret))
        }
    }

    /// Initiates a server-side TLS handshake.
    ///
    /// # Warning
    ///
    /// OpenSSL's default configuration is insecure. It is highly recommended to use
    /// `SslAcceptor` rather than `Ssl` directly, as it manages that configuration.
    #[corresponds(SSL_accept)]
    pub fn accept(&mut self) -> Result<(), Error> {
        let ret = unsafe { ffi::SSL_accept(self.ssl.as_ptr()) };
        if ret > 0 {
            Ok(())
        } else {
            Err(self.make_error(ret))
        }
    }

    /// Initiates the handshake.
    ///
    /// This will fail if `set_accept_state` or `set_connect_state` was not called first.
    #[corresponds(SSL_do_handshake)]
    pub fn do_handshake(&mut self) -> Result<(), Error> {
        let ret = unsafe { ffi::SSL_do_handshake(self.ssl.as_ptr()) };
        if ret > 0 {
            Ok(())
        } else {
            Err(self.make_error(ret))
        }
    }

    /// Perform a stateless server-side handshake.
    ///
    /// Requires that cookie generation and verification callbacks were
    /// set on the SSL context.
    ///
    /// Returns `Ok(true)` if a complete ClientHello containing a valid cookie
    /// was read, in which case the handshake should be continued via
    /// `accept`. If a HelloRetryRequest containing a fresh cookie was
    /// transmitted, `Ok(false)` is returned instead. If the handshake cannot
    /// proceed at all, `Err` is returned.
    #[corresponds(SSL_stateless)]
    #[cfg(ossl111)]
    pub fn stateless(&mut self) -> Result<bool, ErrorStack> {
        match unsafe { ffi::SSL_stateless(self.ssl.as_ptr()) } {
            1 => Ok(true),
            0 => Ok(false),
            -1 => Err(ErrorStack::get()),
            _ => unreachable!(),
        }
    }

    /// Like `read`, but takes a possibly-uninitialized slice.
    ///
    /// # Safety
    ///
    /// No portion of `buf` will be de-initialized by this method. If the method returns `Ok(n)`,
    /// then the first `n` bytes of `buf` are guaranteed to be initialized.
    #[corresponds(SSL_read_ex)]
    pub fn read_uninit(&mut self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        loop {
            match self.ssl_read_uninit(buf) {
                Ok(n) => return Ok(n),
                Err(ref e) if e.code() == ErrorCode::ZERO_RETURN => return Ok(0),
                Err(ref e) if e.code() == ErrorCode::SYSCALL && e.io_error().is_none() => {
                    return Ok(0);
                }
                Err(ref e) if e.code() == ErrorCode::WANT_READ && e.io_error().is_none() => {}
                Err(e) => {
                    return Err(e
                        .into_io_error()
                        .unwrap_or_else(|e| io::Error::new(io::ErrorKind::Other, e)));
                }
            }
        }
    }

    /// Like `read`, but returns an `ssl::Error` rather than an `io::Error`.
    ///
    /// It is particularly useful with a non-blocking socket, where the error value will identify if
    /// OpenSSL is waiting on read or write readiness.
    #[corresponds(SSL_read_ex)]
    pub fn ssl_read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // SAFETY: `ssl_read_uninit` does not de-initialize the buffer.
        unsafe {
            self.ssl_read_uninit(util::from_raw_parts_mut(
                buf.as_mut_ptr().cast::<MaybeUninit<u8>>(),
                buf.len(),
            ))
        }
    }

    /// Like `read_ssl`, but takes a possibly-uninitialized slice.
    ///
    /// # Safety
    ///
    /// No portion of `buf` will be de-initialized by this method. If the method returns `Ok(n)`,
    /// then the first `n` bytes of `buf` are guaranteed to be initialized.
    #[corresponds(SSL_read_ex)]
    pub fn ssl_read_uninit(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<usize, Error> {
        cfg_if! {
            if #[cfg(any(ossl111, libressl350))] {
                let mut readbytes = 0;
                let ret = unsafe {
                    ffi::SSL_read_ex(
                        self.ssl().as_ptr(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        &mut readbytes,
                    )
                };

                if ret > 0 {
                    Ok(readbytes)
                } else {
                    Err(self.make_error(ret))
                }
            } else {
                if buf.is_empty() {
                    return Ok(0);
                }

                let len = usize::min(c_int::MAX as usize, buf.len()) as c_int;
                let ret = unsafe {
                    ffi::SSL_read(self.ssl().as_ptr(), buf.as_mut_ptr().cast(), len)
                };
                if ret > 0 {
                    Ok(ret as usize)
                } else {
                    Err(self.make_error(ret))
                }
            }
        }
    }

    /// Like `write`, but returns an `ssl::Error` rather than an `io::Error`.
    ///
    /// It is particularly useful with a non-blocking socket, where the error value will identify if
    /// OpenSSL is waiting on read or write readiness.
    #[corresponds(SSL_write_ex)]
    pub fn ssl_write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        cfg_if! {
            if #[cfg(any(ossl111, libressl350))] {
                let mut written = 0;
                let ret = unsafe {
                    ffi::SSL_write_ex(
                        self.ssl().as_ptr(),
                        buf.as_ptr().cast(),
                        buf.len(),
                        &mut written,
                    )
                };

                if ret > 0 {
                    Ok(written)
                } else {
                    Err(self.make_error(ret))
                }
            } else {
                if buf.is_empty() {
                    return Ok(0);
                }

                let len = usize::min(c_int::MAX as usize, buf.len()) as c_int;
                let ret = unsafe {
                    ffi::SSL_write(self.ssl().as_ptr(), buf.as_ptr().cast(), len)
                };
                if ret > 0 {
                    Ok(ret as usize)
                } else {
                    Err(self.make_error(ret))
                }
            }
        }
    }

    /// Reads data from the stream, without removing it from the queue.
    #[corresponds(SSL_peek_ex)]
    pub fn ssl_peek(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        cfg_if! {
            if #[cfg(any(ossl111, libressl350))] {
                let mut readbytes = 0;
                let ret = unsafe {
                    ffi::SSL_peek_ex(
                        self.ssl().as_ptr(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        &mut readbytes,
                    )
                };

                if ret > 0 {
                    Ok(readbytes)
                } else {
                    Err(self.make_error(ret))
                }
            } else {
                if buf.is_empty() {
                    return Ok(0);
                }

                let len = usize::min(c_int::MAX as usize, buf.len()) as c_int;
                let ret = unsafe {
                    ffi::SSL_peek(self.ssl().as_ptr(), buf.as_mut_ptr().cast(), len)
                };
                if ret > 0 {
                    Ok(ret as usize)
                } else {
                    Err(self.make_error(ret))
                }
            }
        }
    }

    /// Shuts down the session.
    ///
    /// The shutdown process consists of two steps. The first step sends a close notify message to
    /// the peer, after which `ShutdownResult::Sent` is returned. The second step awaits the receipt
    /// of a close notify message from the peer, after which `ShutdownResult::Received` is returned.
    ///
    /// While the connection may be closed after the first step, it is recommended to fully shut the
    /// session down. In particular, it must be fully shut down if the connection is to be used for
    /// further communication in the future.
    #[corresponds(SSL_shutdown)]
    pub fn shutdown(&mut self) -> Result<ShutdownResult, Error> {
        match unsafe { ffi::SSL_shutdown(self.ssl.as_ptr()) } {
            0 => Ok(ShutdownResult::Sent),
            1 => Ok(ShutdownResult::Received),
            n => Err(self.make_error(n)),
        }
    }

    /// Returns the session's shutdown state.
    #[corresponds(SSL_get_shutdown)]
    pub fn get_shutdown(&mut self) -> ShutdownState {
        unsafe {
            let bits = ffi::SSL_get_shutdown(self.ssl.as_ptr());
            ShutdownState::from_bits_retain(bits)
        }
    }

    /// Sets the session's shutdown state.
    ///
    /// This can be used to tell OpenSSL that the session should be cached even if a full two-way
    /// shutdown was not completed.
    #[corresponds(SSL_set_shutdown)]
    pub fn set_shutdown(&mut self, state: ShutdownState) {
        unsafe { ffi::SSL_set_shutdown(self.ssl.as_ptr(), state.bits()) }
    }
}

impl<S> SslStream<S> {
    fn make_error(&mut self, ret: c_int) -> Error {
        self.check_panic();

        let code = self.ssl.get_error(ret);

        let cause = match code {
            ErrorCode::SSL => Some(InnerError::Ssl(ErrorStack::get())),
            ErrorCode::SYSCALL => {
                let errs = ErrorStack::get();
                if errs.errors().is_empty() {
                    self.get_bio_error().map(InnerError::Io)
                } else {
                    Some(InnerError::Ssl(errs))
                }
            }
            ErrorCode::ZERO_RETURN => None,
            ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                self.get_bio_error().map(InnerError::Io)
            }
            _ => None,
        };

        Error { code, cause }
    }

    fn check_panic(&mut self) {
        if let Some(err) = unsafe { bio::take_panic::<S>(self.ssl.get_raw_rbio()) } {
            resume_unwind(err)
        }
    }

    fn get_bio_error(&mut self) -> Option<io::Error> {
        unsafe { bio::take_error::<S>(self.ssl.get_raw_rbio()) }
    }

    /// Returns a shared reference to the underlying stream.
    pub fn get_ref(&self) -> &S {
        unsafe {
            let bio = self.ssl.get_raw_rbio();
            bio::get_ref(bio)
        }
    }

    /// Returns a mutable reference to the underlying stream.
    ///
    /// # Warning
    ///
    /// It is inadvisable to read from or write to the underlying stream as it
    /// will most likely corrupt the SSL session.
    pub fn get_mut(&mut self) -> &mut S {
        unsafe {
            let bio = self.ssl.get_raw_rbio();
            bio::get_mut(bio)
        }
    }

    /// Returns a shared reference to the `Ssl` object associated with this stream.
    pub fn ssl(&self) -> &SslRef {
        &self.ssl
    }
}

impl<S: Read + Write> Read for SslStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: `read_uninit` does not de-initialize the buffer
        unsafe {
            self.read_uninit(util::from_raw_parts_mut(
                buf.as_mut_ptr().cast::<MaybeUninit<u8>>(),
                buf.len(),
            ))
        }
    }
}

impl<S: Read + Write> Write for SslStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            match self.ssl_write(buf) {
                Ok(n) => return Ok(n),
                Err(ref e) if e.code() == ErrorCode::WANT_READ && e.io_error().is_none() => {}
                Err(e) => {
                    return Err(e
                        .into_io_error()
                        .unwrap_or_else(|e| io::Error::new(io::ErrorKind::Other, e)));
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A partially constructed `SslStream`, useful for unusual handshakes.
#[deprecated(
    since = "0.10.32",
    note = "use the methods directly on Ssl/SslStream instead"
)]
pub struct SslStreamBuilder<S> {
    inner: SslStream<S>,
}

#[allow(deprecated)]
impl<S> SslStreamBuilder<S>
where
    S: Read + Write,
{
    /// Begin creating an `SslStream` atop `stream`
    pub fn new(ssl: Ssl, stream: S) -> Self {
        Self {
            inner: SslStream::new(ssl, stream).unwrap(),
        }
    }

    /// Perform a stateless server-side handshake
    ///
    /// Requires that cookie generation and verification callbacks were
    /// set on the SSL context.
    ///
    /// Returns `Ok(true)` if a complete ClientHello containing a valid cookie
    /// was read, in which case the handshake should be continued via
    /// `accept`. If a HelloRetryRequest containing a fresh cookie was
    /// transmitted, `Ok(false)` is returned instead. If the handshake cannot
    /// proceed at all, `Err` is returned.
    #[corresponds(SSL_stateless)]
    #[cfg(ossl111)]
    pub fn stateless(&mut self) -> Result<bool, ErrorStack> {
        match unsafe { ffi::SSL_stateless(self.inner.ssl.as_ptr()) } {
            1 => Ok(true),
            0 => Ok(false),
            -1 => Err(ErrorStack::get()),
            _ => unreachable!(),
        }
    }

    /// Configure as an outgoing stream from a client.
    #[corresponds(SSL_set_connect_state)]
    pub fn set_connect_state(&mut self) {
        unsafe { ffi::SSL_set_connect_state(self.inner.ssl.as_ptr()) }
    }

    /// Configure as an incoming stream to a server.
    #[corresponds(SSL_set_accept_state)]
    pub fn set_accept_state(&mut self) {
        unsafe { ffi::SSL_set_accept_state(self.inner.ssl.as_ptr()) }
    }

    /// See `Ssl::connect`
    pub fn connect(mut self) -> Result<SslStream<S>, HandshakeError<S>> {
        match self.inner.connect() {
            Ok(()) => Ok(self.inner),
            Err(error) => match error.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                    Err(HandshakeError::WouldBlock(MidHandshakeSslStream {
                        stream: self.inner,
                        error,
                    }))
                }
                _ => Err(HandshakeError::Failure(MidHandshakeSslStream {
                    stream: self.inner,
                    error,
                })),
            },
        }
    }

    /// See `Ssl::accept`
    pub fn accept(mut self) -> Result<SslStream<S>, HandshakeError<S>> {
        match self.inner.accept() {
            Ok(()) => Ok(self.inner),
            Err(error) => match error.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                    Err(HandshakeError::WouldBlock(MidHandshakeSslStream {
                        stream: self.inner,
                        error,
                    }))
                }
                _ => Err(HandshakeError::Failure(MidHandshakeSslStream {
                    stream: self.inner,
                    error,
                })),
            },
        }
    }

    /// Initiates the handshake.
    ///
    /// This will fail if `set_accept_state` or `set_connect_state` was not called first.
    #[corresponds(SSL_do_handshake)]
    pub fn handshake(mut self) -> Result<SslStream<S>, HandshakeError<S>> {
        match self.inner.do_handshake() {
            Ok(()) => Ok(self.inner),
            Err(error) => match error.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                    Err(HandshakeError::WouldBlock(MidHandshakeSslStream {
                        stream: self.inner,
                        error,
                    }))
                }
                _ => Err(HandshakeError::Failure(MidHandshakeSslStream {
                    stream: self.inner,
                    error,
                })),
            },
        }
    }

    /// Read application data transmitted by a client before handshake
    /// completion.
    ///
    /// Useful for reducing latency, but vulnerable to replay attacks. Call
    /// `set_accept_state` first.
    ///
    /// Returns `Ok(0)` if all early data has been read.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_read_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn read_early_data(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.inner.read_early_data(buf)
    }

    /// Send data to the server without blocking on handshake completion.
    ///
    /// Useful for reducing latency, but vulnerable to replay attacks. Call
    /// `set_connect_state` first.
    ///
    /// Requires OpenSSL 1.1.1 or LibreSSL 3.4.0 or newer.
    #[corresponds(SSL_write_early_data)]
    #[cfg(any(ossl111, libressl340))]
    pub fn write_early_data(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.inner.write_early_data(buf)
    }
}

#[allow(deprecated)]
impl<S> SslStreamBuilder<S> {
    /// Returns a shared reference to the underlying stream.
    pub fn get_ref(&self) -> &S {
        unsafe {
            let bio = self.inner.ssl.get_raw_rbio();
            bio::get_ref(bio)
        }
    }

    /// Returns a mutable reference to the underlying stream.
    ///
    /// # Warning
    ///
    /// It is inadvisable to read from or write to the underlying stream as it
    /// will most likely corrupt the SSL session.
    pub fn get_mut(&mut self) -> &mut S {
        unsafe {
            let bio = self.inner.ssl.get_raw_rbio();
            bio::get_mut(bio)
        }
    }

    /// Returns a shared reference to the `Ssl` object associated with this builder.
    pub fn ssl(&self) -> &SslRef {
        &self.inner.ssl
    }

    /// Set the DTLS MTU size.
    ///
    /// It will be ignored if the value is smaller than the minimum packet size
    /// the DTLS protocol requires.
    ///
    /// # Panics
    /// This function panics if the given mtu size can't be represented in a positive `c_long` range
    #[deprecated(note = "Use SslRef::set_mtu instead", since = "0.10.30")]
    pub fn set_dtls_mtu_size(&mut self, mtu_size: usize) {
        unsafe {
            let bio = self.inner.ssl.get_raw_rbio();
            bio::set_dtls_mtu_size::<S>(bio, mtu_size);
        }
    }
}

/// The result of a shutdown request.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShutdownResult {
    /// A close notify message has been sent to the peer.
    Sent,

    /// A close notify response message has been received from the peer.
    Received,
}

bitflags! {
    /// The shutdown state of a session.
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct ShutdownState: c_int {
        /// A close notify message has been sent to the peer.
        const SENT = ffi::SSL_SENT_SHUTDOWN;
        /// A close notify message has been received from the peer.
        const RECEIVED = ffi::SSL_RECEIVED_SHUTDOWN;
    }
}

cfg_if! {
    if #[cfg(any(boringssl, ossl110, libressl273, awslc))] {
        use ffi::{SSL_CTX_up_ref, SSL_SESSION_get_master_key, SSL_SESSION_up_ref, SSL_is_server};
    } else {
        #[allow(bad_style)]
        pub unsafe fn SSL_CTX_up_ref(ssl: *mut ffi::SSL_CTX) -> c_int {
            ffi::CRYPTO_add_lock(
                &mut (*ssl).references,
                1,
                ffi::CRYPTO_LOCK_SSL_CTX,
                "mod.rs\0".as_ptr() as *const _,
                line!() as c_int,
            );
            0
        }

        #[allow(bad_style)]
        pub unsafe fn SSL_SESSION_get_master_key(
            session: *const ffi::SSL_SESSION,
            out: *mut c_uchar,
            mut outlen: usize,
        ) -> usize {
            if outlen == 0 {
                return (*session).master_key_length as usize;
            }
            if outlen > (*session).master_key_length as usize {
                outlen = (*session).master_key_length as usize;
            }
            ptr::copy_nonoverlapping((*session).master_key.as_ptr(), out, outlen);
            outlen
        }

        #[allow(bad_style)]
        pub unsafe fn SSL_is_server(s: *mut ffi::SSL) -> c_int {
            (*s).server
        }

        #[allow(bad_style)]
        pub unsafe fn SSL_SESSION_up_ref(ses: *mut ffi::SSL_SESSION) -> c_int {
            ffi::CRYPTO_add_lock(
                &mut (*ses).references,
                1,
                ffi::CRYPTO_LOCK_SSL_CTX,
                "mod.rs\0".as_ptr() as *const _,
                line!() as c_int,
            );
            0
        }
    }
}

cfg_if! {
    if #[cfg(ossl300)] {
        use ffi::SSL_get1_peer_certificate;
    } else {
        use ffi::SSL_get_peer_certificate as SSL_get1_peer_certificate;
    }
}
cfg_if! {
    if #[cfg(any(boringssl, ossl110, libressl291, awslc))] {
        use ffi::{TLS_method, DTLS_method, TLS_client_method, TLS_server_method, DTLS_server_method, DTLS_client_method};
    } else {
        use ffi::{
            SSLv23_method as TLS_method, DTLSv1_method as DTLS_method, SSLv23_client_method as TLS_client_method,
            SSLv23_server_method as TLS_server_method,
        };
    }
}
cfg_if! {
    if #[cfg(ossl110)] {
        unsafe fn get_new_idx(f: ffi::CRYPTO_EX_free) -> c_int {
            ffi::CRYPTO_get_ex_new_index(
                ffi::CRYPTO_EX_INDEX_SSL_CTX,
                0,
                ptr::null_mut(),
                None,
                None,
                Some(f),
            )
        }

        unsafe fn get_new_ssl_idx(f: ffi::CRYPTO_EX_free) -> c_int {
            ffi::CRYPTO_get_ex_new_index(
                ffi::CRYPTO_EX_INDEX_SSL,
                0,
                ptr::null_mut(),
                None,
                None,
                Some(f),
            )
        }
    } else {
        use std::sync::Once;

        unsafe fn get_new_idx(f: ffi::CRYPTO_EX_free) -> c_int {
            // hack around https://rt.openssl.org/Ticket/Display.html?id=3710&user=guest&pass=guest
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                cfg_if! {
                    if #[cfg(not(any(boringssl, awslc)))] {
                        ffi::SSL_CTX_get_ex_new_index(0, ptr::null_mut(), None, None, None);
                    } else {
                        ffi::SSL_CTX_get_ex_new_index(0, ptr::null_mut(), ptr::null_mut(), None, None);
                    }
                }
            });

            cfg_if! {
                if #[cfg(not(any(boringssl, awslc)))] {
                    ffi::SSL_CTX_get_ex_new_index(0, ptr::null_mut(), None, None, Some(f))
                } else {
                    ffi::SSL_CTX_get_ex_new_index(0, ptr::null_mut(), ptr::null_mut(), None, f)
                }
            }
        }

        unsafe fn get_new_ssl_idx(f: ffi::CRYPTO_EX_free) -> c_int {
            // hack around https://rt.openssl.org/Ticket/Display.html?id=3710&user=guest&pass=guest
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                #[cfg(not(any(boringssl, awslc)))]
                ffi::SSL_get_ex_new_index(0, ptr::null_mut(), None, None, None);
                #[cfg(any(boringssl, awslc))]
                ffi::SSL_get_ex_new_index(0, ptr::null_mut(), ptr::null_mut(), None, None);
            });

            #[cfg(not(any(boringssl, awslc)))]
            return ffi::SSL_get_ex_new_index(0, ptr::null_mut(), None, None, Some(f));
            #[cfg(any(boringssl, awslc))]
            return ffi::SSL_get_ex_new_index(0, ptr::null_mut(), ptr::null_mut(), None, f);
        }
    }
}
