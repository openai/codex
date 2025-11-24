use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

pub(super) use client_hello::CompleteClientHelloHandling;
use pki_types::UnixTime;
use subtle::ConstantTimeEq;

use super::common::ActiveCertifiedKey;
use super::hs::{self, ServerContext};
use super::server_conn::{ProducesTickets, ServerConfig, ServerConnectionData};
use crate::check::inappropriate_message;
use crate::common_state::{CommonState, HandshakeFlightTls12, HandshakeKind, Side, State};
use crate::conn::ConnectionRandoms;
use crate::conn::kernel::{Direction, KernelContext, KernelState};
use crate::crypto::ActiveKeyExchange;
use crate::enums::{AlertDescription, ContentType, HandshakeType, ProtocolVersion};
use crate::error::{Error, PeerIncompatible, PeerMisbehaved};
use crate::hash_hs::HandshakeHash;
use crate::log::{debug, trace};
use crate::msgs::base::Payload;
use crate::msgs::ccs::ChangeCipherSpecPayload;
use crate::msgs::codec::Codec;
use crate::msgs::handshake::{
    CertificateChain, ClientKeyExchangeParams, HandshakeMessagePayload, HandshakePayload,
    NewSessionTicketPayload, NewSessionTicketPayloadTls13, SessionId,
};
use crate::msgs::message::{Message, MessagePayload};
use crate::msgs::persist;
use crate::suites::PartiallyExtractedSecrets;
use crate::sync::Arc;
use crate::tls12::{self, ConnectionSecrets, Tls12CipherSuite};
use crate::{ConnectionTrafficSecrets, verify};

mod client_hello {
    use pki_types::CertificateDer;

    use super::*;
    use crate::common_state::KxState;
    use crate::crypto::SupportedKxGroup;
    use crate::enums::SignatureScheme;
    use crate::msgs::enums::{ClientCertificateType, Compression};
    use crate::msgs::handshake::{
        CertificateRequestPayload, CertificateStatus, ClientHelloPayload, ClientSessionTicket,
        Random, ServerExtensionsInput, ServerHelloPayload, ServerKeyExchange,
        ServerKeyExchangeParams, ServerKeyExchangePayload,
    };
    use crate::sign;
    use crate::verify::DigitallySignedStruct;

    pub(in crate::server) struct CompleteClientHelloHandling {
        pub(in crate::server) config: Arc<ServerConfig>,
        pub(in crate::server) transcript: HandshakeHash,
        pub(in crate::server) session_id: SessionId,
        pub(in crate::server) suite: &'static Tls12CipherSuite,
        pub(in crate::server) using_ems: bool,
        pub(in crate::server) randoms: ConnectionRandoms,
        pub(in crate::server) send_ticket: bool,
        pub(in crate::server) extra_exts: ServerExtensionsInput<'static>,
    }

    impl CompleteClientHelloHandling {
        pub(in crate::server) fn handle_client_hello(
            mut self,
            cx: &mut ServerContext<'_>,
            server_key: ActiveCertifiedKey<'_>,
            chm: &Message<'_>,
            client_hello: &ClientHelloPayload,
            selected_kxg: &'static dyn SupportedKxGroup,
            sigschemes_ext: Vec<SignatureScheme>,
            tls13_enabled: bool,
        ) -> hs::NextStateOrError<'static> {
            // -- TLS1.2 only from hereon in --
            self.transcript.add_message(chm);

            if client_hello
                .extended_master_secret_request
                .is_some()
            {
                self.using_ems = true;
            } else if self.config.require_ems {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::HandshakeFailure,
                    PeerIncompatible::ExtendedMasterSecretExtensionRequired,
                ));
            }

            // "RFC 4492 specified that if this extension is missing,
            // it means that only the uncompressed point format is
            // supported"
            // - <https://datatracker.ietf.org/doc/html/rfc8422#section-5.1.2>
            let supported_ec_point_formats = client_hello
                .ec_point_formats
                .unwrap_or_default();

            trace!("ecpoints {supported_ec_point_formats:?}");

            if !supported_ec_point_formats.uncompressed {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerIncompatible::UncompressedEcPointsRequired,
                ));
            }

            // -- If TLS1.3 is enabled, signal the downgrade in the server random
            if tls13_enabled {
                self.randoms.server[24..].copy_from_slice(&tls12::DOWNGRADE_SENTINEL);
            }

            // -- Check for resumption --
            // We can do this either by (in order of preference):
            // 1. receiving a ticket that decrypts
            // 2. receiving a sessionid that is in our cache
            //
            // If we receive a ticket, the sessionid won't be in our
            // cache, so don't check.
            //
            // If either works, we end up with a ServerConnectionValue
            // which is passed to start_resumption and concludes
            // our handling of the ClientHello.
            //
            let mut ticket_received = false;
            let resume_data = client_hello
                .session_ticket
                .as_ref()
                .and_then(|ticket_ext| match ticket_ext {
                    ClientSessionTicket::Offer(ticket) => Some(ticket),
                    _ => None,
                })
                .and_then(|ticket| {
                    ticket_received = true;
                    debug!("Ticket received");
                    let data = self
                        .config
                        .ticketer
                        .decrypt(ticket.bytes());
                    if data.is_none() {
                        debug!("Ticket didn't decrypt");
                    }
                    data
                })
                .or_else(|| {
                    // Perhaps resume?  If we received a ticket, the sessionid
                    // does not correspond to a real session.
                    if client_hello.session_id.is_empty() || ticket_received {
                        return None;
                    }

                    self.config
                        .session_storage
                        .get(client_hello.session_id.as_ref())
                })
                .and_then(|x| persist::ServerSessionValue::read_bytes(&x).ok())
                .filter(|resumedata| {
                    hs::can_resume(self.suite.into(), &cx.data.sni, self.using_ems, resumedata)
                });

            if let Some(data) = resume_data {
                return self.start_resumption(cx, client_hello, &client_hello.session_id, data);
            }

            // Now we have chosen a ciphersuite, we can make kx decisions.
            let sigschemes = self
                .suite
                .resolve_sig_schemes(&sigschemes_ext);

            if sigschemes.is_empty() {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::HandshakeFailure,
                    PeerIncompatible::NoSignatureSchemesInCommon,
                ));
            }

            let mut ocsp_response = server_key.get_ocsp();

            // If we're not offered a ticket or a potential session ID, allocate a session ID.
            if !self.config.session_storage.can_cache() {
                self.session_id = SessionId::empty();
            } else if self.session_id.is_empty() && !ticket_received {
                self.session_id = SessionId::random(self.config.provider.secure_random)?;
            }

            cx.common.kx_state = KxState::Start(selected_kxg);
            cx.common.handshake_kind = Some(HandshakeKind::Full);

            let mut flight = HandshakeFlightTls12::new(&mut self.transcript);

            self.send_ticket = emit_server_hello(
                &mut flight,
                &self.config,
                cx,
                self.session_id,
                self.suite,
                self.using_ems,
                &mut ocsp_response,
                client_hello,
                None,
                &self.randoms,
                self.extra_exts,
            )?;
            emit_certificate(&mut flight, server_key.get_cert());
            if let Some(ocsp_response) = ocsp_response {
                emit_cert_status(&mut flight, ocsp_response);
            }
            let server_kx = emit_server_kx(
                &mut flight,
                sigschemes,
                selected_kxg,
                server_key.get_key(),
                &self.randoms,
            )?;
            let doing_client_auth = emit_certificate_req(&mut flight, &self.config)?;
            emit_server_hello_done(&mut flight);

            flight.finish(cx.common);

            if doing_client_auth {
                Ok(Box::new(ExpectCertificate {
                    config: self.config,
                    transcript: self.transcript,
                    randoms: self.randoms,
                    session_id: self.session_id,
                    suite: self.suite,
                    using_ems: self.using_ems,
                    server_kx,
                    send_ticket: self.send_ticket,
                }))
            } else {
                Ok(Box::new(ExpectClientKx {
                    config: self.config,
                    transcript: self.transcript,
                    randoms: self.randoms,
                    session_id: self.session_id,
                    suite: self.suite,
                    using_ems: self.using_ems,
                    server_kx,
                    client_cert: None,
                    send_ticket: self.send_ticket,
                }))
            }
        }

        fn start_resumption(
            mut self,
            cx: &mut ServerContext<'_>,
            client_hello: &ClientHelloPayload,
            id: &SessionId,
            resumedata: persist::ServerSessionValue,
        ) -> hs::NextStateOrError<'static> {
            debug!("Resuming connection");

            if resumedata.extended_ms && !self.using_ems {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::IllegalParameter,
                    PeerMisbehaved::ResumptionAttemptedWithVariedEms,
                ));
            }

            self.session_id = *id;
            let mut flight = HandshakeFlightTls12::new(&mut self.transcript);
            self.send_ticket = emit_server_hello(
                &mut flight,
                &self.config,
                cx,
                self.session_id,
                self.suite,
                self.using_ems,
                &mut None,
                client_hello,
                Some(&resumedata),
                &self.randoms,
                self.extra_exts,
            )?;
            flight.finish(cx.common);

            let secrets = ConnectionSecrets::new_resume(
                self.randoms,
                self.suite,
                &resumedata.master_secret.0,
            );
            self.config.key_log.log(
                "CLIENT_RANDOM",
                &secrets.randoms.client,
                &secrets.master_secret,
            );
            cx.common
                .start_encryption_tls12(&secrets, Side::Server);
            cx.common.peer_certificates = resumedata.client_cert_chain;
            cx.common.handshake_kind = Some(HandshakeKind::Resumed);

            if self.send_ticket {
                let now = self.config.current_time()?;

                emit_ticket(
                    &secrets,
                    &mut self.transcript,
                    self.using_ems,
                    cx,
                    &*self.config.ticketer,
                    now,
                )?;
            }
            emit_ccs(cx.common);
            cx.common
                .record_layer
                .start_encrypting();
            emit_finished(&secrets, &mut self.transcript, cx.common);

            Ok(Box::new(ExpectCcs {
                config: self.config,
                secrets,
                transcript: self.transcript,
                session_id: self.session_id,
                using_ems: self.using_ems,
                resuming: true,
                send_ticket: self.send_ticket,
            }))
        }
    }

    fn emit_server_hello(
        flight: &mut HandshakeFlightTls12<'_>,
        config: &ServerConfig,
        cx: &mut ServerContext<'_>,
        session_id: SessionId,
        suite: &'static Tls12CipherSuite,
        using_ems: bool,
        ocsp_response: &mut Option<&[u8]>,
        hello: &ClientHelloPayload,
        resumedata: Option<&persist::ServerSessionValue>,
        randoms: &ConnectionRandoms,
        extra_exts: ServerExtensionsInput<'static>,
    ) -> Result<bool, Error> {
        let mut ep = hs::ExtensionProcessing::new(extra_exts);
        ep.process_common(config, cx, ocsp_response, hello, resumedata)?;
        ep.process_tls12(config, hello, using_ems);

        let sh = HandshakeMessagePayload(HandshakePayload::ServerHello(ServerHelloPayload {
            legacy_version: ProtocolVersion::TLSv1_2,
            random: Random::from(randoms.server),
            session_id,
            cipher_suite: suite.common.suite,
            compression_method: Compression::Null,
            extensions: ep.extensions,
        }));
        trace!("sending server hello {sh:?}");
        flight.add(sh);

        Ok(ep.send_ticket)
    }

    fn emit_certificate(
        flight: &mut HandshakeFlightTls12<'_>,
        cert_chain: &[CertificateDer<'static>],
    ) {
        flight.add(HandshakeMessagePayload(HandshakePayload::Certificate(
            CertificateChain(cert_chain.to_vec()),
        )));
    }

    fn emit_cert_status(flight: &mut HandshakeFlightTls12<'_>, ocsp: &[u8]) {
        flight.add(HandshakeMessagePayload(
            HandshakePayload::CertificateStatus(CertificateStatus::new(ocsp)),
        ));
    }

    fn emit_server_kx(
        flight: &mut HandshakeFlightTls12<'_>,
        sigschemes: Vec<SignatureScheme>,
        selected_group: &'static dyn SupportedKxGroup,
        signing_key: &dyn sign::SigningKey,
        randoms: &ConnectionRandoms,
    ) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let kx = selected_group.start()?;
        let kx_params = ServerKeyExchangeParams::new(&*kx);

        let mut msg = Vec::new();
        msg.extend(randoms.client);
        msg.extend(randoms.server);
        kx_params.encode(&mut msg);

        let signer = signing_key
            .choose_scheme(&sigschemes)
            .ok_or_else(|| Error::General("incompatible signing key".to_string()))?;
        let sigscheme = signer.scheme();
        let sig = signer.sign(&msg)?;

        let skx = ServerKeyExchangePayload::from(ServerKeyExchange {
            params: kx_params,
            dss: DigitallySignedStruct::new(sigscheme, sig),
        });

        flight.add(HandshakeMessagePayload(
            HandshakePayload::ServerKeyExchange(skx),
        ));
        Ok(kx)
    }

    fn emit_certificate_req(
        flight: &mut HandshakeFlightTls12<'_>,
        config: &ServerConfig,
    ) -> Result<bool, Error> {
        let client_auth = &config.verifier;

        if !client_auth.offer_client_auth() {
            return Ok(false);
        }

        let verify_schemes = client_auth.supported_verify_schemes();

        let names = config
            .verifier
            .root_hint_subjects()
            .to_vec();

        let cr = CertificateRequestPayload {
            certtypes: vec![
                ClientCertificateType::RSASign,
                ClientCertificateType::ECDSASign,
            ],
            sigschemes: verify_schemes,
            canames: names,
        };

        let creq = HandshakeMessagePayload(HandshakePayload::CertificateRequest(cr));

        trace!("Sending CertificateRequest {creq:?}");
        flight.add(creq);
        Ok(true)
    }

    fn emit_server_hello_done(flight: &mut HandshakeFlightTls12<'_>) {
        flight.add(HandshakeMessagePayload(HandshakePayload::ServerHelloDone));
    }
}

// --- Process client's Certificate for client auth ---
struct ExpectCertificate {
    config: Arc<ServerConfig>,
    transcript: HandshakeHash,
    randoms: ConnectionRandoms,
    session_id: SessionId,
    suite: &'static Tls12CipherSuite,
    using_ems: bool,
    server_kx: Box<dyn ActiveKeyExchange>,
    send_ticket: bool,
}

impl State<ServerConnectionData> for ExpectCertificate {
    fn handle<'m>(
        mut self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        self.transcript.add_message(&m);
        let cert_chain = require_handshake_msg_move!(
            m,
            HandshakeType::Certificate,
            HandshakePayload::Certificate
        )?;

        // If we can't determine if the auth is mandatory, abort
        let mandatory = self
            .config
            .verifier
            .client_auth_mandatory();

        trace!("certs {cert_chain:?}");

        let client_cert = match cert_chain.split_first() {
            None if mandatory => {
                return Err(cx.common.send_fatal_alert(
                    AlertDescription::CertificateRequired,
                    Error::NoCertificatesPresented,
                ));
            }
            None => {
                debug!("client auth requested but no certificate supplied");
                self.transcript.abandon_client_auth();
                None
            }
            Some((end_entity, intermediates)) => {
                let now = self.config.current_time()?;

                self.config
                    .verifier
                    .verify_client_cert(end_entity, intermediates, now)
                    .map_err(|err| {
                        cx.common
                            .send_cert_verify_error_alert(err)
                    })?;

                Some(cert_chain)
            }
        };

        Ok(Box::new(ExpectClientKx {
            config: self.config,
            transcript: self.transcript,
            randoms: self.randoms,
            session_id: self.session_id,
            suite: self.suite,
            using_ems: self.using_ems,
            server_kx: self.server_kx,
            client_cert,
            send_ticket: self.send_ticket,
        }))
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        self
    }
}

// --- Process client's KeyExchange ---
struct ExpectClientKx<'a> {
    config: Arc<ServerConfig>,
    transcript: HandshakeHash,
    randoms: ConnectionRandoms,
    session_id: SessionId,
    suite: &'static Tls12CipherSuite,
    using_ems: bool,
    server_kx: Box<dyn ActiveKeyExchange>,
    client_cert: Option<CertificateChain<'a>>,
    send_ticket: bool,
}

impl State<ServerConnectionData> for ExpectClientKx<'_> {
    fn handle<'m>(
        mut self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        let client_kx = require_handshake_msg!(
            m,
            HandshakeType::ClientKeyExchange,
            HandshakePayload::ClientKeyExchange
        )?;
        self.transcript.add_message(&m);
        let ems_seed = self
            .using_ems
            .then(|| self.transcript.current_hash());

        // Complete key agreement, and set up encryption with the
        // resulting premaster secret.
        let peer_kx_params = tls12::decode_kx_params::<ClientKeyExchangeParams>(
            self.suite.kx,
            cx.common,
            client_kx.bytes(),
        )?;
        let secrets = ConnectionSecrets::from_key_exchange(
            self.server_kx,
            peer_kx_params.pub_key(),
            ems_seed,
            self.randoms,
            self.suite,
        )
        .map_err(|err| {
            cx.common
                .send_fatal_alert(AlertDescription::IllegalParameter, err)
        })?;
        cx.common.kx_state.complete();

        self.config.key_log.log(
            "CLIENT_RANDOM",
            &secrets.randoms.client,
            &secrets.master_secret,
        );
        cx.common
            .start_encryption_tls12(&secrets, Side::Server);

        match self.client_cert {
            Some(client_cert) => Ok(Box::new(ExpectCertificateVerify {
                config: self.config,
                secrets,
                transcript: self.transcript,
                session_id: self.session_id,
                using_ems: self.using_ems,
                client_cert,
                send_ticket: self.send_ticket,
            })),
            _ => Ok(Box::new(ExpectCcs {
                config: self.config,
                secrets,
                transcript: self.transcript,
                session_id: self.session_id,
                using_ems: self.using_ems,
                resuming: false,
                send_ticket: self.send_ticket,
            })),
        }
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        Box::new(ExpectClientKx {
            config: self.config,
            transcript: self.transcript,
            randoms: self.randoms,
            session_id: self.session_id,
            suite: self.suite,
            using_ems: self.using_ems,
            server_kx: self.server_kx,
            client_cert: self
                .client_cert
                .map(|cert| cert.into_owned()),
            send_ticket: self.send_ticket,
        })
    }
}

// --- Process client's certificate proof ---
struct ExpectCertificateVerify<'a> {
    config: Arc<ServerConfig>,
    secrets: ConnectionSecrets,
    transcript: HandshakeHash,
    session_id: SessionId,
    using_ems: bool,
    client_cert: CertificateChain<'a>,
    send_ticket: bool,
}

impl State<ServerConnectionData> for ExpectCertificateVerify<'_> {
    fn handle<'m>(
        mut self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        let rc = {
            let sig = require_handshake_msg!(
                m,
                HandshakeType::CertificateVerify,
                HandshakePayload::CertificateVerify
            )?;

            match self.transcript.take_handshake_buf() {
                Some(msgs) => {
                    let certs = &self.client_cert;
                    self.config
                        .verifier
                        .verify_tls12_signature(&msgs, &certs[0], sig)
                }
                None => {
                    // This should be unreachable; the handshake buffer was initialized with
                    // client authentication if the verifier wants to offer it.
                    // `transcript.abandon_client_auth()` can extract it, but its only caller in
                    // this flow will also set `ExpectClientKx::client_cert` to `None`, making it
                    // impossible to reach this state.
                    return Err(cx.common.send_fatal_alert(
                        AlertDescription::AccessDenied,
                        Error::General("client authentication not set up".into()),
                    ));
                }
            }
        };

        if let Err(e) = rc {
            return Err(cx
                .common
                .send_cert_verify_error_alert(e));
        }

        trace!("client CertificateVerify OK");
        cx.common.peer_certificates = Some(self.client_cert.into_owned());

        self.transcript.add_message(&m);
        Ok(Box::new(ExpectCcs {
            config: self.config,
            secrets: self.secrets,
            transcript: self.transcript,
            session_id: self.session_id,
            using_ems: self.using_ems,
            resuming: false,
            send_ticket: self.send_ticket,
        }))
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        Box::new(ExpectCertificateVerify {
            config: self.config,
            secrets: self.secrets,
            transcript: self.transcript,
            session_id: self.session_id,
            using_ems: self.using_ems,
            client_cert: self.client_cert.into_owned(),
            send_ticket: self.send_ticket,
        })
    }
}

// --- Process client's ChangeCipherSpec ---
struct ExpectCcs {
    config: Arc<ServerConfig>,
    secrets: ConnectionSecrets,
    transcript: HandshakeHash,
    session_id: SessionId,
    using_ems: bool,
    resuming: bool,
    send_ticket: bool,
}

impl State<ServerConnectionData> for ExpectCcs {
    fn handle<'m>(
        self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        match m.payload {
            MessagePayload::ChangeCipherSpec(..) => {}
            payload => {
                return Err(inappropriate_message(
                    &payload,
                    &[ContentType::ChangeCipherSpec],
                ));
            }
        }

        // CCS should not be received interleaved with fragmented handshake-level
        // message.
        cx.common.check_aligned_handshake()?;

        cx.common
            .record_layer
            .start_decrypting();
        Ok(Box::new(ExpectFinished {
            config: self.config,
            secrets: self.secrets,
            transcript: self.transcript,
            session_id: self.session_id,
            using_ems: self.using_ems,
            resuming: self.resuming,
            send_ticket: self.send_ticket,
        }))
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        self
    }
}

// --- Process client's Finished ---
fn get_server_connection_value_tls12(
    secrets: &ConnectionSecrets,
    using_ems: bool,
    cx: &ServerContext<'_>,
    time_now: UnixTime,
) -> persist::ServerSessionValue {
    let version = ProtocolVersion::TLSv1_2;

    let mut v = persist::ServerSessionValue::new(
        cx.data.sni.as_ref(),
        version,
        secrets.suite().common.suite,
        secrets.master_secret(),
        cx.common.peer_certificates.clone(),
        cx.common.alpn_protocol.clone(),
        cx.data.resumption_data.clone(),
        time_now,
        0,
    );

    if using_ems {
        v.set_extended_ms_used();
    }

    v
}

fn emit_ticket(
    secrets: &ConnectionSecrets,
    transcript: &mut HandshakeHash,
    using_ems: bool,
    cx: &mut ServerContext<'_>,
    ticketer: &dyn ProducesTickets,
    now: UnixTime,
) -> Result<(), Error> {
    let plain = get_server_connection_value_tls12(secrets, using_ems, cx, now).get_encoding();

    // If we can't produce a ticket for some reason, we can't
    // report an error. Send an empty one.
    let ticket = ticketer
        .encrypt(&plain)
        .unwrap_or_default();
    let ticket_lifetime = ticketer.lifetime();

    let m = Message {
        version: ProtocolVersion::TLSv1_2,
        payload: MessagePayload::handshake(HandshakeMessagePayload(
            HandshakePayload::NewSessionTicket(NewSessionTicketPayload::new(
                ticket_lifetime,
                ticket,
            )),
        )),
    };

    transcript.add_message(&m);
    cx.common.send_msg(m, false);
    Ok(())
}

fn emit_ccs(common: &mut CommonState) {
    let m = Message {
        version: ProtocolVersion::TLSv1_2,
        payload: MessagePayload::ChangeCipherSpec(ChangeCipherSpecPayload {}),
    };

    common.send_msg(m, false);
}

fn emit_finished(
    secrets: &ConnectionSecrets,
    transcript: &mut HandshakeHash,
    common: &mut CommonState,
) {
    let vh = transcript.current_hash();
    let verify_data = secrets.server_verify_data(&vh);
    let verify_data_payload = Payload::new(verify_data);

    let f = Message {
        version: ProtocolVersion::TLSv1_2,
        payload: MessagePayload::handshake(HandshakeMessagePayload(HandshakePayload::Finished(
            verify_data_payload,
        ))),
    };

    transcript.add_message(&f);
    common.send_msg(f, true);
}

struct ExpectFinished {
    config: Arc<ServerConfig>,
    secrets: ConnectionSecrets,
    transcript: HandshakeHash,
    session_id: SessionId,
    using_ems: bool,
    resuming: bool,
    send_ticket: bool,
}

impl State<ServerConnectionData> for ExpectFinished {
    fn handle<'m>(
        mut self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        let finished =
            require_handshake_msg!(m, HandshakeType::Finished, HandshakePayload::Finished)?;

        cx.common.check_aligned_handshake()?;

        let vh = self.transcript.current_hash();
        let expect_verify_data = self.secrets.client_verify_data(&vh);

        let _fin_verified =
            match ConstantTimeEq::ct_eq(&expect_verify_data[..], finished.bytes()).into() {
                true => verify::FinishedMessageVerified::assertion(),
                false => {
                    return Err(cx
                        .common
                        .send_fatal_alert(AlertDescription::DecryptError, Error::DecryptError));
                }
            };

        // Save connection, perhaps
        if !self.resuming && !self.session_id.is_empty() {
            let now = self.config.current_time()?;

            let value = get_server_connection_value_tls12(&self.secrets, self.using_ems, cx, now);

            let worked = self
                .config
                .session_storage
                .put(self.session_id.as_ref().to_vec(), value.get_encoding());
            #[cfg_attr(not(feature = "logging"), allow(clippy::if_same_then_else))]
            if worked {
                debug!("Session saved");
            } else {
                debug!("Session not saved");
            }
        }

        // Send our CCS and Finished.
        self.transcript.add_message(&m);
        if !self.resuming {
            if self.send_ticket {
                let now = self.config.current_time()?;
                emit_ticket(
                    &self.secrets,
                    &mut self.transcript,
                    self.using_ems,
                    cx,
                    &*self.config.ticketer,
                    now,
                )?;
            }
            emit_ccs(cx.common);
            cx.common
                .record_layer
                .start_encrypting();
            emit_finished(&self.secrets, &mut self.transcript, cx.common);
        }

        cx.common
            .start_traffic(&mut cx.sendable_plaintext);
        Ok(Box::new(ExpectTraffic {
            secrets: self.secrets,
            _fin_verified,
        }))
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        self
    }
}

// --- Process traffic ---
struct ExpectTraffic {
    secrets: ConnectionSecrets,
    _fin_verified: verify::FinishedMessageVerified,
}

impl ExpectTraffic {}

impl State<ServerConnectionData> for ExpectTraffic {
    fn handle<'m>(
        self: Box<Self>,
        cx: &mut ServerContext<'_>,
        m: Message<'m>,
    ) -> hs::NextStateOrError<'m>
    where
        Self: 'm,
    {
        match m.payload {
            MessagePayload::ApplicationData(payload) => cx
                .common
                .take_received_plaintext(payload),
            payload => {
                return Err(inappropriate_message(
                    &payload,
                    &[ContentType::ApplicationData],
                ));
            }
        }
        Ok(self)
    }

    fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        self.secrets
            .export_keying_material(output, label, context);
        Ok(())
    }

    fn extract_secrets(&self) -> Result<PartiallyExtractedSecrets, Error> {
        self.secrets
            .extract_secrets(Side::Server)
    }

    fn into_external_state(self: Box<Self>) -> Result<Box<dyn KernelState + 'static>, Error> {
        Ok(self)
    }

    fn into_owned(self: Box<Self>) -> hs::NextState<'static> {
        self
    }
}

impl KernelState for ExpectTraffic {
    fn update_secrets(&mut self, _: Direction) -> Result<ConnectionTrafficSecrets, Error> {
        Err(Error::General(
            "TLS 1.2 connections do not support traffic secret updates".into(),
        ))
    }

    fn handle_new_session_ticket(
        &mut self,
        _cx: &mut KernelContext<'_>,
        _message: &NewSessionTicketPayloadTls13,
    ) -> Result<(), Error> {
        unreachable!(
            "server connections should never have handle_new_session_ticket called on them"
        )
    }
}
