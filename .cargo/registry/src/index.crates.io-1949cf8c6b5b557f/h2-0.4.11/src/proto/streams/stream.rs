use crate::Reason;

use super::*;

use std::fmt;
use std::task::{Context, Waker};
use std::time::Instant;

/// Tracks Stream related state
///
/// # Reference counting
///
/// There can be a number of outstanding handles to a single Stream. These are
/// tracked using reference counting. The `ref_count` field represents the
/// number of outstanding userspace handles that can reach this stream.
///
/// It's important to note that when the stream is placed in an internal queue
/// (such as an accept queue), this is **not** tracked by a reference count.
/// Thus, `ref_count` can be zero and the stream still has to be kept around.
pub(super) struct Stream {
    /// The h2 stream identifier
    pub id: StreamId,

    /// Current state of the stream
    pub state: State,

    /// Set to `true` when the stream is counted against the connection's max
    /// concurrent streams.
    pub is_counted: bool,

    /// Number of outstanding handles pointing to this stream
    pub ref_count: usize,

    // ===== Fields related to sending =====
    /// Next node in the accept linked list
    pub next_pending_send: Option<store::Key>,

    /// Set to true when the stream is pending accept
    pub is_pending_send: bool,

    /// Send data flow control
    pub send_flow: FlowControl,

    /// Amount of send capacity that has been requested, but not yet allocated.
    pub requested_send_capacity: WindowSize,

    /// Amount of data buffered at the prioritization layer.
    /// TODO: Technically this could be greater than the window size...
    pub buffered_send_data: usize,

    /// Task tracking additional send capacity (i.e. window updates).
    send_task: Option<Waker>,

    /// Frames pending for this stream being sent to the socket
    pub pending_send: buffer::Deque,

    /// Next node in the linked list of streams waiting for additional
    /// connection level capacity.
    pub next_pending_send_capacity: Option<store::Key>,

    /// True if the stream is waiting for outbound connection capacity
    pub is_pending_send_capacity: bool,

    /// Set to true when the send capacity has been incremented
    pub send_capacity_inc: bool,

    /// Next node in the open linked list
    pub next_open: Option<store::Key>,

    /// Set to true when the stream is pending to be opened
    pub is_pending_open: bool,

    /// Set to true when a push is pending for this stream
    pub is_pending_push: bool,

    // ===== Fields related to receiving =====
    /// Next node in the accept linked list
    pub next_pending_accept: Option<store::Key>,

    /// Set to true when the stream is pending accept
    pub is_pending_accept: bool,

    /// Receive data flow control
    pub recv_flow: FlowControl,

    pub in_flight_recv_data: WindowSize,

    /// Next node in the linked list of streams waiting to send window updates.
    pub next_window_update: Option<store::Key>,

    /// True if the stream is waiting to send a window update
    pub is_pending_window_update: bool,

    /// The time when this stream may have been locally reset.
    pub reset_at: Option<Instant>,

    /// Next node in list of reset streams that should expire eventually
    pub next_reset_expire: Option<store::Key>,

    /// Frames pending for this stream to read
    pub pending_recv: buffer::Deque,

    /// When the RecvStream drop occurs, no data should be received.
    pub is_recv: bool,

    /// Task tracking receiving frames
    pub recv_task: Option<Waker>,

    /// Task tracking pushed promises.
    pub push_task: Option<Waker>,

    /// The stream's pending push promises
    pub pending_push_promises: store::Queue<NextAccept>,

    /// Validate content-length headers
    pub content_length: ContentLength,
}

/// State related to validating a stream's content-length
#[derive(Debug)]
pub enum ContentLength {
    Omitted,
    Head,
    Remaining(u64),
}

#[derive(Debug)]
pub(super) struct NextAccept;

#[derive(Debug)]
pub(super) struct NextSend;

#[derive(Debug)]
pub(super) struct NextSendCapacity;

#[derive(Debug)]
pub(super) struct NextWindowUpdate;

#[derive(Debug)]
pub(super) struct NextOpen;

#[derive(Debug)]
pub(super) struct NextResetExpire;

impl Stream {
    pub fn new(id: StreamId, init_send_window: WindowSize, init_recv_window: WindowSize) -> Stream {
        let mut send_flow = FlowControl::new();
        let mut recv_flow = FlowControl::new();

        recv_flow
            .inc_window(init_recv_window)
            .expect("invalid initial receive window");
        // TODO: proper error handling?
        let _res = recv_flow.assign_capacity(init_recv_window);
        debug_assert!(_res.is_ok());

        send_flow
            .inc_window(init_send_window)
            .expect("invalid initial send window size");

        Stream {
            id,
            state: State::default(),
            ref_count: 0,
            is_counted: false,

            // ===== Fields related to sending =====
            next_pending_send: None,
            is_pending_send: false,
            send_flow,
            requested_send_capacity: 0,
            buffered_send_data: 0,
            send_task: None,
            pending_send: buffer::Deque::new(),
            is_pending_send_capacity: false,
            next_pending_send_capacity: None,
            send_capacity_inc: false,
            is_pending_open: false,
            next_open: None,
            is_pending_push: false,

            // ===== Fields related to receiving =====
            next_pending_accept: None,
            is_pending_accept: false,
            recv_flow,
            in_flight_recv_data: 0,
            next_window_update: None,
            is_pending_window_update: false,
            reset_at: None,
            next_reset_expire: None,
            pending_recv: buffer::Deque::new(),
            is_recv: true,
            recv_task: None,
            push_task: None,
            pending_push_promises: store::Queue::new(),
            content_length: ContentLength::Omitted,
        }
    }

    /// Increment the stream's ref count
    pub fn ref_inc(&mut self) {
        assert!(self.ref_count < usize::MAX);
        self.ref_count += 1;
    }

    /// Decrements the stream's ref count
    pub fn ref_dec(&mut self) {
        assert!(self.ref_count > 0);
        self.ref_count -= 1;
    }

    /// Returns true if stream is currently being held for some time because of
    /// a local reset.
    pub fn is_pending_reset_expiration(&self) -> bool {
        self.reset_at.is_some()
    }

    /// Returns true if frames for this stream are ready to be sent over the wire
    pub fn is_send_ready(&self) -> bool {
        // Why do we check pending_open?
        //
        // We allow users to call send_request() which schedules a stream to be pending_open
        // if there is no room according to the concurrency limit (max_send_streams), and we
        // also allow data to be buffered for send with send_data() if there is no capacity for
        // the stream to send the data, which attempts to place the stream in pending_send.
        // If the stream is not open, we don't want the stream to be scheduled for
        // execution (pending_send). Note that if the stream is in pending_open, it will be
        // pushed to pending_send when there is room for an open stream.
        //
        // In pending_push we track whether a PushPromise still needs to be sent
        // from a different stream before we can start sending frames on this one.
        // This is different from the "open" check because reserved streams don't count
        // toward the concurrency limit.
        // See https://httpwg.org/specs/rfc7540.html#rfc.section.5.1.2
        !self.is_pending_open && !self.is_pending_push
    }

    /// Returns true if the stream is closed
    pub fn is_closed(&self) -> bool {
        // The state has fully transitioned to closed.
        self.state.is_closed() &&
            // Because outbound frames transition the stream state before being
            // buffered, we have to ensure that all frames have been flushed.
            self.pending_send.is_empty() &&
            // Sometimes large data frames are sent out in chunks. After a chunk
            // of the frame is sent, the remainder is pushed back onto the send
            // queue to be rescheduled.
            //
            // Checking for additional buffered data lets us catch this case.
            self.buffered_send_data == 0
    }

    /// Returns true if the stream is no longer in use
    pub fn is_released(&self) -> bool {
        // The stream is closed and fully flushed
        self.is_closed() &&
            // There are no more outstanding references to the stream
            self.ref_count == 0 &&
            // The stream is not in any queue
            !self.is_pending_send && !self.is_pending_send_capacity &&
            !self.is_pending_accept && !self.is_pending_window_update &&
            !self.is_pending_open && self.reset_at.is_none()
    }

    /// Returns true when the consumer of the stream has dropped all handles
    /// (indicating no further interest in the stream) and the stream state is
    /// not actually closed.
    ///
    /// In this case, a reset should be sent.
    pub fn is_canceled_interest(&self) -> bool {
        self.ref_count == 0 && !self.state.is_closed()
    }

    /// Current available stream send capacity
    pub fn capacity(&self, max_buffer_size: usize) -> WindowSize {
        let available = self.send_flow.available().as_size() as usize;
        let buffered = self.buffered_send_data;

        available.min(max_buffer_size).saturating_sub(buffered) as WindowSize
    }

    pub fn assign_capacity(&mut self, capacity: WindowSize, max_buffer_size: usize) {
        let prev_capacity = self.capacity(max_buffer_size);
        debug_assert!(capacity > 0);
        // TODO: proper error handling
        let _res = self.send_flow.assign_capacity(capacity);
        debug_assert!(_res.is_ok());

        tracing::trace!(
            "  assigned capacity to stream; available={}; buffered={}; id={:?}; max_buffer_size={} prev={}",
            self.send_flow.available(),
            self.buffered_send_data,
            self.id,
            max_buffer_size,
            prev_capacity,
        );

        if prev_capacity < self.capacity(max_buffer_size) {
            self.notify_capacity();
        }
    }

    pub fn send_data(&mut self, len: WindowSize, max_buffer_size: usize) {
        let prev_capacity = self.capacity(max_buffer_size);

        // TODO: proper error handling
        let _res = self.send_flow.send_data(len);
        debug_assert!(_res.is_ok());

        // Decrement the stream's buffered data counter
        debug_assert!(self.buffered_send_data >= len as usize);
        self.buffered_send_data -= len as usize;
        self.requested_send_capacity -= len;

        tracing::trace!(
            "  sent stream data; available={}; buffered={}; id={:?}; max_buffer_size={} prev={}",
            self.send_flow.available(),
            self.buffered_send_data,
            self.id,
            max_buffer_size,
            prev_capacity,
        );

        if prev_capacity < self.capacity(max_buffer_size) {
            self.notify_capacity();
        }
    }

    /// If the capacity was limited because of the max_send_buffer_size,
    /// then consider waking the send task again...
    pub fn notify_capacity(&mut self) {
        self.send_capacity_inc = true;
        tracing::trace!("  notifying task");
        self.notify_send();
    }

    /// Returns `Err` when the decrement cannot be completed due to overflow.
    pub fn dec_content_length(&mut self, len: usize) -> Result<(), ()> {
        match self.content_length {
            ContentLength::Remaining(ref mut rem) => match rem.checked_sub(len as u64) {
                Some(val) => *rem = val,
                None => return Err(()),
            },
            ContentLength::Head => {
                if len != 0 {
                    return Err(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn ensure_content_length_zero(&self) -> Result<(), ()> {
        match self.content_length {
            ContentLength::Remaining(0) => Ok(()),
            ContentLength::Remaining(_) => Err(()),
            _ => Ok(()),
        }
    }

    pub fn notify_send(&mut self) {
        if let Some(task) = self.send_task.take() {
            task.wake();
        }
    }

    pub fn wait_send(&mut self, cx: &Context) {
        self.send_task = Some(cx.waker().clone());
    }

    pub fn notify_recv(&mut self) {
        if let Some(task) = self.recv_task.take() {
            task.wake();
        }
    }

    pub(super) fn notify_push(&mut self) {
        if let Some(task) = self.push_task.take() {
            task.wake();
        }
    }

    /// Set the stream's state to `Closed` with the given reason and initiator.
    /// Notify the send and receive tasks, if they exist.
    pub(super) fn set_reset(&mut self, reason: Reason, initiator: Initiator) {
        self.state.set_reset(self.id, reason, initiator);
        self.notify_push();
        self.notify_recv();
    }
}

impl fmt::Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stream")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("is_counted", &self.is_counted)
            .field("ref_count", &self.ref_count)
            .field("next_pending_send", &self.next_pending_send)
            .field("is_pending_send", &self.is_pending_send)
            .field("send_flow", &self.send_flow)
            .field("requested_send_capacity", &self.requested_send_capacity)
            .field("buffered_send_data", &self.buffered_send_data)
            .field("send_task", &self.send_task.as_ref().map(|_| ()))
            .field("pending_send", &self.pending_send)
            .field(
                "next_pending_send_capacity",
                &self.next_pending_send_capacity,
            )
            .field("is_pending_send_capacity", &self.is_pending_send_capacity)
            .field("send_capacity_inc", &self.send_capacity_inc)
            .field("next_open", &self.next_open)
            .field("is_pending_open", &self.is_pending_open)
            .field("is_pending_push", &self.is_pending_push)
            .field("next_pending_accept", &self.next_pending_accept)
            .field("is_pending_accept", &self.is_pending_accept)
            .field("recv_flow", &self.recv_flow)
            .field("in_flight_recv_data", &self.in_flight_recv_data)
            .field("next_window_update", &self.next_window_update)
            .field("is_pending_window_update", &self.is_pending_window_update)
            .field("reset_at", &self.reset_at)
            .field("next_reset_expire", &self.next_reset_expire)
            .field("pending_recv", &self.pending_recv)
            .field("is_recv", &self.is_recv)
            .field("recv_task", &self.recv_task.as_ref().map(|_| ()))
            .field("push_task", &self.push_task.as_ref().map(|_| ()))
            .field("pending_push_promises", &self.pending_push_promises)
            .field("content_length", &self.content_length)
            .finish()
    }
}

impl store::Next for NextAccept {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_accept
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_accept = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_accept.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_accept
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_accept = val;
    }
}

impl store::Next for NextSend {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_send
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_send = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_send.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_send
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            // ensure that stream is not queued for being opened
            // if it's being put into queue for sending data
            debug_assert!(!stream.is_pending_open);
        }
        stream.is_pending_send = val;
    }
}

impl store::Next for NextSendCapacity {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_send_capacity
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_send_capacity = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_send_capacity.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_send_capacity
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_send_capacity = val;
    }
}

impl store::Next for NextWindowUpdate {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_window_update
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_window_update = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_window_update.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_window_update
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_window_update = val;
    }
}

impl store::Next for NextOpen {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_open
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_open = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_open.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_open
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            // ensure that stream is not queued for being sent
            // if it's being put into queue for opening the stream
            debug_assert!(!stream.is_pending_send);
        }
        stream.is_pending_open = val;
    }
}

impl store::Next for NextResetExpire {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_reset_expire
    }

    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_reset_expire = key;
    }

    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_reset_expire.take()
    }

    fn is_queued(stream: &Stream) -> bool {
        stream.reset_at.is_some()
    }

    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            stream.reset_at = Some(Instant::now());
        } else {
            stream.reset_at = None;
        }
    }
}

// ===== impl ContentLength =====

impl ContentLength {
    pub fn is_head(&self) -> bool {
        matches!(*self, Self::Head)
    }
}
