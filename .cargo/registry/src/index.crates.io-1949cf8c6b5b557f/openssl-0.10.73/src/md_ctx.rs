//! The message digest context.
//!
//! # Examples
//!
//! Compute the SHA256 checksum of data
//!
//! ```
//! use openssl::md::Md;
//! use openssl::md_ctx::MdCtx;
//!
//! let mut ctx = MdCtx::new().unwrap();
//! ctx.digest_init(Md::sha256()).unwrap();
//! ctx.digest_update(b"Some Crypto Text").unwrap();
//! let mut digest = [0; 32];
//! ctx.digest_final(&mut digest).unwrap();
//!
//! assert_eq!(
//!     digest,
//!     *b"\x60\x78\x56\x38\x8a\xca\x5c\x51\x83\xc4\xd1\x4d\xc8\xf9\xcc\xf2\
//!        \xa5\x21\xb3\x10\x93\x72\xfa\xd6\x7c\x55\xf5\xc9\xe3\xd1\x83\x19",
//! );
//! ```
//!
//! Sign and verify data with RSA and SHA256
//!
//! ```
//! use openssl::md::Md;
//! use openssl::md_ctx::MdCtx;
//! use openssl::pkey::PKey;
//! use openssl::rsa::Rsa;
//!
//! // Generate a random RSA key.
//! let key = Rsa::generate(4096).unwrap();
//! let key = PKey::from_rsa(key).unwrap();
//!
//! let text = b"Some Crypto Text";
//!
//! // Create the signature.
//! let mut ctx = MdCtx::new().unwrap();
//! ctx.digest_sign_init(Some(Md::sha256()), &key).unwrap();
//! ctx.digest_sign_update(text).unwrap();
//! let mut signature = vec![];
//! ctx.digest_sign_final_to_vec(&mut signature).unwrap();
//!
//! // Verify the signature.
//! let mut ctx = MdCtx::new().unwrap();
//! ctx.digest_verify_init(Some(Md::sha256()), &key).unwrap();
//! ctx.digest_verify_update(text).unwrap();
//! let valid = ctx.digest_verify_final(&signature).unwrap();
//! assert!(valid);
//! ```
//!

#![cfg_attr(
    not(any(boringssl, awslc)),
    doc = r#"\
Compute and verify an HMAC-SHA256

```
use openssl::md::Md;
use openssl::md_ctx::MdCtx;
use openssl::memcmp;
use openssl::pkey::PKey;

// Create a key with the HMAC secret.
let key = PKey::hmac(b"my secret").unwrap();

let text = b"Some Crypto Text";

// Compute the HMAC.
let mut ctx = MdCtx::new().unwrap();
ctx.digest_sign_init(Some(Md::sha256()), &key).unwrap();
ctx.digest_sign_update(text).unwrap();
let mut hmac = vec![];
ctx.digest_sign_final_to_vec(&mut hmac).unwrap();

// Verify the HMAC. You can't use MdCtx to do this; instead use a constant time equality check.
# let target = hmac.clone();
let valid = memcmp::eq(&hmac, &target);
assert!(valid);
```"#
)]

use crate::error::ErrorStack;
use crate::md::MdRef;
use crate::pkey::{HasPrivate, HasPublic, PKeyRef};
use crate::pkey_ctx::PkeyCtxRef;
use crate::{cvt, cvt_p};
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use openssl_macros::corresponds;
use std::convert::TryFrom;
use std::ptr;

cfg_if! {
    if #[cfg(any(ossl110, boringssl, libressl382, awslc))] {
        use ffi::{EVP_MD_CTX_free, EVP_MD_CTX_new};
    } else {
        use ffi::{EVP_MD_CTX_create as EVP_MD_CTX_new, EVP_MD_CTX_destroy as EVP_MD_CTX_free};
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::EVP_MD_CTX;
    fn drop = EVP_MD_CTX_free;

    pub struct MdCtx;
    /// A reference to an [`MdCtx`].
    pub struct MdCtxRef;
}

impl MdCtx {
    /// Creates a new context.
    #[corresponds(EVP_MD_CTX_new)]
    #[inline]
    pub fn new() -> Result<Self, ErrorStack> {
        ffi::init();

        unsafe {
            let ptr = cvt_p(EVP_MD_CTX_new())?;
            Ok(MdCtx::from_ptr(ptr))
        }
    }
}

impl MdCtxRef {
    /// Initializes the context to compute the digest of data.
    #[corresponds(EVP_DigestInit_ex)]
    #[inline]
    pub fn digest_init(&mut self, digest: &MdRef) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestInit_ex(
                self.as_ptr(),
                digest.as_ptr(),
                ptr::null_mut(),
            ))?;
        }

        Ok(())
    }

    /// Initializes the context to compute the signature of data.
    ///
    /// A reference to the context's inner `PkeyCtx` is returned, allowing signature settings to be configured.
    #[corresponds(EVP_DigestSignInit)]
    #[inline]
    pub fn digest_sign_init<'a, T>(
        &'a mut self,
        digest: Option<&MdRef>,
        pkey: &PKeyRef<T>,
    ) -> Result<&'a mut PkeyCtxRef<T>, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            let mut p = ptr::null_mut();
            cvt(ffi::EVP_DigestSignInit(
                self.as_ptr(),
                &mut p,
                digest.map_or(ptr::null(), |p| p.as_ptr()),
                ptr::null_mut(),
                pkey.as_ptr(),
            ))?;
            Ok(PkeyCtxRef::from_ptr_mut(p))
        }
    }

    /// Initializes the context to verify the signature of data.
    ///
    /// A reference to the context's inner `PkeyCtx` is returned, allowing signature settings to be configured.
    #[corresponds(EVP_DigestVerifyInit)]
    #[inline]
    pub fn digest_verify_init<'a, T>(
        &'a mut self,
        digest: Option<&MdRef>,
        pkey: &PKeyRef<T>,
    ) -> Result<&'a mut PkeyCtxRef<T>, ErrorStack>
    where
        T: HasPublic,
    {
        unsafe {
            let mut p = ptr::null_mut();
            cvt(ffi::EVP_DigestVerifyInit(
                self.as_ptr(),
                &mut p,
                digest.map_or(ptr::null(), |p| p.as_ptr()),
                ptr::null_mut(),
                pkey.as_ptr(),
            ))?;
            Ok(PkeyCtxRef::from_ptr_mut(p))
        }
    }

    /// Updates the context with more data.
    #[corresponds(EVP_DigestUpdate)]
    #[inline]
    pub fn digest_update(&mut self, data: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestUpdate(
                self.as_ptr(),
                data.as_ptr() as *const _,
                data.len(),
            ))?;
        }

        Ok(())
    }

    /// Updates the context with more data.
    #[corresponds(EVP_DigestSignUpdate)]
    #[inline]
    pub fn digest_sign_update(&mut self, data: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestSignUpdate(
                self.as_ptr(),
                data.as_ptr() as *const _,
                data.len(),
            ))?;
        }

        Ok(())
    }

    /// Updates the context with more data.
    #[corresponds(EVP_DigestVerifyUpdate)]
    #[inline]
    pub fn digest_verify_update(&mut self, data: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestVerifyUpdate(
                self.as_ptr(),
                data.as_ptr() as *const _,
                data.len(),
            ))?;
        }

        Ok(())
    }

    /// Copies the computed digest into the buffer, returning the number of bytes written.
    #[corresponds(EVP_DigestFinal)]
    #[inline]
    pub fn digest_final(&mut self, out: &mut [u8]) -> Result<usize, ErrorStack> {
        let mut len = u32::try_from(out.len()).unwrap_or(u32::MAX);

        unsafe {
            cvt(ffi::EVP_DigestFinal(
                self.as_ptr(),
                out.as_mut_ptr(),
                &mut len,
            ))?;
        }

        Ok(len as usize)
    }

    /// Copies the computed digest into the buffer.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(EVP_DigestFinalXOF)]
    #[inline]
    #[cfg(any(ossl111, awslc))]
    pub fn digest_final_xof(&mut self, out: &mut [u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestFinalXOF(
                self.as_ptr(),
                out.as_mut_ptr(),
                out.len(),
            ))?;
        }

        Ok(())
    }

    /// Signs the computed digest.
    ///
    /// If `out` is set to `None`, an upper bound on the number of bytes required for the output buffer will be
    /// returned.
    #[corresponds(EVP_DigestSignFinal)]
    #[inline]
    pub fn digest_sign_final(&mut self, out: Option<&mut [u8]>) -> Result<usize, ErrorStack> {
        let mut len = out.as_ref().map_or(0, |b| b.len());

        unsafe {
            cvt(ffi::EVP_DigestSignFinal(
                self.as_ptr(),
                out.map_or(ptr::null_mut(), |b| b.as_mut_ptr()),
                &mut len,
            ))?;
        }

        Ok(len)
    }

    /// Like [`Self::digest_sign_final`] but appends the signature to a [`Vec`].
    pub fn digest_sign_final_to_vec(&mut self, out: &mut Vec<u8>) -> Result<usize, ErrorStack> {
        let base = out.len();
        let len = self.digest_sign_final(None)?;
        out.resize(base + len, 0);
        let len = self.digest_sign_final(Some(&mut out[base..]))?;
        out.truncate(base + len);
        Ok(len)
    }

    /// Verifies the provided signature.
    ///
    /// Returns `Ok(true)` if the signature is valid, `Ok(false)` if the signature is invalid, and `Err` if an error
    /// occurred.
    #[corresponds(EVP_DigestVerifyFinal)]
    #[inline]
    pub fn digest_verify_final(&mut self, signature: &[u8]) -> Result<bool, ErrorStack> {
        unsafe {
            let r = ffi::EVP_DigestVerifyFinal(
                self.as_ptr(),
                signature.as_ptr() as *mut _,
                signature.len(),
            );
            if r == 1 {
                Ok(true)
            } else {
                let errors = ErrorStack::get();
                if errors.errors().is_empty() {
                    Ok(false)
                } else {
                    Err(errors)
                }
            }
        }
    }

    /// Computes the signature of the data in `from`.
    ///
    /// If `to` is set to `None`, an upper bound on the number of bytes required for the output buffer will be
    /// returned.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(EVP_DigestSign)]
    #[cfg(ossl111)]
    #[inline]
    pub fn digest_sign(&mut self, from: &[u8], to: Option<&mut [u8]>) -> Result<usize, ErrorStack> {
        let mut len = to.as_ref().map_or(0, |b| b.len());

        unsafe {
            cvt(ffi::EVP_DigestSign(
                self.as_ptr(),
                to.map_or(ptr::null_mut(), |b| b.as_mut_ptr()),
                &mut len,
                from.as_ptr(),
                from.len(),
            ))?;
        }

        Ok(len)
    }

    /// Like [`Self::digest_sign`] but appends the signature to a [`Vec`].
    #[cfg(ossl111)]
    pub fn digest_sign_to_vec(
        &mut self,
        from: &[u8],
        to: &mut Vec<u8>,
    ) -> Result<usize, ErrorStack> {
        let base = to.len();
        let len = self.digest_sign(from, None)?;
        to.resize(base + len, 0);
        let len = self.digest_sign(from, Some(&mut to[base..]))?;
        to.truncate(base + len);
        Ok(len)
    }

    /// Verifies the signature of the data in `data`.
    ///
    /// Returns `Ok(true)` if the signature is valid, `Ok(false)` if the signature is invalid, and `Err` if an error
    /// occurred.
    ///
    /// Requires OpenSSL 1.1.1 or newer.
    #[corresponds(EVP_DigestVerify)]
    #[cfg(ossl111)]
    #[inline]
    pub fn digest_verify(&mut self, data: &[u8], signature: &[u8]) -> Result<bool, ErrorStack> {
        unsafe {
            let r = cvt(ffi::EVP_DigestVerify(
                self.as_ptr(),
                signature.as_ptr(),
                signature.len(),
                data.as_ptr(),
                data.len(),
            ))?;
            Ok(r == 1)
        }
    }

    /// Returns the size of the message digest, i.e. the size of the hash
    #[corresponds(EVP_MD_CTX_size)]
    #[inline]
    pub fn size(&self) -> usize {
        unsafe { ffi::EVP_MD_CTX_size(self.as_ptr()) as usize }
    }

    /// Resets the underlying EVP_MD_CTX instance
    #[corresponds(EVP_MD_CTX_reset)]
    #[cfg(ossl111)]
    #[inline]
    pub fn reset(&mut self) -> Result<(), ErrorStack> {
        unsafe {
            let _ = cvt(ffi::EVP_MD_CTX_reset(self.as_ptr()))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::md::Md;
    use crate::pkey::PKey;
    use crate::rsa::Rsa;

    #[test]
    fn verify_fail() {
        let key1 = Rsa::generate(4096).unwrap();
        let key1 = PKey::from_rsa(key1).unwrap();

        let md = Md::sha256();
        let data = b"Some Crypto Text";

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_sign_init(Some(md), &key1).unwrap();
        ctx.digest_sign_update(data).unwrap();
        let mut signature = vec![];
        ctx.digest_sign_final_to_vec(&mut signature).unwrap();

        let bad_data = b"Some Crypto text";

        ctx.digest_verify_init(Some(md), &key1).unwrap();
        ctx.digest_verify_update(bad_data).unwrap();
        assert!(matches!(
            ctx.digest_verify_final(&signature),
            Ok(false) | Err(_)
        ));
        assert!(ErrorStack::get().errors().is_empty());
    }

    #[test]
    fn verify_success() {
        let key1 = Rsa::generate(2048).unwrap();
        let key1 = PKey::from_rsa(key1).unwrap();

        let md = Md::sha256();
        let data = b"Some Crypto Text";

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_sign_init(Some(md), &key1).unwrap();
        ctx.digest_sign_update(data).unwrap();
        let mut signature = vec![];
        ctx.digest_sign_final_to_vec(&mut signature).unwrap();

        let good_data = b"Some Crypto Text";

        ctx.digest_verify_init(Some(md), &key1).unwrap();
        ctx.digest_verify_update(good_data).unwrap();
        let valid = ctx.digest_verify_final(&signature).unwrap();
        assert!(valid);
    }

    #[test]
    fn verify_with_public_success() {
        let rsa = Rsa::generate(2048).unwrap();
        let key1 = PKey::from_rsa(rsa.clone()).unwrap();

        let md = Md::sha256();
        let data = b"Some Crypto Text";

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_sign_init(Some(md), &key1).unwrap();
        ctx.digest_sign_update(data).unwrap();
        let mut signature = vec![];
        ctx.digest_sign_final_to_vec(&mut signature).unwrap();

        let good_data = b"Some Crypto Text";

        // try to verify using only public components of the key
        let n = rsa.n().to_owned().unwrap();
        let e = rsa.e().to_owned().unwrap();

        let rsa = Rsa::from_public_components(n, e).unwrap();
        let key1 = PKey::from_rsa(rsa).unwrap();

        ctx.digest_verify_init(Some(md), &key1).unwrap();
        ctx.digest_verify_update(good_data).unwrap();
        let valid = ctx.digest_verify_final(&signature).unwrap();
        assert!(valid);
    }

    #[test]
    fn verify_md_ctx_size() {
        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_init(Md::sha224()).unwrap();
        assert_eq!(Md::sha224().size(), ctx.size());
        assert_eq!(Md::sha224().size(), 28);

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_init(Md::sha256()).unwrap();
        assert_eq!(Md::sha256().size(), ctx.size());
        assert_eq!(Md::sha256().size(), 32);

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_init(Md::sha384()).unwrap();
        assert_eq!(Md::sha384().size(), ctx.size());
        assert_eq!(Md::sha384().size(), 48);

        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_init(Md::sha512()).unwrap();
        assert_eq!(Md::sha512().size(), ctx.size());
        assert_eq!(Md::sha512().size(), 64);
    }

    #[test]
    #[cfg(ossl111)]
    fn verify_md_ctx_reset() {
        let hello_expected =
            hex::decode("185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969")
                .unwrap();
        let world_expected =
            hex::decode("78ae647dc5544d227130a0682a51e30bc7777fbb6d8a8f17007463a3ecd1d524")
                .unwrap();
        // Calculate SHA-256 digest of "Hello"
        let mut ctx = MdCtx::new().unwrap();
        ctx.digest_init(Md::sha256()).unwrap();
        ctx.digest_update(b"Hello").unwrap();
        let mut result = vec![0; 32];
        let result_len = ctx.digest_final(result.as_mut_slice()).unwrap();
        assert_eq!(result_len, result.len());
        // Validate result of "Hello"
        assert_eq!(result, hello_expected);

        // Create new context
        let mut ctx = MdCtx::new().unwrap();
        // Initialize and update to "Hello"
        ctx.digest_init(Md::sha256()).unwrap();
        ctx.digest_update(b"Hello").unwrap();
        // Now reset, init to SHA-256 and use "World"
        ctx.reset().unwrap();
        ctx.digest_init(Md::sha256()).unwrap();
        ctx.digest_update(b"World").unwrap();

        let mut reset_result = vec![0; 32];
        let result_len = ctx.digest_final(reset_result.as_mut_slice()).unwrap();
        assert_eq!(result_len, reset_result.len());
        // Validate result of digest of "World"
        assert_eq!(reset_result, world_expected);
    }
}
