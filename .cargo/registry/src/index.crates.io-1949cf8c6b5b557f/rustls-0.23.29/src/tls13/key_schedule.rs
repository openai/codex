//! Key schedule maintenance for TLS1.3

use alloc::boxed::Box;
use alloc::string::ToString;
use core::ops::Deref;

use crate::common_state::{CommonState, Side};
use crate::crypto::cipher::{AeadKey, Iv, MessageDecrypter, Tls13AeadAlgorithm};
use crate::crypto::tls13::{Hkdf, HkdfExpander, OkmBlock, OutputLengthError, expand};
use crate::crypto::{SharedSecret, hash, hmac};
use crate::error::Error;
use crate::msgs::message::Message;
use crate::suites::PartiallyExtractedSecrets;
use crate::{ConnectionTrafficSecrets, KeyLog, Tls13CipherSuite, quic};

// We express the state of a contained KeySchedule using these
// typestates.  This means we can write code that cannot accidentally
// (e.g.) encrypt application data using a KeySchedule solely constructed
// with an empty or trivial secret, or extract the wrong kind of secrets
// at a given point.

/// The "early secret" stage of the key schedule WITH a PSK.
///
/// This is only useful when you need to use one of the binder
/// keys, the "client_early_traffic_secret", or
/// "early_exporter_master_secret".
///
/// See [`KeySchedulePreHandshake`] for more information.
pub(crate) struct KeyScheduleEarly {
    ks: KeySchedule,
}

impl KeyScheduleEarly {
    pub(crate) fn new(suite: &'static Tls13CipherSuite, secret: &[u8]) -> Self {
        Self {
            ks: KeySchedule::new(suite, secret),
        }
    }

    /// Computes the `client_early_traffic_secret` and writes it
    /// to `common`.
    ///
    /// `hs_hash` is `Transcript-Hash(ClientHello)`.
    ///
    /// ```text
    /// Derive-Secret(., "c e traffic", ClientHello)
    ///               = client_early_traffic_secret
    /// ```
    pub(crate) fn client_early_traffic_secret(
        &self,
        hs_hash: &hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
        common: &mut CommonState,
    ) {
        let client_early_traffic_secret = self.ks.derive_logged_secret(
            SecretKind::ClientEarlyTrafficSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        match common.side {
            Side::Client => self
                .ks
                .set_encrypter(&client_early_traffic_secret, common),
            Side::Server => self
                .ks
                .set_decrypter(&client_early_traffic_secret, common),
        }

        if common.is_quic() {
            // If 0-RTT should be rejected, this will be clobbered by ExtensionProcessing
            // before the application can see.
            common.quic.early_secret = Some(client_early_traffic_secret);
        }
    }

    pub(crate) fn resumption_psk_binder_key_and_sign_verify_data(
        &self,
        hs_hash: &hash::Output,
    ) -> hmac::Tag {
        let resumption_psk_binder_key = self
            .ks
            .derive_for_empty_hash(SecretKind::ResumptionPskBinderKey);
        self.ks
            .sign_verify_data(&resumption_psk_binder_key, hs_hash)
    }
}

/// The "early secret" stage of the key schedule.
///
/// Call [`KeySchedulePreHandshake::new`] to create it without
/// a PSK, or use [`From<KeyScheduleEarly>`] to create it with
/// a PSK.
///
/// ```text
///          0
///          |
///          v
/// PSK -> HKDF-Extract = Early Secret
///          |
///          +-----> Derive-Secret(., "ext binder" | "res binder", "")
///          |                     = binder_key
///          |
///          +-----> Derive-Secret(., "c e traffic", ClientHello)
///          |                     = client_early_traffic_secret
///          |
///          +-----> Derive-Secret(., "e exp master", ClientHello)
///          |                     = early_exporter_master_secret
///          v
///    Derive-Secret(., "derived", "")
/// ```
pub(crate) struct KeySchedulePreHandshake {
    ks: KeySchedule,
}

impl KeySchedulePreHandshake {
    /// Creates a key schedule without a PSK.
    pub(crate) fn new(suite: &'static Tls13CipherSuite) -> Self {
        Self {
            ks: KeySchedule::new_with_empty_secret(suite),
        }
    }

    /// `shared_secret` is the "(EC)DHE" secret input to
    /// "HKDF-Extract":
    ///
    /// ```text
    /// (EC)DHE -> HKDF-Extract = Handshake Secret
    /// ```
    pub(crate) fn into_handshake(
        mut self,
        shared_secret: SharedSecret,
    ) -> KeyScheduleHandshakeStart {
        self.ks
            .input_secret(shared_secret.secret_bytes());
        KeyScheduleHandshakeStart { ks: self.ks }
    }
}

/// Creates a key schedule with a PSK.
impl From<KeyScheduleEarly> for KeySchedulePreHandshake {
    fn from(KeyScheduleEarly { ks }: KeyScheduleEarly) -> Self {
        Self { ks }
    }
}

/// KeySchedule during handshake.
///
/// Created by [`KeySchedulePreHandshake`].
pub(crate) struct KeyScheduleHandshakeStart {
    ks: KeySchedule,
}

impl KeyScheduleHandshakeStart {
    pub(crate) fn derive_client_handshake_secrets(
        mut self,
        early_data_enabled: bool,
        hs_hash: hash::Output,
        suite: &'static Tls13CipherSuite,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
        common: &mut CommonState,
    ) -> KeyScheduleHandshake {
        debug_assert_eq!(common.side, Side::Client);
        // Suite might have changed due to resumption
        self.ks.inner = suite.into();
        let new = self.into_handshake(hs_hash, key_log, client_random, common);

        // Decrypt with the peer's key, encrypt with our own key
        new.ks
            .set_decrypter(&new.server_handshake_traffic_secret, common);

        if !early_data_enabled {
            // Set the client encryption key for handshakes if early data is not used
            new.ks
                .set_encrypter(&new.client_handshake_traffic_secret, common);
        }

        new
    }

    pub(crate) fn derive_server_handshake_secrets(
        self,
        hs_hash: hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
        common: &mut CommonState,
    ) -> KeyScheduleHandshake {
        debug_assert_eq!(common.side, Side::Server);
        let new = self.into_handshake(hs_hash, key_log, client_random, common);

        // Set up to encrypt with handshake secrets, but decrypt with early_data keys.
        // If not doing early_data after all, this is corrected later to the handshake
        // keys (now stored in key_schedule).
        new.ks
            .set_encrypter(&new.server_handshake_traffic_secret, common);
        new
    }

    pub(crate) fn server_ech_confirmation_secret(
        &mut self,
        client_hello_inner_random: &[u8],
        hs_hash: hash::Output,
    ) -> [u8; 8] {
        /*
        Per ietf-tls-esni-17 section 7.2:
        <https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-17#section-7.2>
        accept_confirmation = HKDF-Expand-Label(
          HKDF-Extract(0, ClientHelloInner.random),
          "ech accept confirmation",
          transcript_ech_conf,8)
         */
        hkdf_expand_label(
            self.ks
                .suite
                .hkdf_provider
                .extract_from_secret(None, client_hello_inner_random)
                .as_ref(),
            SecretKind::ServerEchConfirmationSecret.to_bytes(),
            hs_hash.as_ref(),
        )
    }

    fn into_handshake(
        self,
        hs_hash: hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
        common: &mut CommonState,
    ) -> KeyScheduleHandshake {
        // Use an empty handshake hash for the initial handshake.
        let client_secret = self.ks.derive_logged_secret(
            SecretKind::ClientHandshakeTrafficSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        let server_secret = self.ks.derive_logged_secret(
            SecretKind::ServerHandshakeTrafficSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        if common.is_quic() {
            common.quic.hs_secrets = Some(quic::Secrets::new(
                client_secret.clone(),
                server_secret.clone(),
                self.ks.suite,
                self.ks.suite.quic.unwrap(),
                common.side,
                common.quic.version,
            ));
        }

        KeyScheduleHandshake {
            ks: self.ks,
            client_handshake_traffic_secret: client_secret,
            server_handshake_traffic_secret: server_secret,
        }
    }
}

pub(crate) struct KeyScheduleHandshake {
    ks: KeySchedule,
    client_handshake_traffic_secret: OkmBlock,
    server_handshake_traffic_secret: OkmBlock,
}

impl KeyScheduleHandshake {
    pub(crate) fn sign_server_finish(&self, hs_hash: &hash::Output) -> hmac::Tag {
        self.ks
            .sign_finish(&self.server_handshake_traffic_secret, hs_hash)
    }

    pub(crate) fn set_handshake_encrypter(&self, common: &mut CommonState) {
        debug_assert_eq!(common.side, Side::Client);
        self.ks
            .set_encrypter(&self.client_handshake_traffic_secret, common);
    }

    pub(crate) fn set_handshake_decrypter(
        &self,
        skip_requested: Option<usize>,
        common: &mut CommonState,
    ) {
        debug_assert_eq!(common.side, Side::Server);
        let secret = &self.client_handshake_traffic_secret;
        match skip_requested {
            None => self.ks.set_decrypter(secret, common),
            Some(max_early_data_size) => common
                .record_layer
                .set_message_decrypter_with_trial_decryption(
                    self.ks
                        .derive_decrypter(&self.client_handshake_traffic_secret),
                    max_early_data_size,
                ),
        }
    }

    pub(crate) fn into_traffic_with_client_finished_pending(
        self,
        hs_hash: hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
        common: &mut CommonState,
    ) -> KeyScheduleTrafficWithClientFinishedPending {
        debug_assert_eq!(common.side, Side::Server);

        let before_finished =
            KeyScheduleBeforeFinished::new(self.ks, hs_hash, key_log, client_random);
        let (_client_secret, server_secret) = (
            &before_finished.current_client_traffic_secret,
            &before_finished.current_server_traffic_secret,
        );

        before_finished
            .ks
            .set_encrypter(server_secret, common);

        if common.is_quic() {
            common.quic.traffic_secrets = Some(quic::Secrets::new(
                _client_secret.clone(),
                server_secret.clone(),
                before_finished.ks.suite,
                before_finished.ks.suite.quic.unwrap(),
                common.side,
                common.quic.version,
            ));
        }

        KeyScheduleTrafficWithClientFinishedPending {
            handshake_client_traffic_secret: self.client_handshake_traffic_secret,
            before_finished,
        }
    }

    pub(crate) fn into_pre_finished_client_traffic(
        self,
        pre_finished_hash: hash::Output,
        handshake_hash: hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
    ) -> (KeyScheduleClientBeforeFinished, hmac::Tag) {
        let before_finished =
            KeyScheduleBeforeFinished::new(self.ks, pre_finished_hash, key_log, client_random);
        let tag = before_finished
            .ks
            .sign_finish(&self.client_handshake_traffic_secret, &handshake_hash);
        (KeyScheduleClientBeforeFinished(before_finished), tag)
    }
}

/// Keys derived (but not installed) before client's Finished message.
pub(crate) struct KeyScheduleBeforeFinished {
    ks: KeySchedule,
    current_client_traffic_secret: OkmBlock,
    current_server_traffic_secret: OkmBlock,
    current_exporter_secret: OkmBlock,
}

impl KeyScheduleBeforeFinished {
    fn new(
        mut ks: KeySchedule,
        hs_hash: hash::Output,
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
    ) -> Self {
        ks.input_empty();

        let current_client_traffic_secret = ks.derive_logged_secret(
            SecretKind::ClientApplicationTrafficSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        let current_server_traffic_secret = ks.derive_logged_secret(
            SecretKind::ServerApplicationTrafficSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        let current_exporter_secret = ks.derive_logged_secret(
            SecretKind::ExporterMasterSecret,
            hs_hash.as_ref(),
            key_log,
            client_random,
        );

        Self {
            ks,
            current_client_traffic_secret,
            current_server_traffic_secret,
            current_exporter_secret,
        }
    }

    pub(crate) fn into_traffic(
        self,
        hs_hash: hash::Output,
    ) -> (KeyScheduleTraffic, KeyScheduleResumption) {
        let Self {
            ks,
            current_client_traffic_secret,
            current_server_traffic_secret,
            current_exporter_secret,
        } = self;

        let resumption_master_secret =
            ks.derive(SecretKind::ResumptionMasterSecret, hs_hash.as_ref());

        (
            KeyScheduleTraffic {
                ks: ks.inner,
                current_client_traffic_secret,
                current_server_traffic_secret,
                current_exporter_secret,
            },
            KeyScheduleResumption {
                ks: ks.inner,
                resumption_master_secret,
            },
        )
    }
}

/// Client-side key schedule before the finished message is sent.
///
/// This differs from `KeyScheduleTrafficWithClientFinishedPending` because
/// none of the final traffic secrets are installed yet.  After the finished
/// message is sent, `into_traffic()` does that.
pub(crate) struct KeyScheduleClientBeforeFinished(KeyScheduleBeforeFinished);

impl KeyScheduleClientBeforeFinished {
    pub(crate) fn into_traffic(
        self,
        common: &mut CommonState,
        hs_hash: hash::Output,
    ) -> (KeyScheduleTraffic, KeyScheduleResumption) {
        let next = self.0;

        debug_assert_eq!(common.side, Side::Client);
        let (client_secret, server_secret) = (
            &next.current_client_traffic_secret,
            &next.current_server_traffic_secret,
        );

        next.ks
            .set_decrypter(server_secret, common);
        next.ks
            .set_encrypter(client_secret, common);

        if common.is_quic() {
            common.quic.traffic_secrets = Some(quic::Secrets::new(
                client_secret.clone(),
                server_secret.clone(),
                next.ks.suite,
                next.ks.suite.quic.unwrap(),
                common.side,
                common.quic.version,
            ));
        }

        next.into_traffic(hs_hash)
    }
}

/// KeySchedule during traffic stage, retaining the ability to calculate the client's
/// finished verify_data. The traffic stage key schedule can be extracted from it
/// through signing the client finished hash.
pub(crate) struct KeyScheduleTrafficWithClientFinishedPending {
    handshake_client_traffic_secret: OkmBlock,
    before_finished: KeyScheduleBeforeFinished,
}

impl KeyScheduleTrafficWithClientFinishedPending {
    pub(crate) fn update_decrypter(&self, common: &mut CommonState) {
        debug_assert_eq!(common.side, Side::Server);
        self.before_finished
            .ks
            .set_decrypter(&self.handshake_client_traffic_secret, common);
    }

    pub(crate) fn sign_client_finish(
        self,
        hs_hash: &hash::Output,
        common: &mut CommonState,
    ) -> (KeyScheduleBeforeFinished, hmac::Tag) {
        debug_assert_eq!(common.side, Side::Server);
        let tag = self
            .before_finished
            .ks
            .sign_finish(&self.handshake_client_traffic_secret, hs_hash);

        // Install keying to read future messages.
        self.before_finished.ks.set_decrypter(
            &self
                .before_finished
                .current_client_traffic_secret,
            common,
        );

        (self.before_finished, tag)
    }
}

/// KeySchedule during traffic stage.  All traffic & exporter keys are guaranteed
/// to be available.
pub(crate) struct KeyScheduleTraffic {
    ks: KeyScheduleSuite,
    current_client_traffic_secret: OkmBlock,
    current_server_traffic_secret: OkmBlock,
    current_exporter_secret: OkmBlock,
}

impl KeyScheduleTraffic {
    pub(crate) fn update_encrypter_and_notify(&mut self, common: &mut CommonState) {
        let secret = self.next_application_traffic_secret(common.side);
        common.enqueue_key_update_notification();
        self.ks.set_encrypter(&secret, common);
    }

    pub(crate) fn request_key_update_and_update_encrypter(
        &mut self,
        common: &mut CommonState,
    ) -> Result<(), Error> {
        common.check_aligned_handshake()?;
        common.send_msg_encrypt(Message::build_key_update_request().into());
        let secret = self.next_application_traffic_secret(common.side);
        self.ks.set_encrypter(&secret, common);
        Ok(())
    }

    pub(crate) fn update_decrypter(&mut self, common: &mut CommonState) {
        let secret = self.next_application_traffic_secret(common.side.peer());
        self.ks.set_decrypter(&secret, common);
    }

    pub(crate) fn next_application_traffic_secret(&mut self, side: Side) -> OkmBlock {
        let current = match side {
            Side::Client => &mut self.current_client_traffic_secret,
            Side::Server => &mut self.current_server_traffic_secret,
        };

        let secret = self.ks.derive_next(current);
        *current = secret.clone();
        secret
    }

    pub(crate) fn export_keying_material(
        &self,
        out: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        self.ks
            .export_keying_material(&self.current_exporter_secret, out, label, context)
    }

    pub(crate) fn refresh_traffic_secret(
        &mut self,
        side: Side,
    ) -> Result<ConnectionTrafficSecrets, Error> {
        let secret = self.next_application_traffic_secret(side);
        let (key, iv) = expand_secret(
            &secret,
            self.ks.suite.hkdf_provider,
            self.ks.suite.aead_alg.key_len(),
        );
        Ok(self
            .ks
            .suite
            .aead_alg
            .extract_keys(key, iv)?)
    }

    pub(crate) fn extract_secrets(&self, side: Side) -> Result<PartiallyExtractedSecrets, Error> {
        let (client_key, client_iv) = expand_secret(
            &self.current_client_traffic_secret,
            self.ks.suite.hkdf_provider,
            self.ks.suite.aead_alg.key_len(),
        );
        let (server_key, server_iv) = expand_secret(
            &self.current_server_traffic_secret,
            self.ks.suite.hkdf_provider,
            self.ks.suite.aead_alg.key_len(),
        );
        let client_secrets = self
            .ks
            .suite
            .aead_alg
            .extract_keys(client_key, client_iv)?;
        let server_secrets = self
            .ks
            .suite
            .aead_alg
            .extract_keys(server_key, server_iv)?;

        let (tx, rx) = match side {
            Side::Client => (client_secrets, server_secrets),
            Side::Server => (server_secrets, client_secrets),
        };
        Ok(PartiallyExtractedSecrets { tx, rx })
    }
}

pub(crate) struct KeyScheduleResumption {
    ks: KeyScheduleSuite,
    resumption_master_secret: OkmBlock,
}

impl KeyScheduleResumption {
    pub(crate) fn derive_ticket_psk(&self, nonce: &[u8]) -> OkmBlock {
        self.ks
            .derive_ticket_psk(&self.resumption_master_secret, nonce)
    }
}

fn expand_secret(secret: &OkmBlock, hkdf: &'static dyn Hkdf, aead_key_len: usize) -> (AeadKey, Iv) {
    let expander = hkdf.expander_for_okm(secret);

    (
        hkdf_expand_label_aead_key(expander.as_ref(), aead_key_len, b"key", &[]),
        hkdf_expand_label(expander.as_ref(), b"iv", &[]),
    )
}

/// This is the TLS1.3 key schedule.  It stores the current secret and
/// the type of hash.  This isn't used directly; but only through the
/// typestates.
struct KeySchedule {
    current: Box<dyn HkdfExpander>,
    inner: KeyScheduleSuite,
}

impl KeySchedule {
    fn new(suite: &'static Tls13CipherSuite, secret: &[u8]) -> Self {
        Self {
            current: suite
                .hkdf_provider
                .extract_from_secret(None, secret),
            inner: suite.into(),
        }
    }

    /// Creates a key schedule without a PSK.
    fn new_with_empty_secret(suite: &'static Tls13CipherSuite) -> Self {
        Self {
            current: suite
                .hkdf_provider
                .extract_from_zero_ikm(None),
            inner: suite.into(),
        }
    }

    /// Input the empty secret.
    ///
    /// RFC 8446: "If a given secret is not available, then the
    /// 0-value consisting of a string of Hash.length bytes set
    /// to zeros is used."
    fn input_empty(&mut self) {
        let salt = self.derive_for_empty_hash(SecretKind::DerivedSecret);
        self.current = self
            .suite
            .hkdf_provider
            .extract_from_zero_ikm(Some(salt.as_ref()));
    }

    /// Input the given secret.
    fn input_secret(&mut self, secret: &[u8]) {
        let salt = self.derive_for_empty_hash(SecretKind::DerivedSecret);
        self.current = self
            .suite
            .hkdf_provider
            .extract_from_secret(Some(salt.as_ref()), secret);
    }

    /// Derive a secret of given `kind`, using current handshake hash `hs_hash`.
    ///
    /// More specifically
    /// ```text
    ///    Derive-Secret(., "derived", Messages)
    /// ```
    /// where `hs_hash` is `Messages`.
    fn derive(&self, kind: SecretKind, hs_hash: &[u8]) -> OkmBlock {
        hkdf_expand_label_block(self.current.as_ref(), kind.to_bytes(), hs_hash)
    }

    fn derive_logged_secret(
        &self,
        kind: SecretKind,
        hs_hash: &[u8],
        key_log: &dyn KeyLog,
        client_random: &[u8; 32],
    ) -> OkmBlock {
        let output = self.derive(kind, hs_hash);

        let log_label = kind
            .log_label()
            .expect("not a loggable secret");
        if key_log.will_log(log_label) {
            key_log.log(log_label, client_random, output.as_ref());
        }
        output
    }

    /// Derive a secret of given `kind` using the hash of the empty string
    /// for the handshake hash.
    ///
    /// More specifically:
    /// ```text
    /// Derive-Secret(., Label, "")
    /// ```
    /// where `kind` is `Label`.
    ///
    /// Useful only for the following `SecretKind`s:
    /// - `SecretKind::ExternalPskBinderKey`
    /// - `SecretKind::ResumptionPSKBinderKey`
    /// - `SecretKind::DerivedSecret`
    fn derive_for_empty_hash(&self, kind: SecretKind) -> OkmBlock {
        let hp = self.suite.common.hash_provider;
        let empty_hash = hp
            .algorithm()
            .hash_for_empty_input()
            .unwrap_or_else(|| hp.start().finish());
        self.derive(kind, empty_hash.as_ref())
    }
}

impl Deref for KeySchedule {
    type Target = KeyScheduleSuite;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// This is a component part of `KeySchedule`, and groups operations
/// that do not depend on the root key schedule secret.
#[derive(Clone, Copy)]
struct KeyScheduleSuite {
    suite: &'static Tls13CipherSuite,
}

impl KeyScheduleSuite {
    fn set_encrypter(&self, secret: &OkmBlock, common: &mut CommonState) {
        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(secret);
        let key = derive_traffic_key(expander.as_ref(), self.suite.aead_alg);
        let iv = derive_traffic_iv(expander.as_ref());

        common
            .record_layer
            .set_message_encrypter(
                self.suite.aead_alg.encrypter(key, iv),
                self.suite.common.confidentiality_limit,
            );
    }

    fn set_decrypter(&self, secret: &OkmBlock, common: &mut CommonState) {
        common
            .record_layer
            .set_message_decrypter(self.derive_decrypter(secret));
    }

    fn derive_decrypter(&self, secret: &OkmBlock) -> Box<dyn MessageDecrypter> {
        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(secret);
        let key = derive_traffic_key(expander.as_ref(), self.suite.aead_alg);
        let iv = derive_traffic_iv(expander.as_ref());
        self.suite.aead_alg.decrypter(key, iv)
    }

    /// Sign the finished message consisting of `hs_hash` using a current
    /// traffic secret.
    ///
    /// See RFC 8446 section 4.4.4.
    fn sign_finish(&self, base_key: &OkmBlock, hs_hash: &hash::Output) -> hmac::Tag {
        self.sign_verify_data(base_key, hs_hash)
    }

    /// Sign the finished message consisting of `hs_hash` using the key material
    /// `base_key`.
    ///
    /// See RFC 8446 section 4.4.4.
    fn sign_verify_data(&self, base_key: &OkmBlock, hs_hash: &hash::Output) -> hmac::Tag {
        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(base_key);
        let hmac_key = hkdf_expand_label_block(expander.as_ref(), b"finished", &[]);

        self.suite
            .hkdf_provider
            .hmac_sign(&hmac_key, hs_hash.as_ref())
    }

    /// Derive the next application traffic secret, returning it.
    fn derive_next(&self, base_key: &OkmBlock) -> OkmBlock {
        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(base_key);
        hkdf_expand_label_block(expander.as_ref(), b"traffic upd", &[])
    }

    /// Derive the PSK to use given a resumption_master_secret and
    /// ticket_nonce.
    fn derive_ticket_psk(&self, rms: &OkmBlock, nonce: &[u8]) -> OkmBlock {
        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(rms);
        hkdf_expand_label_block(expander.as_ref(), b"resumption", nonce)
    }

    fn export_keying_material(
        &self,
        current_exporter_secret: &OkmBlock,
        out: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        let secret = {
            let h_empty = self
                .suite
                .common
                .hash_provider
                .hash(&[]);

            let expander = self
                .suite
                .hkdf_provider
                .expander_for_okm(current_exporter_secret);
            hkdf_expand_label_block(expander.as_ref(), label, h_empty.as_ref())
        };

        let h_context = self
            .suite
            .common
            .hash_provider
            .hash(context.unwrap_or(&[]));

        let expander = self
            .suite
            .hkdf_provider
            .expander_for_okm(&secret);
        hkdf_expand_label_slice(expander.as_ref(), b"exporter", h_context.as_ref(), out)
            .map_err(|_| Error::General("exporting too much".to_string()))
    }
}

impl From<&'static Tls13CipherSuite> for KeyScheduleSuite {
    fn from(suite: &'static Tls13CipherSuite) -> Self {
        Self { suite }
    }
}

/// [HKDF-Expand-Label] where the output is an AEAD key.
///
/// [HKDF-Expand-Label]: <https://www.rfc-editor.org/rfc/rfc8446#section-7.1>
pub fn derive_traffic_key(
    expander: &dyn HkdfExpander,
    aead_alg: &dyn Tls13AeadAlgorithm,
) -> AeadKey {
    hkdf_expand_label_aead_key(expander, aead_alg.key_len(), b"key", &[])
}

/// [HKDF-Expand-Label] where the output is an IV.
///
/// [HKDF-Expand-Label]: <https://www.rfc-editor.org/rfc/rfc8446#section-7.1>
pub fn derive_traffic_iv(expander: &dyn HkdfExpander) -> Iv {
    hkdf_expand_label(expander, b"iv", &[])
}

/// [HKDF-Expand-Label] where the output length is a compile-time constant, and therefore
/// it is infallible.
///
/// [HKDF-Expand-Label]: <https://www.rfc-editor.org/rfc/rfc8446#section-7.1>
pub(crate) fn hkdf_expand_label<T: From<[u8; N]>, const N: usize>(
    expander: &dyn HkdfExpander,
    label: &[u8],
    context: &[u8],
) -> T {
    hkdf_expand_label_inner(expander, label, context, N, |e, info| expand(e, info))
}

/// [HKDF-Expand-Label] where the output is one block in size.
pub(crate) fn hkdf_expand_label_block(
    expander: &dyn HkdfExpander,
    label: &[u8],
    context: &[u8],
) -> OkmBlock {
    hkdf_expand_label_inner(expander, label, context, expander.hash_len(), |e, info| {
        e.expand_block(info)
    })
}

/// [HKDF-Expand-Label] where the output is an AEAD key.
pub(crate) fn hkdf_expand_label_aead_key(
    expander: &dyn HkdfExpander,
    key_len: usize,
    label: &[u8],
    context: &[u8],
) -> AeadKey {
    hkdf_expand_label_inner(expander, label, context, key_len, |e, info| {
        let key: AeadKey = expand(e, info);
        key.with_length(key_len)
    })
}

/// [HKDF-Expand-Label] where the output is a slice.
///
/// This can fail because HKDF-Expand is limited in its maximum output length.
fn hkdf_expand_label_slice(
    expander: &dyn HkdfExpander,
    label: &[u8],
    context: &[u8],
    output: &mut [u8],
) -> Result<(), OutputLengthError> {
    hkdf_expand_label_inner(expander, label, context, output.len(), |e, info| {
        e.expand_slice(info, output)
    })
}

pub(crate) fn server_ech_hrr_confirmation_secret(
    hkdf_provider: &'static dyn Hkdf,
    client_hello_inner_random: &[u8],
    hs_hash: hash::Output,
) -> [u8; 8] {
    /*
    Per ietf-tls-esni-17 section 7.2.1:
    <https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-17#section-7.2.1>
    hrr_accept_confirmation = HKDF-Expand-Label(
      HKDF-Extract(0, ClientHelloInner1.random),
      "hrr ech accept confirmation",
      transcript_hrr_ech_conf,
      8)
     */
    hkdf_expand_label(
        hkdf_provider
            .extract_from_secret(None, client_hello_inner_random)
            .as_ref(),
        SecretKind::ServerEchHrrConfirmationSecret.to_bytes(),
        hs_hash.as_ref(),
    )
}

fn hkdf_expand_label_inner<F, T>(
    expander: &dyn HkdfExpander,
    label: &[u8],
    context: &[u8],
    n: usize,
    f: F,
) -> T
where
    F: FnOnce(&dyn HkdfExpander, &[&[u8]]) -> T,
{
    const LABEL_PREFIX: &[u8] = b"tls13 ";

    let output_len = u16::to_be_bytes(n as u16);
    let label_len = u8::to_be_bytes((LABEL_PREFIX.len() + label.len()) as u8);
    let context_len = u8::to_be_bytes(context.len() as u8);

    let info = &[
        &output_len[..],
        &label_len[..],
        LABEL_PREFIX,
        label,
        &context_len[..],
        context,
    ];

    f(expander, info)
}

/// The kinds of secret we can extract from `KeySchedule`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SecretKind {
    ResumptionPskBinderKey,
    ClientEarlyTrafficSecret,
    ClientHandshakeTrafficSecret,
    ServerHandshakeTrafficSecret,
    ClientApplicationTrafficSecret,
    ServerApplicationTrafficSecret,
    ExporterMasterSecret,
    ResumptionMasterSecret,
    DerivedSecret,
    ServerEchConfirmationSecret,
    ServerEchHrrConfirmationSecret,
}

impl SecretKind {
    fn to_bytes(self) -> &'static [u8] {
        use self::SecretKind::*;
        match self {
            ResumptionPskBinderKey => b"res binder",
            ClientEarlyTrafficSecret => b"c e traffic",
            ClientHandshakeTrafficSecret => b"c hs traffic",
            ServerHandshakeTrafficSecret => b"s hs traffic",
            ClientApplicationTrafficSecret => b"c ap traffic",
            ServerApplicationTrafficSecret => b"s ap traffic",
            ExporterMasterSecret => b"exp master",
            ResumptionMasterSecret => b"res master",
            DerivedSecret => b"derived",
            // https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-7.2
            ServerEchConfirmationSecret => b"ech accept confirmation",
            // https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-7.2.1
            ServerEchHrrConfirmationSecret => b"hrr ech accept confirmation",
        }
    }

    fn log_label(self) -> Option<&'static str> {
        use self::SecretKind::*;
        Some(match self {
            ClientEarlyTrafficSecret => "CLIENT_EARLY_TRAFFIC_SECRET",
            ClientHandshakeTrafficSecret => "CLIENT_HANDSHAKE_TRAFFIC_SECRET",
            ServerHandshakeTrafficSecret => "SERVER_HANDSHAKE_TRAFFIC_SECRET",
            ClientApplicationTrafficSecret => "CLIENT_TRAFFIC_SECRET_0",
            ServerApplicationTrafficSecret => "SERVER_TRAFFIC_SECRET_0",
            ExporterMasterSecret => "EXPORTER_SECRET",
            _ => {
                return None;
            }
        })
    }
}

#[cfg(test)]
#[macro_rules_attribute::apply(test_for_each_provider)]
mod tests {
    use core::fmt::Debug;
    use std::prelude::v1::*;
    use std::vec;

    use super::provider::ring_like::aead;
    use super::provider::tls13::{
        TLS13_AES_128_GCM_SHA256_INTERNAL, TLS13_CHACHA20_POLY1305_SHA256_INTERNAL,
    };
    use super::{KeySchedule, SecretKind, derive_traffic_iv, derive_traffic_key};
    use crate::KeyLog;
    use crate::msgs::enums::HashAlgorithm;

    #[test]
    fn empty_hash() {
        let sha256 = super::provider::tls13::TLS13_AES_128_GCM_SHA256
            .tls13()
            .unwrap()
            .common
            .hash_provider;
        let sha384 = super::provider::tls13::TLS13_AES_256_GCM_SHA384
            .tls13()
            .unwrap()
            .common
            .hash_provider;

        assert!(
            sha256.start().finish().as_ref()
                == HashAlgorithm::SHA256
                    .hash_for_empty_input()
                    .unwrap()
                    .as_ref()
        );
        assert!(
            sha384.start().finish().as_ref()
                == HashAlgorithm::SHA384
                    .hash_for_empty_input()
                    .unwrap()
                    .as_ref()
        );

        // a theoretical example of unsupported hash
        assert!(
            HashAlgorithm::SHA1
                .hash_for_empty_input()
                .is_none()
        );
    }

    #[test]
    fn test_vectors() {
        /* These test vectors generated with OpenSSL. */
        let hs_start_hash = [
            0xec, 0x14, 0x7a, 0x06, 0xde, 0xa3, 0xc8, 0x84, 0x6c, 0x02, 0xb2, 0x23, 0x8e, 0x41,
            0xbd, 0xdc, 0x9d, 0x89, 0xf9, 0xae, 0xa1, 0x7b, 0x5e, 0xfd, 0x4d, 0x74, 0x82, 0xaf,
            0x75, 0x88, 0x1c, 0x0a,
        ];

        let hs_full_hash = [
            0x75, 0x1a, 0x3d, 0x4a, 0x14, 0xdf, 0xab, 0xeb, 0x68, 0xe9, 0x2c, 0xa5, 0x91, 0x8e,
            0x24, 0x08, 0xb9, 0xbc, 0xb0, 0x74, 0x89, 0x82, 0xec, 0x9c, 0x32, 0x30, 0xac, 0x30,
            0xbb, 0xeb, 0x23, 0xe2,
        ];

        let ecdhe_secret = [
            0xe7, 0xb8, 0xfe, 0xf8, 0x90, 0x3b, 0x52, 0x0c, 0xb9, 0xa1, 0x89, 0x71, 0xb6, 0x9d,
            0xd4, 0x5d, 0xca, 0x53, 0xce, 0x2f, 0x12, 0xbf, 0x3b, 0xef, 0x93, 0x15, 0xe3, 0x12,
            0x71, 0xdf, 0x4b, 0x40,
        ];

        let client_hts = [
            0x61, 0x7b, 0x35, 0x07, 0x6b, 0x9d, 0x0e, 0x08, 0xcf, 0x73, 0x1d, 0x94, 0xa8, 0x66,
            0x14, 0x78, 0x41, 0x09, 0xef, 0x25, 0x55, 0x51, 0x92, 0x1d, 0xd4, 0x6e, 0x04, 0x01,
            0x35, 0xcf, 0x46, 0xab,
        ];

        let client_hts_key = [
            0x62, 0xd0, 0xdd, 0x00, 0xf6, 0x96, 0x19, 0xd3, 0xb8, 0x19, 0x3a, 0xb4, 0xa0, 0x95,
            0x85, 0xa7,
        ];

        let client_hts_iv = [
            0xff, 0xf7, 0x5d, 0xf5, 0xad, 0x35, 0xd5, 0xcb, 0x3c, 0x53, 0xf3, 0xa9,
        ];

        let server_hts = [
            0xfc, 0xf7, 0xdf, 0xe6, 0x4f, 0xa2, 0xc0, 0x4f, 0x62, 0x35, 0x38, 0x7f, 0x43, 0x4e,
            0x01, 0x42, 0x23, 0x36, 0xd9, 0xc0, 0x39, 0xde, 0x68, 0x47, 0xa0, 0xb9, 0xdd, 0xcf,
            0x29, 0xa8, 0x87, 0x59,
        ];

        let server_hts_key = [
            0x04, 0x67, 0xf3, 0x16, 0xa8, 0x05, 0xb8, 0xc4, 0x97, 0xee, 0x67, 0x04, 0x7b, 0xbc,
            0xbc, 0x54,
        ];

        let server_hts_iv = [
            0xde, 0x83, 0xa7, 0x3e, 0x9d, 0x81, 0x4b, 0x04, 0xc4, 0x8b, 0x78, 0x09,
        ];

        let client_ats = [
            0xc1, 0x4a, 0x6d, 0x79, 0x76, 0xd8, 0x10, 0x2b, 0x5a, 0x0c, 0x99, 0x51, 0x49, 0x3f,
            0xee, 0x87, 0xdc, 0xaf, 0xf8, 0x2c, 0x24, 0xca, 0xb2, 0x14, 0xe8, 0xbe, 0x71, 0xa8,
            0x20, 0x6d, 0xbd, 0xa5,
        ];

        let client_ats_key = [
            0xcc, 0x9f, 0x5f, 0x98, 0x0b, 0x5f, 0x10, 0x30, 0x6c, 0xba, 0xd7, 0xbe, 0x98, 0xd7,
            0x57, 0x2e,
        ];

        let client_ats_iv = [
            0xb8, 0x09, 0x29, 0xe8, 0xd0, 0x2c, 0x70, 0xf6, 0x11, 0x62, 0xed, 0x6b,
        ];

        let server_ats = [
            0x2c, 0x90, 0x77, 0x38, 0xd3, 0xf8, 0x37, 0x02, 0xd1, 0xe4, 0x59, 0x8f, 0x48, 0x48,
            0x53, 0x1d, 0x9f, 0x93, 0x65, 0x49, 0x1b, 0x9f, 0x7f, 0x52, 0xc8, 0x22, 0x29, 0x0d,
            0x4c, 0x23, 0x21, 0x92,
        ];

        let server_ats_key = [
            0x0c, 0xb2, 0x95, 0x62, 0xd8, 0xd8, 0x8f, 0x48, 0xb0, 0x2c, 0xbf, 0xbe, 0xd7, 0xe6,
            0x2b, 0xb3,
        ];

        let server_ats_iv = [
            0x0d, 0xb2, 0x8f, 0x98, 0x85, 0x86, 0xa1, 0xb7, 0xe4, 0xd5, 0xc6, 0x9c,
        ];

        let mut ks = KeySchedule::new_with_empty_secret(TLS13_CHACHA20_POLY1305_SHA256_INTERNAL);
        ks.input_secret(&ecdhe_secret);

        assert_traffic_secret(
            &ks,
            SecretKind::ClientHandshakeTrafficSecret,
            &hs_start_hash,
            &client_hts,
            &client_hts_key,
            &client_hts_iv,
        );

        assert_traffic_secret(
            &ks,
            SecretKind::ServerHandshakeTrafficSecret,
            &hs_start_hash,
            &server_hts,
            &server_hts_key,
            &server_hts_iv,
        );

        ks.input_empty();

        assert_traffic_secret(
            &ks,
            SecretKind::ClientApplicationTrafficSecret,
            &hs_full_hash,
            &client_ats,
            &client_ats_key,
            &client_ats_iv,
        );

        assert_traffic_secret(
            &ks,
            SecretKind::ServerApplicationTrafficSecret,
            &hs_full_hash,
            &server_ats,
            &server_ats_key,
            &server_ats_iv,
        );
    }

    fn assert_traffic_secret(
        ks: &KeySchedule,
        kind: SecretKind,
        hash: &[u8],
        expected_traffic_secret: &[u8],
        expected_key: &[u8],
        expected_iv: &[u8],
    ) {
        #[derive(Debug)]
        struct Log<'a>(&'a [u8]);
        impl KeyLog for Log<'_> {
            fn log(&self, _label: &str, _client_random: &[u8], secret: &[u8]) {
                assert_eq!(self.0, secret);
            }
        }
        let log = Log(expected_traffic_secret);
        let traffic_secret = ks.derive_logged_secret(kind, hash, &log, &[0; 32]);

        // Since we can't test key equality, we test the output of sealing with the key instead.
        let aead_alg = &aead::AES_128_GCM;
        let expander = TLS13_AES_128_GCM_SHA256_INTERNAL
            .hkdf_provider
            .expander_for_okm(&traffic_secret);
        let key = derive_traffic_key(
            expander.as_ref(),
            TLS13_AES_128_GCM_SHA256_INTERNAL.aead_alg,
        );
        let key = aead::UnboundKey::new(aead_alg, key.as_ref()).unwrap();
        let seal_output = seal_zeroes(key);
        let expected_key = aead::UnboundKey::new(aead_alg, expected_key).unwrap();
        let expected_seal_output = seal_zeroes(expected_key);
        assert_eq!(seal_output, expected_seal_output);
        assert!(seal_output.len() >= 48); // Sanity check.

        let iv = derive_traffic_iv(expander.as_ref());
        assert_eq!(iv.as_ref(), expected_iv);
    }

    fn seal_zeroes(key: aead::UnboundKey) -> Vec<u8> {
        let key = aead::LessSafeKey::new(key);
        let mut seal_output = vec![0; 32];
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key([0; aead::NONCE_LEN]),
            aead::Aad::empty(),
            &mut seal_output,
        )
        .unwrap();
        seal_output
    }
}

#[cfg(all(test, bench))]
#[macro_rules_attribute::apply(bench_for_each_provider)]
mod benchmarks {
    #[bench]
    fn bench_sha256(b: &mut test::Bencher) {
        use core::fmt::Debug;

        use super::provider::tls13::TLS13_CHACHA20_POLY1305_SHA256_INTERNAL;
        use super::{KeySchedule, SecretKind, derive_traffic_iv, derive_traffic_key};
        use crate::KeyLog;

        fn extract_traffic_secret(ks: &KeySchedule, kind: SecretKind) {
            #[derive(Debug)]
            struct Log;

            impl KeyLog for Log {
                fn log(&self, _label: &str, _client_random: &[u8], _secret: &[u8]) {}
            }

            let hash = [0u8; 32];
            let traffic_secret = ks.derive_logged_secret(kind, &hash, &Log, &[0u8; 32]);
            let traffic_secret_expander = TLS13_CHACHA20_POLY1305_SHA256_INTERNAL
                .hkdf_provider
                .expander_for_okm(&traffic_secret);
            test::black_box(derive_traffic_key(
                traffic_secret_expander.as_ref(),
                TLS13_CHACHA20_POLY1305_SHA256_INTERNAL.aead_alg,
            ));
            test::black_box(derive_traffic_iv(traffic_secret_expander.as_ref()));
        }

        b.iter(|| {
            let mut ks =
                KeySchedule::new_with_empty_secret(TLS13_CHACHA20_POLY1305_SHA256_INTERNAL);
            ks.input_secret(&[0u8; 32]);

            extract_traffic_secret(&ks, SecretKind::ClientHandshakeTrafficSecret);
            extract_traffic_secret(&ks, SecretKind::ServerHandshakeTrafficSecret);

            ks.input_empty();

            extract_traffic_secret(&ks, SecretKind::ClientApplicationTrafficSecret);
            extract_traffic_secret(&ks, SecretKind::ServerApplicationTrafficSecret);
        });
    }
}
