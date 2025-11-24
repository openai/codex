#[cfg(all(feature = "aws-lc-rs", not(feature = "ring")))]
use aws_lc_rs::{aead, error, hkdf, hmac};
#[cfg(feature = "ring")]
use ring::{aead, error, hkdf, hmac};

use crate::crypto::{self, CryptoError};

impl crypto::HmacKey for hmac::Key {
    fn sign(&self, data: &[u8], out: &mut [u8]) {
        out.copy_from_slice(hmac::sign(self, data).as_ref());
    }

    fn signature_len(&self) -> usize {
        32
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
        Ok(hmac::verify(self, data, signature)?)
    }
}

impl crypto::HandshakeTokenKey for hkdf::Prk {
    fn aead_from_hkdf(&self, random_bytes: &[u8]) -> Box<dyn crypto::AeadKey> {
        let mut key_buffer = [0u8; 32];
        let info = [random_bytes];
        let okm = self.expand(&info, hkdf::HKDF_SHA256).unwrap();

        okm.fill(&mut key_buffer).unwrap();

        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_buffer).unwrap();
        Box::new(aead::LessSafeKey::new(key))
    }
}

impl crypto::AeadKey for aead::LessSafeKey {
    fn seal(&self, data: &mut Vec<u8>, additional_data: &[u8]) -> Result<(), CryptoError> {
        let aad = aead::Aad::from(additional_data);
        let zero_nonce = aead::Nonce::assume_unique_for_key([0u8; 12]);
        Ok(self.seal_in_place_append_tag(zero_nonce, aad, data)?)
    }

    fn open<'a>(
        &self,
        data: &'a mut [u8],
        additional_data: &[u8],
    ) -> Result<&'a mut [u8], CryptoError> {
        let aad = aead::Aad::from(additional_data);
        let zero_nonce = aead::Nonce::assume_unique_for_key([0u8; 12]);
        Ok(self.open_in_place(zero_nonce, aad, data)?)
    }
}

impl From<error::Unspecified> for CryptoError {
    fn from(_: error::Unspecified) -> Self {
        Self
    }
}
