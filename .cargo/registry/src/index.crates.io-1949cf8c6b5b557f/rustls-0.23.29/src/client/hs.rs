use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;

use pki_types::ServerName;

#[cfg(feature = "tls12")]
use super::tls12;
use super::{ResolvesClientCert, Tls12Resumption};
use crate::SupportedCipherSuite;
#[cfg(feature = "logging")]
use crate::bs_debug;
use crate::check::inappropriate_handshake_message;
use crate::client::client_conn::ClientConnectionData;
use crate::client::common::ClientHelloDetails;
use crate::client::ech::EchState;
use crate::client::{ClientConfig, EchMode, EchStatus, tls13};
use crate::common_state::{CommonState, HandshakeKind, KxState, State};
use crate::conn::ConnectionRandoms;
use crate::crypto::{ActiveKeyExchange, KeyExchangeAlgorithm};
use crate::enums::{
    AlertDescription, CertificateType, CipherSuite, ContentType, HandshakeType, ProtocolVersion,
};
use crate::error::{Error, PeerIncompatible, PeerMisbehaved};
use crate::hash_hs::HandshakeHashBuffer;
use crate::log::{debug, trace};
use crate::msgs::base::Payload;
use crate::msgs::enums::{Compression, ExtensionType};
use crate::msgs::handshake::{
    CertificateStatusRequest, ClientExtensions, ClientExtensionsInput, ClientHelloPayload,
    ClientSessionTicket, EncryptedClientHello, HandshakeMessagePayload, HandshakePayload,
    HelloRetryRequest, KeyShareEntry, ProtocolName, PskKeyExchangeModes, Random, ServerNamePayload,
    SessionId, SupportedEcPointFormats, SupportedProtocolVersions, TransportParameters,
};
use crate::msgs::message::{Message, MessagePayload};
use crate::msgs::persist;
use crate::sync::Arc;
use crate::tls13::key_schedule::KeyScheduleEarly;
use crate::verify::ServerCertVerifier;

pub(super) type NextState<'a> = Box<dyn State<ClientConnectionData> + 'a>;
pub(super) type NextStateOrError<'a> = Result<NextState<'a>, Error>;
pub(super) type ClientContext<'a> = crate::common_state::Context<'a, ClientConnectionData>;

struct ExpectServerHello {
    input: ClientHelloInput,
    transcript_buffer: HandshakeHashBuffer,
    // The key schedule for sending early data.
    //
    // If the server accepts the PSK used for early data then
    // this is used to compute the rest of the key schedule.
    // Otherwise, it is thrown away.
    //
    // If this is `None` then we do not support early data.
    early_data_key_schedule: Option<KeyScheduleEarly>,
    offered_key_share: Option<Box<dyn ActiveKeyExchange>>,
    suite: Option<SupportedCipherSuite>,
    ech_state: Option<EchState>,
}

struct ExpectServerHelloOrHelloRetryRequest {
    next: ExpectServerHello,
    extra_exts: ClientExtensionsInput<'static>,
}

pub(super) struct ClientHelloInput {
    pub(super) config: Arc<ClientConfig>,
    pub(super) resuming: Option<persist::Retrieved<ClientSessionValue>>,
    pub(super) random: Random,
    pub(super) sent_tls13_fake_ccs: bool,
    pub(super) hello: ClientHelloDetails,
    pub(super) session_id: SessionId,
    pub(super) server_name: ServerName<'static>,
    pub(super) prev_ech_ext: Option<EncryptedClientHello>,
}

impl ClientHelloInput {
    pub(super) fn new(
        server_name: ServerName<'static>,
        extra_exts: &ClientExtensionsInput<'_>,
        cx: &mut ClientContext<'_>,
        config: Arc<ClientConfig>,
    ) -> Result<Self, Error> {
        let mut resuming = ClientSessionValue::retrieve(&server_name, &config, cx);
        let session_id = match &mut resuming {
            Some(_resuming) => {
                debug!("Resuming session");
                match &mut _resuming.value {
                    #[cfg(feature = "tls12")]
                    ClientSessionValue::Tls12(inner) => {
                        // If we have a ticket, we use the sessionid as a signal that
                        // we're  doing an abbreviated handshake.  See section 3.4 in
                        // RFC5077.
                        if !inner.ticket().0.is_empty() {
                            inner.session_id = SessionId::random(config.provider.secure_random)?;
                        }
                        Some(inner.session_id)
                    }
                    _ => None,
                }
            }
            _ => {
                debug!("Not resuming any session");
                None
            }
        };

        // https://tools.ietf.org/html/rfc8446#appendix-D.4
        // https://tools.ietf.org/html/draft-ietf-quic-tls-34#section-8.4
        let session_id = match session_id {
            Some(session_id) => session_id,
            None if cx.common.is_quic() => SessionId::empty(),
            None if !config.supports_version(ProtocolVersion::TLSv1_3) => SessionId::empty(),
            None => SessionId::random(config.provider.secure_random)?,
        };

        let hello = ClientHelloDetails::new(
            extra_exts
                .protocols
                .clone()
                .unwrap_or_default(),
            crate::rand::random_u16(config.provider.secure_random)?,
        );

        Ok(Self {
            resuming,
            random: Random::new(config.provider.secure_random)?,
            sent_tls13_fake_ccs: false,
            hello,
            session_id,
            server_name,
            prev_ech_ext: None,
            config,
        })
    }

    pub(super) fn start_handshake(
        self,
        extra_exts: ClientExtensionsInput<'static>,
        cx: &mut ClientContext<'_>,
    ) -> NextStateOrError<'static> {
        let mut transcript_buffer = HandshakeHashBuffer::new();
        if self
            .config
            .client_auth_cert_resolver
            .has_certs()
        {
            transcript_buffer.set_client_auth_enabled();
        }

        let key_share = if self.config.needs_key_share() {
            Some(tls13::initial_key_share(
                &self.config,
                &self.server_name,
                &mut cx.common.kx_state,
            )?)
        } else {
            None
        };

        let ech_state = match self.config.ech_mode.as_ref() {
            Some(EchMode::Enable(ech_config)) => {
                Some(ech_config.state(self.server_name.clone(), &self.config)?)
            }
            _ => None,
        };

        emit_client_hello_for_retry(
            transcript_buffer,
            None,
            key_share,
            extra_exts,
            None,
            self,
            cx,
            ech_state,
        )
    }
}

/// Emits the initial ClientHello or a ClientHello in response to
/// a HelloRetryRequest.
///
/// `retryreq` and `suite` are `None` if this is the initial
/// ClientHello.
fn emit_client_hello_for_retry(
    mut transcript_buffer: HandshakeHashBuffer,
    retryreq: Option<&HelloRetryRequest>,
    key_share: Option<Box<dyn ActiveKeyExchange>>,
    extra_exts: ClientExtensionsInput<'static>,
    suite: Option<SupportedCipherSuite>,
    mut input: ClientHelloInput,
    cx: &mut ClientContext<'_>,
    mut ech_state: Option<EchState>,
) -> NextStateOrError<'static> {
    let config = &input.config;
    // Defense in depth: the ECH state should be None if ECH is disabled based on config
    // builder semantics.
    let forbids_tls12 = cx.common.is_quic() || ech_state.is_some();

    let supported_versions = SupportedProtocolVersions {
        tls12: config.supports_version(ProtocolVersion::TLSv1_2) && !forbids_tls12,
        tls13: config.supports_version(ProtocolVersion::TLSv1_3),
    };

    // should be unreachable thanks to config builder
    assert!(supported_versions.any(|_| true));

    let mut exts = Box::new(ClientExtensions {
        // offer groups which are usable for any offered version
        named_groups: Some(
            config
                .provider
                .kx_groups
                .iter()
                .filter(|skxg| supported_versions.any(|v| skxg.usable_for_version(v)))
                .map(|skxg| skxg.name())
                .collect(),
        ),
        supported_versions: Some(supported_versions),
        signature_schemes: Some(
            config
                .verifier
                .supported_verify_schemes(),
        ),
        extended_master_secret_request: Some(()),
        certificate_status_request: Some(CertificateStatusRequest::build_ocsp()),
        protocols: extra_exts.protocols.clone(),
        ..Default::default()
    });

    match extra_exts.transport_parameters.clone() {
        Some(TransportParameters::Quic(v)) => exts.transport_parameters = Some(v),
        Some(TransportParameters::QuicDraft(v)) => exts.transport_parameters_draft = Some(v),
        None => {}
    };

    if supported_versions.tls13 {
        if let Some(cas_extension) = config.verifier.root_hint_subjects() {
            exts.certificate_authority_names = Some(cas_extension.to_owned());
        }
    }

    // Send the ECPointFormat extension only if we are proposing ECDHE
    if config
        .provider
        .kx_groups
        .iter()
        .any(|skxg| skxg.name().key_exchange_algorithm() == KeyExchangeAlgorithm::ECDHE)
    {
        exts.ec_point_formats = Some(SupportedEcPointFormats::default());
    }

    exts.server_name = match (ech_state.as_ref(), config.enable_sni) {
        // If we have ECH state we have a "cover name" to send in the outer hello
        // as the SNI domain name. This happens unconditionally so we ignore the
        // `enable_sni` value. That will be used later to decide what to do for
        // the protected inner hello's SNI.
        (Some(ech_state), _) => Some(ServerNamePayload::from(&ech_state.outer_name)),

        // If we have no ECH state, and SNI is enabled, try to use the input server_name
        // for the SNI domain name.
        (None, true) => match &input.server_name {
            ServerName::DnsName(dns_name) => Some(ServerNamePayload::from(dns_name)),
            _ => None,
        },

        // If we have no ECH state, and SNI is not enabled, there's nothing to do.
        (None, false) => None,
    };

    if let Some(key_share) = &key_share {
        debug_assert!(supported_versions.tls13);
        let mut shares = vec![KeyShareEntry::new(key_share.group(), key_share.pub_key())];

        if !retryreq
            .map(|rr| rr.key_share.is_some())
            .unwrap_or_default()
        {
            // Only for the initial client hello, or a HRR that does not specify a kx group,
            // see if we can send a second KeyShare for "free".  We only do this if the same
            // algorithm is also supported separately by our provider for this version
            // (`find_kx_group` looks that up).
            if let Some((component_group, component_share)) =
                key_share
                    .hybrid_component()
                    .filter(|(group, _)| {
                        config
                            .find_kx_group(*group, ProtocolVersion::TLSv1_3)
                            .is_some()
                    })
            {
                shares.push(KeyShareEntry::new(component_group, component_share));
            }
        }

        exts.key_shares = Some(shares);
    }

    if let Some(cookie) = retryreq.and_then(|hrr| hrr.cookie.as_ref()) {
        exts.cookie = Some(cookie.clone());
    }

    if supported_versions.tls13 {
        // We could support PSK_KE here too. Such connections don't
        // have forward secrecy, and are similar to TLS1.2 resumption.
        exts.preshared_key_modes = Some(PskKeyExchangeModes {
            psk: false,
            psk_dhe: true,
        });
    }

    input.hello.offered_cert_compression =
        if supported_versions.tls13 && !config.cert_decompressors.is_empty() {
            exts.certificate_compression_algorithms = Some(
                config
                    .cert_decompressors
                    .iter()
                    .map(|dec| dec.algorithm())
                    .collect(),
            );
            true
        } else {
            false
        };

    if config
        .client_auth_cert_resolver
        .only_raw_public_keys()
    {
        exts.client_certificate_types = Some(vec![CertificateType::RawPublicKey]);
    }

    if config
        .verifier
        .requires_raw_public_keys()
    {
        exts.server_certificate_types = Some(vec![CertificateType::RawPublicKey]);
    }

    // If this is a second client hello we're constructing in response to an HRR, and
    // we've rejected ECH or sent GREASE ECH, then we need to carry forward the
    // exact same ECH extension we used in the first hello.
    if matches!(cx.data.ech_status, EchStatus::Rejected | EchStatus::Grease) & retryreq.is_some() {
        if let Some(prev_ech_ext) = input.prev_ech_ext.take() {
            exts.encrypted_client_hello = Some(prev_ech_ext);
        }
    }

    // Do we have a SessionID or ticket cached for this host?
    let tls13_session = prepare_resumption(&input.resuming, &mut exts, suite, cx, config);

    // Extensions MAY be randomized
    // but they also need to keep the same order as the previous ClientHello
    exts.order_seed = input.hello.extension_order_seed;

    let mut cipher_suites: Vec<_> = config
        .provider
        .cipher_suites
        .iter()
        .filter_map(|cs| match cs.usable_for_protocol(cx.common.protocol) {
            true => Some(cs.suite()),
            false => None,
        })
        .collect();

    if supported_versions.tls12 {
        // We don't do renegotiation at all, in fact.
        cipher_suites.push(CipherSuite::TLS_EMPTY_RENEGOTIATION_INFO_SCSV);
    }

    let mut chp_payload = ClientHelloPayload {
        client_version: ProtocolVersion::TLSv1_2,
        random: input.random,
        session_id: input.session_id,
        cipher_suites,
        compression_methods: vec![Compression::Null],
        extensions: exts,
    };

    let ech_grease_ext = config
        .ech_mode
        .as_ref()
        .and_then(|mode| match mode {
            EchMode::Grease(cfg) => Some(cfg.grease_ext(
                config.provider.secure_random,
                input.server_name.clone(),
                &chp_payload,
            )),
            _ => None,
        });

    match (cx.data.ech_status, &mut ech_state) {
        // If we haven't offered ECH, or have offered ECH but got a non-rejecting HRR, then
        // we need to replace the client hello payload with an ECH client hello payload.
        (EchStatus::NotOffered | EchStatus::Offered, Some(ech_state)) => {
            // Replace the client hello payload with an ECH client hello payload.
            chp_payload = ech_state.ech_hello(chp_payload, retryreq, &tls13_session)?;
            cx.data.ech_status = EchStatus::Offered;
            // Store the ECH extension in case we need to carry it forward in a subsequent hello.
            input.prev_ech_ext = chp_payload
                .encrypted_client_hello
                .clone();
        }
        // If we haven't offered ECH, and have no ECH state, then consider whether to use GREASE
        // ECH.
        (EchStatus::NotOffered, None) => {
            if let Some(grease_ext) = ech_grease_ext {
                // Add the GREASE ECH extension.
                let grease_ext = grease_ext?;
                chp_payload.encrypted_client_hello = Some(grease_ext.clone());
                cx.data.ech_status = EchStatus::Grease;
                // Store the GREASE ECH extension in case we need to carry it forward in a
                // subsequent hello.
                input.prev_ech_ext = Some(grease_ext);
            }
        }
        _ => {}
    }

    // Note what extensions we sent.
    input.hello.sent_extensions = chp_payload.collect_used();

    let mut chp = HandshakeMessagePayload(HandshakePayload::ClientHello(chp_payload));

    let tls13_early_data_key_schedule = match (ech_state.as_mut(), tls13_session) {
        // If we're performing ECH and resuming, then the PSK binder will have been dealt with
        // separately, and we need to take the early_data_key_schedule computed for the inner hello.
        (Some(ech_state), Some(tls13_session)) => ech_state
            .early_data_key_schedule
            .take()
            .map(|schedule| (tls13_session.suite(), schedule)),

        // When we're not doing ECH and resuming, then the PSK binder need to be filled in as
        // normal.
        (_, Some(tls13_session)) => Some((
            tls13_session.suite(),
            tls13::fill_in_psk_binder(&tls13_session, &transcript_buffer, &mut chp),
        )),

        // No early key schedule in other cases.
        _ => None,
    };

    let ch = Message {
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
        payload: MessagePayload::handshake(chp),
    };

    if retryreq.is_some() {
        // send dummy CCS to fool middleboxes prior
        // to second client hello
        tls13::emit_fake_ccs(&mut input.sent_tls13_fake_ccs, cx.common);
    }

    trace!("Sending ClientHello {ch:#?}");

    transcript_buffer.add_message(&ch);
    cx.common.send_msg(ch, false);

    // Calculate the hash of ClientHello and use it to derive EarlyTrafficSecret
    let early_data_key_schedule =
        tls13_early_data_key_schedule.map(|(resuming_suite, schedule)| {
            if !cx.data.early_data.is_enabled() {
                return schedule;
            }

            let (transcript_buffer, random) = match &ech_state {
                // When using ECH the early data key schedule is derived based on the inner
                // hello transcript and random.
                Some(ech_state) => (
                    &ech_state.inner_hello_transcript,
                    &ech_state.inner_hello_random.0,
                ),
                None => (&transcript_buffer, &input.random.0),
            };

            tls13::derive_early_traffic_secret(
                &*config.key_log,
                cx,
                resuming_suite.common.hash_provider,
                &schedule,
                &mut input.sent_tls13_fake_ccs,
                transcript_buffer,
                random,
            );
            schedule
        });

    let next = ExpectServerHello {
        input,
        transcript_buffer,
        early_data_key_schedule,
        offered_key_share: key_share,
        suite,
        ech_state,
    };

    Ok(if supported_versions.tls13 && retryreq.is_none() {
        Box::new(ExpectServerHelloOrHelloRetryRequest {
            next,
            extra_exts: extra_exts.into_owned(),
        })
    } else {
        Box::new(next)
    })
}

/// Prepares `exts` and `cx` with TLS 1.2 or TLS 1.3 session
/// resumption.
///
/// - `suite` is `None` if this is the initial ClientHello, or
///   `Some` if we're retrying in response to
///   a HelloRetryRequest.
///
/// This function will push onto `exts` to
///
/// (a) request a new ticket if we don't have one,
/// (b) send our TLS 1.2 ticket after retrieving an 1.2 session,
/// (c) send a request for 1.3 early data if allowed and
/// (d) send a 1.3 preshared key if we have one.
///
/// It returns the TLS 1.3 PSKs, if any, for further processing.
fn prepare_resumption<'a>(
    resuming: &'a Option<persist::Retrieved<ClientSessionValue>>,
    exts: &mut ClientExtensions<'_>,
    suite: Option<SupportedCipherSuite>,
    cx: &mut ClientContext<'_>,
    config: &ClientConfig,
) -> Option<persist::Retrieved<&'a persist::Tls13ClientSessionValue>> {
    // Check whether we're resuming with a non-empty ticket.
    let resuming = match resuming {
        Some(resuming) if !resuming.ticket().is_empty() => resuming,
        _ => {
            if config.supports_version(ProtocolVersion::TLSv1_2)
                && config.resumption.tls12_resumption == Tls12Resumption::SessionIdOrTickets
            {
                // If we don't have a ticket, request one.
                exts.session_ticket = Some(ClientSessionTicket::Request);
            }
            return None;
        }
    };

    let Some(tls13) = resuming.map(|csv| csv.tls13()) else {
        // TLS 1.2; send the ticket if we have support this protocol version
        if config.supports_version(ProtocolVersion::TLSv1_2)
            && config.resumption.tls12_resumption == Tls12Resumption::SessionIdOrTickets
        {
            exts.session_ticket = Some(ClientSessionTicket::Offer(Payload::new(resuming.ticket())));
        }
        return None; // TLS 1.2, so nothing to return here
    };

    if !config.supports_version(ProtocolVersion::TLSv1_3) {
        return None;
    }

    // If the server selected TLS 1.2, we can't resume.
    let suite = match suite {
        Some(SupportedCipherSuite::Tls13(suite)) => Some(suite),
        #[cfg(feature = "tls12")]
        Some(SupportedCipherSuite::Tls12(_)) => return None,
        None => None,
    };

    // If the selected cipher suite can't select from the session's, we can't resume.
    if let Some(suite) = suite {
        suite.can_resume_from(tls13.suite())?;
    }

    tls13::prepare_resumption(config, cx, &tls13, exts, suite.is_some());
    Some(tls13)
}

pub(super) fn process_alpn_protocol(
    common: &mut CommonState,
    offered_protocols: &[ProtocolName],
    selected: Option<&ProtocolName>,
) -> Result<(), Error> {
    common.alpn_protocol = selected.map(ToOwned::to_owned);

    if let Some(alpn_protocol) = &common.alpn_protocol {
        if !offered_protocols.contains(alpn_protocol) {
            return Err(common.send_fatal_alert(
                AlertDescription::IllegalParameter,
                PeerMisbehaved::SelectedUnofferedApplicationProtocol,
            ));
        }
    }

    // RFC 9001 says: "While ALPN only specifies that servers use this alert, QUIC clients MUST
    // use error 0x0178 to terminate a connection when ALPN negotiation fails." We judge that
    // the user intended to use ALPN (rather than some out-of-band protocol negotiation
    // mechanism) if and only if any ALPN protocols were configured. This defends against badly-behaved
    // servers which accept a connection that requires an application-layer protocol they do not
    // understand.
    if common.is_quic() && common.alpn_protocol.is_none() && !offered_protocols.is_empty() {
        return Err(common.send_fatal_alert(
            AlertDescription::NoApplicationProtocol,
            Error::NoApplicationProtocol,
        ));
    }

    debug!(
        "ALPN protocol is {:?}",
        common
            .alpn_protocol
            .as_ref()
            .map(|v| bs_debug::BsDebug(v.as_ref()))
    );
    Ok(())
}

pub(super) fn process_server_cert_type_extension(
    common: &mut CommonState,
    config: &ClientConfig,
    server_cert_extension: Option<&CertificateType>,
) -> Result<Option<(ExtensionType, CertificateType)>, Error> {
    process_cert_type_extension(
        common,
        config
            .verifier
            .requires_raw_public_keys(),
        server_cert_extension.copied(),
        ExtensionType::ServerCertificateType,
    )
}

pub(super) fn process_client_cert_type_extension(
    common: &mut CommonState,
    config: &ClientConfig,
    client_cert_extension: Option<&CertificateType>,
) -> Result<Option<(ExtensionType, CertificateType)>, Error> {
    process_cert_type_extension(
        common,
        config
            .client_auth_cert_resolver
            .only_raw_public_keys(),
        client_cert_extension.copied(),
        ExtensionType::ClientCertificateType,
    )
}

impl State<ClientConnectionData> for ExpectServerHello {
    fn handle<'m>(
        mut self: Box<Self>,
        cx: &mut ClientContext<'_>,
        m: Message<'m>,
    ) -> NextStateOrError<'m>
    where
        Self: 'm,
    {
        let server_hello =
            require_handshake_msg!(m, HandshakeType::ServerHello, HandshakePayload::ServerHello)?;
        trace!("We got ServerHello {server_hello:#?}");

        use crate::ProtocolVersion::{TLSv1_2, TLSv1_3};
        let config = &self.input.config;
        let tls13_supported = config.supports_version(TLSv1_3);

        let server_version = if server_hello.legacy_version == TLSv1_2 {
            server_hello
                .selected_version
                .unwrap_or(server_hello.legacy_version)
        } else {
            server_hello.legacy_version
        };

        let version = match server_version {
            TLSv1_3 if tls13_supported => TLSv1_3,
            TLSv1_2 if config.supports_version(TLSv1_2) => {
                if cx.data.early_data.is_enabled() && cx.common.early_traffic {
                    // The client must fail with a dedicated error code if the server
                    // responds with TLS 1.2 when offering 0-RTT.
                    return Err(PeerMisbehaved::OfferedEarlyDataWithOldProtocolVersion.into());
                }

                if server_hello.selected_version.is_some() {
                    return Err({
                        cx.common.send_fatal_alert(
                            AlertDescription::IllegalParameter,
                            PeerMisbehaved::SelectedTls12UsingTls13VersionExtension,
                        )
                    });
                }

                TLSv1_2
            }
            _ => {
                let reason = match server_version {
                    TLSv1_2 | TLSv1_3 => PeerIncompatible::ServerTlsVersionIsDisabledByOurConfig,
                    _ => PeerIncompatible::ServerDoesNotSupportTls12Or13,
                };
                return Err(cx
                    .common
                    .send_fatal_alert(AlertDescription::ProtocolVersion, reason));
            }
        };

        if server_hello.compression_method != Compression::Null {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::SelectedUnofferedCompression,
                )
            });
        }

        let allowed_unsolicited = [ExtensionType::RenegotiationInfo];
        if self
            .input
            .hello
            .server_sent_unsolicited_extensions(server_hello, &allowed_unsolicited)
        {
            return Err(cx.common.send_fatal_alert(
                AlertDescription::UnsupportedExtension,
                PeerMisbehaved::UnsolicitedServerHelloExtension,
            ));
        }

        cx.common.negotiated_version = Some(version);

        // Extract ALPN protocol
        if !cx.common.is_tls13() {
            process_alpn_protocol(
                cx.common,
                &self.input.hello.alpn_protocols,
                server_hello
                    .selected_protocol
                    .as_ref()
                    .map(|s| s.as_ref()),
            )?;
        }

        // If ECPointFormats extension is supplied by the server, it must contain
        // Uncompressed.  But it's allowed to be omitted.
        if let Some(point_fmts) = &server_hello.ec_point_formats {
            if !point_fmts.uncompressed {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::HandshakeFailure,
                    PeerMisbehaved::ServerHelloMustOfferUncompressedEcPoints,
                ));
            }
        }

        let suite = config
            .find_cipher_suite(server_hello.cipher_suite)
            .ok_or_else(|| {
                cx.common.send_fatal_alert(
                    AlertDescription::HandshakeFailure,
                    PeerMisbehaved::SelectedUnofferedCipherSuite,
                )
            })?;

        if version != suite.version().version {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::SelectedUnusableCipherSuiteForVersion,
                )
            });
        }

        match self.suite {
            Some(prev_suite) if prev_suite != suite => {
                return Err({
                    cx.common.send_fatal_alert(
                        AlertDescription::IllegalParameter,
                        PeerMisbehaved::SelectedDifferentCipherSuiteAfterRetry,
                    )
                });
            }
            _ => {
                debug!("Using ciphersuite {suite:?}");
                self.suite = Some(suite);
                cx.common.suite = Some(suite);
            }
        }

        // Start our handshake hash, and input the server-hello.
        let mut transcript = self
            .transcript_buffer
            .start_hash(suite.hash_provider());
        transcript.add_message(&m);

        let randoms = ConnectionRandoms::new(self.input.random, server_hello.random);
        // For TLS1.3, start message encryption using
        // handshake_traffic_secret.
        match suite {
            SupportedCipherSuite::Tls13(suite) => {
                tls13::handle_server_hello(
                    cx,
                    server_hello,
                    randoms,
                    suite,
                    transcript,
                    self.early_data_key_schedule,
                    // We always send a key share when TLS 1.3 is enabled.
                    self.offered_key_share.unwrap(),
                    &m,
                    self.ech_state,
                    self.input,
                )
            }
            #[cfg(feature = "tls12")]
            SupportedCipherSuite::Tls12(suite) => tls12::CompleteServerHelloHandling {
                randoms,
                transcript,
                input: self.input,
            }
            .handle_server_hello(cx, suite, server_hello, tls13_supported),
        }
    }

    fn into_owned(self: Box<Self>) -> NextState<'static> {
        self
    }
}

impl ExpectServerHelloOrHelloRetryRequest {
    fn into_expect_server_hello(self) -> NextState<'static> {
        Box::new(self.next)
    }

    fn handle_hello_retry_request(
        mut self,
        cx: &mut ClientContext<'_>,
        m: Message<'_>,
    ) -> NextStateOrError<'static> {
        let hrr = require_handshake_msg!(
            m,
            HandshakeType::HelloRetryRequest,
            HandshakePayload::HelloRetryRequest
        )?;
        trace!("Got HRR {hrr:?}");

        cx.common.check_aligned_handshake()?;

        // We always send a key share when TLS 1.3 is enabled.
        let offered_key_share = self.next.offered_key_share.unwrap();

        // A retry request is illegal if it contains no cookie and asks for
        // retry of a group we already sent.
        let config = &self.next.input.config;

        if let (None, Some(req_group)) = (&hrr.cookie, hrr.key_share) {
            let offered_hybrid = offered_key_share
                .hybrid_component()
                .and_then(|(group_name, _)| {
                    config.find_kx_group(group_name, ProtocolVersion::TLSv1_3)
                })
                .map(|skxg| skxg.name());

            if req_group == offered_key_share.group() || Some(req_group) == offered_hybrid {
                return Err({
                    cx.common.send_fatal_alert(
                        AlertDescription::IllegalParameter,
                        PeerMisbehaved::IllegalHelloRetryRequestWithOfferedGroup,
                    )
                });
            }
        }

        // Or has an empty cookie.
        if let Some(cookie) = &hrr.cookie {
            if cookie.0.is_empty() {
                return Err({
                    cx.common.send_fatal_alert(
                        AlertDescription::IllegalParameter,
                        PeerMisbehaved::IllegalHelloRetryRequestWithEmptyCookie,
                    )
                });
            }
        }

        // Or asks us to change nothing.
        if hrr.cookie.is_none() && hrr.key_share.is_none() {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::IllegalHelloRetryRequestWithNoChanges,
                )
            });
        }

        // Or does not echo the session_id from our ClientHello:
        //
        // > the HelloRetryRequest has the same format as a ServerHello message,
        // > and the legacy_version, legacy_session_id_echo, cipher_suite, and
        // > legacy_compression_method fields have the same meaning
        // <https://www.rfc-editor.org/rfc/rfc8446#section-4.1.4>
        //
        // and
        //
        // > A client which receives a legacy_session_id_echo field that does not
        // > match what it sent in the ClientHello MUST abort the handshake with an
        // > "illegal_parameter" alert.
        // <https://www.rfc-editor.org/rfc/rfc8446#section-4.1.3>
        if hrr.session_id != self.next.input.session_id {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::IllegalHelloRetryRequestWithWrongSessionId,
                )
            });
        }

        // Or asks us to talk a protocol we didn't offer, or doesn't support HRR at all.
        match hrr.supported_versions {
            Some(ProtocolVersion::TLSv1_3) => {
                cx.common.negotiated_version = Some(ProtocolVersion::TLSv1_3);
            }
            _ => {
                return Err({
                    cx.common.send_fatal_alert(
                        AlertDescription::IllegalParameter,
                        PeerMisbehaved::IllegalHelloRetryRequestWithUnsupportedVersion,
                    )
                });
            }
        }

        // Or asks us to use a ciphersuite we didn't offer.
        let Some(cs) = config.find_cipher_suite(hrr.cipher_suite) else {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::IllegalHelloRetryRequestWithUnofferedCipherSuite,
                )
            });
        };

        // Or offers ECH related extensions when we didn't offer ECH.
        if cx.data.ech_status == EchStatus::NotOffered && hrr.encrypted_client_hello.is_some() {
            return Err({
                cx.common.send_fatal_alert(
                    AlertDescription::UnsupportedExtension,
                    PeerMisbehaved::IllegalHelloRetryRequestWithInvalidEch,
                )
            });
        }

        // HRR selects the ciphersuite.
        cx.common.suite = Some(cs);
        cx.common.handshake_kind = Some(HandshakeKind::FullWithHelloRetryRequest);

        // If we offered ECH, we need to confirm that the server accepted it.
        match (self.next.ech_state.as_ref(), cs.tls13()) {
            (Some(ech_state), Some(tls13_cs)) => {
                if !ech_state.confirm_hrr_acceptance(hrr, tls13_cs, cx.common)? {
                    // If the server did not confirm, then note the new ECH status but
                    // continue the handshake. We will abort with an ECH required error
                    // at the end.
                    cx.data.ech_status = EchStatus::Rejected;
                }
            }
            (Some(_), None) => {
                unreachable!("ECH state should only be set when TLS 1.3 was negotiated")
            }
            _ => {}
        };

        // This is the draft19 change where the transcript became a tree
        let transcript = self
            .next
            .transcript_buffer
            .start_hash(cs.hash_provider());
        let mut transcript_buffer = transcript.into_hrr_buffer();
        transcript_buffer.add_message(&m);

        // If we offered ECH and the server accepted, we also need to update the separate
        // ECH transcript with the hello retry request message.
        if let Some(ech_state) = self.next.ech_state.as_mut() {
            ech_state.transcript_hrr_update(cs.hash_provider(), &m);
        }

        // Early data is not allowed after HelloRetryrequest
        if cx.data.early_data.is_enabled() {
            cx.data.early_data.rejected();
        }

        let key_share = match hrr.key_share {
            Some(group) if group != offered_key_share.group() => {
                let Some(skxg) = config.find_kx_group(group, ProtocolVersion::TLSv1_3) else {
                    return Err(cx.common.send_fatal_alert(
                        AlertDescription::IllegalParameter,
                        PeerMisbehaved::IllegalHelloRetryRequestWithUnofferedNamedGroup,
                    ));
                };

                cx.common.kx_state = KxState::Start(skxg);
                skxg.start()?
            }
            _ => offered_key_share,
        };

        emit_client_hello_for_retry(
            transcript_buffer,
            Some(hrr),
            Some(key_share),
            self.extra_exts,
            Some(cs),
            self.next.input,
            cx,
            self.next.ech_state,
        )
    }
}

impl State<ClientConnectionData> for ExpectServerHelloOrHelloRetryRequest {
    fn handle<'m>(
        self: Box<Self>,
        cx: &mut ClientContext<'_>,
        m: Message<'m>,
    ) -> NextStateOrError<'m>
    where
        Self: 'm,
    {
        match m.payload {
            MessagePayload::Handshake {
                parsed: HandshakeMessagePayload(HandshakePayload::ServerHello(..)),
                ..
            } => self
                .into_expect_server_hello()
                .handle(cx, m),
            MessagePayload::Handshake {
                parsed: HandshakeMessagePayload(HandshakePayload::HelloRetryRequest(..)),
                ..
            } => self.handle_hello_retry_request(cx, m),
            payload => Err(inappropriate_handshake_message(
                &payload,
                &[ContentType::Handshake],
                &[HandshakeType::ServerHello, HandshakeType::HelloRetryRequest],
            )),
        }
    }

    fn into_owned(self: Box<Self>) -> NextState<'static> {
        self
    }
}

fn process_cert_type_extension(
    common: &mut CommonState,
    client_expects: bool,
    server_negotiated: Option<CertificateType>,
    extension_type: ExtensionType,
) -> Result<Option<(ExtensionType, CertificateType)>, Error> {
    match (client_expects, server_negotiated) {
        (true, Some(CertificateType::RawPublicKey)) => {
            Ok(Some((extension_type, CertificateType::RawPublicKey)))
        }
        (true, _) => Err(common.send_fatal_alert(
            AlertDescription::HandshakeFailure,
            Error::PeerIncompatible(PeerIncompatible::IncorrectCertificateTypeExtension),
        )),
        (_, Some(CertificateType::RawPublicKey)) => {
            unreachable!("Caught by `PeerMisbehaved::UnsolicitedEncryptedExtension`")
        }
        (_, _) => Ok(None),
    }
}

pub(super) enum ClientSessionValue {
    Tls13(persist::Tls13ClientSessionValue),
    #[cfg(feature = "tls12")]
    Tls12(persist::Tls12ClientSessionValue),
}

impl ClientSessionValue {
    fn retrieve(
        server_name: &ServerName<'static>,
        config: &ClientConfig,
        cx: &mut ClientContext<'_>,
    ) -> Option<persist::Retrieved<Self>> {
        let found = config
            .resumption
            .store
            .take_tls13_ticket(server_name)
            .map(ClientSessionValue::Tls13)
            .or_else(|| {
                #[cfg(feature = "tls12")]
                {
                    config
                        .resumption
                        .store
                        .tls12_session(server_name)
                        .map(ClientSessionValue::Tls12)
                }

                #[cfg(not(feature = "tls12"))]
                None
            })
            .and_then(|resuming| {
                resuming.compatible_config(&config.verifier, &config.client_auth_cert_resolver)
            })
            .and_then(|resuming| {
                let now = config
                    .current_time()
                    .map_err(|_err| debug!("Could not get current time: {_err}"))
                    .ok()?;

                let retrieved = persist::Retrieved::new(resuming, now);
                match retrieved.has_expired() {
                    false => Some(retrieved),
                    true => None,
                }
            })
            .or_else(|| {
                debug!("No cached session for {server_name:?}");
                None
            });

        if let Some(resuming) = &found {
            if cx.common.is_quic() {
                cx.common.quic.params = resuming
                    .tls13()
                    .map(|v| v.quic_params());
            }
        }

        found
    }

    fn common(&self) -> &persist::ClientSessionCommon {
        match self {
            Self::Tls13(inner) => &inner.common,
            #[cfg(feature = "tls12")]
            Self::Tls12(inner) => &inner.common,
        }
    }

    fn tls13(&self) -> Option<&persist::Tls13ClientSessionValue> {
        match self {
            Self::Tls13(v) => Some(v),
            #[cfg(feature = "tls12")]
            Self::Tls12(_) => None,
        }
    }

    fn compatible_config(
        self,
        server_cert_verifier: &Arc<dyn ServerCertVerifier>,
        client_creds: &Arc<dyn ResolvesClientCert>,
    ) -> Option<Self> {
        match &self {
            Self::Tls13(v) => v
                .compatible_config(server_cert_verifier, client_creds)
                .then_some(self),
            #[cfg(feature = "tls12")]
            Self::Tls12(v) => v
                .compatible_config(server_cert_verifier, client_creds)
                .then_some(self),
        }
    }
}

impl Deref for ClientSessionValue {
    type Target = persist::ClientSessionCommon;

    fn deref(&self) -> &Self::Target {
        self.common()
    }
}
