#![allow(clippy::duplicate_mod)]

use alloc::boxed::Box;

use super::ring_like::aead;
use crate::crypto::cipher::{AeadKey, Iv, Nonce};
use crate::error::Error;
use crate::quic;

pub(crate) struct HeaderProtectionKey(aead::quic::HeaderProtectionKey);

impl HeaderProtectionKey {
    pub(crate) fn new(key: AeadKey, alg: &'static aead::quic::Algorithm) -> Self {
        Self(aead::quic::HeaderProtectionKey::new(alg, key.as_ref()).unwrap())
    }

    fn xor_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
        masked: bool,
    ) -> Result<(), Error> {
        // This implements "Header Protection Application" almost verbatim.
        // <https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.1>

        let mask = self
            .0
            .new_mask(sample)
            .map_err(|_| Error::General("sample of invalid length".into()))?;

        // The `unwrap()` will not panic because `new_mask` returns a
        // non-empty result.
        let (first_mask, pn_mask) = mask.split_first().unwrap();

        // It is OK for the `mask` to be longer than `packet_number`,
        // but a valid `packet_number` will never be longer than `mask`.
        if packet_number.len() > pn_mask.len() {
            return Err(Error::General("packet number too long".into()));
        }

        // Infallible from this point on. Before this point, `first` and
        // `packet_number` are unchanged.

        const LONG_HEADER_FORM: u8 = 0x80;
        let bits = match *first & LONG_HEADER_FORM == LONG_HEADER_FORM {
            true => 0x0f,  // Long header: 4 bits masked
            false => 0x1f, // Short header: 5 bits masked
        };

        let first_plain = match masked {
            // When unmasking, use the packet length bits after unmasking
            true => *first ^ (first_mask & bits),
            // When masking, use the packet length bits before masking
            false => *first,
        };
        let pn_len = (first_plain & 0x03) as usize + 1;

        *first ^= first_mask & bits;
        for (dst, m) in packet_number
            .iter_mut()
            .zip(pn_mask)
            .take(pn_len)
        {
            *dst ^= m;
        }

        Ok(())
    }
}

impl quic::HeaderProtectionKey for HeaderProtectionKey {
    fn encrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error> {
        self.xor_in_place(sample, first, packet_number, false)
    }

    fn decrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error> {
        self.xor_in_place(sample, first, packet_number, true)
    }

    #[inline]
    fn sample_len(&self) -> usize {
        self.0.algorithm().sample_len()
    }
}

pub(crate) struct PacketKey {
    /// Encrypts or decrypts a packet's payload
    key: aead::LessSafeKey,
    /// Computes unique nonces for each packet
    iv: Iv,
    /// Confidentiality limit (see [`quic::PacketKey::confidentiality_limit`])
    confidentiality_limit: u64,
    /// Integrity limit (see [`quic::PacketKey::integrity_limit`])
    integrity_limit: u64,
}

impl PacketKey {
    pub(crate) fn new(
        key: AeadKey,
        iv: Iv,
        confidentiality_limit: u64,
        integrity_limit: u64,
        aead_algorithm: &'static aead::Algorithm,
    ) -> Self {
        Self {
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(aead_algorithm, key.as_ref()).unwrap(),
            ),
            iv,
            confidentiality_limit,
            integrity_limit,
        }
    }
}

impl quic::PacketKey for PacketKey {
    fn encrypt_in_place(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &mut [u8],
    ) -> Result<quic::Tag, Error> {
        let aad = aead::Aad::from(header);
        let nonce = aead::Nonce::assume_unique_for_key(Nonce::new(&self.iv, packet_number).0);
        let tag = self
            .key
            .seal_in_place_separate_tag(nonce, aad, payload)
            .map_err(|_| Error::EncryptError)?;
        Ok(quic::Tag::from(tag.as_ref()))
    }

    /// Decrypt a QUIC packet
    ///
    /// Takes the packet `header`, which is used as the additional authenticated data, and the
    /// `payload`, which includes the authentication tag.
    ///
    /// If the return value is `Ok`, the decrypted payload can be found in `payload`, up to the
    /// length found in the return value.
    fn decrypt_in_place<'a>(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &'a mut [u8],
    ) -> Result<&'a [u8], Error> {
        let payload_len = payload.len();
        let aad = aead::Aad::from(header);
        let nonce = aead::Nonce::assume_unique_for_key(Nonce::new(&self.iv, packet_number).0);
        self.key
            .open_in_place(nonce, aad, payload)
            .map_err(|_| Error::DecryptError)?;

        let plain_len = payload_len - self.key.algorithm().tag_len();
        Ok(&payload[..plain_len])
    }

    /// Tag length for the underlying AEAD algorithm
    #[inline]
    fn tag_len(&self) -> usize {
        self.key.algorithm().tag_len()
    }

    /// Confidentiality limit (see [`quic::PacketKey::confidentiality_limit`])
    fn confidentiality_limit(&self) -> u64 {
        self.confidentiality_limit
    }

    /// Integrity limit (see [`quic::PacketKey::integrity_limit`])
    fn integrity_limit(&self) -> u64 {
        self.integrity_limit
    }
}

pub(crate) struct KeyBuilder {
    pub(crate) packet_alg: &'static aead::Algorithm,
    pub(crate) header_alg: &'static aead::quic::Algorithm,
    pub(crate) confidentiality_limit: u64,
    pub(crate) integrity_limit: u64,
}

impl quic::Algorithm for KeyBuilder {
    fn packet_key(&self, key: AeadKey, iv: Iv) -> Box<dyn quic::PacketKey> {
        Box::new(PacketKey::new(
            key,
            iv,
            self.confidentiality_limit,
            self.integrity_limit,
            self.packet_alg,
        ))
    }

    fn header_protection_key(&self, key: AeadKey) -> Box<dyn quic::HeaderProtectionKey> {
        Box::new(HeaderProtectionKey::new(key, self.header_alg))
    }

    fn aead_key_len(&self) -> usize {
        self.packet_alg.key_len()
    }

    fn fips(&self) -> bool {
        super::fips()
    }
}

#[cfg(test)]
#[macro_rules_attribute::apply(test_for_each_provider)]
mod tests {
    use std::dbg;

    use super::provider::tls13::{
        TLS13_AES_128_GCM_SHA256_INTERNAL, TLS13_CHACHA20_POLY1305_SHA256_INTERNAL,
    };
    use crate::common_state::Side;
    use crate::crypto::tls13::OkmBlock;
    use crate::quic::*;

    fn test_short_packet(version: Version, expected: &[u8]) {
        const PN: u64 = 654360564;
        const SECRET: &[u8] = &[
            0x9a, 0xc3, 0x12, 0xa7, 0xf8, 0x77, 0x46, 0x8e, 0xbe, 0x69, 0x42, 0x27, 0x48, 0xad,
            0x00, 0xa1, 0x54, 0x43, 0xf1, 0x82, 0x03, 0xa0, 0x7d, 0x60, 0x60, 0xf6, 0x88, 0xf3,
            0x0f, 0x21, 0x63, 0x2b,
        ];

        let secret = OkmBlock::new(SECRET);
        let builder = KeyBuilder::new(
            &secret,
            version,
            TLS13_CHACHA20_POLY1305_SHA256_INTERNAL
                .quic
                .unwrap(),
            TLS13_CHACHA20_POLY1305_SHA256_INTERNAL.hkdf_provider,
        );
        let packet = builder.packet_key();
        let hpk = builder.header_protection_key();

        const PLAIN: &[u8] = &[0x42, 0x00, 0xbf, 0xf4, 0x01];

        let mut buf = PLAIN.to_vec();
        let (header, payload) = buf.split_at_mut(4);
        let tag = packet
            .encrypt_in_place(PN, header, payload)
            .unwrap();
        buf.extend(tag.as_ref());

        let pn_offset = 1;
        let (header, sample) = buf.split_at_mut(pn_offset + 4);
        let (first, rest) = header.split_at_mut(1);
        let sample = &sample[..hpk.sample_len()];
        hpk.encrypt_in_place(sample, &mut first[0], dbg!(rest))
            .unwrap();

        assert_eq!(&buf, expected);

        let (header, sample) = buf.split_at_mut(pn_offset + 4);
        let (first, rest) = header.split_at_mut(1);
        let sample = &sample[..hpk.sample_len()];
        hpk.decrypt_in_place(sample, &mut first[0], rest)
            .unwrap();

        let (header, payload_tag) = buf.split_at_mut(4);
        let plain = packet
            .decrypt_in_place(PN, header, payload_tag)
            .unwrap();

        assert_eq!(plain, &PLAIN[4..]);
    }

    #[test]
    fn short_packet_header_protection() {
        // https://www.rfc-editor.org/rfc/rfc9001.html#name-chacha20-poly1305-short-hea
        test_short_packet(
            Version::V1,
            &[
                0x4c, 0xfe, 0x41, 0x89, 0x65, 0x5e, 0x5c, 0xd5, 0x5c, 0x41, 0xf6, 0x90, 0x80, 0x57,
                0x5d, 0x79, 0x99, 0xc2, 0x5a, 0x5b, 0xfb,
            ],
        );
    }

    #[test]
    fn key_update_test_vector() {
        fn equal_okm(x: &OkmBlock, y: &OkmBlock) -> bool {
            x.as_ref() == y.as_ref()
        }

        let mut secrets = Secrets::new(
            // Constant dummy values for reproducibility
            OkmBlock::new(
                &[
                    0xb8, 0x76, 0x77, 0x08, 0xf8, 0x77, 0x23, 0x58, 0xa6, 0xea, 0x9f, 0xc4, 0x3e,
                    0x4a, 0xdd, 0x2c, 0x96, 0x1b, 0x3f, 0x52, 0x87, 0xa6, 0xd1, 0x46, 0x7e, 0xe0,
                    0xae, 0xab, 0x33, 0x72, 0x4d, 0xbf,
                ][..],
            ),
            OkmBlock::new(
                &[
                    0x42, 0xdc, 0x97, 0x21, 0x40, 0xe0, 0xf2, 0xe3, 0x98, 0x45, 0xb7, 0x67, 0x61,
                    0x34, 0x39, 0xdc, 0x67, 0x58, 0xca, 0x43, 0x25, 0x9b, 0x87, 0x85, 0x06, 0x82,
                    0x4e, 0xb1, 0xe4, 0x38, 0xd8, 0x55,
                ][..],
            ),
            TLS13_AES_128_GCM_SHA256_INTERNAL,
            TLS13_AES_128_GCM_SHA256_INTERNAL
                .quic
                .unwrap(),
            Side::Client,
            Version::V1,
        );
        secrets.update();

        assert!(equal_okm(
            &secrets.client,
            &OkmBlock::new(
                &[
                    0x42, 0xca, 0xc8, 0xc9, 0x1c, 0xd5, 0xeb, 0x40, 0x68, 0x2e, 0x43, 0x2e, 0xdf,
                    0x2d, 0x2b, 0xe9, 0xf4, 0x1a, 0x52, 0xca, 0x6b, 0x22, 0xd8, 0xe6, 0xcd, 0xb1,
                    0xe8, 0xac, 0xa9, 0x6, 0x1f, 0xce
                ][..]
            )
        ));
        assert!(equal_okm(
            &secrets.server,
            &OkmBlock::new(
                &[
                    0xeb, 0x7f, 0x5e, 0x2a, 0x12, 0x3f, 0x40, 0x7d, 0xb4, 0x99, 0xe3, 0x61, 0xca,
                    0xe5, 0x90, 0xd4, 0xd9, 0x92, 0xe1, 0x4b, 0x7a, 0xce, 0x3, 0xc2, 0x44, 0xe0,
                    0x42, 0x21, 0x15, 0xb6, 0xd3, 0x8a
                ][..]
            )
        ));
    }

    #[test]
    fn short_packet_header_protection_v2() {
        // https://www.ietf.org/archive/id/draft-ietf-quic-v2-10.html#name-chacha20-poly1305-short-head
        test_short_packet(
            Version::V2,
            &[
                0x55, 0x58, 0xb1, 0xc6, 0x0a, 0xe7, 0xb6, 0xb9, 0x32, 0xbc, 0x27, 0xd7, 0x86, 0xf4,
                0xbc, 0x2b, 0xb2, 0x0f, 0x21, 0x62, 0xba,
            ],
        );
    }

    #[test]
    fn initial_test_vector_v2() {
        // https://www.ietf.org/archive/id/draft-ietf-quic-v2-10.html#name-sample-packet-protection-2
        let icid = [0x83, 0x94, 0xc8, 0xf0, 0x3e, 0x51, 0x57, 0x08];
        let server = Keys::initial(
            Version::V2,
            TLS13_AES_128_GCM_SHA256_INTERNAL,
            TLS13_AES_128_GCM_SHA256_INTERNAL
                .quic
                .unwrap(),
            &icid,
            Side::Server,
        );
        let mut server_payload = [
            0x02, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x40, 0x5a, 0x02, 0x00, 0x00, 0x56, 0x03,
            0x03, 0xee, 0xfc, 0xe7, 0xf7, 0xb3, 0x7b, 0xa1, 0xd1, 0x63, 0x2e, 0x96, 0x67, 0x78,
            0x25, 0xdd, 0xf7, 0x39, 0x88, 0xcf, 0xc7, 0x98, 0x25, 0xdf, 0x56, 0x6d, 0xc5, 0x43,
            0x0b, 0x9a, 0x04, 0x5a, 0x12, 0x00, 0x13, 0x01, 0x00, 0x00, 0x2e, 0x00, 0x33, 0x00,
            0x24, 0x00, 0x1d, 0x00, 0x20, 0x9d, 0x3c, 0x94, 0x0d, 0x89, 0x69, 0x0b, 0x84, 0xd0,
            0x8a, 0x60, 0x99, 0x3c, 0x14, 0x4e, 0xca, 0x68, 0x4d, 0x10, 0x81, 0x28, 0x7c, 0x83,
            0x4d, 0x53, 0x11, 0xbc, 0xf3, 0x2b, 0xb9, 0xda, 0x1a, 0x00, 0x2b, 0x00, 0x02, 0x03,
            0x04,
        ];
        let mut server_header = [
            0xd1, 0x6b, 0x33, 0x43, 0xcf, 0x00, 0x08, 0xf0, 0x67, 0xa5, 0x50, 0x2a, 0x42, 0x62,
            0xb5, 0x00, 0x40, 0x75, 0x00, 0x01,
        ];
        let tag = server
            .local
            .packet
            .encrypt_in_place(1, &server_header, &mut server_payload)
            .unwrap();
        let (first, rest) = server_header.split_at_mut(1);
        let rest_len = rest.len();
        server
            .local
            .header
            .encrypt_in_place(
                &server_payload[2..18],
                &mut first[0],
                &mut rest[rest_len - 2..],
            )
            .unwrap();
        let mut server_packet = server_header.to_vec();
        server_packet.extend(server_payload);
        server_packet.extend(tag.as_ref());
        let expected_server_packet = [
            0xdc, 0x6b, 0x33, 0x43, 0xcf, 0x00, 0x08, 0xf0, 0x67, 0xa5, 0x50, 0x2a, 0x42, 0x62,
            0xb5, 0x00, 0x40, 0x75, 0xd9, 0x2f, 0xaa, 0xf1, 0x6f, 0x05, 0xd8, 0xa4, 0x39, 0x8c,
            0x47, 0x08, 0x96, 0x98, 0xba, 0xee, 0xa2, 0x6b, 0x91, 0xeb, 0x76, 0x1d, 0x9b, 0x89,
            0x23, 0x7b, 0xbf, 0x87, 0x26, 0x30, 0x17, 0x91, 0x53, 0x58, 0x23, 0x00, 0x35, 0xf7,
            0xfd, 0x39, 0x45, 0xd8, 0x89, 0x65, 0xcf, 0x17, 0xf9, 0xaf, 0x6e, 0x16, 0x88, 0x6c,
            0x61, 0xbf, 0xc7, 0x03, 0x10, 0x6f, 0xba, 0xf3, 0xcb, 0x4c, 0xfa, 0x52, 0x38, 0x2d,
            0xd1, 0x6a, 0x39, 0x3e, 0x42, 0x75, 0x75, 0x07, 0x69, 0x80, 0x75, 0xb2, 0xc9, 0x84,
            0xc7, 0x07, 0xf0, 0xa0, 0x81, 0x2d, 0x8c, 0xd5, 0xa6, 0x88, 0x1e, 0xaf, 0x21, 0xce,
            0xda, 0x98, 0xf4, 0xbd, 0x23, 0xf6, 0xfe, 0x1a, 0x3e, 0x2c, 0x43, 0xed, 0xd9, 0xce,
            0x7c, 0xa8, 0x4b, 0xed, 0x85, 0x21, 0xe2, 0xe1, 0x40,
        ];
        assert_eq!(server_packet[..], expected_server_packet[..]);
    }
}
