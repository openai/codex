use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use aws_lc_rs::aead::{
    self, Aad, BoundKey, NONCE_LEN, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey,
};
use aws_lc_rs::agreement;
use aws_lc_rs::cipher::{AES_128_KEY_LEN, AES_256_KEY_LEN};
use aws_lc_rs::digest::{SHA256_OUTPUT_LEN, SHA384_OUTPUT_LEN, SHA512_OUTPUT_LEN};
use aws_lc_rs::encoding::{AsBigEndian, Curve25519SeedBin, EcPrivateKeyBin};
use zeroize::Zeroize;

use crate::crypto::aws_lc_rs::hmac::{HMAC_SHA256, HMAC_SHA384, HMAC_SHA512};
use crate::crypto::aws_lc_rs::unspecified_err;
use crate::crypto::hpke::{
    EncapsulatedSecret, Hpke, HpkeOpener, HpkePrivateKey, HpkePublicKey, HpkeSealer, HpkeSuite,
};
use crate::crypto::tls13::{HkdfExpander, HkdfPrkExtract, HkdfUsingHmac, expand};
use crate::msgs::enums::{HpkeAead, HpkeKdf, HpkeKem};
use crate::msgs::handshake::HpkeSymmetricCipherSuite;
#[cfg(feature = "std")]
use crate::sync::Arc;
use crate::{Error, OtherError};

/// Default [RFC 9180] Hybrid Public Key Encryption (HPKE) suites supported by aws-lc-rs cryptography.
pub static ALL_SUPPORTED_SUITES: &[&dyn Hpke] = &[
    DH_KEM_P256_HKDF_SHA256_AES_128,
    DH_KEM_P256_HKDF_SHA256_AES_256,
    #[cfg(not(feature = "fips"))]
    DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305,
    DH_KEM_P384_HKDF_SHA384_AES_128,
    DH_KEM_P384_HKDF_SHA384_AES_256,
    #[cfg(not(feature = "fips"))]
    DH_KEM_P384_HKDF_SHA384_CHACHA20_POLY1305,
    DH_KEM_P521_HKDF_SHA512_AES_128,
    DH_KEM_P521_HKDF_SHA512_AES_256,
    #[cfg(not(feature = "fips"))]
    DH_KEM_P521_HKDF_SHA512_CHACHA20_POLY1305,
    #[cfg(not(feature = "fips"))]
    DH_KEM_X25519_HKDF_SHA256_AES_128,
    #[cfg(not(feature = "fips"))]
    DH_KEM_X25519_HKDF_SHA256_AES_256,
    #[cfg(not(feature = "fips"))]
    DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305,
];

/// HPKE suite using ECDH P-256 for agreement, HKDF SHA-256 for key derivation, and AEAD AES-128-GCM
/// for symmetric encryption.
pub static DH_KEM_P256_HKDF_SHA256_AES_128: &HpkeAwsLcRs<AES_128_KEY_LEN, SHA256_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P256_HKDF_SHA256,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA256,
                aead_id: HpkeAead::AES_128_GCM,
            },
        },
        dh_kem: DH_KEM_P256_HKDF_SHA256,
        hkdf: RING_HKDF_HMAC_SHA256,
        aead: &aead::AES_128_GCM,
    };

/// HPKE suite using ECDH P-256 for agreement, HKDF SHA-256 for key derivation and AEAD AES-256-GCM
/// for symmetric encryption.
pub static DH_KEM_P256_HKDF_SHA256_AES_256: &HpkeAwsLcRs<AES_256_KEY_LEN, SHA256_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P256_HKDF_SHA256,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA256,
                aead_id: HpkeAead::AES_256_GCM,
            },
        },
        dh_kem: DH_KEM_P256_HKDF_SHA256,
        hkdf: RING_HKDF_HMAC_SHA256,
        aead: &aead::AES_256_GCM,
    };

/// HPKE suite using ECDH P-256 for agreement, HKDF SHA-256 for key derivation, and AEAD
/// CHACHA20-POLY-1305 for symmetric encryption.
pub static DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305: &HpkeAwsLcRs<
    CHACHA_KEY_LEN,
    SHA256_OUTPUT_LEN,
> = &HpkeAwsLcRs {
    suite: HpkeSuite {
        kem: HpkeKem::DHKEM_P256_HKDF_SHA256,
        sym: HpkeSymmetricCipherSuite {
            kdf_id: HpkeKdf::HKDF_SHA256,
            aead_id: HpkeAead::CHACHA20_POLY_1305,
        },
    },
    dh_kem: DH_KEM_P256_HKDF_SHA256,
    hkdf: RING_HKDF_HMAC_SHA256,
    aead: &aead::CHACHA20_POLY1305,
};

/// HPKE suite using ECDH P-384 for agreement, HKDF SHA-384 for key derivation, and AEAD AES-128-GCM
/// for symmetric encryption.
pub static DH_KEM_P384_HKDF_SHA384_AES_128: &HpkeAwsLcRs<AES_128_KEY_LEN, SHA384_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P384_HKDF_SHA384,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA384,
                aead_id: HpkeAead::AES_128_GCM,
            },
        },
        dh_kem: DH_KEM_P384_HKDF_SHA384,
        hkdf: RING_HKDF_HMAC_SHA384,
        aead: &aead::AES_128_GCM,
    };

/// HPKE suite using ECDH P-384 for agreement, HKDF SHA-384 for key derivation, and AEAD AES-256-GCM
/// for symmetric encryption.
pub static DH_KEM_P384_HKDF_SHA384_AES_256: &HpkeAwsLcRs<AES_256_KEY_LEN, SHA384_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P384_HKDF_SHA384,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA384,
                aead_id: HpkeAead::AES_256_GCM,
            },
        },
        dh_kem: DH_KEM_P384_HKDF_SHA384,
        hkdf: RING_HKDF_HMAC_SHA384,
        aead: &aead::AES_256_GCM,
    };

/// HPKE suite using ECDH P-384 for agreement, HKDF SHA-384 for key derivation, and AEAD
/// CHACHA20-POLY-1305 for symmetric encryption.
pub static DH_KEM_P384_HKDF_SHA384_CHACHA20_POLY1305: &HpkeAwsLcRs<
    CHACHA_KEY_LEN,
    SHA384_OUTPUT_LEN,
> = &HpkeAwsLcRs {
    suite: HpkeSuite {
        kem: HpkeKem::DHKEM_P384_HKDF_SHA384,
        sym: HpkeSymmetricCipherSuite {
            kdf_id: HpkeKdf::HKDF_SHA384,
            aead_id: HpkeAead::CHACHA20_POLY_1305,
        },
    },
    dh_kem: DH_KEM_P384_HKDF_SHA384,
    hkdf: RING_HKDF_HMAC_SHA384,
    aead: &aead::CHACHA20_POLY1305,
};

/// HPKE suite using ECDH P-521 for agreement, HKDF SHA-512 for key derivation, and AEAD AES-128-GCM
/// for symmetric encryption.
pub static DH_KEM_P521_HKDF_SHA512_AES_128: &HpkeAwsLcRs<AES_128_KEY_LEN, SHA512_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P521_HKDF_SHA512,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA512,
                aead_id: HpkeAead::AES_128_GCM,
            },
        },
        dh_kem: DH_KEM_P521_HKDF_SHA512,
        hkdf: RING_HKDF_HMAC_SHA512,
        aead: &aead::AES_128_GCM,
    };

/// HPKE suite using ECDH P-521 for agreement, HKDF SHA-512 for key derivation, and AEAD AES-256-GCM
/// for symmetric encryption.
pub static DH_KEM_P521_HKDF_SHA512_AES_256: &HpkeAwsLcRs<AES_256_KEY_LEN, SHA512_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_P521_HKDF_SHA512,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA512,
                aead_id: HpkeAead::AES_256_GCM,
            },
        },
        dh_kem: DH_KEM_P521_HKDF_SHA512,
        hkdf: RING_HKDF_HMAC_SHA512,
        aead: &aead::AES_256_GCM,
    };

/// HPKE suite using ECDH P-521 for agreement, HKDF SHA-512 for key derivation, and AEAD
/// CHACHA20-POLY-1305 for symmetric encryption.
pub static DH_KEM_P521_HKDF_SHA512_CHACHA20_POLY1305: &HpkeAwsLcRs<
    CHACHA_KEY_LEN,
    SHA512_OUTPUT_LEN,
> = &HpkeAwsLcRs {
    suite: HpkeSuite {
        kem: HpkeKem::DHKEM_P521_HKDF_SHA512,
        sym: HpkeSymmetricCipherSuite {
            kdf_id: HpkeKdf::HKDF_SHA512,
            aead_id: HpkeAead::CHACHA20_POLY_1305,
        },
    },
    dh_kem: DH_KEM_P521_HKDF_SHA512,
    hkdf: RING_HKDF_HMAC_SHA512,
    aead: &aead::CHACHA20_POLY1305,
};

/// HPKE suite using ECDH X25519 for agreement, HKDF SHA-256 for key derivation, and AEAD AES-128-GCM
/// for symmetric encryption.
pub static DH_KEM_X25519_HKDF_SHA256_AES_128: &HpkeAwsLcRs<AES_128_KEY_LEN, SHA256_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_X25519_HKDF_SHA256,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA256,
                aead_id: HpkeAead::AES_128_GCM,
            },
        },
        dh_kem: DH_KEM_X25519_HKDF_SHA256,
        hkdf: RING_HKDF_HMAC_SHA256,
        aead: &aead::AES_128_GCM,
    };

/// HPKE suite using ECDH X25519 for agreement, HKDF SHA-256 for key derivation, and AEAD AES-256-GCM
/// for symmetric encryption.
pub static DH_KEM_X25519_HKDF_SHA256_AES_256: &HpkeAwsLcRs<AES_256_KEY_LEN, SHA256_OUTPUT_LEN> =
    &HpkeAwsLcRs {
        suite: HpkeSuite {
            kem: HpkeKem::DHKEM_X25519_HKDF_SHA256,
            sym: HpkeSymmetricCipherSuite {
                kdf_id: HpkeKdf::HKDF_SHA256,
                aead_id: HpkeAead::AES_256_GCM,
            },
        },
        dh_kem: DH_KEM_X25519_HKDF_SHA256,
        hkdf: RING_HKDF_HMAC_SHA256,
        aead: &aead::AES_256_GCM,
    };

/// HPKE suite using ECDH X25519 for agreement, HKDF SHA-256 for key derivation, and AEAD
/// CHACHA20-POLY-1305 for symmetric encryption.
pub static DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305: &HpkeAwsLcRs<
    CHACHA_KEY_LEN,
    SHA256_OUTPUT_LEN,
> = &HpkeAwsLcRs {
    suite: HpkeSuite {
        kem: HpkeKem::DHKEM_X25519_HKDF_SHA256,
        sym: HpkeSymmetricCipherSuite {
            kdf_id: HpkeKdf::HKDF_SHA256,
            aead_id: HpkeAead::CHACHA20_POLY_1305,
        },
    },
    dh_kem: DH_KEM_X25519_HKDF_SHA256,
    hkdf: RING_HKDF_HMAC_SHA256,
    aead: &aead::CHACHA20_POLY1305,
};

/// `HpkeAwsLcRs` holds the concrete instantiations of the algorithms specified by the [HpkeSuite].
pub struct HpkeAwsLcRs<const KEY_SIZE: usize, const KDF_SIZE: usize> {
    suite: HpkeSuite,
    dh_kem: &'static DhKem<KDF_SIZE>,
    hkdf: &'static dyn HkdfPrkExtract,
    aead: &'static aead::Algorithm,
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> HpkeAwsLcRs<KEY_SIZE, KDF_SIZE> {
    /// See [RFC 9180 §5.1 "Creating the Encryption Context"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-5.1
    fn key_schedule(
        &self,
        shared_secret: KemSharedSecret<KDF_SIZE>,
        info: &[u8],
    ) -> Result<KeySchedule<KEY_SIZE>, Error> {
        // Note: we use an empty IKM for the `psk_id_hash` and `secret` labelled extractions because
        // there is no PSK ID in base mode HPKE.

        let suite_id = LabeledSuiteId::Hpke(self.suite);
        let psk_id_hash = labeled_extract_for_prk(self.hkdf, suite_id, None, Label::PskIdHash, &[]);
        let info_hash = labeled_extract_for_prk(self.hkdf, suite_id, None, Label::InfoHash, info);
        let key_schedule_context = [
            &[0][..], // base mode (0x00)
            &psk_id_hash,
            &info_hash,
        ]
        .concat();

        let key = AeadKey(self.key_schedule_labeled_expand::<KEY_SIZE>(
            &shared_secret,
            &key_schedule_context,
            Label::Key,
        ));

        let base_nonce = self.key_schedule_labeled_expand::<NONCE_LEN>(
            &shared_secret,
            &key_schedule_context,
            Label::BaseNonce,
        );

        Ok(KeySchedule {
            aead: self.aead,
            key,
            base_nonce,
            seq_num: 0,
        })
    }

    fn key_schedule_labeled_expand<const L: usize>(
        &self,
        shared_secret: &KemSharedSecret<KDF_SIZE>,
        key_schedule_context: &[u8],
        label: Label,
    ) -> [u8; L] {
        let suite_id = LabeledSuiteId::Hpke(self.suite);
        labeled_expand::<L>(
            suite_id,
            labeled_extract_for_expand(
                self.hkdf,
                suite_id,
                Some(&shared_secret.0),
                Label::Secret,
                &[],
            ),
            label,
            key_schedule_context,
        )
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Hpke for HpkeAwsLcRs<KEY_SIZE, KDF_SIZE> {
    fn seal(
        &self,
        info: &[u8],
        aad: &[u8],
        plaintext: &[u8],
        pub_key: &HpkePublicKey,
    ) -> Result<(EncapsulatedSecret, Vec<u8>), Error> {
        let (encap, mut sealer) = self.setup_sealer(info, pub_key)?;
        Ok((encap, sealer.seal(aad, plaintext)?))
    }

    fn setup_sealer(
        &self,
        info: &[u8],
        pub_key: &HpkePublicKey,
    ) -> Result<(EncapsulatedSecret, Box<dyn HpkeSealer + 'static>), Error> {
        let (encap, sealer) = Sealer::new(self, info, pub_key)?;
        Ok((encap, Box::new(sealer)))
    }

    fn open(
        &self,
        enc: &EncapsulatedSecret,
        info: &[u8],
        aad: &[u8],
        ciphertext: &[u8],
        secret_key: &HpkePrivateKey,
    ) -> Result<Vec<u8>, Error> {
        self.setup_opener(enc, info, secret_key)?
            .open(aad, ciphertext)
    }

    fn setup_opener(
        &self,
        enc: &EncapsulatedSecret,
        info: &[u8],
        secret_key: &HpkePrivateKey,
    ) -> Result<Box<dyn HpkeOpener + 'static>, Error> {
        Ok(Box::new(Opener::new(self, enc, info, secret_key)?))
    }

    fn fips(&self) -> bool {
        matches!(
            // We make a FIPS determination based on the suite's DH KEM and AEAD choice.
            // We don't need to examine the KDF choice because all supported KDFs are FIPS
            // compatible.
            (self.suite.kem, self.suite.sym.aead_id),
            (
                // Only the NIST "P-curve" DH KEMs are FIPS compatible.
                HpkeKem::DHKEM_P256_HKDF_SHA256
                    | HpkeKem::DHKEM_P384_HKDF_SHA384
                    | HpkeKem::DHKEM_P521_HKDF_SHA512,
                // Only the AES AEADs are FIPS compatible.
                HpkeAead::AES_128_GCM | HpkeAead::AES_256_GCM,
            )
        )
    }

    fn generate_key_pair(&self) -> Result<(HpkePublicKey, HpkePrivateKey), Error> {
        (self.dh_kem.key_generator)()
    }

    fn suite(&self) -> HpkeSuite {
        self.suite
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Debug for HpkeAwsLcRs<KEY_SIZE, KDF_SIZE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.suite.fmt(f)
    }
}

/// Adapts a [KeySchedule] and [AeadKey] for the role of a [HpkeSealer].
struct Sealer<const KEY_SIZE: usize, const KDF_SIZE: usize> {
    key_schedule: KeySchedule<KEY_SIZE>,
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Sealer<KEY_SIZE, KDF_SIZE> {
    /// See [RFC 9180 §5.1.1 "Encryption to a Public Key"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-5.1.1
    fn new(
        suite: &HpkeAwsLcRs<KEY_SIZE, KDF_SIZE>,
        info: &[u8],
        pub_key: &HpkePublicKey,
    ) -> Result<(EncapsulatedSecret, Self), Error> {
        // def SetupBaseS(pkR, info):
        //   shared_secret, enc = Encap(pkR)
        //   return enc, KeyScheduleS(mode_base, shared_secret, info,
        //                            default_psk, default_psk_id)

        let (shared_secret, enc) = suite.dh_kem.encap(pub_key)?;
        let key_schedule = suite.key_schedule(shared_secret, info)?;
        Ok((enc, Self { key_schedule }))
    }

    /// A **test only** constructor that uses a pre-specified ephemeral agreement private key
    /// instead of one that is randomly generated.
    #[cfg(test)]
    fn test_only_new(
        suite: &HpkeAwsLcRs<KEY_SIZE, KDF_SIZE>,
        info: &[u8],
        pub_key: &HpkePublicKey,
        sk_e: &[u8],
    ) -> Result<(EncapsulatedSecret, Self), Error> {
        let (shared_secret, enc) = suite
            .dh_kem
            .test_only_encap(pub_key, sk_e)?;
        let key_schedule = suite.key_schedule(shared_secret, info)?;
        Ok((enc, Self { key_schedule }))
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> HpkeSealer for Sealer<KEY_SIZE, KDF_SIZE> {
    fn seal(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        // def ContextS.Seal(aad, pt):
        //   ct = Seal(self.key, self.ComputeNonce(self.seq), aad, pt)
        //   self.IncrementSeq()
        //   return ct

        let key = UnboundKey::new(self.key_schedule.aead, &self.key_schedule.key.0)
            .map_err(unspecified_err)?;
        let mut sealing_key = SealingKey::new(key, &mut self.key_schedule);

        let mut in_out_buffer = Vec::from(plaintext);
        sealing_key
            .seal_in_place_append_tag(Aad::from(aad), &mut in_out_buffer)
            .map_err(unspecified_err)?;

        Ok(in_out_buffer)
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Debug for Sealer<KEY_SIZE, KDF_SIZE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sealer").finish()
    }
}

/// Adapts a [KeySchedule] and [AeadKey] for the role of a [HpkeOpener].
struct Opener<const KEY_SIZE: usize, const KDF_SIZE: usize> {
    key_schedule: KeySchedule<KEY_SIZE>,
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Opener<KEY_SIZE, KDF_SIZE> {
    /// See [RFC 9180 §5.1.1 "Encryption to a Public Key"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-5.1.1
    fn new(
        suite: &HpkeAwsLcRs<KEY_SIZE, KDF_SIZE>,
        enc: &EncapsulatedSecret,
        info: &[u8],
        secret_key: &HpkePrivateKey,
    ) -> Result<Self, Error> {
        // def SetupBaseR(enc, skR, info):
        //   shared_secret = Decap(enc, skR)
        //   return KeyScheduleR(mode_base, shared_secret, info,
        //                       default_psk, default_psk_id)
        Ok(Self {
            key_schedule: suite.key_schedule(suite.dh_kem.decap(enc, secret_key)?, info)?,
        })
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> HpkeOpener for Opener<KEY_SIZE, KDF_SIZE> {
    fn open(&mut self, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        // def ContextR.Open(aad, ct):
        //   pt = Open(self.key, self.ComputeNonce(self.seq), aad, ct)
        //   if pt == OpenError:
        //     raise OpenError
        //   self.IncrementSeq()
        //   return pt

        let key = UnboundKey::new(self.key_schedule.aead, &self.key_schedule.key.0)
            .map_err(unspecified_err)?;
        let mut opening_key = OpeningKey::new(key, &mut self.key_schedule);

        let mut in_out_buffer = Vec::from(ciphertext);
        let plaintext = opening_key
            .open_in_place(Aad::from(aad), &mut in_out_buffer)
            .map_err(unspecified_err)?;

        Ok(plaintext.to_vec())
    }
}

impl<const KEY_SIZE: usize, const KDF_SIZE: usize> Debug for Opener<KEY_SIZE, KDF_SIZE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opener").finish()
    }
}

/// A Diffie-Hellman (DH) based Key Encapsulation Mechanism (KEM).
///
/// See [RFC 9180 §4.1 "DH-Based KEM (DHKEM)"][0].
///
/// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4.1
struct DhKem<const KDF_SIZE: usize> {
    id: HpkeKem,
    agreement_algorithm: &'static agreement::Algorithm,
    key_generator:
        &'static (dyn Fn() -> Result<(HpkePublicKey, HpkePrivateKey), Error> + Send + Sync),
    hkdf: &'static dyn HkdfPrkExtract,
}

impl<const KDF_SIZE: usize> DhKem<KDF_SIZE> {
    /// See [RFC 9180 §4.1 "DH-Based KEM (DHKEM)"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4.1
    fn encap(
        &self,
        recipient: &HpkePublicKey,
    ) -> Result<(KemSharedSecret<KDF_SIZE>, EncapsulatedSecret), Error> {
        // def Encap(pkR):
        //   skE, pkE = GenerateKeyPair()

        let sk_e =
            agreement::PrivateKey::generate(self.agreement_algorithm).map_err(unspecified_err)?;
        self.encap_impl(recipient, sk_e)
    }

    /// A test-only encap operation that uses a fixed `test_only_ske` instead of generating
    /// one randomly.
    #[cfg(test)]
    fn test_only_encap(
        &self,
        recipient: &HpkePublicKey,
        test_only_ske: &[u8],
    ) -> Result<(KemSharedSecret<KDF_SIZE>, EncapsulatedSecret), Error> {
        // For test contexts only, we accept a static sk_e as an argument.
        let sk_e = agreement::PrivateKey::from_private_key(self.agreement_algorithm, test_only_ske)
            .map_err(key_rejected_err)?;
        self.encap_impl(recipient, sk_e)
    }

    fn encap_impl(
        &self,
        recipient: &HpkePublicKey,
        sk_e: agreement::PrivateKey,
    ) -> Result<(KemSharedSecret<KDF_SIZE>, EncapsulatedSecret), Error> {
        // def Encap(pkR):
        //   skE, pkE = GenerateKeyPair()
        //   dh = DH(skE, pkR)
        //   enc = SerializePublicKey(pkE)
        //
        //   pkRm = SerializePublicKey(pkR)
        //   kem_context = concat(enc, pkRm)
        //
        //   shared_secret = ExtractAndExpand(dh, kem_context)
        //   return shared_secret, enc

        let enc = sk_e
            .compute_public_key()
            .map_err(unspecified_err)?;
        let pk_r = agreement::UnparsedPublicKey::new(self.agreement_algorithm, &recipient.0);
        let kem_context = [enc.as_ref(), pk_r.bytes()].concat();

        let shared_secret = agreement::agree(&sk_e, &pk_r, aws_lc_rs::error::Unspecified, |dh| {
            Ok(self.extract_and_expand(dh, &kem_context))
        })
        .map_err(unspecified_err)?;

        Ok((
            KemSharedSecret(shared_secret),
            EncapsulatedSecret(enc.as_ref().into()),
        ))
    }

    /// See [RFC 9180 §4.1 "DH-Based KEM (DHKEM)"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4.1
    fn decap(
        &self,
        enc: &EncapsulatedSecret,
        recipient: &HpkePrivateKey,
    ) -> Result<KemSharedSecret<KDF_SIZE>, Error> {
        // def Decap(enc, skR):
        //   pkE = DeserializePublicKey(enc)
        //   dh = DH(skR, pkE)
        //
        //   pkRm = SerializePublicKey(pk(skR))
        //   kem_context = concat(enc, pkRm)
        //
        //   shared_secret = ExtractAndExpand(dh, kem_context)
        //   return shared_secret

        let pk_e = agreement::UnparsedPublicKey::new(self.agreement_algorithm, &enc.0);
        let sk_r = agreement::PrivateKey::from_private_key(
            self.agreement_algorithm,
            recipient.secret_bytes(),
        )
        .map_err(key_rejected_err)?;
        let pk_rm = sk_r
            .compute_public_key()
            .map_err(unspecified_err)?;
        let kem_context = [&enc.0, pk_rm.as_ref()].concat();

        let shared_secret = agreement::agree(&sk_r, &pk_e, aws_lc_rs::error::Unspecified, |dh| {
            Ok(self.extract_and_expand(dh, &kem_context))
        })
        .map_err(unspecified_err)?;

        Ok(KemSharedSecret(shared_secret))
    }

    /// See [RFC 9180 §4.1 "DH-Based KEM (DHKEM)"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4.1
    fn extract_and_expand(&self, dh: &[u8], kem_context: &[u8]) -> [u8; KDF_SIZE] {
        // def ExtractAndExpand(dh, kem_context):
        //   eae_prk = LabeledExtract("", "eae_prk", dh)
        //   shared_secret = LabeledExpand(eae_prk, "shared_secret",
        //                                 kem_context, Nsecret)
        //   return shared_secret

        let suite_id = LabeledSuiteId::Kem(self.id);
        labeled_expand(
            suite_id,
            labeled_extract_for_expand(self.hkdf, suite_id, None, Label::EaePrk, dh),
            Label::SharedSecret,
            kem_context,
        )
    }
}

static DH_KEM_P256_HKDF_SHA256: &DhKem<SHA256_OUTPUT_LEN> = &DhKem {
    id: HpkeKem::DHKEM_P256_HKDF_SHA256,
    agreement_algorithm: &agreement::ECDH_P256,
    key_generator: &|| generate_p_curve_key_pair(&agreement::ECDH_P256),
    hkdf: RING_HKDF_HMAC_SHA256,
};

static DH_KEM_P384_HKDF_SHA384: &DhKem<SHA384_OUTPUT_LEN> = &DhKem {
    id: HpkeKem::DHKEM_P384_HKDF_SHA384,
    agreement_algorithm: &agreement::ECDH_P384,
    key_generator: &|| generate_p_curve_key_pair(&agreement::ECDH_P384),
    hkdf: RING_HKDF_HMAC_SHA384,
};

static DH_KEM_P521_HKDF_SHA512: &DhKem<SHA512_OUTPUT_LEN> = &DhKem {
    id: HpkeKem::DHKEM_P521_HKDF_SHA512,
    agreement_algorithm: &agreement::ECDH_P521,
    key_generator: &|| generate_p_curve_key_pair(&agreement::ECDH_P521),
    hkdf: RING_HKDF_HMAC_SHA512,
};

static DH_KEM_X25519_HKDF_SHA256: &DhKem<SHA256_OUTPUT_LEN> = &DhKem {
    id: HpkeKem::DHKEM_X25519_HKDF_SHA256,
    agreement_algorithm: &agreement::X25519,
    key_generator: &generate_x25519_key_pair,
    hkdf: RING_HKDF_HMAC_SHA256,
};

/// Generate a NIST P-256, P-384 or P-512 key pair expressed as a raw big-endian fixed-length
/// integer.
///
/// We must disambiguate the [`AsBigEndian`] trait in-use and this function uses
/// [`AsBigEndian<EcPrivateKeyBin>`], which does not support [`agreement::X25519`].
/// For generating X25519 keys see [`generate_x25519_key_pair`].
fn generate_p_curve_key_pair(
    alg: &'static agreement::Algorithm,
) -> Result<(HpkePublicKey, HpkePrivateKey), Error> {
    // We only initialize DH KEM instances that use this function as a key generator
    // for non-X25519 algorithms. Debug assert this just in case since `AsBigEndian<EcPrivateKeyBin>`
    // will panic for this algorithm.
    debug_assert_ne!(alg, &agreement::X25519);
    let (public_key, private_key) = generate_key_pair(alg)?;
    let raw_private_key: EcPrivateKeyBin<'_> = private_key
        .as_be_bytes()
        .map_err(unspecified_err)?;
    Ok((
        public_key,
        HpkePrivateKey::from(raw_private_key.as_ref().to_vec()),
    ))
}

/// Generate a X25519 key pair expressed as a raw big-endian fixed-length
/// integer.
///
/// We must disambiguate the [`AsBigEndian`] trait in-use and this function uses
/// [`AsBigEndian<Curve25519SeedBin>`], which only supports [`agreement::X25519`].
/// For generating P-256, P-384 and P-512 keys see [`generate_p_curve_key_pair`].
fn generate_x25519_key_pair() -> Result<(HpkePublicKey, HpkePrivateKey), Error> {
    let (public_key, private_key) = generate_key_pair(&agreement::X25519)?;
    let raw_private_key: Curve25519SeedBin<'_> = private_key
        .as_be_bytes()
        .map_err(unspecified_err)?;
    Ok((
        public_key,
        HpkePrivateKey::from(raw_private_key.as_ref().to_vec()),
    ))
}

fn generate_key_pair(
    alg: &'static agreement::Algorithm,
) -> Result<(HpkePublicKey, agreement::PrivateKey), Error> {
    let private_key = agreement::PrivateKey::generate(alg).map_err(unspecified_err)?;
    let public_key = HpkePublicKey(
        private_key
            .compute_public_key()
            .map_err(unspecified_err)?
            .as_ref()
            .to_vec(),
    );
    Ok((public_key, private_key))
}

/// KeySchedule holds the derived AEAD key, base nonce, and seq number
/// common to both a [Sealer] and [Opener].
struct KeySchedule<const KEY_SIZE: usize> {
    aead: &'static aead::Algorithm,
    key: AeadKey<KEY_SIZE>,
    base_nonce: [u8; NONCE_LEN],
    seq_num: u32,
}

impl<const KEY_SIZE: usize> KeySchedule<KEY_SIZE> {
    /// See [RFC 9180 §5.2 "Encryption and Decryption"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-5.2
    fn compute_nonce(&self) -> [u8; NONCE_LEN] {
        // def Context<ROLE>.ComputeNonce(seq):
        //   seq_bytes = I2OSP(seq, Nn)
        //   return xor(self.base_nonce, seq_bytes)

        // Each new N-byte nonce is conceptually two parts:
        //   * N-4 bytes of the base nonce (0s in `nonce` to XOR in as-is).
        //   * 4 bytes derived from the sequence number XOR the base nonce.
        let mut nonce = [0; NONCE_LEN];
        let seq_bytes = self.seq_num.to_be_bytes();
        nonce[NONCE_LEN - seq_bytes.len()..].copy_from_slice(&seq_bytes);

        for (n, &b) in nonce.iter_mut().zip(&self.base_nonce) {
            *n ^= b;
        }

        nonce
    }

    /// See [RFC 9180 §5.2 "Encryption and Decryption"][0].
    ///
    /// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-5.2
    fn increment_seq_num(&mut self) -> Result<(), aws_lc_rs::error::Unspecified> {
        // def Context<ROLE>.IncrementSeq():
        //   if self.seq >= (1 << (8*Nn)) - 1:
        //     raise MessageLimitReachedError
        //   self.seq += 1

        // Determine the maximum sequence number using the AEAD nonce's length in bits.
        // Do this as an u128 to prevent overflowing.
        let max_seq_num = (1u128 << (NONCE_LEN * 8)) - 1;

        // Promote the u32 sequence number to an u128 and compare against the maximum allowed
        // sequence number.
        if u128::from(self.seq_num) >= max_seq_num {
            return Err(aws_lc_rs::error::Unspecified);
        }

        self.seq_num += 1;
        Ok(())
    }
}

impl<const KEY_SIZE: usize> NonceSequence for &mut KeySchedule<KEY_SIZE> {
    fn advance(&mut self) -> Result<Nonce, aws_lc_rs::error::Unspecified> {
        let nonce = self.compute_nonce();
        self.increment_seq_num()?;
        Nonce::try_assume_unique_for_key(&nonce)
    }
}

/// See [RFC 9180 §4 "Cryptographic Dependencies"][0].
///
/// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4
fn labeled_extract_for_expand(
    hkdf: &'static dyn HkdfPrkExtract,
    suite_id: LabeledSuiteId,
    salt: Option<&[u8]>,
    label: Label,
    ikm: &[u8],
) -> Box<dyn HkdfExpander> {
    // def LabeledExtract(salt, label, ikm):
    //   labeled_ikm = concat("HPKE-v1", suite_id, label, ikm)
    //   return Extract(salt, labeled_ikm)

    let labeled_ikm = [&b"HPKE-v1"[..], &suite_id.encoded(), label.as_ref(), ikm].concat();
    hkdf.extract_from_secret(salt, &labeled_ikm)
}

/// See [RFC 9180 §4 "Cryptographic Dependencies"][0].
///
/// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4
fn labeled_extract_for_prk(
    hkdf: &'static dyn HkdfPrkExtract,
    suite_id: LabeledSuiteId,
    salt: Option<&[u8]>,
    label: Label,
    ikm: &[u8],
) -> Vec<u8> {
    // def LabeledExtract(salt, label, ikm):
    //   labeled_ikm = concat("HPKE-v1", suite_id, label, ikm)
    //   return Extract(salt, labeled_ikm)

    let labeled_ikm = [&b"HPKE-v1"[..], &suite_id.encoded(), label.as_ref(), ikm].concat();
    hkdf.extract_prk_from_secret(salt, &labeled_ikm)
}

/// See [RFC 9180 §4 "Cryptographic Dependencies"][0].
///
/// [0]: https://www.rfc-editor.org/rfc/rfc9180.html#section-4
fn labeled_expand<const L: usize>(
    suite_id: LabeledSuiteId,
    expander: Box<dyn HkdfExpander>,
    label: Label,
    kem_context: &[u8],
) -> [u8; L] {
    // def LabeledExpand(prk, label, info, L):
    //   labeled_info = concat(I2OSP(L, 2), "HPKE-v1", suite_id,
    //                         label, info)
    //   return Expand(prk, labeled_info, L)

    let output_len = u16::to_be_bytes(L as u16);
    let info = &[
        &output_len[..],
        b"HPKE-v1",
        &suite_id.encoded(),
        label.as_ref(),
        kem_context,
    ];

    expand(&*expander, info)
}

/// Label describes the possible labels for use with [labeled_extract_for_expand] and [labeled_expand].
#[derive(Debug)]
enum Label {
    PskIdHash,
    InfoHash,
    Secret,
    Key,
    BaseNonce,
    EaePrk,
    SharedSecret,
}

impl AsRef<[u8]> for Label {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::PskIdHash => b"psk_id_hash",
            Self::InfoHash => b"info_hash",
            Self::Secret => b"secret",
            Self::Key => b"key",
            Self::BaseNonce => b"base_nonce",
            Self::EaePrk => b"eae_prk",
            Self::SharedSecret => b"shared_secret",
        }
    }
}

/// LabeledSuiteId describes the possible suite ID values for use with [labeled_extract_for_expand] and
/// [labeled_expand].
#[derive(Debug, Copy, Clone)]
enum LabeledSuiteId {
    Hpke(HpkeSuite),
    Kem(HpkeKem),
}

impl LabeledSuiteId {
    /// The suite ID encoding depends on the context of use. In the general HPKE context,
    /// we use a "HPKE" prefix and encode the entire ciphersuite. In the KEM context we use a
    /// "KEM" prefix and only encode the KEM ID.
    ///
    /// See the bottom of [RFC 9180 §4](https://www.rfc-editor.org/rfc/rfc9180.html#section-4)
    /// for more information.
    fn encoded(&self) -> Vec<u8> {
        match self {
            Self::Hpke(suite) => [
                &b"HPKE"[..],
                &u16::from(suite.kem).to_be_bytes(),
                &u16::from(suite.sym.kdf_id).to_be_bytes(),
                &u16::from(suite.sym.aead_id).to_be_bytes(),
            ]
            .concat(),
            Self::Kem(kem) => [&b"KEM"[..], &u16::from(*kem).to_be_bytes()].concat(),
        }
    }
}

/// A newtype wrapper for an unbound AEAD key.
struct AeadKey<const KEY_LEN: usize>([u8; KEY_LEN]);

impl<const KEY_LEN: usize> Drop for AeadKey<KEY_LEN> {
    fn drop(&mut self) {
        self.0.zeroize()
    }
}

/// A newtype wrapper for a DH KEM shared secret.
struct KemSharedSecret<const KDF_LEN: usize>([u8; KDF_LEN]);

impl<const KDF_LEN: usize> Drop for KemSharedSecret<KDF_LEN> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn key_rejected_err(_e: aws_lc_rs::error::KeyRejected) -> Error {
    #[cfg(feature = "std")]
    {
        Error::Other(OtherError(Arc::new(_e)))
    }
    #[cfg(not(feature = "std"))]
    {
        Error::Other(OtherError())
    }
}

// The `cipher::chacha::KEY_LEN` const is not exported, so we copy it here:
// https://github.com/aws/aws-lc-rs/blob/0186ef7bb1a4d7e140bae8074a9871f49afedf1b/aws-lc-rs/src/cipher/chacha.rs#L13
const CHACHA_KEY_LEN: usize = 32;

static RING_HKDF_HMAC_SHA256: &HkdfUsingHmac<'static> = &HkdfUsingHmac(&HMAC_SHA256);
static RING_HKDF_HMAC_SHA384: &HkdfUsingHmac<'static> = &HkdfUsingHmac(&HMAC_SHA384);
static RING_HKDF_HMAC_SHA512: &HkdfUsingHmac<'static> = &HkdfUsingHmac(&HMAC_SHA512);

#[cfg(test)]
mod tests {
    use alloc::{format, vec};

    use super::*;

    #[test]
    fn smoke_test() {
        for suite in ALL_SUPPORTED_SUITES {
            _ = format!("{suite:?}"); // HpkeAwsLcRs suites should be Debug.

            // We should be able to generate a random keypair.
            let (pk, sk) = suite.generate_key_pair().unwrap();

            // Info value corresponds to the first RFC 9180 base mode test vector.
            let info = &[
                0x4f, 0x64, 0x65, 0x20, 0x6f, 0x6e, 0x20, 0x61, 0x20, 0x47, 0x72, 0x65, 0x63, 0x69,
                0x61, 0x6e, 0x20, 0x55, 0x72, 0x6e,
            ][..];

            // We should be able to set up a sealer.
            let (enc, mut sealer) = suite.setup_sealer(info, &pk).unwrap();

            _ = format!("{sealer:?}"); // Sealer should be Debug.

            // Setting up a sealer with an invalid public key should fail.
            let bad_setup_res = suite.setup_sealer(info, &HpkePublicKey(vec![]));
            assert!(matches!(bad_setup_res.unwrap_err(), Error::Other(_)));

            // We should be able to seal some plaintext.
            let aad = &[0xC0, 0xFF, 0xEE];
            let pt = &[0xF0, 0x0D];
            let ct = sealer.seal(aad, pt).unwrap();

            // We should be able to set up an opener.
            let mut opener = suite
                .setup_opener(&enc, info, &sk)
                .unwrap();
            _ = format!("{opener:?}"); // Opener should be Debug.

            // Setting up an opener with an invalid private key should fail.
            let bad_key_res = suite.setup_opener(&enc, info, &HpkePrivateKey::from(vec![]));
            assert!(matches!(bad_key_res.unwrap_err(), Error::Other(_)));

            // Opening the plaintext should work with the correct opener and aad.
            let pt_prime = opener.open(aad, &ct).unwrap();
            assert_eq!(pt_prime, pt);

            // Opening the plaintext with the correct opener and wrong aad should fail.
            let open_res = opener.open(&[0x0], &ct);
            assert!(matches!(open_res.unwrap_err(), Error::Other(_)));

            // Opening the plaintext with the wrong opener should fail.
            let mut sk_rm_prime = sk.secret_bytes().to_vec();
            sk_rm_prime[10] ^= 0xFF; // Corrupt a byte of the private key.
            let mut opener_two = suite
                .setup_opener(&enc, info, &HpkePrivateKey::from(sk_rm_prime))
                .unwrap();
            let open_res = opener_two.open(aad, &ct);
            assert!(matches!(open_res.unwrap_err(), Error::Other(_)));
        }
    }

    #[cfg(not(feature = "fips"))] // Ensure all supported suites are available to test.
    #[test]
    fn test_fips() {
        let testcases: &[(&dyn Hpke, bool)] = &[
            // FIPS compatible.
            (DH_KEM_P256_HKDF_SHA256_AES_128, true),
            (DH_KEM_P256_HKDF_SHA256_AES_256, true),
            (DH_KEM_P384_HKDF_SHA384_AES_128, true),
            (DH_KEM_P384_HKDF_SHA384_AES_256, true),
            (DH_KEM_P521_HKDF_SHA512_AES_128, true),
            (DH_KEM_P521_HKDF_SHA512_AES_256, true),
            // AEAD is not FIPS compatible.
            (DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305, false),
            (DH_KEM_P384_HKDF_SHA384_CHACHA20_POLY1305, false),
            (DH_KEM_P521_HKDF_SHA512_CHACHA20_POLY1305, false),
            // KEM is not FIPS compatible.
            (DH_KEM_X25519_HKDF_SHA256_AES_128, false),
            (DH_KEM_X25519_HKDF_SHA256_AES_256, false),
            (DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305, false),
        ];
        for (suite, expected) in testcases {
            assert_eq!(suite.fips(), *expected);
        }
    }
}

#[cfg(test)]
mod rfc_tests {
    use alloc::string::String;
    use std::fs::File;
    use std::println;

    use serde::Deserialize;

    use super::*;

    /// Confirm open/seal operations work using the test vectors from [RFC 9180 Appendix A].
    ///
    /// [RFC 9180 Appendix A]: https://www.rfc-editor.org/rfc/rfc9180#TestVectors
    #[test]
    fn check_test_vectors() {
        for (idx, vec) in test_vectors().into_iter().enumerate() {
            let Some(hpke) = vec.applicable() else {
                println!("skipping inapplicable vector {idx}");
                continue;
            };

            println!("testing vector {idx}");
            let pk_r = HpkePublicKey(hex::decode(vec.pk_rm).unwrap());
            let sk_r = HpkePrivateKey::from(hex::decode(vec.sk_rm).unwrap());
            let sk_em = hex::decode(vec.sk_em).unwrap();
            let info = hex::decode(vec.info).unwrap();
            let expected_enc = hex::decode(vec.enc).unwrap();

            let (enc, mut sealer) = hpke
                .setup_test_sealer(&info, &pk_r, &sk_em)
                .unwrap();
            assert_eq!(enc.0, expected_enc);

            let mut opener = hpke
                .setup_opener(&enc, &info, &sk_r)
                .unwrap();

            for test_encryption in vec.encryptions {
                let aad = hex::decode(test_encryption.aad).unwrap();
                let pt = hex::decode(test_encryption.pt).unwrap();
                let expected_ct = hex::decode(test_encryption.ct).unwrap();

                let ciphertext = sealer.seal(&aad, &pt).unwrap();
                assert_eq!(ciphertext, expected_ct);

                let plaintext = opener.open(&aad, &ciphertext).unwrap();
                assert_eq!(plaintext, pt);
            }
        }
    }

    trait TestHpke: Hpke {
        fn setup_test_sealer(
            &self,
            info: &[u8],
            pub_key: &HpkePublicKey,
            sk_em: &[u8],
        ) -> Result<(EncapsulatedSecret, Box<dyn HpkeSealer + 'static>), Error>;
    }

    impl<const KEY_SIZE: usize, const KDF_SIZE: usize> TestHpke for HpkeAwsLcRs<KEY_SIZE, KDF_SIZE> {
        fn setup_test_sealer(
            &self,
            info: &[u8],
            pub_key: &HpkePublicKey,
            sk_em: &[u8],
        ) -> Result<(EncapsulatedSecret, Box<dyn HpkeSealer + 'static>), Error> {
            let (encap, sealer) = Sealer::test_only_new(self, info, pub_key, sk_em)?;
            Ok((encap, Box::new(sealer)))
        }
    }

    static TEST_SUITES: &[&dyn TestHpke] = &[
        DH_KEM_P256_HKDF_SHA256_AES_128,
        DH_KEM_P256_HKDF_SHA256_AES_256,
        #[cfg(not(feature = "fips"))]
        DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305,
        DH_KEM_P384_HKDF_SHA384_AES_128,
        DH_KEM_P384_HKDF_SHA384_AES_256,
        #[cfg(not(feature = "fips"))]
        DH_KEM_P384_HKDF_SHA384_CHACHA20_POLY1305,
        DH_KEM_P521_HKDF_SHA512_AES_128,
        DH_KEM_P521_HKDF_SHA512_AES_256,
        #[cfg(not(feature = "fips"))]
        DH_KEM_P521_HKDF_SHA512_CHACHA20_POLY1305,
        #[cfg(not(feature = "fips"))]
        DH_KEM_X25519_HKDF_SHA256_AES_128,
        #[cfg(not(feature = "fips"))]
        DH_KEM_X25519_HKDF_SHA256_AES_256,
        #[cfg(not(feature = "fips"))]
        DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305,
    ];

    #[derive(Deserialize, Debug)]
    struct TestVector {
        mode: u8,
        kem_id: u16,
        kdf_id: u16,
        aead_id: u16,
        info: String,
        #[serde(rename(deserialize = "pkRm"))]
        pk_rm: String,
        #[serde(rename(deserialize = "skRm"))]
        sk_rm: String,
        #[serde(rename(deserialize = "skEm"))]
        sk_em: String,
        enc: String,
        encryptions: Vec<TestEncryption>,
    }

    #[derive(Deserialize, Debug)]
    struct TestEncryption {
        aad: String,
        pt: String,
        ct: String,
    }

    impl TestVector {
        fn suite(&self) -> HpkeSuite {
            HpkeSuite {
                kem: HpkeKem::from(self.kem_id),
                sym: HpkeSymmetricCipherSuite {
                    kdf_id: HpkeKdf::from(self.kdf_id),
                    aead_id: HpkeAead::from(self.aead_id),
                },
            }
        }

        fn applicable(&self) -> Option<&'static dyn TestHpke> {
            // Only base mode test vectors for supported suites are applicable.
            if self.mode != 0 {
                return None;
            }

            Self::lookup_suite(self.suite(), TEST_SUITES)
        }

        fn lookup_suite(
            suite: HpkeSuite,
            supported: &[&'static dyn TestHpke],
        ) -> Option<&'static dyn TestHpke> {
            supported
                .iter()
                .find(|s| s.suite() == suite)
                .copied()
        }
    }

    fn test_vectors() -> Vec<TestVector> {
        serde_json::from_reader(
            &mut File::open("../rustls-provider-test/tests/rfc-9180-test-vectors.json")
                .expect("failed to open test vectors data file"),
        )
        .expect("failed to deserialize test vectors")
    }
}
