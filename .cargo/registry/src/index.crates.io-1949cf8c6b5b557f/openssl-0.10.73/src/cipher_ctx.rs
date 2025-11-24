//! The symmetric encryption context.
//!
//! # Examples
//!
//! Encrypt data with AES128 CBC
//!
//! ```
//! use openssl::cipher::Cipher;
//! use openssl::cipher_ctx::CipherCtx;
//!
//! let cipher = Cipher::aes_128_cbc();
//! let data = b"Some Crypto Text";
//! let key = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F";
//! let iv = b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07";
//!
//! let mut ctx = CipherCtx::new().unwrap();
//! ctx.encrypt_init(Some(cipher), Some(key), Some(iv)).unwrap();
//!
//! let mut ciphertext = vec![];
//! ctx.cipher_update_vec(data, &mut ciphertext).unwrap();
//! ctx.cipher_final_vec(&mut ciphertext).unwrap();
//!
//! assert_eq!(
//!     b"\xB4\xB9\xE7\x30\xD6\xD6\xF7\xDE\x77\x3F\x1C\xFF\xB3\x3E\x44\x5A\x91\xD7\x27\x62\x87\x4D\
//!       \xFB\x3C\x5E\xC4\x59\x72\x4A\xF4\x7C\xA1",
//!     &ciphertext[..],
//! );
//! ```
//!
//! Decrypt data with AES128 CBC
//!
//! ```
//! use openssl::cipher::Cipher;
//! use openssl::cipher_ctx::CipherCtx;
//!
//! let cipher = Cipher::aes_128_cbc();
//! let data = b"\xB4\xB9\xE7\x30\xD6\xD6\xF7\xDE\x77\x3F\x1C\xFF\xB3\x3E\x44\x5A\x91\xD7\x27\x62\
//!              \x87\x4D\xFB\x3C\x5E\xC4\x59\x72\x4A\xF4\x7C\xA1";
//! let key = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F";
//! let iv = b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07";
//!
//! let mut ctx = CipherCtx::new().unwrap();
//! ctx.decrypt_init(Some(cipher), Some(key), Some(iv)).unwrap();
//!
//! let mut plaintext = vec![];
//! ctx.cipher_update_vec(data, &mut plaintext).unwrap();
//! ctx.cipher_final_vec(&mut plaintext).unwrap();
//!
//! assert_eq!(b"Some Crypto Text", &plaintext[..]);
//! ```
#![warn(missing_docs)]

use crate::cipher::CipherRef;
use crate::error::ErrorStack;
#[cfg(not(any(boringssl, awslc)))]
use crate::pkey::{HasPrivate, HasPublic, PKey, PKeyRef};
use crate::{cvt, cvt_p};
#[cfg(ossl102)]
use bitflags::bitflags;
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::{c_int, c_uchar};
use openssl_macros::corresponds;
use std::convert::{TryFrom, TryInto};
use std::ptr;

cfg_if! {
    if #[cfg(ossl300)] {
        use ffi::EVP_CIPHER_CTX_get0_cipher;
    } else {
        use ffi::EVP_CIPHER_CTX_cipher as EVP_CIPHER_CTX_get0_cipher;
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::EVP_CIPHER_CTX;
    fn drop = ffi::EVP_CIPHER_CTX_free;

    /// A context object used to perform symmetric encryption operations.
    pub struct CipherCtx;
    /// A reference to a [`CipherCtx`].
    pub struct CipherCtxRef;
}

#[cfg(ossl102)]
bitflags! {
    /// Flags for `EVP_CIPHER_CTX`.
    pub struct CipherCtxFlags : c_int {
        /// The flag used to opt into AES key wrap ciphers.
        const FLAG_WRAP_ALLOW = ffi::EVP_CIPHER_CTX_FLAG_WRAP_ALLOW;
    }
}

impl CipherCtx {
    /// Creates a new context.
    #[corresponds(EVP_CIPHER_CTX_new)]
    pub fn new() -> Result<Self, ErrorStack> {
        ffi::init();

        unsafe {
            let ptr = cvt_p(ffi::EVP_CIPHER_CTX_new())?;
            Ok(CipherCtx::from_ptr(ptr))
        }
    }
}

impl CipherCtxRef {
    #[corresponds(EVP_CIPHER_CTX_copy)]
    pub fn copy(&mut self, src: &CipherCtxRef) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_copy(self.as_ptr(), src.as_ptr()))?;
            Ok(())
        }
    }

    /// Initializes the context for encryption.
    ///
    /// Normally this is called once to set all of the cipher, key, and IV. However, this process can be split up
    /// by first setting the cipher with no key or IV and then setting the key and IV with no cipher. This can be used
    /// to, for example, use a nonstandard IV size.
    ///
    /// # Panics
    ///
    /// Panics if the key buffer is smaller than the key size of the cipher, the IV buffer is smaller than the IV size
    /// of the cipher, or if a key or IV is provided before a cipher.
    #[corresponds(EVP_EncryptInit_ex)]
    pub fn encrypt_init(
        &mut self,
        type_: Option<&CipherRef>,
        key: Option<&[u8]>,
        iv: Option<&[u8]>,
    ) -> Result<(), ErrorStack> {
        self.cipher_init(type_, key, iv, ffi::EVP_EncryptInit_ex)
    }

    /// Initializes the context for decryption.
    ///
    /// Normally this is called once to set all of the cipher, key, and IV. However, this process can be split up
    /// by first setting the cipher with no key or IV and then setting the key and IV with no cipher. This can be used
    /// to, for example, use a nonstandard IV size.
    ///
    /// # Panics
    ///
    /// Panics if the key buffer is smaller than the key size of the cipher, the IV buffer is smaller than the IV size
    /// of the cipher, or if a key or IV is provided before a cipher.
    #[corresponds(EVP_DecryptInit_ex)]
    pub fn decrypt_init(
        &mut self,
        type_: Option<&CipherRef>,
        key: Option<&[u8]>,
        iv: Option<&[u8]>,
    ) -> Result<(), ErrorStack> {
        self.cipher_init(type_, key, iv, ffi::EVP_DecryptInit_ex)
    }

    fn cipher_init(
        &mut self,
        type_: Option<&CipherRef>,
        key: Option<&[u8]>,
        iv: Option<&[u8]>,
        f: unsafe extern "C" fn(
            *mut ffi::EVP_CIPHER_CTX,
            *const ffi::EVP_CIPHER,
            *mut ffi::ENGINE,
            *const c_uchar,
            *const c_uchar,
        ) -> c_int,
    ) -> Result<(), ErrorStack> {
        if let Some(key) = key {
            let key_len = type_.map_or_else(|| self.key_length(), |c| c.key_length());
            assert!(key_len <= key.len());
        }

        if let Some(iv) = iv {
            let iv_len = type_.map_or_else(|| self.iv_length(), |c| c.iv_length());
            assert!(iv_len <= iv.len());
        }

        unsafe {
            cvt(f(
                self.as_ptr(),
                type_.map_or(ptr::null(), |p| p.as_ptr()),
                ptr::null_mut(),
                key.map_or(ptr::null(), |k| k.as_ptr()),
                iv.map_or(ptr::null(), |iv| iv.as_ptr()),
            ))?;
        }

        Ok(())
    }

    /// Initializes the context to perform envelope encryption.
    ///
    /// Normally this is called once to set both the cipher and public keys. However, this process may be split up by
    /// first providing the cipher with no public keys and then setting the public keys with no cipher.
    ///
    /// `encrypted_keys` will contain the generated symmetric key encrypted with each corresponding asymmetric private
    /// key. The generated IV will be written to `iv`.
    ///
    /// # Panics
    ///
    /// Panics if `pub_keys` is not the same size as `encrypted_keys`, the IV buffer is smaller than the cipher's IV
    /// size, or if an IV is provided before the cipher.
    #[corresponds(EVP_SealInit)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn seal_init<T>(
        &mut self,
        type_: Option<&CipherRef>,
        pub_keys: &[PKey<T>],
        encrypted_keys: &mut [Vec<u8>],
        iv: Option<&mut [u8]>,
    ) -> Result<(), ErrorStack>
    where
        T: HasPublic,
    {
        assert_eq!(pub_keys.len(), encrypted_keys.len());
        if !pub_keys.is_empty() {
            let iv_len = type_.map_or_else(|| self.iv_length(), |c| c.iv_length());
            assert!(iv.as_ref().map_or(0, |b| b.len()) >= iv_len);
        }

        for (pub_key, buf) in pub_keys.iter().zip(&mut *encrypted_keys) {
            buf.resize(pub_key.size(), 0);
        }

        let mut keys = encrypted_keys
            .iter_mut()
            .map(|b| b.as_mut_ptr())
            .collect::<Vec<_>>();
        let mut key_lengths = vec![0; pub_keys.len()];
        let pub_keys_len = i32::try_from(pub_keys.len()).unwrap();

        unsafe {
            cvt(ffi::EVP_SealInit(
                self.as_ptr(),
                type_.map_or(ptr::null(), |p| p.as_ptr()),
                keys.as_mut_ptr(),
                key_lengths.as_mut_ptr(),
                iv.map_or(ptr::null_mut(), |b| b.as_mut_ptr()),
                pub_keys.as_ptr() as *mut _,
                pub_keys_len,
            ))?;
        }

        for (buf, len) in encrypted_keys.iter_mut().zip(key_lengths) {
            buf.truncate(len as usize);
        }

        Ok(())
    }

    /// Initializes the context to perform envelope decryption.
    ///
    /// Normally this is called once with all of the arguments present. However, this process may be split up by first
    /// providing the cipher alone and then after providing the rest of the arguments in a second call.
    ///
    /// # Panics
    ///
    /// Panics if the IV buffer is smaller than the cipher's required IV size or if the IV is provided before the
    /// cipher.
    #[corresponds(EVP_OpenInit)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn open_init<T>(
        &mut self,
        type_: Option<&CipherRef>,
        encrypted_key: &[u8],
        iv: Option<&[u8]>,
        priv_key: Option<&PKeyRef<T>>,
    ) -> Result<(), ErrorStack>
    where
        T: HasPrivate,
    {
        if priv_key.is_some() {
            let iv_len = type_.map_or_else(|| self.iv_length(), |c| c.iv_length());
            assert!(iv.map_or(0, |b| b.len()) >= iv_len);
        }

        let len = c_int::try_from(encrypted_key.len()).unwrap();
        unsafe {
            cvt(ffi::EVP_OpenInit(
                self.as_ptr(),
                type_.map_or(ptr::null(), |p| p.as_ptr()),
                encrypted_key.as_ptr(),
                len,
                iv.map_or(ptr::null(), |b| b.as_ptr()),
                priv_key.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr),
            ))?;
        }

        Ok(())
    }

    fn assert_cipher(&self) {
        unsafe {
            assert!(!EVP_CIPHER_CTX_get0_cipher(self.as_ptr()).is_null());
        }
    }

    /// Returns the block size of the context's cipher.
    ///
    /// Stream ciphers will report a block size of 1.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_block_size)]
    pub fn block_size(&self) -> usize {
        self.assert_cipher();

        unsafe { ffi::EVP_CIPHER_CTX_block_size(self.as_ptr()) as usize }
    }

    /// Returns the key length of the context's cipher.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_key_length)]
    pub fn key_length(&self) -> usize {
        self.assert_cipher();

        unsafe { ffi::EVP_CIPHER_CTX_key_length(self.as_ptr()) as usize }
    }

    /// Generates a random key based on the configured cipher.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher or if the buffer is smaller than the cipher's key
    /// length.
    #[corresponds(EVP_CIPHER_CTX_rand_key)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn rand_key(&self, buf: &mut [u8]) -> Result<(), ErrorStack> {
        assert!(buf.len() >= self.key_length());

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_rand_key(
                self.as_ptr(),
                buf.as_mut_ptr(),
            ))?;
        }

        Ok(())
    }

    /// Sets the length of the key expected by the context.
    ///
    /// Only some ciphers support configurable key lengths.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_set_key_length)]
    pub fn set_key_length(&mut self, len: usize) -> Result<(), ErrorStack> {
        self.assert_cipher();

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_set_key_length(
                self.as_ptr(),
                len.try_into().unwrap(),
            ))?;
        }

        Ok(())
    }

    /// Returns the length of the IV expected by this context.
    ///
    /// Returns 0 if the cipher does not use an IV.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_iv_length)]
    pub fn iv_length(&self) -> usize {
        self.assert_cipher();

        unsafe { ffi::EVP_CIPHER_CTX_iv_length(self.as_ptr()) as usize }
    }

    /// Returns the `num` parameter of the cipher.
    ///
    /// Built-in ciphers typically use this to track how much of the
    /// current underlying block has been "used" already.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_num)]
    #[cfg(ossl110)]
    pub fn num(&self) -> usize {
        self.assert_cipher();

        unsafe { ffi::EVP_CIPHER_CTX_num(self.as_ptr()) as usize }
    }

    /// Sets the length of the IV expected by this context.
    ///
    /// Only some ciphers support configurable IV lengths.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    #[corresponds(EVP_CIPHER_CTX_ctrl)]
    pub fn set_iv_length(&mut self, len: usize) -> Result<(), ErrorStack> {
        self.assert_cipher();

        let len = c_int::try_from(len).unwrap();

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_ctrl(
                self.as_ptr(),
                ffi::EVP_CTRL_GCM_SET_IVLEN,
                len,
                ptr::null_mut(),
            ))?;
        }

        Ok(())
    }

    /// Returns the length of the authentication tag expected by this context.
    ///
    /// Returns 0 if the cipher is not authenticated.
    ///
    /// # Panics
    ///
    /// Panics if the context has not been initialized with a cipher.
    ///
    /// Requires OpenSSL 3.0.0 or newer.
    #[corresponds(EVP_CIPHER_CTX_get_tag_length)]
    #[cfg(ossl300)]
    pub fn tag_length(&self) -> usize {
        self.assert_cipher();

        unsafe { ffi::EVP_CIPHER_CTX_get_tag_length(self.as_ptr()) as usize }
    }

    /// Retrieves the calculated authentication tag from the context.
    ///
    /// This should be called after [`Self::cipher_final`], and is only supported by authenticated ciphers.
    ///
    /// The size of the buffer indicates the size of the tag. While some ciphers support a range of tag sizes, it is
    /// recommended to pick the maximum size.
    #[corresponds(EVP_CIPHER_CTX_ctrl)]
    pub fn tag(&self, tag: &mut [u8]) -> Result<(), ErrorStack> {
        let len = c_int::try_from(tag.len()).unwrap();

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_ctrl(
                self.as_ptr(),
                ffi::EVP_CTRL_GCM_GET_TAG,
                len,
                tag.as_mut_ptr() as *mut _,
            ))?;
        }

        Ok(())
    }

    /// Sets the length of the generated authentication tag.
    ///
    /// This must be called when encrypting with a cipher in CCM mode to use a tag size other than the default.
    #[corresponds(EVP_CIPHER_CTX_ctrl)]
    pub fn set_tag_length(&mut self, len: usize) -> Result<(), ErrorStack> {
        let len = c_int::try_from(len).unwrap();

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_ctrl(
                self.as_ptr(),
                ffi::EVP_CTRL_GCM_SET_TAG,
                len,
                ptr::null_mut(),
            ))?;
        }

        Ok(())
    }

    /// Sets the authentication tag for verification during decryption.
    #[corresponds(EVP_CIPHER_CTX_ctrl)]
    pub fn set_tag(&mut self, tag: &[u8]) -> Result<(), ErrorStack> {
        let len = c_int::try_from(tag.len()).unwrap();

        unsafe {
            cvt(ffi::EVP_CIPHER_CTX_ctrl(
                self.as_ptr(),
                ffi::EVP_CTRL_GCM_SET_TAG,
                len,
                tag.as_ptr() as *mut _,
            ))?;
        }

        Ok(())
    }

    /// Enables or disables padding.
    ///
    /// If padding is disabled, the plaintext must be an exact multiple of the cipher's block size.
    #[corresponds(EVP_CIPHER_CTX_set_padding)]
    pub fn set_padding(&mut self, padding: bool) {
        unsafe {
            ffi::EVP_CIPHER_CTX_set_padding(self.as_ptr(), padding as c_int);
        }
    }

    /// Sets the total length of plaintext data.
    ///
    /// This is required for ciphers operating in CCM mode.
    #[corresponds(EVP_CipherUpdate)]
    pub fn set_data_len(&mut self, len: usize) -> Result<(), ErrorStack> {
        let len = c_int::try_from(len).unwrap();

        unsafe {
            cvt(ffi::EVP_CipherUpdate(
                self.as_ptr(),
                ptr::null_mut(),
                &mut 0,
                ptr::null(),
                len,
            ))?;
        }

        Ok(())
    }

    /// Set ctx flags.
    ///
    /// This function is currently used to enable AES key wrap feature supported by OpenSSL 1.0.2 or newer.
    #[corresponds(EVP_CIPHER_CTX_set_flags)]
    #[cfg(ossl102)]
    pub fn set_flags(&mut self, flags: CipherCtxFlags) {
        unsafe {
            ffi::EVP_CIPHER_CTX_set_flags(self.as_ptr(), flags.bits());
        }
    }

    /// Writes data into the context.
    ///
    /// Providing no output buffer will cause the input to be considered additional authenticated data (AAD).
    ///
    /// Returns the number of bytes written to `output`.
    ///
    /// # Panics
    ///
    /// Panics if `output` doesn't contain enough space for data to be
    /// written.
    #[corresponds(EVP_CipherUpdate)]
    pub fn cipher_update(
        &mut self,
        input: &[u8],
        output: Option<&mut [u8]>,
    ) -> Result<usize, ErrorStack> {
        if let Some(output) = &output {
            let mut block_size = self.block_size();
            if block_size == 1 {
                block_size = 0;
            }
            let min_output_size = input.len() + block_size;
            assert!(
                output.len() >= min_output_size,
                "Output buffer size should be at least {} bytes.",
                min_output_size
            );
        }

        unsafe { self.cipher_update_unchecked(input, output) }
    }

    /// Writes data into the context.
    ///
    /// Providing no output buffer will cause the input to be considered additional authenticated data (AAD).
    ///
    /// Returns the number of bytes written to `output`.
    ///
    /// This function is the same as [`Self::cipher_update`] but with the
    /// output size check removed. It can be used when the exact
    /// buffer size control is maintained by the caller.
    ///
    /// # Safety
    ///
    /// The caller is expected to provide `output` buffer
    /// large enough to contain correct number of bytes. For streaming
    /// ciphers the output buffer size should be at least as big as
    /// the input buffer. For block ciphers the size of the output
    /// buffer depends on the state of partially updated blocks.
    #[corresponds(EVP_CipherUpdate)]
    pub unsafe fn cipher_update_unchecked(
        &mut self,
        input: &[u8],
        output: Option<&mut [u8]>,
    ) -> Result<usize, ErrorStack> {
        let inlen = c_int::try_from(input.len()).unwrap();

        let mut outlen = 0;

        cvt(ffi::EVP_CipherUpdate(
            self.as_ptr(),
            output.map_or(ptr::null_mut(), |b| b.as_mut_ptr()),
            &mut outlen,
            input.as_ptr(),
            inlen,
        ))?;

        Ok(outlen as usize)
    }

    /// Like [`Self::cipher_update`] except that it appends output to a [`Vec`].
    pub fn cipher_update_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<usize, ErrorStack> {
        let base = output.len();
        output.resize(base + input.len() + self.block_size(), 0);
        let len = self.cipher_update(input, Some(&mut output[base..]))?;
        output.truncate(base + len);

        Ok(len)
    }

    /// Like [`Self::cipher_update`] except that it writes output into the
    /// `data` buffer. The `inlen` parameter specifies the number of bytes in
    /// `data` that are considered the input. For streaming ciphers, the size of
    /// `data` must be at least the input size. Otherwise, it must be at least
    /// an additional block size larger.
    ///
    /// Note: Use [`Self::cipher_update`] with no output argument to write AAD.
    ///
    /// # Panics
    ///
    /// This function panics if the input size cannot be represented as `int` or
    /// exceeds the buffer size, or if the output buffer does not contain enough
    /// additional space.
    #[corresponds(EVP_CipherUpdate)]
    pub fn cipher_update_inplace(
        &mut self,
        data: &mut [u8],
        inlen: usize,
    ) -> Result<usize, ErrorStack> {
        assert!(inlen <= data.len(), "Input size may not exceed buffer size");
        let block_size = self.block_size();
        if block_size != 1 {
            assert!(
                data.len() >= inlen + block_size,
                "Output buffer size must be at least {} bytes.",
                inlen + block_size
            );
        }

        let inlen = c_int::try_from(inlen).unwrap();
        let mut outlen = 0;
        unsafe {
            cvt(ffi::EVP_CipherUpdate(
                self.as_ptr(),
                data.as_mut_ptr(),
                &mut outlen,
                data.as_ptr(),
                inlen,
            ))
        }?;

        Ok(outlen as usize)
    }

    /// Finalizes the encryption or decryption process.
    ///
    /// Any remaining data will be written to the output buffer.
    ///
    /// Returns the number of bytes written to `output`.
    ///
    /// # Panics
    ///
    /// Panics if `output` is smaller than the cipher's block size.
    #[corresponds(EVP_CipherFinal)]
    pub fn cipher_final(&mut self, output: &mut [u8]) -> Result<usize, ErrorStack> {
        let block_size = self.block_size();
        if block_size > 1 {
            assert!(output.len() >= block_size);
        }

        unsafe { self.cipher_final_unchecked(output) }
    }

    /// Finalizes the encryption or decryption process.
    ///
    /// Any remaining data will be written to the output buffer.
    ///
    /// Returns the number of bytes written to `output`.
    ///
    /// This function is the same as [`Self::cipher_final`] but with
    /// the output buffer size check removed.
    ///
    /// # Safety
    ///
    /// The caller is expected to provide `output` buffer
    /// large enough to contain correct number of bytes. For streaming
    /// ciphers the output buffer can be empty, for block ciphers the
    /// output buffer should be at least as big as the block.
    #[corresponds(EVP_CipherFinal)]
    pub unsafe fn cipher_final_unchecked(
        &mut self,
        output: &mut [u8],
    ) -> Result<usize, ErrorStack> {
        let mut outl = 0;

        cvt(ffi::EVP_CipherFinal(
            self.as_ptr(),
            output.as_mut_ptr(),
            &mut outl,
        ))?;

        Ok(outl as usize)
    }

    /// Like [`Self::cipher_final`] except that it appends output to a [`Vec`].
    pub fn cipher_final_vec(&mut self, output: &mut Vec<u8>) -> Result<usize, ErrorStack> {
        let base = output.len();
        output.resize(base + self.block_size(), 0);
        let len = self.cipher_final(&mut output[base..])?;
        output.truncate(base + len);

        Ok(len)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{cipher::Cipher, rand::rand_bytes};
    #[cfg(not(any(boringssl, awslc)))]
    use std::slice;

    #[test]
    #[cfg(not(any(boringssl, awslc)))]
    fn seal_open() {
        let private_pem = include_bytes!("../test/rsa.pem");
        let public_pem = include_bytes!("../test/rsa.pem.pub");
        let private_key = PKey::private_key_from_pem(private_pem).unwrap();
        let public_key = PKey::public_key_from_pem(public_pem).unwrap();
        let cipher = Cipher::aes_256_cbc();
        let secret = b"My secret message";

        let mut ctx = CipherCtx::new().unwrap();
        let mut encrypted_key = vec![];
        let mut iv = vec![0; cipher.iv_length()];
        let mut encrypted = vec![];
        ctx.seal_init(
            Some(cipher),
            &[public_key],
            slice::from_mut(&mut encrypted_key),
            Some(&mut iv),
        )
        .unwrap();
        ctx.cipher_update_vec(secret, &mut encrypted).unwrap();
        ctx.cipher_final_vec(&mut encrypted).unwrap();

        let mut decrypted = vec![];
        ctx.open_init(Some(cipher), &encrypted_key, Some(&iv), Some(&private_key))
            .unwrap();
        ctx.cipher_update_vec(&encrypted, &mut decrypted).unwrap();
        ctx.cipher_final_vec(&mut decrypted).unwrap();

        assert_eq!(secret, &decrypted[..]);
    }

    fn aes_128_cbc(cipher: &CipherRef) {
        // from https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-38a.pdf
        let key = hex::decode("2b7e151628aed2a6abf7158809cf4f3c").unwrap();
        let iv = hex::decode("000102030405060708090a0b0c0d0e0f").unwrap();
        let pt = hex::decode("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51")
            .unwrap();
        let ct = hex::decode("7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2")
            .unwrap();

        let mut ctx = CipherCtx::new().unwrap();

        ctx.encrypt_init(Some(cipher), Some(&key), Some(&iv))
            .unwrap();
        ctx.set_padding(false);

        let mut buf = vec![];
        ctx.cipher_update_vec(&pt, &mut buf).unwrap();
        ctx.cipher_final_vec(&mut buf).unwrap();

        assert_eq!(buf, ct);

        ctx.decrypt_init(Some(cipher), Some(&key), Some(&iv))
            .unwrap();
        ctx.set_padding(false);

        let mut buf = vec![];
        ctx.cipher_update_vec(&ct, &mut buf).unwrap();
        ctx.cipher_final_vec(&mut buf).unwrap();

        assert_eq!(buf, pt);
    }

    #[test]
    #[cfg(ossl300)]
    fn fetched_aes_128_cbc() {
        let cipher = Cipher::fetch(None, "AES-128-CBC", None).unwrap();
        aes_128_cbc(&cipher);
    }

    #[test]
    fn default_aes_128_cbc() {
        let cipher = Cipher::aes_128_cbc();
        aes_128_cbc(cipher);
    }

    #[cfg(not(boringssl))]
    #[test]
    fn default_aes_128_ccm() {
        // from https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/mac/ccmtestvectors.zip
        let cipher = Cipher::aes_128_ccm();
        aes_ccm(
            cipher,
            "26511fb51fcfa75cb4b44da75a6e5a0e",
            "ea98ec44f5a86715014783172e",
            "4da40b80579c1d9a5309f7efecb7c059a2f914511ca5fc10",
            "e4692b9f06b666c7451b146c8aeb07a6e30c629d28065c3dde5940325b14b810",
            "1bf0ba0ebb20d8edba59f29a9371750c9c714078f73c335d",
            "2f1322ac69b848b001476323aed84c47",
        );
    }

    #[cfg(not(boringssl))]
    #[test]
    fn default_aes_192_ccm() {
        // from https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/mac/ccmtestvectors.zip
        let cipher = Cipher::aes_192_ccm();
        aes_ccm(
            cipher,
            "26511fb51fcfa75cb4b44da75a6e5a0eb8d9c8f3b906f886",
            "ea98ec44f5a86715014783172e",
            "4da40b80579c1d9a5309f7efecb7c059a2f914511ca5fc10",
            "e4692b9f06b666c7451b146c8aeb07a6e30c629d28065c3dde5940325b14b810",
            "30c154c616946eccc2e241d336ad33720953e449a0e6b0f0",
            "dbf8e9464909bdf337e48093c082a10b",
        );
    }

    #[cfg(not(boringssl))]
    #[test]
    fn default_aes_256_ccm() {
        // from https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/mac/ccmtestvectors.zip
        let cipher = Cipher::aes_256_ccm();
        aes_ccm(
            cipher,
            "314a202f836f9f257e22d8c11757832ae5131d357a72df88f3eff0ffcee0da4e",
            "3542fbe0f59a6d5f3abf619b7d",
            "c5b3d71312ea14f2f8fae5bd1a453192b6604a45db75c5ed",
            "dd4531f158a2fa3bc8a339f770595048f4a42bc1b03f2e824efc6ba4985119d8",
            "39c2e8f6edfe663b90963b98eb79e2d4f7f28a5053ae8881",
            "567a6b4426f1667136bed4a5e32a2bc1",
        );
    }

    #[cfg(not(boringssl))]
    fn aes_ccm(
        cipher: &CipherRef,
        key: &'static str,
        iv: &'static str,
        pt: &'static str,
        aad: &'static str,
        ct: &'static str,
        tag: &'static str,
    ) {
        let key = hex::decode(key).unwrap();
        let iv = hex::decode(iv).unwrap();
        let pt = hex::decode(pt).unwrap();
        let ct = hex::decode(ct).unwrap();
        let aad = hex::decode(aad).unwrap();
        let tag = hex::decode(tag).unwrap();

        let mut ctx = CipherCtx::new().unwrap();

        ctx.encrypt_init(Some(cipher), None, None).unwrap();
        ctx.set_iv_length(iv.len()).unwrap();
        ctx.set_tag_length(tag.len()).unwrap();
        ctx.encrypt_init(None, Some(&key), Some(&iv)).unwrap();
        ctx.set_data_len(pt.len()).unwrap();

        let mut buf = vec![];
        ctx.cipher_update(&aad, None).unwrap();
        ctx.cipher_update_vec(&pt, &mut buf).unwrap();
        ctx.cipher_final_vec(&mut buf).unwrap();
        assert_eq!(buf, ct);

        let mut out_tag = vec![0u8; tag.len()];
        ctx.tag(&mut out_tag).unwrap();
        assert_eq!(tag, out_tag);

        ctx.decrypt_init(Some(cipher), None, None).unwrap();
        ctx.set_iv_length(iv.len()).unwrap();
        ctx.set_tag(&tag).unwrap();
        ctx.decrypt_init(None, Some(&key), Some(&iv)).unwrap();
        ctx.set_data_len(pt.len()).unwrap();

        let mut buf = vec![];
        ctx.cipher_update(&aad, None).unwrap();
        ctx.cipher_update_vec(&ct, &mut buf).unwrap();
        // Some older libraries don't support calling EVP_CipherFinal/EVP_DecryptFinal for CCM
        // https://wiki.openssl.org/index.php/EVP_Authenticated_Encryption_and_Decryption#Authenticated_Decryption_using_CCM_mode
        #[cfg(any(ossl111, awslc, boringssl))]
        ctx.cipher_final_vec(&mut buf).unwrap();

        assert_eq!(buf, pt);
    }

    #[cfg(not(any(boringssl, awslc)))]
    #[test]
    fn default_aes_128_xts() {
        // https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/aes/XTSTestVectors.zip
        let cipher = Cipher::aes_128_xts();
        aes_xts(
            cipher,
            "a1b90cba3f06ac353b2c343876081762090923026e91771815f29dab01932f2f",
            "4faef7117cda59c66e4b92013e768ad5",
            "ebabce95b14d3c8d6fb350390790311c",
            "778ae8b43cb98d5a825081d5be471c63",
        );
    }

    #[cfg(not(boringssl))]
    #[test]
    fn default_aes_256_xts() {
        // https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/aes/XTSTestVectors.zip
        let cipher = Cipher::aes_256_xts();
        aes_xts(cipher, "1ea661c58d943a0e4801e42f4b0947149e7f9f8e3e68d0c7505210bd311a0e7cd6e13ffdf2418d8d1911c004cda58da3d619b7e2b9141e58318eea392cf41b08", "adf8d92627464ad2f0428e84a9f87564", "2eedea52cd8215e1acc647e810bbc3642e87287f8d2e57e36c0a24fbc12a202e", "cbaad0e2f6cea3f50b37f934d46a9b130b9d54f07e34f36af793e86f73c6d7db");
    }

    #[cfg(not(boringssl))]
    fn aes_xts(
        cipher: &CipherRef,
        key: &'static str,
        i: &'static str,
        pt: &'static str,
        ct: &'static str,
    ) {
        let key = hex::decode(key).unwrap();
        let i = hex::decode(i).unwrap();
        let pt = hex::decode(pt).unwrap();
        let ct = hex::decode(ct).unwrap();

        let mut ctx = CipherCtx::new().unwrap();
        ctx.encrypt_init(Some(cipher), Some(&key), Some(&i))
            .unwrap();
        let mut buf = vec![];
        ctx.cipher_update_vec(&pt, &mut buf).unwrap();
        ctx.cipher_final_vec(&mut buf).unwrap();

        assert_eq!(ct, buf);

        ctx.decrypt_init(Some(cipher), Some(&key), Some(&i))
            .unwrap();
        let mut buf = vec![];
        ctx.cipher_update_vec(&ct, &mut buf).unwrap();
        ctx.cipher_final_vec(&mut buf).unwrap();

        assert_eq!(pt, buf);
    }

    #[test]
    fn test_stream_ciphers() {
        #[cfg(not(boringssl))]
        {
            test_stream_cipher(Cipher::aes_128_cfb1());
            test_stream_cipher(Cipher::aes_128_cfb8());
            test_stream_cipher(Cipher::aes_128_cfb128());
            test_stream_cipher(Cipher::aes_192_cfb1());
            test_stream_cipher(Cipher::aes_192_cfb8());
            test_stream_cipher(Cipher::aes_192_cfb128());
            test_stream_cipher(Cipher::aes_256_cfb1());
            test_stream_cipher(Cipher::aes_256_cfb8());
            test_stream_cipher(Cipher::aes_256_cfb128());
        }
        test_stream_cipher(Cipher::aes_192_ctr());
        test_stream_cipher(Cipher::aes_256_ctr());
    }

    fn test_stream_cipher(cipher: &'static CipherRef) {
        let mut key = vec![0; cipher.key_length()];
        rand_bytes(&mut key).unwrap();
        let mut iv = vec![0; cipher.iv_length()];
        rand_bytes(&mut iv).unwrap();

        let mut ctx = CipherCtx::new().unwrap();

        ctx.encrypt_init(Some(cipher), Some(&key), Some(&iv))
            .unwrap();
        ctx.set_padding(false);

        assert_eq!(
            1,
            cipher.block_size(),
            "Need a stream cipher, not a block cipher"
        );

        // update cipher with non-full block
        // this is a streaming cipher so the number of output bytes
        // will be the same as the number of input bytes
        let mut output = vec![0; 32];
        let outlen = ctx
            .cipher_update(&[1; 15], Some(&mut output[0..15]))
            .unwrap();
        assert_eq!(15, outlen);

        // update cipher with missing bytes from the previous block
        // as previously it will output the same number of bytes as
        // the input
        let outlen = ctx
            .cipher_update(&[1; 17], Some(&mut output[15..]))
            .unwrap();
        assert_eq!(17, outlen);

        ctx.cipher_final_vec(&mut vec![0; 0]).unwrap();

        // encrypt again, but use in-place encryption this time
        // First reset the IV
        ctx.encrypt_init(None, None, Some(&iv)).unwrap();
        ctx.set_padding(false);
        let mut data_inplace: [u8; 32] = [1; 32];
        let outlen = ctx
            .cipher_update_inplace(&mut data_inplace[0..15], 15)
            .unwrap();
        assert_eq!(15, outlen);

        let outlen = ctx
            .cipher_update_inplace(&mut data_inplace[15..32], 17)
            .unwrap();
        assert_eq!(17, outlen);

        ctx.cipher_final(&mut [0u8; 0]).unwrap();

        // Check that the resulting data is encrypted in the same manner
        assert_eq!(data_inplace.as_slice(), output.as_slice());

        // try to decrypt
        ctx.decrypt_init(Some(cipher), Some(&key), Some(&iv))
            .unwrap();
        ctx.set_padding(false);

        // update cipher with non-full block
        // expect that the output for stream cipher will contain
        // the same number of bytes as the input
        let mut output_decrypted = vec![0; 32];
        let outlen = ctx
            .cipher_update(&output[0..15], Some(&mut output_decrypted[0..15]))
            .unwrap();
        assert_eq!(15, outlen);

        let outlen = ctx
            .cipher_update(&output[15..], Some(&mut output_decrypted[15..]))
            .unwrap();
        assert_eq!(17, outlen);

        ctx.cipher_final_vec(&mut vec![0; 0]).unwrap();
        // check if the decrypted blocks are the same as input (all ones)
        assert_eq!(output_decrypted, vec![1; 32]);

        // decrypt again, but now the output in-place
        ctx.decrypt_init(None, None, Some(&iv)).unwrap();
        ctx.set_padding(false);

        let outlen = ctx.cipher_update_inplace(&mut output[0..15], 15).unwrap();
        assert_eq!(15, outlen);

        let outlen = ctx.cipher_update_inplace(&mut output[15..], 17).unwrap();
        assert_eq!(17, outlen);

        ctx.cipher_final_vec(&mut vec![0; 0]).unwrap();
        assert_eq!(output_decrypted, output);
    }

    #[test]
    #[should_panic(expected = "Output buffer size should be at least 33 bytes.")]
    fn full_block_updates_aes_128() {
        output_buffer_too_small(Cipher::aes_128_cbc());
    }

    #[test]
    #[should_panic(expected = "Output buffer size should be at least 33 bytes.")]
    fn full_block_updates_aes_256() {
        output_buffer_too_small(Cipher::aes_256_cbc());
    }

    #[test]
    #[should_panic(expected = "Output buffer size should be at least 17 bytes.")]
    fn full_block_updates_3des() {
        output_buffer_too_small(Cipher::des_ede3_cbc());
    }

    fn output_buffer_too_small(cipher: &'static CipherRef) {
        let mut key = vec![0; cipher.key_length()];
        rand_bytes(&mut key).unwrap();
        let mut iv = vec![0; cipher.iv_length()];
        rand_bytes(&mut iv).unwrap();

        let mut ctx = CipherCtx::new().unwrap();

        ctx.encrypt_init(Some(cipher), Some(&key), Some(&iv))
            .unwrap();
        ctx.set_padding(false);

        let block_size = cipher.block_size();
        assert!(block_size > 1, "Need a block cipher, not a stream cipher");

        ctx.cipher_update(&vec![0; block_size + 1], Some(&mut vec![0; block_size - 1]))
            .unwrap();
    }

    #[cfg(ossl102)]
    fn cipher_wrap_test(cipher: &CipherRef, pt: &str, ct: &str, key: &str, iv: Option<&str>) {
        let pt = hex::decode(pt).unwrap();
        let key = hex::decode(key).unwrap();
        let expected = hex::decode(ct).unwrap();
        let iv = iv.map(|v| hex::decode(v).unwrap());
        let padding = 8 - pt.len() % 8;
        let mut computed = vec![0; pt.len() + padding + cipher.block_size() * 2];
        let mut ctx = CipherCtx::new().unwrap();

        ctx.set_flags(CipherCtxFlags::FLAG_WRAP_ALLOW);
        ctx.encrypt_init(Some(cipher), Some(&key), iv.as_deref())
            .unwrap();

        let count = ctx.cipher_update(&pt, Some(&mut computed)).unwrap();
        let rest = ctx.cipher_final(&mut computed[count..]).unwrap();
        computed.truncate(count + rest);

        if computed != expected {
            println!("Computed: {}", hex::encode(&computed));
            println!("Expected: {}", hex::encode(&expected));
            if computed.len() != expected.len() {
                println!(
                    "Lengths differ: {} in computed vs {} expected",
                    computed.len(),
                    expected.len()
                );
            }
            panic!("test failure");
        }
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes128_wrap() {
        let pt = "00112233445566778899aabbccddeeff";
        let ct = "7940ff694448b5bb5139c959a4896832e55d69aa04daa27e";
        let key = "2b7e151628aed2a6abf7158809cf4f3c";
        let iv = "0001020304050607";

        cipher_wrap_test(Cipher::aes_128_wrap(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes128_wrap_default_iv() {
        let pt = "00112233445566778899aabbccddeeff";
        let ct = "38f1215f0212526f8a70b51955b9fbdc9fe3041d9832306e";
        let key = "2b7e151628aed2a6abf7158809cf4f3c";

        cipher_wrap_test(Cipher::aes_128_wrap(), pt, ct, key, None);
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes128_wrap_pad() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "f13998f5ab32ef82a1bdbcbe585e1d837385b529572a1e1b";
        let key = "2b7e151628aed2a6abf7158809cf4f3c";
        let iv = "00010203";

        cipher_wrap_test(Cipher::aes_128_wrap_pad(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes128_wrap_pad_default_iv() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "3a501085fb8cf66f4186b7df851914d471ed823411598add";
        let key = "2b7e151628aed2a6abf7158809cf4f3c";

        cipher_wrap_test(Cipher::aes_128_wrap_pad(), pt, ct, key, None);
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes192_wrap() {
        let pt = "9f6dee187d35302116aecbfd059657efd9f7589c4b5e7f5b";
        let ct = "83b89142dfeeb4871e078bfb81134d33e23fedc19b03a1cf689973d3831b6813";
        let key = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";
        let iv = "0001020304050607";

        cipher_wrap_test(Cipher::aes_192_wrap(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes192_wrap_default_iv() {
        let pt = "9f6dee187d35302116aecbfd059657efd9f7589c4b5e7f5b";
        let ct = "c02c2cf11505d3e4851030d5534cbf5a1d7eca7ba8839adbf239756daf1b43e6";
        let key = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";

        cipher_wrap_test(Cipher::aes_192_wrap(), pt, ct, key, None);
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes192_wrap_pad() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "b4f6bb167ef7caf061a74da82b36ad038ca057ab51e98d3a";
        let key = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";
        let iv = "00010203";

        cipher_wrap_test(Cipher::aes_192_wrap_pad(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes192_wrap_pad_default_iv() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "b2c37a28cc602753a7c944a4c2555a2df9c98b2eded5312e";
        let key = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";

        cipher_wrap_test(Cipher::aes_192_wrap_pad(), pt, ct, key, None);
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes256_wrap() {
        let pt = "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51";
        let ct = "cc05da2a7f56f7dd0c144231f90bce58648fa20a8278f5a6b7d13bba6aa57a33229d4333866b7fd6";
        let key = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
        let iv = "0001020304050607";

        cipher_wrap_test(Cipher::aes_256_wrap(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl102)]
    fn test_aes256_wrap_default_iv() {
        let pt = "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51";
        let ct = "0b24f068b50e52bc6987868411c36e1b03900866ed12af81eb87cef70a8d1911731c1d7abf789d88";
        let key = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";

        cipher_wrap_test(Cipher::aes_256_wrap(), pt, ct, key, None);
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes256_wrap_pad() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "91594e044ccc06130d60e6c84a996aa4f96a9faff8c5f6e7";
        let key = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
        let iv = "00010203";

        cipher_wrap_test(Cipher::aes_256_wrap_pad(), pt, ct, key, Some(iv));
    }

    #[test]
    #[cfg(ossl110)]
    fn test_aes256_wrap_pad_default_iv() {
        let pt = "00112233445566778899aabbccddee";
        let ct = "dc3c166a854afd68aea624a4272693554bf2e4fcbae602cd";
        let key = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";

        cipher_wrap_test(Cipher::aes_256_wrap_pad(), pt, ct, key, None);
    }
}
