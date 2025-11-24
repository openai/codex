use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::{fmt, mem};

use pki_types::{ServerName, UnixTime};

use super::handy::NoClientSessionStorage;
use super::hs::{self, ClientHelloInput};
#[cfg(feature = "std")]
use crate::WantsVerifier;
use crate::builder::ConfigBuilder;
use crate::client::{EchMode, EchStatus};
use crate::common_state::{CommonState, Protocol, Side};
use crate::conn::{ConnectionCore, UnbufferedConnectionCommon};
use crate::crypto::{CryptoProvider, SupportedKxGroup};
use crate::enums::{CipherSuite, ProtocolVersion, SignatureScheme};
use crate::error::Error;
use crate::kernel::KernelConnection;
use crate::log::trace;
use crate::msgs::enums::NamedGroup;
use crate::msgs::handshake::ClientExtensionsInput;
use crate::msgs::persist;
use crate::suites::{ExtractedSecrets, SupportedCipherSuite};
use crate::sync::Arc;
#[cfg(feature = "std")]
use crate::time_provider::DefaultTimeProvider;
use crate::time_provider::TimeProvider;
use crate::unbuffered::{EncryptError, TransmitTlsData};
#[cfg(doc)]
use crate::{DistinguishedName, crypto};
use crate::{KeyLog, WantsVersions, compress, sign, verify, versions};

/// A trait for the ability to store client session data, so that sessions
/// can be resumed in future connections.
///
/// Generally all data in this interface should be treated as
/// **highly sensitive**, containing enough key material to break all security
/// of the corresponding session.
///
/// `set_`, `insert_`, `remove_` and `take_` operations are mutating; this isn't
/// expressed in the type system to allow implementations freedom in
/// how to achieve interior mutability.  `Mutex` is a common choice.
pub trait ClientSessionStore: fmt::Debug + Send + Sync {
    /// Remember what `NamedGroup` the given server chose.
    fn set_kx_hint(&self, server_name: ServerName<'static>, group: NamedGroup);

    /// This should return the value most recently passed to `set_kx_hint`
    /// for the given `server_name`.
    ///
    /// If `None` is returned, the caller chooses the first configured group,
    /// and an extra round trip might happen if that choice is unsatisfactory
    /// to the server.
    fn kx_hint(&self, server_name: &ServerName<'_>) -> Option<NamedGroup>;

    /// Remember a TLS1.2 session.
    ///
    /// At most one of these can be remembered at a time, per `server_name`.
    fn set_tls12_session(
        &self,
        server_name: ServerName<'static>,
        value: persist::Tls12ClientSessionValue,
    );

    /// Get the most recently saved TLS1.2 session for `server_name` provided to `set_tls12_session`.
    fn tls12_session(
        &self,
        server_name: &ServerName<'_>,
    ) -> Option<persist::Tls12ClientSessionValue>;

    /// Remove and forget any saved TLS1.2 session for `server_name`.
    fn remove_tls12_session(&self, server_name: &ServerName<'static>);

    /// Remember a TLS1.3 ticket that might be retrieved later from `take_tls13_ticket`, allowing
    /// resumption of this session.
    ///
    /// This can be called multiple times for a given session, allowing multiple independent tickets
    /// to be valid at once.  The number of times this is called is controlled by the server, so
    /// implementations of this trait should apply a reasonable bound of how many items are stored
    /// simultaneously.
    fn insert_tls13_ticket(
        &self,
        server_name: ServerName<'static>,
        value: persist::Tls13ClientSessionValue,
    );

    /// Return a TLS1.3 ticket previously provided to `add_tls13_ticket`.
    ///
    /// Implementations of this trait must return each value provided to `add_tls13_ticket` _at most once_.
    fn take_tls13_ticket(
        &self,
        server_name: &ServerName<'static>,
    ) -> Option<persist::Tls13ClientSessionValue>;
}

/// A trait for the ability to choose a certificate chain and
/// private key for the purposes of client authentication.
pub trait ResolvesClientCert: fmt::Debug + Send + Sync {
    /// Resolve a client certificate chain/private key to use as the client's
    /// identity.
    ///
    /// `root_hint_subjects` is an optional list of certificate authority
    /// subject distinguished names that the client can use to help
    /// decide on a client certificate the server is likely to accept. If
    /// the list is empty, the client should send whatever certificate it
    /// has. The hints are expected to be DER-encoded X.500 distinguished names,
    /// per [RFC 5280 A.1]. See [`DistinguishedName`] for more information
    /// on decoding with external crates like `x509-parser`.
    ///
    /// `sigschemes` is the list of the [`SignatureScheme`]s the server
    /// supports.
    ///
    /// Return `None` to continue the handshake without any client
    /// authentication.  The server may reject the handshake later
    /// if it requires authentication.
    ///
    /// [RFC 5280 A.1]: https://www.rfc-editor.org/rfc/rfc5280#appendix-A.1
    fn resolve(
        &self,
        root_hint_subjects: &[&[u8]],
        sigschemes: &[SignatureScheme],
    ) -> Option<Arc<sign::CertifiedKey>>;

    /// Return true if the client only supports raw public keys.
    ///
    /// See [RFC 7250](https://www.rfc-editor.org/rfc/rfc7250).
    fn only_raw_public_keys(&self) -> bool {
        false
    }

    /// Return true if any certificates at all are available.
    fn has_certs(&self) -> bool;
}

/// Common configuration for (typically) all connections made by a program.
///
/// Making one of these is cheap, though one of the inputs may be expensive: gathering trust roots
/// from the operating system to add to the [`RootCertStore`] passed to `with_root_certificates()`
/// (the rustls-native-certs crate is often used for this) may take on the order of a few hundred
/// milliseconds.
///
/// These must be created via the [`ClientConfig::builder()`] or [`ClientConfig::builder_with_provider()`]
/// function.
///
/// Note that using [`ConfigBuilder<ClientConfig, WantsVersions>::with_ech()`] will produce a common
/// configuration specific to the provided [`crate::client::EchConfig`] that may not be appropriate
/// for all connections made by the program. In this case the configuration should only be shared
/// by connections intended for domains that offer the provided [`crate::client::EchConfig`] in
/// their DNS zone.
///
/// # Defaults
///
/// * [`ClientConfig::max_fragment_size`]: the default is `None` (meaning 16kB).
/// * [`ClientConfig::resumption`]: supports resumption with up to 256 server names, using session
///   ids or tickets, with a max of eight tickets per server.
/// * [`ClientConfig::alpn_protocols`]: the default is empty -- no ALPN protocol is negotiated.
/// * [`ClientConfig::key_log`]: key material is not logged.
/// * [`ClientConfig::cert_decompressors`]: depends on the crate features, see [`compress::default_cert_decompressors()`].
/// * [`ClientConfig::cert_compressors`]: depends on the crate features, see [`compress::default_cert_compressors()`].
/// * [`ClientConfig::cert_compression_cache`]: caches the most recently used 4 compressions
///
/// [`RootCertStore`]: crate::RootCertStore
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Which ALPN protocols we include in our client hello.
    /// If empty, no ALPN extension is sent.
    pub alpn_protocols: Vec<Vec<u8>>,

    /// How and when the client can resume a previous session.
    ///
    /// # Sharing `resumption` between `ClientConfig`s
    /// In a program using many `ClientConfig`s it may improve resumption rates
    /// (which has a significant impact on connection performance) if those
    /// configs share a single `Resumption`.
    ///
    /// However, resumption is only allowed between two `ClientConfig`s if their
    /// `client_auth_cert_resolver` (ie, potential client authentication credentials)
    /// and `verifier` (ie, server certificate verification settings) are
    /// the same (according to `Arc::ptr_eq`).
    ///
    /// To illustrate, imagine two `ClientConfig`s `A` and `B`.  `A` fully validates
    /// the server certificate, `B` does not.  If `A` and `B` shared a resumption store,
    /// it would be possible for a session originated by `B` to be inserted into the
    /// store, and then resumed by `A`.  This would give a false impression to the user
    /// of `A` that the server certificate is fully validated.
    pub resumption: Resumption,

    /// The maximum size of plaintext input to be emitted in a single TLS record.
    /// A value of None is equivalent to the [TLS maximum] of 16 kB.
    ///
    /// rustls enforces an arbitrary minimum of 32 bytes for this field.
    /// Out of range values are reported as errors from [ClientConnection::new].
    ///
    /// Setting this value to a little less than the TCP MSS may improve latency
    /// for stream-y workloads.
    ///
    /// [TLS maximum]: https://datatracker.ietf.org/doc/html/rfc8446#section-5.1
    /// [ClientConnection::new]: crate::client::ClientConnection::new
    pub max_fragment_size: Option<usize>,

    /// How to decide what client auth certificate/keys to use.
    pub client_auth_cert_resolver: Arc<dyn ResolvesClientCert>,

    /// Whether to send the Server Name Indication (SNI) extension
    /// during the client handshake.
    ///
    /// The default is true.
    pub enable_sni: bool,

    /// How to output key material for debugging.  The default
    /// does nothing.
    pub key_log: Arc<dyn KeyLog>,

    /// Allows traffic secrets to be extracted after the handshake,
    /// e.g. for kTLS setup.
    pub enable_secret_extraction: bool,

    /// Whether to send data on the first flight ("early data") in
    /// TLS 1.3 handshakes.
    ///
    /// The default is false.
    pub enable_early_data: bool,

    /// If set to `true`, requires the server to support the extended
    /// master secret extraction method defined in [RFC 7627].
    ///
    /// The default is `true` if the `fips` crate feature is enabled,
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

    /// Source of randomness and other crypto.
    pub(super) provider: Arc<CryptoProvider>,

    /// Supported versions, in no particular order.  The default
    /// is all supported versions.
    pub(super) versions: versions::EnabledVersions,

    /// How to verify the server certificate chain.
    pub(super) verifier: Arc<dyn verify::ServerCertVerifier>,

    /// How to decompress the server's certificate chain.
    ///
    /// If this is non-empty, the [RFC8779] certificate compression
    /// extension is offered, and any compressed certificates are
    /// transparently decompressed during the handshake.
    ///
    /// This only applies to TLS1.3 connections.  It is ignored for
    /// TLS1.2 connections.
    ///
    /// [RFC8779]: https://datatracker.ietf.org/doc/rfc8879/
    pub cert_decompressors: Vec<&'static dyn compress::CertDecompressor>,

    /// How to compress the client's certificate chain.
    ///
    /// If a server supports this extension, and advertises support
    /// for one of the compression algorithms included here, the
    /// client certificate will be compressed according to [RFC8779].
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

    /// How to offer Encrypted Client Hello (ECH). The default is to not offer ECH.
    pub(super) ech_mode: Option<EchMode>,
}

impl ClientConfig {
    /// Create a builder for a client configuration with
    /// [the process-default `CryptoProvider`][CryptoProvider#using-the-per-process-default-cryptoprovider]
    /// and safe protocol version defaults.
    ///
    /// For more information, see the [`ConfigBuilder`] documentation.
    #[cfg(feature = "std")]
    pub fn builder() -> ConfigBuilder<Self, WantsVerifier> {
        Self::builder_with_protocol_versions(versions::DEFAULT_VERSIONS)
    }

    /// Create a builder for a client configuration with
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

    /// Create a builder for a client configuration with a specific [`CryptoProvider`].
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
    /// Create a builder for a client configuration with no default implementation details.
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

    /// Return true if connections made with this `ClientConfig` will
    /// operate in FIPS mode.
    ///
    /// This is different from [`CryptoProvider::fips()`]: [`CryptoProvider::fips()`]
    /// is concerned only with cryptography, whereas this _also_ covers TLS-level
    /// configuration that NIST recommends, as well as ECH HPKE suites if applicable.
    pub fn fips(&self) -> bool {
        let mut is_fips = self.provider.fips();

        #[cfg(feature = "tls12")]
        {
            is_fips = is_fips && self.require_ems
        }

        if let Some(ech_mode) = &self.ech_mode {
            is_fips = is_fips && ech_mode.fips();
        }

        is_fips
    }

    /// Return the crypto provider used to construct this client configuration.
    pub fn crypto_provider(&self) -> &Arc<CryptoProvider> {
        &self.provider
    }

    /// Access configuration options whose use is dangerous and requires
    /// extra care.
    pub fn dangerous(&mut self) -> danger::DangerousClientConfig<'_> {
        danger::DangerousClientConfig { cfg: self }
    }

    pub(super) fn needs_key_share(&self) -> bool {
        self.supports_version(ProtocolVersion::TLSv1_3)
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

    pub(super) fn find_cipher_suite(&self, suite: CipherSuite) -> Option<SupportedCipherSuite> {
        self.provider
            .cipher_suites
            .iter()
            .copied()
            .find(|&scs| scs.suite() == suite)
    }

    pub(super) fn find_kx_group(
        &self,
        group: NamedGroup,
        version: ProtocolVersion,
    ) -> Option<&'static dyn SupportedKxGroup> {
        self.provider
            .kx_groups
            .iter()
            .copied()
            .find(|skxg| skxg.usable_for_version(version) && skxg.name() == group)
    }

    pub(super) fn current_time(&self) -> Result<UnixTime, Error> {
        self.time_provider
            .current_time()
            .ok_or(Error::FailedToGetCurrentTime)
    }
}

/// Configuration for how/when a client is allowed to resume a previous session.
#[derive(Clone, Debug)]
pub struct Resumption {
    /// How we store session data or tickets. The default is to use an in-memory
    /// [super::handy::ClientSessionMemoryCache].
    pub(super) store: Arc<dyn ClientSessionStore>,

    /// What mechanism is used for resuming a TLS 1.2 session.
    pub(super) tls12_resumption: Tls12Resumption,
}

impl Resumption {
    /// Create a new `Resumption` that stores data for the given number of sessions in memory.
    ///
    /// This is the default `Resumption` choice, and enables resuming a TLS 1.2 session with
    /// a session id or RFC 5077 ticket.
    #[cfg(feature = "std")]
    pub fn in_memory_sessions(num: usize) -> Self {
        Self {
            store: Arc::new(super::handy::ClientSessionMemoryCache::new(num)),
            tls12_resumption: Tls12Resumption::SessionIdOrTickets,
        }
    }

    /// Use a custom [`ClientSessionStore`] implementation to store sessions.
    ///
    /// By default, enables resuming a TLS 1.2 session with a session id or RFC 5077 ticket.
    pub fn store(store: Arc<dyn ClientSessionStore>) -> Self {
        Self {
            store,
            tls12_resumption: Tls12Resumption::SessionIdOrTickets,
        }
    }

    /// Disable all use of session resumption.
    pub fn disabled() -> Self {
        Self {
            store: Arc::new(NoClientSessionStorage),
            tls12_resumption: Tls12Resumption::Disabled,
        }
    }

    /// Configure whether TLS 1.2 sessions may be resumed, and by what mechanism.
    ///
    /// This is meaningless if you've disabled resumption entirely, which is the case in `no-std`
    /// contexts.
    pub fn tls12_resumption(mut self, tls12: Tls12Resumption) -> Self {
        self.tls12_resumption = tls12;
        self
    }
}

impl Default for Resumption {
    /// Create an in-memory session store resumption with up to 256 server names, allowing
    /// a TLS 1.2 session to resume with a session id or RFC 5077 ticket.
    fn default() -> Self {
        #[cfg(feature = "std")]
        let ret = Self::in_memory_sessions(256);

        #[cfg(not(feature = "std"))]
        let ret = Self::disabled();

        ret
    }
}

/// What mechanisms to support for resuming a TLS 1.2 session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tls12Resumption {
    /// Disable 1.2 resumption.
    Disabled,
    /// Support 1.2 resumption using session ids only.
    SessionIdOnly,
    /// Support 1.2 resumption using session ids or RFC 5077 tickets.
    ///
    /// See[^1] for why you might like to disable RFC 5077 by instead choosing the `SessionIdOnly`
    /// option. Note that TLS 1.3 tickets do not have those issues.
    ///
    /// [^1]: <https://words.filippo.io/we-need-to-talk-about-session-tickets/>
    SessionIdOrTickets,
}

/// Container for unsafe APIs
pub(super) mod danger {
    use super::ClientConfig;
    use super::verify::ServerCertVerifier;
    use crate::sync::Arc;

    /// Accessor for dangerous configuration options.
    #[derive(Debug)]
    pub struct DangerousClientConfig<'a> {
        /// The underlying ClientConfig
        pub cfg: &'a mut ClientConfig,
    }

    impl DangerousClientConfig<'_> {
        /// Overrides the default `ServerCertVerifier` with something else.
        pub fn set_certificate_verifier(&mut self, verifier: Arc<dyn ServerCertVerifier>) {
            self.cfg.verifier = verifier;
        }
    }
}

#[derive(Debug, PartialEq)]
enum EarlyDataState {
    Disabled,
    Ready,
    Accepted,
    AcceptedFinished,
    Rejected,
}

#[derive(Debug)]
pub(super) struct EarlyData {
    state: EarlyDataState,
    left: usize,
}

impl EarlyData {
    fn new() -> Self {
        Self {
            left: 0,
            state: EarlyDataState::Disabled,
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        matches!(self.state, EarlyDataState::Ready | EarlyDataState::Accepted)
    }

    #[cfg(feature = "std")]
    fn is_accepted(&self) -> bool {
        matches!(
            self.state,
            EarlyDataState::Accepted | EarlyDataState::AcceptedFinished
        )
    }

    pub(super) fn enable(&mut self, max_data: usize) {
        assert_eq!(self.state, EarlyDataState::Disabled);
        self.state = EarlyDataState::Ready;
        self.left = max_data;
    }

    pub(super) fn rejected(&mut self) {
        trace!("EarlyData rejected");
        self.state = EarlyDataState::Rejected;
    }

    pub(super) fn accepted(&mut self) {
        trace!("EarlyData accepted");
        assert_eq!(self.state, EarlyDataState::Ready);
        self.state = EarlyDataState::Accepted;
    }

    pub(super) fn finished(&mut self) {
        trace!("EarlyData finished");
        self.state = match self.state {
            EarlyDataState::Accepted => EarlyDataState::AcceptedFinished,
            _ => panic!("bad EarlyData state"),
        }
    }

    fn check_write_opt(&mut self, sz: usize) -> Option<usize> {
        match self.state {
            EarlyDataState::Disabled => unreachable!(),
            EarlyDataState::Ready | EarlyDataState::Accepted => {
                let take = if self.left < sz {
                    mem::replace(&mut self.left, 0)
                } else {
                    self.left -= sz;
                    sz
                };

                Some(take)
            }
            EarlyDataState::Rejected | EarlyDataState::AcceptedFinished => None,
        }
    }
}

#[cfg(feature = "std")]
mod connection {
    use alloc::vec::Vec;
    use core::fmt;
    use core::ops::{Deref, DerefMut};
    use std::io;

    use pki_types::ServerName;

    use super::{ClientConnectionData, ClientExtensionsInput};
    use crate::ClientConfig;
    use crate::client::EchStatus;
    use crate::common_state::Protocol;
    use crate::conn::{ConnectionCommon, ConnectionCore};
    use crate::error::Error;
    use crate::suites::ExtractedSecrets;
    use crate::sync::Arc;

    /// Stub that implements io::Write and dispatches to `write_early_data`.
    pub struct WriteEarlyData<'a> {
        sess: &'a mut ClientConnection,
    }

    impl<'a> WriteEarlyData<'a> {
        fn new(sess: &'a mut ClientConnection) -> Self {
            WriteEarlyData { sess }
        }

        /// How many bytes you may send.  Writes will become short
        /// once this reaches zero.
        pub fn bytes_left(&self) -> usize {
            self.sess
                .inner
                .core
                .data
                .early_data
                .bytes_left()
        }
    }

    impl io::Write for WriteEarlyData<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sess.write_early_data(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl super::EarlyData {
        fn check_write(&mut self, sz: usize) -> io::Result<usize> {
            self.check_write_opt(sz)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))
        }

        fn bytes_left(&self) -> usize {
            self.left
        }
    }

    /// This represents a single TLS client connection.
    pub struct ClientConnection {
        inner: ConnectionCommon<ClientConnectionData>,
    }

    impl fmt::Debug for ClientConnection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ClientConnection")
                .finish()
        }
    }

    impl ClientConnection {
        /// Make a new ClientConnection.  `config` controls how
        /// we behave in the TLS protocol, `name` is the
        /// name of the server we want to talk to.
        pub fn new(config: Arc<ClientConfig>, name: ServerName<'static>) -> Result<Self, Error> {
            Self::new_with_alpn(config.clone(), name, config.alpn_protocols.clone())
        }

        /// Make a new ClientConnection with custom ALPN protocols.
        pub fn new_with_alpn(
            config: Arc<ClientConfig>,
            name: ServerName<'static>,
            alpn_protocols: Vec<Vec<u8>>,
        ) -> Result<Self, Error> {
            Ok(Self {
                inner: ConnectionCommon::from(ConnectionCore::for_client(
                    config,
                    name,
                    ClientExtensionsInput::from_alpn(alpn_protocols),
                    Protocol::Tcp,
                )?),
            })
        }
        /// Returns an `io::Write` implementer you can write bytes to
        /// to send TLS1.3 early data (a.k.a. "0-RTT data") to the server.
        ///
        /// This returns None in many circumstances when the capability to
        /// send early data is not available, including but not limited to:
        ///
        /// - The server hasn't been talked to previously.
        /// - The server does not support resumption.
        /// - The server does not support early data.
        /// - The resumption data for the server has expired.
        ///
        /// The server specifies a maximum amount of early data.  You can
        /// learn this limit through the returned object, and writes through
        /// it will process only this many bytes.
        ///
        /// The server can choose not to accept any sent early data --
        /// in this case the data is lost but the connection continues.  You
        /// can tell this happened using `is_early_data_accepted`.
        pub fn early_data(&mut self) -> Option<WriteEarlyData<'_>> {
            if self
                .inner
                .core
                .data
                .early_data
                .is_enabled()
            {
                Some(WriteEarlyData::new(self))
            } else {
                None
            }
        }

        /// Returns True if the server signalled it will process early data.
        ///
        /// If you sent early data and this returns false at the end of the
        /// handshake then the server will not process the data.  This
        /// is not an error, but you may wish to resend the data.
        pub fn is_early_data_accepted(&self) -> bool {
            self.inner.core.is_early_data_accepted()
        }

        /// Extract secrets, so they can be used when configuring kTLS, for example.
        /// Should be used with care as it exposes secret key material.
        pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
            self.inner.dangerous_extract_secrets()
        }

        /// Return the connection's Encrypted Client Hello (ECH) status.
        pub fn ech_status(&self) -> EchStatus {
            self.inner.core.data.ech_status
        }

        /// Returns the number of TLS1.3 tickets that have been received.
        pub fn tls13_tickets_received(&self) -> u32 {
            self.inner.tls13_tickets_received
        }

        /// Return true if the connection was made with a `ClientConfig` that is FIPS compatible.
        ///
        /// This is different from [`crate::crypto::CryptoProvider::fips()`]:
        /// it is concerned only with cryptography, whereas this _also_ covers TLS-level
        /// configuration that NIST recommends, as well as ECH HPKE suites if applicable.
        pub fn fips(&self) -> bool {
            self.inner.core.common_state.fips
        }

        fn write_early_data(&mut self, data: &[u8]) -> io::Result<usize> {
            self.inner
                .core
                .data
                .early_data
                .check_write(data.len())
                .map(|sz| {
                    self.inner
                        .send_early_plaintext(&data[..sz])
                })
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

    #[doc(hidden)]
    impl<'a> TryFrom<&'a mut crate::Connection> for &'a mut ClientConnection {
        type Error = ();

        fn try_from(value: &'a mut crate::Connection) -> Result<Self, Self::Error> {
            use crate::Connection::*;
            match value {
                Client(conn) => Ok(conn),
                Server(_) => Err(()),
            }
        }
    }

    impl From<ClientConnection> for crate::Connection {
        fn from(conn: ClientConnection) -> Self {
            Self::Client(conn)
        }
    }
}
#[cfg(feature = "std")]
pub use connection::{ClientConnection, WriteEarlyData};

impl ConnectionCore<ClientConnectionData> {
    pub(crate) fn for_client(
        config: Arc<ClientConfig>,
        name: ServerName<'static>,
        extra_exts: ClientExtensionsInput<'static>,
        proto: Protocol,
    ) -> Result<Self, Error> {
        let mut common_state = CommonState::new(Side::Client);
        common_state.set_max_fragment_size(config.max_fragment_size)?;
        common_state.protocol = proto;
        common_state.enable_secret_extraction = config.enable_secret_extraction;
        common_state.fips = config.fips();
        let mut data = ClientConnectionData::new();

        let mut cx = hs::ClientContext {
            common: &mut common_state,
            data: &mut data,
            // `start_handshake` won't produce plaintext
            sendable_plaintext: None,
        };

        let input = ClientHelloInput::new(name, &extra_exts, &mut cx, config)?;
        let state = input.start_handshake(extra_exts, &mut cx)?;
        Ok(Self::new(state, data, common_state))
    }

    #[cfg(feature = "std")]
    pub(crate) fn is_early_data_accepted(&self) -> bool {
        self.data.early_data.is_accepted()
    }
}

/// Unbuffered version of `ClientConnection`
///
/// See the [`crate::unbuffered`] module docs for more details
pub struct UnbufferedClientConnection {
    inner: UnbufferedConnectionCommon<ClientConnectionData>,
}

impl UnbufferedClientConnection {
    /// Make a new ClientConnection. `config` controls how we behave in the TLS protocol, `name` is
    /// the name of the server we want to talk to.
    pub fn new(config: Arc<ClientConfig>, name: ServerName<'static>) -> Result<Self, Error> {
        Self::new_with_extensions(
            config.clone(),
            name,
            ClientExtensionsInput::from_alpn(config.alpn_protocols.clone()),
        )
    }

    /// Make a new UnbufferedClientConnection with custom ALPN protocols.
    pub fn new_with_alpn(
        config: Arc<ClientConfig>,
        name: ServerName<'static>,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<Self, Error> {
        Self::new_with_extensions(
            config,
            name,
            ClientExtensionsInput::from_alpn(alpn_protocols),
        )
    }

    fn new_with_extensions(
        config: Arc<ClientConfig>,
        name: ServerName<'static>,
        extensions: ClientExtensionsInput<'static>,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner: UnbufferedConnectionCommon::from(ConnectionCore::for_client(
                config,
                name,
                extensions,
                Protocol::Tcp,
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

    /// Extract secrets and a [`KernelConnection`] object.
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
    ) -> Result<(ExtractedSecrets, KernelConnection<ClientConnectionData>), Error> {
        self.inner
            .core
            .dangerous_into_kernel_connection()
    }

    /// Returns the number of TLS1.3 tickets that have been received.
    pub fn tls13_tickets_received(&self) -> u32 {
        self.inner.tls13_tickets_received
    }
}

impl Deref for UnbufferedClientConnection {
    type Target = UnbufferedConnectionCommon<ClientConnectionData>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for UnbufferedClientConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl TransmitTlsData<'_, ClientConnectionData> {
    /// returns an adapter that allows encrypting early (RTT-0) data before transmitting the
    /// already encoded TLS data
    ///
    /// IF allowed by the protocol
    pub fn may_encrypt_early_data(&mut self) -> Option<MayEncryptEarlyData<'_>> {
        if self
            .conn
            .core
            .data
            .early_data
            .is_enabled()
        {
            Some(MayEncryptEarlyData { conn: self.conn })
        } else {
            None
        }
    }
}

/// Allows encrypting early (RTT-0) data
pub struct MayEncryptEarlyData<'c> {
    conn: &'c mut UnbufferedConnectionCommon<ClientConnectionData>,
}

impl MayEncryptEarlyData<'_> {
    /// Encrypts `application_data` into the `outgoing_tls` buffer
    ///
    /// returns the number of bytes that were written into `outgoing_tls`, or an error if
    /// the provided buffer was too small. In the error case, `outgoing_tls` is not modified
    pub fn encrypt(
        &mut self,
        early_data: &[u8],
        outgoing_tls: &mut [u8],
    ) -> Result<usize, EarlyDataError> {
        let Some(allowed) = self
            .conn
            .core
            .data
            .early_data
            .check_write_opt(early_data.len())
        else {
            return Err(EarlyDataError::ExceededAllowedEarlyData);
        };

        self.conn
            .core
            .common_state
            .write_plaintext(early_data[..allowed].into(), outgoing_tls)
            .map_err(|e| e.into())
    }
}

/// Errors that may arise when encrypting early (RTT-0) data
#[derive(Debug)]
pub enum EarlyDataError {
    /// Cannot encrypt more early data due to imposed limits
    ExceededAllowedEarlyData,
    /// Encryption error
    Encrypt(EncryptError),
}

impl From<EncryptError> for EarlyDataError {
    fn from(v: EncryptError) -> Self {
        Self::Encrypt(v)
    }
}

impl fmt::Display for EarlyDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExceededAllowedEarlyData => f.write_str("cannot send any more early data"),
            Self::Encrypt(e) => fmt::Display::fmt(e, f),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EarlyDataError {}

/// State associated with a client connection.
#[derive(Debug)]
pub struct ClientConnectionData {
    pub(super) early_data: EarlyData,
    pub(super) ech_status: EchStatus,
}

impl ClientConnectionData {
    fn new() -> Self {
        Self {
            early_data: EarlyData::new(),
            ech_status: EchStatus::NotOffered,
        }
    }
}

impl crate::conn::SideData for ClientConnectionData {}
