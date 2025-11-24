use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::vec::Vec;

use pki_types::DnsName;

use super::server_conn::ServerConnectionData;
#[cfg(feature = "tls12")]
use super::tls12;
use crate::common_state::{KxState, Protocol, State};
use crate::conn::ConnectionRandoms;
use crate::crypto::SupportedKxGroup;
use crate::enums::{
    AlertDescription, CertificateType, CipherSuite, HandshakeType, ProtocolVersion,
    SignatureAlgorithm, SignatureScheme,
};
use crate::error::{Error, PeerIncompatible, PeerMisbehaved};
use crate::hash_hs::{HandshakeHash, HandshakeHashBuffer};
use crate::log::{debug, trace};
use crate::msgs::enums::{Compression, ExtensionType, NamedGroup};
#[cfg(feature = "tls12")]
use crate::msgs::handshake::SessionId;
use crate::msgs::handshake::{
    ClientHelloPayload, HandshakePayload, KeyExchangeAlgorithm, ProtocolName, Random,
    ServerExtensions, ServerExtensionsInput, ServerNamePayload, SingleProtocolName,
    TransportParameters,
};
use crate::msgs::message::{Message, MessagePayload};
use crate::msgs::persist;
use crate::server::common::ActiveCertifiedKey;
use crate::server::{ClientHello, ServerConfig, tls13};
use crate::sync::Arc;
use crate::{SupportedCipherSuite, suites};

pub(super) type NextState<'a> = Box<dyn State<ServerConnectionData> + 'a>;
pub(super) type NextStateOrError<'a> = Result<NextState<'a>, Error>;
pub(super) type ServerContext<'a> = crate::common_state::Context<'a, ServerConnectionData>;

pub(super) fn can_resume(
    suite: SupportedCipherSuite,
    sni: &Option<DnsName<'_>>,
    using_ems: bool,
    resumedata: &persist::ServerSessionValue,
) -> bool {
    // The RFCs underspecify what happens if we try to resume to
    // an unoffered/varying suite.  We merely don't resume in weird cases.
    //
    // RFC 6066 says "A server that implements this extension MUST NOT accept
    // the request to resume the session if the server_name extension contains
    // a different name. Instead, it proceeds with a full handshake to
    // establish a new session."
    //
    // RFC 8446: "The server MUST ensure that it selects
    // a compatible PSK (if any) and cipher suite."
    resumedata.cipher_suite == suite.suite()
        && (resumedata.extended_ms == using_ems || (resumedata.extended_ms && !using_ems))
        && &resumedata.sni == sni
}

#[derive(Default)]
pub(super) struct ExtensionProcessing {
    // extensions to reply with
    pub(super) extensions: Box<ServerExtensions<'static>>,
    #[cfg(feature = "tls12")]
    pub(super) send_ticket: bool,
}

impl ExtensionProcessing {
    pub(super) fn new(extra_exts: ServerExtensionsInput<'static>) -> Self {
        let ServerExtensionsInput {
            transport_parameters,
        } = extra_exts;

        let mut extensions = Box::new(ServerExtensions::default());
        match transport_parameters {
            Some(TransportParameters::Quic(v)) => extensions.transport_parameters = Some(v),
            Some(TransportParameters::QuicDraft(v)) => {
                extensions.transport_parameters_draft = Some(v)
            }
            None => {}
        }

        Self {
            extensions,
            #[cfg(feature = "tls12")]
            send_ticket: false,
        }
    }

    pub(super) fn process_common(
        &mut self,
        config: &ServerConfig,
        cx: &mut ServerContext<'_>,
        ocsp_response: &mut Option<&[u8]>,
        hello: &ClientHelloPayload,
        resumedata: Option<&persist::ServerSessionValue>,
    ) -> Result<(), Error> {
        // ALPN
        let our_protocols = &config.alpn_protocols;
        if let Some(their_protocols) = &hello.protocols {
            cx.common.alpn_protocol = our_protocols
                .iter()
                .find(|ours| {
                    their_protocols
                        .iter()
                        .any(|theirs| theirs.as_ref() == ours.as_slice())
                })
                .map(|bytes| ProtocolName::from(bytes.clone()));
            if let Some(selected_protocol) = &cx.common.alpn_protocol {
                debug!("Chosen ALPN protocol {selected_protocol:?}");

                self.extensions.selected_protocol =
                    Some(SingleProtocolName::new(selected_protocol.clone()));
            } else if !our_protocols.is_empty() {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::NoApplicationProtocol,
                    Error::NoApplicationProtocol,
                ));
            }
        }

        if cx.common.is_quic() {
            // QUIC has strict ALPN, unlike TLS's more backwards-compatible behavior. RFC 9001
            // says: "The server MUST treat the inability to select a compatible application
            // protocol as a connection error of type 0x0178". We judge that ALPN was desired
            // (rather than some out-of-band protocol negotiation mechanism) if and only if any ALPN
            // protocols were configured locally or offered by the client. This helps prevent
            // successful establishment of connections between peers that can't understand
            // each other.
            if cx.common.alpn_protocol.is_none()
                && (!our_protocols.is_empty() || hello.protocols.is_some())
            {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::NoApplicationProtocol,
                    Error::NoApplicationProtocol,
                ));
            }

            let transport_params = hello
                .transport_parameters
                .as_ref()
                .or(hello
                    .transport_parameters_draft
                    .as_ref());
            match transport_params {
                Some(params) => cx.common.quic.params = Some(params.to_owned().into_vec()),
                None => {
                    return Err(cx
                        .common
                        .missing_extension(PeerMisbehaved::MissingQuicTransportParameters));
                }
            }
        }

        let for_resume = resumedata.is_some();
        // SNI
        if let (false, Some(ServerNamePayload::SingleDnsName(_))) = (for_resume, &hello.server_name)
        {
            self.extensions.server_name_ack = Some(());
        }

        // Send status_request response if we have one.  This is not allowed
        // if we're resuming, and is only triggered if we have an OCSP response
        // to send.
        if !for_resume
            && hello
                .certificate_status_request
                .is_some()
        {
            if ocsp_response.is_some() && !cx.common.is_tls13() {
                // Only TLS1.2 sends confirmation in ServerHello
                self.extensions
                    .certificate_status_request_ack = Some(());
            }
        } else {
            // Throw away any OCSP response so we don't try to send it later.
            ocsp_response.take();
        }

        self.validate_server_cert_type_extension(hello, config, cx)?;
        self.validate_client_cert_type_extension(hello, config, cx)?;

        Ok(())
    }

    #[cfg(feature = "tls12")]
    pub(super) fn process_tls12(
        &mut self,
        config: &ServerConfig,
        hello: &ClientHelloPayload,
        using_ems: bool,
    ) {
        // Renegotiation.
        // (We don't do reneg at all, but would support the secure version if we did.)

        use crate::msgs::base::PayloadU8;
        let secure_reneg_offered = hello.renegotiation_info.is_some()
            || hello
                .cipher_suites
                .contains(&CipherSuite::TLS_EMPTY_RENEGOTIATION_INFO_SCSV);

        if secure_reneg_offered {
            self.extensions.renegotiation_info = Some(PayloadU8::new(Vec::new()));
        }

        // Tickets:
        // If we get any SessionTicket extension and have tickets enabled,
        // we send an ack.
        if hello.session_ticket.is_some() && config.ticketer.enabled() {
            self.send_ticket = true;
            self.extensions.session_ticket_ack = Some(());
        }

        // Confirm use of EMS if offered.
        if using_ems {
            self.extensions
                .extended_master_secret_ack = Some(());
        }
    }

    fn validate_server_cert_type_extension(
        &mut self,
        hello: &ClientHelloPayload,
        config: &ServerConfig,
        cx: &mut ServerContext<'_>,
    ) -> Result<(), Error> {
        let client_supports = hello
            .server_certificate_types
            .as_deref()
            .unwrap_or_default();

        self.process_cert_type_extension(
            client_supports,
            config
                .cert_resolver
                .only_raw_public_keys(),
            ExtensionType::ServerCertificateType,
            cx,
        )
    }

    fn validate_client_cert_type_extension(
        &mut self,
        hello: &ClientHelloPayload,
        config: &ServerConfig,
        cx: &mut ServerContext<'_>,
    ) -> Result<(), Error> {
        let client_supports = hello
            .client_certificate_types
            .as_deref()
            .unwrap_or_default();

        self.process_cert_type_extension(
            client_supports,
            config
                .verifier
                .requires_raw_public_keys(),
            ExtensionType::ClientCertificateType,
            cx,
        )
    }

    fn process_cert_type_extension(
        &mut self,
        client_supports: &[CertificateType],
        requires_raw_keys: bool,
        extension_type: ExtensionType,
        cx: &mut ServerContext<'_>,
    ) -> Result<(), Error> {
        debug_assert!(
            extension_type == ExtensionType::ClientCertificateType
                || extension_type == ExtensionType::ServerCertificateType
        );
        let raw_key_negotation_result = match (
            requires_raw_keys,
            client_supports.contains(&CertificateType::RawPublicKey),
            client_supports.contains(&CertificateType::X509),
        ) {
            (true, true, _) => Ok((extension_type, CertificateType::RawPublicKey)),
            (false, _, true) => Ok((extension_type, CertificateType::X509)),
            (false, true, false) => Err(Error::PeerIncompatible(
                PeerIncompatible::IncorrectCertificateTypeExtension,
            )),
            (true, false, _) => Err(Error::PeerIncompatible(
                PeerIncompatible::IncorrectCertificateTypeExtension,
            )),
            (false, false, false) => return Ok(()),
        };

        match raw_key_negotation_result {
            Ok((ExtensionType::ClientCertificateType, cert_type)) => {
                self.extensions.client_certificate_type = Some(cert_type);
            }
            Ok((ExtensionType::ServerCertificateType, cert_type)) => {
                self.extensions.server_certificate_type = Some(cert_type);
            }
            Err(err) => {
                return Err(cx
                    .common
                    .send_fatal_alert(AlertDescription::HandshakeFailure, err));
            }
            Ok((_, _)) => unreachable!(),
        }
        Ok(())
    }
}

pub(super) struct ExpectClientHello {
    pub(super) config: Arc<ServerConfig>,
    pub(super) extra_exts: ServerExtensionsInput<'static>,
    pub(super) transcript: HandshakeHashOrBuffer,
    #[cfg(feature = "tls12")]
    pub(super) session_id: SessionId,
    #[cfg(feature = "tls12")]
    pub(super) using_ems: bool,
    pub(super) done_retry: bool,
    pub(super) send_tickets: usize,
}

impl ExpectClientHello {
    pub(super) fn new(
        config: Arc<ServerConfig>,
        extra_exts: ServerExtensionsInput<'static>,
    ) -> Self {
        let mut transcript_buffer = HandshakeHashBuffer::new();

        if config.verifier.offer_client_auth() {
            transcript_buffer.set_client_auth_enabled();
        }

        Self {
            config,
            extra_exts,
            transcript: HandshakeHashOrBuffer::Buffer(transcript_buffer),
            #[cfg(feature = "tls12")]
            session_id: SessionId::empty(),
            #[cfg(feature = "tls12")]
            using_ems: false,
            done_retry: false,
            send_tickets: 0,
        }
    }

    /// Continues handling of a `ClientHello` message once config and certificate are available.
    pub(super) fn with_certified_key(
        self,
        mut sig_schemes: Vec<SignatureScheme>,
        client_hello: &ClientHelloPayload,
        m: &Message<'_>,
        cx: &mut ServerContext<'_>,
    ) -> NextStateOrError<'static> {
        let tls13_enabled = self
            .config
            .supports_version(ProtocolVersion::TLSv1_3);
        let tls12_enabled = self
            .config
            .supports_version(ProtocolVersion::TLSv1_2);

        // Are we doing TLS1.3?
        let version = if let Some(versions) = &client_hello.supported_versions {
            if versions.tls13 && tls13_enabled {
                ProtocolVersion::TLSv1_3
            } else if !versions.tls12 || !tls12_enabled {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::ProtocolVersion,
                    PeerIncompatible::Tls12NotOfferedOrEnabled,
                ));
            } else if cx.common.is_quic() {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::ProtocolVersion,
                    PeerIncompatible::Tls13RequiredForQuic,
                ));
            } else {
                ProtocolVersion::TLSv1_2
            }
        } else if u16::from(client_hello.client_version) < u16::from(ProtocolVersion::TLSv1_2) {
            return Err(cx.common.send_fatal_alert(
                AlertDescription::ProtocolVersion,
                PeerIncompatible::Tls12NotOffered,
            ));
        } else if !tls12_enabled && tls13_enabled {
            return Err(cx.common.send_fatal_alert(
                AlertDescription::ProtocolVersion,
                PeerIncompatible::SupportedVersionsExtensionRequired,
            ));
        } else if cx.common.is_quic() {
            return Err(cx.common.send_fatal_alert(
                AlertDescription::ProtocolVersion,
                PeerIncompatible::Tls13RequiredForQuic,
            ));
        } else {
            ProtocolVersion::TLSv1_2
        };

        cx.common.negotiated_version = Some(version);

        // We communicate to the upper layer what kind of key they should choose
        // via the sigschemes value.  Clients tend to treat this extension
        // orthogonally to offered ciphersuites (even though, in TLS1.2 it is not).
        // So: reduce the offered sigschemes to those compatible with the
        // intersection of ciphersuites.
        let client_suites = self
            .config
            .provider
            .cipher_suites
            .iter()
            .copied()
            .filter(|scs| {
                client_hello
                    .cipher_suites
                    .contains(&scs.suite())
            })
            .collect::<Vec<_>>();

        sig_schemes
            .retain(|scheme| suites::compatible_sigscheme_for_suites(*scheme, &client_suites));

        // We adhere to the TLS 1.2 RFC by not exposing this to the cert resolver if TLS version is 1.2
        let certificate_authorities = match version {
            ProtocolVersion::TLSv1_2 => None,
            _ => client_hello
                .certificate_authority_names
                .as_deref(),
        };
        // Choose a certificate.
        let certkey = {
            let client_hello = ClientHello {
                server_name: &cx.data.sni,
                signature_schemes: &sig_schemes,
                alpn: client_hello.protocols.as_ref(),
                client_cert_types: client_hello
                    .client_certificate_types
                    .as_deref(),
                server_cert_types: client_hello
                    .server_certificate_types
                    .as_deref(),
                cipher_suites: &client_hello.cipher_suites,
                certificate_authorities,
                named_groups: client_hello.named_groups.as_deref(),
            };
            trace!("Resolving server certificate: {client_hello:#?}");

            let certkey = self
                .config
                .cert_resolver
                .resolve(client_hello);

            certkey.ok_or_else(|| {
                cx.common.send_fatal_alert(
                    AlertDescription::AccessDenied,
                    Error::General("no server certificate chain resolved".to_owned()),
                )
            })?
        };
        let certkey = ActiveCertifiedKey::from_certified_key(&certkey);

        let (suite, skxg) = self
            .choose_suite_and_kx_group(
                version,
                certkey.get_key().algorithm(),
                cx.common.protocol,
                client_hello
                    .named_groups
                    .as_deref()
                    .unwrap_or_default(),
                &client_hello.cipher_suites,
            )
            .map_err(|incompat| {
                cx.common
                    .send_fatal_alert(AlertDescription::HandshakeFailure, incompat)
            })?;

        debug!("decided upon suite {suite:?}");
        cx.common.suite = Some(suite);
        cx.common.kx_state = KxState::Start(skxg);

        // Start handshake hash.
        let starting_hash = suite.hash_provider();
        let transcript = match self.transcript {
            HandshakeHashOrBuffer::Buffer(inner) => inner.start_hash(starting_hash),
            HandshakeHashOrBuffer::Hash(inner)
                if inner.algorithm() == starting_hash.algorithm() =>
            {
                inner
            }
            _ => {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::HandshakeHashVariedAfterRetry,
                ));
            }
        };

        // Save their Random.
        let randoms = ConnectionRandoms::new(
            client_hello.random,
            Random::new(self.config.provider.secure_random)?,
        );
        match suite {
            SupportedCipherSuite::Tls13(suite) => tls13::CompleteClientHelloHandling {
                config: self.config,
                transcript,
                suite,
                randoms,
                done_retry: self.done_retry,
                send_tickets: self.send_tickets,
                extra_exts: self.extra_exts,
            }
            .handle_client_hello(cx, certkey, m, client_hello, skxg, sig_schemes),
            #[cfg(feature = "tls12")]
            SupportedCipherSuite::Tls12(suite) => tls12::CompleteClientHelloHandling {
                config: self.config,
                transcript,
                session_id: self.session_id,
                suite,
                using_ems: self.using_ems,
                randoms,
                send_ticket: self.send_tickets > 0,
                extra_exts: self.extra_exts,
            }
            .handle_client_hello(
                cx,
                certkey,
                m,
                client_hello,
                skxg,
                sig_schemes,
                tls13_enabled,
            ),
        }
    }

    fn choose_suite_and_kx_group(
        &self,
        selected_version: ProtocolVersion,
        sig_key_algorithm: SignatureAlgorithm,
        protocol: Protocol,
        client_groups: &[NamedGroup],
        client_suites: &[CipherSuite],
    ) -> Result<(SupportedCipherSuite, &'static dyn SupportedKxGroup), PeerIncompatible> {
        // Determine which `KeyExchangeAlgorithm`s are theoretically possible, based
        // on the offered and supported groups.
        let mut ecdhe_possible = false;
        let mut ffdhe_possible = false;
        let mut ffdhe_offered = false;
        let mut supported_groups = Vec::with_capacity(client_groups.len());

        for offered_group in client_groups {
            let supported = self
                .config
                .provider
                .kx_groups
                .iter()
                .find(|skxg| {
                    skxg.usable_for_version(selected_version) && skxg.name() == *offered_group
                });

            match offered_group.key_exchange_algorithm() {
                KeyExchangeAlgorithm::DHE => {
                    ffdhe_possible |= supported.is_some();
                    ffdhe_offered = true;
                }
                KeyExchangeAlgorithm::ECDHE => {
                    ecdhe_possible |= supported.is_some();
                }
            }

            supported_groups.push(supported);
        }

        let first_supported_dhe_kxg = if selected_version == ProtocolVersion::TLSv1_2 {
            // https://datatracker.ietf.org/doc/html/rfc7919#section-4 (paragraph 2)
            let first_supported_dhe_kxg = self
                .config
                .provider
                .kx_groups
                .iter()
                .find(|skxg| skxg.name().key_exchange_algorithm() == KeyExchangeAlgorithm::DHE);
            ffdhe_possible |= !ffdhe_offered && first_supported_dhe_kxg.is_some();
            first_supported_dhe_kxg
        } else {
            // In TLS1.3, the server may only directly negotiate a group.
            None
        };

        if !ecdhe_possible && !ffdhe_possible {
            return Err(PeerIncompatible::NoKxGroupsInCommon);
        }

        let mut suitable_suites_iter = self
            .config
            .provider
            .cipher_suites
            .iter()
            .filter(|suite| {
                // Reduce our supported ciphersuites by the certified key's algorithm.
                suite.usable_for_signature_algorithm(sig_key_algorithm)
                // And version
                && suite.version().version == selected_version
                // And protocol
                && suite.usable_for_protocol(protocol)
                // And support one of key exchange groups
                && (ecdhe_possible && suite.usable_for_kx_algorithm(KeyExchangeAlgorithm::ECDHE)
                || ffdhe_possible && suite.usable_for_kx_algorithm(KeyExchangeAlgorithm::DHE))
            });

        // RFC 7919 (https://datatracker.ietf.org/doc/html/rfc7919#section-4) requires us to send
        // the InsufficientSecurity alert in case we don't recognize client's FFDHE groups (i.e.,
        // `suitable_suites` becomes empty). But that does not make a lot of sense (e.g., client
        // proposes FFDHE4096 and we only support FFDHE2048), so we ignore that requirement here,
        // and continue to send HandshakeFailure.

        let suite = if self.config.ignore_client_order {
            suitable_suites_iter.find(|suite| client_suites.contains(&suite.suite()))
        } else {
            let suitable_suites = suitable_suites_iter.collect::<Vec<_>>();
            client_suites
                .iter()
                .find_map(|client_suite| {
                    suitable_suites
                        .iter()
                        .find(|x| *client_suite == x.suite())
                })
                .copied()
        }
        .ok_or(PeerIncompatible::NoCipherSuitesInCommon)?;

        // Finally, choose a key exchange group that is compatible with the selected cipher
        // suite.
        let maybe_skxg = supported_groups
            .iter()
            .find_map(|maybe_skxg| match maybe_skxg {
                Some(skxg) => suite
                    .usable_for_kx_algorithm(skxg.name().key_exchange_algorithm())
                    .then_some(*skxg),
                None => None,
            });

        if selected_version == ProtocolVersion::TLSv1_3 {
            // This unwrap is structurally guaranteed by the early return for `!ffdhe_possible && !ecdhe_possible`
            return Ok((*suite, *maybe_skxg.unwrap()));
        }

        // For TLS1.2, the server can unilaterally choose a DHE group if it has one and
        // there was no better option.
        match maybe_skxg {
            Some(skxg) => Ok((*suite, *skxg)),
            None if suite.usable_for_kx_algorithm(KeyExchangeAlgorithm::DHE) => {
                // If kx for the selected cipher suite is DHE and no DHE groups are specified in the extension,
                // the server is free to choose DHE params, we choose the first DHE kx group of the provider.
                if let Some(server_selected_ffdhe_skxg) = first_supported_dhe_kxg {
                    Ok((*suite, *server_selected_ffdhe_skxg))
                } else {
                    Err(PeerIncompatible::NoKxGroupsInCommon)
                }
            }
            None => Err(PeerIncompatible::NoKxGroupsInCommon),
        }
    }
}

impl State<ServerConnectionData> for ExpectClientHello {
    fn handle<'m>(
        self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> NextStateOrError<'m>
    where
        Self: 'm,
    {
        let (client_hello, sig_schemes) = process_client_hello(&m, self.done_retry, cx)?;
        self.with_certified_key(sig_schemes, client_hello, &m, cx)
    }

    fn into_owned(self: Box<Self>) -> NextState<'static> {
        self
    }
}

/// Configuration-independent validation of a `ClientHello` message.
///
/// This represents the first part of the `ClientHello` handling, where we do all validation that
/// doesn't depend on a `ServerConfig` being available and extract everything needed to build a
/// [`ClientHello`] value for a [`ResolvesServerCert`].
///
/// Note that this will modify `data.sni` even if config or certificate resolution fail.
///
/// [`ResolvesServerCert`]: crate::server::ResolvesServerCert
pub(super) fn process_client_hello<'m>(
    m: &'m Message<'m>,
    done_retry: bool,
    cx: &mut ServerContext<'_>,
) -> Result<(&'m ClientHelloPayload, Vec<SignatureScheme>), Error> {
    let client_hello =
        require_handshake_msg!(m, HandshakeType::ClientHello, HandshakePayload::ClientHello)?;
    trace!("we got a clienthello {client_hello:?}");

    if !client_hello
        .compression_methods
        .contains(&Compression::Null)
    {
        return Err(cx.common.send_fatal_alert(
            AlertDescription::IllegalParameter,
            PeerIncompatible::NullCompressionRequired,
        ));
    }

    // No handshake messages should follow this one in this flight.
    cx.common.check_aligned_handshake()?;

    // Extract and validate the SNI DNS name, if any, before giving it to
    // the cert resolver. In particular, if it is invalid then we should
    // send an Illegal Parameter alert instead of the Internal Error alert
    // (or whatever) that we'd send if this were checked later or in a
    // different way.
    //
    // [RFC6066][] specifies that literal IP addresses are illegal in
    // `ServerName`s with a `name_type` of `host_name`.
    //
    // Some clients incorrectly send such extensions: we choose to
    // successfully parse these (into `ServerNamePayload::IpAddress`)
    // but then act like the client sent no `server_name` extension.
    //
    // [RFC6066]: https://datatracker.ietf.org/doc/html/rfc6066#section-3
    let sni = match &client_hello.server_name {
        Some(ServerNamePayload::SingleDnsName(dns_name)) => Some(dns_name.to_lowercase_owned()),
        Some(ServerNamePayload::IpAddress) => None,
        Some(ServerNamePayload::Invalid) => {
            return Err(cx.common.send_fatal_alert(
                AlertDescription::IllegalParameter,
                PeerMisbehaved::ServerNameMustContainOneHostName,
            ));
        }
        None => None,
    };

    // save only the first SNI
    if let (Some(sni), false) = (&sni, done_retry) {
        // Save the SNI into the session.
        // The SNI hostname is immutable once set.
        assert!(cx.data.sni.is_none());
        cx.data.sni = Some(sni.clone());
    } else if cx.data.sni != sni {
        return Err(PeerMisbehaved::ServerNameDifferedOnRetry.into());
    }

    let sig_schemes = client_hello
        .signature_schemes
        .as_ref()
        .ok_or_else(|| {
            cx.common.send_fatal_alert(
                AlertDescription::HandshakeFailure,
                PeerIncompatible::SignatureAlgorithmsExtensionRequired,
            )
        })?;

    Ok((client_hello, sig_schemes.to_owned()))
}

pub(crate) enum HandshakeHashOrBuffer {
    Buffer(HandshakeHashBuffer),
    Hash(HandshakeHash),
}
