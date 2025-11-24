#![allow(clippy::upper_case_acronyms)]
#![allow(non_camel_case_types)]
use crate::crypto::{KeyExchangeAlgorithm, hash};
use crate::msgs::codec::{Codec, Reader};

enum_builder! {
    /// The `HashAlgorithm` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum HashAlgorithm {
        NONE => 0x00,
        MD5 => 0x01,
        SHA1 => 0x02,
        SHA224 => 0x03,
        SHA256 => 0x04,
        SHA384 => 0x05,
        SHA512 => 0x06,
    }
}

impl HashAlgorithm {
    /// Returns the hash of the empty input.
    ///
    /// This returns `None` for some hash algorithms, so the caller
    /// should be prepared to do the computation themselves in this case.
    pub(crate) fn hash_for_empty_input(&self) -> Option<hash::Output> {
        match self {
            Self::SHA256 => Some(hash::Output::new(
                b"\xe3\xb0\xc4\x42\x98\xfc\x1c\x14\
                  \x9a\xfb\xf4\xc8\x99\x6f\xb9\x24\
                  \x27\xae\x41\xe4\x64\x9b\x93\x4c\
                  \xa4\x95\x99\x1b\x78\x52\xb8\x55",
            )),
            Self::SHA384 => Some(hash::Output::new(
                b"\x38\xb0\x60\xa7\x51\xac\x96\x38\
                  \x4c\xd9\x32\x7e\xb1\xb1\xe3\x6a\
                  \x21\xfd\xb7\x11\x14\xbe\x07\x43\
                  \x4c\x0c\xc7\xbf\x63\xf6\xe1\xda\
                  \x27\x4e\xde\xbf\xe7\x6f\x65\xfb\
                  \xd5\x1a\xd2\xf1\x48\x98\xb9\x5b",
            )),
            _ => None,
        }
    }
}

enum_builder! {
    /// The `ClientCertificateType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub(crate) enum ClientCertificateType {
        RSASign => 0x01,
        DSSSign => 0x02,
        RSAFixedDH => 0x03,
        DSSFixedDH => 0x04,
        RSAEphemeralDH => 0x05,
        DSSEphemeralDH => 0x06,
        FortezzaDMS => 0x14,
        ECDSASign => 0x40,
        RSAFixedECDH => 0x41,
        ECDSAFixedECDH => 0x42,
    }
}

enum_builder! {
    /// The `Compression` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum Compression {
        Null => 0x00,
        Deflate => 0x01,
        LSZ => 0x40,
    }
}

enum_builder! {
    /// The `AlertLevel` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum AlertLevel {
        Warning => 0x01,
        Fatal => 0x02,
    }
}

enum_builder! {
    /// The `HeartbeatMessageType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub(crate) enum HeartbeatMessageType {
        Request => 0x01,
        Response => 0x02,
    }
}

enum_builder! {
    /// The `ExtensionType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u16)]
    pub enum ExtensionType {
        ServerName => 0x0000,
        MaxFragmentLength => 0x0001,
        ClientCertificateUrl => 0x0002,
        TrustedCAKeys => 0x0003,
        TruncatedHMAC => 0x0004,
        StatusRequest => 0x0005,
        UserMapping => 0x0006,
        ClientAuthz => 0x0007,
        ServerAuthz => 0x0008,
        CertificateType => 0x0009,
        EllipticCurves => 0x000a,
        ECPointFormats => 0x000b,
        SRP => 0x000c,
        SignatureAlgorithms => 0x000d,
        UseSRTP => 0x000e,
        Heartbeat => 0x000f,
        ALProtocolNegotiation => 0x0010,
        SCT => 0x0012,
        ClientCertificateType => 0x0013,
        ServerCertificateType => 0x0014,
        Padding => 0x0015,
        ExtendedMasterSecret => 0x0017,
        CompressCertificate => 0x001b,
        SessionTicket => 0x0023,
        PreSharedKey => 0x0029,
        EarlyData => 0x002a,
        SupportedVersions => 0x002b,
        Cookie => 0x002c,
        PSKKeyExchangeModes => 0x002d,
        TicketEarlyDataInfo => 0x002e,
        CertificateAuthorities => 0x002f,
        OIDFilters => 0x0030,
        PostHandshakeAuth => 0x0031,
        SignatureAlgorithmsCert => 0x0032,
        KeyShare => 0x0033,
        TransportParameters => 0x0039,
        NextProtocolNegotiation => 0x3374,
        ChannelId => 0x754f,
        RenegotiationInfo => 0xff01,
        TransportParametersDraft => 0xffa5,
        EncryptedClientHello => 0xfe0d, // https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-11.1
        EncryptedClientHelloOuterExtensions => 0xfd00, // https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-5.1
    }
}

impl ExtensionType {
    /// Returns true if the extension type can be compressed in an "inner" client hello for ECH.
    ///
    /// This function should only return true for extension types where the inner hello and outer
    /// hello extensions values will always be identical. Extensions that may be identical
    /// sometimes (e.g. server name, cert compression methods), but not always, SHOULD NOT be
    /// compressed.
    ///
    /// See [draft-ietf-esni-18 §5](https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-5)
    /// and [draft-ietf-esni-18 §10.5](https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-10.5)
    /// for more information.
    pub(crate) fn ech_compress(&self) -> bool {
        // We match which extensions we will compress with BoringSSL and Go's stdlib.
        matches!(
            self,
            Self::StatusRequest
                | Self::EllipticCurves
                | Self::SignatureAlgorithms
                | Self::SignatureAlgorithmsCert
                | Self::ALProtocolNegotiation
                | Self::SupportedVersions
                | Self::Cookie
                | Self::KeyShare
                | Self::PSKKeyExchangeModes
        )
    }
}

enum_builder! {
    /// The `ServerNameType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub(crate) enum ServerNameType {
        HostName => 0x00,
    }
}

enum_builder! {
    /// The `NamedCurve` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    ///
    /// This enum is used for recognizing elliptic curve parameters advertised
    /// by a peer during a TLS handshake. It is **not** a list of curves that
    /// Rustls supports. See [`crate::crypto::ring::kx_group`] for the list of supported
    /// elliptic curve groups.
    #[repr(u16)]
    pub(crate) enum NamedCurve {
        sect163k1 => 0x0001,
        sect163r1 => 0x0002,
        sect163r2 => 0x0003,
        sect193r1 => 0x0004,
        sect193r2 => 0x0005,
        sect233k1 => 0x0006,
        sect233r1 => 0x0007,
        sect239k1 => 0x0008,
        sect283k1 => 0x0009,
        sect283r1 => 0x000a,
        sect409k1 => 0x000b,
        sect409r1 => 0x000c,
        sect571k1 => 0x000d,
        sect571r1 => 0x000e,
        secp160k1 => 0x000f,
        secp160r1 => 0x0010,
        secp160r2 => 0x0011,
        secp192k1 => 0x0012,
        secp192r1 => 0x0013,
        secp224k1 => 0x0014,
        secp224r1 => 0x0015,
        secp256k1 => 0x0016,
        secp256r1 => 0x0017,
        secp384r1 => 0x0018,
        secp521r1 => 0x0019,
        brainpoolp256r1 => 0x001a,
        brainpoolp384r1 => 0x001b,
        brainpoolp512r1 => 0x001c,
        X25519 => 0x001d,
        X448 => 0x001e,
        arbitrary_explicit_prime_curves => 0xff01,
        arbitrary_explicit_char2_curves => 0xff02,
    }
}

enum_builder! {
    /// The `NamedGroup` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u16)]
    pub enum NamedGroup {
        secp256r1 => 0x0017,
        secp384r1 => 0x0018,
        secp521r1 => 0x0019,
        X25519 => 0x001d,
        X448 => 0x001e,
        FFDHE2048 => 0x0100,
        FFDHE3072 => 0x0101,
        FFDHE4096 => 0x0102,
        FFDHE6144 => 0x0103,
        FFDHE8192 => 0x0104,
        MLKEM512 => 0x0200,
        MLKEM768 => 0x0201,
        MLKEM1024 => 0x0202,
        secp256r1MLKEM768 => 0x11eb,
        X25519MLKEM768 => 0x11ec,
    }
}

impl NamedGroup {
    /// Return the key exchange algorithm associated with this `NamedGroup`
    pub fn key_exchange_algorithm(self) -> KeyExchangeAlgorithm {
        match u16::from(self) {
            x if (0x100..0x200).contains(&x) => KeyExchangeAlgorithm::DHE,
            _ => KeyExchangeAlgorithm::ECDHE,
        }
    }
}

enum_builder! {
    /// The `ECPointFormat` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum ECPointFormat {
        Uncompressed => 0x00,
        ANSIX962CompressedPrime => 0x01,
        ANSIX962CompressedChar2 => 0x02,
    }
}

enum_builder! {
    /// The `HeartbeatMode` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub(crate) enum HeartbeatMode {
        PeerAllowedToSend => 0x01,
        PeerNotAllowedToSend => 0x02,
    }
}

enum_builder! {
    /// The `ECCurveType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub(crate) enum ECCurveType {
        ExplicitPrime => 0x01,
        ExplicitChar2 => 0x02,
        NamedCurve => 0x03,
    }
}

enum_builder! {
    /// The `PskKeyExchangeMode` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum PskKeyExchangeMode {
        PSK_KE => 0x00,
        PSK_DHE_KE => 0x01,
    }
}

enum_builder! {
    /// The `KeyUpdateRequest` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum KeyUpdateRequest {
        UpdateNotRequested => 0x00,
        UpdateRequested => 0x01,
    }
}

enum_builder! {
    /// The `CertificateStatusType` TLS protocol enum.  Values in this enum are taken
    /// from the various RFCs covering TLS, and are listed by IANA.
    /// The `Unknown` item is used when processing unrecognised ordinals.
    #[repr(u8)]
    pub enum CertificateStatusType {
        OCSP => 0x01,
    }
}

enum_builder! {
    /// The Key Encapsulation Mechanism (`Kem`) type for HPKE operations.
    /// Listed by IANA, as specified in [RFC 9180 Section 7.1]
    ///
    /// [RFC 9180 Section 7.1]: <https://datatracker.ietf.org/doc/html/rfc9180#kemid-values>
    #[repr(u16)]
    pub enum HpkeKem {
        DHKEM_P256_HKDF_SHA256 => 0x0010,
        DHKEM_P384_HKDF_SHA384 => 0x0011,
        DHKEM_P521_HKDF_SHA512 => 0x0012,
        DHKEM_X25519_HKDF_SHA256 => 0x0020,
        DHKEM_X448_HKDF_SHA512 => 0x0021,
    }
}

enum_builder! {
    /// The Key Derivation Function (`Kdf`) type for HPKE operations.
    /// Listed by IANA, as specified in [RFC 9180 Section 7.2]
    ///
    /// [RFC 9180 Section 7.2]: <https://datatracker.ietf.org/doc/html/rfc9180#name-key-derivation-functions-kd>
    #[repr(u16)]
    pub enum HpkeKdf {
        HKDF_SHA256 => 0x0001,
        HKDF_SHA384 => 0x0002,
        HKDF_SHA512 => 0x0003,
    }
}

impl Default for HpkeKdf {
    // TODO(XXX): revisit the default configuration. This is just what Cloudflare ships right now.
    fn default() -> Self {
        Self::HKDF_SHA256
    }
}

enum_builder! {
    /// The Authenticated Encryption with Associated Data (`Aead`) type for HPKE operations.
    /// Listed by IANA, as specified in [RFC 9180 Section 7.3]
    ///
    /// [RFC 9180 Section 7.3]: <https://datatracker.ietf.org/doc/html/rfc9180#name-authenticated-encryption-wi>
    #[repr(u16)]
    pub enum HpkeAead {
        AES_128_GCM => 0x0001,
        AES_256_GCM => 0x0002,
        CHACHA20_POLY_1305 => 0x0003,
        EXPORT_ONLY => 0xFFFF,
    }
}

impl HpkeAead {
    /// Returns the length of the tag for the AEAD algorithm, or none if the AEAD is EXPORT_ONLY.
    pub(crate) fn tag_len(&self) -> Option<usize> {
        match self {
            // See RFC 9180 Section 7.3, column `Nt`, the length in bytes of the authentication tag
            // for the algorithm.
            // https://www.rfc-editor.org/rfc/rfc9180.html#section-7.3
            Self::AES_128_GCM | Self::AES_256_GCM | Self::CHACHA20_POLY_1305 => Some(16),
            _ => None,
        }
    }
}

impl Default for HpkeAead {
    // TODO(XXX): revisit the default configuration. This is just what Cloudflare ships right now.
    fn default() -> Self {
        Self::AES_128_GCM
    }
}

enum_builder! {
    /// The Encrypted Client Hello protocol version (`EchVersion`).
    ///
    /// Specified in [draft-ietf-tls-esni Section 4].
    /// TODO(XXX): Update reference once RFC is published.
    ///
    /// [draft-ietf-tls-esni Section 4]: <https://www.ietf.org/archive/id/draft-ietf-tls-esni-17.html#section-4>
    #[repr(u16)]
    pub enum EchVersion {
        V18 => 0xfe0d,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    // These tests are intended to provide coverage and
    // check panic-safety of relatively unused values.

    use std::prelude::v1::*;

    use super::*;

    #[test]
    fn test_enums() {
        test_enum8::<HashAlgorithm>(HashAlgorithm::NONE, HashAlgorithm::SHA512);
        test_enum8::<ClientCertificateType>(
            ClientCertificateType::RSASign,
            ClientCertificateType::ECDSAFixedECDH,
        );
        test_enum8::<Compression>(Compression::Null, Compression::LSZ);
        test_enum8::<AlertLevel>(AlertLevel::Warning, AlertLevel::Fatal);
        test_enum8::<HeartbeatMessageType>(
            HeartbeatMessageType::Request,
            HeartbeatMessageType::Response,
        );
        test_enum16::<ExtensionType>(ExtensionType::ServerName, ExtensionType::RenegotiationInfo);
        test_enum8::<ServerNameType>(ServerNameType::HostName, ServerNameType::HostName);
        test_enum16::<NamedCurve>(
            NamedCurve::sect163k1,
            NamedCurve::arbitrary_explicit_char2_curves,
        );
        test_enum16::<NamedGroup>(NamedGroup::secp256r1, NamedGroup::FFDHE8192);
        test_enum8::<ECPointFormat>(
            ECPointFormat::Uncompressed,
            ECPointFormat::ANSIX962CompressedChar2,
        );
        test_enum8::<HeartbeatMode>(
            HeartbeatMode::PeerAllowedToSend,
            HeartbeatMode::PeerNotAllowedToSend,
        );
        test_enum8::<ECCurveType>(ECCurveType::ExplicitPrime, ECCurveType::NamedCurve);
        test_enum8::<PskKeyExchangeMode>(
            PskKeyExchangeMode::PSK_KE,
            PskKeyExchangeMode::PSK_DHE_KE,
        );
        test_enum8::<KeyUpdateRequest>(
            KeyUpdateRequest::UpdateNotRequested,
            KeyUpdateRequest::UpdateRequested,
        );
        test_enum8::<CertificateStatusType>(
            CertificateStatusType::OCSP,
            CertificateStatusType::OCSP,
        );
    }

    pub(crate) fn test_enum8<T: for<'a> Codec<'a>>(first: T, last: T) {
        let first_v = get8(&first);
        let last_v = get8(&last);

        for val in first_v..last_v + 1 {
            let mut buf = Vec::new();
            val.encode(&mut buf);
            assert_eq!(buf.len(), 1);

            let t = T::read_bytes(&buf).unwrap();
            assert_eq!(val, get8(&t));
        }
    }

    pub(crate) fn test_enum16<T: for<'a> Codec<'a>>(first: T, last: T) {
        let first_v = get16(&first);
        let last_v = get16(&last);

        for val in first_v..last_v + 1 {
            let mut buf = Vec::new();
            val.encode(&mut buf);
            assert_eq!(buf.len(), 2);

            let t = T::read_bytes(&buf).unwrap();
            assert_eq!(val, get16(&t));
        }
    }

    fn get8<T: for<'a> Codec<'a>>(enum_value: &T) -> u8 {
        let enc = enum_value.get_encoding();
        assert_eq!(enc.len(), 1);
        enc[0]
    }

    fn get16<T: for<'a> Codec<'a>>(enum_value: &T) -> u16 {
        let enc = enum_value.get_encoding();
        assert_eq!(enc.len(), 2);
        (enc[0] as u16 >> 8) | (enc[1] as u16)
    }
}
