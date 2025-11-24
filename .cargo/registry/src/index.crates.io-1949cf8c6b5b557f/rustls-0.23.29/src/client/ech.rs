use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use pki_types::{DnsName, EchConfigListBytes, ServerName};
use subtle::ConstantTimeEq;

use crate::CipherSuite::TLS_EMPTY_RENEGOTIATION_INFO_SCSV;
use crate::client::tls13;
use crate::crypto::SecureRandom;
use crate::crypto::hash::Hash;
use crate::crypto::hpke::{EncapsulatedSecret, Hpke, HpkePublicKey, HpkeSealer, HpkeSuite};
use crate::hash_hs::{HandshakeHash, HandshakeHashBuffer};
use crate::log::{debug, trace, warn};
use crate::msgs::base::{Payload, PayloadU16};
use crate::msgs::codec::{Codec, Reader};
use crate::msgs::enums::{ExtensionType, HpkeKem};
use crate::msgs::handshake::{
    ClientExtensions, ClientHelloPayload, EchConfigContents, EchConfigPayload, Encoding,
    EncryptedClientHello, EncryptedClientHelloOuter, HandshakeMessagePayload, HandshakePayload,
    HelloRetryRequest, HpkeKeyConfig, HpkeSymmetricCipherSuite, PresharedKeyBinder,
    PresharedKeyOffer, Random, ServerHelloPayload, ServerNamePayload,
};
use crate::msgs::message::{Message, MessagePayload};
use crate::msgs::persist;
use crate::msgs::persist::Retrieved;
use crate::tls13::key_schedule::{
    KeyScheduleEarly, KeyScheduleHandshakeStart, server_ech_hrr_confirmation_secret,
};
use crate::{
    AlertDescription, ClientConfig, CommonState, EncryptedClientHelloError, Error,
    PeerIncompatible, PeerMisbehaved, ProtocolVersion, Tls13CipherSuite,
};

/// Controls how Encrypted Client Hello (ECH) is used in a client handshake.
#[derive(Clone, Debug)]
pub enum EchMode {
    /// ECH is enabled and the ClientHello will be encrypted based on the provided
    /// configuration.
    Enable(EchConfig),

    /// No ECH configuration is available but the client should act as though it were.
    ///
    /// This is an anti-ossification measure, sometimes referred to as "GREASE"[^0].
    /// [^0]: <https://www.rfc-editor.org/rfc/rfc8701>
    Grease(EchGreaseConfig),
}

impl EchMode {
    /// Returns true if the ECH mode will use a FIPS approved HPKE suite.
    pub fn fips(&self) -> bool {
        match self {
            Self::Enable(ech_config) => ech_config.suite.fips(),
            Self::Grease(grease_config) => grease_config.suite.fips(),
        }
    }
}

impl From<EchConfig> for EchMode {
    fn from(config: EchConfig) -> Self {
        Self::Enable(config)
    }
}

impl From<EchGreaseConfig> for EchMode {
    fn from(config: EchGreaseConfig) -> Self {
        Self::Grease(config)
    }
}

/// Configuration for performing encrypted client hello.
///
/// Note: differs from the protocol-encoded EchConfig (`EchConfigMsg`).
#[derive(Clone, Debug)]
pub struct EchConfig {
    /// The selected EchConfig.
    pub(crate) config: EchConfigPayload,

    /// An HPKE instance corresponding to a suite from the `config` we have selected as
    /// a compatible choice.
    pub(crate) suite: &'static dyn Hpke,
}

impl EchConfig {
    /// Construct an EchConfig by selecting a ECH config from the provided bytes that is compatible
    /// with one of the given HPKE suites.
    ///
    /// The config list bytes should be sourced from a DNS-over-HTTPS lookup resolving the `HTTPS`
    /// resource record for the host name of the server you wish to connect via ECH,
    /// and extracting the ECH configuration from the `ech` parameter. The extracted bytes should
    /// be base64 decoded to yield the `EchConfigListBytes` you provide to rustls.
    ///
    /// One of the provided ECH configurations must be compatible with the HPKE provider's supported
    /// suites or an error will be returned.
    ///
    /// See the [`ech-client.rs`] example for a complete example of fetching ECH configs from DNS.
    ///
    /// [`ech-client.rs`]: https://github.com/rustls/rustls/blob/main/examples/src/bin/ech-client.rs
    pub fn new(
        ech_config_list: EchConfigListBytes<'_>,
        hpke_suites: &[&'static dyn Hpke],
    ) -> Result<Self, Error> {
        let ech_configs = Vec::<EchConfigPayload>::read(&mut Reader::init(&ech_config_list))
            .map_err(|_| {
                Error::InvalidEncryptedClientHello(EncryptedClientHelloError::InvalidConfigList)
            })?;

        // Note: we name the index var _i because if the log feature is disabled
        //       it is unused.
        #[cfg_attr(not(feature = "logging"), allow(clippy::unused_enumerate_index))]
        for (_i, config) in ech_configs.iter().enumerate() {
            let contents = match config {
                EchConfigPayload::V18(contents) => contents,
                EchConfigPayload::Unknown {
                    version: _version, ..
                } => {
                    warn!(
                        "ECH config {} has unsupported version {:?}",
                        _i + 1,
                        _version
                    );
                    continue; // Unsupported version.
                }
            };

            if contents.has_unknown_mandatory_extension() || contents.has_duplicate_extension() {
                warn!("ECH config has duplicate, or unknown mandatory extensions: {contents:?}",);
                continue; // Unsupported, or malformed extensions.
            }

            let key_config = &contents.key_config;
            for cipher_suite in &key_config.symmetric_cipher_suites {
                if cipher_suite.aead_id.tag_len().is_none() {
                    continue; // Unsupported EXPORT_ONLY AEAD cipher suite.
                }

                let suite = HpkeSuite {
                    kem: key_config.kem_id,
                    sym: *cipher_suite,
                };
                if let Some(hpke) = hpke_suites
                    .iter()
                    .find(|hpke| hpke.suite() == suite)
                {
                    debug!(
                        "selected ECH config ID {:?} suite {:?} public_name {:?}",
                        key_config.config_id, suite, contents.public_name
                    );
                    return Ok(Self {
                        config: config.clone(),
                        suite: *hpke,
                    });
                }
            }
        }

        Err(EncryptedClientHelloError::NoCompatibleConfig.into())
    }

    pub(super) fn state(
        &self,
        server_name: ServerName<'static>,
        config: &ClientConfig,
    ) -> Result<EchState, Error> {
        EchState::new(
            self,
            server_name.clone(),
            config
                .client_auth_cert_resolver
                .has_certs(),
            config.provider.secure_random,
            config.enable_sni,
        )
    }

    /// Compute the HPKE `SetupBaseS` `info` parameter for this ECH configuration.
    ///
    /// See <https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-17#section-6.1>.
    pub(crate) fn hpke_info(&self) -> Vec<u8> {
        let mut info = Vec::with_capacity(128);
        // "tls ech" || 0x00 || ECHConfig
        info.extend_from_slice(b"tls ech\0");
        self.config.encode(&mut info);
        info
    }
}

/// Configuration for GREASE Encrypted Client Hello.
#[derive(Clone, Debug)]
pub struct EchGreaseConfig {
    pub(crate) suite: &'static dyn Hpke,
    pub(crate) placeholder_key: HpkePublicKey,
}

impl EchGreaseConfig {
    /// Construct a GREASE ECH configuration.
    ///
    /// This configuration is used when the client wishes to offer ECH to prevent ossification,
    /// but doesn't have a real ECH configuration to use for the remote server. In this case
    /// a placeholder or "GREASE"[^0] extension is used.
    ///
    /// Returns an error if the HPKE provider does not support the given suite.
    ///
    /// [^0]: <https://www.rfc-editor.org/rfc/rfc8701>
    pub fn new(suite: &'static dyn Hpke, placeholder_key: HpkePublicKey) -> Self {
        Self {
            suite,
            placeholder_key,
        }
    }

    /// Build a GREASE ECH extension based on the placeholder configuration.
    ///
    /// See <https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#name-grease-ech> for
    /// more information.
    pub(crate) fn grease_ext(
        &self,
        secure_random: &'static dyn SecureRandom,
        inner_name: ServerName<'static>,
        outer_hello: &ClientHelloPayload,
    ) -> Result<EncryptedClientHello, Error> {
        trace!("Preparing GREASE ECH extension");

        // Pick a random config id.
        let mut config_id: [u8; 1] = [0; 1];
        secure_random.fill(&mut config_id[..])?;

        let suite = self.suite.suite();

        // Construct a dummy ECH state - we don't have a real ECH config from a server since
        // this is for GREASE.
        let mut grease_state = EchState::new(
            &EchConfig {
                config: EchConfigPayload::V18(EchConfigContents {
                    key_config: HpkeKeyConfig {
                        config_id: config_id[0],
                        kem_id: HpkeKem::DHKEM_P256_HKDF_SHA256,
                        public_key: PayloadU16::new(self.placeholder_key.0.clone()),
                        symmetric_cipher_suites: vec![suite.sym],
                    },
                    maximum_name_length: 0,
                    public_name: DnsName::try_from("filler").unwrap(),
                    extensions: Vec::default(),
                }),
                suite: self.suite,
            },
            inner_name,
            false,
            secure_random,
            false, // Does not matter if we enable/disable SNI here. Inner hello is not used.
        )?;

        // Construct an inner hello using the outer hello - this allows us to know the size of
        // dummy payload we should use for the GREASE extension.
        let encoded_inner_hello = grease_state.encode_inner_hello(outer_hello, None, &None);

        // Generate a payload of random data equivalent in length to a real inner hello.
        let payload_len = encoded_inner_hello.len()
            + suite
                .sym
                .aead_id
                .tag_len()
                // Safety: we have confirmed the AEAD is supported when building the config. All
                //  supported AEADs have a tag length.
                .unwrap();
        let mut payload = vec![0; payload_len];
        secure_random.fill(&mut payload)?;

        // Return the GREASE extension.
        Ok(EncryptedClientHello::Outer(EncryptedClientHelloOuter {
            cipher_suite: suite.sym,
            config_id: config_id[0],
            enc: PayloadU16::new(grease_state.enc.0),
            payload: PayloadU16::new(payload),
        }))
    }
}

/// An enum representing ECH offer status.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EchStatus {
    /// ECH was not offered - it is a normal TLS handshake.
    NotOffered,
    /// GREASE ECH was sent. This is not considered offering ECH.
    Grease,
    /// ECH was offered but we do not yet know whether the offer was accepted or rejected.
    Offered,
    /// ECH was offered and the server accepted.
    Accepted,
    /// ECH was offered and the server rejected.
    Rejected,
}

/// Contextual data for a TLS client handshake that has offered encrypted client hello (ECH).
pub(crate) struct EchState {
    // The public DNS name from the ECH configuration we've chosen - this is included as the SNI
    // value for the "outer" client hello. It can only be a DnsName, not an IP address.
    pub(crate) outer_name: DnsName<'static>,
    // If we're resuming in the inner hello, this is the early key schedule to use for encrypting
    // early data if the ECH offer is accepted.
    pub(crate) early_data_key_schedule: Option<KeyScheduleEarly>,
    // A random value we use for the inner hello.
    pub(crate) inner_hello_random: Random,
    // A transcript buffer maintained for the inner hello. Once ECH is confirmed we switch to
    // using this transcript for the handshake.
    pub(crate) inner_hello_transcript: HandshakeHashBuffer,
    // A source of secure random data.
    secure_random: &'static dyn SecureRandom,
    // An HPKE sealer context that can be used for encrypting ECH data.
    sender: Box<dyn HpkeSealer>,
    // The ID of the ECH configuration we've chosen - this is included in the outer ECH extension.
    config_id: u8,
    // The private server name we'll use for the inner protected hello.
    inner_name: ServerName<'static>,
    // The advertised maximum name length from the ECH configuration we've chosen - this is used
    // for padding calculations.
    maximum_name_length: u8,
    // A supported symmetric cipher suite from the ECH configuration we've chosen - this is
    // included in the outer ECH extension.
    cipher_suite: HpkeSymmetricCipherSuite,
    // A secret encapsulated to the public key of the remote server. This is included in the
    // outer ECH extension for non-retry outer hello messages.
    enc: EncapsulatedSecret,
    // Whether the inner client hello should contain a server name indication (SNI) extension.
    enable_sni: bool,
    // The extensions sent in the inner hello.
    sent_extensions: Vec<ExtensionType>,
}

impl EchState {
    pub(crate) fn new(
        config: &EchConfig,
        inner_name: ServerName<'static>,
        client_auth_enabled: bool,
        secure_random: &'static dyn SecureRandom,
        enable_sni: bool,
    ) -> Result<Self, Error> {
        let EchConfigPayload::V18(config_contents) = &config.config else {
            // the public EchConfig::new() constructor ensures we only have supported
            // configurations.
            unreachable!("ECH config version mismatch");
        };
        let key_config = &config_contents.key_config;

        // Encapsulate a secret for the server's public key, and set up a sender context
        // we can use to seal messages.
        let (enc, sender) = config.suite.setup_sealer(
            &config.hpke_info(),
            &HpkePublicKey(key_config.public_key.0.clone()),
        )?;

        // Start a new transcript buffer for the inner hello.
        let mut inner_hello_transcript = HandshakeHashBuffer::new();
        if client_auth_enabled {
            inner_hello_transcript.set_client_auth_enabled();
        }

        Ok(Self {
            secure_random,
            sender,
            config_id: key_config.config_id,
            inner_name,
            outer_name: config_contents.public_name.clone(),
            maximum_name_length: config_contents.maximum_name_length,
            cipher_suite: config.suite.suite().sym,
            enc,
            inner_hello_random: Random::new(secure_random)?,
            inner_hello_transcript,
            early_data_key_schedule: None,
            enable_sni,
            sent_extensions: Vec::new(),
        })
    }

    /// Construct a ClientHelloPayload offering ECH.
    ///
    /// An outer hello, with a protected inner hello for the `inner_name` will be returned, and the
    /// ECH context will be updated to reflect the inner hello that was offered.
    ///
    /// If `retry_req` is `Some`, then the outer hello will be constructed for a hello retry request.
    ///
    /// If `resuming` is `Some`, then the inner hello will be constructed for a resumption handshake.
    pub(crate) fn ech_hello(
        &mut self,
        mut outer_hello: ClientHelloPayload,
        retry_req: Option<&HelloRetryRequest>,
        resuming: &Option<Retrieved<&persist::Tls13ClientSessionValue>>,
    ) -> Result<ClientHelloPayload, Error> {
        trace!(
            "Preparing ECH offer {}",
            if retry_req.is_some() { "for retry" } else { "" }
        );

        // Construct the encoded inner hello and update the transcript.
        let encoded_inner_hello = self.encode_inner_hello(&outer_hello, retry_req, resuming);

        // Complete the ClientHelloOuterAAD with an ech extension, the payload should be a placeholder
        // of size L, all zeroes. L == length of encrypting encoded client hello inner w/ the selected
        // HPKE AEAD. (sum of plaintext + tag length, typically).
        let payload_len = encoded_inner_hello.len()
            + self
                .cipher_suite
                .aead_id
                .tag_len()
                // Safety: we've already verified this AEAD is supported when loading the config
                // that was used to create the ECH context. All supported AEADs have a tag length.
                .unwrap();

        // Outer hello's created in response to a hello retry request omit the enc value.
        let enc = match retry_req.is_some() {
            true => Vec::default(),
            false => self.enc.0.clone(),
        };

        fn outer_hello_ext(ctx: &EchState, enc: Vec<u8>, payload: Vec<u8>) -> EncryptedClientHello {
            EncryptedClientHello::Outer(EncryptedClientHelloOuter {
                cipher_suite: ctx.cipher_suite,
                config_id: ctx.config_id,
                enc: PayloadU16::new(enc),
                payload: PayloadU16::new(payload),
            })
        }

        // The outer handshake is not permitted to resume a session. If we're resuming in the
        // inner handshake we remove the PSK extension from the outer hello, replacing it
        // with a GREASE PSK to implement the "ClientHello Malleability Mitigation" mentioned
        // in 10.12.3.
        if let Some(psk_offer) = outer_hello.preshared_key_offer.as_mut() {
            self.grease_psk(psk_offer)?;
        }

        // To compute the encoded AAD we add a placeholder extension with an empty payload.
        outer_hello.encrypted_client_hello =
            Some(outer_hello_ext(self, enc.clone(), vec![0; payload_len]));

        // Next we compute the proper extension payload.
        let payload = self
            .sender
            .seal(&outer_hello.get_encoding(), &encoded_inner_hello)?;

        // And then we replace the placeholder extension with the real one.
        outer_hello.encrypted_client_hello = Some(outer_hello_ext(self, enc, payload));

        Ok(outer_hello)
    }

    /// Confirm whether an ECH offer was accepted based on examining the server hello.
    pub(crate) fn confirm_acceptance(
        self,
        ks: &mut KeyScheduleHandshakeStart,
        server_hello: &ServerHelloPayload,
        server_hello_encoded: &Payload<'_>,
        hash: &'static dyn Hash,
    ) -> Result<Option<EchAccepted>, Error> {
        // Start the inner transcript hash now that we know the hash algorithm to use.
        let inner_transcript = self
            .inner_hello_transcript
            .start_hash(hash);

        // Fork the transcript that we've started with the inner hello to use for a confirmation step.
        // We need to preserve the original inner_transcript to use if this confirmation succeeds.
        let mut confirmation_transcript = inner_transcript.clone();

        // Add the server hello confirmation - this is computed by altering the received
        // encoding rather than reencoding it.
        confirmation_transcript
            .add_message(&Self::server_hello_conf(server_hello, server_hello_encoded));

        // Derive a confirmation secret from the inner hello random and the confirmation transcript.
        let derived = ks.server_ech_confirmation_secret(
            self.inner_hello_random.0.as_ref(),
            confirmation_transcript.current_hash(),
        );

        // Check that first 8 digits of the derived secret match the last 8 digits of the original
        // server random. This match signals that the server accepted the ECH offer.
        // Indexing safety: Random is [0; 32] by construction.

        match ConstantTimeEq::ct_eq(derived.as_ref(), server_hello.random.0[24..].as_ref()).into() {
            true => {
                trace!("ECH accepted by server");
                Ok(Some(EchAccepted {
                    transcript: inner_transcript,
                    random: self.inner_hello_random,
                    sent_extensions: self.sent_extensions,
                }))
            }
            false => {
                trace!("ECH rejected by server");
                Ok(None)
            }
        }
    }

    pub(crate) fn confirm_hrr_acceptance(
        &self,
        hrr: &HelloRetryRequest,
        cs: &Tls13CipherSuite,
        common: &mut CommonState,
    ) -> Result<bool, Error> {
        // The client checks for the "encrypted_client_hello" extension.
        let ech_conf = match &hrr.encrypted_client_hello {
            // If none is found, the server has implicitly rejected ECH.
            None => return Ok(false),
            // Otherwise, if it has a length other than 8, the client aborts the
            // handshake with a "decode_error" alert.
            Some(ech_conf) if ech_conf.bytes().len() != 8 => {
                return Err({
                    common.send_fatal_alert(
                        AlertDescription::DecodeError,
                        PeerMisbehaved::IllegalHelloRetryRequestWithInvalidEch,
                    )
                });
            }
            Some(ech_conf) => ech_conf,
        };

        // Otherwise the client computes hrr_accept_confirmation as described in Section
        // 7.2.1
        let confirmation_transcript = self.inner_hello_transcript.clone();
        let mut confirmation_transcript =
            confirmation_transcript.start_hash(cs.common.hash_provider);
        confirmation_transcript.rollup_for_hrr();
        confirmation_transcript.add_message(&Self::hello_retry_request_conf(hrr));

        let derived = server_ech_hrr_confirmation_secret(
            cs.hkdf_provider,
            &self.inner_hello_random.0,
            confirmation_transcript.current_hash(),
        );

        match ConstantTimeEq::ct_eq(derived.as_ref(), ech_conf.bytes()).into() {
            true => {
                trace!("ECH accepted by server in hello retry request");
                Ok(true)
            }
            false => {
                trace!("ECH rejected by server in hello retry request");
                Ok(false)
            }
        }
    }

    /// Update the ECH context inner hello transcript based on a received hello retry request message.
    ///
    /// This will start the in-progress transcript using the given `hash`, convert it into an HRR
    /// buffer, and then add the hello retry message `m`.
    pub(crate) fn transcript_hrr_update(&mut self, hash: &'static dyn Hash, m: &Message<'_>) {
        trace!("Updating ECH inner transcript for HRR");

        let inner_transcript = self
            .inner_hello_transcript
            .clone()
            .start_hash(hash);

        let mut inner_transcript_buffer = inner_transcript.into_hrr_buffer();
        inner_transcript_buffer.add_message(m);
        self.inner_hello_transcript = inner_transcript_buffer;
    }

    // 5.1 "Encoding the ClientHelloInner"
    fn encode_inner_hello(
        &mut self,
        outer_hello: &ClientHelloPayload,
        retryreq: Option<&HelloRetryRequest>,
        resuming: &Option<Retrieved<&persist::Tls13ClientSessionValue>>,
    ) -> Vec<u8> {
        // Start building an inner hello using the outer_hello as a template.
        let mut inner_hello = ClientHelloPayload {
            // Some information is copied over as-is.
            client_version: outer_hello.client_version,
            session_id: outer_hello.session_id,
            compression_methods: outer_hello.compression_methods.clone(),

            // We will build up the included extensions ourselves.
            extensions: Box::new(ClientExtensions::default()),

            // Set the inner hello random to the one we generated when creating the ECH state.
            // We hold on to the inner_hello_random in the ECH state to use later for confirming
            // whether ECH was accepted or not.
            random: self.inner_hello_random,

            // We remove the empty renegotiation info SCSV from the outer hello's ciphersuite.
            // Similar to the TLS 1.2 specific extensions we will filter out, this is seen as a
            // TLS 1.2 only feature by bogo.
            cipher_suites: outer_hello
                .cipher_suites
                .iter()
                .filter(|cs| **cs != TLS_EMPTY_RENEGOTIATION_INFO_SCSV)
                .cloned()
                .collect(),
        };

        inner_hello.order_seed = outer_hello.order_seed;

        // The inner hello will always have an inner variant of the ECH extension added.
        // See Section 6.1 rule 4.
        inner_hello.encrypted_client_hello = Some(EncryptedClientHello::Inner);

        let inner_sni = match &self.inner_name {
            // The inner hello only gets a SNI value if enable_sni is true and the inner name
            // is a domain name (not an IP address).
            ServerName::DnsName(dns_name) if self.enable_sni => Some(dns_name),
            _ => None,
        };

        // Now we consider each of the outer hello's extensions - we can either:
        // 1. Omit the extension if it isn't appropriate (e.g. is a TLS 1.2 extension).
        // 2. Add the extension to the inner hello as-is.
        // 3. Compress the extension, by collecting it into a list of to-be-compressed
        //    extensions we'll handle separately.
        let outer_extensions = outer_hello.used_extensions_in_encoding_order();
        let mut compressed_exts = Vec::with_capacity(outer_extensions.len());
        for ext in outer_extensions {
            // Some outer hello extensions are only useful in the context where a TLS 1.3
            // connection allows TLS 1.2. This isn't the case for ECH so we skip adding them
            // to the inner hello.
            if matches!(
                ext,
                ExtensionType::ExtendedMasterSecret
                    | ExtensionType::SessionTicket
                    | ExtensionType::ECPointFormats
            ) {
                continue;
            }

            if ext == ExtensionType::ServerName {
                // We may want to replace the outer hello SNI with our own inner hello specific SNI.
                if let Some(sni_value) = inner_sni {
                    inner_hello.server_name = Some(ServerNamePayload::from(sni_value));
                }
                // We don't want to add, or compress, the SNI from the outer hello.
                continue;
            }

            // Compressed extensions need to be put aside to include in one contiguous block.
            // Uncompressed extensions get added directly to the inner hello.
            if ext.ech_compress() {
                compressed_exts.push(ext);
            }

            inner_hello.clone_one(outer_hello, ext);
        }

        // We've added all the uncompressed extensions. Now we need to add the contiguous
        // block of to-be-compressed extensions.
        inner_hello.contiguous_extensions = compressed_exts.clone();

        // Note which extensions we're sending in the inner hello. This may differ from
        // the outer hello (e.g. the inner hello may omit SNI while the outer hello will
        // always have the ECH cover name in SNI).
        self.sent_extensions = inner_hello.collect_used();

        // If we're resuming, we need to update the PSK binder in the inner hello.
        if let Some(resuming) = resuming.as_ref() {
            let mut chp = HandshakeMessagePayload(HandshakePayload::ClientHello(inner_hello));

            // Retain the early key schedule we get from processing the binder.
            self.early_data_key_schedule = Some(tls13::fill_in_psk_binder(
                resuming,
                &self.inner_hello_transcript,
                &mut chp,
            ));

            // fill_in_psk_binder works on an owned HandshakeMessagePayload, so we need to
            // extract our inner hello back out of it to retain ownership.
            inner_hello = match chp.0 {
                HandshakePayload::ClientHello(chp) => chp,
                // Safety: we construct the HMP above and know its type unconditionally.
                _ => unreachable!(),
            };
        }

        trace!("ECH Inner Hello: {inner_hello:#?}");

        // Encode the inner hello according to the rules required for ECH. This differs
        // from the standard encoding in several ways. Notably this is where we will
        // replace the block of contiguous to-be-compressed extensions with a marker.
        let mut encoded_hello = inner_hello.ech_inner_encoding(compressed_exts);

        // Calculate padding
        // max_name_len = L
        let max_name_len = self.maximum_name_length;
        let max_name_len = if max_name_len > 0 { max_name_len } else { 255 };

        let padding_len = match &self.inner_name {
            ServerName::DnsName(name) => {
                // name.len() = D
                // max(0, L - D)
                core::cmp::max(
                    0,
                    max_name_len.saturating_sub(name.as_ref().len() as u8) as usize,
                )
            }
            _ => {
                // L + 9
                // "This is the length of a "server_name" extension with an L-byte name."
                // We widen to usize here to avoid overflowing u8 + u8.
                max_name_len as usize + 9
            }
        };

        // Let L be the length of the EncodedClientHelloInner with all the padding computed so far
        // Let N = 31 - ((L - 1) % 32) and add N bytes of padding.
        let padding_len = 31 - ((encoded_hello.len() + padding_len - 1) % 32);
        encoded_hello.extend(vec![0; padding_len]);

        // Construct the inner hello message that will be used for the transcript.
        let inner_hello_msg = Message {
            version: match retryreq {
                // <https://datatracker.ietf.org/doc/html/rfc8446#section-5.1>:
                // "This value MUST be set to 0x0303 for all records generated
                //  by a TLS 1.3 implementation ..."
                Some(_) => ProtocolVersion::TLSv1_2,
                // "... other than an initial ClientHello (i.e., one not
                // generated after a HelloRetryRequest), where it MAY also be
                // 0x0301 for compatibility purposes"
                //
                // (retryreq == None means we're in the "initial ClientHello" case)
                None => ProtocolVersion::TLSv1_0,
            },
            payload: MessagePayload::handshake(HandshakeMessagePayload(
                HandshakePayload::ClientHello(inner_hello),
            )),
        };

        // Update the inner transcript buffer with the inner hello message.
        self.inner_hello_transcript
            .add_message(&inner_hello_msg);

        encoded_hello
    }

    // See https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#name-grease-psk
    fn grease_psk(&self, psk_offer: &mut PresharedKeyOffer) -> Result<(), Error> {
        for ident in psk_offer.identities.iter_mut() {
            // "For each PSK identity advertised in the ClientHelloInner, the
            // client generates a random PSK identity with the same length."
            self.secure_random
                .fill(&mut ident.identity.0)?;
            // "It also generates a random, 32-bit, unsigned integer to use as
            // the obfuscated_ticket_age."
            let mut ticket_age = [0_u8; 4];
            self.secure_random
                .fill(&mut ticket_age)?;
            ident.obfuscated_ticket_age = u32::from_be_bytes(ticket_age);
        }

        // "Likewise, for each inner PSK binder, the client generates a random string
        // of the same length."
        psk_offer.binders = psk_offer
            .binders
            .iter()
            .map(|old_binder| {
                // We can't access the wrapped binder PresharedKeyBinder's PayloadU8 mutably,
                // so we construct new PresharedKeyBinder's from scratch with the same length.
                let mut new_binder = vec![0; old_binder.as_ref().len()];
                self.secure_random
                    .fill(&mut new_binder)?;
                Ok::<PresharedKeyBinder, Error>(PresharedKeyBinder::from(new_binder))
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    fn server_hello_conf(
        server_hello: &ServerHelloPayload,
        server_hello_encoded: &Payload<'_>,
    ) -> Message<'static> {
        // The confirmation is computed over the server hello, which has had
        // its `random` field altered to zero the final 8 bytes.
        //
        // nb. we don't require that we can round-trip a `ServerHelloPayload`, to
        // allow for efficiency in its in-memory representation.  That means
        // we operate here on the received encoding, as the confirmation needs
        // to be computed on that.
        let mut encoded = server_hello_encoded.clone().into_vec();
        encoded[SERVER_HELLO_ECH_CONFIRMATION_SPAN].fill(0x00);

        Message {
            version: ProtocolVersion::TLSv1_3,
            payload: MessagePayload::Handshake {
                encoded: Payload::Owned(encoded),
                parsed: HandshakeMessagePayload(HandshakePayload::ServerHello(
                    server_hello.clone(),
                )),
            },
        }
    }

    fn hello_retry_request_conf(retry_req: &HelloRetryRequest) -> Message<'_> {
        Self::ech_conf_message(HandshakeMessagePayload(
            HandshakePayload::HelloRetryRequest(retry_req.clone()),
        ))
    }

    fn ech_conf_message(hmp: HandshakeMessagePayload<'_>) -> Message<'_> {
        let mut hmp_encoded = Vec::new();
        hmp.payload_encode(&mut hmp_encoded, Encoding::EchConfirmation);
        Message {
            version: ProtocolVersion::TLSv1_3,
            payload: MessagePayload::Handshake {
                encoded: Payload::new(hmp_encoded),
                parsed: hmp,
            },
        }
    }
}

/// The last eight bytes of the ServerHello's random, taken from a Handshake message containing it.
///
/// This has:
/// - a HandshakeType (1 byte),
/// - an exterior length (3 bytes),
/// - the legacy_version (2 bytes), and
/// - the balance of the random field (24 bytes).
const SERVER_HELLO_ECH_CONFIRMATION_SPAN: core::ops::Range<usize> =
    (1 + 3 + 2 + 24)..(1 + 3 + 2 + 32);

/// Returned from EchState::check_acceptance when the server has accepted the ECH offer.
///
/// Holds the state required to continue the handshake with the inner hello from the ECH offer.
pub(crate) struct EchAccepted {
    pub(crate) transcript: HandshakeHash,
    pub(crate) random: Random,
    pub(crate) sent_extensions: Vec<ExtensionType>,
}

pub(crate) fn fatal_alert_required(
    retry_configs: Option<Vec<EchConfigPayload>>,
    common: &mut CommonState,
) -> Error {
    common.send_fatal_alert(
        AlertDescription::EncryptedClientHelloRequired,
        PeerIncompatible::ServerRejectedEncryptedClientHello(retry_configs),
    )
}

#[cfg(test)]
mod tests {
    use crate::enums::CipherSuite;
    use crate::msgs::handshake::{Random, ServerExtensions, SessionId};

    use super::*;

    #[test]
    fn server_hello_conf_alters_server_hello_random() {
        let server_hello = ServerHelloPayload {
            legacy_version: ProtocolVersion::TLSv1_2,
            random: Random([0xffu8; 32]),
            session_id: SessionId::empty(),
            cipher_suite: CipherSuite::TLS13_AES_256_GCM_SHA384,
            compression_method: crate::msgs::enums::Compression::Null,
            extensions: Box::new(ServerExtensions::default()),
        };
        let message = Message {
            version: ProtocolVersion::TLSv1_3,
            payload: MessagePayload::handshake(HandshakeMessagePayload(
                HandshakePayload::ServerHello(server_hello.clone()),
            )),
        };
        let Message {
            payload:
                MessagePayload::Handshake {
                    encoded: server_hello_encoded_before,
                    ..
                },
            ..
        } = &message
        else {
            unreachable!("ServerHello is a handshake message");
        };

        let message = EchState::server_hello_conf(&server_hello, server_hello_encoded_before);

        let Message {
            payload:
                MessagePayload::Handshake {
                    encoded: server_hello_encoded_after,
                    ..
                },
            ..
        } = &message
        else {
            unreachable!("ServerHello is a handshake message");
        };

        assert_eq!(
            std::format!("{server_hello_encoded_before:x?}"),
            "020000280303ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff001302000000",
            "beforehand eight bytes at end of Random should be 0xff here ^^^^^^^^^^^^^^^^            "
        );
        assert_eq!(
            std::format!("{server_hello_encoded_after:x?}"),
            "020000280303ffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000001302000000",
            "                          afterwards those bytes are zeroed ^^^^^^^^^^^^^^^^            "
        );
    }
}
