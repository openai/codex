use crate::codec::Codec;
use crate::frame::{self, Reason, StreamId};

use bytes::Buf;
use std::io;
use std::task::{Context, Poll};
use tokio::io::AsyncWrite;

/// Manages our sending of GOAWAY frames.
#[derive(Debug)]
pub(super) struct GoAway {
    /// Whether the connection should close now, or wait until idle.
    close_now: bool,
    /// Records if we've sent any GOAWAY before.
    going_away: Option<GoingAway>,
    /// Whether the user started the GOAWAY by calling `abrupt_shutdown`.
    is_user_initiated: bool,
    /// A GOAWAY frame that must be buffered in the Codec immediately.
    pending: Option<frame::GoAway>,
}

/// Keeps a memory of any GOAWAY frames we've sent before.
///
/// This looks very similar to a `frame::GoAway`, but is a separate type. Why?
/// Mostly for documentation purposes. This type is to record status. If it
/// were a `frame::GoAway`, it might appear like we eventually wanted to
/// serialize it. We **only** want to be able to look up these fields at a
/// later time.
#[derive(Debug)]
pub(crate) struct GoingAway {
    /// Stores the highest stream ID of a GOAWAY that has been sent.
    ///
    /// It's illegal to send a subsequent GOAWAY with a higher ID.
    last_processed_id: StreamId,

    /// Records the error code of any GOAWAY frame sent.
    reason: Reason,
}

impl GoAway {
    pub fn new() -> Self {
        GoAway {
            close_now: false,
            going_away: None,
            is_user_initiated: false,
            pending: None,
        }
    }

    /// Enqueue a GOAWAY frame to be written.
    ///
    /// The connection is expected to continue to run until idle.
    pub fn go_away(&mut self, f: frame::GoAway) {
        if let Some(ref going_away) = self.going_away {
            assert!(
                f.last_stream_id() <= going_away.last_processed_id,
                "GOAWAY stream IDs shouldn't be higher; \
                 last_processed_id = {:?}, f.last_stream_id() = {:?}",
                going_away.last_processed_id,
                f.last_stream_id(),
            );
        }

        self.going_away = Some(GoingAway {
            last_processed_id: f.last_stream_id(),
            reason: f.reason(),
        });
        self.pending = Some(f);
    }

    pub fn go_away_now(&mut self, f: frame::GoAway) {
        self.close_now = true;
        if let Some(ref going_away) = self.going_away {
            // Prevent sending the same GOAWAY twice.
            if going_away.last_processed_id == f.last_stream_id() && going_away.reason == f.reason()
            {
                return;
            }
        }
        self.go_away(f);
    }

    pub fn go_away_from_user(&mut self, f: frame::GoAway) {
        self.is_user_initiated = true;
        self.go_away_now(f);
    }

    /// Return if a GOAWAY has ever been scheduled.
    pub fn is_going_away(&self) -> bool {
        self.going_away.is_some()
    }

    pub fn is_user_initiated(&self) -> bool {
        self.is_user_initiated
    }

    /// Returns the going away info, if any.
    pub fn going_away(&self) -> Option<&GoingAway> {
        self.going_away.as_ref()
    }

    /// Returns if the connection should close now, or wait until idle.
    pub fn should_close_now(&self) -> bool {
        self.pending.is_none() && self.close_now
    }

    /// Returns if the connection should be closed when idle.
    pub fn should_close_on_idle(&self) -> bool {
        !self.close_now
            && self
                .going_away
                .as_ref()
                .map(|g| g.last_processed_id != StreamId::MAX)
                .unwrap_or(false)
    }

    /// Try to write a pending GOAWAY frame to the buffer.
    ///
    /// If a frame is written, the `Reason` of the GOAWAY is returned.
    pub fn send_pending_go_away<T, B>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, B>,
    ) -> Poll<Option<io::Result<Reason>>>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if let Some(frame) = self.pending.take() {
            if !dst.poll_ready(cx)?.is_ready() {
                self.pending = Some(frame);
                return Poll::Pending;
            }

            let reason = frame.reason();
            dst.buffer(frame.into()).expect("invalid GOAWAY frame");

            return Poll::Ready(Some(Ok(reason)));
        } else if self.should_close_now() {
            return match self.going_away().map(|going_away| going_away.reason) {
                Some(reason) => Poll::Ready(Some(Ok(reason))),
                None => Poll::Ready(None),
            };
        }

        Poll::Ready(None)
    }
}

impl GoingAway {
    pub(crate) fn reason(&self) -> Reason {
        self.reason
    }
}
