use std::io;

use crate::codec::UserError;
use crate::frame::{self, Reason, StreamId};
use crate::proto::{self, Error, Initiator, PollReset};

use self::Inner::*;
use self::Peer::*;

/// Represents the state of an H2 stream
///
/// ```not_rust
///                              +--------+
///                      send PP |        | recv PP
///                     ,--------|  idle  |--------.
///                    /         |        |         \
///                   v          +--------+          v
///            +----------+          |           +----------+
///            |          |          | send H /  |          |
///     ,------| reserved |          | recv H    | reserved |------.
///     |      | (local)  |          |           | (remote) |      |
///     |      +----------+          v           +----------+      |
///     |          |             +--------+             |          |
///     |          |     recv ES |        | send ES     |          |
///     |   send H |     ,-------|  open  |-------.     | recv H   |
///     |          |    /        |        |        \    |          |
///     |          v   v         +--------+         v   v          |
///     |      +----------+          |           +----------+      |
///     |      |   half   |          |           |   half   |      |
///     |      |  closed  |          | send R /  |  closed  |      |
///     |      | (remote) |          | recv R    | (local)  |      |
///     |      +----------+          |           +----------+      |
///     |           |                |                 |           |
///     |           | send ES /      |       recv ES / |           |
///     |           | send R /       v        send R / |           |
///     |           | recv R     +--------+   recv R   |           |
///     | send R /  `----------->|        |<-----------'  send R / |
///     | recv R                 | closed |               recv R   |
///     `----------------------->|        |<----------------------'
///                              +--------+
///
///        send:   endpoint sends this frame
///        recv:   endpoint receives this frame
///
///        H:  HEADERS frame (with implied CONTINUATIONs)
///        PP: PUSH_PROMISE frame (with implied CONTINUATIONs)
///        ES: END_STREAM flag
///        R:  RST_STREAM frame
/// ```
#[derive(Debug, Clone)]
pub struct State {
    inner: Inner,
}

#[derive(Debug, Clone)]
enum Inner {
    Idle,
    // TODO: these states shouldn't count against concurrency limits:
    ReservedLocal,
    ReservedRemote,
    Open { local: Peer, remote: Peer },
    HalfClosedLocal(Peer), // TODO: explicitly name this value
    HalfClosedRemote(Peer),
    Closed(Cause),
}

#[derive(Debug, Copy, Clone, Default)]
enum Peer {
    #[default]
    AwaitingHeaders,
    Streaming,
}

#[derive(Debug, Clone)]
enum Cause {
    EndStream,
    Error(Error),

    /// This indicates to the connection that a reset frame must be sent out
    /// once the send queue has been flushed.
    ///
    /// Examples of when this could happen:
    /// - User drops all references to a stream, so we want to CANCEL the it.
    /// - Header block size was too large, so we want to REFUSE, possibly
    ///   after sending a 431 response frame.
    ScheduledLibraryReset(Reason),
}

impl State {
    /// Opens the send-half of a stream if it is not already open.
    pub fn send_open(&mut self, eos: bool) -> Result<(), UserError> {
        let local = Streaming;

        self.inner = match self.inner {
            Idle => {
                if eos {
                    HalfClosedLocal(AwaitingHeaders)
                } else {
                    Open {
                        local,
                        remote: AwaitingHeaders,
                    }
                }
            }
            Open {
                local: AwaitingHeaders,
                remote,
            } => {
                if eos {
                    HalfClosedLocal(remote)
                } else {
                    Open { local, remote }
                }
            }
            HalfClosedRemote(AwaitingHeaders) | ReservedLocal => {
                if eos {
                    Closed(Cause::EndStream)
                } else {
                    HalfClosedRemote(local)
                }
            }
            _ => {
                // All other transitions result in a protocol error
                return Err(UserError::UnexpectedFrameType);
            }
        };

        Ok(())
    }

    /// Opens the receive-half of the stream when a HEADERS frame is received.
    ///
    /// Returns true if this transitions the state to Open.
    pub fn recv_open(&mut self, frame: &frame::Headers) -> Result<bool, Error> {
        let mut initial = false;
        let eos = frame.is_end_stream();

        self.inner = match self.inner {
            Idle => {
                initial = true;

                if eos {
                    HalfClosedRemote(AwaitingHeaders)
                } else {
                    Open {
                        local: AwaitingHeaders,
                        remote: if frame.is_informational() {
                            tracing::trace!("skipping 1xx response headers");
                            AwaitingHeaders
                        } else {
                            Streaming
                        },
                    }
                }
            }
            ReservedRemote => {
                initial = true;

                if eos {
                    Closed(Cause::EndStream)
                } else if frame.is_informational() {
                    tracing::trace!("skipping 1xx response headers");
                    ReservedRemote
                } else {
                    HalfClosedLocal(Streaming)
                }
            }
            Open {
                local,
                remote: AwaitingHeaders,
            } => {
                if eos {
                    HalfClosedRemote(local)
                } else {
                    Open {
                        local,
                        remote: if frame.is_informational() {
                            tracing::trace!("skipping 1xx response headers");
                            AwaitingHeaders
                        } else {
                            Streaming
                        },
                    }
                }
            }
            HalfClosedLocal(AwaitingHeaders) => {
                if eos {
                    Closed(Cause::EndStream)
                } else if frame.is_informational() {
                    tracing::trace!("skipping 1xx response headers");
                    HalfClosedLocal(AwaitingHeaders)
                } else {
                    HalfClosedLocal(Streaming)
                }
            }
            ref state => {
                // All other transitions result in a protocol error
                proto_err!(conn: "recv_open: in unexpected state {:?}", state);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }
        };

        Ok(initial)
    }

    /// Transition from Idle -> ReservedRemote
    pub fn reserve_remote(&mut self) -> Result<(), Error> {
        match self.inner {
            Idle => {
                self.inner = ReservedRemote;
                Ok(())
            }
            ref state => {
                proto_err!(conn: "reserve_remote: in unexpected state {:?}", state);
                Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
            }
        }
    }

    /// Transition from Idle -> ReservedLocal
    pub fn reserve_local(&mut self) -> Result<(), UserError> {
        match self.inner {
            Idle => {
                self.inner = ReservedLocal;
                Ok(())
            }
            _ => Err(UserError::UnexpectedFrameType),
        }
    }

    /// Indicates that the remote side will not send more data to the local.
    pub fn recv_close(&mut self) -> Result<(), Error> {
        match self.inner {
            Open { local, .. } => {
                // The remote side will continue to receive data.
                tracing::trace!("recv_close: Open => HalfClosedRemote({:?})", local);
                self.inner = HalfClosedRemote(local);
                Ok(())
            }
            HalfClosedLocal(..) => {
                tracing::trace!("recv_close: HalfClosedLocal => Closed");
                self.inner = Closed(Cause::EndStream);
                Ok(())
            }
            ref state => {
                proto_err!(conn: "recv_close: in unexpected state {:?}", state);
                Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
            }
        }
    }

    /// The remote explicitly sent a RST_STREAM.
    ///
    /// # Arguments
    /// - `frame`: the received RST_STREAM frame.
    /// - `queued`: true if this stream has frames in the pending send queue.
    pub fn recv_reset(&mut self, frame: frame::Reset, queued: bool) {
        match self.inner {
            // If the stream is already in a `Closed` state, do nothing,
            // provided that there are no frames still in the send queue.
            Closed(..) if !queued => {}
            // A notionally `Closed` stream may still have queued frames in
            // the following cases:
            //
            // - if the cause is `Cause::Scheduled(..)` (i.e. we have not
            //   actually closed the stream yet).
            // - if the cause is `Cause::EndStream`: we transition to this
            //   state when an EOS frame is *enqueued* (so that it's invalid
            //   to enqueue more frames), not when the EOS frame is *sent*;
            //   therefore, there may still be frames ahead of the EOS frame
            //   in the send queue.
            //
            // In either of these cases, we want to overwrite the stream's
            // previous state with the received RST_STREAM, so that the queue
            // will be cleared by `Prioritize::pop_frame`.
            ref state => {
                tracing::trace!(
                    "recv_reset; frame={:?}; state={:?}; queued={:?}",
                    frame,
                    state,
                    queued
                );
                self.inner = Closed(Cause::Error(Error::remote_reset(
                    frame.stream_id(),
                    frame.reason(),
                )));
            }
        }
    }

    /// Handle a connection-level error.
    pub fn handle_error(&mut self, err: &proto::Error) {
        match self.inner {
            Closed(..) => {}
            _ => {
                tracing::trace!("handle_error; err={:?}", err);
                self.inner = Closed(Cause::Error(err.clone()));
            }
        }
    }

    pub fn recv_eof(&mut self) {
        match self.inner {
            Closed(..) => {}
            ref state => {
                tracing::trace!("recv_eof; state={:?}", state);
                self.inner = Closed(Cause::Error(
                    io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "stream closed because of a broken pipe",
                    )
                    .into(),
                ));
            }
        }
    }

    /// Indicates that the local side will not send more data to the local.
    pub fn send_close(&mut self) {
        match self.inner {
            Open { remote, .. } => {
                // The remote side will continue to receive data.
                tracing::trace!("send_close: Open => HalfClosedLocal({:?})", remote);
                self.inner = HalfClosedLocal(remote);
            }
            HalfClosedRemote(..) => {
                tracing::trace!("send_close: HalfClosedRemote => Closed");
                self.inner = Closed(Cause::EndStream);
            }
            ref state => panic!("send_close: unexpected state {:?}", state),
        }
    }

    /// Set the stream state to reset locally.
    pub fn set_reset(&mut self, stream_id: StreamId, reason: Reason, initiator: Initiator) {
        self.inner = Closed(Cause::Error(Error::Reset(stream_id, reason, initiator)));
    }

    /// Set the stream state to a scheduled reset.
    pub fn set_scheduled_reset(&mut self, reason: Reason) {
        debug_assert!(!self.is_closed());
        self.inner = Closed(Cause::ScheduledLibraryReset(reason));
    }

    pub fn get_scheduled_reset(&self) -> Option<Reason> {
        match self.inner {
            Closed(Cause::ScheduledLibraryReset(reason)) => Some(reason),
            _ => None,
        }
    }

    pub fn is_scheduled_reset(&self) -> bool {
        matches!(self.inner, Closed(Cause::ScheduledLibraryReset(..)))
    }

    pub fn is_local_error(&self) -> bool {
        match self.inner {
            Closed(Cause::Error(ref e)) => e.is_local(),
            Closed(Cause::ScheduledLibraryReset(..)) => true,
            _ => false,
        }
    }

    pub fn is_remote_reset(&self) -> bool {
        matches!(
            self.inner,
            Closed(Cause::Error(Error::Reset(_, _, Initiator::Remote)))
        )
    }

    /// Returns true if the stream is already reset.
    pub fn is_reset(&self) -> bool {
        match self.inner {
            Closed(Cause::EndStream) => false,
            Closed(_) => true,
            _ => false,
        }
    }

    pub fn is_send_streaming(&self) -> bool {
        matches!(
            self.inner,
            Open {
                local: Streaming,
                ..
            } | HalfClosedRemote(Streaming)
        )
    }

    /// Returns true when the stream is in a state to receive headers
    pub fn is_recv_headers(&self) -> bool {
        matches!(
            self.inner,
            Idle | Open {
                remote: AwaitingHeaders,
                ..
            } | HalfClosedLocal(AwaitingHeaders)
                | ReservedRemote
        )
    }

    pub fn is_recv_streaming(&self) -> bool {
        matches!(
            self.inner,
            Open {
                remote: Streaming,
                ..
            } | HalfClosedLocal(Streaming)
        )
    }

    pub fn is_recv_end_stream(&self) -> bool {
        // In either case END_STREAM has been received
        matches!(self.inner, Closed(Cause::EndStream) | HalfClosedRemote(..))
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.inner, Closed(_))
    }

    pub fn is_send_closed(&self) -> bool {
        matches!(
            self.inner,
            Closed(..) | HalfClosedLocal(..) | ReservedRemote
        )
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.inner, Idle)
    }

    pub fn ensure_recv_open(&self) -> Result<bool, proto::Error> {
        // TODO: Is this correct?
        match self.inner {
            Closed(Cause::Error(ref e)) => Err(e.clone()),
            Closed(Cause::ScheduledLibraryReset(reason)) => {
                Err(proto::Error::library_go_away(reason))
            }
            Closed(Cause::EndStream) | HalfClosedRemote(..) | ReservedLocal => Ok(false),
            _ => Ok(true),
        }
    }

    /// Returns a reason if the stream has been reset.
    pub(super) fn ensure_reason(&self, mode: PollReset) -> Result<Option<Reason>, crate::Error> {
        match self.inner {
            Closed(Cause::Error(Error::Reset(_, reason, _)))
            | Closed(Cause::Error(Error::GoAway(_, reason, _)))
            | Closed(Cause::ScheduledLibraryReset(reason)) => Ok(Some(reason)),
            Closed(Cause::Error(ref e)) => Err(e.clone().into()),
            Open {
                local: Streaming, ..
            }
            | HalfClosedRemote(Streaming) => match mode {
                PollReset::AwaitingHeaders => Err(UserError::PollResetAfterSendResponse.into()),
                PollReset::Streaming => Ok(None),
            },
            _ => Ok(None),
        }
    }
}

impl Default for State {
    fn default() -> State {
        State { inner: Inner::Idle }
    }
}
