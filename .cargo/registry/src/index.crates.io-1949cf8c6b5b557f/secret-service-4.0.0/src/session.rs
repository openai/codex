//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

// key exchange and crypto for session:
// 1. Before session negotiation (openSession), set private key and public key using DH method.
// 2. In session negotiation, send public key.
// 3. As result of session negotiation, get object path for session, which (I think
//      it means that it uses the same server public key to create an aes key which is used
//      to decode the encoded secret using the aes seed that's sent with the secret).
// 4. As result of session negotition, get server public key.
// 5. Use server public key, my private key, to set an aes key using HKDF.
// 6. Format Secret: aes iv is random seed, in secret struct it's the parameter (Array(Byte))
// 7. Format Secret: encode the secret value for the value field in secret struct.
//      This encoding uses the aes_key from the associated Session.

use crate::proxy::service::{OpenSessionResult, ServiceProxy, ServiceProxyBlocking};
use crate::ss::{ALGORITHM_DH, ALGORITHM_PLAIN};
use crate::Error;

use generic_array::{typenum::U16, GenericArray};
use num::{
    bigint::BigUint,
    integer::Integer,
    traits::{One, Zero},
    FromPrimitive,
};
use once_cell::sync::Lazy;
use rand::{rngs::OsRng, Rng};
use zbus::zvariant::OwnedObjectPath;

use std::ops::{Mul, Rem, Shr};

// for key exchange
static DH_GENERATOR: Lazy<BigUint> = Lazy::new(|| BigUint::from_u64(0x2).unwrap());
static DH_PRIME: Lazy<BigUint> = Lazy::new(|| {
    BigUint::from_bytes_be(&[
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68, 0xC2,
        0x34, 0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1, 0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67,
        0xCC, 0x74, 0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A, 0x08, 0x79, 0x8E,
        0x34, 0x04, 0xDD, 0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B, 0x30, 0x2B, 0x0A, 0x6D,
        0xF2, 0x5F, 0x14, 0x37, 0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45, 0xE4, 0x85, 0xB5,
        0x76, 0x62, 0x5E, 0x7E, 0xC6, 0xF4, 0x4C, 0x42, 0xE9, 0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF,
        0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED, 0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE,
        0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6, 0x49, 0x28, 0x66, 0x51, 0xEC, 0xE6, 0x53, 0x81,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ])
});

#[allow(unused_macros)]
macro_rules! feature_needed {
    () => {
        compile_error!("Please enable a feature to pick a runtime (such as rt-async-io-crypto-rust or rt-tokio-crypto-rust) for the secret-service crate")
    }
}

type AesKey = GenericArray<u8, U16>;

#[derive(Debug, Eq, PartialEq)]
pub enum EncryptionType {
    Plain,
    Dh,
}

struct Keypair {
    private: BigUint,
    public: BigUint,
}

impl Keypair {
    fn generate() -> Self {
        let mut rng = OsRng {};
        let mut private_key_bytes = [0; 128];
        rng.fill(&mut private_key_bytes);

        let private_key = BigUint::from_bytes_be(&private_key_bytes);
        let public_key = powm(&DH_GENERATOR, &private_key, &DH_PRIME);

        Self {
            private: private_key,
            public: public_key,
        }
    }

    fn derive_shared(&self, server_public_key: &BigUint) -> AesKey {
        // Derive the shared secret the server and us.
        let common_secret = powm(server_public_key, &self.private, &DH_PRIME);

        let mut common_secret_bytes = common_secret.to_bytes_be();
        let mut common_secret_padded = vec![0; 128 - common_secret_bytes.len()];
        common_secret_padded.append(&mut common_secret_bytes);

        // hkdf

        // input keying material
        let ikm = common_secret_padded;
        let salt = None;

        // output keying material
        let mut okm = [0; 16];
        hkdf(ikm, salt, &mut okm);

        GenericArray::clone_from_slice(&okm)
    }
}

#[cfg(feature = "crypto-openssl")]
fn hkdf(ikm: Vec<u8>, salt: Option<&[u8]>, okm: &mut [u8]) {
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
fn hkdf(ikm: Vec<u8>, salt: Option<&[u8]>, okm: &mut [u8]) {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let info = [];
    let (_, hk) = Hkdf::<Sha256>::extract(salt, &ikm);
    hk.expand(&info, okm)
        .expect("hkdf expand should never fail");
}

#[cfg(all(not(feature = "crypto-rust"), not(feature = "crypto-openssl")))]
fn hkdf(ikm: Vec<u8>, salt: Option<&[u8]>, okm: &mut [u8]) {
    feature_needed!()
}

pub struct Session {
    pub object_path: OwnedObjectPath,
    aes_key: Option<AesKey>,
}

impl Session {
    fn encrypted_session(keypair: &Keypair, session: OpenSessionResult) -> Result<Self, Error> {
        let server_public_key = session
            .output
            .try_into()
            .map(|key: Vec<u8>| BigUint::from_bytes_be(&key))?;

        let aes_key = keypair.derive_shared(&server_public_key);

        Ok(Session {
            object_path: session.result,
            aes_key: Some(aes_key),
        })
    }

    pub fn new_blocking(
        service_proxy: &ServiceProxyBlocking,
        encryption: EncryptionType,
    ) -> Result<Self, Error> {
        match encryption {
            EncryptionType::Plain => {
                let session = service_proxy.open_session(ALGORITHM_PLAIN, "".into())?;
                let session_path = session.result;

                Ok(Session {
                    object_path: session_path,
                    aes_key: None,
                })
            }
            EncryptionType::Dh => {
                let keypair = Keypair::generate();

                let session = service_proxy
                    .open_session(ALGORITHM_DH, keypair.public.to_bytes_be().into())?;

                Self::encrypted_session(&keypair, session)
            }
        }
    }

    pub async fn new(
        service_proxy: &ServiceProxy<'_>,
        encryption: EncryptionType,
    ) -> Result<Self, Error> {
        match encryption {
            EncryptionType::Plain => {
                let session = service_proxy
                    .open_session(ALGORITHM_PLAIN, "".into())
                    .await?;
                let session_path = session.result;

                Ok(Session {
                    object_path: session_path,
                    aes_key: None,
                })
            }
            EncryptionType::Dh => {
                let keypair = Keypair::generate();

                let session = service_proxy
                    .open_session(ALGORITHM_DH, keypair.public.to_bytes_be().into())
                    .await?;

                Self::encrypted_session(&keypair, session)
            }
        }
    }

    pub fn get_aes_key(&self) -> Option<&AesKey> {
        self.aes_key.as_ref()
    }
}

/// from https://github.com/plietar/librespot/blob/master/core/src/util/mod.rs#L53
fn powm(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
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

#[cfg(feature = "crypto-rust")]
pub fn encrypt(data: &[u8], key: &AesKey, iv: &[u8]) -> Vec<u8> {
    use aes::cipher::block_padding::Pkcs7;
    use aes::cipher::{BlockEncryptMut, KeyIvInit};

    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

    let iv = GenericArray::from_slice(iv);

    Aes128CbcEnc::new(key, iv).encrypt_padded_vec_mut::<Pkcs7>(data)
}

#[cfg(feature = "crypto-rust")]
pub fn decrypt(encrypted_data: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, Error> {
    use aes::cipher::block_padding::Pkcs7;
    use aes::cipher::{BlockDecryptMut, KeyIvInit};

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    let iv = GenericArray::from_slice(iv);
    Aes128CbcDec::new(key, iv)
        .decrypt_padded_vec_mut::<Pkcs7>(encrypted_data)
        .map_err(|_| Error::Crypto("message decryption failed"))
}

#[cfg(feature = "crypto-openssl")]
pub fn encrypt(data: &[u8], key: &AesKey, iv: &[u8]) -> Vec<u8> {
    use openssl::cipher::Cipher;
    use openssl::cipher_ctx::CipherCtx;

    let mut ctx = CipherCtx::new().expect("cipher creation should not fail");
    ctx.encrypt_init(Some(Cipher::aes_128_cbc()), Some(key), Some(iv))
        .expect("cipher init should not fail");

    let mut output = vec![];
    ctx.cipher_update_vec(data, &mut output)
        .expect("cipher update should not fail");
    ctx.cipher_final_vec(&mut output)
        .expect("cipher final should not fail");
    output
}

#[cfg(feature = "crypto-openssl")]
pub fn decrypt(encrypted_data: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, Error> {
    use openssl::cipher::Cipher;
    use openssl::cipher_ctx::CipherCtx;

    let mut ctx = CipherCtx::new().expect("cipher creation should not fail");
    ctx.decrypt_init(Some(Cipher::aes_128_cbc()), Some(key), Some(iv))
        .expect("cipher init should not fail");

    let mut output = vec![];
    ctx.cipher_update_vec(encrypted_data, &mut output)
        .map_err(|_| Error::Crypto("message decryption failed"))?;
    ctx.cipher_final_vec(&mut output)
        .map_err(|_| Error::Crypto("message decryption failed"))?;
    Ok(output)
}

#[cfg(all(not(feature = "crypto-rust"), not(feature = "crypto-openssl")))]
pub fn encrypt(data: &[u8], key: &AesKey, iv: &[u8]) -> Vec<u8> {
    feature_needed!()
}

#[cfg(all(not(feature = "crypto-rust"), not(feature = "crypto-openssl")))]
pub fn decrypt(encrypted_data: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, Error> {
    feature_needed!()
}

#[cfg(test)]
mod test {
    use super::*;

    // There is no async test because this tests that an encryption session can be made, nothing more.

    #[test]
    fn should_create_plain_session() {
        let conn = zbus::blocking::Connection::session().unwrap();
        let service_proxy = ServiceProxyBlocking::new(&conn).unwrap();
        let session = Session::new_blocking(&service_proxy, EncryptionType::Plain).unwrap();
        assert!(session.get_aes_key().is_none());
    }

    #[test]
    fn should_create_encrypted_session() {
        let conn = zbus::blocking::Connection::session().unwrap();
        let service_proxy = ServiceProxyBlocking::new(&conn).unwrap();
        let session = Session::new_blocking(&service_proxy, EncryptionType::Dh).unwrap();
        assert!(session.get_aes_key().is_some());
    }
}
