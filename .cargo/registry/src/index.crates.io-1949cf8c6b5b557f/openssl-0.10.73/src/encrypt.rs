//! Message encryption.
//!
//! The [`Encrypter`] allows for encryption of data given a public key. The [`Decrypter`] can be
//! used with the corresponding private key to decrypt the data.
//!
//! # Examples
//!
//! Encrypt and decrypt data given an RSA keypair:
//!
//! ```rust
//! use openssl::encrypt::{Encrypter, Decrypter};
//! use openssl::rsa::{Rsa, Padding};
//! use openssl::pkey::PKey;
//!
//! // Generate a keypair
//! let keypair = Rsa::generate(2048).unwrap();
//! let keypair = PKey::from_rsa(keypair).unwrap();
//!
//! let data = b"hello, world!";
//!
//! // Encrypt the data with RSA PKCS1
//! let mut encrypter = Encrypter::new(&keypair).unwrap();
//! encrypter.set_rsa_padding(Padding::PKCS1).unwrap();
//! // Create an output buffer
//! let buffer_len = encrypter.encrypt_len(data).unwrap();
//! let mut encrypted = vec![0; buffer_len];
//! // Encrypt and truncate the buffer
//! let encrypted_len = encrypter.encrypt(data, &mut encrypted).unwrap();
//! encrypted.truncate(encrypted_len);
//!
//! // Decrypt the data
//! let mut decrypter = Decrypter::new(&keypair).unwrap();
//! decrypter.set_rsa_padding(Padding::PKCS1).unwrap();
//! // Create an output buffer
//! let buffer_len = decrypter.decrypt_len(&encrypted).unwrap();
//! let mut decrypted = vec![0; buffer_len];
//! // Encrypt and truncate the buffer
//! let decrypted_len = decrypter.decrypt(&encrypted, &mut decrypted).unwrap();
//! decrypted.truncate(decrypted_len);
//! assert_eq!(&*decrypted, data);
//! ```
#[cfg(any(ossl102, libressl310))]
use libc::c_int;
use std::{marker::PhantomData, ptr};

use crate::error::ErrorStack;
use crate::hash::MessageDigest;
use crate::pkey::{HasPrivate, HasPublic, PKeyRef};
use crate::rsa::Padding;
use crate::{cvt, cvt_p};
use foreign_types::ForeignTypeRef;

/// A type which encrypts data.
pub struct Encrypter<'a> {
    pctx: *mut ffi::EVP_PKEY_CTX,
    _p: PhantomData<&'a ()>,
}

unsafe impl Sync for Encrypter<'_> {}
unsafe impl Send for Encrypter<'_> {}

impl Drop for Encrypter<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::EVP_PKEY_CTX_free(self.pctx);
        }
    }
}

impl<'a> Encrypter<'a> {
    /// Creates a new `Encrypter`.
    ///
    /// OpenSSL documentation at [`EVP_PKEY_encrypt_init`].
    ///
    /// [`EVP_PKEY_encrypt_init`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_encrypt_init.html
    pub fn new<T>(pkey: &'a PKeyRef<T>) -> Result<Encrypter<'a>, ErrorStack>
    where
        T: HasPublic,
    {
        unsafe {
            ffi::init();

            let pctx = cvt_p(ffi::EVP_PKEY_CTX_new(pkey.as_ptr(), ptr::null_mut()))?;
            let r = ffi::EVP_PKEY_encrypt_init(pctx);
            if r != 1 {
                ffi::EVP_PKEY_CTX_free(pctx);
                return Err(ErrorStack::get());
            }

            Ok(Encrypter {
                pctx,
                _p: PhantomData,
            })
        }
    }

    /// Returns the RSA padding mode in use.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to `EVP_PKEY_CTX_get_rsa_padding`.
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
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_padding`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_padding`]: https://www.openssl.org/docs/manmaster/crypto/EVP_PKEY_CTX_set_rsa_padding.html
    pub fn set_rsa_padding(&mut self, padding: Padding) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_padding(
                self.pctx,
                padding.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA MGF1 algorithm.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_mgf1_md`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_mgf1_md`]: https://www.openssl.org/docs/manmaster/man7/RSA-PSS.html
    pub fn set_rsa_mgf1_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_mgf1_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA OAEP algorithm.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_oaep_md`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_oaep_md`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_CTX_set_rsa_oaep_md.html
    #[cfg(any(ossl102, libressl310, boringssl, awslc))]
    pub fn set_rsa_oaep_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_oaep_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA OAEP label.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set0_rsa_oaep_label`].
    ///
    /// [`EVP_PKEY_CTX_set0_rsa_oaep_label`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_CTX_set0_rsa_oaep_label.html
    #[cfg(any(ossl102, libressl310))]
    pub fn set_rsa_oaep_label(&mut self, label: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            let p = cvt_p(ffi::OPENSSL_malloc(label.len() as _))?;
            ptr::copy_nonoverlapping(label.as_ptr(), p as *mut u8, label.len());

            cvt(ffi::EVP_PKEY_CTX_set0_rsa_oaep_label(
                self.pctx,
                p,
                label.len() as c_int,
            ))
            .map(|_| ())
            .map_err(|e| {
                ffi::OPENSSL_free(p);
                e
            })
        }
    }

    /// Performs public key encryption.
    ///
    /// In order to know the size needed for the output buffer, use [`encrypt_len`](Encrypter::encrypt_len).
    /// Note that the length of the output buffer can be greater of the length of the encoded data.
    /// ```
    /// # use openssl::{
    /// #   encrypt::Encrypter,
    /// #   pkey::PKey,
    /// #   rsa::{Rsa, Padding},
    /// # };
    /// #
    /// # let key = include_bytes!("../test/rsa.pem");
    /// # let private_key = Rsa::private_key_from_pem(key).unwrap();
    /// # let pkey = PKey::from_rsa(private_key).unwrap();
    /// # let input = b"hello world".to_vec();
    /// #
    /// let mut encrypter = Encrypter::new(&pkey).unwrap();
    /// encrypter.set_rsa_padding(Padding::PKCS1).unwrap();
    ///
    /// // Get the length of the output buffer
    /// let buffer_len = encrypter.encrypt_len(&input).unwrap();
    /// let mut encoded = vec![0u8; buffer_len];
    ///
    /// // Encode the data and get its length
    /// let encoded_len = encrypter.encrypt(&input, &mut encoded).unwrap();
    ///
    /// // Use only the part of the buffer with the encoded data
    /// let encoded = &encoded[..encoded_len];
    /// ```
    ///
    /// This corresponds to [`EVP_PKEY_encrypt`].
    ///
    /// [`EVP_PKEY_encrypt`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_encrypt.html
    pub fn encrypt(&self, from: &[u8], to: &mut [u8]) -> Result<usize, ErrorStack> {
        let mut written = to.len();
        unsafe {
            cvt(ffi::EVP_PKEY_encrypt(
                self.pctx,
                to.as_mut_ptr(),
                &mut written,
                from.as_ptr(),
                from.len(),
            ))?;
        }

        Ok(written)
    }

    /// Gets the size of the buffer needed to encrypt the input data.
    ///
    /// This corresponds to [`EVP_PKEY_encrypt`] called with a null pointer as output argument.
    ///
    /// [`EVP_PKEY_encrypt`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_encrypt.html
    pub fn encrypt_len(&self, from: &[u8]) -> Result<usize, ErrorStack> {
        let mut written = 0;
        unsafe {
            cvt(ffi::EVP_PKEY_encrypt(
                self.pctx,
                ptr::null_mut(),
                &mut written,
                from.as_ptr(),
                from.len(),
            ))?;
        }

        Ok(written)
    }
}

/// A type which decrypts data.
pub struct Decrypter<'a> {
    pctx: *mut ffi::EVP_PKEY_CTX,
    _p: PhantomData<&'a ()>,
}

unsafe impl Sync for Decrypter<'_> {}
unsafe impl Send for Decrypter<'_> {}

impl Drop for Decrypter<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::EVP_PKEY_CTX_free(self.pctx);
        }
    }
}

impl<'a> Decrypter<'a> {
    /// Creates a new `Decrypter`.
    ///
    /// OpenSSL documentation at [`EVP_PKEY_decrypt_init`].
    ///
    /// [`EVP_PKEY_decrypt_init`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_decrypt_init.html
    pub fn new<T>(pkey: &'a PKeyRef<T>) -> Result<Decrypter<'a>, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            ffi::init();

            let pctx = cvt_p(ffi::EVP_PKEY_CTX_new(pkey.as_ptr(), ptr::null_mut()))?;
            let r = ffi::EVP_PKEY_decrypt_init(pctx);
            if r != 1 {
                ffi::EVP_PKEY_CTX_free(pctx);
                return Err(ErrorStack::get());
            }

            Ok(Decrypter {
                pctx,
                _p: PhantomData,
            })
        }
    }

    /// Returns the RSA padding mode in use.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to `EVP_PKEY_CTX_get_rsa_padding`.
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
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_padding`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_padding`]: https://www.openssl.org/docs/manmaster/crypto/EVP_PKEY_CTX_set_rsa_padding.html
    pub fn set_rsa_padding(&mut self, padding: Padding) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_padding(
                self.pctx,
                padding.as_raw(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA MGF1 algorithm.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_mgf1_md`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_mgf1_md`]: https://www.openssl.org/docs/manmaster/man7/RSA-PSS.html
    pub fn set_rsa_mgf1_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_mgf1_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA OAEP algorithm.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set_rsa_oaep_md`].
    ///
    /// [`EVP_PKEY_CTX_set_rsa_oaep_md`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_CTX_set_rsa_oaep_md.html
    #[cfg(any(ossl102, libressl310, boringssl, awslc))]
    pub fn set_rsa_oaep_md(&mut self, md: MessageDigest) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_PKEY_CTX_set_rsa_oaep_md(
                self.pctx,
                md.as_ptr() as *mut _,
            ))
            .map(|_| ())
        }
    }

    /// Sets the RSA OAEP label.
    ///
    /// This is only useful for RSA keys.
    ///
    /// This corresponds to [`EVP_PKEY_CTX_set0_rsa_oaep_label`].
    ///
    /// [`EVP_PKEY_CTX_set0_rsa_oaep_label`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_CTX_set0_rsa_oaep_label.html
    #[cfg(any(ossl102, libressl310))]
    pub fn set_rsa_oaep_label(&mut self, label: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            let p = cvt_p(ffi::OPENSSL_malloc(label.len() as _))?;
            ptr::copy_nonoverlapping(label.as_ptr(), p as *mut u8, label.len());

            cvt(ffi::EVP_PKEY_CTX_set0_rsa_oaep_label(
                self.pctx,
                p,
                label.len() as c_int,
            ))
            .map(|_| ())
            .map_err(|e| {
                ffi::OPENSSL_free(p);
                e
            })
        }
    }

    /// Performs public key decryption.
    ///
    /// In order to know the size needed for the output buffer, use [`decrypt_len`](Decrypter::decrypt_len).
    /// Note that the length of the output buffer can be greater of the length of the decoded data.
    /// ```
    /// # use openssl::{
    /// #   encrypt::Decrypter,
    /// #   pkey::PKey,
    /// #   rsa::{Rsa, Padding},
    /// # };
    /// #
    /// # const INPUT: &[u8] = b"\
    /// #     \x26\xa1\xc1\x13\xc5\x7f\xb4\x9f\xa0\xb4\xde\x61\x5e\x2e\xc6\xfb\x76\x5c\xd1\x2b\x5f\
    /// #     \x1d\x36\x60\xfa\xf8\xe8\xb3\x21\xf4\x9c\x70\xbc\x03\xea\xea\xac\xce\x4b\xb3\xf6\x45\
    /// #     \xcc\xb3\x80\x9e\xa8\xf7\xc3\x5d\x06\x12\x7a\xa3\x0c\x30\x67\xf1\xe7\x94\x6c\xf6\x26\
    /// #     \xac\x28\x17\x59\x69\xe1\xdc\xed\x7e\xc0\xe9\x62\x57\x49\xce\xdd\x13\x07\xde\x18\x03\
    /// #     \x0f\x9d\x61\x65\xb9\x23\x8c\x78\x4b\xad\x23\x49\x75\x47\x64\xa0\xa0\xa2\x90\xc1\x49\
    /// #     \x1b\x05\x24\xc2\xe9\x2c\x0d\x49\x78\x72\x61\x72\xed\x8b\x6f\x8a\xe8\xca\x05\x5c\x58\
    /// #     \xd6\x95\xd6\x7b\xe3\x2d\x0d\xaa\x3e\x6d\x3c\x9a\x1c\x1d\xb4\x6c\x42\x9d\x9a\x82\x55\
    /// #     \xd9\xde\xc8\x08\x7b\x17\xac\xd7\xaf\x86\x7b\x69\x9e\x3c\xf4\x5e\x1c\x39\x52\x6d\x62\
    /// #     \x50\x51\xbd\xa6\xc8\x4e\xe9\x34\xf0\x37\x0d\xa9\xa9\x77\xe6\xf5\xc2\x47\x2d\xa8\xee\
    /// #     \x3f\x69\x78\xff\xa9\xdc\x70\x22\x20\x9a\x5c\x9b\x70\x15\x90\xd3\xb4\x0e\x54\x9e\x48\
    /// #     \xed\xb6\x2c\x88\xfc\xb4\xa9\x37\x10\xfa\x71\xb2\xec\x75\xe7\xe7\x0e\xf4\x60\x2c\x7b\
    /// #     \x58\xaf\xa0\x53\xbd\x24\xf1\x12\xe3\x2e\x99\x25\x0a\x54\x54\x9d\xa1\xdb\xca\x41\x85\
    /// #     \xf4\x62\x78\x64";
    /// #
    /// # let key = include_bytes!("../test/rsa.pem");
    /// # let private_key = Rsa::private_key_from_pem(key).unwrap();
    /// # let pkey = PKey::from_rsa(private_key).unwrap();
    /// # let input = INPUT.to_vec();
    /// #
    /// let mut decrypter = Decrypter::new(&pkey).unwrap();
    /// decrypter.set_rsa_padding(Padding::PKCS1).unwrap();
    ///
    /// // Get the length of the output buffer
    /// let buffer_len = decrypter.decrypt_len(&input).unwrap();
    /// let mut decoded = vec![0u8; buffer_len];
    ///
    /// // Decrypt the data and get its length
    /// let decoded_len = decrypter.decrypt(&input, &mut decoded).unwrap();
    ///
    /// // Use only the part of the buffer with the decrypted data
    /// let decoded = &decoded[..decoded_len];
    /// ```
    ///
    /// This corresponds to [`EVP_PKEY_decrypt`].
    ///
    /// [`EVP_PKEY_decrypt`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_decrypt.html
    pub fn decrypt(&self, from: &[u8], to: &mut [u8]) -> Result<usize, ErrorStack> {
        let mut written = to.len();
        unsafe {
            cvt(ffi::EVP_PKEY_decrypt(
                self.pctx,
                to.as_mut_ptr(),
                &mut written,
                from.as_ptr(),
                from.len(),
            ))?;
        }

        Ok(written)
    }

    /// Gets the size of the buffer needed to decrypt the input data.
    ///
    /// This corresponds to [`EVP_PKEY_decrypt`] called with a null pointer as output argument.
    ///
    /// [`EVP_PKEY_decrypt`]: https://www.openssl.org/docs/manmaster/man3/EVP_PKEY_decrypt.html
    pub fn decrypt_len(&self, from: &[u8]) -> Result<usize, ErrorStack> {
        let mut written = 0;
        unsafe {
            cvt(ffi::EVP_PKEY_decrypt(
                self.pctx,
                ptr::null_mut(),
                &mut written,
                from.as_ptr(),
                from.len(),
            ))?;
        }

        Ok(written)
    }
}

#[cfg(test)]
mod test {
    use hex::FromHex;

    use crate::encrypt::{Decrypter, Encrypter};
    #[cfg(any(ossl102, libressl310, boringssl, awslc))]
    use crate::hash::MessageDigest;
    use crate::pkey::PKey;
    use crate::rsa::{Padding, Rsa};

    const INPUT: &str =
        "65794a68624763694f694a53557a49314e694a392e65794a7063334d694f694a71623255694c41304b49434a6c\
         654841694f6a457a4d4441344d546b7a4f44417344516f67496d6830644841364c79396c654746746347786c4c\
         6d4e76625339706331397962323930496a7030636e566c6651";

    #[test]
    fn rsa_encrypt_decrypt() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut encrypter = Encrypter::new(&pkey).unwrap();
        encrypter.set_rsa_padding(Padding::PKCS1).unwrap();
        let input = Vec::from_hex(INPUT).unwrap();
        let buffer_len = encrypter.encrypt_len(&input).unwrap();
        let mut encoded = vec![0u8; buffer_len];
        let encoded_len = encrypter.encrypt(&input, &mut encoded).unwrap();
        let encoded = &encoded[..encoded_len];

        let mut decrypter = Decrypter::new(&pkey).unwrap();
        decrypter.set_rsa_padding(Padding::PKCS1).unwrap();
        let buffer_len = decrypter.decrypt_len(encoded).unwrap();
        let mut decoded = vec![0u8; buffer_len];
        let decoded_len = decrypter.decrypt(encoded, &mut decoded).unwrap();
        let decoded = &decoded[..decoded_len];

        assert_eq!(decoded, &*input);
    }

    #[test]
    #[cfg(any(ossl102, libressl310, boringssl, awslc))]
    fn rsa_encrypt_decrypt_with_sha256() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let md = MessageDigest::sha256();

        let mut encrypter = Encrypter::new(&pkey).unwrap();
        encrypter.set_rsa_padding(Padding::PKCS1_OAEP).unwrap();
        encrypter.set_rsa_oaep_md(md).unwrap();
        encrypter.set_rsa_mgf1_md(md).unwrap();
        let input = Vec::from_hex(INPUT).unwrap();
        let buffer_len = encrypter.encrypt_len(&input).unwrap();
        let mut encoded = vec![0u8; buffer_len];
        let encoded_len = encrypter.encrypt(&input, &mut encoded).unwrap();
        let encoded = &encoded[..encoded_len];

        let mut decrypter = Decrypter::new(&pkey).unwrap();
        decrypter.set_rsa_padding(Padding::PKCS1_OAEP).unwrap();
        decrypter.set_rsa_oaep_md(md).unwrap();
        decrypter.set_rsa_mgf1_md(md).unwrap();
        let buffer_len = decrypter.decrypt_len(encoded).unwrap();
        let mut decoded = vec![0u8; buffer_len];
        let decoded_len = decrypter.decrypt(encoded, &mut decoded).unwrap();
        let decoded = &decoded[..decoded_len];

        assert_eq!(decoded, &*input);
    }

    #[test]
    #[cfg(any(ossl102, libressl310))]
    fn rsa_encrypt_decrypt_oaep_label() {
        let key = include_bytes!("../test/rsa.pem");
        let private_key = Rsa::private_key_from_pem(key).unwrap();
        let pkey = PKey::from_rsa(private_key).unwrap();

        let mut encrypter = Encrypter::new(&pkey).unwrap();
        encrypter.set_rsa_padding(Padding::PKCS1_OAEP).unwrap();
        encrypter.set_rsa_oaep_label(b"test_oaep_label").unwrap();
        let input = Vec::from_hex(INPUT).unwrap();
        let buffer_len = encrypter.encrypt_len(&input).unwrap();
        let mut encoded = vec![0u8; buffer_len];
        let encoded_len = encrypter.encrypt(&input, &mut encoded).unwrap();
        let encoded = &encoded[..encoded_len];

        let mut decrypter = Decrypter::new(&pkey).unwrap();
        decrypter.set_rsa_padding(Padding::PKCS1_OAEP).unwrap();
        decrypter.set_rsa_oaep_label(b"test_oaep_label").unwrap();
        let buffer_len = decrypter.decrypt_len(encoded).unwrap();
        let mut decoded = vec![0u8; buffer_len];
        let decoded_len = decrypter.decrypt(encoded, &mut decoded).unwrap();
        let decoded = &decoded[..decoded_len];

        assert_eq!(decoded, &*input);

        decrypter.set_rsa_oaep_label(b"wrong_oaep_label").unwrap();
        let buffer_len = decrypter.decrypt_len(encoded).unwrap();
        let mut decoded = vec![0u8; buffer_len];

        assert!(decrypter.decrypt(encoded, &mut decoded).is_err());
    }
}
