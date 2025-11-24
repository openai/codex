use alloc::boxed::Box;
use alloc::vec::Vec;

use pki_types::CertificateDer;

use crate::conn::kernel::KernelState;
use crate::crypto::SupportedKxGroup;
use crate::enums::{AlertDescription, ContentType, HandshakeType, ProtocolVersion};
use crate::error::{Error, InvalidMessage, PeerMisbehaved};
use crate::hash_hs::HandshakeHash;
use crate::log::{debug, error, warn};
use crate::msgs::alert::AlertMessagePayload;
use crate::msgs::base::Payload;
use crate::msgs::codec::Codec;
use crate::msgs::enums::{AlertLevel, KeyUpdateRequest};
use crate::msgs::fragmenter::MessageFragmenter;
use crate::msgs::handshake::{CertificateChain, HandshakeMessagePayload, ProtocolName};
use crate::msgs::message::{
    Message, MessagePayload, OutboundChunks, OutboundOpaqueMessage, OutboundPlainMessage,
    PlainMessage,
};
use crate::record_layer::PreEncryptAction;
use crate::suites::{PartiallyExtractedSecrets, SupportedCipherSuite};
#[cfg(feature = "tls12")]
use crate::tls12::ConnectionSecrets;
use crate::unbuffered::{EncryptError, InsufficientSizeError};
use crate::vecbuf::ChunkVecBuffer;
use crate::{quic, record_layer};

/// Connection state common to both client and server connections.
pub struct CommonState {
    pub(crate) negotiated_version: Option<ProtocolVersion>,
    pub(crate) handshake_kind: Option<HandshakeKind>,
    pub(crate) side: Side,
    pub(crate) record_layer: record_layer::RecordLayer,
    pub(crate) suite: Option<SupportedCipherSuite>,
    pub(crate) kx_state: KxState,
    pub(crate) alpn_protocol: Option<ProtocolName>,
    pub(crate) aligned_handshake: bool,
    pub(crate) may_send_application_data: bool,
    pub(crate) may_receive_application_data: bool,
    pub(crate) early_traffic: bool,
    sent_fatal_alert: bool,
    /// If we signaled end of stream.
    pub(crate) has_sent_close_notify: bool,
    /// If the peer has signaled end of stream.
    pub(crate) has_received_close_notify: bool,
    #[cfg(feature = "std")]
    pub(crate) has_seen_eof: bool,
    pub(crate) peer_certificates: Option<CertificateChain<'static>>,
    message_fragmenter: MessageFragmenter,
    pub(crate) received_plaintext: ChunkVecBuffer,
    pub(crate) sendable_tls: ChunkVecBuffer,
    queued_key_update_message: Option<Vec<u8>>,

    /// Protocol whose key schedule should be used. Unused for TLS < 1.3.
    pub(crate) protocol: Protocol,
    pub(crate) quic: quic::Quic,
    pub(crate) enable_secret_extraction: bool,
    temper_counters: TemperCounters,
    pub(crate) refresh_traffic_keys_pending: bool,
    pub(crate) fips: bool,
    pub(crate) tls13_tickets_received: u32,
}

impl CommonState {
    pub(crate) fn new(side: Side) -> Self {
        Self {
            negotiated_version: None,
            handshake_kind: None,
            side,
            record_layer: record_layer::RecordLayer::new(),
            suite: None,
            kx_state: KxState::default(),
            alpn_protocol: None,
            aligned_handshake: true,
            may_send_application_data: false,
            may_receive_application_data: false,
            early_traffic: false,
            sent_fatal_alert: false,
            has_sent_close_notify: false,
            has_received_close_notify: false,
            #[cfg(feature = "std")]
            has_seen_eof: false,
            peer_certificates: None,
            message_fragmenter: MessageFragmenter::default(),
            received_plaintext: ChunkVecBuffer::new(Some(DEFAULT_RECEIVED_PLAINTEXT_LIMIT)),
            sendable_tls: ChunkVecBuffer::new(Some(DEFAULT_BUFFER_LIMIT)),
            queued_key_update_message: None,
            protocol: Protocol::Tcp,
            quic: quic::Quic::default(),
            enable_secret_extraction: false,
            temper_counters: TemperCounters::default(),
            refresh_traffic_keys_pending: false,
            fips: false,
            tls13_tickets_received: 0,
        }
    }

    /// Returns true if the caller should call [`Connection::write_tls`] as soon as possible.
    ///
    /// [`Connection::write_tls`]: crate::Connection::write_tls
    pub fn wants_write(&self) -> bool {
        !self.sendable_tls.is_empty()
    }

    /// Returns true if the connection is currently performing the TLS handshake.
    ///
    /// During this time plaintext written to the connection is buffered in memory. After
    /// [`Connection::process_new_packets()`] has been called, this might start to return `false`
    /// while the final handshake packets still need to be extracted from the connection's buffers.
    ///
    /// [`Connection::process_new_packets()`]: crate::Connection::process_new_packets
    pub fn is_handshaking(&self) -> bool {
        !(self.may_send_application_data && self.may_receive_application_data)
    }

    /// Retrieves the certificate chain or the raw public key used by the peer to authenticate.
    ///
    /// The order of the certificate chain is as it appears in the TLS
    /// protocol: the first certificate relates to the peer, the
    /// second certifies the first, the third certifies the second, and
    /// so on.
    ///
    /// When using raw public keys, the first and only element is the raw public key.
    ///
    /// This is made available for both full and resumed handshakes.
    ///
    /// For clients, this is the certificate chain or the raw public key of the server.
    ///
    /// For servers, this is the certificate chain or the raw public key of the client,
    /// if client authentication was completed.
    ///
    /// The return value is None until this value is available.
    ///
    /// Note: the return type of the 'certificate', when using raw public keys is `CertificateDer<'static>`
    /// even though this should technically be a `SubjectPublicKeyInfoDer<'static>`.
    /// This choice simplifies the API and ensures backwards compatibility.
    pub fn peer_certificates(&self) -> Option<&[CertificateDer<'static>]> {
        self.peer_certificates.as_deref()
    }

    /// Retrieves the protocol agreed with the peer via ALPN.
    ///
    /// A return value of `None` after handshake completion
    /// means no protocol was agreed (because no protocols
    /// were offered or accepted by the peer).
    pub fn alpn_protocol(&self) -> Option<&[u8]> {
        self.get_alpn_protocol()
    }

    /// Retrieves the ciphersuite agreed with the peer.
    ///
    /// This returns None until the ciphersuite is agreed.
    pub fn negotiated_cipher_suite(&self) -> Option<SupportedCipherSuite> {
        self.suite
    }

    /// Retrieves the key exchange group agreed with the peer.
    ///
    /// This function may return `None` depending on the state of the connection,
    /// the type of handshake, and the protocol version.
    ///
    /// If [`CommonState::is_handshaking()`] is true this function will return `None`.
    /// Similarly, if the [`CommonState::handshake_kind()`] is [`HandshakeKind::Resumed`]
    /// and the [`CommonState::protocol_version()`] is TLS 1.2, then no key exchange will have
    /// occurred and this function will return `None`.
    pub fn negotiated_key_exchange_group(&self) -> Option<&'static dyn SupportedKxGroup> {
        match self.kx_state {
            KxState::Complete(group) => Some(group),
            _ => None,
        }
    }

    /// Retrieves the protocol version agreed with the peer.
    ///
    /// This returns `None` until the version is agreed.
    pub fn protocol_version(&self) -> Option<ProtocolVersion> {
        self.negotiated_version
    }

    /// Which kind of handshake was performed.
    ///
    /// This tells you whether the handshake was a resumption or not.
    ///
    /// This will return `None` before it is known which sort of
    /// handshake occurred.
    pub fn handshake_kind(&self) -> Option<HandshakeKind> {
        self.handshake_kind
    }

    pub(crate) fn is_tls13(&self) -> bool {
        matches!(self.negotiated_version, Some(ProtocolVersion::TLSv1_3))
    }

    pub(crate) fn process_main_protocol<Data>(
        &mut self,
        msg: Message<'_>,
        mut state: Box<dyn State<Data>>,
        data: &mut Data,
        sendable_plaintext: Option<&mut ChunkVecBuffer>,
    ) -> Result<Box<dyn State<Data>>, Error> {
        // For TLS1.2, outside of the handshake, send rejection alerts for
        // renegotiation requests.  These can occur any time.
        if self.may_receive_application_data && !self.is_tls13() {
            let reject_ty = match self.side {
                Side::Client => HandshakeType::HelloRequest,
                Side::Server => HandshakeType::ClientHello,
            };
            if msg.is_handshake_type(reject_ty) {
                self.temper_counters
                    .received_renegotiation_request()?;
                self.send_warning_alert(AlertDescription::NoRenegotiation);
                return Ok(state);
            }
        }

        let mut cx = Context {
            common: self,
            data,
            sendable_plaintext,
        };
        match state.handle(&mut cx, msg) {
            Ok(next) => {
                state = next.into_owned();
                Ok(state)
            }
            Err(e @ Error::InappropriateMessage { .. })
            | Err(e @ Error::InappropriateHandshakeMessage { .. }) => {
                Err(self.send_fatal_alert(AlertDescription::UnexpectedMessage, e))
            }
            Err(e) => Err(e),
        }
    }

    pub(crate) fn write_plaintext(
        &mut self,
        payload: OutboundChunks<'_>,
        outgoing_tls: &mut [u8],
    ) -> Result<usize, EncryptError> {
        if payload.is_empty() {
            return Ok(0);
        }

        let fragments = self
            .message_fragmenter
            .fragment_payload(
                ContentType::ApplicationData,
                ProtocolVersion::TLSv1_2,
                payload.clone(),
            );

        for f in 0..fragments.len() {
            match self
                .record_layer
                .pre_encrypt_action(f as u64)
            {
                PreEncryptAction::Nothing => {}
                PreEncryptAction::RefreshOrClose => match self.negotiated_version {
                    Some(ProtocolVersion::TLSv1_3) => {
                        // driven by caller, as we don't have the `State` here
                        self.refresh_traffic_keys_pending = true;
                    }
                    _ => {
                        error!(
                            "traffic keys exhausted, closing connection to prevent security failure"
                        );
                        self.send_close_notify();
                        return Err(EncryptError::EncryptExhausted);
                    }
                },
                PreEncryptAction::Refuse => {
                    return Err(EncryptError::EncryptExhausted);
                }
            }
        }

        self.perhaps_write_key_update();

        self.check_required_size(outgoing_tls, fragments)?;

        let fragments = self
            .message_fragmenter
            .fragment_payload(
                ContentType::ApplicationData,
                ProtocolVersion::TLSv1_2,
                payload,
            );

        Ok(self.write_fragments(outgoing_tls, fragments))
    }

    // Changing the keys must not span any fragmented handshake
    // messages.  Otherwise the defragmented messages will have
    // been protected with two different record layer protections,
    // which is illegal.  Not mentioned in RFC.
    pub(crate) fn check_aligned_handshake(&mut self) -> Result<(), Error> {
        if !self.aligned_handshake {
            Err(self.send_fatal_alert(
                AlertDescription::UnexpectedMessage,
                PeerMisbehaved::KeyEpochWithPendingFragment,
            ))
        } else {
            Ok(())
        }
    }

    /// Fragment `m`, encrypt the fragments, and then queue
    /// the encrypted fragments for sending.
    pub(crate) fn send_msg_encrypt(&mut self, m: PlainMessage) {
        let iter = self
            .message_fragmenter
            .fragment_message(&m);
        for m in iter {
            self.send_single_fragment(m);
        }
    }

    /// Like send_msg_encrypt, but operate on an appdata directly.
    fn send_appdata_encrypt(&mut self, payload: OutboundChunks<'_>, limit: Limit) -> usize {
        // Here, the limit on sendable_tls applies to encrypted data,
        // but we're respecting it for plaintext data -- so we'll
        // be out by whatever the cipher+record overhead is.  That's a
        // constant and predictable amount, so it's not a terrible issue.
        let len = match limit {
            #[cfg(feature = "std")]
            Limit::Yes => self
                .sendable_tls
                .apply_limit(payload.len()),
            Limit::No => payload.len(),
        };

        let iter = self
            .message_fragmenter
            .fragment_payload(
                ContentType::ApplicationData,
                ProtocolVersion::TLSv1_2,
                payload.split_at(len).0,
            );
        for m in iter {
            self.send_single_fragment(m);
        }

        len
    }

    fn send_single_fragment(&mut self, m: OutboundPlainMessage<'_>) {
        if m.typ == ContentType::Alert {
            // Alerts are always sendable -- never quashed by a PreEncryptAction.
            let em = self.record_layer.encrypt_outgoing(m);
            self.queue_tls_message(em);
            return;
        }

        match self
            .record_layer
            .next_pre_encrypt_action()
        {
            PreEncryptAction::Nothing => {}

            // Close connection once we start to run out of
            // sequence space.
            PreEncryptAction::RefreshOrClose => {
                match self.negotiated_version {
                    Some(ProtocolVersion::TLSv1_3) => {
                        // driven by caller, as we don't have the `State` here
                        self.refresh_traffic_keys_pending = true;
                    }
                    _ => {
                        error!(
                            "traffic keys exhausted, closing connection to prevent security failure"
                        );
                        self.send_close_notify();
                        return;
                    }
                }
            }

            // Refuse to wrap counter at all costs.  This
            // is basically untestable unfortunately.
            PreEncryptAction::Refuse => {
                return;
            }
        };

        let em = self.record_layer.encrypt_outgoing(m);
        self.queue_tls_message(em);
    }

    fn send_plain_non_buffering(&mut self, payload: OutboundChunks<'_>, limit: Limit) -> usize {
        debug_assert!(self.may_send_application_data);
        debug_assert!(self.record_layer.is_encrypting());

        if payload.is_empty() {
            // Don't send empty fragments.
            return 0;
        }

        self.send_appdata_encrypt(payload, limit)
    }

    /// Mark the connection as ready to send application data.
    ///
    /// Also flush `sendable_plaintext` if it is `Some`.
    pub(crate) fn start_outgoing_traffic(
        &mut self,
        sendable_plaintext: &mut Option<&mut ChunkVecBuffer>,
    ) {
        self.may_send_application_data = true;
        if let Some(sendable_plaintext) = sendable_plaintext {
            self.flush_plaintext(sendable_plaintext);
        }
    }

    /// Mark the connection as ready to send and receive application data.
    ///
    /// Also flush `sendable_plaintext` if it is `Some`.
    pub(crate) fn start_traffic(&mut self, sendable_plaintext: &mut Option<&mut ChunkVecBuffer>) {
        self.may_receive_application_data = true;
        self.start_outgoing_traffic(sendable_plaintext);
    }

    /// Send any buffered plaintext.  Plaintext is buffered if
    /// written during handshake.
    fn flush_plaintext(&mut self, sendable_plaintext: &mut ChunkVecBuffer) {
        if !self.may_send_application_data {
            return;
        }

        while let Some(buf) = sendable_plaintext.pop() {
            self.send_plain_non_buffering(buf.as_slice().into(), Limit::No);
        }
    }

    // Put m into sendable_tls for writing.
    fn queue_tls_message(&mut self, m: OutboundOpaqueMessage) {
        self.perhaps_write_key_update();
        self.sendable_tls.append(m.encode());
    }

    pub(crate) fn perhaps_write_key_update(&mut self) {
        if let Some(message) = self.queued_key_update_message.take() {
            self.sendable_tls.append(message);
        }
    }

    /// Send a raw TLS message, fragmenting it if needed.
    pub(crate) fn send_msg(&mut self, m: Message<'_>, must_encrypt: bool) {
        {
            if let Protocol::Quic = self.protocol {
                if let MessagePayload::Alert(alert) = m.payload {
                    self.quic.alert = Some(alert.description);
                } else {
                    debug_assert!(
                        matches!(
                            m.payload,
                            MessagePayload::Handshake { .. } | MessagePayload::HandshakeFlight(_)
                        ),
                        "QUIC uses TLS for the cryptographic handshake only"
                    );
                    let mut bytes = Vec::new();
                    m.payload.encode(&mut bytes);
                    self.quic
                        .hs_queue
                        .push_back((must_encrypt, bytes));
                }
                return;
            }
        }
        if !must_encrypt {
            let msg = &m.into();
            let iter = self
                .message_fragmenter
                .fragment_message(msg);
            for m in iter {
                self.queue_tls_message(m.to_unencrypted_opaque());
            }
        } else {
            self.send_msg_encrypt(m.into());
        }
    }

    pub(crate) fn take_received_plaintext(&mut self, bytes: Payload<'_>) {
        self.received_plaintext
            .append(bytes.into_vec());
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn start_encryption_tls12(&mut self, secrets: &ConnectionSecrets, side: Side) {
        let (dec, enc) = secrets.make_cipher_pair(side);
        self.record_layer
            .prepare_message_encrypter(
                enc,
                secrets
                    .suite()
                    .common
                    .confidentiality_limit,
            );
        self.record_layer
            .prepare_message_decrypter(dec);
    }

    pub(crate) fn missing_extension(&mut self, why: PeerMisbehaved) -> Error {
        self.send_fatal_alert(AlertDescription::MissingExtension, why)
    }

    fn send_warning_alert(&mut self, desc: AlertDescription) {
        warn!("Sending warning alert {desc:?}");
        self.send_warning_alert_no_log(desc);
    }

    pub(crate) fn process_alert(&mut self, alert: &AlertMessagePayload) -> Result<(), Error> {
        // Reject unknown AlertLevels.
        if let AlertLevel::Unknown(_) = alert.level {
            return Err(self.send_fatal_alert(
                AlertDescription::IllegalParameter,
                Error::AlertReceived(alert.description),
            ));
        }

        // If we get a CloseNotify, make a note to declare EOF to our
        // caller.  But do not treat unauthenticated alerts like this.
        if self.may_receive_application_data && alert.description == AlertDescription::CloseNotify {
            self.has_received_close_notify = true;
            return Ok(());
        }

        // Warnings are nonfatal for TLS1.2, but outlawed in TLS1.3
        // (except, for no good reason, user_cancelled).
        let err = Error::AlertReceived(alert.description);
        if alert.level == AlertLevel::Warning {
            self.temper_counters
                .received_warning_alert()?;
            if self.is_tls13() && alert.description != AlertDescription::UserCanceled {
                return Err(self.send_fatal_alert(AlertDescription::DecodeError, err));
            }

            // Some implementations send pointless `user_canceled` alerts, don't log them
            // in release mode (https://bugs.openjdk.org/browse/JDK-8323517).
            if alert.description != AlertDescription::UserCanceled || cfg!(debug_assertions) {
                warn!("TLS alert warning received: {alert:?}");
            }

            return Ok(());
        }

        Err(err)
    }

    pub(crate) fn send_cert_verify_error_alert(&mut self, err: Error) -> Error {
        self.send_fatal_alert(
            match &err {
                Error::InvalidCertificate(e) => e.clone().into(),
                Error::PeerMisbehaved(_) => AlertDescription::IllegalParameter,
                _ => AlertDescription::HandshakeFailure,
            },
            err,
        )
    }

    pub(crate) fn send_fatal_alert(
        &mut self,
        desc: AlertDescription,
        err: impl Into<Error>,
    ) -> Error {
        debug_assert!(!self.sent_fatal_alert);
        let m = Message::build_alert(AlertLevel::Fatal, desc);
        self.send_msg(m, self.record_layer.is_encrypting());
        self.sent_fatal_alert = true;
        err.into()
    }

    /// Queues a `close_notify` warning alert to be sent in the next
    /// [`Connection::write_tls`] call.  This informs the peer that the
    /// connection is being closed.
    ///
    /// Does nothing if any `close_notify` or fatal alert was already sent.
    ///
    /// [`Connection::write_tls`]: crate::Connection::write_tls
    pub fn send_close_notify(&mut self) {
        if self.sent_fatal_alert {
            return;
        }
        debug!("Sending warning alert {:?}", AlertDescription::CloseNotify);
        self.sent_fatal_alert = true;
        self.has_sent_close_notify = true;
        self.send_warning_alert_no_log(AlertDescription::CloseNotify);
    }

    pub(crate) fn eager_send_close_notify(
        &mut self,
        outgoing_tls: &mut [u8],
    ) -> Result<usize, EncryptError> {
        self.send_close_notify();
        self.check_required_size(outgoing_tls, [].into_iter())?;
        Ok(self.write_fragments(outgoing_tls, [].into_iter()))
    }

    fn send_warning_alert_no_log(&mut self, desc: AlertDescription) {
        let m = Message::build_alert(AlertLevel::Warning, desc);
        self.send_msg(m, self.record_layer.is_encrypting());
    }

    fn check_required_size<'a>(
        &self,
        outgoing_tls: &mut [u8],
        fragments: impl Iterator<Item = OutboundPlainMessage<'a>>,
    ) -> Result<(), EncryptError> {
        let mut required_size = self.sendable_tls.len();

        for m in fragments {
            required_size += m.encoded_len(&self.record_layer);
        }

        if required_size > outgoing_tls.len() {
            return Err(EncryptError::InsufficientSize(InsufficientSizeError {
                required_size,
            }));
        }

        Ok(())
    }

    fn write_fragments<'a>(
        &mut self,
        outgoing_tls: &mut [u8],
        fragments: impl Iterator<Item = OutboundPlainMessage<'a>>,
    ) -> usize {
        let mut written = 0;

        // Any pre-existing encrypted messages in `sendable_tls` must
        // be output before encrypting any of the `fragments`.
        while let Some(message) = self.sendable_tls.pop() {
            let len = message.len();
            outgoing_tls[written..written + len].copy_from_slice(&message);
            written += len;
        }

        for m in fragments {
            let em = self
                .record_layer
                .encrypt_outgoing(m)
                .encode();

            let len = em.len();
            outgoing_tls[written..written + len].copy_from_slice(&em);
            written += len;
        }

        written
    }

    pub(crate) fn set_max_fragment_size(&mut self, new: Option<usize>) -> Result<(), Error> {
        self.message_fragmenter
            .set_max_fragment_size(new)
    }

    pub(crate) fn get_alpn_protocol(&self) -> Option<&[u8]> {
        self.alpn_protocol
            .as_ref()
            .map(AsRef::as_ref)
    }

    /// Returns true if the caller should call [`Connection::read_tls`] as soon
    /// as possible.
    ///
    /// If there is pending plaintext data to read with [`Connection::reader`],
    /// this returns false.  If your application respects this mechanism,
    /// only one full TLS message will be buffered by rustls.
    ///
    /// [`Connection::reader`]: crate::Connection::reader
    /// [`Connection::read_tls`]: crate::Connection::read_tls
    pub fn wants_read(&self) -> bool {
        // We want to read more data all the time, except when we have unprocessed plaintext.
        // This provides back-pressure to the TCP buffers. We also don't want to read more after
        // the peer has sent us a close notification.
        //
        // In the handshake case we don't have readable plaintext before the handshake has
        // completed, but also don't want to read if we still have sendable tls.
        self.received_plaintext.is_empty()
            && !self.has_received_close_notify
            && (self.may_send_application_data || self.sendable_tls.is_empty())
    }

    pub(crate) fn current_io_state(&self) -> IoState {
        IoState {
            tls_bytes_to_write: self.sendable_tls.len(),
            plaintext_bytes_to_read: self.received_plaintext.len(),
            peer_has_closed: self.has_received_close_notify,
        }
    }

    pub(crate) fn is_quic(&self) -> bool {
        self.protocol == Protocol::Quic
    }

    pub(crate) fn should_update_key(
        &mut self,
        key_update_request: &KeyUpdateRequest,
    ) -> Result<bool, Error> {
        self.temper_counters
            .received_key_update_request()?;

        match key_update_request {
            KeyUpdateRequest::UpdateNotRequested => Ok(false),
            KeyUpdateRequest::UpdateRequested => Ok(self.queued_key_update_message.is_none()),
            _ => Err(self.send_fatal_alert(
                AlertDescription::IllegalParameter,
                InvalidMessage::InvalidKeyUpdate,
            )),
        }
    }

    pub(crate) fn enqueue_key_update_notification(&mut self) {
        let message = PlainMessage::from(Message::build_key_update_notify());
        self.queued_key_update_message = Some(
            self.record_layer
                .encrypt_outgoing(message.borrow_outbound())
                .encode(),
        );
    }

    pub(crate) fn received_tls13_change_cipher_spec(&mut self) -> Result<(), Error> {
        self.temper_counters
            .received_tls13_change_cipher_spec()
    }
}

#[cfg(feature = "std")]
impl CommonState {
    /// Send plaintext application data, fragmenting and
    /// encrypting it as it goes out.
    ///
    /// If internal buffers are too small, this function will not accept
    /// all the data.
    pub(crate) fn buffer_plaintext(
        &mut self,
        payload: OutboundChunks<'_>,
        sendable_plaintext: &mut ChunkVecBuffer,
    ) -> usize {
        self.perhaps_write_key_update();
        self.send_plain(payload, Limit::Yes, sendable_plaintext)
    }

    pub(crate) fn send_early_plaintext(&mut self, data: &[u8]) -> usize {
        debug_assert!(self.early_traffic);
        debug_assert!(self.record_layer.is_encrypting());

        if data.is_empty() {
            // Don't send empty fragments.
            return 0;
        }

        self.send_appdata_encrypt(data.into(), Limit::Yes)
    }

    /// Encrypt and send some plaintext `data`.  `limit` controls
    /// whether the per-connection buffer limits apply.
    ///
    /// Returns the number of bytes written from `data`: this might
    /// be less than `data.len()` if buffer limits were exceeded.
    fn send_plain(
        &mut self,
        payload: OutboundChunks<'_>,
        limit: Limit,
        sendable_plaintext: &mut ChunkVecBuffer,
    ) -> usize {
        if !self.may_send_application_data {
            // If we haven't completed handshaking, buffer
            // plaintext to send once we do.
            let len = match limit {
                Limit::Yes => sendable_plaintext.append_limited_copy(payload),
                Limit::No => sendable_plaintext.append(payload.to_vec()),
            };
            return len;
        }

        self.send_plain_non_buffering(payload, limit)
    }
}

/// Describes which sort of handshake happened.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HandshakeKind {
    /// A full handshake.
    ///
    /// This is the typical TLS connection initiation process when resumption is
    /// not yet unavailable, and the initial `ClientHello` was accepted by the server.
    Full,

    /// A full TLS1.3 handshake, with an extra round-trip for a `HelloRetryRequest`.
    ///
    /// The server can respond with a `HelloRetryRequest` if the initial `ClientHello`
    /// is unacceptable for several reasons, the most likely if no supported key
    /// shares were offered by the client.
    FullWithHelloRetryRequest,

    /// A resumed handshake.
    ///
    /// Resumed handshakes involve fewer round trips and less cryptography than
    /// full ones, but can only happen when the peers have previously done a full
    /// handshake together, and then remember data about it.
    Resumed,
}

/// Values of this structure are returned from [`Connection::process_new_packets`]
/// and tell the caller the current I/O state of the TLS connection.
///
/// [`Connection::process_new_packets`]: crate::Connection::process_new_packets
#[derive(Debug, Eq, PartialEq)]
pub struct IoState {
    tls_bytes_to_write: usize,
    plaintext_bytes_to_read: usize,
    peer_has_closed: bool,
}

impl IoState {
    /// How many bytes could be written by [`Connection::write_tls`] if called
    /// right now.  A non-zero value implies [`CommonState::wants_write`].
    ///
    /// [`Connection::write_tls`]: crate::Connection::write_tls
    pub fn tls_bytes_to_write(&self) -> usize {
        self.tls_bytes_to_write
    }

    /// How many plaintext bytes could be obtained via [`std::io::Read`]
    /// without further I/O.
    pub fn plaintext_bytes_to_read(&self) -> usize {
        self.plaintext_bytes_to_read
    }

    /// True if the peer has sent us a close_notify alert.  This is
    /// the TLS mechanism to securely half-close a TLS connection,
    /// and signifies that the peer will not send any further data
    /// on this connection.
    ///
    /// This is also signalled via returning `Ok(0)` from
    /// [`std::io::Read`], after all the received bytes have been
    /// retrieved.
    pub fn peer_has_closed(&self) -> bool {
        self.peer_has_closed
    }
}

pub(crate) trait State<Data>: Send + Sync {
    fn handle<'m>(
        self: Box<Self>,
        cx: &mut Context<'_, Data>,
        message: Message<'m>,
    ) -> Result<Box<dyn State<Data> + 'm>, Error>
    where
        Self: 'm;

    fn export_keying_material(
        &self,
        _output: &mut [u8],
        _label: &[u8],
        _context: Option<&[u8]>,
    ) -> Result<(), Error> {
        Err(Error::HandshakeNotComplete)
    }

    fn extract_secrets(&self) -> Result<PartiallyExtractedSecrets, Error> {
        Err(Error::HandshakeNotComplete)
    }

    fn send_key_update_request(&mut self, _common: &mut CommonState) -> Result<(), Error> {
        Err(Error::HandshakeNotComplete)
    }

    fn handle_decrypt_error(&self) {}

    fn into_external_state(self: Box<Self>) -> Result<Box<dyn KernelState + 'static>, Error> {
        Err(Error::HandshakeNotComplete)
    }

    fn into_owned(self: Box<Self>) -> Box<dyn State<Data> + 'static>;
}

pub(crate) struct Context<'a, Data> {
    pub(crate) common: &'a mut CommonState,
    pub(crate) data: &'a mut Data,
    /// Buffered plaintext. This is `Some` if any plaintext was written during handshake and `None`
    /// otherwise.
    pub(crate) sendable_plaintext: Option<&'a mut ChunkVecBuffer>,
}

/// Side of the connection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    /// A client initiates the connection.
    Client,
    /// A server waits for a client to connect.
    Server,
}

impl Side {
    pub(crate) fn peer(&self) -> Self {
        match self {
            Self::Client => Self::Server,
            Self::Server => Self::Client,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum Protocol {
    Tcp,
    Quic,
}

enum Limit {
    #[cfg(feature = "std")]
    Yes,
    No,
}

/// Tracking technically-allowed protocol actions
/// that we limit to avoid denial-of-service vectors.
struct TemperCounters {
    allowed_warning_alerts: u8,
    allowed_renegotiation_requests: u8,
    allowed_key_update_requests: u8,
    allowed_middlebox_ccs: u8,
}

impl TemperCounters {
    fn received_warning_alert(&mut self) -> Result<(), Error> {
        match self.allowed_warning_alerts {
            0 => Err(PeerMisbehaved::TooManyWarningAlertsReceived.into()),
            _ => {
                self.allowed_warning_alerts -= 1;
                Ok(())
            }
        }
    }

    fn received_renegotiation_request(&mut self) -> Result<(), Error> {
        match self.allowed_renegotiation_requests {
            0 => Err(PeerMisbehaved::TooManyRenegotiationRequests.into()),
            _ => {
                self.allowed_renegotiation_requests -= 1;
                Ok(())
            }
        }
    }

    fn received_key_update_request(&mut self) -> Result<(), Error> {
        match self.allowed_key_update_requests {
            0 => Err(PeerMisbehaved::TooManyKeyUpdateRequests.into()),
            _ => {
                self.allowed_key_update_requests -= 1;
                Ok(())
            }
        }
    }

    fn received_tls13_change_cipher_spec(&mut self) -> Result<(), Error> {
        match self.allowed_middlebox_ccs {
            0 => Err(PeerMisbehaved::IllegalMiddleboxChangeCipherSpec.into()),
            _ => {
                self.allowed_middlebox_ccs -= 1;
                Ok(())
            }
        }
    }
}

impl Default for TemperCounters {
    fn default() -> Self {
        Self {
            // cf. BoringSSL `kMaxWarningAlerts`
            // <https://github.com/google/boringssl/blob/dec5989b793c56ad4dd32173bd2d8595ca78b398/ssl/tls_record.cc#L137-L139>
            allowed_warning_alerts: 4,

            // we rebuff renegotiation requests with a `NoRenegotiation` warning alerts.
            // a second request after this is fatal.
            allowed_renegotiation_requests: 1,

            // cf. BoringSSL `kMaxKeyUpdates`
            // <https://github.com/google/boringssl/blob/dec5989b793c56ad4dd32173bd2d8595ca78b398/ssl/tls13_both.cc#L35-L38>
            allowed_key_update_requests: 32,

            // At most two CCS are allowed: one after each ClientHello (recall a second
            // ClientHello happens after a HelloRetryRequest).
            //
            // note BoringSSL allows up to 32.
            allowed_middlebox_ccs: 2,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) enum KxState {
    #[default]
    None,
    Start(&'static dyn SupportedKxGroup),
    Complete(&'static dyn SupportedKxGroup),
}

impl KxState {
    pub(crate) fn complete(&mut self) {
        debug_assert!(matches!(self, Self::Start(_)));
        if let Self::Start(group) = self {
            *self = Self::Complete(*group);
        }
    }
}

pub(crate) struct HandshakeFlight<'a, const TLS13: bool> {
    pub(crate) transcript: &'a mut HandshakeHash,
    body: Vec<u8>,
}

impl<'a, const TLS13: bool> HandshakeFlight<'a, TLS13> {
    pub(crate) fn new(transcript: &'a mut HandshakeHash) -> Self {
        Self {
            transcript,
            body: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, hs: HandshakeMessagePayload<'_>) {
        let start_len = self.body.len();
        hs.encode(&mut self.body);
        self.transcript
            .add(&self.body[start_len..]);
    }

    pub(crate) fn finish(self, common: &mut CommonState) {
        common.send_msg(
            Message {
                version: match TLS13 {
                    true => ProtocolVersion::TLSv1_3,
                    false => ProtocolVersion::TLSv1_2,
                },
                payload: MessagePayload::HandshakeFlight(Payload::new(self.body)),
            },
            TLS13,
        );
    }
}

#[cfg(feature = "tls12")]
pub(crate) type HandshakeFlightTls12<'a> = HandshakeFlight<'a, false>;
pub(crate) type HandshakeFlightTls13<'a> = HandshakeFlight<'a, true>;

const DEFAULT_RECEIVED_PLAINTEXT_LIMIT: usize = 16 * 1024;
pub(crate) const DEFAULT_BUFFER_LIMIT: usize = 64 * 1024;
