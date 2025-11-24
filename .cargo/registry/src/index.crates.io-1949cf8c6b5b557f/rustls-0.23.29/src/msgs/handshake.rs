use alloc::boxed::Box;
use alloc::collections::BTreeSet;
#[cfg(feature = "logging")]
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Deref, DerefMut};
use core::{fmt, iter};

use pki_types::{CertificateDer, DnsName};

#[cfg(feature = "tls12")]
use crate::crypto::ActiveKeyExchange;
use crate::crypto::SecureRandom;
use crate::enums::{
    CertificateCompressionAlgorithm, CertificateType, CipherSuite, EchClientHelloType,
    HandshakeType, ProtocolVersion, SignatureScheme,
};
use crate::error::InvalidMessage;
#[cfg(feature = "tls12")]
use crate::ffdhe_groups::FfdheGroup;
use crate::log::warn;
use crate::msgs::base::{MaybeEmpty, NonEmpty, Payload, PayloadU8, PayloadU16, PayloadU24};
use crate::msgs::codec::{
    self, Codec, LengthPrefixedBuffer, ListLength, Reader, TlsListElement, TlsListIter,
};
use crate::msgs::enums::{
    CertificateStatusType, ClientCertificateType, Compression, ECCurveType, ECPointFormat,
    EchVersion, ExtensionType, HpkeAead, HpkeKdf, HpkeKem, KeyUpdateRequest, NamedGroup,
    PskKeyExchangeMode, ServerNameType,
};
use crate::rand;
use crate::sync::Arc;
use crate::verify::DigitallySignedStruct;
use crate::x509::wrap_in_sequence;

/// Create a newtype wrapper around a given type.
///
/// This is used to create newtypes for the various TLS message types which is used to wrap
/// the `PayloadU8` or `PayloadU16` types. This is typically used for types where we don't need
/// anything other than access to the underlying bytes.
macro_rules! wrapped_payload(
  ($(#[$comment:meta])* $vis:vis struct $name:ident, $inner:ident$(<$inner_ty:ty>)?,) => {
    $(#[$comment])*
    #[derive(Clone, Debug)]
    $vis struct $name($inner$(<$inner_ty>)?);

    impl From<Vec<u8>> for $name {
        fn from(v: Vec<u8>) -> Self {
            Self($inner::new(v))
        }
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.0.as_slice()
        }
    }

    impl Codec<'_> for $name {
        fn encode(&self, bytes: &mut Vec<u8>) {
            self.0.encode(bytes);
        }

        fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
            Ok(Self($inner::read(r)?))
        }
    }
  }
);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Random(pub(crate) [u8; 32]);

impl fmt::Debug for Random {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        super::base::hex(f, &self.0)
    }
}

static HELLO_RETRY_REQUEST_RANDOM: Random = Random([
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
]);

static ZERO_RANDOM: Random = Random([0u8; 32]);

impl Codec<'_> for Random {
    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let Some(bytes) = r.take(32) else {
            return Err(InvalidMessage::MissingData("Random"));
        };

        let mut opaque = [0; 32];
        opaque.clone_from_slice(bytes);
        Ok(Self(opaque))
    }
}

impl Random {
    pub(crate) fn new(secure_random: &dyn SecureRandom) -> Result<Self, rand::GetRandomFailed> {
        let mut data = [0u8; 32];
        secure_random.fill(&mut data)?;
        Ok(Self(data))
    }
}

impl From<[u8; 32]> for Random {
    #[inline]
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct SessionId {
    len: usize,
    data: [u8; 32],
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        super::base::hex(f, &self.data[..self.len])
    }
}

impl PartialEq for SessionId {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }

        let mut diff = 0u8;
        for i in 0..self.len {
            diff |= self.data[i] ^ other.data[i];
        }

        diff == 0u8
    }
}

impl Codec<'_> for SessionId {
    fn encode(&self, bytes: &mut Vec<u8>) {
        debug_assert!(self.len <= 32);
        bytes.push(self.len as u8);
        bytes.extend_from_slice(self.as_ref());
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let len = u8::read(r)? as usize;
        if len > 32 {
            return Err(InvalidMessage::TrailingData("SessionID"));
        }

        let Some(bytes) = r.take(len) else {
            return Err(InvalidMessage::MissingData("SessionID"));
        };

        let mut out = [0u8; 32];
        out[..len].clone_from_slice(&bytes[..len]);
        Ok(Self { data: out, len })
    }
}

impl SessionId {
    pub(crate) fn random(secure_random: &dyn SecureRandom) -> Result<Self, rand::GetRandomFailed> {
        let mut data = [0u8; 32];
        secure_random.fill(&mut data)?;
        Ok(Self { data, len: 32 })
    }

    pub(crate) fn empty() -> Self {
        Self {
            data: [0u8; 32],
            len: 0,
        }
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl AsRef<[u8]> for SessionId {
    fn as_ref(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnknownExtension {
    pub(crate) typ: ExtensionType,
    pub(crate) payload: Payload<'static>,
}

impl UnknownExtension {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.payload.encode(bytes);
    }

    fn read(typ: ExtensionType, r: &mut Reader<'_>) -> Self {
        let payload = Payload::read(r).into_owned();
        Self { typ, payload }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SupportedEcPointFormats {
    pub(crate) uncompressed: bool,
}

impl Codec<'_> for SupportedEcPointFormats {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let inner = LengthPrefixedBuffer::new(ECPointFormat::SIZE_LEN, bytes);

        if self.uncompressed {
            ECPointFormat::Uncompressed.encode(inner.buf);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let mut uncompressed = false;

        for pf in TlsListIter::<ECPointFormat>::new(r)? {
            if let ECPointFormat::Uncompressed = pf? {
                uncompressed = true;
            }
        }

        Ok(Self { uncompressed })
    }
}

impl Default for SupportedEcPointFormats {
    fn default() -> Self {
        Self { uncompressed: true }
    }
}

/// RFC8422: `ECPointFormat ec_point_format_list<1..2^8-1>`
impl TlsListElement for ECPointFormat {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("ECPointFormats"),
    };
}

/// RFC8422: `NamedCurve named_curve_list<2..2^16-1>`
impl TlsListElement for NamedGroup {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("NamedGroups"),
    };
}

/// RFC8446: `SignatureScheme supported_signature_algorithms<2..2^16-2>;`
impl TlsListElement for SignatureScheme {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::NoSignatureSchemes,
    };
}

#[derive(Clone, Debug)]
pub(crate) enum ServerNamePayload<'a> {
    /// A successfully decoded value:
    SingleDnsName(DnsName<'a>),

    /// A DNS name which was actually an IP address
    IpAddress,

    /// A successfully decoded, but syntactically-invalid value.
    Invalid,
}

impl ServerNamePayload<'_> {
    fn into_owned(self) -> ServerNamePayload<'static> {
        match self {
            Self::SingleDnsName(d) => ServerNamePayload::SingleDnsName(d.to_owned()),
            Self::IpAddress => ServerNamePayload::IpAddress,
            Self::Invalid => ServerNamePayload::Invalid,
        }
    }

    /// RFC6066: `ServerName server_name_list<1..2^16-1>`
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("ServerNames"),
    };
}

/// Simplified encoding/decoding for a `ServerName` extension payload to/from `DnsName`
///
/// This is possible because:
///
/// - the spec (RFC6066) disallows multiple names for a given name type
/// - name types other than ServerNameType::HostName are not defined, and they and
///   any data that follows them cannot be skipped over.
impl<'a> Codec<'a> for ServerNamePayload<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let server_name_list = LengthPrefixedBuffer::new(Self::SIZE_LEN, bytes);

        let ServerNamePayload::SingleDnsName(dns_name) = self else {
            return;
        };

        ServerNameType::HostName.encode(server_name_list.buf);
        let name_slice = dns_name.as_ref().as_bytes();
        (name_slice.len() as u16).encode(server_name_list.buf);
        server_name_list
            .buf
            .extend_from_slice(name_slice);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let mut found = None;

        let len = Self::SIZE_LEN.read(r)?;
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            let typ = ServerNameType::read(&mut sub)?;

            let payload = match typ {
                ServerNameType::HostName => HostNamePayload::read(&mut sub)?,
                _ => {
                    // Consume remainder of extension bytes.  Since the length of the item
                    // is an unknown encoding, we cannot continue.
                    sub.rest();
                    break;
                }
            };

            // "The ServerNameList MUST NOT contain more than one name of
            // the same name_type." - RFC6066
            if found.is_some() {
                warn!("Illegal SNI extension: duplicate host_name received");
                return Err(InvalidMessage::InvalidServerName);
            }

            found = match payload {
                HostNamePayload::HostName(dns_name) => {
                    Some(Self::SingleDnsName(dns_name.to_owned()))
                }

                HostNamePayload::IpAddress(_invalid) => {
                    warn!(
                        "Illegal SNI extension: ignoring IP address presented as hostname ({_invalid:?})"
                    );
                    Some(Self::IpAddress)
                }

                HostNamePayload::Invalid(_invalid) => {
                    warn!(
                        "Illegal SNI hostname received {:?}",
                        String::from_utf8_lossy(&_invalid.0)
                    );
                    Some(Self::Invalid)
                }
            };
        }

        Ok(found.unwrap_or(Self::Invalid))
    }
}

impl<'a> From<&DnsName<'a>> for ServerNamePayload<'static> {
    fn from(value: &DnsName<'a>) -> Self {
        Self::SingleDnsName(trim_hostname_trailing_dot_for_sni(value))
    }
}

#[derive(Clone, Debug)]
pub(crate) enum HostNamePayload {
    HostName(DnsName<'static>),
    IpAddress(PayloadU16<NonEmpty>),
    Invalid(PayloadU16<NonEmpty>),
}

impl HostNamePayload {
    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        use pki_types::ServerName;
        let raw = PayloadU16::<NonEmpty>::read(r)?;

        match ServerName::try_from(raw.0.as_slice()) {
            Ok(ServerName::DnsName(d)) => Ok(Self::HostName(d.to_owned())),
            Ok(ServerName::IpAddress(_)) => Ok(Self::IpAddress(raw)),
            Ok(_) | Err(_) => Ok(Self::Invalid(raw)),
        }
    }
}

wrapped_payload!(
    /// RFC7301: `opaque ProtocolName<1..2^8-1>;`
    pub(crate) struct ProtocolName, PayloadU8<NonEmpty>,
);

impl PartialEq for ProtocolName {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Deref for ProtocolName {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

/// RFC7301: `ProtocolName protocol_name_list<2..2^16-1>`
impl TlsListElement for ProtocolName {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("ProtocolNames"),
    };
}

/// RFC7301 encodes a single protocol name as `Vec<ProtocolName>`
#[derive(Clone, Debug)]
pub(crate) struct SingleProtocolName(ProtocolName);

impl SingleProtocolName {
    pub(crate) fn new(single: ProtocolName) -> Self {
        Self(single)
    }

    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("ProtocolNames"),
    };
}

impl Codec<'_> for SingleProtocolName {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let body = LengthPrefixedBuffer::new(Self::SIZE_LEN, bytes);
        self.0.encode(body.buf);
    }

    fn read(reader: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let len = Self::SIZE_LEN.read(reader)?;
        let mut sub = reader.sub(len)?;

        let item = ProtocolName::read(&mut sub)?;

        if sub.any_left() {
            Err(InvalidMessage::TrailingData("SingleProtocolName"))
        } else {
            Ok(Self(item))
        }
    }
}

impl AsRef<ProtocolName> for SingleProtocolName {
    fn as_ref(&self) -> &ProtocolName {
        &self.0
    }
}

// --- TLS 1.3 Key shares ---
#[derive(Clone, Debug)]
pub(crate) struct KeyShareEntry {
    pub(crate) group: NamedGroup,
    /// RFC8446: `opaque key_exchange<1..2^16-1>;`
    pub(crate) payload: PayloadU16<NonEmpty>,
}

impl KeyShareEntry {
    pub(crate) fn new(group: NamedGroup, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            group,
            payload: PayloadU16::new(payload.into()),
        }
    }
}

impl Codec<'_> for KeyShareEntry {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.group.encode(bytes);
        self.payload.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let group = NamedGroup::read(r)?;
        let payload = PayloadU16::read(r)?;

        Ok(Self { group, payload })
    }
}

// --- TLS 1.3 PresharedKey offers ---
#[derive(Clone, Debug)]
pub(crate) struct PresharedKeyIdentity {
    /// RFC8446: `opaque identity<1..2^16-1>;`
    pub(crate) identity: PayloadU16<NonEmpty>,
    pub(crate) obfuscated_ticket_age: u32,
}

impl PresharedKeyIdentity {
    pub(crate) fn new(id: Vec<u8>, age: u32) -> Self {
        Self {
            identity: PayloadU16::new(id),
            obfuscated_ticket_age: age,
        }
    }
}

impl Codec<'_> for PresharedKeyIdentity {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.identity.encode(bytes);
        self.obfuscated_ticket_age.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            identity: PayloadU16::read(r)?,
            obfuscated_ticket_age: u32::read(r)?,
        })
    }
}

/// RFC8446: `PskIdentity identities<7..2^16-1>;`
impl TlsListElement for PresharedKeyIdentity {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("PskIdentities"),
    };
}

wrapped_payload!(
    /// RFC8446: `opaque PskBinderEntry<32..255>;`
    pub(crate) struct PresharedKeyBinder, PayloadU8<NonEmpty>,
);

/// RFC8446: `PskBinderEntry binders<33..2^16-1>;`
impl TlsListElement for PresharedKeyBinder {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("PskBinders"),
    };
}

#[derive(Clone, Debug)]
pub(crate) struct PresharedKeyOffer {
    pub(crate) identities: Vec<PresharedKeyIdentity>,
    pub(crate) binders: Vec<PresharedKeyBinder>,
}

impl PresharedKeyOffer {
    /// Make a new one with one entry.
    pub(crate) fn new(id: PresharedKeyIdentity, binder: Vec<u8>) -> Self {
        Self {
            identities: vec![id],
            binders: vec![PresharedKeyBinder::from(binder)],
        }
    }
}

impl Codec<'_> for PresharedKeyOffer {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.identities.encode(bytes);
        self.binders.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            identities: Vec::read(r)?,
            binders: Vec::read(r)?,
        })
    }
}

// --- RFC6066 certificate status request ---
wrapped_payload!(pub(crate) struct ResponderId, PayloadU16,);

/// RFC6066: `ResponderID responder_id_list<0..2^16-1>;`
impl TlsListElement for ResponderId {
    const SIZE_LEN: ListLength = ListLength::U16;
}

#[derive(Clone, Debug)]
pub(crate) struct OcspCertificateStatusRequest {
    pub(crate) responder_ids: Vec<ResponderId>,
    pub(crate) extensions: PayloadU16,
}

impl Codec<'_> for OcspCertificateStatusRequest {
    fn encode(&self, bytes: &mut Vec<u8>) {
        CertificateStatusType::OCSP.encode(bytes);
        self.responder_ids.encode(bytes);
        self.extensions.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            responder_ids: Vec::read(r)?,
            extensions: PayloadU16::read(r)?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CertificateStatusRequest {
    Ocsp(OcspCertificateStatusRequest),
    Unknown((CertificateStatusType, Payload<'static>)),
}

impl Codec<'_> for CertificateStatusRequest {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Ocsp(r) => r.encode(bytes),
            Self::Unknown((typ, payload)) => {
                typ.encode(bytes);
                payload.encode(bytes);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let typ = CertificateStatusType::read(r)?;

        match typ {
            CertificateStatusType::OCSP => {
                let ocsp_req = OcspCertificateStatusRequest::read(r)?;
                Ok(Self::Ocsp(ocsp_req))
            }
            _ => {
                let data = Payload::read(r).into_owned();
                Ok(Self::Unknown((typ, data)))
            }
        }
    }
}

impl CertificateStatusRequest {
    pub(crate) fn build_ocsp() -> Self {
        let ocsp = OcspCertificateStatusRequest {
            responder_ids: Vec::new(),
            extensions: PayloadU16::empty(),
        };
        Self::Ocsp(ocsp)
    }
}

// ---

/// RFC8446: `PskKeyExchangeMode ke_modes<1..255>;`
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PskKeyExchangeModes {
    pub(crate) psk_dhe: bool,
    pub(crate) psk: bool,
}

impl Codec<'_> for PskKeyExchangeModes {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let inner = LengthPrefixedBuffer::new(PskKeyExchangeMode::SIZE_LEN, bytes);
        if self.psk_dhe {
            PskKeyExchangeMode::PSK_DHE_KE.encode(inner.buf);
        }
        if self.psk {
            PskKeyExchangeMode::PSK_KE.encode(inner.buf);
        }
    }

    fn read(reader: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let mut psk_dhe = false;
        let mut psk = false;

        for ke in TlsListIter::<PskKeyExchangeMode>::new(reader)? {
            match ke? {
                PskKeyExchangeMode::PSK_DHE_KE => psk_dhe = true,
                PskKeyExchangeMode::PSK_KE => psk = true,
                _ => continue,
            };
        }

        Ok(Self { psk_dhe, psk })
    }
}

impl TlsListElement for PskKeyExchangeMode {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("PskKeyExchangeModes"),
    };
}

/// RFC8446: `KeyShareEntry client_shares<0..2^16-1>;`
impl TlsListElement for KeyShareEntry {
    const SIZE_LEN: ListLength = ListLength::U16;
}

/// The body of the `SupportedVersions` extension when it appears in a
/// `ClientHello`
///
/// This is documented as a preference-order vector, but we (as a server)
/// ignore the preference of the client.
///
/// RFC8446: `ProtocolVersion versions<2..254>;`
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SupportedProtocolVersions {
    pub(crate) tls13: bool,
    pub(crate) tls12: bool,
}

impl SupportedProtocolVersions {
    /// Return true if `filter` returns true for any enabled version.
    pub(crate) fn any(&self, filter: impl Fn(ProtocolVersion) -> bool) -> bool {
        if self.tls13 && filter(ProtocolVersion::TLSv1_3) {
            return true;
        }
        if self.tls12 && filter(ProtocolVersion::TLSv1_2) {
            return true;
        }
        false
    }

    const LIST_LENGTH: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("ProtocolVersions"),
    };
}

impl Codec<'_> for SupportedProtocolVersions {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let inner = LengthPrefixedBuffer::new(Self::LIST_LENGTH, bytes);
        if self.tls13 {
            ProtocolVersion::TLSv1_3.encode(inner.buf);
        }
        if self.tls12 {
            ProtocolVersion::TLSv1_2.encode(inner.buf);
        }
    }

    fn read(reader: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let mut tls12 = false;
        let mut tls13 = false;

        for pv in TlsListIter::<ProtocolVersion>::new(reader)? {
            match pv? {
                ProtocolVersion::TLSv1_3 => tls13 = true,
                ProtocolVersion::TLSv1_2 => tls12 = true,
                _ => continue,
            };
        }

        Ok(Self { tls13, tls12 })
    }
}

impl TlsListElement for ProtocolVersion {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("ProtocolVersions"),
    };
}

/// RFC7250: `CertificateType client_certificate_types<1..2^8-1>;`
///
/// Ditto `CertificateType server_certificate_types<1..2^8-1>;`
impl TlsListElement for CertificateType {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("CertificateTypes"),
    };
}

/// RFC8879: `CertificateCompressionAlgorithm algorithms<2..2^8-2>;`
impl TlsListElement for CertificateCompressionAlgorithm {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("CertificateCompressionAlgorithms"),
    };
}

/// A precursor to `ClientExtensions`, allowing customisation.
///
/// This is smaller than `ClientExtensions`, as it only contains the extensions
/// we need to vary between different protocols (eg, TCP-TLS versus QUIC).
#[derive(Clone, Default)]
pub(crate) struct ClientExtensionsInput<'a> {
    /// QUIC transport parameters
    pub(crate) transport_parameters: Option<TransportParameters<'a>>,

    /// ALPN protocols
    pub(crate) protocols: Option<Vec<ProtocolName>>,
}

impl ClientExtensionsInput<'_> {
    pub(crate) fn from_alpn(alpn_protocols: Vec<Vec<u8>>) -> ClientExtensionsInput<'static> {
        let protocols = match alpn_protocols.is_empty() {
            true => None,
            false => Some(
                alpn_protocols
                    .into_iter()
                    .map(ProtocolName::from)
                    .collect::<Vec<_>>(),
            ),
        };

        ClientExtensionsInput {
            transport_parameters: None,
            protocols,
        }
    }

    pub(crate) fn into_owned(self) -> ClientExtensionsInput<'static> {
        let Self {
            transport_parameters,
            protocols,
        } = self;
        ClientExtensionsInput {
            transport_parameters: transport_parameters.map(|x| x.into_owned()),
            protocols,
        }
    }
}

#[derive(Clone)]
pub(crate) enum TransportParameters<'a> {
    /// QUIC transport parameters (RFC9001 prior to draft 33)
    QuicDraft(Payload<'a>),

    /// QUIC transport parameters (RFC9001)
    Quic(Payload<'a>),
}

impl TransportParameters<'_> {
    pub(crate) fn into_owned(self) -> TransportParameters<'static> {
        match self {
            Self::QuicDraft(v) => TransportParameters::QuicDraft(v.into_owned()),
            Self::Quic(v) => TransportParameters::Quic(v.into_owned()),
        }
    }
}

extension_struct! {
    /// A representation of extensions present in a `ClientHello` message
    ///
    /// All extensions are optional (by definition) so are represented with `Option<T>`.
    ///
    /// Some extensions have an empty value and are represented with Option<()>.
    ///
    /// Unknown extensions are dropped during parsing.
    pub(crate) struct ClientExtensions<'a> {
        /// Requested server name indication (RFC6066)
        ExtensionType::ServerName =>
            pub(crate) server_name: Option<ServerNamePayload<'a>>,

        /// Certificate status is requested (RFC6066)
        ExtensionType::StatusRequest =>
            pub(crate) certificate_status_request: Option<CertificateStatusRequest>,

        /// Supported groups (RFC4492/RFC8446)
        ExtensionType::EllipticCurves =>
            pub(crate) named_groups: Option<Vec<NamedGroup>>,

        /// Supported EC point formats (RFC4492)
        ExtensionType::ECPointFormats =>
            pub(crate) ec_point_formats: Option<SupportedEcPointFormats>,

        /// Supported signature schemes (RFC5246/RFC8446)
        ExtensionType::SignatureAlgorithms =>
            pub(crate) signature_schemes: Option<Vec<SignatureScheme>>,

        /// Offered ALPN protocols (RFC6066)
        ExtensionType::ALProtocolNegotiation =>
            pub(crate) protocols: Option<Vec<ProtocolName>>,

        /// Available client certificate types (RFC7250)
        ExtensionType::ClientCertificateType =>
            pub(crate) client_certificate_types: Option<Vec<CertificateType>>,

        /// Acceptable server certificate types (RFC7250)
        ExtensionType::ServerCertificateType =>
            pub(crate) server_certificate_types: Option<Vec<CertificateType>>,

        /// Extended master secret is requested (RFC7627)
        ExtensionType::ExtendedMasterSecret =>
            pub(crate) extended_master_secret_request: Option<()>,

        /// Offered certificate compression methods (RFC8879)
        ExtensionType::CompressCertificate =>
            pub(crate) certificate_compression_algorithms: Option<Vec<CertificateCompressionAlgorithm>>,

        /// Session ticket offer or request (RFC5077/RFC8446)
        ExtensionType::SessionTicket =>
            pub(crate) session_ticket: Option<ClientSessionTicket>,

        /// Offered preshared keys (RFC8446)
        ExtensionType::PreSharedKey =>
            pub(crate) preshared_key_offer: Option<PresharedKeyOffer>,

        /// Early data is requested (RFC8446)
        ExtensionType::EarlyData =>
            pub(crate) early_data_request: Option<()>,

        /// Supported TLS versions (RFC8446)
        ExtensionType::SupportedVersions =>
            pub(crate) supported_versions: Option<SupportedProtocolVersions>,

        /// Stateless HelloRetryRequest cookie (RFC8446)
        ExtensionType::Cookie =>
            pub(crate) cookie: Option<PayloadU16<NonEmpty>>,

        /// Offered preshared key modes (RFC8446)
        ExtensionType::PSKKeyExchangeModes =>
            pub(crate) preshared_key_modes: Option<PskKeyExchangeModes>,

        /// Certificate authority names (RFC8446)
        ExtensionType::CertificateAuthorities =>
            pub(crate) certificate_authority_names: Option<Vec<DistinguishedName>>,

        /// Offered key exchange shares (RFC8446)
        ExtensionType::KeyShare =>
            pub(crate) key_shares: Option<Vec<KeyShareEntry>>,

        /// QUIC transport parameters (RFC9001)
        ExtensionType::TransportParameters =>
            pub(crate) transport_parameters: Option<Payload<'a>>,

        /// Secure renegotiation (RFC5746)
        ExtensionType::RenegotiationInfo =>
            pub(crate) renegotiation_info: Option<PayloadU8>,

        /// QUIC transport parameters (RFC9001 prior to draft 33)
        ExtensionType::TransportParametersDraft =>
            pub(crate) transport_parameters_draft: Option<Payload<'a>>,

        /// Encrypted inner client hello (draft-ietf-tls-esni)
        ExtensionType::EncryptedClientHello =>
            pub(crate) encrypted_client_hello: Option<EncryptedClientHello>,

        /// Encrypted client hello outer extensions (draft-ietf-tls-esni)
        ExtensionType::EncryptedClientHelloOuterExtensions =>
            pub(crate) encrypted_client_hello_outer: Option<Vec<ExtensionType>>,
    } + {
        /// Order randomization seed.
        pub(crate) order_seed: u16,

        /// Extensions that must appear contiguously.
        pub(crate) contiguous_extensions: Vec<ExtensionType>,
    }
}

impl ClientExtensions<'_> {
    pub(crate) fn into_owned(self) -> ClientExtensions<'static> {
        let Self {
            server_name,
            certificate_status_request,
            named_groups,
            ec_point_formats,
            signature_schemes,
            protocols,
            client_certificate_types,
            server_certificate_types,
            extended_master_secret_request,
            certificate_compression_algorithms,
            session_ticket,
            preshared_key_offer,
            early_data_request,
            supported_versions,
            cookie,
            preshared_key_modes,
            certificate_authority_names,
            key_shares,
            transport_parameters,
            renegotiation_info,
            transport_parameters_draft,
            encrypted_client_hello,
            encrypted_client_hello_outer,
            order_seed,
            contiguous_extensions,
        } = self;
        ClientExtensions {
            server_name: server_name.map(|x| x.into_owned()),
            certificate_status_request,
            named_groups,
            ec_point_formats,
            signature_schemes,
            protocols,
            client_certificate_types,
            server_certificate_types,
            extended_master_secret_request,
            certificate_compression_algorithms,
            session_ticket,
            preshared_key_offer,
            early_data_request,
            supported_versions,
            cookie,
            preshared_key_modes,
            certificate_authority_names,
            key_shares,
            transport_parameters: transport_parameters.map(|x| x.into_owned()),
            renegotiation_info,
            transport_parameters_draft: transport_parameters_draft.map(|x| x.into_owned()),
            encrypted_client_hello,
            encrypted_client_hello_outer,
            order_seed,
            contiguous_extensions,
        }
    }

    pub(crate) fn used_extensions_in_encoding_order(&self) -> Vec<ExtensionType> {
        let mut exts = self.order_insensitive_extensions_in_random_order();
        exts.extend(&self.contiguous_extensions);

        if self
            .encrypted_client_hello_outer
            .is_some()
        {
            exts.push(ExtensionType::EncryptedClientHelloOuterExtensions);
        }
        if self.encrypted_client_hello.is_some() {
            exts.push(ExtensionType::EncryptedClientHello);
        }
        if self.preshared_key_offer.is_some() {
            exts.push(ExtensionType::PreSharedKey);
        }
        exts
    }

    /// Returns extensions which don't need a specific order, in randomized order.
    ///
    /// Extensions are encoded in three portions:
    ///
    /// - First, extensions not otherwise dealt with by other cases.
    ///   These are encoded in random order, controlled by `self.order_seed`,
    ///   and this is the set of extensions returned by this function.
    ///
    /// - Second, extensions named in `self.contiguous_extensions`, in the order
    ///   given by that field.
    ///
    /// - Lastly, any ECH and PSK extensions (in that order).  These
    ///   are required to be last by the standard.
    fn order_insensitive_extensions_in_random_order(&self) -> Vec<ExtensionType> {
        let mut order = self.collect_used();

        // Remove extensions which have specific order requirements.
        order.retain(|ext| {
            !(matches!(
                ext,
                ExtensionType::PreSharedKey
                    | ExtensionType::EncryptedClientHello
                    | ExtensionType::EncryptedClientHelloOuterExtensions
            ) || self.contiguous_extensions.contains(ext))
        });

        order.sort_by_cached_key(|new_ext| {
            let seed = ((self.order_seed as u32) << 16) | (u16::from(*new_ext) as u32);
            low_quality_integer_hash(seed)
        });

        order
    }
}

impl<'a> Codec<'a> for ClientExtensions<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let order = self.used_extensions_in_encoding_order();

        if order.is_empty() {
            return;
        }

        let body = LengthPrefixedBuffer::new(ListLength::U16, bytes);
        for item in order {
            self.encode_one(item, body.buf);
        }
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();

        // extensions length can be absent if no extensions
        if !r.any_left() {
            return Ok(out);
        }

        let mut checker = DuplicateExtensionChecker::new();

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            let typ = out.read_one(&mut sub, |unknown| checker.check(unknown))?;

            // PreSharedKey offer must come last
            if typ == ExtensionType::PreSharedKey && sub.any_left() {
                return Err(InvalidMessage::PreSharedKeyIsNotFinalExtension);
            }
        }

        Ok(out)
    }
}

fn trim_hostname_trailing_dot_for_sni(dns_name: &DnsName<'_>) -> DnsName<'static> {
    let dns_name_str = dns_name.as_ref();

    // RFC6066: "The hostname is represented as a byte string using
    // ASCII encoding without a trailing dot"
    if dns_name_str.ends_with('.') {
        let trimmed = &dns_name_str[0..dns_name_str.len() - 1];
        DnsName::try_from(trimmed)
            .unwrap()
            .to_owned()
    } else {
        dns_name.to_owned()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ClientSessionTicket {
    Request,
    Offer(Payload<'static>),
}

impl<'a> Codec<'a> for ClientSessionTicket {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Request => (),
            Self::Offer(p) => p.encode(bytes),
        }
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Ok(match r.left() {
            0 => Self::Request,
            _ => Self::Offer(Payload::read(r).into_owned()),
        })
    }
}

#[derive(Default)]
pub(crate) struct ServerExtensionsInput<'a> {
    /// QUIC transport parameters
    pub(crate) transport_parameters: Option<TransportParameters<'a>>,
}

extension_struct! {
    pub(crate) struct ServerExtensions<'a> {
        /// Supported EC point formats (RFC4492)
        ExtensionType::ECPointFormats =>
            pub(crate) ec_point_formats: Option<SupportedEcPointFormats>,

        /// Server name indication acknowledgement (RFC6066)
        ExtensionType::ServerName =>
            pub(crate) server_name_ack: Option<()>,

        /// Session ticket acknowledgement (RFC5077)
        ExtensionType::SessionTicket =>
            pub(crate) session_ticket_ack: Option<()>,

        ExtensionType::RenegotiationInfo =>
            pub(crate) renegotiation_info: Option<PayloadU8>,

        /// Selected ALPN protocol (RFC7301)
        ExtensionType::ALProtocolNegotiation =>
            pub(crate) selected_protocol: Option<SingleProtocolName>,

        /// Key exchange server share (RFC8446)
        ExtensionType::KeyShare =>
            pub(crate) key_share: Option<KeyShareEntry>,

        /// Selected preshared key index (RFC8446)
        ExtensionType::PreSharedKey =>
            pub(crate) preshared_key: Option<u16>,

        /// Required client certificate type (RFC7250)
        ExtensionType::ClientCertificateType =>
            pub(crate) client_certificate_type: Option<CertificateType>,

        /// Selected server certificate type (RFC7250)
        ExtensionType::ServerCertificateType =>
            pub(crate) server_certificate_type: Option<CertificateType>,

        /// Extended master secret is in use (RFC7627)
        ExtensionType::ExtendedMasterSecret =>
            pub(crate) extended_master_secret_ack: Option<()>,

        /// Certificate status acknowledgement (RFC6066)
        ExtensionType::StatusRequest =>
            pub(crate) certificate_status_request_ack: Option<()>,

        /// Selected TLS version (RFC8446)
        ExtensionType::SupportedVersions =>
            pub(crate) selected_version: Option<ProtocolVersion>,

        /// QUIC transport parameters (RFC9001)
        ExtensionType::TransportParameters =>
            pub(crate) transport_parameters: Option<Payload<'a>>,

        /// QUIC transport parameters (RFC9001 prior to draft 33)
        ExtensionType::TransportParametersDraft =>
            pub(crate) transport_parameters_draft: Option<Payload<'a>>,

        /// Early data is accepted (RFC8446)
        ExtensionType::EarlyData =>
            pub(crate) early_data_ack: Option<()>,

        /// Encrypted inner client hello response (draft-ietf-tls-esni)
        ExtensionType::EncryptedClientHello =>
            pub(crate) encrypted_client_hello_ack: Option<ServerEncryptedClientHello>,
    } + {
        pub(crate) unknown_extensions: BTreeSet<u16>,
    }
}

impl ServerExtensions<'_> {
    fn into_owned(self) -> ServerExtensions<'static> {
        let Self {
            ec_point_formats,
            server_name_ack,
            session_ticket_ack,
            renegotiation_info,
            selected_protocol,
            key_share,
            preshared_key,
            client_certificate_type,
            server_certificate_type,
            extended_master_secret_ack,
            certificate_status_request_ack,
            selected_version,
            transport_parameters,
            transport_parameters_draft,
            early_data_ack,
            encrypted_client_hello_ack,
            unknown_extensions,
        } = self;
        ServerExtensions {
            ec_point_formats,
            server_name_ack,
            session_ticket_ack,
            renegotiation_info,
            selected_protocol,
            key_share,
            preshared_key,
            client_certificate_type,
            server_certificate_type,
            extended_master_secret_ack,
            certificate_status_request_ack,
            selected_version,
            transport_parameters: transport_parameters.map(|x| x.into_owned()),
            transport_parameters_draft: transport_parameters_draft.map(|x| x.into_owned()),
            early_data_ack,
            encrypted_client_hello_ack,
            unknown_extensions,
        }
    }
}

impl<'a> Codec<'a> for ServerExtensions<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let extensions = LengthPrefixedBuffer::new(ListLength::U16, bytes);

        for ext in Self::ALL_EXTENSIONS {
            self.encode_one(*ext, extensions.buf);
        }
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();
        let mut checker = DuplicateExtensionChecker::new();

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            out.read_one(&mut sub, |unknown| checker.check(unknown))?;
        }

        out.unknown_extensions = checker.0;
        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClientHelloPayload {
    pub(crate) client_version: ProtocolVersion,
    pub(crate) random: Random,
    pub(crate) session_id: SessionId,
    pub(crate) cipher_suites: Vec<CipherSuite>,
    pub(crate) compression_methods: Vec<Compression>,
    pub(crate) extensions: Box<ClientExtensions<'static>>,
}

impl ClientHelloPayload {
    pub(crate) fn ech_inner_encoding(&self, to_compress: Vec<ExtensionType>) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.payload_encode(&mut bytes, Encoding::EchInnerHello { to_compress });
        bytes
    }

    pub(crate) fn payload_encode(&self, bytes: &mut Vec<u8>, purpose: Encoding) {
        self.client_version.encode(bytes);
        self.random.encode(bytes);

        match purpose {
            // SessionID is required to be empty in the encoded inner client hello.
            Encoding::EchInnerHello { .. } => SessionId::empty().encode(bytes),
            _ => self.session_id.encode(bytes),
        }

        self.cipher_suites.encode(bytes);
        self.compression_methods.encode(bytes);

        let to_compress = match purpose {
            Encoding::EchInnerHello { to_compress } if !to_compress.is_empty() => to_compress,
            _ => {
                self.extensions.encode(bytes);
                return;
            }
        };

        let mut compressed = self.extensions.clone();

        // First, eliminate the full-fat versions of the extensions
        for e in &to_compress {
            compressed.clear(*e);
        }

        // Replace with the marker noting which extensions were elided.
        compressed.encrypted_client_hello_outer = Some(to_compress);

        // And encode as normal.
        compressed.encode(bytes);
    }

    pub(crate) fn has_keyshare_extension_with_duplicates(&self) -> bool {
        self.key_shares
            .as_ref()
            .map(|entries| {
                has_duplicates::<_, _, u16>(
                    entries
                        .iter()
                        .map(|kse| u16::from(kse.group)),
                )
            })
            .unwrap_or_default()
    }

    pub(crate) fn has_certificate_compression_extension_with_duplicates(&self) -> bool {
        if let Some(algs) = &self.certificate_compression_algorithms {
            has_duplicates::<_, _, u16>(algs.iter().cloned())
        } else {
            false
        }
    }
}

impl Codec<'_> for ClientHelloPayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.payload_encode(bytes, Encoding::Standard)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let ret = Self {
            client_version: ProtocolVersion::read(r)?,
            random: Random::read(r)?,
            session_id: SessionId::read(r)?,
            cipher_suites: Vec::read(r)?,
            compression_methods: Vec::read(r)?,
            extensions: Box::new(ClientExtensions::read(r)?.into_owned()),
        };

        match r.any_left() {
            true => Err(InvalidMessage::TrailingData("ClientHelloPayload")),
            false => Ok(ret),
        }
    }
}

impl Deref for ClientHelloPayload {
    type Target = ClientExtensions<'static>;
    fn deref(&self) -> &Self::Target {
        &self.extensions
    }
}

impl DerefMut for ClientHelloPayload {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.extensions
    }
}

/// RFC8446: `CipherSuite cipher_suites<2..2^16-2>;`
impl TlsListElement for CipherSuite {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("CipherSuites"),
    };
}

/// RFC5246: `CompressionMethod compression_methods<1..2^8-1>;`
impl TlsListElement for Compression {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("Compressions"),
    };
}

/// draft-ietf-tls-esni-17: `ExtensionType OuterExtensions<2..254>;`
impl TlsListElement for ExtensionType {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("ExtensionTypes"),
    };
}

extension_struct! {
    /// A representation of extensions present in a `HelloRetryRequest` message
    pub(crate) struct HelloRetryRequestExtensions<'a> {
        ExtensionType::KeyShare =>
            pub(crate) key_share: Option<NamedGroup>,

        ExtensionType::Cookie =>
            pub(crate) cookie: Option<PayloadU16<NonEmpty>>,

        ExtensionType::SupportedVersions =>
            pub(crate) supported_versions: Option<ProtocolVersion>,

        ExtensionType::EncryptedClientHello =>
            pub(crate) encrypted_client_hello: Option<Payload<'a>>,
    } + {
        /// Records decoding order of records, and controls encoding order.
        pub(crate) order: Option<Vec<ExtensionType>>,
    }
}

impl HelloRetryRequestExtensions<'_> {
    fn into_owned(self) -> HelloRetryRequestExtensions<'static> {
        let Self {
            key_share,
            cookie,
            supported_versions,
            encrypted_client_hello,
            order,
        } = self;
        HelloRetryRequestExtensions {
            key_share,
            cookie,
            supported_versions,
            encrypted_client_hello: encrypted_client_hello.map(|x| x.into_owned()),
            order,
        }
    }
}

impl<'a> Codec<'a> for HelloRetryRequestExtensions<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let extensions = LengthPrefixedBuffer::new(ListLength::U16, bytes);

        for ext in self
            .order
            .as_deref()
            .unwrap_or(Self::ALL_EXTENSIONS)
        {
            self.encode_one(*ext, extensions.buf);
        }
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();

        // we must record order, so re-encoding round trips.  this is needed,
        // unfortunately, for ECH HRR confirmation
        let mut order = vec![];

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            let typ = out.read_one(&mut sub, |_unk| {
                Err(InvalidMessage::UnknownHelloRetryRequestExtension)
            })?;

            order.push(typ);
        }

        out.order = Some(order);
        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HelloRetryRequest {
    pub(crate) legacy_version: ProtocolVersion,
    pub(crate) session_id: SessionId,
    pub(crate) cipher_suite: CipherSuite,
    pub(crate) extensions: HelloRetryRequestExtensions<'static>,
}

impl Codec<'_> for HelloRetryRequest {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.payload_encode(bytes, Encoding::Standard)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let session_id = SessionId::read(r)?;
        let cipher_suite = CipherSuite::read(r)?;
        let compression = Compression::read(r)?;

        if compression != Compression::Null {
            return Err(InvalidMessage::UnsupportedCompression);
        }

        Ok(Self {
            legacy_version: ProtocolVersion::Unknown(0),
            session_id,
            cipher_suite,
            extensions: HelloRetryRequestExtensions::read(r)?.into_owned(),
        })
    }
}

impl HelloRetryRequest {
    fn payload_encode(&self, bytes: &mut Vec<u8>, purpose: Encoding) {
        self.legacy_version.encode(bytes);
        HELLO_RETRY_REQUEST_RANDOM.encode(bytes);
        self.session_id.encode(bytes);
        self.cipher_suite.encode(bytes);
        Compression::Null.encode(bytes);

        match purpose {
            // For the purpose of ECH confirmation, the Encrypted Client Hello extension
            // must have its payload replaced by 8 zero bytes.
            //
            // See draft-ietf-tls-esni-18 7.2.1:
            // <https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#name-sending-helloretryrequest-2>
            Encoding::EchConfirmation
                if self
                    .extensions
                    .encrypted_client_hello
                    .is_some() =>
            {
                let hrr_confirmation = [0u8; 8];
                HelloRetryRequestExtensions {
                    encrypted_client_hello: Some(Payload::Borrowed(&hrr_confirmation)),
                    ..self.extensions.clone()
                }
                .encode(bytes);
            }
            _ => self.extensions.encode(bytes),
        }
    }
}

impl Deref for HelloRetryRequest {
    type Target = HelloRetryRequestExtensions<'static>;
    fn deref(&self) -> &Self::Target {
        &self.extensions
    }
}

impl DerefMut for HelloRetryRequest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.extensions
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ServerHelloPayload {
    pub(crate) legacy_version: ProtocolVersion,
    pub(crate) random: Random,
    pub(crate) session_id: SessionId,
    pub(crate) cipher_suite: CipherSuite,
    pub(crate) compression_method: Compression,
    pub(crate) extensions: Box<ServerExtensions<'static>>,
}

impl Codec<'_> for ServerHelloPayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.payload_encode(bytes, Encoding::Standard)
    }

    // minus version and random, which have already been read.
    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let session_id = SessionId::read(r)?;
        let suite = CipherSuite::read(r)?;
        let compression = Compression::read(r)?;

        // RFC5246:
        // "The presence of extensions can be detected by determining whether
        //  there are bytes following the compression_method field at the end of
        //  the ServerHello."
        let extensions = Box::new(
            if r.any_left() {
                ServerExtensions::read(r)?
            } else {
                ServerExtensions::default()
            }
            .into_owned(),
        );

        let ret = Self {
            legacy_version: ProtocolVersion::Unknown(0),
            random: ZERO_RANDOM,
            session_id,
            cipher_suite: suite,
            compression_method: compression,
            extensions,
        };

        r.expect_empty("ServerHelloPayload")
            .map(|_| ret)
    }
}

impl ServerHelloPayload {
    fn payload_encode(&self, bytes: &mut Vec<u8>, encoding: Encoding) {
        debug_assert!(
            !matches!(encoding, Encoding::EchConfirmation),
            "we cannot compute an ECH confirmation on a received ServerHello"
        );

        self.legacy_version.encode(bytes);
        self.random.encode(bytes);
        self.session_id.encode(bytes);
        self.cipher_suite.encode(bytes);
        self.compression_method.encode(bytes);
        self.extensions.encode(bytes);
    }
}

impl Deref for ServerHelloPayload {
    type Target = ServerExtensions<'static>;
    fn deref(&self) -> &Self::Target {
        &self.extensions
    }
}

impl DerefMut for ServerHelloPayload {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.extensions
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct CertificateChain<'a>(pub(crate) Vec<CertificateDer<'a>>);

impl CertificateChain<'_> {
    pub(crate) fn into_owned(self) -> CertificateChain<'static> {
        CertificateChain(
            self.0
                .into_iter()
                .map(|c| c.into_owned())
                .collect(),
        )
    }
}

impl<'a> Codec<'a> for CertificateChain<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        Vec::encode(&self.0, bytes)
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Vec::read(r).map(Self)
    }
}

impl<'a> Deref for CertificateChain<'a> {
    type Target = [CertificateDer<'a>];

    fn deref(&self) -> &[CertificateDer<'a>] {
        &self.0
    }
}

impl TlsListElement for CertificateDer<'_> {
    const SIZE_LEN: ListLength = ListLength::U24 {
        max: CERTIFICATE_MAX_SIZE_LIMIT,
        error: InvalidMessage::CertificatePayloadTooLarge,
    };
}

/// TLS has a 16MB size limit on any handshake message,
/// plus a 16MB limit on any given certificate.
///
/// We contract that to 64KB to limit the amount of memory allocation
/// that is directly controllable by the peer.
pub(crate) const CERTIFICATE_MAX_SIZE_LIMIT: usize = 0x1_0000;

extension_struct! {
    pub(crate) struct CertificateExtensions<'a> {
        ExtensionType::StatusRequest =>
            pub(crate) status: Option<CertificateStatus<'a>>,
    }
}

impl CertificateExtensions<'_> {
    fn into_owned(self) -> CertificateExtensions<'static> {
        CertificateExtensions {
            status: self.status.map(|s| s.into_owned()),
        }
    }
}

impl<'a> Codec<'a> for CertificateExtensions<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let extensions = LengthPrefixedBuffer::new(ListLength::U16, bytes);

        for ext in Self::ALL_EXTENSIONS {
            self.encode_one(*ext, extensions.buf);
        }
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            out.read_one(&mut sub, |_unk| {
                Err(InvalidMessage::UnknownCertificateExtension)
            })?;
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub(crate) struct CertificateEntry<'a> {
    pub(crate) cert: CertificateDer<'a>,
    pub(crate) extensions: CertificateExtensions<'a>,
}

impl<'a> Codec<'a> for CertificateEntry<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.cert.encode(bytes);
        self.extensions.encode(bytes);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            cert: CertificateDer::read(r)?,
            extensions: CertificateExtensions::read(r)?.into_owned(),
        })
    }
}

impl<'a> CertificateEntry<'a> {
    pub(crate) fn new(cert: CertificateDer<'a>) -> Self {
        Self {
            cert,
            extensions: CertificateExtensions::default(),
        }
    }

    pub(crate) fn into_owned(self) -> CertificateEntry<'static> {
        CertificateEntry {
            cert: self.cert.into_owned(),
            extensions: self.extensions.into_owned(),
        }
    }
}

impl TlsListElement for CertificateEntry<'_> {
    const SIZE_LEN: ListLength = ListLength::U24 {
        max: CERTIFICATE_MAX_SIZE_LIMIT,
        error: InvalidMessage::CertificatePayloadTooLarge,
    };
}

#[derive(Debug)]
pub(crate) struct CertificatePayloadTls13<'a> {
    pub(crate) context: PayloadU8,
    pub(crate) entries: Vec<CertificateEntry<'a>>,
}

impl<'a> Codec<'a> for CertificatePayloadTls13<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.context.encode(bytes);
        self.entries.encode(bytes);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            context: PayloadU8::read(r)?,
            entries: Vec::read(r)?,
        })
    }
}

impl<'a> CertificatePayloadTls13<'a> {
    pub(crate) fn new(
        certs: impl Iterator<Item = &'a CertificateDer<'a>>,
        ocsp_response: Option<&'a [u8]>,
    ) -> Self {
        Self {
            context: PayloadU8::empty(),
            entries: certs
                // zip certificate iterator with `ocsp_response` followed by
                // an infinite-length iterator of `None`.
                .zip(
                    ocsp_response
                        .into_iter()
                        .map(Some)
                        .chain(iter::repeat(None)),
                )
                .map(|(cert, ocsp)| {
                    let mut e = CertificateEntry::new(cert.clone());
                    if let Some(ocsp) = ocsp {
                        e.extensions.status = Some(CertificateStatus::new(ocsp));
                    }
                    e
                })
                .collect(),
        }
    }

    pub(crate) fn into_owned(self) -> CertificatePayloadTls13<'static> {
        CertificatePayloadTls13 {
            context: self.context,
            entries: self
                .entries
                .into_iter()
                .map(CertificateEntry::into_owned)
                .collect(),
        }
    }

    pub(crate) fn end_entity_ocsp(&self) -> Vec<u8> {
        let Some(entry) = self.entries.first() else {
            return vec![];
        };
        entry
            .extensions
            .status
            .as_ref()
            .map(|status| {
                status
                    .ocsp_response
                    .0
                    .clone()
                    .into_vec()
            })
            .unwrap_or_default()
    }

    pub(crate) fn into_certificate_chain(self) -> CertificateChain<'a> {
        CertificateChain(
            self.entries
                .into_iter()
                .map(|e| e.cert)
                .collect(),
        )
    }
}

/// Describes supported key exchange mechanisms.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum KeyExchangeAlgorithm {
    /// Diffie-Hellman Key exchange (with only known parameters as defined in [RFC 7919]).
    ///
    /// [RFC 7919]: https://datatracker.ietf.org/doc/html/rfc7919
    DHE,
    /// Key exchange performed via elliptic curve Diffie-Hellman.
    ECDHE,
}

pub(crate) static ALL_KEY_EXCHANGE_ALGORITHMS: &[KeyExchangeAlgorithm] =
    &[KeyExchangeAlgorithm::ECDHE, KeyExchangeAlgorithm::DHE];

// We don't support arbitrary curves.  It's a terrible
// idea and unnecessary attack surface.  Please,
// get a grip.
#[derive(Debug)]
pub(crate) struct EcParameters {
    pub(crate) curve_type: ECCurveType,
    pub(crate) named_group: NamedGroup,
}

impl Codec<'_> for EcParameters {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.curve_type.encode(bytes);
        self.named_group.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let ct = ECCurveType::read(r)?;
        if ct != ECCurveType::NamedCurve {
            return Err(InvalidMessage::UnsupportedCurveType);
        }

        let grp = NamedGroup::read(r)?;

        Ok(Self {
            curve_type: ct,
            named_group: grp,
        })
    }
}

#[cfg(feature = "tls12")]
pub(crate) trait KxDecode<'a>: fmt::Debug + Sized {
    /// Decode a key exchange message given the key_exchange `algo`
    fn decode(r: &mut Reader<'a>, algo: KeyExchangeAlgorithm) -> Result<Self, InvalidMessage>;
}

#[cfg(feature = "tls12")]
#[derive(Debug)]
pub(crate) enum ClientKeyExchangeParams {
    Ecdh(ClientEcdhParams),
    Dh(ClientDhParams),
}

#[cfg(feature = "tls12")]
impl ClientKeyExchangeParams {
    pub(crate) fn pub_key(&self) -> &[u8] {
        match self {
            Self::Ecdh(ecdh) => &ecdh.public.0,
            Self::Dh(dh) => &dh.public.0,
        }
    }

    pub(crate) fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Ecdh(ecdh) => ecdh.encode(buf),
            Self::Dh(dh) => dh.encode(buf),
        }
    }
}

#[cfg(feature = "tls12")]
impl KxDecode<'_> for ClientKeyExchangeParams {
    fn decode(r: &mut Reader<'_>, algo: KeyExchangeAlgorithm) -> Result<Self, InvalidMessage> {
        use KeyExchangeAlgorithm::*;
        Ok(match algo {
            ECDHE => Self::Ecdh(ClientEcdhParams::read(r)?),
            DHE => Self::Dh(ClientDhParams::read(r)?),
        })
    }
}

#[cfg(feature = "tls12")]
#[derive(Debug)]
pub(crate) struct ClientEcdhParams {
    /// RFC4492: `opaque point <1..2^8-1>;`
    pub(crate) public: PayloadU8<NonEmpty>,
}

#[cfg(feature = "tls12")]
impl Codec<'_> for ClientEcdhParams {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.public.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let pb = PayloadU8::read(r)?;
        Ok(Self { public: pb })
    }
}

#[cfg(feature = "tls12")]
#[derive(Debug)]
pub(crate) struct ClientDhParams {
    /// RFC5246: `opaque dh_Yc<1..2^16-1>;`
    pub(crate) public: PayloadU16<NonEmpty>,
}

#[cfg(feature = "tls12")]
impl Codec<'_> for ClientDhParams {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.public.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            public: PayloadU16::read(r)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ServerEcdhParams {
    pub(crate) curve_params: EcParameters,
    /// RFC4492: `opaque point <1..2^8-1>;`
    pub(crate) public: PayloadU8<NonEmpty>,
}

impl ServerEcdhParams {
    #[cfg(feature = "tls12")]
    pub(crate) fn new(kx: &dyn ActiveKeyExchange) -> Self {
        Self {
            curve_params: EcParameters {
                curve_type: ECCurveType::NamedCurve,
                named_group: kx.group(),
            },
            public: PayloadU8::new(kx.pub_key().to_vec()),
        }
    }
}

impl Codec<'_> for ServerEcdhParams {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.curve_params.encode(bytes);
        self.public.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let cp = EcParameters::read(r)?;
        let pb = PayloadU8::read(r)?;

        Ok(Self {
            curve_params: cp,
            public: pb,
        })
    }
}

#[derive(Debug)]
#[allow(non_snake_case)]
pub(crate) struct ServerDhParams {
    /// RFC5246: `opaque dh_p<1..2^16-1>;`
    pub(crate) dh_p: PayloadU16<NonEmpty>,
    /// RFC5246: `opaque dh_g<1..2^16-1>;`
    pub(crate) dh_g: PayloadU16<NonEmpty>,
    /// RFC5246: `opaque dh_Ys<1..2^16-1>;`
    pub(crate) dh_Ys: PayloadU16<NonEmpty>,
}

impl ServerDhParams {
    #[cfg(feature = "tls12")]
    pub(crate) fn new(kx: &dyn ActiveKeyExchange) -> Self {
        let Some(params) = kx.ffdhe_group() else {
            panic!("invalid NamedGroup for DHE key exchange: {:?}", kx.group());
        };

        Self {
            dh_p: PayloadU16::new(params.p.to_vec()),
            dh_g: PayloadU16::new(params.g.to_vec()),
            dh_Ys: PayloadU16::new(kx.pub_key().to_vec()),
        }
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn as_ffdhe_group(&self) -> FfdheGroup<'_> {
        FfdheGroup::from_params_trimming_leading_zeros(&self.dh_p.0, &self.dh_g.0)
    }
}

impl Codec<'_> for ServerDhParams {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.dh_p.encode(bytes);
        self.dh_g.encode(bytes);
        self.dh_Ys.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            dh_p: PayloadU16::read(r)?,
            dh_g: PayloadU16::read(r)?,
            dh_Ys: PayloadU16::read(r)?,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ServerKeyExchangeParams {
    Ecdh(ServerEcdhParams),
    Dh(ServerDhParams),
}

impl ServerKeyExchangeParams {
    #[cfg(feature = "tls12")]
    pub(crate) fn new(kx: &dyn ActiveKeyExchange) -> Self {
        match kx.group().key_exchange_algorithm() {
            KeyExchangeAlgorithm::DHE => Self::Dh(ServerDhParams::new(kx)),
            KeyExchangeAlgorithm::ECDHE => Self::Ecdh(ServerEcdhParams::new(kx)),
        }
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn pub_key(&self) -> &[u8] {
        match self {
            Self::Ecdh(ecdh) => &ecdh.public.0,
            Self::Dh(dh) => &dh.dh_Ys.0,
        }
    }

    pub(crate) fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Ecdh(ecdh) => ecdh.encode(buf),
            Self::Dh(dh) => dh.encode(buf),
        }
    }
}

#[cfg(feature = "tls12")]
impl KxDecode<'_> for ServerKeyExchangeParams {
    fn decode(r: &mut Reader<'_>, algo: KeyExchangeAlgorithm) -> Result<Self, InvalidMessage> {
        use KeyExchangeAlgorithm::*;
        Ok(match algo {
            ECDHE => Self::Ecdh(ServerEcdhParams::read(r)?),
            DHE => Self::Dh(ServerDhParams::read(r)?),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ServerKeyExchange {
    pub(crate) params: ServerKeyExchangeParams,
    pub(crate) dss: DigitallySignedStruct,
}

impl ServerKeyExchange {
    pub(crate) fn encode(&self, buf: &mut Vec<u8>) {
        self.params.encode(buf);
        self.dss.encode(buf);
    }
}

#[derive(Debug)]
pub(crate) enum ServerKeyExchangePayload {
    Known(ServerKeyExchange),
    Unknown(Payload<'static>),
}

impl From<ServerKeyExchange> for ServerKeyExchangePayload {
    fn from(value: ServerKeyExchange) -> Self {
        Self::Known(value)
    }
}

impl Codec<'_> for ServerKeyExchangePayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Known(x) => x.encode(bytes),
            Self::Unknown(x) => x.encode(bytes),
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        // read as Unknown, fully parse when we know the
        // KeyExchangeAlgorithm
        Ok(Self::Unknown(Payload::read(r).into_owned()))
    }
}

impl ServerKeyExchangePayload {
    #[cfg(feature = "tls12")]
    pub(crate) fn unwrap_given_kxa(&self, kxa: KeyExchangeAlgorithm) -> Option<ServerKeyExchange> {
        if let Self::Unknown(unk) = self {
            let mut rd = Reader::init(unk.bytes());

            let result = ServerKeyExchange {
                params: ServerKeyExchangeParams::decode(&mut rd, kxa).ok()?,
                dss: DigitallySignedStruct::read(&mut rd).ok()?,
            };

            if !rd.any_left() {
                return Some(result);
            };
        }

        None
    }
}

/// RFC5246: `ClientCertificateType certificate_types<1..2^8-1>;`
impl TlsListElement for ClientCertificateType {
    const SIZE_LEN: ListLength = ListLength::NonZeroU8 {
        empty_error: InvalidMessage::IllegalEmptyList("ClientCertificateTypes"),
    };
}

wrapped_payload!(
    /// A `DistinguishedName` is a `Vec<u8>` wrapped in internal types.
    ///
    /// It contains the DER or BER encoded [`Subject` field from RFC 5280](https://datatracker.ietf.org/doc/html/rfc5280#section-4.1.2.6)
    /// for a single certificate. The Subject field is [encoded as an RFC 5280 `Name`](https://datatracker.ietf.org/doc/html/rfc5280#page-116).
    /// It can be decoded using [x509-parser's FromDer trait](https://docs.rs/x509-parser/latest/x509_parser/prelude/trait.FromDer.html).
    ///
    /// ```ignore
    /// for name in distinguished_names {
    ///     use x509_parser::prelude::FromDer;
    ///     println!("{}", x509_parser::x509::X509Name::from_der(&name.0)?.1);
    /// }
    /// ```
    ///
    /// The TLS encoding is defined in RFC5246: `opaque DistinguishedName<1..2^16-1>;`
    pub struct DistinguishedName,
    PayloadU16<NonEmpty>,
);

impl DistinguishedName {
    /// Create a [`DistinguishedName`] after prepending its outer SEQUENCE encoding.
    ///
    /// This can be decoded using [x509-parser's FromDer trait](https://docs.rs/x509-parser/latest/x509_parser/prelude/trait.FromDer.html).
    ///
    /// ```ignore
    /// use x509_parser::prelude::FromDer;
    /// println!("{}", x509_parser::x509::X509Name::from_der(dn.as_ref())?.1);
    /// ```
    pub fn in_sequence(bytes: &[u8]) -> Self {
        Self(PayloadU16::new(wrap_in_sequence(bytes)))
    }
}

/// RFC8446: `DistinguishedName authorities<3..2^16-1>;` however,
/// RFC5246: `DistinguishedName certificate_authorities<0..2^16-1>;`
impl TlsListElement for DistinguishedName {
    const SIZE_LEN: ListLength = ListLength::U16;
}

#[derive(Debug)]
pub(crate) struct CertificateRequestPayload {
    pub(crate) certtypes: Vec<ClientCertificateType>,
    pub(crate) sigschemes: Vec<SignatureScheme>,
    pub(crate) canames: Vec<DistinguishedName>,
}

impl Codec<'_> for CertificateRequestPayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.certtypes.encode(bytes);
        self.sigschemes.encode(bytes);
        self.canames.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let certtypes = Vec::read(r)?;
        let sigschemes = Vec::read(r)?;
        let canames = Vec::read(r)?;

        if sigschemes.is_empty() {
            warn!("meaningless CertificateRequest message");
            Err(InvalidMessage::NoSignatureSchemes)
        } else {
            Ok(Self {
                certtypes,
                sigschemes,
                canames,
            })
        }
    }
}

extension_struct! {
    pub(crate) struct CertificateRequestExtensions {
        ExtensionType::SignatureAlgorithms =>
            pub(crate) signature_algorithms: Option<Vec<SignatureScheme>>,

        ExtensionType::CertificateAuthorities =>
            pub(crate) authority_names: Option<Vec<DistinguishedName>>,

        ExtensionType::CompressCertificate =>
            pub(crate) certificate_compression_algorithms: Option<Vec<CertificateCompressionAlgorithm>>,
    }
}

impl Codec<'_> for CertificateRequestExtensions {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let extensions = LengthPrefixedBuffer::new(ListLength::U16, bytes);

        for ext in Self::ALL_EXTENSIONS {
            self.encode_one(*ext, extensions.buf);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();

        let mut checker = DuplicateExtensionChecker::new();

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            out.read_one(&mut sub, |unknown| checker.check(unknown))?;
        }

        if out
            .signature_algorithms
            .as_ref()
            .map(|algs| algs.is_empty())
            .unwrap_or_default()
        {
            return Err(InvalidMessage::NoSignatureSchemes);
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub(crate) struct CertificateRequestPayloadTls13 {
    pub(crate) context: PayloadU8,
    pub(crate) extensions: CertificateRequestExtensions,
}

impl Codec<'_> for CertificateRequestPayloadTls13 {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.context.encode(bytes);
        self.extensions.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let context = PayloadU8::read(r)?;
        let extensions = CertificateRequestExtensions::read(r)?;

        Ok(Self {
            context,
            extensions,
        })
    }
}

// -- NewSessionTicket --
#[derive(Debug)]
pub(crate) struct NewSessionTicketPayload {
    pub(crate) lifetime_hint: u32,
    // Tickets can be large (KB), so we deserialise this straight
    // into an Arc, so it can be passed directly into the client's
    // session object without copying.
    pub(crate) ticket: Arc<PayloadU16>,
}

impl NewSessionTicketPayload {
    #[cfg(feature = "tls12")]
    pub(crate) fn new(lifetime_hint: u32, ticket: Vec<u8>) -> Self {
        Self {
            lifetime_hint,
            ticket: Arc::new(PayloadU16::new(ticket)),
        }
    }
}

impl Codec<'_> for NewSessionTicketPayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.lifetime_hint.encode(bytes);
        self.ticket.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let lifetime = u32::read(r)?;
        let ticket = Arc::new(PayloadU16::read(r)?);

        Ok(Self {
            lifetime_hint: lifetime,
            ticket,
        })
    }
}

// -- NewSessionTicket electric boogaloo --
extension_struct! {
    pub(crate) struct NewSessionTicketExtensions {
        ExtensionType::EarlyData =>
            pub(crate) max_early_data_size: Option<u32>,
    }
}

impl Codec<'_> for NewSessionTicketExtensions {
    fn encode(&self, bytes: &mut Vec<u8>) {
        let extensions = LengthPrefixedBuffer::new(ListLength::U16, bytes);

        for ext in Self::ALL_EXTENSIONS {
            self.encode_one(*ext, extensions.buf);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let mut out = Self::default();

        let mut checker = DuplicateExtensionChecker::new();

        let len = usize::from(u16::read(r)?);
        let mut sub = r.sub(len)?;

        while sub.any_left() {
            out.read_one(&mut sub, |unknown| checker.check(unknown))?;
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub(crate) struct NewSessionTicketPayloadTls13 {
    pub(crate) lifetime: u32,
    pub(crate) age_add: u32,
    pub(crate) nonce: PayloadU8,
    pub(crate) ticket: Arc<PayloadU16>,
    pub(crate) extensions: NewSessionTicketExtensions,
}

impl NewSessionTicketPayloadTls13 {
    pub(crate) fn new(lifetime: u32, age_add: u32, nonce: Vec<u8>, ticket: Vec<u8>) -> Self {
        Self {
            lifetime,
            age_add,
            nonce: PayloadU8::new(nonce),
            ticket: Arc::new(PayloadU16::new(ticket)),
            extensions: NewSessionTicketExtensions::default(),
        }
    }
}

impl Codec<'_> for NewSessionTicketPayloadTls13 {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.lifetime.encode(bytes);
        self.age_add.encode(bytes);
        self.nonce.encode(bytes);
        self.ticket.encode(bytes);
        self.extensions.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let lifetime = u32::read(r)?;
        let age_add = u32::read(r)?;
        let nonce = PayloadU8::read(r)?;
        // nb. RFC8446: `opaque ticket<1..2^16-1>;`
        let ticket = Arc::new(match PayloadU16::<NonEmpty>::read(r) {
            Err(InvalidMessage::IllegalEmptyValue) => Err(InvalidMessage::EmptyTicketValue),
            Err(err) => Err(err),
            Ok(pl) => Ok(PayloadU16::new(pl.0)),
        }?);
        let extensions = NewSessionTicketExtensions::read(r)?;

        Ok(Self {
            lifetime,
            age_add,
            nonce,
            ticket,
            extensions,
        })
    }
}

// -- RFC6066 certificate status types

/// Only supports OCSP
#[derive(Clone, Debug)]
pub(crate) struct CertificateStatus<'a> {
    pub(crate) ocsp_response: PayloadU24<'a>,
}

impl<'a> Codec<'a> for CertificateStatus<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        CertificateStatusType::OCSP.encode(bytes);
        self.ocsp_response.encode(bytes);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        let typ = CertificateStatusType::read(r)?;

        match typ {
            CertificateStatusType::OCSP => Ok(Self {
                ocsp_response: PayloadU24::read(r)?,
            }),
            _ => Err(InvalidMessage::InvalidCertificateStatusType),
        }
    }
}

impl<'a> CertificateStatus<'a> {
    pub(crate) fn new(ocsp: &'a [u8]) -> Self {
        CertificateStatus {
            ocsp_response: PayloadU24(Payload::Borrowed(ocsp)),
        }
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.ocsp_response.0.into_vec()
    }

    pub(crate) fn into_owned(self) -> CertificateStatus<'static> {
        CertificateStatus {
            ocsp_response: self.ocsp_response.into_owned(),
        }
    }
}

// -- RFC8879 compressed certificates

#[derive(Debug)]
pub(crate) struct CompressedCertificatePayload<'a> {
    pub(crate) alg: CertificateCompressionAlgorithm,
    pub(crate) uncompressed_len: u32,
    pub(crate) compressed: PayloadU24<'a>,
}

impl<'a> Codec<'a> for CompressedCertificatePayload<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.alg.encode(bytes);
        codec::u24(self.uncompressed_len).encode(bytes);
        self.compressed.encode(bytes);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            alg: CertificateCompressionAlgorithm::read(r)?,
            uncompressed_len: codec::u24::read(r)?.0,
            compressed: PayloadU24::read(r)?,
        })
    }
}

impl CompressedCertificatePayload<'_> {
    fn into_owned(self) -> CompressedCertificatePayload<'static> {
        CompressedCertificatePayload {
            compressed: self.compressed.into_owned(),
            ..self
        }
    }

    pub(crate) fn as_borrowed(&self) -> CompressedCertificatePayload<'_> {
        CompressedCertificatePayload {
            alg: self.alg,
            uncompressed_len: self.uncompressed_len,
            compressed: PayloadU24(Payload::Borrowed(self.compressed.0.bytes())),
        }
    }
}

#[derive(Debug)]
pub(crate) enum HandshakePayload<'a> {
    HelloRequest,
    ClientHello(ClientHelloPayload),
    ServerHello(ServerHelloPayload),
    HelloRetryRequest(HelloRetryRequest),
    Certificate(CertificateChain<'a>),
    CertificateTls13(CertificatePayloadTls13<'a>),
    CompressedCertificate(CompressedCertificatePayload<'a>),
    ServerKeyExchange(ServerKeyExchangePayload),
    CertificateRequest(CertificateRequestPayload),
    CertificateRequestTls13(CertificateRequestPayloadTls13),
    CertificateVerify(DigitallySignedStruct),
    ServerHelloDone,
    EndOfEarlyData,
    ClientKeyExchange(Payload<'a>),
    NewSessionTicket(NewSessionTicketPayload),
    NewSessionTicketTls13(NewSessionTicketPayloadTls13),
    EncryptedExtensions(Box<ServerExtensions<'a>>),
    KeyUpdate(KeyUpdateRequest),
    Finished(Payload<'a>),
    CertificateStatus(CertificateStatus<'a>),
    MessageHash(Payload<'a>),
    Unknown((HandshakeType, Payload<'a>)),
}

impl HandshakePayload<'_> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        use self::HandshakePayload::*;
        match self {
            HelloRequest | ServerHelloDone | EndOfEarlyData => {}
            ClientHello(x) => x.encode(bytes),
            ServerHello(x) => x.encode(bytes),
            HelloRetryRequest(x) => x.encode(bytes),
            Certificate(x) => x.encode(bytes),
            CertificateTls13(x) => x.encode(bytes),
            CompressedCertificate(x) => x.encode(bytes),
            ServerKeyExchange(x) => x.encode(bytes),
            ClientKeyExchange(x) => x.encode(bytes),
            CertificateRequest(x) => x.encode(bytes),
            CertificateRequestTls13(x) => x.encode(bytes),
            CertificateVerify(x) => x.encode(bytes),
            NewSessionTicket(x) => x.encode(bytes),
            NewSessionTicketTls13(x) => x.encode(bytes),
            EncryptedExtensions(x) => x.encode(bytes),
            KeyUpdate(x) => x.encode(bytes),
            Finished(x) => x.encode(bytes),
            CertificateStatus(x) => x.encode(bytes),
            MessageHash(x) => x.encode(bytes),
            Unknown((_, x)) => x.encode(bytes),
        }
    }

    pub(crate) fn handshake_type(&self) -> HandshakeType {
        use self::HandshakePayload::*;
        match self {
            HelloRequest => HandshakeType::HelloRequest,
            ClientHello(_) => HandshakeType::ClientHello,
            ServerHello(_) => HandshakeType::ServerHello,
            HelloRetryRequest(_) => HandshakeType::HelloRetryRequest,
            Certificate(_) | CertificateTls13(_) => HandshakeType::Certificate,
            CompressedCertificate(_) => HandshakeType::CompressedCertificate,
            ServerKeyExchange(_) => HandshakeType::ServerKeyExchange,
            CertificateRequest(_) | CertificateRequestTls13(_) => HandshakeType::CertificateRequest,
            CertificateVerify(_) => HandshakeType::CertificateVerify,
            ServerHelloDone => HandshakeType::ServerHelloDone,
            EndOfEarlyData => HandshakeType::EndOfEarlyData,
            ClientKeyExchange(_) => HandshakeType::ClientKeyExchange,
            NewSessionTicket(_) | NewSessionTicketTls13(_) => HandshakeType::NewSessionTicket,
            EncryptedExtensions(_) => HandshakeType::EncryptedExtensions,
            KeyUpdate(_) => HandshakeType::KeyUpdate,
            Finished(_) => HandshakeType::Finished,
            CertificateStatus(_) => HandshakeType::CertificateStatus,
            MessageHash(_) => HandshakeType::MessageHash,
            Unknown((t, _)) => *t,
        }
    }

    fn wire_handshake_type(&self) -> HandshakeType {
        match self.handshake_type() {
            // A `HelloRetryRequest` appears on the wire as a `ServerHello` with a magic `random` value.
            HandshakeType::HelloRetryRequest => HandshakeType::ServerHello,
            other => other,
        }
    }

    fn into_owned(self) -> HandshakePayload<'static> {
        use HandshakePayload::*;

        match self {
            HelloRequest => HelloRequest,
            ClientHello(x) => ClientHello(x),
            ServerHello(x) => ServerHello(x),
            HelloRetryRequest(x) => HelloRetryRequest(x),
            Certificate(x) => Certificate(x.into_owned()),
            CertificateTls13(x) => CertificateTls13(x.into_owned()),
            CompressedCertificate(x) => CompressedCertificate(x.into_owned()),
            ServerKeyExchange(x) => ServerKeyExchange(x),
            CertificateRequest(x) => CertificateRequest(x),
            CertificateRequestTls13(x) => CertificateRequestTls13(x),
            CertificateVerify(x) => CertificateVerify(x),
            ServerHelloDone => ServerHelloDone,
            EndOfEarlyData => EndOfEarlyData,
            ClientKeyExchange(x) => ClientKeyExchange(x.into_owned()),
            NewSessionTicket(x) => NewSessionTicket(x),
            NewSessionTicketTls13(x) => NewSessionTicketTls13(x),
            EncryptedExtensions(x) => EncryptedExtensions(Box::new(x.into_owned())),
            KeyUpdate(x) => KeyUpdate(x),
            Finished(x) => Finished(x.into_owned()),
            CertificateStatus(x) => CertificateStatus(x.into_owned()),
            MessageHash(x) => MessageHash(x.into_owned()),
            Unknown((t, x)) => Unknown((t, x.into_owned())),
        }
    }
}

#[derive(Debug)]
pub struct HandshakeMessagePayload<'a>(pub(crate) HandshakePayload<'a>);

impl<'a> Codec<'a> for HandshakeMessagePayload<'a> {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.payload_encode(bytes, Encoding::Standard);
    }

    fn read(r: &mut Reader<'a>) -> Result<Self, InvalidMessage> {
        Self::read_version(r, ProtocolVersion::TLSv1_2)
    }
}

impl<'a> HandshakeMessagePayload<'a> {
    pub(crate) fn read_version(
        r: &mut Reader<'a>,
        vers: ProtocolVersion,
    ) -> Result<Self, InvalidMessage> {
        let typ = HandshakeType::read(r)?;
        let len = codec::u24::read(r)?.0 as usize;
        let mut sub = r.sub(len)?;

        let payload = match typ {
            HandshakeType::HelloRequest if sub.left() == 0 => HandshakePayload::HelloRequest,
            HandshakeType::ClientHello => {
                HandshakePayload::ClientHello(ClientHelloPayload::read(&mut sub)?)
            }
            HandshakeType::ServerHello => {
                let version = ProtocolVersion::read(&mut sub)?;
                let random = Random::read(&mut sub)?;

                if random == HELLO_RETRY_REQUEST_RANDOM {
                    let mut hrr = HelloRetryRequest::read(&mut sub)?;
                    hrr.legacy_version = version;
                    HandshakePayload::HelloRetryRequest(hrr)
                } else {
                    let mut shp = ServerHelloPayload::read(&mut sub)?;
                    shp.legacy_version = version;
                    shp.random = random;
                    HandshakePayload::ServerHello(shp)
                }
            }
            HandshakeType::Certificate if vers == ProtocolVersion::TLSv1_3 => {
                let p = CertificatePayloadTls13::read(&mut sub)?;
                HandshakePayload::CertificateTls13(p)
            }
            HandshakeType::Certificate => {
                HandshakePayload::Certificate(CertificateChain::read(&mut sub)?)
            }
            HandshakeType::ServerKeyExchange => {
                let p = ServerKeyExchangePayload::read(&mut sub)?;
                HandshakePayload::ServerKeyExchange(p)
            }
            HandshakeType::ServerHelloDone => {
                sub.expect_empty("ServerHelloDone")?;
                HandshakePayload::ServerHelloDone
            }
            HandshakeType::ClientKeyExchange => {
                HandshakePayload::ClientKeyExchange(Payload::read(&mut sub))
            }
            HandshakeType::CertificateRequest if vers == ProtocolVersion::TLSv1_3 => {
                let p = CertificateRequestPayloadTls13::read(&mut sub)?;
                HandshakePayload::CertificateRequestTls13(p)
            }
            HandshakeType::CertificateRequest => {
                let p = CertificateRequestPayload::read(&mut sub)?;
                HandshakePayload::CertificateRequest(p)
            }
            HandshakeType::CompressedCertificate => HandshakePayload::CompressedCertificate(
                CompressedCertificatePayload::read(&mut sub)?,
            ),
            HandshakeType::CertificateVerify => {
                HandshakePayload::CertificateVerify(DigitallySignedStruct::read(&mut sub)?)
            }
            HandshakeType::NewSessionTicket if vers == ProtocolVersion::TLSv1_3 => {
                let p = NewSessionTicketPayloadTls13::read(&mut sub)?;
                HandshakePayload::NewSessionTicketTls13(p)
            }
            HandshakeType::NewSessionTicket => {
                let p = NewSessionTicketPayload::read(&mut sub)?;
                HandshakePayload::NewSessionTicket(p)
            }
            HandshakeType::EncryptedExtensions => {
                HandshakePayload::EncryptedExtensions(Box::new(ServerExtensions::read(&mut sub)?))
            }
            HandshakeType::KeyUpdate => {
                HandshakePayload::KeyUpdate(KeyUpdateRequest::read(&mut sub)?)
            }
            HandshakeType::EndOfEarlyData => {
                sub.expect_empty("EndOfEarlyData")?;
                HandshakePayload::EndOfEarlyData
            }
            HandshakeType::Finished => HandshakePayload::Finished(Payload::read(&mut sub)),
            HandshakeType::CertificateStatus => {
                HandshakePayload::CertificateStatus(CertificateStatus::read(&mut sub)?)
            }
            HandshakeType::MessageHash => {
                // does not appear on the wire
                return Err(InvalidMessage::UnexpectedMessage("MessageHash"));
            }
            HandshakeType::HelloRetryRequest => {
                // not legal on wire
                return Err(InvalidMessage::UnexpectedMessage("HelloRetryRequest"));
            }
            _ => HandshakePayload::Unknown((typ, Payload::read(&mut sub))),
        };

        sub.expect_empty("HandshakeMessagePayload")
            .map(|_| Self(payload))
    }

    pub(crate) fn encoding_for_binder_signing(&self) -> Vec<u8> {
        let mut ret = self.get_encoding();
        let ret_len = ret.len() - self.total_binder_length();
        ret.truncate(ret_len);
        ret
    }

    pub(crate) fn total_binder_length(&self) -> usize {
        match &self.0 {
            HandshakePayload::ClientHello(ch) => match &ch.preshared_key_offer {
                Some(offer) => {
                    let mut binders_encoding = Vec::new();
                    offer
                        .binders
                        .encode(&mut binders_encoding);
                    binders_encoding.len()
                }
                _ => 0,
            },
            _ => 0,
        }
    }

    pub(crate) fn payload_encode(&self, bytes: &mut Vec<u8>, encoding: Encoding) {
        // output type, length, and encoded payload
        self.0
            .wire_handshake_type()
            .encode(bytes);

        let nested = LengthPrefixedBuffer::new(
            ListLength::U24 {
                max: usize::MAX,
                error: InvalidMessage::MessageTooLarge,
            },
            bytes,
        );

        match &self.0 {
            // for Server Hello and HelloRetryRequest payloads we need to encode the payload
            // differently based on the purpose of the encoding.
            HandshakePayload::ServerHello(payload) => payload.payload_encode(nested.buf, encoding),
            HandshakePayload::HelloRetryRequest(payload) => {
                payload.payload_encode(nested.buf, encoding)
            }

            // All other payload types are encoded the same regardless of purpose.
            _ => self.0.encode(nested.buf),
        }
    }

    pub(crate) fn build_handshake_hash(hash: &[u8]) -> Self {
        Self(HandshakePayload::MessageHash(Payload::new(hash.to_vec())))
    }

    pub(crate) fn into_owned(self) -> HandshakeMessagePayload<'static> {
        HandshakeMessagePayload(self.0.into_owned())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HpkeSymmetricCipherSuite {
    pub kdf_id: HpkeKdf,
    pub aead_id: HpkeAead,
}

impl Codec<'_> for HpkeSymmetricCipherSuite {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.kdf_id.encode(bytes);
        self.aead_id.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            kdf_id: HpkeKdf::read(r)?,
            aead_id: HpkeAead::read(r)?,
        })
    }
}

/// draft-ietf-tls-esni-24: `HpkeSymmetricCipherSuite cipher_suites<4..2^16-4>;`
impl TlsListElement for HpkeSymmetricCipherSuite {
    const SIZE_LEN: ListLength = ListLength::NonZeroU16 {
        empty_error: InvalidMessage::IllegalEmptyList("HpkeSymmetricCipherSuites"),
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct HpkeKeyConfig {
    pub config_id: u8,
    pub kem_id: HpkeKem,
    /// draft-ietf-tls-esni-24: `opaque HpkePublicKey<1..2^16-1>;`
    pub public_key: PayloadU16<NonEmpty>,
    pub symmetric_cipher_suites: Vec<HpkeSymmetricCipherSuite>,
}

impl Codec<'_> for HpkeKeyConfig {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.config_id.encode(bytes);
        self.kem_id.encode(bytes);
        self.public_key.encode(bytes);
        self.symmetric_cipher_suites
            .encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            config_id: u8::read(r)?,
            kem_id: HpkeKem::read(r)?,
            public_key: PayloadU16::read(r)?,
            symmetric_cipher_suites: Vec::<HpkeSymmetricCipherSuite>::read(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EchConfigContents {
    pub key_config: HpkeKeyConfig,
    pub maximum_name_length: u8,
    pub public_name: DnsName<'static>,
    pub extensions: Vec<EchConfigExtension>,
}

impl EchConfigContents {
    /// Returns true if there is more than one extension of a given
    /// type.
    pub(crate) fn has_duplicate_extension(&self) -> bool {
        has_duplicates::<_, _, u16>(
            self.extensions
                .iter()
                .map(|ext| ext.ext_type()),
        )
    }

    /// Returns true if there is at least one mandatory unsupported extension.
    pub(crate) fn has_unknown_mandatory_extension(&self) -> bool {
        self.extensions
            .iter()
            // An extension is considered mandatory if the high bit of its type is set.
            .any(|ext| {
                matches!(ext.ext_type(), ExtensionType::Unknown(_))
                    && u16::from(ext.ext_type()) & 0x8000 != 0
            })
    }
}

impl Codec<'_> for EchConfigContents {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.key_config.encode(bytes);
        self.maximum_name_length.encode(bytes);
        let dns_name = &self.public_name.borrow();
        PayloadU8::<MaybeEmpty>::encode_slice(dns_name.as_ref().as_ref(), bytes);
        self.extensions.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            key_config: HpkeKeyConfig::read(r)?,
            maximum_name_length: u8::read(r)?,
            public_name: {
                DnsName::try_from(
                    PayloadU8::<MaybeEmpty>::read(r)?
                        .0
                        .as_slice(),
                )
                .map_err(|_| InvalidMessage::InvalidServerName)?
                .to_owned()
            },
            extensions: Vec::read(r)?,
        })
    }
}

/// An encrypted client hello (ECH) config.
#[derive(Clone, Debug, PartialEq)]
pub enum EchConfigPayload {
    /// A recognized V18 ECH configuration.
    V18(EchConfigContents),
    /// An unknown version ECH configuration.
    Unknown {
        version: EchVersion,
        contents: PayloadU16,
    },
}

impl TlsListElement for EchConfigPayload {
    const SIZE_LEN: ListLength = ListLength::U16;
}

impl Codec<'_> for EchConfigPayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::V18(c) => {
                // Write the version, the length, and the contents.
                EchVersion::V18.encode(bytes);
                let inner = LengthPrefixedBuffer::new(ListLength::U16, bytes);
                c.encode(inner.buf);
            }
            Self::Unknown { version, contents } => {
                // Unknown configuration versions are opaque.
                version.encode(bytes);
                contents.encode(bytes);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let version = EchVersion::read(r)?;
        let length = u16::read(r)?;
        let mut contents = r.sub(length as usize)?;

        Ok(match version {
            EchVersion::V18 => Self::V18(EchConfigContents::read(&mut contents)?),
            _ => {
                // Note: we don't PayloadU16::read() here because we've already read the length prefix.
                let data = PayloadU16::new(contents.rest().into());
                Self::Unknown {
                    version,
                    contents: data,
                }
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EchConfigExtension {
    Unknown(UnknownExtension),
}

impl EchConfigExtension {
    pub(crate) fn ext_type(&self) -> ExtensionType {
        match self {
            Self::Unknown(r) => r.typ,
        }
    }
}

impl Codec<'_> for EchConfigExtension {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.ext_type().encode(bytes);

        let nested = LengthPrefixedBuffer::new(ListLength::U16, bytes);
        match self {
            Self::Unknown(r) => r.encode(nested.buf),
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let typ = ExtensionType::read(r)?;
        let len = u16::read(r)? as usize;
        let mut sub = r.sub(len)?;

        #[allow(clippy::match_single_binding)] // Future-proofing.
        let ext = match typ {
            _ => Self::Unknown(UnknownExtension::read(typ, &mut sub)),
        };

        sub.expect_empty("EchConfigExtension")
            .map(|_| ext)
    }
}

impl TlsListElement for EchConfigExtension {
    const SIZE_LEN: ListLength = ListLength::U16;
}

/// Representation of the `ECHClientHello` client extension specified in
/// [draft-ietf-tls-esni Section 5].
///
/// [draft-ietf-tls-esni Section 5]: <https://www.ietf.org/archive/id/draft-ietf-tls-esni-18.html#section-5>
#[derive(Clone, Debug)]
pub(crate) enum EncryptedClientHello {
    /// A `ECHClientHello` with type [EchClientHelloType::ClientHelloOuter].
    Outer(EncryptedClientHelloOuter),
    /// An empty `ECHClientHello` with type [EchClientHelloType::ClientHelloInner].
    ///
    /// This variant has no payload.
    Inner,
}

impl Codec<'_> for EncryptedClientHello {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Outer(payload) => {
                EchClientHelloType::ClientHelloOuter.encode(bytes);
                payload.encode(bytes);
            }
            Self::Inner => {
                EchClientHelloType::ClientHelloInner.encode(bytes);
                // Empty payload.
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        match EchClientHelloType::read(r)? {
            EchClientHelloType::ClientHelloOuter => {
                Ok(Self::Outer(EncryptedClientHelloOuter::read(r)?))
            }
            EchClientHelloType::ClientHelloInner => Ok(Self::Inner),
            _ => Err(InvalidMessage::InvalidContentType),
        }
    }
}

/// Representation of the ECHClientHello extension with type outer specified in
/// [draft-ietf-tls-esni Section 5].
///
/// [draft-ietf-tls-esni Section 5]: <https://www.ietf.org/archive/id/draft-ietf-tls-esni-18.html#section-5>
#[derive(Clone, Debug)]
pub(crate) struct EncryptedClientHelloOuter {
    /// The cipher suite used to encrypt ClientHelloInner. Must match a value from
    /// ECHConfigContents.cipher_suites list.
    pub cipher_suite: HpkeSymmetricCipherSuite,
    /// The ECHConfigContents.key_config.config_id for the chosen ECHConfig.
    pub config_id: u8,
    /// The HPKE encapsulated key, used by servers to decrypt the corresponding payload field.
    /// This field is empty in a ClientHelloOuter sent in response to a HelloRetryRequest.
    pub enc: PayloadU16,
    /// The serialized and encrypted ClientHelloInner structure, encrypted using HPKE.
    pub payload: PayloadU16<NonEmpty>,
}

impl Codec<'_> for EncryptedClientHelloOuter {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.cipher_suite.encode(bytes);
        self.config_id.encode(bytes);
        self.enc.encode(bytes);
        self.payload.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            cipher_suite: HpkeSymmetricCipherSuite::read(r)?,
            config_id: u8::read(r)?,
            enc: PayloadU16::read(r)?,
            payload: PayloadU16::read(r)?,
        })
    }
}

/// Representation of the ECHEncryptedExtensions extension specified in
/// [draft-ietf-tls-esni Section 5].
///
/// [draft-ietf-tls-esni Section 5]: <https://www.ietf.org/archive/id/draft-ietf-tls-esni-18.html#section-5>
#[derive(Clone, Debug)]
pub(crate) struct ServerEncryptedClientHello {
    pub(crate) retry_configs: Vec<EchConfigPayload>,
}

impl Codec<'_> for ServerEncryptedClientHello {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.retry_configs.encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        Ok(Self {
            retry_configs: Vec::<EchConfigPayload>::read(r)?,
        })
    }
}

/// The method of encoding to use for a handshake message.
///
/// In some cases a handshake message may be encoded differently depending on the purpose
/// the encoded message is being used for.
pub(crate) enum Encoding {
    /// Standard RFC 8446 encoding.
    Standard,
    /// Encoding for ECH confirmation for HRR.
    EchConfirmation,
    /// Encoding for ECH inner client hello.
    EchInnerHello { to_compress: Vec<ExtensionType> },
}

fn has_duplicates<I: IntoIterator<Item = E>, E: Into<T>, T: Eq + Ord>(iter: I) -> bool {
    let mut seen = BTreeSet::new();

    for x in iter {
        if !seen.insert(x.into()) {
            return true;
        }
    }

    false
}

struct DuplicateExtensionChecker(BTreeSet<u16>);

impl DuplicateExtensionChecker {
    fn new() -> Self {
        Self(BTreeSet::new())
    }

    fn check(&mut self, typ: ExtensionType) -> Result<(), InvalidMessage> {
        let u = u16::from(typ);
        match self.0.insert(u) {
            true => Ok(()),
            false => Err(InvalidMessage::DuplicateExtension(u)),
        }
    }
}

fn low_quality_integer_hash(mut x: u32) -> u32 {
    x = x
        .wrapping_add(0x7ed55d16)
        .wrapping_add(x << 12);
    x = (x ^ 0xc761c23c) ^ (x >> 19);
    x = x
        .wrapping_add(0x165667b1)
        .wrapping_add(x << 5);
    x = x.wrapping_add(0xd3a2646c) ^ (x << 9);
    x = x
        .wrapping_add(0xfd7046c5)
        .wrapping_add(x << 3);
    x = (x ^ 0xb55a4f09) ^ (x >> 16);
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ech_config_dupe_exts() {
        let unknown_ext = EchConfigExtension::Unknown(UnknownExtension {
            typ: ExtensionType::Unknown(0x42),
            payload: Payload::new(vec![0x42]),
        });
        let mut config = config_template();
        config
            .extensions
            .push(unknown_ext.clone());
        config.extensions.push(unknown_ext);

        assert!(config.has_duplicate_extension());
        assert!(!config.has_unknown_mandatory_extension());
    }

    #[test]
    fn test_ech_config_mandatory_exts() {
        let mandatory_unknown_ext = EchConfigExtension::Unknown(UnknownExtension {
            typ: ExtensionType::Unknown(0x42 | 0x8000), // Note: high bit set.
            payload: Payload::new(vec![0x42]),
        });
        let mut config = config_template();
        config
            .extensions
            .push(mandatory_unknown_ext);

        assert!(!config.has_duplicate_extension());
        assert!(config.has_unknown_mandatory_extension());
    }

    fn config_template() -> EchConfigContents {
        EchConfigContents {
            key_config: HpkeKeyConfig {
                config_id: 0,
                kem_id: HpkeKem::DHKEM_P256_HKDF_SHA256,
                public_key: PayloadU16::new(b"xxx".into()),
                symmetric_cipher_suites: vec![HpkeSymmetricCipherSuite {
                    kdf_id: HpkeKdf::HKDF_SHA256,
                    aead_id: HpkeAead::AES_128_GCM,
                }],
            },
            maximum_name_length: 0,
            public_name: DnsName::try_from("example.com").unwrap(),
            extensions: vec![],
        }
    }
}
