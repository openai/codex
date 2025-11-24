//! Describe a context in which to verify an `X509` certificate.
//!
//! The `X509` certificate store holds trusted CA certificates used to verify
//! peer certificates.
//!
//! # Example
//!
//! ```rust
//! use openssl::x509::store::{X509StoreBuilder, X509Store};
//! use openssl::x509::{X509, X509Name};
//! use openssl::asn1::Asn1Time;
//! use openssl::pkey::PKey;
//! use openssl::hash::MessageDigest;
//! use openssl::rsa::Rsa;
//! use openssl::nid::Nid;
//!
//! let rsa = Rsa::generate(2048).unwrap();
//! let pkey = PKey::from_rsa(rsa).unwrap();
//!
//! let mut name = X509Name::builder().unwrap();
//! name.append_entry_by_nid(Nid::COMMONNAME, "foobar.com").unwrap();
//! let name = name.build();
//!
//! // Sep 27th, 2016
//! let sample_time = Asn1Time::from_unix(1474934400).unwrap();
//!
//! let mut builder = X509::builder().unwrap();
//! builder.set_version(2).unwrap();
//! builder.set_subject_name(&name).unwrap();
//! builder.set_issuer_name(&name).unwrap();
//! builder.set_pubkey(&pkey).unwrap();
//! builder.set_not_before(&sample_time);
//! builder.set_not_after(&sample_time);
//! builder.sign(&pkey, MessageDigest::sha256()).unwrap();
//!
//! let certificate: X509 = builder.build();
//!
//! let mut builder = X509StoreBuilder::new().unwrap();
//! let _ = builder.add_cert(certificate);
//!
//! let store: X509Store = builder.build();
//! ```

use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use std::mem;

use crate::error::ErrorStack;
#[cfg(not(any(boringssl, awslc)))]
use crate::ssl::SslFiletype;
#[cfg(ossl300)]
use crate::stack::Stack;
use crate::stack::StackRef;
use crate::util::ForeignTypeRefExt;
#[cfg(any(ossl102, boringssl, libressl261, awslc))]
use crate::x509::verify::{X509VerifyFlags, X509VerifyParamRef};
use crate::x509::{X509Object, X509PurposeId, X509};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;
#[cfg(not(any(boringssl, awslc)))]
use std::ffi::CString;
#[cfg(not(any(boringssl, awslc)))]
use std::path::Path;

foreign_type_and_impl_send_sync! {
    type CType = ffi::X509_STORE;
    fn drop = ffi::X509_STORE_free;

    /// A builder type used to construct an `X509Store`.
    pub struct X509StoreBuilder;
    /// A reference to an [`X509StoreBuilder`].
    pub struct X509StoreBuilderRef;
}

impl X509StoreBuilder {
    /// Returns a builder for a certificate store.
    ///
    /// The store is initially empty.
    #[corresponds(X509_STORE_new)]
    pub fn new() -> Result<X509StoreBuilder, ErrorStack> {
        unsafe {
            ffi::init();

            cvt_p(ffi::X509_STORE_new()).map(X509StoreBuilder)
        }
    }

    /// Constructs the `X509Store`.
    pub fn build(self) -> X509Store {
        let store = X509Store(self.0);
        mem::forget(self);
        store
    }
}

impl X509StoreBuilderRef {
    /// Adds a certificate to the certificate store.
    // FIXME should take an &X509Ref
    #[corresponds(X509_STORE_add_cert)]
    pub fn add_cert(&mut self, cert: X509) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::X509_STORE_add_cert(self.as_ptr(), cert.as_ptr())).map(|_| ()) }
    }

    /// Load certificates from their default locations.
    ///
    /// These locations are read from the `SSL_CERT_FILE` and `SSL_CERT_DIR`
    /// environment variables if present, or defaults specified at OpenSSL
    /// build time otherwise.
    #[corresponds(X509_STORE_set_default_paths)]
    pub fn set_default_paths(&mut self) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::X509_STORE_set_default_paths(self.as_ptr())).map(|_| ()) }
    }

    /// Adds a lookup method to the store.
    #[corresponds(X509_STORE_add_lookup)]
    pub fn add_lookup<T>(
        &mut self,
        method: &'static X509LookupMethodRef<T>,
    ) -> Result<&mut X509LookupRef<T>, ErrorStack> {
        let lookup = unsafe { ffi::X509_STORE_add_lookup(self.as_ptr(), method.as_ptr()) };
        cvt_p(lookup).map(|ptr| unsafe { X509LookupRef::from_ptr_mut(ptr) })
    }

    /// Sets certificate chain validation related flags.
    #[corresponds(X509_STORE_set_flags)]
    #[cfg(any(ossl102, boringssl, libressl261, awslc))]
    pub fn set_flags(&mut self, flags: X509VerifyFlags) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::X509_STORE_set_flags(self.as_ptr(), flags.bits())).map(|_| ()) }
    }

    /// Sets the certificate purpose.
    /// The purpose value can be obtained by `X509PurposeRef::get_by_sname()`
    #[corresponds(X509_STORE_set_purpose)]
    pub fn set_purpose(&mut self, purpose: X509PurposeId) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::X509_STORE_set_purpose(self.as_ptr(), purpose.as_raw())).map(|_| ()) }
    }

    /// Sets certificate chain validation related parameters.
    #[corresponds[X509_STORE_set1_param]]
    #[cfg(any(ossl102, boringssl, libressl261, awslc))]
    pub fn set_param(&mut self, param: &X509VerifyParamRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::X509_STORE_set1_param(self.as_ptr(), param.as_ptr())).map(|_| ()) }
    }
}

generic_foreign_type_and_impl_send_sync! {
    type CType = ffi::X509_LOOKUP;
    fn drop = ffi::X509_LOOKUP_free;

    /// Information used by an `X509Store` to look up certificates and CRLs.
    pub struct X509Lookup<T>;
    /// A reference to an [`X509Lookup`].
    pub struct X509LookupRef<T>;
}

/// Marker type corresponding to the [`X509_LOOKUP_hash_dir`] lookup method.
///
/// [`X509_LOOKUP_hash_dir`]: https://www.openssl.org/docs/manmaster/crypto/X509_LOOKUP_hash_dir.html
// FIXME should be an enum
pub struct HashDir;

impl X509Lookup<HashDir> {
    /// Lookup method that loads certificates and CRLs on demand and caches
    /// them in memory once they are loaded. It also checks for newer CRLs upon
    /// each lookup, so that newer CRLs are used as soon as they appear in the
    /// directory.
    #[corresponds(X509_LOOKUP_hash_dir)]
    pub fn hash_dir() -> &'static X509LookupMethodRef<HashDir> {
        unsafe { X509LookupMethodRef::from_const_ptr(ffi::X509_LOOKUP_hash_dir()) }
    }
}

#[cfg(not(any(boringssl, awslc)))]
impl X509LookupRef<HashDir> {
    /// Specifies a directory from which certificates and CRLs will be loaded
    /// on-demand. Must be used with `X509Lookup::hash_dir`.
    #[corresponds(X509_LOOKUP_add_dir)]
    pub fn add_dir(&mut self, name: &str, file_type: SslFiletype) -> Result<(), ErrorStack> {
        let name = CString::new(name).unwrap();
        unsafe {
            cvt(ffi::X509_LOOKUP_add_dir(
                self.as_ptr(),
                name.as_ptr(),
                file_type.as_raw(),
            ))
            .map(|_| ())
        }
    }
}

/// Marker type corresponding to the [`X509_LOOKUP_file`] lookup method.
///
/// [`X509_LOOKUP_file`]: https://www.openssl.org/docs/man1.1.1/man3/X509_LOOKUP_file.html
pub struct File;

impl X509Lookup<File> {
    /// Lookup method loads all the certificates or CRLs present in a file
    /// into memory at the time the file is added as a lookup source.
    #[corresponds(X509_LOOKUP_file)]
    pub fn file() -> &'static X509LookupMethodRef<File> {
        unsafe { X509LookupMethodRef::from_const_ptr(ffi::X509_LOOKUP_file()) }
    }
}

#[cfg(not(any(boringssl, awslc)))]
impl X509LookupRef<File> {
    /// Specifies a file from which certificates will be loaded
    #[corresponds(X509_load_cert_file)]
    // FIXME should return 'Result<i32, ErrorStack' like load_crl_file
    pub fn load_cert_file<P: AsRef<Path>>(
        &mut self,
        file: P,
        file_type: SslFiletype,
    ) -> Result<(), ErrorStack> {
        let file = CString::new(file.as_ref().as_os_str().to_str().unwrap()).unwrap();
        unsafe {
            cvt(ffi::X509_load_cert_file(
                self.as_ptr(),
                file.as_ptr(),
                file_type.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Specifies a file from which certificate revocation lists will be loaded
    #[corresponds(X509_load_crl_file)]
    pub fn load_crl_file<P: AsRef<Path>>(
        &mut self,
        file: P,
        file_type: SslFiletype,
    ) -> Result<i32, ErrorStack> {
        let file = CString::new(file.as_ref().as_os_str().to_str().unwrap()).unwrap();
        unsafe {
            cvt(ffi::X509_load_crl_file(
                self.as_ptr(),
                file.as_ptr(),
                file_type.as_raw(),
            ))
        }
    }
}

generic_foreign_type_and_impl_send_sync! {
    type CType = ffi::X509_LOOKUP_METHOD;
    fn drop = X509_LOOKUP_meth_free;

    /// Method used to look up certificates and CRLs.
    pub struct X509LookupMethod<T>;
    /// A reference to an [`X509LookupMethod`].
    pub struct X509LookupMethodRef<T>;
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::X509_STORE;
    fn drop = ffi::X509_STORE_free;

    /// A certificate store to hold trusted `X509` certificates.
    pub struct X509Store;
    /// Reference to an `X509Store`.
    pub struct X509StoreRef;
}

impl X509StoreRef {
    /// Get a reference to the cache of certificates in this store.
    ///
    /// This method is deprecated. It is **unsound** and will be removed in a
    /// future version of rust-openssl. `X509StoreRef::all_certificates`
    /// should be used instead.
    #[deprecated(
        note = "This method is unsound, and will be removed in a future version of rust-openssl. X509StoreRef::all_certificates should be used instead."
    )]
    #[corresponds(X509_STORE_get0_objects)]
    pub fn objects(&self) -> &StackRef<X509Object> {
        unsafe { StackRef::from_ptr(X509_STORE_get0_objects(self.as_ptr())) }
    }

    /// Returns a stack of all the certificates in this store.
    #[corresponds(X509_STORE_get1_all_certs)]
    #[cfg(ossl300)]
    pub fn all_certificates(&self) -> Stack<X509> {
        unsafe { Stack::from_ptr(ffi::X509_STORE_get1_all_certs(self.as_ptr())) }
    }
}

cfg_if! {
    if #[cfg(any(boringssl, ossl110, libressl270, awslc))] {
        use ffi::X509_STORE_get0_objects;
    } else {
        #[allow(bad_style)]
        unsafe fn X509_STORE_get0_objects(x: *mut ffi::X509_STORE) -> *mut ffi::stack_st_X509_OBJECT {
            (*x).objs
        }
    }
}

cfg_if! {
    if #[cfg(ossl110)] {
        use ffi::X509_LOOKUP_meth_free;
    } else {
        #[allow(bad_style)]
        unsafe fn X509_LOOKUP_meth_free(_x: *mut ffi::X509_LOOKUP_METHOD) {}
    }
}
