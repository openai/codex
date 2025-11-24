//! Envelope encryption.
//!
//! # Example
//!
//! ```rust
//! use openssl::rsa::Rsa;
//! use openssl::envelope::Seal;
//! use openssl::pkey::PKey;
//! use openssl::symm::Cipher;
//!
//! let rsa = Rsa::generate(2048).unwrap();
//! let key = PKey::from_rsa(rsa).unwrap();
//!
//! let cipher = Cipher::aes_256_cbc();
//! let mut seal = Seal::new(cipher, &[key]).unwrap();
//!
//! let secret = b"My secret message";
//! let mut encrypted = vec![0; secret.len() + cipher.block_size()];
//!
//! let mut enc_len = seal.update(secret, &mut encrypted).unwrap();
//! enc_len += seal.finalize(&mut encrypted[enc_len..]).unwrap();
//! encrypted.truncate(enc_len);
//! ```
use crate::cipher::CipherRef;
use crate::cipher_ctx::CipherCtx;
use crate::error::ErrorStack;
use crate::pkey::{HasPrivate, HasPublic, PKey, PKeyRef};
use crate::symm::Cipher;
use foreign_types::ForeignTypeRef;

/// Represents an EVP_Seal context.
pub struct Seal {
    ctx: CipherCtx,
    iv: Option<Vec<u8>>,
    enc_keys: Vec<Vec<u8>>,
}

impl Seal {
    /// Creates a new `Seal`.
    pub fn new<T>(cipher: Cipher, pub_keys: &[PKey<T>]) -> Result<Seal, ErrorStack>
    where
        T: HasPublic,
    {
        let mut iv = cipher.iv_len().map(|len| vec![0; len]);
        let mut enc_keys = vec![vec![]; pub_keys.len()];

        let mut ctx = CipherCtx::new()?;
        ctx.seal_init(
            Some(unsafe { CipherRef::from_ptr(cipher.as_ptr() as *mut _) }),
            pub_keys,
            &mut enc_keys,
            iv.as_deref_mut(),
        )?;

        Ok(Seal { ctx, iv, enc_keys })
    }

    /// Returns the initialization vector, if the cipher uses one.
    #[allow(clippy::option_as_ref_deref)]
    pub fn iv(&self) -> Option<&[u8]> {
        self.iv.as_ref().map(|v| &**v)
    }

    /// Returns the encrypted keys.
    pub fn encrypted_keys(&self) -> &[Vec<u8>] {
        &self.enc_keys
    }

    /// Feeds data from `input` through the cipher, writing encrypted bytes into `output`.
    ///
    /// The number of bytes written to `output` is returned. Note that this may
    /// not be equal to the length of `input`.
    ///
    /// # Panics
    ///
    /// Panics if `output.len() < input.len() + block_size` where `block_size` is
    /// the block size of the cipher (see `Cipher::block_size`), or if
    /// `output.len() > c_int::MAX`.
    pub fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, ErrorStack> {
        self.ctx.cipher_update(input, Some(output))
    }

    /// Finishes the encryption process, writing any remaining data to `output`.
    ///
    /// The number of bytes written to `output` is returned.
    ///
    /// `update` should not be called after this method.
    ///
    /// # Panics
    ///
    /// Panics if `output` is less than the cipher's block size.
    pub fn finalize(&mut self, output: &mut [u8]) -> Result<usize, ErrorStack> {
        self.ctx.cipher_final(output)
    }
}

/// Represents an EVP_Open context.
pub struct Open {
    ctx: CipherCtx,
}

impl Open {
    /// Creates a new `Open`.
    pub fn new<T>(
        cipher: Cipher,
        priv_key: &PKeyRef<T>,
        iv: Option<&[u8]>,
        encrypted_key: &[u8],
    ) -> Result<Open, ErrorStack>
    where
        T: HasPrivate,
    {
        let mut ctx = CipherCtx::new()?;
        ctx.open_init(
            Some(unsafe { CipherRef::from_ptr(cipher.as_ptr() as *mut _) }),
            encrypted_key,
            iv,
            Some(priv_key),
        )?;

        Ok(Open { ctx })
    }

    /// Feeds data from `input` through the cipher, writing decrypted bytes into `output`.
    ///
    /// The number of bytes written to `output` is returned. Note that this may
    /// not be equal to the length of `input`.
    ///
    /// # Panics
    ///
    /// Panics if `output.len() < input.len() + block_size` where
    /// `block_size` is the block size of the cipher (see `Cipher::block_size`),
    /// or if `output.len() > c_int::MAX`.
    pub fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, ErrorStack> {
        self.ctx.cipher_update(input, Some(output))
    }

    /// Finishes the decryption process, writing any remaining data to `output`.
    ///
    /// The number of bytes written to `output` is returned.
    ///
    /// `update` should not be called after this method.
    ///
    /// # Panics
    ///
    /// Panics if `output` is less than the cipher's block size.
    pub fn finalize(&mut self, output: &mut [u8]) -> Result<usize, ErrorStack> {
        self.ctx.cipher_final(output)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pkey::PKey;
    use crate::symm::Cipher;

    #[test]
    fn public_encrypt_private_decrypt() {
        let private_pem = include_bytes!("../test/rsa.pem");
        let public_pem = include_bytes!("../test/rsa.pem.pub");
        let private_key = PKey::private_key_from_pem(private_pem).unwrap();
        let public_key = PKey::public_key_from_pem(public_pem).unwrap();
        let cipher = Cipher::aes_256_cbc();
        let secret = b"My secret message";

        let mut seal = Seal::new(cipher, &[public_key]).unwrap();
        let mut encrypted = vec![0; secret.len() + cipher.block_size()];
        let mut enc_len = seal.update(secret, &mut encrypted).unwrap();
        enc_len += seal.finalize(&mut encrypted[enc_len..]).unwrap();
        let iv = seal.iv();
        let encrypted_key = &seal.encrypted_keys()[0];

        let mut open = Open::new(cipher, &private_key, iv, encrypted_key).unwrap();
        let mut decrypted = vec![0; enc_len + cipher.block_size()];
        let mut dec_len = open.update(&encrypted[..enc_len], &mut decrypted).unwrap();
        dec_len += open.finalize(&mut decrypted[dec_len..]).unwrap();

        assert_eq!(&secret[..], &decrypted[..dec_len]);
    }
}
