use alloc::boxed::Box;

use aws_lc_rs::{aead, tls_prf};

use crate::crypto::cipher::{
    AeadKey, InboundOpaqueMessage, Iv, KeyBlockShape, MessageDecrypter, MessageEncrypter,
    NONCE_LEN, Nonce, Tls12AeadAlgorithm, UnsupportedOperationError, make_tls12_aad,
};
use crate::crypto::tls12::Prf;
use crate::crypto::{ActiveKeyExchange, KeyExchangeAlgorithm};
use crate::enums::{CipherSuite, SignatureScheme};
use crate::error::Error;
use crate::msgs::fragmenter::MAX_FRAGMENT_LEN;
use crate::msgs::message::{
    InboundPlainMessage, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload,
};
use crate::suites::{CipherSuiteCommon, ConnectionTrafficSecrets, SupportedCipherSuite};
use crate::tls12::Tls12CipherSuite;
use crate::version::TLS12;

/// The TLS1.2 ciphersuite TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256.
pub static TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            hash_provider: &super::hash::SHA256,
            confidentiality_limit: u64::MAX,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_ECDSA_SCHEMES,
        aead_alg: &ChaCha20Poly1305,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA256),
    });

/// The TLS1.2 ciphersuite TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
pub static TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
            hash_provider: &super::hash::SHA256,
            confidentiality_limit: u64::MAX,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_RSA_SCHEMES,
        aead_alg: &ChaCha20Poly1305,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA256),
    });

/// The TLS1.2 ciphersuite TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
pub static TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            hash_provider: &super::hash::SHA256,
            confidentiality_limit: 1 << 24,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_RSA_SCHEMES,
        aead_alg: &AES128_GCM,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA256),
    });

/// The TLS1.2 ciphersuite TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
pub static TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
            hash_provider: &super::hash::SHA384,
            confidentiality_limit: 1 << 24,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_RSA_SCHEMES,
        aead_alg: &AES256_GCM,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA384),
    });

/// The TLS1.2 ciphersuite TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
pub static TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
            hash_provider: &super::hash::SHA256,
            confidentiality_limit: 1 << 24,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_ECDSA_SCHEMES,
        aead_alg: &AES128_GCM,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA256),
    });

/// The TLS1.2 ciphersuite TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
pub static TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
            hash_provider: &super::hash::SHA384,
            confidentiality_limit: 1 << 24,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: TLS12_ECDSA_SCHEMES,
        aead_alg: &AES256_GCM,
        prf_provider: &Tls12Prf(&tls_prf::P_SHA384),
    });

static TLS12_ECDSA_SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::ED25519,
    SignatureScheme::ECDSA_NISTP521_SHA512,
    SignatureScheme::ECDSA_NISTP384_SHA384,
    SignatureScheme::ECDSA_NISTP256_SHA256,
];

static TLS12_RSA_SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::RSA_PSS_SHA512,
    SignatureScheme::RSA_PSS_SHA384,
    SignatureScheme::RSA_PSS_SHA256,
    SignatureScheme::RSA_PKCS1_SHA512,
    SignatureScheme::RSA_PKCS1_SHA384,
    SignatureScheme::RSA_PKCS1_SHA256,
];

pub(crate) static AES128_GCM: GcmAlgorithm = GcmAlgorithm(&aead::AES_128_GCM);
pub(crate) static AES256_GCM: GcmAlgorithm = GcmAlgorithm(&aead::AES_256_GCM);

pub(crate) struct GcmAlgorithm(&'static aead::Algorithm);

impl Tls12AeadAlgorithm for GcmAlgorithm {
    fn decrypter(&self, dec_key: AeadKey, dec_iv: &[u8]) -> Box<dyn MessageDecrypter> {
        // safety: see `encrypter()`.
        let dec_key =
            aead::TlsRecordOpeningKey::new(self.0, aead::TlsProtocolId::TLS12, dec_key.as_ref())
                .unwrap();

        let mut ret = GcmMessageDecrypter {
            dec_key,
            dec_salt: [0u8; 4],
        };

        debug_assert_eq!(dec_iv.len(), 4);
        ret.dec_salt.copy_from_slice(dec_iv);
        Box::new(ret)
    }

    fn encrypter(
        &self,
        enc_key: AeadKey,
        write_iv: &[u8],
        explicit: &[u8],
    ) -> Box<dyn MessageEncrypter> {
        // safety: `TlsRecordSealingKey::new` fails if
        // - `enc_key`'s length is wrong for `algorithm`.  But the length is defined by
        //   `algorithm.key_len()` in `key_block_shape()`, below.
        // - `algorithm` is not supported: but `AES_128_GCM` and `AES_256_GCM` is.
        // thus, this `unwrap()` is unreachable.
        //
        // `TlsProtocolId::TLS13` is deliberate: we reuse the nonce construction from
        // RFC7905 and TLS13: a random starting point, XOR'd with the sequence number.  This means
        // `TlsProtocolId::TLS12` (which wants to see a plain sequence number) is unsuitable.
        //
        // The most important property is that nonce is unique per key, which is satisfied by
        // this construction, even if the nonce is not monotonically increasing.
        let enc_key =
            aead::TlsRecordSealingKey::new(self.0, aead::TlsProtocolId::TLS13, enc_key.as_ref())
                .unwrap();
        let iv = gcm_iv(write_iv, explicit);
        Box::new(GcmMessageEncrypter { enc_key, iv })
    }

    fn key_block_shape(&self) -> KeyBlockShape {
        KeyBlockShape {
            enc_key_len: self.0.key_len(),
            fixed_iv_len: 4,
            explicit_nonce_len: 8,
        }
    }

    fn extract_keys(
        &self,
        key: AeadKey,
        write_iv: &[u8],
        explicit: &[u8],
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        let iv = gcm_iv(write_iv, explicit);
        Ok(match self.0.key_len() {
            16 => ConnectionTrafficSecrets::Aes128Gcm { key, iv },
            32 => ConnectionTrafficSecrets::Aes256Gcm { key, iv },
            _ => unreachable!(),
        })
    }

    fn fips(&self) -> bool {
        super::fips()
    }
}

pub(crate) struct ChaCha20Poly1305;

impl Tls12AeadAlgorithm for ChaCha20Poly1305 {
    fn decrypter(&self, dec_key: AeadKey, iv: &[u8]) -> Box<dyn MessageDecrypter> {
        let dec_key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::CHACHA20_POLY1305, dec_key.as_ref()).unwrap(),
        );
        Box::new(ChaCha20Poly1305MessageDecrypter {
            dec_key,
            dec_offset: Iv::copy(iv),
        })
    }

    fn encrypter(&self, enc_key: AeadKey, enc_iv: &[u8], _: &[u8]) -> Box<dyn MessageEncrypter> {
        let enc_key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::CHACHA20_POLY1305, enc_key.as_ref()).unwrap(),
        );
        Box::new(ChaCha20Poly1305MessageEncrypter {
            enc_key,
            enc_offset: Iv::copy(enc_iv),
        })
    }

    fn key_block_shape(&self) -> KeyBlockShape {
        KeyBlockShape {
            enc_key_len: 32,
            fixed_iv_len: 12,
            explicit_nonce_len: 0,
        }
    }

    fn extract_keys(
        &self,
        key: AeadKey,
        iv: &[u8],
        _explicit: &[u8],
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        // This should always be true because KeyBlockShape and the Iv nonce len are in agreement.
        debug_assert_eq!(aead::NONCE_LEN, iv.len());
        Ok(ConnectionTrafficSecrets::Chacha20Poly1305 {
            key,
            iv: Iv::new(iv[..].try_into().unwrap()),
        })
    }

    fn fips(&self) -> bool {
        false // not FIPS approved
    }
}

/// A `MessageEncrypter` for AES-GCM AEAD ciphersuites. TLS 1.2 only.
struct GcmMessageEncrypter {
    enc_key: aead::TlsRecordSealingKey,
    iv: Iv,
}

/// A `MessageDecrypter` for AES-GCM AEAD ciphersuites.  TLS1.2 only.
struct GcmMessageDecrypter {
    dec_key: aead::TlsRecordOpeningKey,
    dec_salt: [u8; 4],
}

const GCM_EXPLICIT_NONCE_LEN: usize = 8;
const GCM_OVERHEAD: usize = GCM_EXPLICIT_NONCE_LEN + 16;

impl MessageDecrypter for GcmMessageDecrypter {
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, Error> {
        let payload = &msg.payload;
        if payload.len() < GCM_OVERHEAD {
            return Err(Error::DecryptError);
        }

        let nonce = {
            let mut nonce = [0u8; 12];
            nonce[..4].copy_from_slice(&self.dec_salt);
            nonce[4..].copy_from_slice(&payload[..8]);
            aead::Nonce::assume_unique_for_key(nonce)
        };

        let aad = aead::Aad::from(make_tls12_aad(
            seq,
            msg.typ,
            msg.version,
            payload.len() - GCM_OVERHEAD,
        ));

        let payload = &mut msg.payload;
        let plain_len = self
            .dec_key
            .open_in_place(nonce, aad, &mut payload[GCM_EXPLICIT_NONCE_LEN..])
            .map_err(|_| Error::DecryptError)?
            .len();

        if plain_len > MAX_FRAGMENT_LEN {
            return Err(Error::PeerSentOversizedRecord);
        }

        Ok(
            msg.into_plain_message_range(
                GCM_EXPLICIT_NONCE_LEN..GCM_EXPLICIT_NONCE_LEN + plain_len,
            ),
        )
    }
}

impl MessageEncrypter for GcmMessageEncrypter {
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, Error> {
        let total_len = self.encrypted_payload_len(msg.payload.len());
        let mut payload = PrefixedPayload::with_capacity(total_len);

        let nonce = aead::Nonce::assume_unique_for_key(Nonce::new(&self.iv, seq).0);
        let aad = aead::Aad::from(make_tls12_aad(seq, msg.typ, msg.version, msg.payload.len()));
        payload.extend_from_slice(&nonce.as_ref()[4..]);
        payload.extend_from_chunks(&msg.payload);

        self.enc_key
            .seal_in_place_separate_tag(nonce, aad, &mut payload.as_mut()[GCM_EXPLICIT_NONCE_LEN..])
            .map(|tag| payload.extend_from_slice(tag.as_ref()))
            .map_err(|_| Error::EncryptError)?;

        Ok(OutboundOpaqueMessage::new(msg.typ, msg.version, payload))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len + GCM_EXPLICIT_NONCE_LEN + self.enc_key.algorithm().tag_len()
    }
}

/// The RFC7905/RFC7539 ChaCha20Poly1305 construction.
/// This implementation does the AAD construction required in TLS1.2.
/// TLS1.3 uses `TLS13MessageEncrypter`.
struct ChaCha20Poly1305MessageEncrypter {
    enc_key: aead::LessSafeKey,
    enc_offset: Iv,
}

/// The RFC7905/RFC7539 ChaCha20Poly1305 construction.
/// This implementation does the AAD construction required in TLS1.2.
/// TLS1.3 uses `TLS13MessageDecrypter`.
struct ChaCha20Poly1305MessageDecrypter {
    dec_key: aead::LessSafeKey,
    dec_offset: Iv,
}

const CHACHAPOLY1305_OVERHEAD: usize = 16;

impl MessageDecrypter for ChaCha20Poly1305MessageDecrypter {
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, Error> {
        let payload = &msg.payload;

        if payload.len() < CHACHAPOLY1305_OVERHEAD {
            return Err(Error::DecryptError);
        }

        let nonce = aead::Nonce::assume_unique_for_key(Nonce::new(&self.dec_offset, seq).0);
        let aad = aead::Aad::from(make_tls12_aad(
            seq,
            msg.typ,
            msg.version,
            payload.len() - CHACHAPOLY1305_OVERHEAD,
        ));

        let payload = &mut msg.payload;
        let plain_len = self
            .dec_key
            .open_in_place(nonce, aad, payload)
            .map_err(|_| Error::DecryptError)?
            .len();

        if plain_len > MAX_FRAGMENT_LEN {
            return Err(Error::PeerSentOversizedRecord);
        }

        payload.truncate(plain_len);
        Ok(msg.into_plain_message())
    }
}

impl MessageEncrypter for ChaCha20Poly1305MessageEncrypter {
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, Error> {
        let total_len = self.encrypted_payload_len(msg.payload.len());
        let mut payload = PrefixedPayload::with_capacity(total_len);

        let nonce = aead::Nonce::assume_unique_for_key(Nonce::new(&self.enc_offset, seq).0);
        let aad = aead::Aad::from(make_tls12_aad(seq, msg.typ, msg.version, msg.payload.len()));
        payload.extend_from_chunks(&msg.payload);

        self.enc_key
            .seal_in_place_append_tag(nonce, aad, &mut payload)
            .map_err(|_| Error::EncryptError)?;

        Ok(OutboundOpaqueMessage::new(msg.typ, msg.version, payload))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len + self.enc_key.algorithm().tag_len()
    }
}

fn gcm_iv(write_iv: &[u8], explicit: &[u8]) -> Iv {
    debug_assert_eq!(write_iv.len(), 4);
    debug_assert_eq!(explicit.len(), 8);

    // The GCM nonce is constructed from a 32-bit 'salt' derived
    // from the master-secret, and a 64-bit explicit part,
    // with no specified construction.  Thanks for that.
    //
    // We use the same construction as TLS1.3/ChaCha20Poly1305:
    // a starting point extracted from the key block, xored with
    // the sequence number.
    let mut iv = [0; NONCE_LEN];
    iv[..4].copy_from_slice(write_iv);
    iv[4..].copy_from_slice(explicit);

    Iv::new(iv)
}

struct Tls12Prf(&'static tls_prf::Algorithm);

impl Prf for Tls12Prf {
    fn for_secret(&self, output: &mut [u8], secret: &[u8], label: &[u8], seed: &[u8]) {
        // safety:
        // - [1] is safe because our caller guarantees `secret` is non-empty; this is
        //   the only documented error case.
        // - [2] is safe in practice because the only failure from `derive()` is due
        //   to zero `output.len()`; this is outlawed at higher levels
        let derived = tls_prf::Secret::new(self.0, secret)
            .unwrap() // [1]
            .derive(label, seed, output.len())
            .unwrap(); // [2]
        output.copy_from_slice(derived.as_ref());
    }

    fn for_key_exchange(
        &self,
        output: &mut [u8; 48],
        kx: Box<dyn ActiveKeyExchange>,
        peer_pub_key: &[u8],
        label: &[u8],
        seed: &[u8],
    ) -> Result<(), Error> {
        self.for_secret(
            output,
            kx.complete_for_tls_version(peer_pub_key, &TLS12)?
                .secret_bytes(),
            label,
            seed,
        );
        Ok(())
    }

    fn fips(&self) -> bool {
        super::fips()
    }
}
