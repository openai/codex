//! Message signatures.
//!
//! The `Signer` allows for the computation of cryptographic signatures of
//! data given a private key. The `Verifier` can then be used with the
//! corresponding public key to verify the integrity and authenticity of that
//! data given the signature.
//!
//! # Examples
//!
//! Sign and verify data given an RSA keypair:
//!
//! ```rust
//! use openssl::sign::{Signer, Verifier};
//! use openssl::rsa::Rsa;
//! use openssl::pkey::PKey;
//! use openssl::hash::MessageDigest;
//!
//! // Generate a keypair
//! let keypair = Rsa::generate(2048).unwrap();
//! let keypair = PKey::from_rsa(keypair).unwrap();
//!
//! let data = b"hello, world!";
//! let data2 = b"hola, mundo!";
//!
//! // Sign the data
//! let mut signer = Signer::new(MessageDigest::sha256(), &keypair).unwrap();
//! signer.update(data).unwrap();
//! signer.update(data2).unwrap();
//! let signature = signer.sign_to_vec().unwrap();
//!
//! // Verify the data
//! let mut verifier = Verifier::new(MessageDigest::sha256(), &keypair).unwrap();
//! verifier.update(data).unwrap();
//! verifier.update(data2).unwrap();
//! assert!(verifier.verify(&signature).unwrap());
//! ```

#![cfg_attr(
    not(any(boringssl, awslc)),
    doc = r#"\

Compute an HMAC:

```rust
use openssl::hash::MessageDigest;
use openssl::memcmp;
use openssl::pkey::PKey;
use openssl::sign::Signer;

// Create a PKey
let key = PKey::hmac(b"my secret").unwrap();

let data = b"hello, world!";
let data2 = b"hola, mundo!";

// Compute the HMAC
let mut signer = Signer::new(MessageDigest::sha256(), &key).unwrap();
signer.update(data).unwrap();
signer.update(data2).unwrap();
let hmac = signer.sign_to_vec().unwrap();

// `Verifier` cannot be used with HMACs; use the `memcmp::eq` function instead
//
// Do not simply check for equality with `==`!
# let target = hmac.clone();
assert!(memcmp::eq(&hmac, &target));
```"#
)]

use cfg_if::cfg_if;
use foreign_types::ForeignTypeRef;
use libc::c_int;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::ptr;

use crate::error::ErrorStack;
use crate::hash::MessageDigest;
use crate::pkey::{HasPrivate, HasPublic, PKeyRef};
use crate::rsa::Padding;
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

cfg_if! {
    if #[cfg(any(ossl110, libressl382))] {
        use ffi::{EVP_MD_CTX_free, EVP_MD_CTX_new};
    } else {
        use ffi::{EVP_MD_CTX_create as EVP_MD_CTX_new, EVP_MD_CTX_destroy as EVP_MD_CTX_free};
    }
}

/// Salt lengths that must be used with `set_rsa_pss_saltlen`.
pub struct RsaPssSaltlen(c_int);

impl RsaPssSaltlen {
    /// Returns the integer representation of `RsaPssSaltlen`.
    pub(crate) fn as_raw(&self) -> c_int {
        self.0
    }

    /// Sets the salt length to the given value.
    pub fn custom(val: c_int) -> RsaPssSaltlen {
        RsaPssSaltlen(val)
    }

    /// The salt length is set to the digest length.
    /// Corresponds to the special value `-1`.
    pub const DIGEST_LENGTH: RsaPssSaltlen = RsaPssSaltlen(-1);
    /// The salt length is set to the maximum permissible value.
    /// Corresponds to the special value `-2`.
    pub const MAXIMUM_LENGTH: RsaPssSaltlen = RsaPssSaltlen(-2);
}

/// A type which computes cryptographic signatures of data.
pub struct Signer<'a> {
    md_ctx: *mut ffi::EVP_MD_CTX,
    pctx: *mut ffi::EVP_PKEY_CTX,
    _p: PhantomData<&'a ()>,
}

unsafe impl Sync for Signer<'_> {}
unsafe impl Send for Signer<'_> {}

impl Drop for Signer<'_> {
    fn drop(&mut self) {
        // pkey_ctx is owned by the md_ctx, so no need to explicitly free it.
        unsafe {
            EVP_MD_CTX_free(self.md_ctx);
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Signer<'_> {
    /// Creates a new `Signer`.
    ///
    /// This cannot be used with Ed25519 or Ed448 keys. Please refer to
    /// `new_without_digest`.
    #[corresponds(EVP_DigestSignInit)]
    pub fn new<'a, T>(type_: MessageDigest, pkey: &PKeyRef<T>) -> Result<Signer<'a>, ErrorStack>
    where
        T: HasPrivate,
    {
        Self::new_intern(Some(type_), pkey)
    }

    /// Creates a new `Signer` without a digest.
    ///
    /// This is the only way to create a `Verifier` for Ed25519 or Ed448 keys.
    /// It can also be used to create a CMAC.
    #[corresponds(EVP_DigestSignInit)]
    pub fn new_without_digest<'a, T>(pkey: &PKeyRef<T>) -> Result<Signer<'a>, ErrorStack>
    where
        T: HasPrivate,
    {
        Self::new_intern(None, pkey)
    }

    fn new_intern<'a, T>(
        type_: Option<MessageDigest>,
        pkey: &PKeyRef<T>,
    ) -> Result<Signer<'a>, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            ffi::init();

            let ctx = cvt_p(EVP_MD_CTX_new())?;
            let mut pctx: *mut ffi::EVP_PKEY_CTX = ptr::null_mut();
            let r = ffi::EVP_DigestSignInit(
                ctx,
                &mut pctx,
                type_.map(|t| t.as_ptr()).unwrap_or(ptr::null()),
                ptr::null_mut(),
                pkey.as_ptr(),
            );
            if r != 1 {
                EVP_MD_CTX_free(ctx);
                return Err(ErrorStack::get());
            }

            assert!(!pctx.is_null());

            Ok(Signer {
                md_ctx: ctx,
                pctx,
                _p: PhantomData,
            })
        }
    }

    /// Returns the RSA padding mode in use.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_get_rsa_padding)]
    pub fn rsa_padding(&self) -> Result<Padding, ErrorStack> {
        unsafe {
            let mut pad = 0;
            cvt(ffi::EVP_PKEY_CTX_get_rsa_padding(self.pctx, &mut pad))
                .map(|_| Padding::from_raw(pad))
        }
    }

    /// Sets the RSA padding mode.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_padding)]
    pub fn set_rsa_padding(&mut self, padding: Padding) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_padding(
                self.pctx,
                padding.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA PSS salt length.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_pss_saltlen)]
    pub fn set_rsa_pss_saltlen(&mut self, len: RsaPssSaltlen) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_pss_saltlen(
                self.pctx,
                len.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA MGF1 algorithm.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_mgf1_md)]
    pub fn set_rsa_mgf1_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_mgf1_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Feeds more data into the `Signer`.
    ///
    /// Please note that PureEdDSA (Ed25519 and Ed448 keys) do not support streaming.
    /// Use `sign_oneshot` instead.
    #[corresponds(EVP_DigestUpdate)]
    pub fn update(&mut self, buf: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestUpdate(
                self.md_ctx,
                buf.as_ptr() as *const _,
                buf.len(),
            ))
            .map(|_| ())
        }
    }

    /// Computes an upper bound on the signature length.
    ///
    /// The actual signature may be shorter than this value. Check the return value of
    /// `sign` to get the exact length.
    #[corresponds(EVP_DigestSignFinal)]
    pub fn len(&self) -> Result<usize, ErrorStack> {
        self.len_intern()
    }

    #[cfg(not(any(ossl111, boringssl, libressl370, awslc)))]
    fn len_intern(&self) -> Result<usize, ErrorStack> {
        unsafe {
            let mut len = 0;
            cvt(ffi::EVP_DigestSignFinal(
                self.md_ctx,
                ptr::null_mut(),
                &mut len,
            ))?;
            Ok(len)
        }
    }

    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    fn len_intern(&self) -> Result<usize, ErrorStack> {
        unsafe {
            let mut len = 0;
            cvt(ffi::EVP_DigestSign(
                self.md_ctx,
                ptr::null_mut(),
                &mut len,
                ptr::null(),
                0,
            ))?;
            Ok(len)
        }
    }

    /// Writes the signature into the provided buffer, returning the number of bytes written.
    ///
    /// This method will fail if the buffer is not large enough for the signature. Use the `len`
    /// method to get an upper bound on the required size.
    #[corresponds(EVP_DigestSignFinal)]
    pub fn sign(&self, buf: &mut [u8]) -> Result<usize, ErrorStack> {
        unsafe {
            let mut len = buf.len();
            cvt(ffi::EVP_DigestSignFinal(
                self.md_ctx,
                buf.as_mut_ptr() as *mut _,
                &mut len,
            ))?;
            Ok(len)
        }
    }

    /// Returns the signature.
    ///
    /// This is a simple convenience wrapper over `len` and `sign`.
    pub fn sign_to_vec(&self) -> Result<Vec<u8>, ErrorStack> {
        let mut buf = vec![0; self.len()?];
        let len = self.sign(&mut buf)?;
        // The advertised length is not always equal to the real length for things like DSA
        buf.truncate(len);
        Ok(buf)
    }

    /// Signs the data in `data_buf` and writes the signature into the buffer `sig_buf`, returning the
    /// number of bytes written.
    ///
    /// For PureEdDSA (Ed25519 and Ed448 keys), this is the only way to sign data.
    ///
    /// This method will fail if the buffer is not large enough for the signature. Use the `len`
    /// method to get an upper bound on the required size.
    #[corresponds(EVP_DigestSign)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn sign_oneshot(
        &mut self,
        sig_buf: &mut [u8],
        data_buf: &[u8],
    ) -> Result<usize, ErrorStack> {
        unsafe {
            let mut sig_len = sig_buf.len();
            cvt(ffi::EVP_DigestSign(
                self.md_ctx,
                sig_buf.as_mut_ptr() as *mut _,
                &mut sig_len,
                data_buf.as_ptr() as *const _,
                data_buf.len(),
            ))?;
            Ok(sig_len)
        }
    }

    /// Returns the signature.
    ///
    /// This is a simple convenience wrapper over `len` and `sign_oneshot`.
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn sign_oneshot_to_vec(&mut self, data_buf: &[u8]) -> Result<Vec<u8>, ErrorStack> {
        let mut sig_buf = vec![0; self.len()?];
        let len = self.sign_oneshot(&mut sig_buf, data_buf)?;
        // The advertised length is not always equal to the real length for things like DSA
        sig_buf.truncate(len);
        Ok(sig_buf)
    }
}

impl Write for Signer<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A type which can be used to verify the integrity and authenticity
/// of data given the signature.
pub struct Verifier<'a> {
    md_ctx: *mut ffi::EVP_MD_CTX,
    pctx: *mut ffi::EVP_PKEY_CTX,
    pkey_pd: PhantomData<&'a ()>,
}

unsafe impl Sync for Verifier<'_> {}
unsafe impl Send for Verifier<'_> {}

impl Drop for Verifier<'_> {
    fn drop(&mut self) {
        // pkey_ctx is owned by the md_ctx, so no need to explicitly free it.
        unsafe {
            EVP_MD_CTX_free(self.md_ctx);
        }
    }
}

/// A type which verifies cryptographic signatures of data.
impl<'a> Verifier<'a> {
    /// Creates a new `Verifier`.
    ///
    /// This cannot be used with Ed25519 or Ed448 keys. Please refer to
    /// [`Verifier::new_without_digest`].
    #[corresponds(EVP_DigestVerifyInit)]
    pub fn new<T>(type_: MessageDigest, pkey: &'a PKeyRef<T>) -> Result<Verifier<'a>, ErrorStack>
    where
        T: HasPublic,
    {
        Verifier::new_intern(Some(type_), pkey)
    }

    /// Creates a new `Verifier` without a digest.
    ///
    /// This is the only way to create a `Verifier` for Ed25519 or Ed448 keys.
    #[corresponds(EVP_DigestVerifyInit)]
    pub fn new_without_digest<T>(pkey: &'a PKeyRef<T>) -> Result<Verifier<'a>, ErrorStack>
    where
        T: HasPublic,
    {
        Verifier::new_intern(None, pkey)
    }

    fn new_intern<T>(
        type_: Option<MessageDigest>,
        pkey: &'a PKeyRef<T>,
    ) -> Result<Verifier<'a>, ErrorStack>
    where
        T: HasPublic,
    {
        unsafe {
            ffi::init();

            let ctx = cvt_p(EVP_MD_CTX_new())?;
            let mut pctx: *mut ffi::EVP_PKEY_CTX = ptr::null_mut();
            let r = ffi::EVP_DigestVerifyInit(
                ctx,
                &mut pctx,
                type_.map(|t| t.as_ptr()).unwrap_or(ptr::null()),
                ptr::null_mut(),
                pkey.as_ptr(),
            );
            if r != 1 {
                EVP_MD_CTX_free(ctx);
                return Err(ErrorStack::get());
            }

            assert!(!pctx.is_null());

            Ok(Verifier {
                md_ctx: ctx,
                pctx,
                pkey_pd: PhantomData,
            })
        }
    }

    /// Returns the RSA padding mode in use.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_get_rsa_padding)]
    pub fn rsa_padding(&self) -> Result<Padding, ErrorStack> {
        unsafe {
            let mut pad = 0;
            cvt(ffi::EVP_PKEY_CTX_get_rsa_padding(self.pctx, &mut pad))
                .map(|_| Padding::from_raw(pad))
        }
    }

    /// Sets the RSA padding mode.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_padding)]
    pub fn set_rsa_padding(&mut self, padding: Padding) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_padding(
                self.pctx,
                padding.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA PSS salt length.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_pss_saltlen)]
    pub fn set_rsa_pss_saltlen(&mut self, len: RsaPssSaltlen) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_pss_saltlen(
                self.pctx,
                len.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA MGF1 algorithm.
    ///
    /// This is only useful for RSA keys.
    #[corresponds(EVP_PKEY_CTX_set_rsa_mgf1_md)]
    pub fn set_rsa_mgf1_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_mgf1_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Feeds more data into the `Verifier`.
    ///
    /// Please note that PureEdDSA (Ed25519 and Ed448 keys) do not support streaming.
    /// Use [`Verifier::verify_oneshot`] instead.
    #[corresponds(EVP_DigestUpdate)]
    pub fn update(&mut self, buf: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestUpdate(
                self.md_ctx,
                buf.as_ptr() as *const _,
                buf.len(),
            ))
            .map(|_| ())
        }
    }

    /// Determines if the data fed into the `Verifier` matches the provided signature.
    #[corresponds(EVP_DigestVerifyFinal)]
    pub fn verify(&self, signature: &[u8]) -> Result<bool, ErrorStack> {
        unsafe {
            let r =
                EVP_DigestVerifyFinal(self.md_ctx, signature.as_ptr() as *mut _, signature.len());
            match r {
                1 => Ok(true),
                0 => {
                    ErrorStack::get(); // discard error stack
                    Ok(false)
                }
                _ => Err(ErrorStack::get()),
            }
        }
    }

    /// Determines if the data given in `buf` matches the provided signature.
    #[corresponds(EVP_DigestVerify)]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    pub fn verify_oneshot(&mut self, signature: &[u8], buf: &[u8]) -> Result<bool, ErrorStack> {
        unsafe {
            let r = ffi::EVP_DigestVerify(
                self.md_ctx,
                signature.as_ptr() as *const _,
                signature.len(),
                buf.as_ptr() as *const _,
                buf.len(),
            );
            match r {
                1 => Ok(true),
                0 => {
                    ErrorStack::get();
                    Ok(false)
                }
                _ => Err(ErrorStack::get()),
            }
        }
    }
}

impl Write for Verifier<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(not(ossl101))]
use ffi::EVP_DigestVerifyFinal;

#[cfg(ossl101)]
#[allow(bad_style)]
unsafe fn EVP_DigestVerifyFinal(
    ctx: *mut ffi::EVP_MD_CTX,
    sigret: *const ::libc::c_uchar,
    siglen: ::libc::size_t,
) -> ::libc::c_int {
    ffi::EVP_DigestVerifyFinal(ctx, sigret as *mut _, siglen)
}

#[cfg(test)]
mod test {
    use hex::{self, FromHex};
    #[cfg(not(boringssl))]
    use std::iter;

    use crate::ec::{EcGroup, EcKey};
    use crate::hash::MessageDigest;
    use crate::nid::Nid;
    use crate::pkey::PKey;
    use crate::rsa::{Padding, Rsa};
    #[cfg(any(ossl111, awslc))]
    use crate::sign::RsaPssSaltlen;
    use crate::sign::{Signer, Verifier};

    const INPUT: &str =
        "65794a68624763694f694a53557a49314e694a392e65794a7063334d694f694a71623255694c41304b49434a6c\
         654841694f6a457a4d4441344d546b7a4f44417344516f67496d6830644841364c79396c654746746347786c4c\
         6d4e76625339706331397962323930496a7030636e566c6651";

    const SIGNATURE: &str =
        "702e218943e88fd11eb5d82dbf7845f34106ae1b81fff7731116add1717d83656d420afd3c96eedd73a2663e51\
         66687b000b87226e0187ed1073f945e582adfcef16d85a798ee8c66ddb3db8975b17d09402beedd5d9d9700710\
         8db28160d5f8040ca7445762b81fbe7ff9d92e0ae76f24f25b33bbe6f44ae61eb1040acb20044d3ef9128ed401\
         30795bd4bd3b41eecad066ab651981fde48df77f372dc38b9fafdd3befb18b5da3cc3c2eb02f9e3a41d612caad\
         15911273a05f23b9e838faaf849d698429ef5a1e88798236c3d40e604522a544c8f27a7a2db80663d16cf7caea\
         56de405cb2215a45b2c25566b55ac1a748a070dfc8a32a469543d019eefb47";

    #[test]
    fn rsa_sign() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut signer = Signer::new(MessageDigest::sha256(), &pkey).unwrap();
        assert_eq!(signer.rsa_padding().unwrap(), Padding::PKCS1);
        signer.set_rsa_padding(Padding::PKCS1).unwrap();
        signer.update(&Vec::from_hex(INPUT).unwrap()).unwrap();
        let result = signer.sign_to_vec().unwrap();

        assert_eq!(hex::encode(result), SIGNATURE);
    }

    #[test]
    fn rsa_verify_ok() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).unwrap();
        assert_eq!(verifier.rsa_padding().unwrap(), Padding::PKCS1);
        verifier.update(&Vec::from_hex(INPUT).unwrap()).unwrap();
        assert!(verifier.verify(&Vec::from_hex(SIGNATURE).unwrap()).unwrap());
    }

    #[test]
    fn rsa_verify_invalid() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).unwrap();
        verifier.update(&Vec::from_hex(INPUT).unwrap()).unwrap();
        verifier.update(b"foobar").unwrap();
        assert!(!verifier.verify(&Vec::from_hex(SIGNATURE).unwrap()).unwrap());
    }

    #[cfg(not(boringssl))]
    fn test_hmac(ty: MessageDigest, tests: &[(Vec<u8>, Vec<u8>, Vec<u8>)]) {
        for (key, data, res) in tests.iter() {
            let pkey = PKey::hmac(key).unwrap();
            let mut signer = Signer::new(ty, &pkey).unwrap();
            signer.update(data).unwrap();
            assert_eq!(signer.sign_to_vec().unwrap(), *res);
        }
    }

    #[test]
    #[cfg(not(boringssl))]
    fn hmac_md5() {
        // test vectors from RFC 2202
        let tests: [(Vec<u8>, Vec<u8>, Vec<u8>); 7] = [
            (
                iter::repeat(0x0b_u8).take(16).collect(),
                b"Hi There".to_vec(),
                Vec::from_hex("9294727a3638bb1c13f48ef8158bfc9d").unwrap(),
            ),
            (
                b"Jefe".to_vec(),
                b"what do ya want for nothing?".to_vec(),
                Vec::from_hex("750c783e6ab0b503eaa86e310a5db738").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(16).collect(),
                iter::repeat(0xdd_u8).take(50).collect(),
                Vec::from_hex("56be34521d144c88dbb8c733f0e8b3f6").unwrap(),
            ),
            (
                Vec::from_hex("0102030405060708090a0b0c0d0e0f10111213141516171819").unwrap(),
                iter::repeat(0xcd_u8).take(50).collect(),
                Vec::from_hex("697eaf0aca3a3aea3a75164746ffaa79").unwrap(),
            ),
            (
                iter::repeat(0x0c_u8).take(16).collect(),
                b"Test With Truncation".to_vec(),
                Vec::from_hex("56461ef2342edc00f9bab995690efd4c").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(80).collect(),
                b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
                Vec::from_hex("6b1ab7fe4bd7bf8f0b62e6ce61b9d0cd").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(80).collect(),
                b"Test Using Larger Than Block-Size Key \
              and Larger Than One Block-Size Data"
                    .to_vec(),
                Vec::from_hex("6f630fad67cda0ee1fb1f562db3aa53e").unwrap(),
            ),
        ];

        test_hmac(MessageDigest::md5(), &tests);
    }

    #[test]
    #[cfg(not(boringssl))]
    fn hmac_sha1() {
        // test vectors from RFC 2202
        let tests: [(Vec<u8>, Vec<u8>, Vec<u8>); 7] = [
            (
                iter::repeat(0x0b_u8).take(20).collect(),
                b"Hi There".to_vec(),
                Vec::from_hex("b617318655057264e28bc0b6fb378c8ef146be00").unwrap(),
            ),
            (
                b"Jefe".to_vec(),
                b"what do ya want for nothing?".to_vec(),
                Vec::from_hex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(20).collect(),
                iter::repeat(0xdd_u8).take(50).collect(),
                Vec::from_hex("125d7342b9ac11cd91a39af48aa17b4f63f175d3").unwrap(),
            ),
            (
                Vec::from_hex("0102030405060708090a0b0c0d0e0f10111213141516171819").unwrap(),
                iter::repeat(0xcd_u8).take(50).collect(),
                Vec::from_hex("4c9007f4026250c6bc8414f9bf50c86c2d7235da").unwrap(),
            ),
            (
                iter::repeat(0x0c_u8).take(20).collect(),
                b"Test With Truncation".to_vec(),
                Vec::from_hex("4c1a03424b55e07fe7f27be1d58bb9324a9a5a04").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(80).collect(),
                b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
                Vec::from_hex("aa4ae5e15272d00e95705637ce8a3b55ed402112").unwrap(),
            ),
            (
                iter::repeat(0xaa_u8).take(80).collect(),
                b"Test Using Larger Than Block-Size Key \
              and Larger Than One Block-Size Data"
                    .to_vec(),
                Vec::from_hex("e8e99d0f45237d786d6bbaa7965c7808bbff1a91").unwrap(),
            ),
        ];

        test_hmac(MessageDigest::sha1(), &tests);
    }

    #[test]
    #[cfg(ossl110)]
    fn test_cmac() {
        let cipher = crate::symm::Cipher::aes_128_cbc();
        let key = Vec::from_hex("9294727a3638bb1c13f48ef8158bfc9d").unwrap();
        let pkey = PKey::cmac(&cipher, &key).unwrap();
        let mut signer = Signer::new_without_digest(&pkey).unwrap();

        let data = b"Hi There";
        signer.update(data as &[u8]).unwrap();

        let expected = vec![
            136, 101, 61, 167, 61, 30, 248, 234, 124, 166, 196, 157, 203, 52, 171, 19,
        ];
        assert_eq!(signer.sign_to_vec().unwrap(), expected);
    }

    #[test]
    fn ec() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let key = PKey::from_ec_key(key).unwrap();

        let mut signer = Signer::new(MessageDigest::sha256(), &key).unwrap();
        signer.update(b"hello world").unwrap();
        let signature = signer.sign_to_vec().unwrap();

        let mut verifier = Verifier::new(MessageDigest::sha256(), &key).unwrap();
        verifier.update(b"hello world").unwrap();
        assert!(verifier.verify(&signature).unwrap());
    }

    #[test]
    #[cfg(any(ossl111, boringssl, libressl370, awslc))]
    fn eddsa() {
        let key = PKey::generate_ed25519().unwrap();

        let mut signer = Signer::new_without_digest(&key).unwrap();
        let signature = signer.sign_oneshot_to_vec(b"hello world").unwrap();

        let mut verifier = Verifier::new_without_digest(&key).unwrap();
        assert!(verifier.verify_oneshot(&signature, b"hello world").unwrap());
    }

    #[test]
    #[cfg(any(ossl111, awslc))]
    fn rsa_sign_verify() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut signer = Signer::new(MessageDigest::sha256(), &pkey).unwrap();
        signer.set_rsa_padding(Padding::PKCS1_PSS).unwrap();
        assert_eq!(signer.rsa_padding().unwrap(), Padding::PKCS1_PSS);
        signer
            .set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
            .unwrap();
        signer.set_rsa_mgf1_md(MessageDigest::sha256()).unwrap();
        signer.update(&Vec::from_hex(INPUT).unwrap()).unwrap();
        let signature = signer.sign_to_vec().unwrap();

        let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).unwrap();
        verifier.set_rsa_padding(Padding::PKCS1_PSS).unwrap();
        verifier
            .set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
            .unwrap();
        verifier.set_rsa_mgf1_md(MessageDigest::sha256()).unwrap();
        verifier.update(&Vec::from_hex(INPUT).unwrap()).unwrap();
        assert!(verifier.verify(&signature).unwrap());
    }
}
