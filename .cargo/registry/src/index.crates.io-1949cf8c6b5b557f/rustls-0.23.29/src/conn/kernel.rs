//! Kernel connection API.
//!
//! This module gives you the bare minimum you need to implement a TLS connection
//! that does its own encryption and decryption while still using rustls to manage
//! connection secrets and session tickets. It is intended for use cases like kTLS
//! where you want to use rustls to establish the connection but want to use
//! something else to do the encryption/decryption after that.
//!
//! There are only two things that [`KernelConnection`] is able to do:
//! 1. Compute new traffic secrets when a key update occurs.
//! 2. Save received session tickets sent by a server peer.
//!
//! That's it. Everything else you will need to implement yourself.
//!
//! # Entry Point
//! The entry points into this API are
//! [`UnbufferedClientConnection::dangerous_into_kernel_connection`][client-into]
//! and
//! [`UnbufferedServerConnection::dangerous_into_kernel_connection`][server-into].
//!
//! In order to actually create an [`KernelConnection`] all of the following
//! must be true:
//! - the connection must have completed its handshake,
//! - the connection must have no buffered TLS data waiting to be sent, and,
//! - the config used to create the connection must have `enable_extract_secrets`
//!   set to true.
//!
//! This sounds fairly complicated to achieve at first glance. However, if you
//! drive an unbuffered connection through the handshake until it returns
//! [`WriteTraffic`] then it will end up in an appropriate state to convert
//! into an external connection.
//!
//! [client-into]: crate::client::UnbufferedClientConnection::dangerous_into_kernel_connection
//! [server-into]: crate::server::UnbufferedServerConnection::dangerous_into_kernel_connection
//! [`WriteTraffic`]: crate::unbuffered::ConnectionState::WriteTraffic
//!
//! # Cipher Suite Confidentiality Limits
//! Some cipher suites (notably AES-GCM) have vulnerabilities where they are no
//! longer secure once a certain number of messages have been sent. Normally,
//! rustls tracks how many messages have been written or read and will
//! automatically either refresh keys or emit an error when approaching the
//! confidentiality limit of the cipher suite.
//!
//! [`KernelConnection`] has no way to track this. It is the responsibility
//! of the user of the API to track approximately how many messages have been
//! sent and either refresh the traffic keys or abort the connection before the
//! confidentiality limit is reached.
//!
//! You can find the current confidentiality limit by looking at
//! [`CipherSuiteCommon::confidentiality_limit`] for the cipher suite selected
//! by the connection.
//!
//! [`CipherSuiteCommon::confidentiality_limit`]: crate::CipherSuiteCommon::confidentiality_limit
//! [`KernelConnection`]: crate::kernel::KernelConnection

use alloc::boxed::Box;
use core::marker::PhantomData;

use crate::client::ClientConnectionData;
use crate::common_state::Protocol;
use crate::msgs::codec::Codec;
use crate::msgs::handshake::{CertificateChain, NewSessionTicketPayloadTls13};
use crate::quic::Quic;
use crate::{CommonState, ConnectionTrafficSecrets, Error, ProtocolVersion, SupportedCipherSuite};

/// A kernel connection.
///
/// This does not directly wrap a kernel connection, rather it gives you the
/// minimal interfaces you need to implement a well-behaved TLS connection on
/// top of kTLS.
///
/// See the [`crate::kernel`] module docs for more details.
pub struct KernelConnection<Data> {
    state: Box<dyn KernelState>,

    peer_certificates: Option<CertificateChain<'static>>,
    quic: Quic,

    negotiated_version: ProtocolVersion,
    protocol: Protocol,
    suite: SupportedCipherSuite,

    _data: PhantomData<Data>,
}

impl<Data> KernelConnection<Data> {
    pub(crate) fn new(state: Box<dyn KernelState>, common: CommonState) -> Result<Self, Error> {
        Ok(Self {
            state,

            peer_certificates: common.peer_certificates,
            quic: common.quic,
            negotiated_version: common
                .negotiated_version
                .ok_or(Error::HandshakeNotComplete)?,
            protocol: common.protocol,
            suite: common
                .suite
                .ok_or(Error::HandshakeNotComplete)?,

            _data: PhantomData,
        })
    }

    /// Retrieves the ciphersuite agreed with the peer.
    pub fn negotiated_cipher_suite(&self) -> SupportedCipherSuite {
        self.suite
    }

    /// Retrieves the protocol version agreed with the peer.
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.negotiated_version
    }

    /// Update the traffic secret used for encrypting messages sent to the peer.
    ///
    /// Returns the new traffic secret and initial sequence number to use.
    ///
    /// In order to use the new secret you should send a TLS 1.3 key update to
    /// the peer and then use the new traffic secrets to encrypt any future
    /// messages.
    ///
    /// Note that it is only possible to update the traffic secrets on a TLS 1.3
    /// connection. Attempting to do so on a non-TLS 1.3 connection will result
    /// in an error.
    pub fn update_tx_secret(&mut self) -> Result<(u64, ConnectionTrafficSecrets), Error> {
        // The sequence number always starts at 0 after a key update.
        self.state
            .update_secrets(Direction::Transmit)
            .map(|secret| (0, secret))
    }

    /// Update the traffic secret used for decrypting messages received from the
    /// peer.
    ///
    /// Returns the new traffic secret and initial sequence number to use.
    ///
    /// You should call this method once you receive a TLS 1.3 key update message
    /// from the peer.
    ///
    /// Note that it is only possible to update the traffic secrets on a TLS 1.3
    /// connection. Attempting to do so on a non-TLS 1.3 connection will result
    /// in an error.
    pub fn update_rx_secret(&mut self) -> Result<(u64, ConnectionTrafficSecrets), Error> {
        // The sequence number always starts at 0 after a key update.
        self.state
            .update_secrets(Direction::Receive)
            .map(|secret| (0, secret))
    }
}

impl KernelConnection<ClientConnectionData> {
    /// Handle a `new_session_ticket` message from the peer.
    ///
    /// This will register the session ticket within with rustls so that it can
    /// be used to establish future TLS connections.
    ///
    /// # Getting the right payload
    ///
    /// This method expects to be passed the inner payload of the handshake
    /// message. This means that you will need to parse the header of the
    /// handshake message in order to determine the correct payload to pass in.
    /// The message format is described in [RFC 8446 section 4][0]. `payload`
    /// should not include the `msg_type` or `length` fields.
    ///
    /// Code to parse out the payload should look something like this
    /// ```no_run
    /// use rustls::{ContentType, HandshakeType};
    /// use rustls::kernel::KernelConnection;
    /// use rustls::client::ClientConnectionData;
    ///
    /// # fn doctest(conn: &mut KernelConnection<ClientConnectionData>, typ: ContentType, message: &[u8]) -> Result<(), rustls::Error> {
    /// let conn: &mut KernelConnection<ClientConnectionData> = // ...
    /// #   conn;
    /// let typ: ContentType = // ...
    /// #   typ;
    /// let mut message: &[u8] = // ...
    /// #   message;
    ///
    /// // Processing for other messages not included in this example
    /// assert_eq!(typ, ContentType::Handshake);
    ///
    /// // There may be multiple handshake payloads within a single handshake message.
    /// while !message.is_empty() {
    ///     let (typ, len, rest) = match message {
    ///         &[typ, a, b, c, ref rest @ ..] => (
    ///             HandshakeType::from(typ),
    ///             u32::from_be_bytes([0, a, b, c]) as usize,
    ///             rest
    ///         ),
    ///         _ => panic!("error handling not included in this example")
    ///     };
    ///
    ///     // Processing for other messages not included in this example.
    ///     assert_eq!(typ, HandshakeType::NewSessionTicket);
    ///     assert!(rest.len() >= len, "invalid handshake message");
    ///
    ///     let (payload, rest) = rest.split_at(len);
    ///     message = rest;
    ///
    ///     conn.handle_new_session_ticket(payload)?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// This method will return an error if:
    /// - This connection is not a TLS 1.3 connection (in TLS 1.2 session tickets
    ///   are sent as part of the handshake).
    /// - The provided payload is not a valid `new_session_ticket` payload or has
    ///   extra unparsed trailing data.
    /// - An error occurs while the connection updates the session ticket store.
    ///
    /// [0]: https://datatracker.ietf.org/doc/html/rfc8446#section-4
    pub fn handle_new_session_ticket(&mut self, payload: &[u8]) -> Result<(), Error> {
        // We want to return a more specific error here first if this is called
        // on a non-TLS 1.3 connection since a parsing error isn't the real issue
        // here.
        if self.protocol_version() != ProtocolVersion::TLSv1_3 {
            return Err(Error::General(
                "TLS 1.2 session tickets may not be sent once the handshake has completed".into(),
            ));
        }

        let nst = NewSessionTicketPayloadTls13::read_bytes(payload)?;
        let mut cx = KernelContext {
            peer_certificates: self.peer_certificates.as_ref(),
            protocol: self.protocol,
            quic: &self.quic,
        };
        self.state
            .handle_new_session_ticket(&mut cx, &nst)
    }
}

pub(crate) trait KernelState: Send + Sync {
    /// Update the traffic secret for the specified direction on the connection.
    fn update_secrets(&mut self, dir: Direction) -> Result<ConnectionTrafficSecrets, Error>;

    /// Handle a new session ticket.
    ///
    /// This will only ever be called for client connections, as [`KernelConnection`]
    /// only exposes the relevant API for client connections.
    fn handle_new_session_ticket(
        &mut self,
        cx: &mut KernelContext<'_>,
        message: &NewSessionTicketPayloadTls13,
    ) -> Result<(), Error>;
}

pub(crate) struct KernelContext<'a> {
    pub(crate) peer_certificates: Option<&'a CertificateChain<'static>>,
    pub(crate) protocol: Protocol,
    pub(crate) quic: &'a Quic,
}

impl KernelContext<'_> {
    pub(crate) fn is_quic(&self) -> bool {
        self.protocol == Protocol::Quic
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    Transmit,
    Receive,
}
