use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::fmt::Debug;

/// This module contains optional APIs for implementing QUIC TLS.
use crate::common_state::Side;
use crate::crypto::cipher::{AeadKey, Iv};
use crate::crypto::tls13::{Hkdf, HkdfExpander, OkmBlock};
use crate::enums::AlertDescription;
use crate::error::Error;
use crate::tls13::Tls13CipherSuite;
use crate::tls13::key_schedule::{
    hkdf_expand_label, hkdf_expand_label_aead_key, hkdf_expand_label_block,
};

#[cfg(feature = "std")]
mod connection {
    use alloc::vec::Vec;
    use core::fmt::{self, Debug};
    use core::ops::{Deref, DerefMut};

    use pki_types::ServerName;

    use super::{DirectionalKeys, KeyChange, Version};
    use crate::client::{ClientConfig, ClientConnectionData};
    use crate::common_state::{CommonState, DEFAULT_BUFFER_LIMIT, Protocol};
    use crate::conn::{ConnectionCore, SideData};
    use crate::enums::{AlertDescription, ContentType, ProtocolVersion};
    use crate::error::Error;
    use crate::msgs::base::Payload;
    use crate::msgs::deframer::buffers::{DeframerVecBuffer, Locator};
    use crate::msgs::handshake::{
        ClientExtensionsInput, ServerExtensionsInput, TransportParameters,
    };
    use crate::msgs::message::InboundPlainMessage;
    use crate::server::{ServerConfig, ServerConnectionData};
    use crate::sync::Arc;
    use crate::vecbuf::ChunkVecBuffer;

    /// A QUIC client or server connection.
    #[derive(Debug)]
    pub enum Connection {
        /// A client connection
        Client(ClientConnection),
        /// A server connection
        Server(ServerConnection),
    }

    impl Connection {
        /// Return the TLS-encoded transport parameters for the session's peer.
        ///
        /// See [`ConnectionCommon::quic_transport_parameters()`] for more details.
        pub fn quic_transport_parameters(&self) -> Option<&[u8]> {
            match self {
                Self::Client(conn) => conn.quic_transport_parameters(),
                Self::Server(conn) => conn.quic_transport_parameters(),
            }
        }

        /// Compute the keys for encrypting/decrypting 0-RTT packets, if available
        pub fn zero_rtt_keys(&self) -> Option<DirectionalKeys> {
            match self {
                Self::Client(conn) => conn.zero_rtt_keys(),
                Self::Server(conn) => conn.zero_rtt_keys(),
            }
        }

        /// Consume unencrypted TLS handshake data.
        ///
        /// Handshake data obtained from separate encryption levels should be supplied in separate calls.
        pub fn read_hs(&mut self, plaintext: &[u8]) -> Result<(), Error> {
            match self {
                Self::Client(conn) => conn.read_hs(plaintext),
                Self::Server(conn) => conn.read_hs(plaintext),
            }
        }

        /// Emit unencrypted TLS handshake data.
        ///
        /// When this returns `Some(_)`, the new keys must be used for future handshake data.
        pub fn write_hs(&mut self, buf: &mut Vec<u8>) -> Option<KeyChange> {
            match self {
                Self::Client(conn) => conn.write_hs(buf),
                Self::Server(conn) => conn.write_hs(buf),
            }
        }

        /// Emit the TLS description code of a fatal alert, if one has arisen.
        ///
        /// Check after `read_hs` returns `Err(_)`.
        pub fn alert(&self) -> Option<AlertDescription> {
            match self {
                Self::Client(conn) => conn.alert(),
                Self::Server(conn) => conn.alert(),
            }
        }

        /// Derives key material from the agreed connection secrets.
        ///
        /// This function fills in `output` with `output.len()` bytes of key
        /// material derived from the master session secret using `label`
        /// and `context` for diversification. Ownership of the buffer is taken
        /// by the function and returned via the Ok result to ensure no key
        /// material leaks if the function fails.
        ///
        /// See RFC5705 for more details on what this does and is for.
        ///
        /// For TLS1.3 connections, this function does not use the
        /// "early" exporter at any point.
        ///
        /// This function fails if called prior to the handshake completing;
        /// check with [`CommonState::is_handshaking`] first.
        #[inline]
        pub fn export_keying_material<T: AsMut<[u8]>>(
            &self,
            output: T,
            label: &[u8],
            context: Option<&[u8]>,
        ) -> Result<T, Error> {
            match self {
                Self::Client(conn) => conn
                    .core
                    .export_keying_material(output, label, context),
                Self::Server(conn) => conn
                    .core
                    .export_keying_material(output, label, context),
            }
        }
    }

    impl Deref for Connection {
        type Target = CommonState;

        fn deref(&self) -> &Self::Target {
            match self {
                Self::Client(conn) => &conn.core.common_state,
                Self::Server(conn) => &conn.core.common_state,
            }
        }
    }

    impl DerefMut for Connection {
        fn deref_mut(&mut self) -> &mut Self::Target {
            match self {
                Self::Client(conn) => &mut conn.core.common_state,
                Self::Server(conn) => &mut conn.core.common_state,
            }
        }
    }

    /// A QUIC client connection.
    pub struct ClientConnection {
        inner: ConnectionCommon<ClientConnectionData>,
    }

    impl ClientConnection {
        /// Make a new QUIC ClientConnection.
        ///
        /// This differs from `ClientConnection::new()` in that it takes an extra `params` argument,
        /// which contains the TLS-encoded transport parameters to send.
        pub fn new(
            config: Arc<ClientConfig>,
            quic_version: Version,
            name: ServerName<'static>,
            params: Vec<u8>,
        ) -> Result<Self, Error> {
            Self::new_with_alpn(
                config.clone(),
                quic_version,
                name,
                params,
                config.alpn_protocols.clone(),
            )
        }

        /// Make a new QUIC ClientConnection with custom ALPN protocols.
        pub fn new_with_alpn(
            config: Arc<ClientConfig>,
            quic_version: Version,
            name: ServerName<'static>,
            params: Vec<u8>,
            alpn_protocols: Vec<Vec<u8>>,
        ) -> Result<Self, Error> {
            if !config.supports_version(ProtocolVersion::TLSv1_3) {
                return Err(Error::General(
                    "TLS 1.3 support is required for QUIC".into(),
                ));
            }

            if !config.supports_protocol(Protocol::Quic) {
                return Err(Error::General(
                    "at least one ciphersuite must support QUIC".into(),
                ));
            }

            let exts = ClientExtensionsInput {
                transport_parameters: Some(match quic_version {
                    Version::V1Draft => TransportParameters::QuicDraft(Payload::new(params)),
                    Version::V1 | Version::V2 => TransportParameters::Quic(Payload::new(params)),
                }),

                ..ClientExtensionsInput::from_alpn(alpn_protocols)
            };

            let mut inner = ConnectionCore::for_client(config, name, exts, Protocol::Quic)?;
            inner.common_state.quic.version = quic_version;
            Ok(Self {
                inner: inner.into(),
            })
        }

        /// Returns True if the server signalled it will process early data.
        ///
        /// If you sent early data and this returns false at the end of the
        /// handshake then the server will not process the data.  This
        /// is not an error, but you may wish to resend the data.
        pub fn is_early_data_accepted(&self) -> bool {
            self.inner.core.is_early_data_accepted()
        }

        /// Returns the number of TLS1.3 tickets that have been received.
        pub fn tls13_tickets_received(&self) -> u32 {
            self.inner.tls13_tickets_received
        }
    }

    impl Deref for ClientConnection {
        type Target = ConnectionCommon<ClientConnectionData>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl DerefMut for ClientConnection {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    impl Debug for ClientConnection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("quic::ClientConnection")
                .finish()
        }
    }

    impl From<ClientConnection> for Connection {
        fn from(c: ClientConnection) -> Self {
            Self::Client(c)
        }
    }

    /// A QUIC server connection.
    pub struct ServerConnection {
        inner: ConnectionCommon<ServerConnectionData>,
    }

    impl ServerConnection {
        /// Make a new QUIC ServerConnection.
        ///
        /// This differs from `ServerConnection::new()` in that it takes an extra `params` argument,
        /// which contains the TLS-encoded transport parameters to send.
        pub fn new(
            config: Arc<ServerConfig>,
            quic_version: Version,
            params: Vec<u8>,
        ) -> Result<Self, Error> {
            if !config.supports_version(ProtocolVersion::TLSv1_3) {
                return Err(Error::General(
                    "TLS 1.3 support is required for QUIC".into(),
                ));
            }

            if !config.supports_protocol(Protocol::Quic) {
                return Err(Error::General(
                    "at least one ciphersuite must support QUIC".into(),
                ));
            }

            if config.max_early_data_size != 0 && config.max_early_data_size != 0xffff_ffff {
                return Err(Error::General(
                    "QUIC sessions must set a max early data of 0 or 2^32-1".into(),
                ));
            }

            let exts = ServerExtensionsInput {
                transport_parameters: Some(match quic_version {
                    Version::V1Draft => TransportParameters::QuicDraft(Payload::new(params)),
                    Version::V1 | Version::V2 => TransportParameters::Quic(Payload::new(params)),
                }),
            };

            let mut core = ConnectionCore::for_server(config, exts)?;
            core.common_state.protocol = Protocol::Quic;
            core.common_state.quic.version = quic_version;
            Ok(Self { inner: core.into() })
        }

        /// Explicitly discard early data, notifying the client
        ///
        /// Useful if invariants encoded in `received_resumption_data()` cannot be respected.
        ///
        /// Must be called while `is_handshaking` is true.
        pub fn reject_early_data(&mut self) {
            self.inner.core.reject_early_data()
        }

        /// Retrieves the server name, if any, used to select the certificate and
        /// private key.
        ///
        /// This returns `None` until some time after the client's server name indication
        /// (SNI) extension value is processed during the handshake. It will never be
        /// `None` when the connection is ready to send or process application data,
        /// unless the client does not support SNI.
        ///
        /// This is useful for application protocols that need to enforce that the
        /// server name matches an application layer protocol hostname. For
        /// example, HTTP/1.1 servers commonly expect the `Host:` header field of
        /// every request on a connection to match the hostname in the SNI extension
        /// when the client provides the SNI extension.
        ///
        /// The server name is also used to match sessions during session resumption.
        pub fn server_name(&self) -> Option<&str> {
            self.inner.core.get_sni_str()
        }
    }

    impl Deref for ServerConnection {
        type Target = ConnectionCommon<ServerConnectionData>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl DerefMut for ServerConnection {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    impl Debug for ServerConnection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("quic::ServerConnection")
                .finish()
        }
    }

    impl From<ServerConnection> for Connection {
        fn from(c: ServerConnection) -> Self {
            Self::Server(c)
        }
    }

    /// A shared interface for QUIC connections.
    pub struct ConnectionCommon<Data> {
        core: ConnectionCore<Data>,
        deframer_buffer: DeframerVecBuffer,
        sendable_plaintext: ChunkVecBuffer,
    }

    impl<Data: SideData> ConnectionCommon<Data> {
        /// Return the TLS-encoded transport parameters for the session's peer.
        ///
        /// While the transport parameters are technically available prior to the
        /// completion of the handshake, they cannot be fully trusted until the
        /// handshake completes, and reliance on them should be minimized.
        /// However, any tampering with the parameters will cause the handshake
        /// to fail.
        pub fn quic_transport_parameters(&self) -> Option<&[u8]> {
            self.core
                .common_state
                .quic
                .params
                .as_ref()
                .map(|v| v.as_ref())
        }

        /// Compute the keys for encrypting/decrypting 0-RTT packets, if available
        pub fn zero_rtt_keys(&self) -> Option<DirectionalKeys> {
            let suite = self
                .core
                .common_state
                .suite
                .and_then(|suite| suite.tls13())?;
            Some(DirectionalKeys::new(
                suite,
                suite.quic?,
                self.core
                    .common_state
                    .quic
                    .early_secret
                    .as_ref()?,
                self.core.common_state.quic.version,
            ))
        }

        /// Consume unencrypted TLS handshake data.
        ///
        /// Handshake data obtained from separate encryption levels should be supplied in separate calls.
        pub fn read_hs(&mut self, plaintext: &[u8]) -> Result<(), Error> {
            let range = self.deframer_buffer.extend(plaintext);

            self.core.hs_deframer.input_message(
                InboundPlainMessage {
                    typ: ContentType::Handshake,
                    version: ProtocolVersion::TLSv1_3,
                    payload: &self.deframer_buffer.filled()[range.clone()],
                },
                &Locator::new(self.deframer_buffer.filled()),
                range.end,
            );

            self.core
                .hs_deframer
                .coalesce(self.deframer_buffer.filled_mut())?;

            self.core
                .process_new_packets(&mut self.deframer_buffer, &mut self.sendable_plaintext)?;

            Ok(())
        }

        /// Emit unencrypted TLS handshake data.
        ///
        /// When this returns `Some(_)`, the new keys must be used for future handshake data.
        pub fn write_hs(&mut self, buf: &mut Vec<u8>) -> Option<KeyChange> {
            self.core
                .common_state
                .quic
                .write_hs(buf)
        }

        /// Emit the TLS description code of a fatal alert, if one has arisen.
        ///
        /// Check after `read_hs` returns `Err(_)`.
        pub fn alert(&self) -> Option<AlertDescription> {
            self.core.common_state.quic.alert
        }
    }

    impl<Data> Deref for ConnectionCommon<Data> {
        type Target = CommonState;

        fn deref(&self) -> &Self::Target {
            &self.core.common_state
        }
    }

    impl<Data> DerefMut for ConnectionCommon<Data> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.core.common_state
        }
    }

    impl<Data> From<ConnectionCore<Data>> for ConnectionCommon<Data> {
        fn from(core: ConnectionCore<Data>) -> Self {
            Self {
                core,
                deframer_buffer: DeframerVecBuffer::default(),
                sendable_plaintext: ChunkVecBuffer::new(Some(DEFAULT_BUFFER_LIMIT)),
            }
        }
    }
}

#[cfg(feature = "std")]
pub use connection::{ClientConnection, Connection, ConnectionCommon, ServerConnection};

#[derive(Default)]
pub(crate) struct Quic {
    /// QUIC transport parameters received from the peer during the handshake
    pub(crate) params: Option<Vec<u8>>,
    pub(crate) alert: Option<AlertDescription>,
    pub(crate) hs_queue: VecDeque<(bool, Vec<u8>)>,
    pub(crate) early_secret: Option<OkmBlock>,
    pub(crate) hs_secrets: Option<Secrets>,
    pub(crate) traffic_secrets: Option<Secrets>,
    /// Whether keys derived from traffic_secrets have been passed to the QUIC implementation
    #[cfg(feature = "std")]
    pub(crate) returned_traffic_keys: bool,
    pub(crate) version: Version,
}

#[cfg(feature = "std")]
impl Quic {
    pub(crate) fn write_hs(&mut self, buf: &mut Vec<u8>) -> Option<KeyChange> {
        while let Some((_, msg)) = self.hs_queue.pop_front() {
            buf.extend_from_slice(&msg);
            if let Some(&(true, _)) = self.hs_queue.front() {
                if self.hs_secrets.is_some() {
                    // Allow the caller to switch keys before proceeding.
                    break;
                }
            }
        }

        if let Some(secrets) = self.hs_secrets.take() {
            return Some(KeyChange::Handshake {
                keys: Keys::new(&secrets),
            });
        }

        if let Some(mut secrets) = self.traffic_secrets.take() {
            if !self.returned_traffic_keys {
                self.returned_traffic_keys = true;
                let keys = Keys::new(&secrets);
                secrets.update();
                return Some(KeyChange::OneRtt {
                    keys,
                    next: secrets,
                });
            }
        }

        None
    }
}

/// Secrets used to encrypt/decrypt traffic
#[derive(Clone)]
pub struct Secrets {
    /// Secret used to encrypt packets transmitted by the client
    pub(crate) client: OkmBlock,
    /// Secret used to encrypt packets transmitted by the server
    pub(crate) server: OkmBlock,
    /// Cipher suite used with these secrets
    suite: &'static Tls13CipherSuite,
    quic: &'static dyn Algorithm,
    side: Side,
    version: Version,
}

impl Secrets {
    pub(crate) fn new(
        client: OkmBlock,
        server: OkmBlock,
        suite: &'static Tls13CipherSuite,
        quic: &'static dyn Algorithm,
        side: Side,
        version: Version,
    ) -> Self {
        Self {
            client,
            server,
            suite,
            quic,
            side,
            version,
        }
    }

    /// Derive the next set of packet keys
    pub fn next_packet_keys(&mut self) -> PacketKeySet {
        let keys = PacketKeySet::new(self);
        self.update();
        keys
    }

    pub(crate) fn update(&mut self) {
        self.client = hkdf_expand_label_block(
            self.suite
                .hkdf_provider
                .expander_for_okm(&self.client)
                .as_ref(),
            self.version.key_update_label(),
            &[],
        );
        self.server = hkdf_expand_label_block(
            self.suite
                .hkdf_provider
                .expander_for_okm(&self.server)
                .as_ref(),
            self.version.key_update_label(),
            &[],
        );
    }

    fn local_remote(&self) -> (&OkmBlock, &OkmBlock) {
        match self.side {
            Side::Client => (&self.client, &self.server),
            Side::Server => (&self.server, &self.client),
        }
    }
}

/// Keys used to communicate in a single direction
pub struct DirectionalKeys {
    /// Encrypts or decrypts a packet's headers
    pub header: Box<dyn HeaderProtectionKey>,
    /// Encrypts or decrypts the payload of a packet
    pub packet: Box<dyn PacketKey>,
}

impl DirectionalKeys {
    pub(crate) fn new(
        suite: &'static Tls13CipherSuite,
        quic: &'static dyn Algorithm,
        secret: &OkmBlock,
        version: Version,
    ) -> Self {
        let builder = KeyBuilder::new(secret, version, quic, suite.hkdf_provider);
        Self {
            header: builder.header_protection_key(),
            packet: builder.packet_key(),
        }
    }
}

/// All AEADs we support have 16-byte tags.
const TAG_LEN: usize = 16;

/// Authentication tag from an AEAD seal operation.
pub struct Tag([u8; TAG_LEN]);

impl From<&[u8]> for Tag {
    fn from(value: &[u8]) -> Self {
        let mut array = [0u8; TAG_LEN];
        array.copy_from_slice(value);
        Self(array)
    }
}

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// How a `Tls13CipherSuite` generates `PacketKey`s and `HeaderProtectionKey`s.
pub trait Algorithm: Send + Sync {
    /// Produce a `PacketKey` encrypter/decrypter for this suite.
    ///
    /// `suite` is the entire suite this `Algorithm` appeared in.
    /// `key` and `iv` is the key material to use.
    fn packet_key(&self, key: AeadKey, iv: Iv) -> Box<dyn PacketKey>;

    /// Produce a `HeaderProtectionKey` encrypter/decrypter for this suite.
    ///
    /// `key` is the key material, which is `aead_key_len()` bytes in length.
    fn header_protection_key(&self, key: AeadKey) -> Box<dyn HeaderProtectionKey>;

    /// The length in bytes of keys for this Algorithm.
    ///
    /// This controls the size of `AeadKey`s presented to `packet_key()` and `header_protection_key()`.
    fn aead_key_len(&self) -> usize;

    /// Whether this algorithm is FIPS-approved.
    fn fips(&self) -> bool {
        false
    }
}

/// A QUIC header protection key
pub trait HeaderProtectionKey: Send + Sync {
    /// Adds QUIC Header Protection.
    ///
    /// `sample` must contain the sample of encrypted payload; see
    /// [Header Protection Sample].
    ///
    /// `first` must reference the first byte of the header, referred to as
    /// `packet[0]` in [Header Protection Application].
    ///
    /// `packet_number` must reference the Packet Number field; this is
    /// `packet[pn_offset:pn_offset+pn_length]` in [Header Protection Application].
    ///
    /// Returns an error without modifying anything if `sample` is not
    /// the correct length (see [Header Protection Sample] and [`Self::sample_len()`]),
    /// or `packet_number` is longer than allowed (see [Packet Number Encoding and Decoding]).
    ///
    /// Otherwise, `first` and `packet_number` will have the header protection added.
    ///
    /// [Header Protection Application]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.1
    /// [Header Protection Sample]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.2
    /// [Packet Number Encoding and Decoding]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.1
    fn encrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error>;

    /// Removes QUIC Header Protection.
    ///
    /// `sample` must contain the sample of encrypted payload; see
    /// [Header Protection Sample].
    ///
    /// `first` must reference the first byte of the header, referred to as
    /// `packet[0]` in [Header Protection Application].
    ///
    /// `packet_number` must reference the Packet Number field; this is
    /// `packet[pn_offset:pn_offset+pn_length]` in [Header Protection Application].
    ///
    /// Returns an error without modifying anything if `sample` is not
    /// the correct length (see [Header Protection Sample] and [`Self::sample_len()`]),
    /// or `packet_number` is longer than allowed (see
    /// [Packet Number Encoding and Decoding]).
    ///
    /// Otherwise, `first` and `packet_number` will have the header protection removed.
    ///
    /// [Header Protection Application]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.1
    /// [Header Protection Sample]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.2
    /// [Packet Number Encoding and Decoding]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.1
    fn decrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error>;

    /// Expected sample length for the key's algorithm
    fn sample_len(&self) -> usize;
}

/// Keys to encrypt or decrypt the payload of a packet
pub trait PacketKey: Send + Sync {
    /// Encrypt a QUIC packet
    ///
    /// Takes a `packet_number`, used to derive the nonce; the packet `header`, which is used as
    /// the additional authenticated data; and the `payload`. The authentication tag is returned if
    /// encryption succeeds.
    ///
    /// Fails if and only if the payload is longer than allowed by the cipher suite's AEAD algorithm.
    fn encrypt_in_place(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &mut [u8],
    ) -> Result<Tag, Error>;

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
    ) -> Result<&'a [u8], Error>;

    /// Tag length for the underlying AEAD algorithm
    fn tag_len(&self) -> usize;

    /// Number of QUIC messages that can be safely encrypted with a single key of this type.
    ///
    /// Once a `MessageEncrypter` produced for this suite has encrypted more than
    /// `confidentiality_limit` messages, an attacker gains an advantage in distinguishing it
    /// from an ideal pseudorandom permutation (PRP).
    ///
    /// This is to be set on the assumption that messages are maximally sized --
    /// 2 ** 16. For non-QUIC TCP connections see [`CipherSuiteCommon::confidentiality_limit`][csc-limit].
    ///
    /// [csc-limit]: crate::crypto::CipherSuiteCommon::confidentiality_limit
    fn confidentiality_limit(&self) -> u64;

    /// Number of QUIC messages that can be safely decrypted with a single key of this type
    ///
    /// Once a `MessageDecrypter` produced for this suite has failed to decrypt `integrity_limit`
    /// messages, an attacker gains an advantage in forging messages.
    ///
    /// This is not relevant for TLS over TCP (which is also implemented in this crate)
    /// because a single failed decryption is fatal to the connection.
    /// However, this quantity is used by QUIC.
    fn integrity_limit(&self) -> u64;
}

/// Packet protection keys for bidirectional 1-RTT communication
pub struct PacketKeySet {
    /// Encrypts outgoing packets
    pub local: Box<dyn PacketKey>,
    /// Decrypts incoming packets
    pub remote: Box<dyn PacketKey>,
}

impl PacketKeySet {
    fn new(secrets: &Secrets) -> Self {
        let (local, remote) = secrets.local_remote();
        let (version, alg, hkdf) = (secrets.version, secrets.quic, secrets.suite.hkdf_provider);
        Self {
            local: KeyBuilder::new(local, version, alg, hkdf).packet_key(),
            remote: KeyBuilder::new(remote, version, alg, hkdf).packet_key(),
        }
    }
}

pub(crate) struct KeyBuilder<'a> {
    expander: Box<dyn HkdfExpander>,
    version: Version,
    alg: &'a dyn Algorithm,
}

impl<'a> KeyBuilder<'a> {
    pub(crate) fn new(
        secret: &OkmBlock,
        version: Version,
        alg: &'a dyn Algorithm,
        hkdf: &'a dyn Hkdf,
    ) -> Self {
        Self {
            expander: hkdf.expander_for_okm(secret),
            version,
            alg,
        }
    }

    /// Derive packet keys
    pub(crate) fn packet_key(&self) -> Box<dyn PacketKey> {
        let aead_key_len = self.alg.aead_key_len();
        let packet_key = hkdf_expand_label_aead_key(
            self.expander.as_ref(),
            aead_key_len,
            self.version.packet_key_label(),
            &[],
        );

        let packet_iv =
            hkdf_expand_label(self.expander.as_ref(), self.version.packet_iv_label(), &[]);
        self.alg
            .packet_key(packet_key, packet_iv)
    }

    /// Derive header protection keys
    pub(crate) fn header_protection_key(&self) -> Box<dyn HeaderProtectionKey> {
        let header_key = hkdf_expand_label_aead_key(
            self.expander.as_ref(),
            self.alg.aead_key_len(),
            self.version.header_key_label(),
            &[],
        );
        self.alg
            .header_protection_key(header_key)
    }
}

/// Produces QUIC initial keys from a TLS 1.3 ciphersuite and a QUIC key generation algorithm.
#[derive(Clone, Copy)]
pub struct Suite {
    /// The TLS 1.3 ciphersuite used to derive keys.
    pub suite: &'static Tls13CipherSuite,
    /// The QUIC key generation algorithm used to derive keys.
    pub quic: &'static dyn Algorithm,
}

impl Suite {
    /// Produce a set of initial keys given the connection ID, side and version
    pub fn keys(&self, client_dst_connection_id: &[u8], side: Side, version: Version) -> Keys {
        Keys::initial(
            version,
            self.suite,
            self.quic,
            client_dst_connection_id,
            side,
        )
    }
}

/// Complete set of keys used to communicate with the peer
pub struct Keys {
    /// Encrypts outgoing packets
    pub local: DirectionalKeys,
    /// Decrypts incoming packets
    pub remote: DirectionalKeys,
}

impl Keys {
    /// Construct keys for use with initial packets
    pub fn initial(
        version: Version,
        suite: &'static Tls13CipherSuite,
        quic: &'static dyn Algorithm,
        client_dst_connection_id: &[u8],
        side: Side,
    ) -> Self {
        const CLIENT_LABEL: &[u8] = b"client in";
        const SERVER_LABEL: &[u8] = b"server in";
        let salt = version.initial_salt();
        let hs_secret = suite
            .hkdf_provider
            .extract_from_secret(Some(salt), client_dst_connection_id);

        let secrets = Secrets {
            version,
            client: hkdf_expand_label_block(hs_secret.as_ref(), CLIENT_LABEL, &[]),
            server: hkdf_expand_label_block(hs_secret.as_ref(), SERVER_LABEL, &[]),
            suite,
            quic,
            side,
        };
        Self::new(&secrets)
    }

    fn new(secrets: &Secrets) -> Self {
        let (local, remote) = secrets.local_remote();
        Self {
            local: DirectionalKeys::new(secrets.suite, secrets.quic, local, secrets.version),
            remote: DirectionalKeys::new(secrets.suite, secrets.quic, remote, secrets.version),
        }
    }
}

/// Key material for use in QUIC packet spaces
///
/// QUIC uses 4 different sets of keys (and progressive key updates for long-running connections):
///
/// * Initial: these can be created from [`Keys::initial()`]
/// * 0-RTT keys: can be retrieved from [`ConnectionCommon::zero_rtt_keys()`]
/// * Handshake: these are returned from [`ConnectionCommon::write_hs()`] after `ClientHello` and
///   `ServerHello` messages have been exchanged
/// * 1-RTT keys: these are returned from [`ConnectionCommon::write_hs()`] after the handshake is done
///
/// Once the 1-RTT keys have been exchanged, either side may initiate a key update. Progressive
/// update keys can be obtained from the [`Secrets`] returned in [`KeyChange::OneRtt`]. Note that
/// only packet keys are updated by key updates; header protection keys remain the same.
pub enum KeyChange {
    /// Keys for the handshake space
    Handshake {
        /// Header and packet keys for the handshake space
        keys: Keys,
    },
    /// Keys for 1-RTT data
    OneRtt {
        /// Header and packet keys for 1-RTT data
        keys: Keys,
        /// Secrets to derive updated keys from
        next: Secrets,
    },
}

/// QUIC protocol version
///
/// Governs version-specific behavior in the TLS layer
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum Version {
    /// Draft versions 29, 30, 31 and 32
    V1Draft,
    /// First stable RFC
    V1,
    /// Anti-ossification variant of V1
    V2,
}

impl Version {
    fn initial_salt(self) -> &'static [u8; 20] {
        match self {
            Self::V1Draft => &[
                // https://datatracker.ietf.org/doc/html/draft-ietf-quic-tls-32#section-5.2
                0xaf, 0xbf, 0xec, 0x28, 0x99, 0x93, 0xd2, 0x4c, 0x9e, 0x97, 0x86, 0xf1, 0x9c, 0x61,
                0x11, 0xe0, 0x43, 0x90, 0xa8, 0x99,
            ],
            Self::V1 => &[
                // https://www.rfc-editor.org/rfc/rfc9001.html#name-initial-secrets
                0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8,
                0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a,
            ],
            Self::V2 => &[
                // https://www.ietf.org/archive/id/draft-ietf-quic-v2-10.html#name-initial-salt-2
                0x0d, 0xed, 0xe3, 0xde, 0xf7, 0x00, 0xa6, 0xdb, 0x81, 0x93, 0x81, 0xbe, 0x6e, 0x26,
                0x9d, 0xcb, 0xf9, 0xbd, 0x2e, 0xd9,
            ],
        }
    }

    /// Key derivation label for packet keys.
    pub(crate) fn packet_key_label(&self) -> &'static [u8] {
        match self {
            Self::V1Draft | Self::V1 => b"quic key",
            Self::V2 => b"quicv2 key",
        }
    }

    /// Key derivation label for packet "IV"s.
    pub(crate) fn packet_iv_label(&self) -> &'static [u8] {
        match self {
            Self::V1Draft | Self::V1 => b"quic iv",
            Self::V2 => b"quicv2 iv",
        }
    }

    /// Key derivation for header keys.
    pub(crate) fn header_key_label(&self) -> &'static [u8] {
        match self {
            Self::V1Draft | Self::V1 => b"quic hp",
            Self::V2 => b"quicv2 hp",
        }
    }

    fn key_update_label(&self) -> &'static [u8] {
        match self {
            Self::V1Draft | Self::V1 => b"quic ku",
            Self::V2 => b"quicv2 ku",
        }
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::V1
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;

    use super::PacketKey;
    use crate::quic::HeaderProtectionKey;

    #[test]
    fn auto_traits() {
        fn assert_auto<T: Send + Sync>() {}
        assert_auto::<Box<dyn PacketKey>>();
        assert_auto::<Box<dyn HeaderProtectionKey>>();
    }
}
