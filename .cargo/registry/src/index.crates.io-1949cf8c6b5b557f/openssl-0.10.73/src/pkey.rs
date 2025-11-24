//! Public/private key processing.
//!
//! Asymmetric public key algorithms solve the problem of establishing and sharing
//! secret keys to securely send and receive messages.
//! This system uses a pair of keys: a public key, which can be freely
//! distributed, and a private key, which is kept to oneself. An entity may
//! encrypt information using a user's public key. The encrypted information can
//! only be deciphered using that user's private key.
//!
//! This module offers support for five popular algorithms:
//!
//! * RSA
//!
//! * DSA
//!
//! * Diffie-Hellman
//!
//! * Elliptic Curves
//!
//! * HMAC
//!
//! These algorithms rely on hard mathematical problems - namely integer factorization,
//! discrete logarithms, and elliptic curve relationships - that currently do not
//! yield efficient solutions. This property ensures the security of these
//! cryptographic algorithms.
//!
//! # Example
//!
//! Generate a 2048-bit RSA public/private key pair and print the public key.
//!
//! ```rust
//! use openssl::rsa::Rsa;
//! use openssl::pkey::PKey;
//! use std::str;
//!
//! let rsa = Rsa::generate(2048).unwrap();
//! let pkey = PKey::from_rsa(rsa).unwrap();
//!
//! let pub_key: Vec<u8> = pkey.public_key_to_pem().unwrap();
//! println!("{:?}", str::from_utf8(pub_key.as_slice()).unwrap());
//! ```
#![allow(clippy::missing_safety_doc)]
use crate::bio::{MemBio, MemBioSlice};
#[cfg(ossl110)]
use crate::cipher::CipherRef;
use crate::dh::Dh;
use crate::dsa::Dsa;
use crate::ec::EcKey;
use crate::error::ErrorStack;
#[cfg(any(ossl110, boringssl, libressl370, awslc))]
use crate::pkey_ctx::PkeyCtx;
use crate::rsa::Rsa;
use crate::symm::Cipher;
use crate::util::{invoke_passwd_cb, CallbackState};
use crate::{cvt, cvt_p};
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::{c_int, c_long};
use openssl_macros::corresponds;
use std::convert::{TryFrom, TryInto};
use std::ffi::CString;
use std::fmt;
#[cfg(all(not(any(boringssl, awslc)), ossl110))]
use std::mem;
use std::ptr;

/// A tag type indicating that a key only has parameters.
pub enum Params {}

/// A tag type indicating that a key only has public components.
pub enum Public {}

/// A tag type indicating that a key has private components.
pub enum Private {}

/// An identifier of a kind of key.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Id(c_int);

impl Id {
    pub const RSA: Id = Id(ffi::EVP_PKEY_RSA);
    #[cfg(any(ossl111, libressl310, boringssl, awslc))]
    pub const RSA_PSS: Id = Id(ffi::EVP_PKEY_RSA_PSS);
    #[cfg(not(boringssl))]
    pub const HMAC: Id = Id(ffi::EVP_PKEY_HMAC);
    #[cfg(not(any(boringssl, awslc)))]
    pub const CMAC: Id = Id(ffi::EVP_PKEY_CMAC);
    pub const DSA: Id = Id(ffi::EVP_PKEY_DSA);
    pub const DH: Id = Id(ffi::EVP_PKEY_DH);
    #[cfg(ossl110)]
    pub const DHX: Id = Id(ffi::EVP_PKEY_DHX);
    pub const EC: Id = Id(ffi::EVP_PKEY_EC);
    #[cfg(ossl111)]
    pub const SM2: Id = Id(ffi::EVP_PKEY_SM2);

    #[cfg(any(ossl110, boringssl, libressl360, awslc))]
    pub const HKDF: Id = Id(ffi::EVP_PKEY_HKDF);

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub const ED25519: Id = Id(ffi::EVP_PKEY_ED25519);
    #[cfg(ossl111)]
    pub const ED448: Id = Id(ffi::EVP_PKEY_ED448);
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub const X25519: Id = Id(ffi::EVP_PKEY_X25519);
    #[cfg(ossl111)]
    pub const X448: Id = Id(ffi::EVP_PKEY_X448);
    #[cfg(ossl111)]
    pub const POLY1305: Id = Id(ffi::EVP_PKEY_POLY1305);

    /// Creates a `Id` from an integer representation.
    pub fn from_raw(value: c_int) -> Id {
        Id(value)
    }

    /// Returns the integer representation of the `Id`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

/// A trait indicating that a key has parameters.
pub unsafe trait HasParams {}

unsafe impl HasParams for Params {}

unsafe impl<T> HasParams for T where T: HasPublic {}

/// A trait indicating that a key has public components.
pub unsafe trait HasPublic {}

unsafe impl HasPublic for Public {}

unsafe impl<T> HasPublic for T where T: HasPrivate {}

/// A trait indicating that a key has private components.
pub unsafe trait HasPrivate {}

unsafe impl HasPrivate for Private {}

generic_foreign_type_and_impl_send_sync! {
    type CType = ffi::EVP_PKEY;
    fn drop = ffi::EVP_PKEY_free;

    /// A public or private key.
    pub struct PKey<T>;
    /// Reference to `PKey`.
    pub struct PKeyRef<T>;
}

impl<T> ToOwned for PKeyRef<T> {
    type Owned = PKey<T>;

    fn to_owned(&self) -> PKey<T> {
        unsafe {
            EVP_PKEY_up_ref(self.as_ptr());
            PKey::from_ptr(self.as_ptr())
        }
    }
}

impl<T> PKeyRef<T> {
    /// Returns a copy of the internal RSA key.
    #[corresponds(EVP_PKEY_get1_RSA)]
    pub fn rsa(&self) -> Result<Rsa<T>, ErrorStack> {
        unsafe {
            let rsa = cvt_p(ffi::EVP_PKEY_get1_RSA(self.as_ptr()))?;
            Ok(Rsa::from_ptr(rsa))
        }
    }

    /// Returns a copy of the internal DSA key.
    #[corresponds(EVP_PKEY_get1_DSA)]
    pub fn dsa(&self) -> Result<Dsa<T>, ErrorStack> {
        unsafe {
            let dsa = cvt_p(ffi::EVP_PKEY_get1_DSA(self.as_ptr()))?;
            Ok(Dsa::from_ptr(dsa))
        }
    }

    /// Returns a copy of the internal DH key.
    #[corresponds(EVP_PKEY_get1_DH)]
    pub fn dh(&self) -> Result<Dh<T>, ErrorStack> {
        unsafe {
            let dh = cvt_p(ffi::EVP_PKEY_get1_DH(self.as_ptr()))?;
            Ok(Dh::from_ptr(dh))
        }
    }

    /// Returns a copy of the internal elliptic curve key.
    #[corresponds(EVP_PKEY_get1_EC_KEY)]
    pub fn ec_key(&self) -> Result<EcKey<T>, ErrorStack> {
        unsafe {
            let ec_key = cvt_p(ffi::EVP_PKEY_get1_EC_KEY(self.as_ptr()))?;
            Ok(EcKey::from_ptr(ec_key))
        }
    }

    /// Returns the `Id` that represents the type of this key.
    #[corresponds(EVP_PKEY_id)]
    pub fn id(&self) -> Id {
        unsafe { Id::from_raw(ffi::EVP_PKEY_id(self.as_ptr())) }
    }

    /// Returns the maximum size of a signature in bytes.
    #[corresponds(EVP_PKEY_size)]
    pub fn size(&self) -> usize {
        unsafe { ffi::EVP_PKEY_size(self.as_ptr()) as usize }
    }
}

impl<T> PKeyRef<T>
where
    T: HasPublic,
{
    to_pem! {
        /// Serializes the public key into a PEM-encoded SubjectPublicKeyInfo structure.
        ///
        /// The output will have a header of `-----BEGIN PUBLIC KEY-----`.
        #[corresponds(PEM_write_bio_PUBKEY)]
        public_key_to_pem,
        ffi::PEM_write_bio_PUBKEY
    }

    to_der! {
        /// Serializes the public key into a DER-encoded SubjectPublicKeyInfo structure.
        #[corresponds(i2d_PUBKEY)]
        public_key_to_der,
        ffi::i2d_PUBKEY
    }

    /// Returns the size of the key.
    ///
    /// This corresponds to the bit length of the modulus of an RSA key, and the bit length of the
    /// group order for an elliptic curve key, for example.
    #[corresponds(EVP_PKEY_bits)]
    pub fn bits(&self) -> u32 {
        unsafe { ffi::EVP_PKEY_bits(self.as_ptr()) as u32 }
    }

    ///Returns the number of security bits.
    ///
    ///Bits of security is defined in NIST SP800-57.
    #[corresponds(EVP_PKEY_security_bits)]
    #[cfg(any(ossl110, libressl360))]
    pub fn security_bits(&self) -> u32 {
        unsafe { ffi::EVP_PKEY_security_bits(self.as_ptr()) as u32 }
    }

    /// Compares the public component of this key with another.
    #[corresponds(EVP_PKEY_cmp)]
    pub fn public_eq<U>(&self, other: &PKeyRef<U>) -> bool
    where
        U: HasPublic,
    {
        let res = unsafe { ffi::EVP_PKEY_cmp(self.as_ptr(), other.as_ptr()) == 1 };
        // Clear the stack. OpenSSL will put an error on the stack when the
        // keys are different types in some situations.
        let _ = ErrorStack::get();
        res
    }

    /// Raw byte representation of a public key.
    ///
    /// This function only works for algorithms that support raw public keys.
    /// Currently this is: [`Id::X25519`], [`Id::ED25519`], [`Id::X448`] or [`Id::ED448`].
    #[corresponds(EVP_PKEY_get_raw_public_key)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn raw_public_key(&self) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let mut len = 0;
            cvt(ffi::EVP_PKEY_get_raw_public_key(
                self.as_ptr(),
                ptr::null_mut(),
                &mut len,
            ))?;
            let mut buf = vec![0u8; len];
            cvt(ffi::EVP_PKEY_get_raw_public_key(
                self.as_ptr(),
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            buf.truncate(len);
            Ok(buf)
        }
    }
}

impl<T> PKeyRef<T>
where
    T: HasPrivate,
{
    private_key_to_pem! {
        /// Serializes the private key to a PEM-encoded PKCS#8 PrivateKeyInfo structure.
        ///
        /// The output will have a header of `-----BEGIN PRIVATE KEY-----`.
        #[corresponds(PEM_write_bio_PKCS8PrivateKey)]
        private_key_to_pem_pkcs8,
        /// Serializes the private key to a PEM-encoded PKCS#8 EncryptedPrivateKeyInfo structure.
        ///
        /// The output will have a header of `-----BEGIN ENCRYPTED PRIVATE KEY-----`.
        #[corresponds(PEM_write_bio_PKCS8PrivateKey)]
        private_key_to_pem_pkcs8_passphrase,
        ffi::PEM_write_bio_PKCS8PrivateKey
    }

    to_der! {
        /// Serializes the private key to a DER-encoded key type specific format.
        #[corresponds(i2d_PrivateKey)]
        private_key_to_der,
        ffi::i2d_PrivateKey
    }

    /// Raw byte representation of a private key.
    ///
    /// This function only works for algorithms that support raw private keys.
    /// Currently this is: [`Id::HMAC`], [`Id::X25519`], [`Id::ED25519`], [`Id::X448`] or [`Id::ED448`].
    #[corresponds(EVP_PKEY_get_raw_private_key)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn raw_private_key(&self) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let mut len = 0;
            cvt(ffi::EVP_PKEY_get_raw_private_key(
                self.as_ptr(),
                ptr::null_mut(),
                &mut len,
            ))?;
            let mut buf = vec![0u8; len];
            cvt(ffi::EVP_PKEY_get_raw_private_key(
                self.as_ptr(),
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            buf.truncate(len);
            Ok(buf)
        }
    }

    /// Serializes a private key into an unencrypted DER-formatted PKCS#8
    #[corresponds(i2d_PKCS8PrivateKey_bio)]
    pub fn private_key_to_pkcs8(&self) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let bio = MemBio::new()?;
            cvt(ffi::i2d_PKCS8PrivateKey_bio(
                bio.as_ptr(),
                self.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                0,
                None,
                ptr::null_mut(),
            ))?;

            Ok(bio.get_buf().to_owned())
        }
    }

    /// Serializes a private key into a DER-formatted PKCS#8, using the supplied password to
    /// encrypt the key.
    #[corresponds(i2d_PKCS8PrivateKey_bio)]
    pub fn private_key_to_pkcs8_passphrase(
        &self,
        cipher: Cipher,
        passphrase: &[u8],
    ) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let bio = MemBio::new()?;
            cvt(ffi::i2d_PKCS8PrivateKey_bio(
                bio.as_ptr(),
                self.as_ptr(),
                cipher.as_ptr(),
                passphrase.as_ptr() as *const _ as *mut _,
                passphrase.len().try_into().unwrap(),
                None,
                ptr::null_mut(),
            ))?;

            Ok(bio.get_buf().to_owned())
        }
    }
}

impl<T> fmt::Debug for PKey<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alg = match self.id() {
            Id::RSA => "RSA",
            #[cfg(any(ossl111, libressl310, boringssl, awslc))]
            Id::RSA_PSS => "RSA-PSS",
            #[cfg(not(boringssl))]
            Id::HMAC => "HMAC",
            #[cfg(not(any(boringssl, awslc)))]
            Id::CMAC => "CMAC",
            Id::DSA => "DSA",
            Id::DH => "DH",
            #[cfg(ossl110)]
            Id::DHX => "DHX",
            Id::EC => "EC",
            #[cfg(ossl111)]
            Id::SM2 => "SM2",
            #[cfg(any(ossl110, boringssl, libressl360, awslc))]
            Id::HKDF => "HKDF",
            #[cfg(any(ossl111, boringssl, libressl370, awslc))]
            Id::ED25519 => "Ed25519",
            #[cfg(ossl111)]
            Id::ED448 => "Ed448",
            #[cfg(any(ossl111, boringssl, libressl370, awslc))]
            Id::X25519 => "X25519",
            #[cfg(ossl111)]
            Id::X448 => "X448",
            #[cfg(ossl111)]
            Id::POLY1305 => "POLY1305",
            _ => "unknown",
        };
        fmt.debug_struct("PKey").field("algorithm", &alg).finish()
        // TODO: Print details for each specific type of key
    }
}

impl<T> Clone for PKey<T> {
    fn clone(&self) -> PKey<T> {
        PKeyRef::to_owned(self)
    }
}

impl<T> PKey<T> {
    /// Creates a new `PKey` containing an RSA key.
    #[corresponds(EVP_PKEY_set1_RSA)]
    pub fn from_rsa(rsa: Rsa<T>) -> Result<PKey<T>, ErrorStack> {
        // TODO: Next time we make backwards incompatible changes, this could
        // become an `&RsaRef<T>`. Same for all the other `from_*` methods.
        unsafe {
            let evp = cvt_p(ffi::EVP_PKEY_new())?;
            let pkey = PKey::from_ptr(evp);
            cvt(ffi::EVP_PKEY_set1_RSA(pkey.0, rsa.as_ptr()))?;
            Ok(pkey)
        }
    }

    /// Creates a new `PKey` containing a DSA key.
    #[corresponds(EVP_PKEY_set1_DSA)]
    pub fn from_dsa(dsa: Dsa<T>) -> Result<PKey<T>, ErrorStack> {
        unsafe {
            let evp = cvt_p(ffi::EVP_PKEY_new())?;
            let pkey = PKey::from_ptr(evp);
            cvt(ffi::EVP_PKEY_set1_DSA(pkey.0, dsa.as_ptr()))?;
            Ok(pkey)
        }
    }

    /// Creates a new `PKey` containing a Diffie-Hellman key.
    #[corresponds(EVP_PKEY_set1_DH)]
    #[cfg(not(boringssl))]
    pub fn from_dh(dh: Dh<T>) -> Result<PKey<T>, ErrorStack> {
        unsafe {
            let evp = cvt_p(ffi::EVP_PKEY_new())?;
            let pkey = PKey::from_ptr(evp);
            cvt(ffi::EVP_PKEY_set1_DH(pkey.0, dh.as_ptr()))?;
            Ok(pkey)
        }
    }

    /// Creates a new `PKey` containing a Diffie-Hellman key with type DHX.
    #[cfg(all(not(any(boringssl, awslc)), ossl110))]
    pub fn from_dhx(dh: Dh<T>) -> Result<PKey<T>, ErrorStack> {
        unsafe {
            let evp = cvt_p(ffi::EVP_PKEY_new())?;
            let pkey = PKey::from_ptr(evp);
            cvt(ffi::EVP_PKEY_assign(
                pkey.0,
                ffi::EVP_PKEY_DHX,
                dh.as_ptr().cast(),
            ))?;
            mem::forget(dh);
            Ok(pkey)
        }
    }

    /// Creates a new `PKey` containing an elliptic curve key.
    #[corresponds(EVP_PKEY_set1_EC_KEY)]
    pub fn from_ec_key(ec_key: EcKey<T>) -> Result<PKey<T>, ErrorStack> {
        unsafe {
            let evp = cvt_p(ffi::EVP_PKEY_new())?;
            let pkey = PKey::from_ptr(evp);
            cvt(ffi::EVP_PKEY_set1_EC_KEY(pkey.0, ec_key.as_ptr()))?;
            Ok(pkey)
        }
    }
}

impl PKey<Private> {
    /// Creates a new `PKey` containing an HMAC key.
    ///
    /// # Note
    ///
    /// To compute HMAC values, use the `sign` module.
    #[corresponds(EVP_PKEY_new_mac_key)]
    #[cfg(not(boringssl))]
    pub fn hmac(key: &[u8]) -> Result<PKey<Private>, ErrorStack> {
        #[cfg(awslc)]
        let key_len = key.len();
        #[cfg(not(awslc))]
        let key_len = key.len() as c_int;
        unsafe {
            assert!(key.len() <= c_int::MAX as usize);
            let key = cvt_p(ffi::EVP_PKEY_new_mac_key(
                ffi::EVP_PKEY_HMAC,
                ptr::null_mut(),
                key.as_ptr() as *const _,
                key_len,
            ))?;
            Ok(PKey::from_ptr(key))
        }
    }

    /// Creates a new `PKey` containing a CMAC key.
    ///
    /// Requires OpenSSL 1.1.0 or newer.
    ///
    /// # Note
    ///
    /// To compute CMAC values, use the `sign` module.
    #[cfg(all(not(any(boringssl, awslc)), ossl110))]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn cmac(cipher: &Cipher, key: &[u8]) -> Result<PKey<Private>, ErrorStack> {
        let mut ctx = PkeyCtx::new_id(Id::CMAC)?;
        ctx.keygen_init()?;
        ctx.set_keygen_cipher(unsafe { CipherRef::from_ptr(cipher.as_ptr() as *mut _) })?;
        ctx.set_keygen_mac_key(key)?;
        ctx.keygen()
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    fn generate_eddsa(id: Id) -> Result<PKey<Private>, ErrorStack> {
        let mut ctx = PkeyCtx::new_id(id)?;
        ctx.keygen_init()?;
        ctx.keygen()
    }

    /// Generates a new private X25519 key.
    ///
    /// To import a private key from raw bytes see [`PKey::private_key_from_raw_bytes`].
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::pkey::{PKey, Id};
    /// use openssl::derive::Deriver;
    ///
    /// let public = // ...
    /// # &PKey::generate_x25519()?.raw_public_key()?;
    /// let public_key = PKey::public_key_from_raw_bytes(public, Id::X25519)?;
    ///
    /// let key = PKey::generate_x25519()?;
    /// let mut deriver = Deriver::new(&key)?;
    /// deriver.set_peer(&public_key)?;
    ///
    /// let secret = deriver.derive_to_vec()?;
    /// assert_eq!(secret.len(), 32);
    /// # Ok(()) }
    /// ```
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn generate_x25519() -> Result<PKey<Private>, ErrorStack> {
        PKey::generate_eddsa(Id::X25519)
    }

    /// Generates a new private X448 key.
    ///
    /// To import a private key from raw bytes see [`PKey::private_key_from_raw_bytes`].
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::pkey::{PKey, Id};
    /// use openssl::derive::Deriver;
    ///
    /// let public = // ...
    /// # &PKey::generate_x448()?.raw_public_key()?;
    /// let public_key = PKey::public_key_from_raw_bytes(public, Id::X448)?;
    ///
    /// let key = PKey::generate_x448()?;
    /// let mut deriver = Deriver::new(&key)?;
    /// deriver.set_peer(&public_key)?;
    ///
    /// let secret = deriver.derive_to_vec()?;
    /// assert_eq!(secret.len(), 56);
    /// # Ok(()) }
    /// ```
    #[cfg(ossl111)]
    pub fn generate_x448() -> Result<PKey<Private>, ErrorStack> {
        PKey::generate_eddsa(Id::X448)
    }

    /// Generates a new private Ed25519 key.
    ///
    /// To import a private key from raw bytes see [`PKey::private_key_from_raw_bytes`].
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::pkey::{PKey, Id};
    /// use openssl::sign::Signer;
    ///
    /// let key = PKey::generate_ed25519()?;
    /// let public_key = key.raw_public_key()?;
    ///
    /// let mut signer = Signer::new_without_digest(&key)?;
    /// let digest = // ...
    /// # &vec![0; 32];
    /// let signature = signer.sign_oneshot_to_vec(digest)?;
    /// assert_eq!(signature.len(), 64);
    /// # Ok(()) }
    /// ```
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn generate_ed25519() -> Result<PKey<Private>, ErrorStack> {
        PKey::generate_eddsa(Id::ED25519)
    }

    /// Generates a new private Ed448 key.
    ///
    /// To import a private key from raw bytes see [`PKey::private_key_from_raw_bytes`].
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::pkey::{PKey, Id};
    /// use openssl::sign::Signer;
    ///
    /// let key = PKey::generate_ed448()?;
    /// let public_key = key.raw_public_key()?;
    ///
    /// let mut signer = Signer::new_without_digest(&key)?;
    /// let digest = // ...
    /// # &vec![0; 32];
    /// let signature = signer.sign_oneshot_to_vec(digest)?;
    /// assert_eq!(signature.len(), 114);
    /// # Ok(()) }
    /// ```
    #[cfg(ossl111)]
    pub fn generate_ed448() -> Result<PKey<Private>, ErrorStack> {
        PKey::generate_eddsa(Id::ED448)
    }

    /// Generates a new EC key using the provided curve.
    ///
    /// Requires OpenSSL 3.0.0 or newer.
    #[corresponds(EVP_EC_gen)]
    #[cfg(ossl300)]
    pub fn ec_gen(curve: &str) -> Result<PKey<Private>, ErrorStack> {
        ffi::init();

        let curve = CString::new(curve).unwrap();
        unsafe {
            let ptr = cvt_p(ffi::EVP_EC_gen(curve.as_ptr()))?;
            Ok(PKey::from_ptr(ptr))
        }
    }

    private_key_from_pem! {
        /// Deserializes a private key from a PEM-encoded key type specific format.
        #[corresponds(PEM_read_bio_PrivateKey)]
        private_key_from_pem,

        /// Deserializes a private key from a PEM-encoded encrypted key type specific format.
        #[corresponds(PEM_read_bio_PrivateKey)]
        private_key_from_pem_passphrase,

        /// Deserializes a private key from a PEM-encoded encrypted key type specific format.
        ///
        /// The callback should fill the password into the provided buffer and return its length.
        #[corresponds(PEM_read_bio_PrivateKey)]
        private_key_from_pem_callback,
        PKey<Private>,
        ffi::PEM_read_bio_PrivateKey
    }

    from_der! {
        /// Decodes a DER-encoded private key.
        ///
        /// This function will attempt to automatically detect the underlying key format, and
        /// supports the unencrypted PKCS#8 PrivateKeyInfo structures as well as key type specific
        /// formats.
        #[corresponds(d2i_AutoPrivateKey)]
        private_key_from_der,
        PKey<Private>,
        ffi::d2i_AutoPrivateKey
    }

    /// Deserializes a DER-formatted PKCS#8 unencrypted private key.
    ///
    /// This method is mainly for interoperability reasons. Encrypted keyfiles should be preferred.
    pub fn private_key_from_pkcs8(der: &[u8]) -> Result<PKey<Private>, ErrorStack> {
        unsafe {
            ffi::init();
            let len = der.len().min(c_long::MAX as usize) as c_long;
            let p8inf = cvt_p(ffi::d2i_PKCS8_PRIV_KEY_INFO(
                ptr::null_mut(),
                &mut der.as_ptr(),
                len,
            ))?;
            let res = cvt_p(ffi::EVP_PKCS82PKEY(p8inf)).map(|p| PKey::from_ptr(p));
            ffi::PKCS8_PRIV_KEY_INFO_free(p8inf);
            res
        }
    }

    /// Deserializes a DER-formatted PKCS#8 private key, using a callback to retrieve the password
    /// if the key is encrypted.
    ///
    /// The callback should copy the password into the provided buffer and return the number of
    /// bytes written.
    #[corresponds(d2i_PKCS8PrivateKey_bio)]
    pub fn private_key_from_pkcs8_callback<F>(
        der: &[u8],
        callback: F,
    ) -> Result<PKey<Private>, ErrorStack>
    where
        F: FnOnce(&mut [u8]) -> Result<usize, ErrorStack>,
    {
        unsafe {
            ffi::init();
            let mut cb = CallbackState::new(callback);
            let bio = MemBioSlice::new(der)?;
            cvt_p(ffi::d2i_PKCS8PrivateKey_bio(
                bio.as_ptr(),
                ptr::null_mut(),
                Some(invoke_passwd_cb::<F>),
                &mut cb as *mut _ as *mut _,
            ))
            .map(|p| PKey::from_ptr(p))
        }
    }

    /// Deserializes a DER-formatted PKCS#8 private key, using the supplied password if the key is
    /// encrypted.
    ///
    /// # Panics
    ///
    /// Panics if `passphrase` contains an embedded null.
    #[corresponds(d2i_PKCS8PrivateKey_bio)]
    pub fn private_key_from_pkcs8_passphrase(
        der: &[u8],
        passphrase: &[u8],
    ) -> Result<PKey<Private>, ErrorStack> {
        unsafe {
            ffi::init();
            let bio = MemBioSlice::new(der)?;
            let passphrase = CString::new(passphrase).unwrap();
            cvt_p(ffi::d2i_PKCS8PrivateKey_bio(
                bio.as_ptr(),
                ptr::null_mut(),
                None,
                passphrase.as_ptr() as *const _ as *mut _,
            ))
            .map(|p| PKey::from_ptr(p))
        }
    }

    /// Creates a private key from its raw byte representation
    ///
    /// Algorithm types that support raw private keys are HMAC, X25519, ED25519, X448 or ED448
    #[corresponds(EVP_PKEY_new_raw_private_key)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn private_key_from_raw_bytes(
        bytes: &[u8],
        key_type: Id,
    ) -> Result<PKey<Private>, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::EVP_PKEY_new_raw_private_key(
                key_type.as_raw(),
                ptr::null_mut(),
                bytes.as_ptr(),
                bytes.len(),
            ))
            .map(|p| PKey::from_ptr(p))
        }
    }
}

impl PKey<Public> {
    private_key_from_pem! {
        /// Decodes a PEM-encoded SubjectPublicKeyInfo structure.
        ///
        /// The input should have a header of `-----BEGIN PUBLIC KEY-----`.
        #[corresponds(PEM_read_bio_PUBKEY)]
        public_key_from_pem,

        /// Decodes a PEM-encoded SubjectPublicKeyInfo structure.
        #[corresponds(PEM_read_bio_PUBKEY)]
        public_key_from_pem_passphrase,

        /// Decodes a PEM-encoded SubjectPublicKeyInfo structure.
        ///
        /// The callback should fill the password into the provided buffer and return its length.
        #[corresponds(PEM_read_bio_PrivateKey)]
        public_key_from_pem_callback,
        PKey<Public>,
        ffi::PEM_read_bio_PUBKEY
    }

    from_der! {
        /// Decodes a DER-encoded SubjectPublicKeyInfo structure.
        #[corresponds(d2i_PUBKEY)]
        public_key_from_der,
        PKey<Public>,
        ffi::d2i_PUBKEY
    }

    /// Creates a public key from its raw byte representation
    ///
    /// Algorithm types that support raw public keys are X25519, ED25519, X448 or ED448
    #[corresponds(EVP_PKEY_new_raw_public_key)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn public_key_from_raw_bytes(
        bytes: &[u8],
        key_type: Id,
    ) -> Result<PKey<Public>, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::EVP_PKEY_new_raw_public_key(
                key_type.as_raw(),
                ptr::null_mut(),
                bytes.as_ptr(),
                bytes.len(),
            ))
            .map(|p| PKey::from_ptr(p))
        }
    }
}

cfg_if! {
    if #[cfg(any(boringssl, ossl110, libressl270, awslc))] {
        use ffi::EVP_PKEY_up_ref;
    } else {
        #[allow(bad_style)]
        unsafe extern "C" fn EVP_PKEY_up_ref(pkey: *mut ffi::EVP_PKEY) {
            ffi::CRYPTO_add_lock(
                &mut (*pkey).references,
                1,
                ffi::CRYPTO_LOCK_EVP_PKEY,
                "pkey.rs\0".as_ptr() as *const _,
                line!() as c_int,
            );
        }
    }
}

impl<T> TryFrom<EcKey<T>> for PKey<T> {
    type Error = ErrorStack;

    fn try_from(ec_key: EcKey<T>) -> Result<PKey<T>, ErrorStack> {
        PKey::from_ec_key(ec_key)
    }
}

impl<T> TryFrom<PKey<T>> for EcKey<T> {
    type Error = ErrorStack;

    fn try_from(pkey: PKey<T>) -> Result<EcKey<T>, ErrorStack> {
        pkey.ec_key()
    }
}

impl<T> TryFrom<Rsa<T>> for PKey<T> {
    type Error = ErrorStack;

    fn try_from(rsa: Rsa<T>) -> Result<PKey<T>, ErrorStack> {
        PKey::from_rsa(rsa)
    }
}

impl<T> TryFrom<PKey<T>> for Rsa<T> {
    type Error = ErrorStack;

    fn try_from(pkey: PKey<T>) -> Result<Rsa<T>, ErrorStack> {
        pkey.rsa()
    }
}

impl<T> TryFrom<Dsa<T>> for PKey<T> {
    type Error = ErrorStack;

    fn try_from(dsa: Dsa<T>) -> Result<PKey<T>, ErrorStack> {
        PKey::from_dsa(dsa)
    }
}

impl<T> TryFrom<PKey<T>> for Dsa<T> {
    type Error = ErrorStack;

    fn try_from(pkey: PKey<T>) -> Result<Dsa<T>, ErrorStack> {
        pkey.dsa()
    }
}

#[cfg(not(boringssl))]
impl<T> TryFrom<Dh<T>> for PKey<T> {
    type Error = ErrorStack;

    fn try_from(dh: Dh<T>) -> Result<PKey<T>, ErrorStack> {
        PKey::from_dh(dh)
    }
}

impl<T> TryFrom<PKey<T>> for Dh<T> {
    type Error = ErrorStack;

    fn try_from(pkey: PKey<T>) -> Result<Dh<T>, ErrorStack> {
        pkey.dh()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    #[cfg(not(boringssl))]
    use crate::dh::Dh;
    use crate::dsa::Dsa;
    use crate::ec::EcKey;
    use crate::error::Error;
    use crate::nid::Nid;
    use crate::rsa::Rsa;
    use crate::symm::Cipher;

    use super::*;

    #[cfg(any(ossl111, awslc))]
    use crate::rand::rand_bytes;

    #[test]
    fn test_to_password() {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();
        let pem = pkey
            .private_key_to_pem_pkcs8_passphrase(Cipher::aes_128_cbc(), b"foobar")
            .unwrap();
        PKey::private_key_from_pem_passphrase(&pem, b"foobar").unwrap();
        assert!(PKey::private_key_from_pem_passphrase(&pem, b"fizzbuzz").is_err());
    }

    #[test]
    fn test_unencrypted_pkcs8() {
        let key = include_bytes!("../test/pkcs8-nocrypt.der");
        let pkey = PKey::private_key_from_pkcs8(key).unwrap();
        let serialized = pkey.private_key_to_pkcs8().unwrap();
        let pkey2 = PKey::private_key_from_pkcs8(&serialized).unwrap();

        assert_eq!(
            pkey2.private_key_to_der().unwrap(),
            pkey.private_key_to_der().unwrap()
        );
    }

    #[test]
    fn test_encrypted_pkcs8_passphrase() {
        let key = include_bytes!("../test/pkcs8.der");
        PKey::private_key_from_pkcs8_passphrase(key, b"mypass").unwrap();

        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();
        let der = pkey
            .private_key_to_pkcs8_passphrase(Cipher::aes_128_cbc(), b"mypass")
            .unwrap();
        let pkey2 = PKey::private_key_from_pkcs8_passphrase(&der, b"mypass").unwrap();
        assert_eq!(
            pkey.private_key_to_der().unwrap(),
            pkey2.private_key_to_der().unwrap()
        );
    }

    #[test]
    fn test_encrypted_pkcs8_callback() {
        let mut password_queried = false;
        let key = include_bytes!("../test/pkcs8.der");
        PKey::private_key_from_pkcs8_callback(key, |password| {
            password_queried = true;
            password[..6].copy_from_slice(b"mypass");
            Ok(6)
        })
        .unwrap();
        assert!(password_queried);
    }

    #[test]
    fn test_private_key_from_pem() {
        let key = include_bytes!("../test/key.pem");
        PKey::private_key_from_pem(key).unwrap();
    }

    #[test]
    fn test_public_key_from_pem() {
        let key = include_bytes!("../test/key.pem.pub");
        PKey::public_key_from_pem(key).unwrap();
    }

    #[test]
    fn test_public_key_from_der() {
        let key = include_bytes!("../test/key.der.pub");
        PKey::public_key_from_der(key).unwrap();
    }

    #[test]
    fn test_private_key_from_der() {
        let key = include_bytes!("../test/key.der");
        PKey::private_key_from_der(key).unwrap();
    }

    #[test]
    fn test_pem() {
        let key = include_bytes!("../test/key.pem");
        let key = PKey::private_key_from_pem(key).unwrap();

        let priv_key = key.private_key_to_pem_pkcs8().unwrap();
        let pub_key = key.public_key_to_pem().unwrap();

        // As a super-simple verification, just check that the buffers contain
        // the `PRIVATE KEY` or `PUBLIC KEY` strings.
        assert!(priv_key.windows(11).any(|s| s == b"PRIVATE KEY"));
        assert!(pub_key.windows(10).any(|s| s == b"PUBLIC KEY"));
    }

    #[test]
    fn test_rsa_accessor() {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();
        pkey.rsa().unwrap();
        assert_eq!(pkey.id(), Id::RSA);
        assert!(pkey.dsa().is_err());
    }

    #[test]
    fn test_dsa_accessor() {
        let dsa = Dsa::generate(2048).unwrap();
        let pkey = PKey::from_dsa(dsa).unwrap();
        pkey.dsa().unwrap();
        assert_eq!(pkey.id(), Id::DSA);
        assert!(pkey.rsa().is_err());
    }

    #[test]
    #[cfg(not(boringssl))]
    fn test_dh_accessor() {
        let dh = include_bytes!("../test/dhparams.pem");
        let dh = Dh::params_from_pem(dh).unwrap();
        let pkey = PKey::from_dh(dh).unwrap();
        pkey.dh().unwrap();
        assert_eq!(pkey.id(), Id::DH);
        assert!(pkey.rsa().is_err());
    }

    #[test]
    fn test_ec_key_accessor() {
        let ec_key = EcKey::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();
        pkey.ec_key().unwrap();
        assert_eq!(pkey.id(), Id::EC);
        assert!(pkey.rsa().is_err());
    }

    #[test]
    fn test_rsa_conversion() {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey: PKey<Private> = rsa.clone().try_into().unwrap();
        let rsa_: Rsa<Private> = pkey.try_into().unwrap();
        // Eq is missing
        assert_eq!(rsa.p(), rsa_.p());
        assert_eq!(rsa.q(), rsa_.q());
    }

    #[test]
    fn test_dsa_conversion() {
        let dsa = Dsa::generate(2048).unwrap();
        let pkey: PKey<Private> = dsa.clone().try_into().unwrap();
        let dsa_: Dsa<Private> = pkey.try_into().unwrap();
        // Eq is missing
        assert_eq!(dsa.priv_key(), dsa_.priv_key());
    }

    #[test]
    fn test_ec_key_conversion() {
        let group = crate::ec::EcGroup::from_curve_name(crate::nid::Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey: PKey<Private> = ec_key.clone().try_into().unwrap();
        let ec_key_: EcKey<Private> = pkey.try_into().unwrap();
        // Eq is missing
        assert_eq!(ec_key.private_key(), ec_key_.private_key());
    }

    #[test]
    #[cfg(any(ossl110, libressl360))]
    fn test_security_bits() {
        let group = crate::ec::EcGroup::from_curve_name(crate::nid::Nid::SECP521R1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey: PKey<Private> = ec_key.try_into().unwrap();

        assert_eq!(pkey.security_bits(), 256);
    }

    #[test]
    #[cfg(not(boringssl))]
    fn test_dh_conversion() {
        let dh_params = include_bytes!("../test/dhparams.pem");
        let dh_params = Dh::params_from_pem(dh_params).unwrap();
        let dh = dh_params.generate_key().unwrap();

        // Clone is missing for Dh, save the parameters
        let p = dh.prime_p().to_owned().unwrap();
        let q = dh.prime_q().map(|q| q.to_owned().unwrap());
        let g = dh.generator().to_owned().unwrap();

        let pkey: PKey<Private> = dh.try_into().unwrap();
        let dh_: Dh<Private> = pkey.try_into().unwrap();

        // Eq is missing
        assert_eq!(&p, dh_.prime_p());
        assert_eq!(q, dh_.prime_q().map(|q| q.to_owned().unwrap()));
        assert_eq!(&g, dh_.generator());
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    fn test_raw_public_key(gen: fn() -> Result<PKey<Private>, ErrorStack>, key_type: Id) {
        // Generate a new key
        let key = gen().unwrap();

        // Get the raw bytes, and create a new key from the raw bytes
        let raw = key.raw_public_key().unwrap();
        let from_raw = PKey::public_key_from_raw_bytes(&raw, key_type).unwrap();

        // Compare the der encoding of the original and raw / restored public key
        assert_eq!(
            key.public_key_to_der().unwrap(),
            from_raw.public_key_to_der().unwrap()
        );
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    fn test_raw_private_key(gen: fn() -> Result<PKey<Private>, ErrorStack>, key_type: Id) {
        // Generate a new key
        let key = gen().unwrap();

        // Get the raw bytes, and create a new key from the raw bytes
        let raw = key.raw_private_key().unwrap();
        let from_raw = PKey::private_key_from_raw_bytes(&raw, key_type).unwrap();

        // Compare the der encoding of the original and raw / restored public key
        assert_eq!(
            key.private_key_to_pkcs8().unwrap(),
            from_raw.private_key_to_pkcs8().unwrap()
        );
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    #[test]
    fn test_raw_public_key_bytes() {
        test_raw_public_key(PKey::generate_x25519, Id::X25519);
        test_raw_public_key(PKey::generate_ed25519, Id::ED25519);
        #[cfg(not(any(boringssl, libressl370, awslc)))]
        test_raw_public_key(PKey::generate_x448, Id::X448);
        #[cfg(not(any(boringssl, libressl370, awslc)))]
        test_raw_public_key(PKey::generate_ed448, Id::ED448);
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    #[test]
    fn test_raw_private_key_bytes() {
        test_raw_private_key(PKey::generate_x25519, Id::X25519);
        test_raw_private_key(PKey::generate_ed25519, Id::ED25519);
        #[cfg(not(any(boringssl, libressl370, awslc)))]
        test_raw_private_key(PKey::generate_x448, Id::X448);
        #[cfg(not(any(boringssl, libressl370, awslc)))]
        test_raw_private_key(PKey::generate_ed448, Id::ED448);
    }

    #[cfg(any(ossl111, awslc))]
    #[test]
    fn test_raw_hmac() {
        let mut test_bytes = vec![0u8; 32];
        rand_bytes(&mut test_bytes).unwrap();

        let hmac_key = PKey::hmac(&test_bytes).unwrap();
        assert!(hmac_key.raw_public_key().is_err());

        let key_bytes = hmac_key.raw_private_key().unwrap();
        assert_eq!(key_bytes, test_bytes);
    }

    #[cfg(any(ossl111, awslc))]
    #[test]
    fn test_raw_key_fail() {
        // Getting a raw byte representation will not work with Nist curves
        let group = crate::ec::EcGroup::from_curve_name(Nid::SECP256K1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();
        assert!(pkey.raw_private_key().is_err());
        assert!(pkey.raw_public_key().is_err());
    }

    #[cfg(ossl300)]
    #[test]
    fn test_ec_gen() {
        let key = PKey::ec_gen("prime256v1").unwrap();
        assert!(key.ec_key().is_ok());
    }

    #[test]
    fn test_public_eq() {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey1 = PKey::from_rsa(rsa).unwrap();

        let group = crate::ec::EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey2 = PKey::from_ec_key(ec_key).unwrap();

        assert!(!pkey1.public_eq(&pkey2));
        assert!(Error::get().is_none());
    }
}
