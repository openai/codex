use alloc::boxed::Box;
use core::fmt::Debug;
use core::mem;
use core::ops::{Deref, DerefMut, Range};
#[cfg(feature = "std")]
use std::io;

use kernel::KernelConnection;

use crate::common_state::{CommonState, Context, DEFAULT_BUFFER_LIMIT, IoState, State};
use crate::enums::{AlertDescription, ContentType, ProtocolVersion};
use crate::error::{Error, PeerMisbehaved};
use crate::log::trace;
use crate::msgs::deframer::DeframerIter;
use crate::msgs::deframer::buffers::{BufferProgress, DeframerVecBuffer, Delocator, Locator};
use crate::msgs::deframer::handshake::HandshakeDeframer;
use crate::msgs::handshake::Random;
use crate::msgs::message::{InboundPlainMessage, Message, MessagePayload};
use crate::record_layer::Decrypted;
use crate::suites::ExtractedSecrets;
use crate::vecbuf::ChunkVecBuffer;

// pub so that it can be re-exported from the crate root
pub mod kernel;
pub(crate) mod unbuffered;

#[cfg(feature = "std")]
mod connection {
    use alloc::vec::Vec;
    use core::fmt::Debug;
    use core::ops::{Deref, DerefMut};
    use std::io::{self, BufRead, Read};

    use crate::ConnectionCommon;
    use crate::common_state::{CommonState, IoState};
    use crate::error::Error;
    use crate::msgs::message::OutboundChunks;
    use crate::suites::ExtractedSecrets;
    use crate::vecbuf::ChunkVecBuffer;

    /// A client or server connection.
    #[derive(Debug)]
    pub enum Connection {
        /// A client connection
        Client(crate::client::ClientConnection),
        /// A server connection
        Server(crate::server::ServerConnection),
    }

    impl Connection {
        /// Read TLS content from `rd`.
        ///
        /// See [`ConnectionCommon::read_tls()`] for more information.
        pub fn read_tls(&mut self, rd: &mut dyn Read) -> Result<usize, io::Error> {
            match self {
                Self::Client(conn) => conn.read_tls(rd),
                Self::Server(conn) => conn.read_tls(rd),
            }
        }

        /// Writes TLS messages to `wr`.
        ///
        /// See [`ConnectionCommon::write_tls()`] for more information.
        pub fn write_tls(&mut self, wr: &mut dyn io::Write) -> Result<usize, io::Error> {
            self.sendable_tls.write_to(wr)
        }

        /// Returns an object that allows reading plaintext.
        pub fn reader(&mut self) -> Reader<'_> {
            match self {
                Self::Client(conn) => conn.reader(),
                Self::Server(conn) => conn.reader(),
            }
        }

        /// Returns an object that allows writing plaintext.
        pub fn writer(&mut self) -> Writer<'_> {
            match self {
                Self::Client(conn) => Writer::new(&mut **conn),
                Self::Server(conn) => Writer::new(&mut **conn),
            }
        }

        /// Processes any new packets read by a previous call to [`Connection::read_tls`].
        ///
        /// See [`ConnectionCommon::process_new_packets()`] for more information.
        pub fn process_new_packets(&mut self) -> Result<IoState, Error> {
            match self {
                Self::Client(conn) => conn.process_new_packets(),
                Self::Server(conn) => conn.process_new_packets(),
            }
        }

        /// Derives key material from the agreed connection secrets.
        ///
        /// See [`ConnectionCommon::export_keying_material()`] for more information.
        pub fn export_keying_material<T: AsMut<[u8]>>(
            &self,
            output: T,
            label: &[u8],
            context: Option<&[u8]>,
        ) -> Result<T, Error> {
            match self {
                Self::Client(conn) => conn.export_keying_material(output, label, context),
                Self::Server(conn) => conn.export_keying_material(output, label, context),
            }
        }

        /// This function uses `io` to complete any outstanding IO for this connection.
        ///
        /// See [`ConnectionCommon::complete_io()`] for more information.
        pub fn complete_io<T>(&mut self, io: &mut T) -> Result<(usize, usize), io::Error>
        where
            Self: Sized,
            T: Read + io::Write,
        {
            match self {
                Self::Client(conn) => conn.complete_io(io),
                Self::Server(conn) => conn.complete_io(io),
            }
        }

        /// Extract secrets, so they can be used when configuring kTLS, for example.
        /// Should be used with care as it exposes secret key material.
        pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
            match self {
                Self::Client(client) => client.dangerous_extract_secrets(),
                Self::Server(server) => server.dangerous_extract_secrets(),
            }
        }

        /// Sets a limit on the internal buffers
        ///
        /// See [`ConnectionCommon::set_buffer_limit()`] for more information.
        pub fn set_buffer_limit(&mut self, limit: Option<usize>) {
            match self {
                Self::Client(client) => client.set_buffer_limit(limit),
                Self::Server(server) => server.set_buffer_limit(limit),
            }
        }

        /// Sends a TLS1.3 `key_update` message to refresh a connection's keys
        ///
        /// See [`ConnectionCommon::refresh_traffic_keys()`] for more information.
        pub fn refresh_traffic_keys(&mut self) -> Result<(), Error> {
            match self {
                Self::Client(client) => client.refresh_traffic_keys(),
                Self::Server(server) => server.refresh_traffic_keys(),
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

    /// A structure that implements [`std::io::Read`] for reading plaintext.
    pub struct Reader<'a> {
        pub(super) received_plaintext: &'a mut ChunkVecBuffer,
        pub(super) has_received_close_notify: bool,
        pub(super) has_seen_eof: bool,
    }

    impl<'a> Reader<'a> {
        /// Check the connection's state if no bytes are available for reading.
        fn check_no_bytes_state(&self) -> io::Result<()> {
            match (self.has_received_close_notify, self.has_seen_eof) {
                // cleanly closed; don't care about TCP EOF: express this as Ok(0)
                (true, _) => Ok(()),
                // unclean closure
                (false, true) => Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    UNEXPECTED_EOF_MESSAGE,
                )),
                // connection still going, but needs more data: signal `WouldBlock` so that
                // the caller knows this
                (false, false) => Err(io::ErrorKind::WouldBlock.into()),
            }
        }

        /// Obtain a chunk of plaintext data received from the peer over this TLS connection.
        ///
        /// This method consumes `self` so that it can return a slice whose lifetime is bounded by
        /// the [`ConnectionCommon`] that created this `Reader`.
        pub fn into_first_chunk(self) -> io::Result<&'a [u8]> {
            match self.received_plaintext.chunk() {
                Some(chunk) => Ok(chunk),
                None => {
                    self.check_no_bytes_state()?;
                    Ok(&[])
                }
            }
        }
    }

    impl Read for Reader<'_> {
        /// Obtain plaintext data received from the peer over this TLS connection.
        ///
        /// If the peer closes the TLS session cleanly, this returns `Ok(0)`  once all
        /// the pending data has been read. No further data can be received on that
        /// connection, so the underlying TCP connection should be half-closed too.
        ///
        /// If the peer closes the TLS session uncleanly (a TCP EOF without sending a
        /// `close_notify` alert) this function returns a `std::io::Error` of type
        /// `ErrorKind::UnexpectedEof` once any pending data has been read.
        ///
        /// Note that support for `close_notify` varies in peer TLS libraries: many do not
        /// support it and uncleanly close the TCP connection (this might be
        /// vulnerable to truncation attacks depending on the application protocol).
        /// This means applications using rustls must both handle EOF
        /// from this function, *and* unexpected EOF of the underlying TCP connection.
        ///
        /// If there are no bytes to read, this returns `Err(ErrorKind::WouldBlock.into())`.
        ///
        /// You may learn the number of bytes available at any time by inspecting
        /// the return of [`Connection::process_new_packets`].
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let len = self.received_plaintext.read(buf)?;
            if len > 0 || buf.is_empty() {
                return Ok(len);
            }

            self.check_no_bytes_state()
                .map(|()| len)
        }

        /// Obtain plaintext data received from the peer over this TLS connection.
        ///
        /// If the peer closes the TLS session, this returns `Ok(())` without filling
        /// any more of the buffer once all the pending data has been read. No further
        /// data can be received on that connection, so the underlying TCP connection
        /// should be half-closed too.
        ///
        /// If the peer closes the TLS session uncleanly (a TCP EOF without sending a
        /// `close_notify` alert) this function returns a `std::io::Error` of type
        /// `ErrorKind::UnexpectedEof` once any pending data has been read.
        ///
        /// Note that support for `close_notify` varies in peer TLS libraries: many do not
        /// support it and uncleanly close the TCP connection (this might be
        /// vulnerable to truncation attacks depending on the application protocol).
        /// This means applications using rustls must both handle EOF
        /// from this function, *and* unexpected EOF of the underlying TCP connection.
        ///
        /// If there are no bytes to read, this returns `Err(ErrorKind::WouldBlock.into())`.
        ///
        /// You may learn the number of bytes available at any time by inspecting
        /// the return of [`Connection::process_new_packets`].
        #[cfg(read_buf)]
        fn read_buf(&mut self, mut cursor: core::io::BorrowedCursor<'_>) -> io::Result<()> {
            let before = cursor.written();
            self.received_plaintext
                .read_buf(cursor.reborrow())?;
            let len = cursor.written() - before;
            if len > 0 || cursor.capacity() == 0 {
                return Ok(());
            }

            self.check_no_bytes_state()
        }
    }

    impl BufRead for Reader<'_> {
        /// Obtain a chunk of plaintext data received from the peer over this TLS connection.
        /// This reads the same data as [`Reader::read()`], but returns a reference instead of
        /// copying the data.
        ///
        /// The caller should call [`Reader::consume()`] afterward to advance the buffer.
        ///
        /// See [`Reader::into_first_chunk()`] for a version of this function that returns a
        /// buffer with a longer lifetime.
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Reader {
                // reborrow
                received_plaintext: self.received_plaintext,
                ..*self
            }
            .into_first_chunk()
        }

        fn consume(&mut self, amt: usize) {
            self.received_plaintext
                .consume_first_chunk(amt)
        }
    }

    const UNEXPECTED_EOF_MESSAGE: &str = "peer closed connection without sending TLS close_notify: \
https://docs.rs/rustls/latest/rustls/manual/_03_howto/index.html#unexpected-eof";

    /// A structure that implements [`std::io::Write`] for writing plaintext.
    pub struct Writer<'a> {
        sink: &'a mut dyn PlaintextSink,
    }

    impl<'a> Writer<'a> {
        /// Create a new Writer.
        ///
        /// This is not an external interface.  Get one of these objects
        /// from [`Connection::writer`].
        pub(crate) fn new(sink: &'a mut dyn PlaintextSink) -> Self {
            Writer { sink }
        }
    }

    impl io::Write for Writer<'_> {
        /// Send the plaintext `buf` to the peer, encrypting
        /// and authenticating it.  Once this function succeeds
        /// you should call [`Connection::write_tls`] which will output the
        /// corresponding TLS records.
        ///
        /// This function buffers plaintext sent before the
        /// TLS handshake completes, and sends it as soon
        /// as it can.  See [`ConnectionCommon::set_buffer_limit`] to control
        /// the size of this buffer.
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sink.write(buf)
        }

        fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
            self.sink.write_vectored(bufs)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.sink.flush()
        }
    }

    /// Internal trait implemented by the [`ServerConnection`]/[`ClientConnection`]
    /// allowing them to be the subject of a [`Writer`].
    ///
    /// [`ServerConnection`]: crate::ServerConnection
    /// [`ClientConnection`]: crate::ClientConnection
    pub(crate) trait PlaintextSink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
        fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize>;
        fn flush(&mut self) -> io::Result<()>;
    }

    impl<T> PlaintextSink for ConnectionCommon<T> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let len = self
                .core
                .common_state
                .buffer_plaintext(buf.into(), &mut self.sendable_plaintext);
            self.core.maybe_refresh_traffic_keys();
            Ok(len)
        }

        fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
            let payload_owner: Vec<&[u8]>;
            let payload = match bufs.len() {
                0 => return Ok(0),
                1 => OutboundChunks::Single(bufs[0].deref()),
                _ => {
                    payload_owner = bufs
                        .iter()
                        .map(|io_slice| io_slice.deref())
                        .collect();

                    OutboundChunks::new(&payload_owner)
                }
            };
            let len = self
                .core
                .common_state
                .buffer_plaintext(payload, &mut self.sendable_plaintext);
            self.core.maybe_refresh_traffic_keys();
            Ok(len)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(feature = "std")]
pub use connection::{Connection, Reader, Writer};

#[derive(Debug)]
pub(crate) struct ConnectionRandoms {
    pub(crate) client: [u8; 32],
    pub(crate) server: [u8; 32],
}

impl ConnectionRandoms {
    pub(crate) fn new(client: Random, server: Random) -> Self {
        Self {
            client: client.0,
            server: server.0,
        }
    }
}

/// Interface shared by client and server connections.
pub struct ConnectionCommon<Data> {
    pub(crate) core: ConnectionCore<Data>,
    deframer_buffer: DeframerVecBuffer,
    sendable_plaintext: ChunkVecBuffer,
}

impl<Data> ConnectionCommon<Data> {
    /// Processes any new packets read by a previous call to
    /// [`Connection::read_tls`].
    ///
    /// Errors from this function relate to TLS protocol errors, and
    /// are fatal to the connection.  Future calls after an error will do
    /// no new work and will return the same error. After an error is
    /// received from [`process_new_packets`], you should not call [`read_tls`]
    /// any more (it will fill up buffers to no purpose). However, you
    /// may call the other methods on the connection, including `write`,
    /// `send_close_notify`, and `write_tls`. Most likely you will want to
    /// call `write_tls` to send any alerts queued by the error and then
    /// close the underlying connection.
    ///
    /// Success from this function comes with some sundry state data
    /// about the connection.
    ///
    /// [`read_tls`]: Connection::read_tls
    /// [`process_new_packets`]: Connection::process_new_packets
    #[inline]
    pub fn process_new_packets(&mut self) -> Result<IoState, Error> {
        self.core
            .process_new_packets(&mut self.deframer_buffer, &mut self.sendable_plaintext)
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
    ///
    /// This function fails if `output.len()` is zero.
    #[inline]
    pub fn export_keying_material<T: AsMut<[u8]>>(
        &self,
        output: T,
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<T, Error> {
        self.core
            .export_keying_material(output, label, context)
    }

    /// Extract secrets, so they can be used when configuring kTLS, for example.
    /// Should be used with care as it exposes secret key material.
    pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
        self.core.dangerous_extract_secrets()
    }

    /// Sets a limit on the internal buffers used to buffer
    /// unsent plaintext (prior to completing the TLS handshake)
    /// and unsent TLS records.  This limit acts only on application
    /// data written through [`Connection::writer`].
    ///
    /// By default the limit is 64KB.  The limit can be set
    /// at any time, even if the current buffer use is higher.
    ///
    /// [`None`] means no limit applies, and will mean that written
    /// data is buffered without bound -- it is up to the application
    /// to appropriately schedule its plaintext and TLS writes to bound
    /// memory usage.
    ///
    /// For illustration: `Some(1)` means a limit of one byte applies:
    /// [`Connection::writer`] will accept only one byte, encrypt it and
    /// add a TLS header.  Once this is sent via [`Connection::write_tls`],
    /// another byte may be sent.
    ///
    /// # Internal write-direction buffering
    /// rustls has two buffers whose size are bounded by this setting:
    ///
    /// ## Buffering of unsent plaintext data prior to handshake completion
    ///
    /// Calls to [`Connection::writer`] before or during the handshake
    /// are buffered (up to the limit specified here).  Once the
    /// handshake completes this data is encrypted and the resulting
    /// TLS records are added to the outgoing buffer.
    ///
    /// ## Buffering of outgoing TLS records
    ///
    /// This buffer is used to store TLS records that rustls needs to
    /// send to the peer.  It is used in these two circumstances:
    ///
    /// - by [`Connection::process_new_packets`] when a handshake or alert
    ///   TLS record needs to be sent.
    /// - by [`Connection::writer`] post-handshake: the plaintext is
    ///   encrypted and the resulting TLS record is buffered.
    ///
    /// This buffer is emptied by [`Connection::write_tls`].
    ///
    /// [`Connection::writer`]: crate::Connection::writer
    /// [`Connection::write_tls`]: crate::Connection::write_tls
    /// [`Connection::process_new_packets`]: crate::Connection::process_new_packets
    pub fn set_buffer_limit(&mut self, limit: Option<usize>) {
        self.sendable_plaintext.set_limit(limit);
        self.sendable_tls.set_limit(limit);
    }

    /// Sends a TLS1.3 `key_update` message to refresh a connection's keys.
    ///
    /// This call refreshes our encryption keys. Once the peer receives the message,
    /// it refreshes _its_ encryption and decryption keys and sends a response.
    /// Once we receive that response, we refresh our decryption keys to match.
    /// At the end of this process, keys in both directions have been refreshed.
    ///
    /// Note that this process does not happen synchronously: this call just
    /// arranges that the `key_update` message will be included in the next
    /// `write_tls` output.
    ///
    /// This fails with `Error::HandshakeNotComplete` if called before the initial
    /// handshake is complete, or if a version prior to TLS1.3 is negotiated.
    ///
    /// # Usage advice
    /// Note that other implementations (including rustls) may enforce limits on
    /// the number of `key_update` messages allowed on a given connection to prevent
    /// denial of service.  Therefore, this should be called sparingly.
    ///
    /// rustls implicitly and automatically refreshes traffic keys when needed
    /// according to the selected cipher suite's cryptographic constraints.  There
    /// is therefore no need to call this manually to avoid cryptographic keys
    /// "wearing out".
    ///
    /// The main reason to call this manually is to roll keys when it is known
    /// a connection will be idle for a long period.
    pub fn refresh_traffic_keys(&mut self) -> Result<(), Error> {
        self.core.refresh_traffic_keys()
    }
}

#[cfg(feature = "std")]
impl<Data> ConnectionCommon<Data> {
    /// Returns an object that allows reading plaintext.
    pub fn reader(&mut self) -> Reader<'_> {
        let common = &mut self.core.common_state;
        Reader {
            received_plaintext: &mut common.received_plaintext,
            // Are we done? i.e., have we processed all received messages, and received a
            // close_notify to indicate that no new messages will arrive?
            has_received_close_notify: common.has_received_close_notify,
            has_seen_eof: common.has_seen_eof,
        }
    }

    /// Returns an object that allows writing plaintext.
    pub fn writer(&mut self) -> Writer<'_> {
        Writer::new(self)
    }

    /// This function uses `io` to complete any outstanding IO for
    /// this connection.
    ///
    /// This is a convenience function which solely uses other parts
    /// of the public API.
    ///
    /// What this means depends on the connection  state:
    ///
    /// - If the connection [`is_handshaking`], then IO is performed until
    ///   the handshake is complete.
    /// - Otherwise, if [`wants_write`] is true, [`write_tls`] is invoked
    ///   until it is all written.
    /// - Otherwise, if [`wants_read`] is true, [`read_tls`] is invoked
    ///   once.
    ///
    /// The return value is the number of bytes read from and written
    /// to `io`, respectively.
    ///
    /// This function will block if `io` blocks.
    ///
    /// Errors from TLS record handling (i.e., from [`process_new_packets`])
    /// are wrapped in an `io::ErrorKind::InvalidData`-kind error.
    ///
    /// [`is_handshaking`]: CommonState::is_handshaking
    /// [`wants_read`]: CommonState::wants_read
    /// [`wants_write`]: CommonState::wants_write
    /// [`write_tls`]: ConnectionCommon::write_tls
    /// [`read_tls`]: ConnectionCommon::read_tls
    /// [`process_new_packets`]: ConnectionCommon::process_new_packets
    pub fn complete_io<T>(&mut self, io: &mut T) -> Result<(usize, usize), io::Error>
    where
        Self: Sized,
        T: io::Read + io::Write,
    {
        let mut eof = false;
        let mut wrlen = 0;
        let mut rdlen = 0;

        loop {
            let until_handshaked = self.is_handshaking();

            if !self.wants_write() && !self.wants_read() {
                // We will make no further progress.
                return Ok((rdlen, wrlen));
            }

            while self.wants_write() {
                match self.write_tls(io)? {
                    0 => {
                        io.flush()?;
                        return Ok((rdlen, wrlen)); // EOF.
                    }
                    n => wrlen += n,
                }
            }
            io.flush()?;

            if !until_handshaked && wrlen > 0 {
                return Ok((rdlen, wrlen));
            }

            while !eof && self.wants_read() {
                let read_size = match self.read_tls(io) {
                    Ok(0) => {
                        eof = true;
                        Some(0)
                    }
                    Ok(n) => {
                        rdlen += n;
                        Some(n)
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => None, // nothing to do
                    Err(err) => return Err(err),
                };
                if read_size.is_some() {
                    break;
                }
            }

            match self.process_new_packets() {
                Ok(_) => {}
                Err(e) => {
                    // In case we have an alert to send describing this error,
                    // try a last-gasp write -- but don't predate the primary
                    // error.
                    let _ignored = self.write_tls(io);
                    let _ignored = io.flush();

                    return Err(io::Error::new(io::ErrorKind::InvalidData, e));
                }
            };

            // if we're doing IO until handshaked, and we believe we've finished handshaking,
            // but process_new_packets() has queued TLS data to send, loop around again to write
            // the queued messages.
            if until_handshaked && !self.is_handshaking() && self.wants_write() {
                continue;
            }

            match (eof, until_handshaked, self.is_handshaking()) {
                (_, true, false) => return Ok((rdlen, wrlen)),
                (_, false, _) => return Ok((rdlen, wrlen)),
                (true, true, true) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                (..) => {}
            }
        }
    }

    /// Extract the first handshake message.
    ///
    /// This is a shortcut to the `process_new_packets()` -> `process_msg()` ->
    /// `process_handshake_messages()` path, specialized for the first handshake message.
    pub(crate) fn first_handshake_message(&mut self) -> Result<Option<Message<'static>>, Error> {
        let mut buffer_progress = self.core.hs_deframer.progress();

        let res = self
            .core
            .deframe(
                None,
                self.deframer_buffer.filled_mut(),
                &mut buffer_progress,
            )
            .map(|opt| opt.map(|pm| Message::try_from(pm).map(|m| m.into_owned())));

        match res? {
            Some(Ok(msg)) => {
                self.deframer_buffer
                    .discard(buffer_progress.take_discard());
                Ok(Some(msg))
            }
            Some(Err(err)) => Err(self.send_fatal_alert(AlertDescription::DecodeError, err)),
            None => Ok(None),
        }
    }

    pub(crate) fn replace_state(&mut self, new: Box<dyn State<Data>>) {
        self.core.state = Ok(new);
    }

    /// Read TLS content from `rd` into the internal buffer.
    ///
    /// Due to the internal buffering, `rd` can supply TLS messages in arbitrary-sized chunks (like
    /// a socket or pipe might).
    ///
    /// You should call [`process_new_packets()`] each time a call to this function succeeds in order
    /// to empty the incoming TLS data buffer.
    ///
    /// This function returns `Ok(0)` when the underlying `rd` does so. This typically happens when
    /// a socket is cleanly closed, or a file is at EOF. Errors may result from the IO done through
    /// `rd`; additionally, errors of `ErrorKind::Other` are emitted to signal backpressure:
    ///
    /// * In order to empty the incoming TLS data buffer, you should call [`process_new_packets()`]
    ///   each time a call to this function succeeds.
    /// * In order to empty the incoming plaintext data buffer, you should empty it through
    ///   the [`reader()`] after the call to [`process_new_packets()`].
    ///
    /// This function also returns `Ok(0)` once a `close_notify` alert has been successfully
    /// received.  No additional data is ever read in this state.
    ///
    /// [`process_new_packets()`]: ConnectionCommon::process_new_packets
    /// [`reader()`]: ConnectionCommon::reader
    pub fn read_tls(&mut self, rd: &mut dyn io::Read) -> Result<usize, io::Error> {
        if self.received_plaintext.is_full() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "received plaintext buffer full",
            ));
        }

        if self.has_received_close_notify {
            return Ok(0);
        }

        let res = self
            .deframer_buffer
            .read(rd, self.core.hs_deframer.is_active());
        if let Ok(0) = res {
            self.has_seen_eof = true;
        }
        res
    }

    /// Writes TLS messages to `wr`.
    ///
    /// On success, this function returns `Ok(n)` where `n` is a number of bytes written to `wr`
    /// (after encoding and encryption).
    ///
    /// After this function returns, the connection buffer may not yet be fully flushed. The
    /// [`CommonState::wants_write`] function can be used to check if the output buffer is empty.
    pub fn write_tls(&mut self, wr: &mut dyn io::Write) -> Result<usize, io::Error> {
        self.sendable_tls.write_to(wr)
    }
}

impl<'a, Data> From<&'a mut ConnectionCommon<Data>> for Context<'a, Data> {
    fn from(conn: &'a mut ConnectionCommon<Data>) -> Self {
        Self {
            common: &mut conn.core.common_state,
            data: &mut conn.core.data,
            sendable_plaintext: Some(&mut conn.sendable_plaintext),
        }
    }
}

impl<T> Deref for ConnectionCommon<T> {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        &self.core.common_state
    }
}

impl<T> DerefMut for ConnectionCommon<T> {
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

/// Interface shared by unbuffered client and server connections.
pub struct UnbufferedConnectionCommon<Data> {
    pub(crate) core: ConnectionCore<Data>,
    wants_write: bool,
    emitted_peer_closed_state: bool,
}

impl<Data> From<ConnectionCore<Data>> for UnbufferedConnectionCommon<Data> {
    fn from(core: ConnectionCore<Data>) -> Self {
        Self {
            core,
            wants_write: false,
            emitted_peer_closed_state: false,
        }
    }
}

impl<Data> UnbufferedConnectionCommon<Data> {
    /// Extract secrets, so they can be used when configuring kTLS, for example.
    /// Should be used with care as it exposes secret key material.
    pub fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
        self.core.dangerous_extract_secrets()
    }
}

impl<T> Deref for UnbufferedConnectionCommon<T> {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        &self.core.common_state
    }
}

pub(crate) struct ConnectionCore<Data> {
    pub(crate) state: Result<Box<dyn State<Data>>, Error>,
    pub(crate) data: Data,
    pub(crate) common_state: CommonState,
    pub(crate) hs_deframer: HandshakeDeframer,

    /// We limit consecutive empty fragments to avoid a route for the peer to send
    /// us significant but fruitless traffic.
    seen_consecutive_empty_fragments: u8,
}

impl<Data> ConnectionCore<Data> {
    pub(crate) fn new(state: Box<dyn State<Data>>, data: Data, common_state: CommonState) -> Self {
        Self {
            state: Ok(state),
            data,
            common_state,
            hs_deframer: HandshakeDeframer::default(),
            seen_consecutive_empty_fragments: 0,
        }
    }

    pub(crate) fn process_new_packets(
        &mut self,
        deframer_buffer: &mut DeframerVecBuffer,
        sendable_plaintext: &mut ChunkVecBuffer,
    ) -> Result<IoState, Error> {
        let mut state = match mem::replace(&mut self.state, Err(Error::HandshakeNotComplete)) {
            Ok(state) => state,
            Err(e) => {
                self.state = Err(e.clone());
                return Err(e);
            }
        };

        let mut buffer_progress = self.hs_deframer.progress();

        loop {
            let res = self.deframe(
                Some(&*state),
                deframer_buffer.filled_mut(),
                &mut buffer_progress,
            );

            let opt_msg = match res {
                Ok(opt_msg) => opt_msg,
                Err(e) => {
                    self.state = Err(e.clone());
                    deframer_buffer.discard(buffer_progress.take_discard());
                    return Err(e);
                }
            };

            let Some(msg) = opt_msg else {
                break;
            };

            match self.process_msg(msg, state, Some(sendable_plaintext)) {
                Ok(new) => state = new,
                Err(e) => {
                    self.state = Err(e.clone());
                    deframer_buffer.discard(buffer_progress.take_discard());
                    return Err(e);
                }
            }

            if self
                .common_state
                .has_received_close_notify
            {
                // "Any data received after a closure alert has been received MUST be ignored."
                // -- <https://datatracker.ietf.org/doc/html/rfc8446#section-6.1>
                // This is data that has already been accepted in `read_tls`.
                buffer_progress.add_discard(deframer_buffer.filled().len());
                break;
            }

            deframer_buffer.discard(buffer_progress.take_discard());
        }

        deframer_buffer.discard(buffer_progress.take_discard());
        self.state = Ok(state);
        Ok(self.common_state.current_io_state())
    }

    /// Pull a message out of the deframer and send any messages that need to be sent as a result.
    fn deframe<'b>(
        &mut self,
        state: Option<&dyn State<Data>>,
        buffer: &'b mut [u8],
        buffer_progress: &mut BufferProgress,
    ) -> Result<Option<InboundPlainMessage<'b>>, Error> {
        // before processing any more of `buffer`, return any extant messages from `hs_deframer`
        if self.hs_deframer.has_message_ready() {
            Ok(self.take_handshake_message(buffer, buffer_progress))
        } else {
            self.process_more_input(state, buffer, buffer_progress)
        }
    }

    fn take_handshake_message<'b>(
        &mut self,
        buffer: &'b mut [u8],
        buffer_progress: &mut BufferProgress,
    ) -> Option<InboundPlainMessage<'b>> {
        self.hs_deframer
            .iter(buffer)
            .next()
            .map(|(message, discard)| {
                buffer_progress.add_discard(discard);
                message
            })
    }

    fn process_more_input<'b>(
        &mut self,
        state: Option<&dyn State<Data>>,
        buffer: &'b mut [u8],
        buffer_progress: &mut BufferProgress,
    ) -> Result<Option<InboundPlainMessage<'b>>, Error> {
        let version_is_tls13 = matches!(
            self.common_state.negotiated_version,
            Some(ProtocolVersion::TLSv1_3)
        );

        let locator = Locator::new(buffer);

        loop {
            let mut iter = DeframerIter::new(&mut buffer[buffer_progress.processed()..]);

            let (message, processed) = loop {
                let message = match iter.next().transpose() {
                    Ok(Some(message)) => message,
                    Ok(None) => return Ok(None),
                    Err(err) => return Err(self.handle_deframe_error(err, state)),
                };

                let allowed_plaintext = match message.typ {
                    // CCS messages are always plaintext.
                    ContentType::ChangeCipherSpec => true,
                    // Alerts are allowed to be plaintext if-and-only-if:
                    // * The negotiated protocol version is TLS 1.3. - In TLS 1.2 it is unambiguous when
                    //   keying changes based on the CCS message. Only TLS 1.3 requires these heuristics.
                    // * We have not yet decrypted any messages from the peer - if we have we don't
                    //   expect any plaintext.
                    // * The payload size is indicative of a plaintext alert message.
                    ContentType::Alert
                        if version_is_tls13
                            && !self
                                .common_state
                                .record_layer
                                .has_decrypted()
                            && message.payload.len() <= 2 =>
                    {
                        true
                    }
                    // In other circumstances, we expect all messages to be encrypted.
                    _ => false,
                };

                if allowed_plaintext && !self.hs_deframer.is_active() {
                    break (message.into_plain_message(), iter.bytes_consumed());
                }

                let message = match self
                    .common_state
                    .record_layer
                    .decrypt_incoming(message)
                {
                    // failed decryption during trial decryption is not allowed to be
                    // interleaved with partial handshake data.
                    Ok(None) if !self.hs_deframer.is_aligned() => {
                        return Err(
                            PeerMisbehaved::RejectedEarlyDataInterleavedWithHandshakeMessage.into(),
                        );
                    }

                    // failed decryption during trial decryption.
                    Ok(None) => continue,

                    Ok(Some(message)) => message,

                    Err(err) => return Err(self.handle_deframe_error(err, state)),
                };

                let Decrypted {
                    want_close_before_decrypt,
                    plaintext,
                } = message;

                if want_close_before_decrypt {
                    self.common_state.send_close_notify();
                }

                break (plaintext, iter.bytes_consumed());
            };

            if !self.hs_deframer.is_aligned() && message.typ != ContentType::Handshake {
                // "Handshake messages MUST NOT be interleaved with other record
                // types.  That is, if a handshake message is split over two or more
                // records, there MUST NOT be any other records between them."
                // https://www.rfc-editor.org/rfc/rfc8446#section-5.1
                return Err(PeerMisbehaved::MessageInterleavedWithHandshakeMessage.into());
            }

            match message.payload.len() {
                0 => {
                    if self.seen_consecutive_empty_fragments
                        == ALLOWED_CONSECUTIVE_EMPTY_FRAGMENTS_MAX
                    {
                        return Err(PeerMisbehaved::TooManyEmptyFragments.into());
                    }
                    self.seen_consecutive_empty_fragments += 1;
                }
                _ => {
                    self.seen_consecutive_empty_fragments = 0;
                }
            };

            buffer_progress.add_processed(processed);

            // do an end-run around the borrow checker, converting `message` (containing
            // a borrowed slice) to an unborrowed one (containing a `Range` into the
            // same buffer).  the reborrow happens inside the branch that returns the
            // message.
            //
            // is fixed by -Zpolonius
            // https://github.com/rust-lang/rfcs/blob/master/text/2094-nll.md#problem-case-3-conditional-control-flow-across-functions
            let unborrowed = InboundUnborrowedMessage::unborrow(&locator, message);

            if unborrowed.typ != ContentType::Handshake {
                let message = unborrowed.reborrow(&Delocator::new(buffer));
                buffer_progress.add_discard(processed);
                return Ok(Some(message));
            }

            let message = unborrowed.reborrow(&Delocator::new(buffer));
            self.hs_deframer
                .input_message(message, &locator, buffer_progress.processed());
            self.hs_deframer.coalesce(buffer)?;

            self.common_state.aligned_handshake = self.hs_deframer.is_aligned();

            if self.hs_deframer.has_message_ready() {
                // trial decryption finishes with the first handshake message after it started.
                self.common_state
                    .record_layer
                    .finish_trial_decryption();

                return Ok(self.take_handshake_message(buffer, buffer_progress));
            }
        }
    }

    fn handle_deframe_error(&mut self, error: Error, state: Option<&dyn State<Data>>) -> Error {
        match error {
            error @ Error::InvalidMessage(_) => {
                if self.common_state.is_quic() {
                    self.common_state.quic.alert = Some(AlertDescription::DecodeError);
                    error
                } else {
                    self.common_state
                        .send_fatal_alert(AlertDescription::DecodeError, error)
                }
            }
            Error::PeerSentOversizedRecord => self
                .common_state
                .send_fatal_alert(AlertDescription::RecordOverflow, error),
            Error::DecryptError => {
                if let Some(state) = state {
                    state.handle_decrypt_error();
                }
                self.common_state
                    .send_fatal_alert(AlertDescription::BadRecordMac, error)
            }

            error => error,
        }
    }

    fn process_msg(
        &mut self,
        msg: InboundPlainMessage<'_>,
        state: Box<dyn State<Data>>,
        sendable_plaintext: Option<&mut ChunkVecBuffer>,
    ) -> Result<Box<dyn State<Data>>, Error> {
        // Drop CCS messages during handshake in TLS1.3
        if msg.typ == ContentType::ChangeCipherSpec
            && !self
                .common_state
                .may_receive_application_data
            && self.common_state.is_tls13()
        {
            if !msg.is_valid_ccs() {
                // "An implementation which receives any other change_cipher_spec value or
                //  which receives a protected change_cipher_spec record MUST abort the
                //  handshake with an "unexpected_message" alert."
                return Err(self.common_state.send_fatal_alert(
                    AlertDescription::UnexpectedMessage,
                    PeerMisbehaved::IllegalMiddleboxChangeCipherSpec,
                ));
            }

            self.common_state
                .received_tls13_change_cipher_spec()?;
            trace!("Dropping CCS");
            return Ok(state);
        }

        // Now we can fully parse the message payload.
        let msg = match Message::try_from(msg) {
            Ok(msg) => msg,
            Err(err) => {
                return Err(self
                    .common_state
                    .send_fatal_alert(AlertDescription::from(err), err));
            }
        };

        // For alerts, we have separate logic.
        if let MessagePayload::Alert(alert) = &msg.payload {
            self.common_state.process_alert(alert)?;
            return Ok(state);
        }

        self.common_state
            .process_main_protocol(msg, state, &mut self.data, sendable_plaintext)
    }

    pub(crate) fn dangerous_extract_secrets(self) -> Result<ExtractedSecrets, Error> {
        Ok(self
            .dangerous_into_kernel_connection()?
            .0)
    }

    pub(crate) fn dangerous_into_kernel_connection(
        self,
    ) -> Result<(ExtractedSecrets, KernelConnection<Data>), Error> {
        if !self
            .common_state
            .enable_secret_extraction
        {
            return Err(Error::General("Secret extraction is disabled".into()));
        }

        if self.common_state.is_handshaking() {
            return Err(Error::HandshakeNotComplete);
        }

        if !self
            .common_state
            .sendable_tls
            .is_empty()
        {
            return Err(Error::General(
                "cannot convert into an KernelConnection while there are still buffered TLS records to send"
                    .into()
            ));
        }

        let state = self.state?;

        let record_layer = &self.common_state.record_layer;
        let secrets = state.extract_secrets()?;
        let secrets = ExtractedSecrets {
            tx: (record_layer.write_seq(), secrets.tx),
            rx: (record_layer.read_seq(), secrets.rx),
        };

        let state = state.into_external_state()?;
        let external = KernelConnection::new(state, self.common_state)?;

        Ok((secrets, external))
    }

    pub(crate) fn export_keying_material<T: AsMut<[u8]>>(
        &self,
        mut output: T,
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<T, Error> {
        if output.as_mut().is_empty() {
            return Err(Error::General(
                "export_keying_material with zero-length output".into(),
            ));
        }

        match self.state.as_ref() {
            Ok(st) => st
                .export_keying_material(output.as_mut(), label, context)
                .map(|_| output),
            Err(e) => Err(e.clone()),
        }
    }

    /// Trigger a `refresh_traffic_keys` if required by `CommonState`.
    fn maybe_refresh_traffic_keys(&mut self) {
        if mem::take(
            &mut self
                .common_state
                .refresh_traffic_keys_pending,
        ) {
            let _ = self.refresh_traffic_keys();
        }
    }

    fn refresh_traffic_keys(&mut self) -> Result<(), Error> {
        match &mut self.state {
            Ok(st) => st.send_key_update_request(&mut self.common_state),
            Err(e) => Err(e.clone()),
        }
    }
}

/// Data specific to the peer's side (client or server).
pub trait SideData: Debug {}

/// An InboundPlainMessage which does not borrow its payload, but
/// references a range that can later be borrowed.
struct InboundUnborrowedMessage {
    typ: ContentType,
    version: ProtocolVersion,
    bounds: Range<usize>,
}

impl InboundUnborrowedMessage {
    fn unborrow(locator: &Locator, msg: InboundPlainMessage<'_>) -> Self {
        Self {
            typ: msg.typ,
            version: msg.version,
            bounds: locator.locate(msg.payload),
        }
    }

    fn reborrow<'b>(self, delocator: &Delocator<'b>) -> InboundPlainMessage<'b> {
        InboundPlainMessage {
            typ: self.typ,
            version: self.version,
            payload: delocator.slice_from_range(&self.bounds),
        }
    }
}

/// cf. BoringSSL's `kMaxEmptyRecords`
/// <https://github.com/google/boringssl/blob/dec5989b793c56ad4dd32173bd2d8595ca78b398/ssl/tls_record.cc#L124-L128>
const ALLOWED_CONSECUTIVE_EMPTY_FRAGMENTS_MAX: u8 = 32;
