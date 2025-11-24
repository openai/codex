//! Low level AES IGE and key wrapping functionality
//!
//! AES ECB, CBC, XTS, CTR, CFB, GCM and other conventional symmetric encryption
//! modes are found in [`symm`].  This is the implementation of AES IGE and key wrapping
//!
//! Advanced Encryption Standard (AES) provides symmetric key cipher that
//! the same key is used to encrypt and decrypt data.  This implementation
//! uses 128, 192, or 256 bit keys.  This module provides functions to
//! create a new key with [`new_encrypt`] and perform an encryption/decryption
//! using that key with [`aes_ige`].
//!
//! [`new_encrypt`]: struct.AesKey.html#method.new_encrypt
//! [`aes_ige`]: fn.aes_ige.html
//!
//! The [`symm`] module should be used in preference to this module in most cases.
//! The IGE block cipher is a non-traditional cipher mode.  More traditional AES
//! encryption methods are found in the [`Crypter`] and [`Cipher`] structs.
//!
//! [`symm`]: ../symm/index.html
//! [`Crypter`]: ../symm/struct.Crypter.html
//! [`Cipher`]: ../symm/struct.Cipher.html
//!
//! # Examples

#![cfg_attr(
    all(
        not(boringssl),
        not(awslc),
        not(osslconf = "OPENSSL_NO_DEPRECATED_3_0")
    ),
    doc = r#"\
## AES IGE
```rust
use openssl::aes::{AesKey, aes_ige};
use openssl::symm::Mode;

let key = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F";
let plaintext = b"\x12\x34\x56\x78\x90\x12\x34\x56\x12\x34\x56\x78\x90\x12\x34\x56";
let mut iv = *b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\
                \x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1A\x1B\x1C\x1D\x1E\x1F";

 let key = AesKey::new_encrypt(key).unwrap();
 let mut output = [0u8; 16];
 aes_ige(plaintext, &mut output, &key, &mut iv, Mode::Encrypt);
 assert_eq!(output, *b"\xa6\xad\x97\x4d\x5c\xea\x1d\x36\xd2\xf3\x67\x98\x09\x07\xed\x32");
```"#
)]

//!
//! ## Key wrapping
//! ```rust
//! use openssl::aes::{AesKey, unwrap_key, wrap_key};
//!
//! let kek = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F";
//! let key_to_wrap = b"\x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF";
//!
//! let enc_key = AesKey::new_encrypt(kek).unwrap();
//! let mut ciphertext = [0u8; 24];
//! wrap_key(&enc_key, None, &mut ciphertext, &key_to_wrap[..]).unwrap();
//! let dec_key = AesKey::new_decrypt(kek).unwrap();
//! let mut orig_key = [0u8; 16];
//! unwrap_key(&dec_key, None, &mut orig_key, &ciphertext[..]).unwrap();
//!
//! assert_eq!(&orig_key[..], &key_to_wrap[..]);
//! ```
//!
use cfg_if::cfg_if;
use libc::{c_int, c_uint};
use std::mem::MaybeUninit;
use std::ptr;

#[cfg(not(any(boringssl, awslc)))]
use crate::symm::Mode;
use openssl_macros::corresponds;

/// Provides Error handling for parsing keys.
#[derive(Debug)]
pub struct KeyError(());

/// The key used to encrypt or decrypt cipher blocks.
pub struct AesKey(ffi::AES_KEY);

cfg_if! {
    if #[cfg(any(boringssl, awslc))] {
        type AesBitType = c_uint;
        type AesSizeType = usize;
    } else {
        type AesBitType = c_int;
        type AesSizeType = c_uint;
    }
}

impl AesKey {
    /// Prepares a key for encryption.
    ///
    /// # Failure
    ///
    /// Returns an error if the key is not 128, 192, or 256 bits.
    #[corresponds(AES_set_encrypt_key)]
    pub fn new_encrypt(key: &[u8]) -> Result<AesKey, KeyError> {
        unsafe {
            assert!(key.len() <= c_int::MAX as usize / 8);

            let mut aes_key = MaybeUninit::uninit();
            let r = ffi::AES_set_encrypt_key(
                key.as_ptr() as *const _,
                key.len() as AesBitType * 8,
                aes_key.as_mut_ptr(),
            );
            if r == 0 {
                Ok(AesKey(aes_key.assume_init()))
            } else {
                Err(KeyError(()))
            }
        }
    }

    /// Prepares a key for decryption.
    ///
    /// # Failure
    ///
    /// Returns an error if the key is not 128, 192, or 256 bits.
    #[corresponds(AES_set_decrypt_key)]
    pub fn new_decrypt(key: &[u8]) -> Result<AesKey, KeyError> {
        unsafe {
            assert!(key.len() <= c_int::MAX as usize / 8);

            let mut aes_key = MaybeUninit::uninit();
            let r = ffi::AES_set_decrypt_key(
                key.as_ptr() as *const _,
                key.len() as AesBitType * 8,
                aes_key.as_mut_ptr(),
            );

            if r == 0 {
                Ok(AesKey(aes_key.assume_init()))
            } else {
                Err(KeyError(()))
            }
        }
    }
}

/// Performs AES IGE encryption or decryption
///
/// AES IGE (Infinite Garble Extension) is a form of AES block cipher utilized in
/// OpenSSL.  Infinite Garble refers to propagating forward errors.  IGE, like other
/// block ciphers implemented for AES requires an initialization vector.  The IGE mode
/// allows a stream of blocks to be encrypted or decrypted without having the entire
/// plaintext available.  For more information, visit [AES IGE Encryption].
///
/// This block cipher uses 16 byte blocks.  The rust implementation will panic
/// if the input or output does not meet this 16-byte boundary.  Attention must
/// be made in this low level implementation to pad the value to the 128-bit boundary.
///
/// [AES IGE Encryption]: http://www.links.org/files/openssl-ige.pdf
///
/// # Panics
///
/// Panics if `in_` is not the same length as `out`, if that length is not a multiple of 16, or if
/// `iv` is not at least 32 bytes.
#[cfg(not(any(boringssl, awslc)))]
#[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
#[corresponds(AES_ige_encrypt)]
pub fn aes_ige(in_: &[u8], out: &mut [u8], key: &AesKey, iv: &mut [u8], mode: Mode) {
    unsafe {
        assert!(in_.len() == out.len());
        assert!(in_.len() % ffi::AES_BLOCK_SIZE as usize == 0);
        assert!(iv.len() >= ffi::AES_BLOCK_SIZE as usize * 2);

        let mode = match mode {
            Mode::Encrypt => ffi::AES_ENCRYPT,
            Mode::Decrypt => ffi::AES_DECRYPT,
        };
        ffi::AES_ige_encrypt(
            in_.as_ptr() as *const _,
            out.as_mut_ptr() as *mut _,
            in_.len(),
            &key.0,
            iv.as_mut_ptr() as *mut _,
            mode,
        );
    }
}

/// Wrap a key, according to [RFC 3394](https://tools.ietf.org/html/rfc3394)
///
/// * `key`: The key-encrypting-key to use. Must be a encrypting key
/// * `iv`: The IV to use. You must use the same IV for both wrapping and unwrapping
/// * `out`: The output buffer to store the ciphertext
/// * `in_`: The input buffer, storing the key to be wrapped
///
/// Returns the number of bytes written into `out`
///
/// # Panics
///
/// Panics if either `out` or `in_` do not have sizes that are a multiple of 8, or if
/// `out` is not 8 bytes longer than `in_`
#[corresponds(AES_wrap_key)]
pub fn wrap_key(
    key: &AesKey,
    iv: Option<[u8; 8]>,
    out: &mut [u8],
    in_: &[u8],
) -> Result<usize, KeyError> {
    unsafe {
        assert!(out.len() >= in_.len() + 8); // Ciphertext is 64 bits longer (see 2.2.1)

        let written = ffi::AES_wrap_key(
            &key.0 as *const _ as *mut _, // this is safe, the implementation only uses the key as a const pointer.
            iv.as_ref()
                .map_or(ptr::null(), |iv| iv.as_ptr() as *const _),
            out.as_ptr() as *mut _,
            in_.as_ptr() as *const _,
            in_.len() as AesSizeType,
        );
        if written <= 0 {
            Err(KeyError(()))
        } else {
            Ok(written as usize)
        }
    }
}

/// Unwrap a key, according to [RFC 3394](https://tools.ietf.org/html/rfc3394)
///
/// * `key`: The key-encrypting-key to decrypt the wrapped key. Must be a decrypting key
/// * `iv`: The same IV used for wrapping the key
/// * `out`: The buffer to write the unwrapped key to
/// * `in_`: The input ciphertext
///
/// Returns the number of bytes written into `out`
///
/// # Panics
///
/// Panics if either `out` or `in_` do not have sizes that are a multiple of 8, or
/// if `in_` is not 8 bytes longer than `out`
#[corresponds(AES_unwrap_key)]
pub fn unwrap_key(
    key: &AesKey,
    iv: Option<[u8; 8]>,
    out: &mut [u8],
    in_: &[u8],
) -> Result<usize, KeyError> {
    unsafe {
        assert!(out.len() + 8 <= in_.len());

        let written = ffi::AES_unwrap_key(
            &key.0 as *const _ as *mut _, // this is safe, the implementation only uses the key as a const pointer.
            iv.as_ref()
                .map_or(ptr::null(), |iv| iv.as_ptr() as *const _),
            out.as_ptr() as *mut _,
            in_.as_ptr() as *const _,
            in_.len() as AesSizeType,
        );

        if written <= 0 {
            Err(KeyError(()))
        } else {
            Ok(written as usize)
        }
    }
}

#[cfg(test)]
mod test {
    use hex::FromHex;

    use super::*;
    #[cfg(not(any(boringssl, awslc)))]
    use crate::symm::Mode;

    // From https://www.mgp25.com/AESIGE/
    #[test]
    #[cfg(not(any(boringssl, awslc)))]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn ige_vector_1() {
        let raw_key = "000102030405060708090A0B0C0D0E0F";
        let raw_iv = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F";
        let raw_pt = "0000000000000000000000000000000000000000000000000000000000000000";
        let raw_ct = "1A8519A6557BE652E9DA8E43DA4EF4453CF456B4CA488AA383C79C98B34797CB";

        let key = AesKey::new_encrypt(&Vec::from_hex(raw_key).unwrap()).unwrap();
        let mut iv = Vec::from_hex(raw_iv).unwrap();
        let pt = Vec::from_hex(raw_pt).unwrap();
        let ct = Vec::from_hex(raw_ct).unwrap();

        let mut ct_actual = vec![0; ct.len()];
        aes_ige(&pt, &mut ct_actual, &key, &mut iv, Mode::Encrypt);
        assert_eq!(ct_actual, ct);

        let key = AesKey::new_decrypt(&Vec::from_hex(raw_key).unwrap()).unwrap();
        let mut iv = Vec::from_hex(raw_iv).unwrap();
        let mut pt_actual = vec![0; pt.len()];
        aes_ige(&ct, &mut pt_actual, &key, &mut iv, Mode::Decrypt);
        assert_eq!(pt_actual, pt);
    }

    // from the RFC https://tools.ietf.org/html/rfc3394#section-2.2.3
    #[test]
    fn test_wrap_unwrap() {
        let raw_key = Vec::from_hex("000102030405060708090A0B0C0D0E0F").unwrap();
        let key_data = Vec::from_hex("00112233445566778899AABBCCDDEEFF").unwrap();
        let expected_ciphertext =
            Vec::from_hex("1FA68B0A8112B447AEF34BD8FB5A7B829D3E862371D2CFE5").unwrap();

        let enc_key = AesKey::new_encrypt(&raw_key).unwrap();
        let mut wrapped = [0; 24];
        assert_eq!(
            wrap_key(&enc_key, None, &mut wrapped, &key_data).unwrap(),
            24
        );
        assert_eq!(&wrapped[..], &expected_ciphertext[..]);

        let dec_key = AesKey::new_decrypt(&raw_key).unwrap();
        let mut unwrapped = [0; 16];
        assert_eq!(
            unwrap_key(&dec_key, None, &mut unwrapped, &wrapped).unwrap(),
            16
        );
        assert_eq!(&unwrapped[..], &key_data[..]);
    }
}
