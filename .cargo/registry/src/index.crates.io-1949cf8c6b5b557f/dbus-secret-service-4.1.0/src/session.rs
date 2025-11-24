// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

// key exchange and crypto for session:
// 1. Before session negotiation (openSession), set private key and public key using DH method.
// 2. In session negotiation, send public key.
// 3. In session negotiation, exchange my public key for server's public key.
// 4. Use server public key and my private key to derive a shared AES key using HKDF.
// 5. Format Secret: aes iv is random seed, in secret struct it's the parameter (Array(Byte))
// 6. Format Secret: encode the secret value for the value field in secret struct.
//      This encoding uses the aes_key from the associated Session.

use dbus::{
    arg::{RefArg, Variant},
    blocking::{Connection, Proxy},
    Path,
};
use zeroize::ZeroizeOnDrop;

use crate::Error;

#[cfg(all(feature = "crypto-rust", feature = "crypto-openssl"))]
compile_error!("You cannot specify both feature \"crypto-rust\" and feature \"crypto-openssl\"");

/// The algorithms that can be used for encryption-in-transit.
///
/// If you are writing an ultra-secure program that accesses the secret service,
/// and you want to be sure that your secrets are encrypted while being sent to
/// or retrieved from the service, you can specify either the "crypto-rust" or
/// the "crypto-openssl" feature to this crate and tell it to use Diffie-Hellman
/// shared key encryption when passing secrets.  If you don't specify one of those
/// features, then your only choice is to use no encryption.
#[derive(Debug, Eq, PartialEq)]
pub enum EncryptionType {
    /// Use no encryption when sending/receiving secrets
    Plain,
    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    /// Use Diffie-Hellman shared key encryption when sending/receiving secrets
    Dh,
}

#[derive(ZeroizeOnDrop)]
pub(crate) struct EncryptedSecret {
    #[zeroize(skip)]
    path: Path<'static>, // the session path
    salt: Vec<u8>,           // the salt for the encrypted data
    data: Vec<u8>,           // the encrypted data
    pub(crate) mime: String, // the mime type of the decrypted data
}

impl EncryptedSecret {
    pub(crate) fn from_dbus(value: (Path<'static>, Vec<u8>, Vec<u8>, String)) -> Self {
        Self {
            path: value.0,
            salt: value.1,
            data: value.2,
            mime: value.3.to_string(),
        }
    }

    pub(crate) fn to_dbus(&self) -> (Path<'static>, Vec<u8>, Vec<u8>, &str) {
        (
            self.path.clone(),
            self.salt.clone(),
            self.data.clone(),
            &self.mime,
        )
    }
}

#[derive(ZeroizeOnDrop)]
pub struct Session {
    #[zeroize(skip)]
    pub(crate) path: Path<'static>,
    #[zeroize(skip)]
    encryption: EncryptionType,
    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    shared_key: Option<crypto::AesKey>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("path", &self.path)
            .field(
                "secrets",
                if self.is_encrypted() {
                    &"(Hidden)"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}

impl Session {
    pub fn new(p: Proxy<'_, &'_ Connection>, encryption: EncryptionType) -> Result<Session, Error> {
        use crate::proxy::service::Service;
        match encryption {
            EncryptionType::Plain => {
                use crate::ss::ALGORITHM_PLAIN;
                // in rust 1.70, this lint applies here even though it shouldn't
                // because we need an explicit string to interpret as a RefArg
                #[allow(clippy::box_default)]
                let bytes_arg = Box::new(String::new()) as Box<dyn RefArg>;
                let (_, path) = p.open_session(ALGORITHM_PLAIN, Variant(bytes_arg))?;
                Ok(Session {
                    path,
                    encryption,
                    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
                    shared_key: None,
                })
            }
            #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
            EncryptionType::Dh => {
                use crate::ss::ALGORITHM_DH;
                use dbus::arg::cast;

                // crypto: create private and public key
                let keypair = crypto::Keypair::generate();

                // send our public key with algorithm to service
                let public_bytes = keypair.public.to_bytes_be();
                let bytes_arg = Variant(Box::new(public_bytes) as Box<dyn RefArg>);
                let (out, path) = p.open_session(ALGORITHM_DH, bytes_arg)?;

                // get service public key back and create shared key from it
                if let Some(server_public_key_bytes) = cast::<Vec<u8>>(&out.0) {
                    let shared_key = keypair.derive_shared(server_public_key_bytes);
                    Ok(Session {
                        path,
                        encryption,
                        #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
                        shared_key: Some(shared_key),
                    })
                } else {
                    Err(Error::Parse)
                }
            }
        }
    }

    pub fn is_encrypted(&self) -> bool {
        match self.encryption {
            EncryptionType::Plain => false,
            #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
            EncryptionType::Dh => true,
        }
    }

    pub(crate) fn encrypt_secret(&self, data: &[u8], mime: &str) -> EncryptedSecret {
        match self.encryption {
            EncryptionType::Plain => EncryptedSecret {
                path: self.path.clone(),
                salt: vec![],
                data: data.to_vec(),
                mime: mime.to_string(),
            },
            #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
            EncryptionType::Dh => {
                // encrypt the secret with the data
                let (encrypted, salt) = crypto::encrypt(data, &self.shared_key.unwrap());
                EncryptedSecret {
                    path: self.path.clone(),
                    salt,
                    data: encrypted,
                    mime: mime.to_string(),
                }
            }
        }
    }

    pub(crate) fn decrypt_secret(&self, secret: EncryptedSecret) -> Result<Vec<u8>, Error> {
        match self.encryption {
            EncryptionType::Plain => Ok(secret.data.clone()),
            #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
            EncryptionType::Dh => {
                let clear = crypto::decrypt(&secret.data, &self.shared_key.unwrap(), &secret.salt)?;
                Ok(clear)
            }
        }
    }
}

#[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
mod crypto {
    use std::ops::{Mul, Rem, Shr};

    use fastrand::Rng;
    use num::{
        bigint::BigUint,
        integer::Integer,
        traits::{One, Zero},
        FromPrimitive,
    };
    use once_cell::sync::Lazy;

    #[cfg(feature = "crypto-rust")]
    pub(super) fn encrypt(data: &[u8], key: &AesKey) -> (Vec<u8>, Vec<u8>) {
        use aes::cipher::block_padding::Pkcs7;
        use aes::cipher::generic_array::GenericArray;
        use aes::cipher::{BlockEncryptMut, KeyIvInit};

        type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

        // create the salt for the encryption
        let aes_iv = salt();

        // convert key and salt to input parameter form
        let key = GenericArray::from_slice(key);
        let iv = GenericArray::from_slice(&aes_iv);

        // return encrypted data and salt
        (
            Aes128CbcEnc::new(key, iv).encrypt_padded_vec_mut::<Pkcs7>(data),
            aes_iv.to_vec(),
        )
    }

    #[cfg(feature = "crypto-rust")]
    pub(super) fn decrypt(
        encrypted_data: &[u8],
        key: &AesKey,
        iv: &[u8],
    ) -> Result<Vec<u8>, crate::Error> {
        use aes::cipher::block_padding::Pkcs7;
        use aes::cipher::generic_array::GenericArray;
        use aes::cipher::{BlockDecryptMut, KeyIvInit};

        type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

        let key = GenericArray::from_slice(key);
        let iv = GenericArray::from_slice(iv);

        let output = Aes128CbcDec::new(key, iv).decrypt_padded_vec_mut::<Pkcs7>(encrypted_data)?;
        Ok(output)
    }

    #[cfg(all(feature = "crypto-openssl", not(feature = "crypto-rust")))]
    pub(super) fn encrypt(data: &[u8], key: &AesKey) -> (Vec<u8>, Vec<u8>) {
        use openssl::cipher::Cipher;
        use openssl::cipher_ctx::CipherCtx;

        // create the salt for the encryption
        let aes_iv = salt();

        let mut ctx = CipherCtx::new().expect("cipher creation should not fail");
        ctx.encrypt_init(Some(Cipher::aes_128_cbc()), Some(key), Some(&aes_iv))
            .expect("cipher init should not fail");

        let mut output = vec![];
        ctx.cipher_update_vec(data, &mut output)
            .expect("cipher update should not fail");
        ctx.cipher_final_vec(&mut output)
            .expect("cipher final should not fail");
        (output, aes_iv.to_vec())
    }

    #[cfg(all(feature = "crypto-openssl", not(feature = "crypto-rust")))]
    pub(super) fn decrypt(
        encrypted_data: &[u8],
        key: &AesKey,
        iv: &[u8],
    ) -> Result<Vec<u8>, crate::Error> {
        use openssl::cipher::Cipher;
        use openssl::cipher_ctx::CipherCtx;

        let mut ctx = CipherCtx::new().expect("cipher creation should not fail");
        ctx.decrypt_init(Some(Cipher::aes_128_cbc()), Some(key), Some(iv))
            .expect("cipher init should not fail");

        let mut output = vec![];
        ctx.cipher_update_vec(encrypted_data, &mut output)?;
        ctx.cipher_final_vec(&mut output)?;
        Ok(output)
    }

    fn salt() -> [u8; 16] {
        let mut rng = Rng::new();
        let mut salt = [0; 16];
        rng.fill(&mut salt);
        salt
    }

    // for key exchange
    static DH_GENERATOR: Lazy<BigUint> = Lazy::new(|| BigUint::from_u64(0x2).unwrap());
    static DH_PRIME: Lazy<BigUint> = Lazy::new(|| {
        BigUint::from_bytes_be(&[
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68,
            0xC2, 0x34, 0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1, 0x29, 0x02, 0x4E, 0x08,
            0x8A, 0x67, 0xCC, 0x74, 0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A,
            0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD, 0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B,
            0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37, 0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51,
            0xC2, 0x45, 0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6, 0xF4, 0x4C, 0x42, 0xE9,
            0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED, 0xEE, 0x38,
            0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE, 0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6,
            0x49, 0x28, 0x66, 0x51, 0xEC, 0xE6, 0x53, 0x81, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ])
    });

    pub(super) type AesKey = [u8; 16];

    #[derive(Clone)]
    pub(super) struct Keypair {
        pub(super) private: BigUint,
        pub(super) public: BigUint,
    }

    impl Keypair {
        pub(super) fn generate() -> Self {
            let mut rng = Rng::new();
            let mut private_key_bytes = [0; 128];
            rng.fill(&mut private_key_bytes);

            let private_key = BigUint::from_bytes_be(&private_key_bytes);
            let public_key = pow_base_exp_mod(&DH_GENERATOR, &private_key, &DH_PRIME);

            Self {
                private: private_key,
                public: public_key,
            }
        }

        pub(super) fn derive_shared(&self, server_public_key_bytes: &[u8]) -> AesKey {
            // Derive the shared secret the server and us.
            let server_public_key = BigUint::from_bytes_be(server_public_key_bytes);
            let common_secret = pow_base_exp_mod(&server_public_key, &self.private, &DH_PRIME);

            let common_secret_bytes = common_secret.to_bytes_be();
            let mut common_secret_padded = vec![0; 128 - common_secret_bytes.len()];
            common_secret_padded.extend(common_secret_bytes);

            // hkdf

            // input keying material
            let ikm = common_secret_padded;
            let salt = None;

            // output keying material
            let mut okm = [0; 16];
            hkdf(ikm, salt, &mut okm);

            okm
        }
    }

    #[cfg(all(feature = "crypto-openssl", not(feature = "crypto-rust")))]
    pub(super) fn hkdf(ikm: Vec<u8>, salt: Option<&[u8]>, okm: &mut [u8]) {
        let mut ctx = openssl::pkey_ctx::PkeyCtx::new_id(openssl::pkey::Id::HKDF)
            .expect("hkdf context should not fail");
        ctx.derive_init().expect("hkdf derive init should not fail");
        ctx.set_hkdf_md(openssl::md::Md::sha256())
            .expect("hkdf set md should not fail");

        ctx.set_hkdf_key(&ikm)
            .expect("hkdf set key should not fail");
        if let Some(salt) = salt {
            ctx.set_hkdf_salt(salt)
                .expect("hkdf set salt should not fail");
        }

        ctx.add_hkdf_info(&[]).unwrap();
        ctx.derive(Some(okm))
            .expect("hkdf expand should never fail");
    }

    #[cfg(feature = "crypto-rust")]
    pub(super) fn hkdf(ikm: Vec<u8>, salt: Option<&[u8]>, okm: &mut [u8]) {
        use sha2::Sha256;

        let info = [];
        let (_, hk) = hkdf::Hkdf::<Sha256>::extract(salt, &ikm);
        hk.expand(&info, okm)
            .expect("hkdf expand should never fail");
    }

    /// from https://github.com/plietar/librespot/blob/master/core/src/util/mod.rs#L53
    pub(super) fn pow_base_exp_mod(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
        let mut base = base.clone();
        let mut exp = exp.clone();
        let mut result: BigUint = One::one();

        while !exp.is_zero() {
            if exp.is_odd() {
                result = result.mul(&base).rem(modulus);
            }
            exp = exp.shr(1);
            base = (&base).mul(&base).rem(modulus);
        }

        result
    }
}

#[cfg(test)]
mod test {
    use dbus::blocking::Connection;

    use crate::proxy::new_proxy;
    use crate::ss::SS_DBUS_PATH;

    use super::*;

    #[test]
    fn should_create_plain_session() {
        let connection = Connection::new_session().unwrap();
        let proxy = new_proxy(&connection, SS_DBUS_PATH);
        let session = Session::new(proxy, EncryptionType::Plain).unwrap();
        assert!(!session.is_encrypted());
    }

    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    #[test]
    fn should_create_encrypted_session() {
        let connection = Connection::new_session().unwrap();
        let proxy = new_proxy(&connection, SS_DBUS_PATH);
        let session = Session::new(proxy, EncryptionType::Dh).unwrap();
        assert!(session.is_encrypted());
    }
}
