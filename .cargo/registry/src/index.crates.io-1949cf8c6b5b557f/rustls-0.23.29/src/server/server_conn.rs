use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
#[cfg(feature = "std")]
use std::io;

use pki_types::{DnsName, UnixTime};

use super::hs;
#[cfg(feature = "std")]
use crate::WantsVerifier;
use crate::builder::ConfigBuilder;
use crate::common_state::{CommonState, Side};
#[cfg(feature = "std")]
use crate::common_state::{Protocol, State};
use crate::conn::{ConnectionCommon, ConnectionCore, UnbufferedConnectionCommon};
#[cfg(doc)]
use crate::crypto;
use crate::crypto::CryptoProvider;
use crate::enums::{CertificateType, CipherSuite, ProtocolVersion, SignatureScheme};
use crate::error::Error;
use crate::kernel::KernelConnection;
use crate::log::trace;
use crate::msgs::base::Payload;
use crate::msgs::handshake::{ClientHelloPayload, ProtocolName, ServerExtensionsInput};
use crate::msgs::message::Message;
use crate::suites::ExtractedSecrets;
use crate::sync::Arc;
#[cfg(feature = "std")]
use crate::time_provider::DefaultTimeProvider;
use crate::time_provider::TimeProvider;
use crate::vecbuf::ChunkVecBuffer;
use crate::{
    DistinguishedName, KeyLog, NamedGroup, WantsVersions, compress, sign, verify, versions,
};

/// A trait for the ability to store server session data.
///
/// The keys and values are opaque.
///
/// Inserted keys are randomly chosen by the library and have
/// no internal structure (in other words, you may rely on all
/// bits being uniformly random).  Queried keys are untrusted data.
///
/// Both the keys and values should be treated as
/// **highly sensitive data**, containing enough key material
/// to break all security of the corresponding sessions.
///
/// Implementations can be lossy (in other words, forgetting
/// key/value pairs) without any negative security consequences.
///
/// However, note that `take` **must** reliably delete a returned
/// value.  If it does not, there may be security consequences.
///
/// `put` and `take` are mutating operations; this isn't expressed
/// in the type system to allow implementations freedom in
/// how to achieve interior mutability.  `Mutex` is a common
/// choice.
pub trait StoresServerSessions: Debug + Send + Sync {
    /// Store session secrets encoded in `value` against `key`,
    /// overwrites any existing value against `key`.  Returns `true`
    /// if the value was stored.
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool;

    /// Find a value with the given `key`.  Return it, or None
    /// if it doesn't exist.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Find a value with the given `key`.  Return it and delete it;
    /// or None if it doesn't exist.
    fn take(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Whether the store can cache another session. This is used to indicate to clients
    /// whether their session can be resumed; the implementation is not required to remember
    /// a session even if it returns `true` here.
    fn can_cache(&self) -> bool;
}

/// A trait for the ability to encrypt and decrypt tickets.
pub trait ProducesTickets: Debug + Send + Sync {
    /// Returns true if this implementation will encrypt/decrypt
    /// tickets.  Should return false if this is a dummy
    /// implementation: the server will not send the SessionTicket
    /// extension and will not call the other functions.
    fn enabled(&self) -> bool;

    /// Returns the lifetime in seconds of tickets produced now.
    /// The lifetime is provided as a hint to clients that the
    /// ticket will not be useful after the given time.
    ///
    /// This lifetime must be implemented by key rolling and
    /// erasure, *not* by storing a lifetime in the ticket.
    ///
    /// The objective is to limit damage to forward secrecy caused
    /// by tickets, not just limiting their lifetime.
    fn lifetime(&self) -> u32;

    /// Encrypt and authenticate `plain`, returning the resulting
    /// ticket.  Return None if `plain` cannot be encrypted for
    /// some reason: an empty ticket will be sent and the connection
    /// will continue.
    fn encrypt(&self, plain: &[u8]) -> Option<Vec<u8>>;

    /// Decrypt `cipher`, validating its authenticity protection
    /// and recovering the plaintext.  `cipher` is fully attacker
    /// controlled, so this decryption must be side-channel free,
    /// panic-proof, and otherwise bullet-proof.  If the decryption
    /// fails, return None.
    fn decrypt(&self, cipher: &[u8]) -> Option<Vec<u8>>;
}

/// How to choose a certificate chain and signing key for use
/// in server authentication.
///
/// This is suitable when selecting a certificate does not require
/// I/O or when the application is using blocking I/O anyhow.
///
/// For applications that use async I/O and need to do I/O to choose
/// a certificate (for instance, fetching a certificate from a data store),
/// the [`Acceptor`] interface is more suitable.
pub trait ResolvesServerCert: Debug + Send + Sync {
    /// Choose a certificate chain and matching key given simplified
    /// ClientHello information.
    ///
    /// Return `None` to abort the handshake.
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<sign::CertifiedKey>>;

    /// Return true when the server only supports raw public keys.
    fn only_raw_public_keys(&self) -> bool {
        false
    }
}

/// A struct representing the received Client Hello
#[derive(Debug)]
pub struct ClientHello<'a> {
    pub(super) server_name: &'a Option<DnsName<'a>>,
    pub(super) signature_schemes: &'a [SignatureScheme],
    pub(super) alpn: Option<&'a Vec<ProtocolName>>,
    pub(super) server_cert_types: Option<&'a [CertificateType]>,
    pub(super) client_cert_types: Option<&'a [CertificateType]>,
    pub(super) cipher_suites: &'a [CipherSuite],
    /// The [certificate_authorities] extension, if it was sent by the client.
    ///
    /// [certificate_authorities]: https://datatracker.ietf.org/doc/html/rfc8446#section-4.2.4
    pub(super) certificate_authorities: Option<&'a [DistinguishedName]>,
    pub(super) named_groups: Option<&'a [NamedGroup]>,
}

impl<'a> ClientHello<'a> {
    /// Get the server name indicator.
    ///
    /// Returns `None` if the client did not supply a SNI.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name
            .as_ref()
            .map(<DnsName<'_> as AsRef<str>>::as_ref)
    }

    /// Get the compatible signature schemes.
    ///
    /// Returns standard-specified default if the client omitted this extension.
    pub fn signature_schemes(&self) -> &[SignatureScheme] {
        self.signature_schemes
    }

    /// Get the ALPN protocol identifiers submitted by the client.
    ///
    /// Returns `None` if the client did not include an ALPN extension.
    ///
    /// Application Layer Protocol Negotiation (ALPN) is a TLS extension that lets a client
    /// submit a set of identifiers that each a represent an application-layer protocol.
    /// The server will then pick its preferred protocol from the set submitted by the client.
    /// Each identifier is represented as a byte array, although common values are often ASCII-encoded.
    /// See the official RFC-7301 specifications at <https://datatracker.ietf.org/doc/html/rfc7301>
    /// for more information on ALPN.
    ///
    /// For example, a HTTP client might specify "http/1.1" and/or "h2". Other well-known values
    /// are listed in the at IANA registry at
    /// <https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids>.
    ///
    /// The server can specify supported ALPN protocols by setting [`ServerConfig::alpn_protocols`].
    /// During the handshake, the server will select the first protocol configured that the client supports.
    pub fn alpn(&self) -> Option<impl Iterator<Item = &'a [u8]>> {
        self.alpn.map(|protocols| {
            protocols
                .iter()
                .map(|proto| proto.as_ref())
        })
    }

    /// Get cipher suites.
    pub fn cipher_suites(&self) -> &[CipherSuite] {
        self.cipher_suites
    }

    /// Get the server certificate types offered in the ClientHello.
    ///
    /// Returns `None` if the client did not include a certificate type extension.
    pub fn server_cert_types(&self) -> Option<&'a [CertificateType]> {
        self.server_cert_types
    }

    /// Get the client certificate types offered in the ClientHello.
    ///
    /// Returns `None` if the client did not include a certificate type extension.
    pub fn client_cert_types(&self) -> Option<&'a [CertificateType]> {
        self.client_cert_types
    }

    /// Get the [certificate_authorities] extension sent by the client.
    ///
    /// Returns `None` if the client did not send this extension.
    ///
    /// [certificate_authorities]: https://datatracker.ietf.org/doc/html/rfc8446#section-4.2.4
    pub fn certificate_authorities(&self) -> Option<&'a [DistinguishedName]> {
        self.certificate_authorities
    }

    /// Get the [`named_groups`] extension sent by the client.
    ///
    /// This means different things in different versions of TLS:
    ///
    /// Originally it was introduced as the "[`elliptic_curves`]" extension for TLS1.2.
    /// It described the elliptic curves supported by a client for all purposes: key
    /// exchange, signature verification (for server authentication), and signing (for
    /// client auth).  Later [RFC7919] extended this to include FFDHE "named groups",
    /// but FFDHE groups in this context only relate to key exchange.
    ///
    /// In TLS1.3 it was renamed to "[`named_groups`]" and now describes all types
    /// of key exchange mechanisms, and does not relate at all to elliptic curves
    /// used for signatures.
    ///
    /// [`elliptic_curves`]: https://datatracker.ietf.org/doc/html/rfc4492#section-5.1.1
    /// [RFC7919]: https://datatracker.ietf.org/doc/html/rfc7919#section-2
    /// [`named_groups`]:https://datatracker.ietf.org/doc/html/rfc8446#section-4.2.7
    pub fn named_groups(&self) -> Option<&'a [NamedGroup]> {
        self.named_groups
    }
}

/// Common configuration for a set of server sessions.
///
/// Making one of these is cheap, though one of the inputs may be expensive: gathering trust roots
/// from the operating system to add to the [`RootCertStore`] passed to a `ClientCertVerifier`
/// builder may take on the order of a few hundred milliseconds.
///
/// These must be created via the [`ServerConfig::builder()`] or [`ServerConfig::builder_with_provider()`]
/// function.
///
/// # Defaults
///
/// * [`ServerConfig::max_fragment_size`]: the default is `None` (meaning 16kB).
/// * [`ServerConfig::session_storage`]: if the `std` feature is enabled, the default stores 256
///   sessions in memory. If the `std` feature is not enabled, the default is to not store any
///   sessions. In a no-std context, by enabling the `hashbrown` feature you may provide your
///   own `session_storage` using [`ServerSessionMemoryCache`] and a `crate::lock::MakeMutex`
///   implementation.
/// * [`ServerConfig::alpn_protocols`]: the default is empty -- no ALPN protocol is negotiated.
/// * [`ServerConfig::key_log`]: key material is not logged.
/// * [`ServerConfig::send_tls13_tickets`]: 2 tickets are sent.
/// * [`ServerConfig::cert_compressors`]: depends on the crate features, see [`compress::default_cert_compressors()`].
/// * [`ServerConfig::cert_compression_cache`]: caches the most recently used 4 compressions
/// * [`ServerConfig::cert_decompressors`]: depends on the crate features, see [`compress::default_cert_decompressors()`].
///
/// # Sharing resumption storage between `ServerConfig`s
///
/// In a program using many `ServerConfig`s it may improve resumption rates
/// (which has a significant impact on connection performance) if those
/// configs share [`ServerConfig::session_storage`] or [`ServerConfig::ticketer`].
///
/// However, caution is needed: other fields influence the security of a session
/// and resumption between them can be surprising.  If sharing
/// [`ServerConfig::session_storage`] or [`ServerConfig::ticketer`] between two
/// `ServerConfig`s, you should also evaluate the following fields and ensure
/// they are equivalent:
///
/// * `ServerConfig::verifier` -- client authentication requirements,
/// * [`ServerConfig::cert_resolver`] -- server identities.
///
/// To illustrate, imagine two `ServerConfig`s `A` and `B`.  `A` requires
/// client authentication, `B` does not.  If `A` and `B` shared a resumption store,
/// it would be possible for a session originated by `B` (that is, an unauthenticated client)
/// to be inserted into the store, and then resumed by `A`.  This would give a false
/// impression to the user of `A` that the client was authenticated.  This is possible
/// whether the resumption is performed statefully (via [`ServerConfig::session_storage`])
/// or statelessly (via [`ServerConfig::ticketer`]).
///
/// _Unlike_ `ClientConfig`, rustls does not enforce any policy here.
///
/// [`RootCertStore`]: crate::RootCertStore
/// [`ServerSessionMemoryCache`]: crate::server::handy::ServerSessionMemoryCache
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Source of randomness and other crypto.
    pub(super) provider: Arc<CryptoProvider>,

    /// Ignore the client's ciphersuite order. Instead,
    /// choose the top ciphersuite in the server list
    /// which is supported by the client.
    pub ignore_client_order: bool,

    /// The maximum size of plaintext input to be emitted in a single TLS record.
    /// A value of None is equivalent to the [TLS maximum] of 16 kB.
    ///
    /// rustls enforces an arbitrary minimum of 32 bytes for this field.
    /// Out of range values are reported as errors from [ServerConnection::new].
    ///
    /// Setting this value to a little less than the TCP MSS may improve latency
    /// for stream-y workloads.
    ///
    /// [TLS maximum]: https://datatracker.ietf.org/doc/html/rfc8446#section-5.1
    /// [ServerConnection::new]: crate::server::ServerConnection::new
    pub max_fragment_size: Option<usize>,

    /// How to store client sessions.
    ///
    /// See [ServerConfig#sharing-resumption-storage-between-serverconfigs]
    /// for a warning related to this field.
    pub session_storage: Arc<dyn StoresServerSessions>,

    /// How to produce tickets.
    ///
    /// See [ServerConfig#sharing-resumption-storage-between-serverconfigs]
    /// for a warning related to this field.
    pub ticketer: Arc<dyn ProducesTickets>,

    /// How to choose a server cert and key. This is usually set by
    /// [ConfigBuilder::with_single_cert] or [ConfigBuilder::with_cert_resolver].
    /// For async applications, see also [Acceptor].
    pub cert_resolver: Arc<dyn ResolvesServerCert>,

    /// Protocol names we support, most preferred first.
    /// If empty we don't do ALPN at all.
    pub alpn_protocols: Vec<Vec<u8>>,

    /// Supported protocol versions, in no particular order.
    /// The default is all supported versions.
    pub(super) versions: versions::EnabledVersions,

    /// How to verify client certificates.
    pub(super) verifier: Arc<dyn verify::ClientCertVerifier>,

    /// How to output key material for debugging.  The default
    /// does nothing.
    pub key_log: Arc<dyn KeyLog>,

    /// Allows traffic secrets to be extracted after the handshake,
    /// e.g. for kTLS setup.
    pub enable_secret_extraction: bool,

    /// Amount of early data to accept for sessions created by
    /// this config.  Specify 0 to disable early data.  The
    /// default is 0.
    ///
    /// Read the early data via [`ServerConnection::early_data`].
    ///
    /// The units for this are _both_ plaintext bytes, _and_ ciphertext
    /// bytes, depending on whether the server accepts a client's early_data
    /// or not.  It is therefore recommended to include some slop in
    /// this value to account for the unknown amount of ciphertext
    /// expansion in the latter case.
    pub max_early_data_size: u32,

    /// Whether the server should send "0.5RTT" data.  This means the server
    /// sends data after its first flight of handshake messages, without
    /// waiting for the client to complete the handshake.
    ///
    /// This can improve TTFB latency for either server-speaks-first protocols,
    /// or client-speaks-first protocols when paired with "0RTT" data.  This
    /// comes at the cost of a subtle weakening of the normal handshake
    /// integrity guarantees that TLS provides.  Note that the initial
    /// `ClientHello` is indirectly authenticated because it is included
    /// in the transcript used to derive the keys used to encrypt the data.
    ///
    /// This only applies to TLS1.3 connections.  TLS1.2 connections cannot
    /// do this optimisation and this setting is ignored for them.  It is
    /// also ignored for TLS1.3 connections that even attempt client
    /// authentication.
    ///
    /// This defaults to false.  This means the first application data
    /// sent by the server comes after receiving and validating the client's
    /// handshake up to the `Finished` message.  This is the safest option.
    pub send_half_rtt_data: bool,

    /// How many TLS1.3 tickets to send immediately after a successful
    /// handshake.
    ///
    /// Because TLS1.3 tickets are single-use, this allows
    /// a client to perform multiple resumptions.
    ///
    /// The default is 2.
    ///
    /// If this is 0, no tickets are sent and clients will not be able to
    /// do any resumption.
    pub send_tls13_tickets: usize,

    /// If set to `true`, requires the client to support the extended
    /// master secret extraction method defined in [RFC 7627].
    ///
    /// The default is `true` if the "fips" crate feature is enabled,
    /// `false` otherwise.
    ///
    /// It must be set to `true` to meet FIPS requirement mentioned in section
    /// **D.Q Transition of the TLS 1.2 KDF to Support the Extended Master
    /// Secret** from [FIPS 140-3 IG.pdf].
    ///
    /// [RFC 7627]: https://datatracker.ietf.org/doc/html/rfc7627
    /// [FIPS 140-3 IG.pdf]: https://csrc.nist.gov/csrc/media/Projects/cryptographic-module-validation-program/documents/fips%20140-3/FIPS%20140-3%20IG.pdf
    #[cfg(feature = "tls12")]
    pub require_ems: bool,

    /// Provides the current system time
    pub time_provider: Arc<dyn TimeProvider>,

    /// How to compress the server's certificate chain.
    ///
    /// If a client supports this extension, and advertises support
    /// for one of the compression algorithms included here, the
    /// server certificate will be compressed according to [RFC8779].
    ///
    /// This only applies to TLS1.3 connections.  It is ignored for
    /// TLS1.2 connections.
    ///
    /// [RFC8779]: https://datatracker.ietf.org/doc/rfc8879/
    pub cert_compressors: Vec<&'static dyn compress::CertCompressor>,

    /// Caching for compressed certificates.
    ///
    /// This is optional: [`compress::CompressionCache::Disabled`] gives
    /// a cache that does no caching.
    pub cert_compression_cache: Arc<compress::CompressionCache>,

    /// How to decompress the clients's certificate chain.
    ///
    /// If this is non-empty, the [RFC8779] certificate compression
    /// extension is offered when requesting client authentication,
    /// and any compressed certificates are transparently decompressed
    /// during the handshake.
    ///
    /// This only applies to TLS1.3 connections.  It is ignored for
    /// TLS1.2 connections.
    ///
    /// [RFC8779]: https://datatracker.ietf.org/doc/rfc8879/
    pub cert_decompressors: Vec<&'static dyn compress::CertDecompressor>,
}

impl ServerConfig {
    /// Create a builder for a server configuration with
    /// [the process-default `CryptoProvider`][CryptoProvider#using-the-per-process-default-cryptoprovider]
    /// and safe protocol version defaults.
    ///
    /// For more information, see the [`ConfigBuilder`] documentation.
    #[cfg(feature = "std")]
    pub fn builder() -> ConfigBuilder<Self, WantsVerifier> {
        Self::builder_with_protocol_versions(versions::DEFAULT_VERSIONS)
    }

    /// Create a builder for a server configuration with
    /// [the process-default `CryptoProvider`][CryptoProvider#using-the-per-process-default-cryptoprovider]
    /// and the provided protocol versions.
    ///
    /// Panics if
    /// - the supported versions are not compatible with the provider (eg.
    ///   the combination of ciphersuites supported by the provider and supported
    ///   versions lead to zero cipher suites being usable),
    /// - if a `CryptoProvider` cannot be resolved using a combination of
    ///   the crate features and process default.
    ///
    /// For more information, see the [`ConfigBuilder`] documentation.
    #[cfg(feature = "std")]
    pub fn builder_with_protocol_versions(
        versions: &[&'static versions::SupportedProtocolVersion],
    ) -> ConfigBuilder<Self, WantsVerifier> {
        // Safety assumptions:
        // 1. that the provider has been installed (explicitly or implicitly)
        // 2. that the process-level default provider is usable with the supplied protocol versions.
        Self::builder_with_provider(
            CryptoProvider::get_default_or_install_from_crate_features().clone(),
        )
        .with_protocol_versions(versions)
        .unwrap()
    }

    /// Create a builder for a server configuration with a specific [`CryptoProvider`].
    ///
    /// This will use the provider's configured ciphersuites. You must additionally choose
    /// which protocol versions to enable, using `with_protocol_versions` or
    /// `with_safe_default_protocol_versions` and handling the `Result` in case a protocol
    /// version is not supported by the provider's ciphersuites.
    ///
    /// For more information, see the [`ConfigBuilder`] documentation.
    #[cfg(feature = "std")]
    pub fn builder_with_provider(
        provider: Arc<CryptoProvider>,
    ) -> ConfigBuilder<Self, WantsVersions> {
        ConfigBuilder {
            state: WantsVersions {},
            provider,
            time_provider: Arc::new(DefaultTimeProvider),
            side: PhantomData,
        }
    }

    /// Create a builder for a server configuration with no default implementation details.
    ///
    /// This API must be used by `no_std` users.
    ///
    /// You must provide a specific [`TimeProvider`].
    ///
    /// You must provide a specific [`CryptoProvider`].
    ///
    /// This will use the provider's configured ciphersuites. You must additionally choose
    /// which protocol versions to enable, using `with_protocol_versions` or
    /// `with_safe_default_protocol_versions` and handling the `Result` in case a protocol
    /// version is not supported by the provider's ciphersuites.
    ///
    /// For more information, see the [`ConfigBuilder`] documentation.
    pub fn builder_with_details(
        provider: Arc<CryptoProvider>,
        time_provider: Arc<dyn TimeProvider>,
    ) -> ConfigBuilder<Self, WantsVersions> {
        ConfigBuilder {
            state: WantsVersions {},
            provider,
            time_provider,
            side: PhantomData,
        }
    }

    /// Return `true` if connections made with this `ServerConfig` will
    /// operate in FIPS mode.
    ///
    /// This is different from [`CryptoProvider::fips()`]: [`CryptoProvider::fips()`]
    /// is concerned only with cryptography, whereas this _also_ covers TLS-level
    /// configuration that NIST recommends.
    pub fn fips(&self) -> bool {
        #[cfg(feature = "tls12")]
        {
            self.provider.fips() && self.require_ems
        }

        #[cfg(not(feature = "tls12"))]
        {
            self.provider.fips()
        }
    }

    /// Return the crypto provider used to construct this client configuration.
    pub fn crypto_provider(&self) -> &Arc<CryptoProvider> {
        &self.provider
    }

    /// We support a given TLS version if it's quoted in the configured
    /// versions *and* at least one ciphersuite for this version is
    /// also configured.
    pub(crate) fn supports_version(&self, v: ProtocolVersion) -> bool {
        self.versions.contains(v)
            && self
                .provider
                .cipher_suites
                .iter()
                .any(|cs| cs.version().version == v)
    }

    #[cfg(feature = "std")]
    pub(crate) fn supports_protocol(&self, proto: Protocol) -> bool {
        self.provider
            .cipher_suites
            .iter()
            .any(|cs| cs.usable_for_protocol(proto))
    }

    pub(super) fn current_time(&self) -> Result<UnixTime, Error> {
        self.time_provider
            .current_time()
            .ok_or(Error::FailedToGetCurrentTime)
    }
}

#[cfg(feature = "std")]
mod connection {
    use alloc::boxed::Box;
    use core::fmt;
    use core::fmt::{Debug, Formatter};
    use core::ops::{Deref, DerefMut};
    use std::io;

    use super::{
        Accepted, Accepting, EarlyDataState, ServerConfig, ServerConnectionData,
        ServerExtensionsInput,
    };
    use crate::common_state::{CommonState, Context, Side};
    use crate::conn::{ConnectionCommon, ConnectionCore};
    use crate::error::Error;
    use crate::server::hs;
    use crate::suites::ExtractedSecrets;
    use crate::sync::Arc;
    use crate::vecbuf::ChunkVecBuffer;

    /// Allows reading of early data in resumed TLS1.3 connections.
    ///
    /// "Early data" is also known as "0-RTT data".
    ///
    /// This structure implements [`std::io::Read`].
    pub struct ReadEarlyData<'a> {
        early_data: &'a mut EarlyDataState,
    }

    impl<'a> ReadEarlyData<'a> {
        fn new(early_data: &'a mut EarlyDataState) -> Self {
            ReadEarlyData { early_data }
        }
    }

    impl io::Read for ReadEarlyData<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.early_data.read(buf)
        }

        #[cfg(read_buf)]
        fn read_buf(&mut self, cursor: core::io::BorrowedCursor<'_>) -> io::Result<()> {
            self.early_data.read_buf(cursor)
        }
    }

    /// This represents a single TLS server connection.
    ///
    /// Send TLS-protected data to the peer using the `io::Write` trait implementation.
    /// Read data from the peer using the `io::Read` trait implementation.
    pub struct ServerConnection {
        pub(super) inner: ConnectionCommon<ServerConnectionData>,
    }

    impl ServerConnection {
        /// Make a new ServerConnection.  `config` controls how
        /// we behave in the TLS protocol.
        pub fn new(config: Arc<ServerConfig>) -> Result<Self, Error> {
            Ok(Self {
                inner: ConnectionCommon::from(ConnectionCore::for_server(
                    config,
                    ServerExtensionsInput::default(),
                )?),
            })
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

        /// Application-controlled portion of the resumption ticket supplied by the client, if any.
        ///
        /// Recovered from the prior session's `set_resumption_data`. Integrity is guaranteed by rustls.
        ///
        /// Returns `Some` if and only if a valid resumption ticket has been received from the client.
        pub fn received_resumption_data(&self) -> Option<&[u8]> {
            self.inner
                .core
                .data
                .received_resumption_data
                .as_ref()
                .map(|x| &x[..])
        }

        /// Set the resumption data to embed in future resumption tickets supplied to the client.
        ///
        /// Defaults to the empty byte string. Must be less than 2^15 bytes to allow room for other
        /// data. Should be called while `is_handshaking` returns true to ensure all transmitted
        /// resumption tickets are affected.
        ///
        /// Integrity will be assured by rustls, but the data will be visible to the client. If secrecy
        /// from the client is desired, encrypt the data separately.
        pub fn set_resumption_data(&mut self, data: &[u8]) {
            assert!(data.len() < 2usize.pow(15));
            self.inner.core.data.resumption_data = data.into();
        }

        /// Explicitly discard early data, notifying the client
        ///
        /// Useful if invariants encoded in `received_resumption_data()` cannot be respected.
        ///
        /// Must be called while `is_handshaking` is true.
        pub fn reject_early_data(&mut self) {
            self.inner.core.reject_early_data()
        }

        /// Returns an `io::Read` implementer you can read bytes from that are
        /// received from a client as TLS1.3 0RTT/"early" data, during the handshake.
        ///
        /// This returns `None` in many circumstances, such as :
        ///
        /// - Early data is disabled if [`ServerConfig::max_early_data_size`] is zero (the default).
        /// - The session negotiated with the client is not TLS1.3.
        /// - The client just doesn't support early data.
        /// - The connection doesn't resume an existing session.
        /// - The client hasn't sent a full ClientHello yet.
        pub fn early_data(&mut self) -> Option<ReadEarlyData<'_>> {
            let data = &mut self.inner.core.data;
            if data.early_data.was_accepted() {
                Some(ReadEarlyData::new(&mut data.early_data))
            } else {
                None
            }
        }

        /// Return true if the connection was made with a `ServerConfig` that is FIPS compatible.
        ///
        /// This is different from [`crate::crypto::CryptoProvider::fips()`]:
        /// it is concerned only with cryptography, whereas this _also_ covers TLS-level
        /// configuration that NIST recommends, as well as ECH HPKE suites if applicable.
        pub fn fips(&self) -> bool {
            self.inner.core.common_state.fips
        }

        /// Extract secrets, so they can be used when configuring kTLS, for example.
        /// Should be used with care as it exposes secret key material.
        pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
            self.inner.dangerous_extract_secrets()
        }
    }

    impl Debug for ServerConnection {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("ServerConnection")
                .finish()
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

    impl From<ServerConnection> for crate::Connection {
        fn from(conn: ServerConnection) -> Self {
            Self::Server(conn)
        }
    }

    /// Handle a server-side connection before configuration is available.
    ///
    /// `Acceptor` allows the caller to choose a [`ServerConfig`] after reading
    /// the [`super::ClientHello`] of an incoming connection. This is useful for servers
    /// that choose different certificates or cipher suites based on the
    /// characteristics of the `ClientHello`. In particular it is useful for
    /// servers that need to do some I/O to load a certificate and its private key
    /// and don't want to use the blocking interface provided by
    /// [`super::ResolvesServerCert`].
    ///
    /// Create an Acceptor with [`Acceptor::default()`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #[cfg(feature = "aws_lc_rs")] {
    /// # fn choose_server_config(
    /// #     _: rustls::server::ClientHello,
    /// # ) -> std::sync::Arc<rustls::ServerConfig> {
    /// #     unimplemented!();
    /// # }
    /// # #[allow(unused_variables)]
    /// # fn main() {
    /// use rustls::server::{Acceptor, ServerConfig};
    /// let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    /// for stream in listener.incoming() {
    ///     let mut stream = stream.unwrap();
    ///     let mut acceptor = Acceptor::default();
    ///     let accepted = loop {
    ///         acceptor.read_tls(&mut stream).unwrap();
    ///         if let Some(accepted) = acceptor.accept().unwrap() {
    ///             break accepted;
    ///         }
    ///     };
    ///
    ///     // For some user-defined choose_server_config:
    ///     let config = choose_server_config(accepted.client_hello());
    ///     let conn = accepted
    ///         .into_connection(config)
    ///         .unwrap();
    ///
    ///     // Proceed with handling the ServerConnection.
    /// }
    /// # }
    /// # }
    /// ```
    pub struct Acceptor {
        inner: Option<ConnectionCommon<ServerConnectionData>>,
    }

    impl Default for Acceptor {
        /// Return an empty Acceptor, ready to receive bytes from a new client connection.
        fn default() -> Self {
            Self {
                inner: Some(
                    ConnectionCore::new(
                        Box::new(Accepting),
                        ServerConnectionData::default(),
                        CommonState::new(Side::Server),
                    )
                    .into(),
                ),
            }
        }
    }

    impl Acceptor {
        /// Read TLS content from `rd`.
        ///
        /// Returns an error if this `Acceptor` has already yielded an [`Accepted`]. For more details,
        /// refer to [`Connection::read_tls()`].
        ///
        /// [`Connection::read_tls()`]: crate::Connection::read_tls
        pub fn read_tls(&mut self, rd: &mut dyn io::Read) -> Result<usize, io::Error> {
            match &mut self.inner {
                Some(conn) => conn.read_tls(rd),
                None => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "acceptor cannot read after successful acceptance",
                )),
            }
        }

        /// Check if a `ClientHello` message has been received.
        ///
        /// Returns `Ok(None)` if the complete `ClientHello` has not yet been received.
        /// Do more I/O and then call this function again.
        ///
        /// Returns `Ok(Some(accepted))` if the connection has been accepted. Call
        /// `accepted.into_connection()` to continue. Do not call this function again.
        ///
        /// Returns `Err((err, alert))` if an error occurred. If an alert is returned, the
        /// application should call `alert.write()` to send the alert to the client. It should
        /// not call `accept()` again.
        pub fn accept(&mut self) -> Result<Option<Accepted>, (Error, AcceptedAlert)> {
            let Some(mut connection) = self.inner.take() else {
                return Err((
                    Error::General("Acceptor polled after completion".into()),
                    AcceptedAlert::empty(),
                ));
            };

            let message = match connection.first_handshake_message() {
                Ok(Some(msg)) => msg,
                Ok(None) => {
                    self.inner = Some(connection);
                    return Ok(None);
                }
                Err(err) => return Err((err, AcceptedAlert::from(connection))),
            };

            let mut cx = Context::from(&mut connection);
            let sig_schemes = match hs::process_client_hello(&message, false, &mut cx) {
                Ok((_, sig_schemes)) => sig_schemes,
                Err(err) => {
                    return Err((err, AcceptedAlert::from(connection)));
                }
            };

            Ok(Some(Accepted {
                connection,
                message,
                sig_schemes,
            }))
        }
    }

    /// Represents a TLS alert resulting from handling the client's `ClientHello` message.
    ///
    /// When [`Acceptor::accept()`] returns an error, it yields an `AcceptedAlert` such that the
    /// application can communicate failure to the client via [`AcceptedAlert::write()`].
    pub struct AcceptedAlert(ChunkVecBuffer);

    impl AcceptedAlert {
        pub(super) fn empty() -> Self {
            Self(ChunkVecBuffer::new(None))
        }

        /// Send the alert to the client.
        ///
        /// To account for short writes this function should be called repeatedly until it
        /// returns `Ok(0)` or an error.
        pub fn write(&mut self, wr: &mut dyn io::Write) -> Result<usize, io::Error> {
            self.0.write_to(wr)
        }

        /// Send the alert to the client.
        ///
        /// This function will invoke the writer until the buffer is empty.
        pub fn write_all(&mut self, wr: &mut dyn io::Write) -> Result<(), io::Error> {
            while self.write(wr)? != 0 {}
            Ok(())
        }
    }

    impl From<ConnectionCommon<ServerConnectionData>> for AcceptedAlert {
        fn from(conn: ConnectionCommon<ServerConnectionData>) -> Self {
            Self(conn.core.common_state.sendable_tls)
        }
    }

    impl Debug for AcceptedAlert {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("AcceptedAlert").finish()
        }
    }
}

#[cfg(feature = "std")]
pub use connection::{AcceptedAlert, Acceptor, ReadEarlyData, ServerConnection};

/// Unbuffered version of `ServerConnection`
///
/// See the [`crate::unbuffered`] module docs for more details
pub struct UnbufferedServerConnection {
    inner: UnbufferedConnectionCommon<ServerConnectionData>,
}

impl UnbufferedServerConnection {
    /// Make a new ServerConnection. `config` controls how we behave in the TLS protocol.
    pub fn new(config: Arc<ServerConfig>) -> Result<Self, Error> {
        Ok(Self {
            inner: UnbufferedConnectionCommon::from(ConnectionCore::for_server(
                config,
                ServerExtensionsInput::default(),
            )?),
        })
    }

    /// Extract secrets, so they can be used when configuring kTLS, for example.
    /// Should be used with care as it exposes secret key material.
    #[deprecated = "dangerous_extract_secrets() does not support session tickets or \
                    key updates, use dangerous_into_kernel_connection() instead"]
    pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
        self.inner.dangerous_extract_secrets()
    }

    /// Extract secrets and an [`KernelConnection`] object.
    ///
    /// This allows you use rustls to manage keys and then manage encryption and
    /// decryption yourself (e.g. for kTLS).
    ///
    /// Should be used with care as it exposes secret key material.
    ///
    /// See the [`crate::kernel`] documentations for details on prerequisites
    /// for calling this method.
    pub fn dangerous_into_kernel_connection(
        self,
    ) -> Result<(ExtractedSecrets, KernelConnection<ServerConnectionData>), Error> {
        self.inner
            .core
            .dangerous_into_kernel_connection()
    }
}

impl Deref for UnbufferedServerConnection {
    type Target = UnbufferedConnectionCommon<ServerConnectionData>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for UnbufferedServerConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl UnbufferedConnectionCommon<ServerConnectionData> {
    pub(crate) fn pop_early_data(&mut self) -> Option<Vec<u8>> {
        self.core.data.early_data.pop()
    }

    pub(crate) fn peek_early_data(&self) -> Option<&[u8]> {
        self.core.data.early_data.peek()
    }
}

/// Represents a `ClientHello` message received through the [`Acceptor`].
///
/// Contains the state required to resume the connection through [`Accepted::into_connection()`].
pub struct Accepted {
    connection: ConnectionCommon<ServerConnectionData>,
    message: Message<'static>,
    sig_schemes: Vec<SignatureScheme>,
}

impl Accepted {
    /// Get the [`ClientHello`] for this connection.
    pub fn client_hello(&self) -> ClientHello<'_> {
        let payload = Self::client_hello_payload(&self.message);
        let ch = ClientHello {
            server_name: &self.connection.core.data.sni,
            signature_schemes: &self.sig_schemes,
            alpn: payload.protocols.as_ref(),
            server_cert_types: payload
                .server_certificate_types
                .as_deref(),
            client_cert_types: payload
                .client_certificate_types
                .as_deref(),
            cipher_suites: &payload.cipher_suites,
            certificate_authorities: payload
                .certificate_authority_names
                .as_deref(),
            named_groups: payload.named_groups.as_deref(),
        };

        trace!("Accepted::client_hello(): {ch:#?}");
        ch
    }

    /// Convert the [`Accepted`] into a [`ServerConnection`].
    ///
    /// Takes the state returned from [`Acceptor::accept()`] as well as the [`ServerConfig`] and
    /// [`sign::CertifiedKey`] that should be used for the session. Returns an error if
    /// configuration-dependent validation of the received `ClientHello` message fails.
    #[cfg(feature = "std")]
    pub fn into_connection(
        mut self,
        config: Arc<ServerConfig>,
    ) -> Result<ServerConnection, (Error, AcceptedAlert)> {
        if let Err(err) = self
            .connection
            .set_max_fragment_size(config.max_fragment_size)
        {
            // We have a connection here, but it won't contain an alert since the error
            // is with the fragment size configured in the `ServerConfig`.
            return Err((err, AcceptedAlert::empty()));
        }

        self.connection.enable_secret_extraction = config.enable_secret_extraction;

        let state = hs::ExpectClientHello::new(config, ServerExtensionsInput::default());
        let mut cx = hs::ServerContext::from(&mut self.connection);

        let ch = Self::client_hello_payload(&self.message);
        let new = match state.with_certified_key(self.sig_schemes, ch, &self.message, &mut cx) {
            Ok(new) => new,
            Err(err) => return Err((err, AcceptedAlert::from(self.connection))),
        };

        self.connection.replace_state(new);
        Ok(ServerConnection {
            inner: self.connection,
        })
    }

    fn client_hello_payload<'a>(message: &'a Message<'_>) -> &'a ClientHelloPayload {
        match &message.payload {
            crate::msgs::message::MessagePayload::Handshake { parsed, .. } => match &parsed.0 {
                crate::msgs::handshake::HandshakePayload::ClientHello(ch) => ch,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}

impl Debug for Accepted {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Accepted").finish()
    }
}

#[cfg(feature = "std")]
struct Accepting;

#[cfg(feature = "std")]
impl State<ServerConnectionData> for Accepting {
    fn handle<'m>(
        self: Box<Self>,
        _cx: &mut hs::ServerContext<'_>,
        _m: Message<'m>,
    ) -> Result<Box<dyn State<ServerConnectionData> + 'm>, Error>
    where
        Self: 'm,
    {
        Err(Error::General("unreachable state".into()))
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        self
    }
}

pub(super) enum EarlyDataState {
    New,
    Accepted {
        received: ChunkVecBuffer,
        left: usize,
    },
    Rejected,
}

impl Default for EarlyDataState {
    fn default() -> Self {
        Self::New
    }
}

impl EarlyDataState {
    pub(super) fn reject(&mut self) {
        *self = Self::Rejected;
    }

    pub(super) fn accept(&mut self, max_size: usize) {
        *self = Self::Accepted {
            received: ChunkVecBuffer::new(Some(max_size)),
            left: max_size,
        };
    }

    #[cfg(feature = "std")]
    fn was_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }

    pub(super) fn was_rejected(&self) -> bool {
        matches!(self, Self::Rejected)
    }

    fn peek(&self) -> Option<&[u8]> {
        match self {
            Self::Accepted { received, .. } => received.peek(),
            _ => None,
        }
    }

    fn pop(&mut self) -> Option<Vec<u8>> {
        match self {
            Self::Accepted { received, .. } => received.pop(),
            _ => None,
        }
    }

    #[cfg(feature = "std")]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Accepted { received, .. } => received.read(buf),
            _ => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
        }
    }

    #[cfg(read_buf)]
    fn read_buf(&mut self, cursor: core::io::BorrowedCursor<'_>) -> io::Result<()> {
        match self {
            Self::Accepted { received, .. } => received.read_buf(cursor),
            _ => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
        }
    }

    pub(super) fn take_received_plaintext(&mut self, bytes: Payload<'_>) -> bool {
        let available = bytes.bytes().len();
        let Self::Accepted { received, left } = self else {
            return false;
        };

        if received.apply_limit(available) != available || available > *left {
            return false;
        }

        received.append(bytes.into_vec());
        *left -= available;
        true
    }
}

impl Debug for EarlyDataState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::New => write!(f, "EarlyDataState::New"),
            Self::Accepted { received, left } => write!(
                f,
                "EarlyDataState::Accepted {{ received: {}, left: {} }}",
                received.len(),
                left
            ),
            Self::Rejected => write!(f, "EarlyDataState::Rejected"),
        }
    }
}

impl ConnectionCore<ServerConnectionData> {
    pub(crate) fn for_server(
        config: Arc<ServerConfig>,
        extra_exts: ServerExtensionsInput<'static>,
    ) -> Result<Self, Error> {
        let mut common = CommonState::new(Side::Server);
        common.set_max_fragment_size(config.max_fragment_size)?;
        common.enable_secret_extraction = config.enable_secret_extraction;
        common.fips = config.fips();
        Ok(Self::new(
            Box::new(hs::ExpectClientHello::new(config, extra_exts)),
            ServerConnectionData::default(),
            common,
        ))
    }

    #[cfg(feature = "std")]
    pub(crate) fn reject_early_data(&mut self) {
        assert!(
            self.common_state.is_handshaking(),
            "cannot retroactively reject early data"
        );
        self.data.early_data.reject();
    }

    #[cfg(feature = "std")]
    pub(crate) fn get_sni_str(&self) -> Option<&str> {
        self.data.get_sni_str()
    }
}

/// State associated with a server connection.
#[derive(Default, Debug)]
pub struct ServerConnectionData {
    pub(super) sni: Option<DnsName<'static>>,
    pub(super) received_resumption_data: Option<Vec<u8>>,
    pub(super) resumption_data: Vec<u8>,
    pub(super) early_data: EarlyDataState,
}

impl ServerConnectionData {
    #[cfg(feature = "std")]
    pub(super) fn get_sni_str(&self) -> Option<&str> {
        self.sni.as_ref().map(AsRef::as_ref)
    }
}

impl crate::conn::SideData for ServerConnectionData {}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::format;

    use super::*;

    // these branches not reachable externally, unless something else goes wrong.
    #[test]
    fn test_read_in_new_state() {
        assert_eq!(
            format!("{:?}", EarlyDataState::default().read(&mut [0u8; 5])),
            "Err(Kind(BrokenPipe))"
        );
    }

    #[cfg(read_buf)]
    #[test]
    fn test_read_buf_in_new_state() {
        use core::io::BorrowedBuf;

        let mut buf = [0u8; 5];
        let mut buf: BorrowedBuf<'_> = buf.as_mut_slice().into();
        assert_eq!(
            format!("{:?}", EarlyDataState::default().read_buf(buf.unfilled())),
            "Err(Kind(BrokenPipe))"
        );
    }
}
