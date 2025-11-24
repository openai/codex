use std::fmt;

use super::afd;
use super::iocp::CompletionStatus;
use crate::Token;

#[derive(Clone)]
pub struct Event {
    pub flags: u32,
    pub data: u64,
}

pub fn token(event: &Event) -> Token {
    Token(event.data as usize)
}

impl Event {
    pub(super) fn new(token: Token) -> Event {
        Event {
            flags: 0,
            data: usize::from(token) as u64,
        }
    }

    pub(super) fn set_readable(&mut self) {
        self.flags |= afd::POLL_RECEIVE
    }

    #[cfg(feature = "os-ext")]
    pub(super) fn set_writable(&mut self) {
        self.flags |= afd::POLL_SEND;
    }

    pub(super) fn from_completion_status(status: &CompletionStatus) -> Event {
        Event {
            flags: status.bytes_transferred(),
            data: status.token() as u64,
        }
    }

    pub(super) fn to_completion_status(&self) -> CompletionStatus {
        CompletionStatus::new(self.flags, self.data as usize, std::ptr::null_mut())
    }

    #[cfg(feature = "os-ext")]
    pub(super) fn to_completion_status_with_overlapped(
        &self,
        overlapped: *mut super::Overlapped,
    ) -> CompletionStatus {
        CompletionStatus::new(self.flags, self.data as usize, overlapped)
    }
}

pub(crate) const READABLE_FLAGS: u32 = afd::POLL_RECEIVE
    | afd::POLL_DISCONNECT
    | afd::POLL_ACCEPT
    | afd::POLL_ABORT
    | afd::POLL_CONNECT_FAIL;
pub(crate) const WRITABLE_FLAGS: u32 = afd::POLL_SEND | afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;
pub(crate) const ERROR_FLAGS: u32 = afd::POLL_CONNECT_FAIL;
pub(crate) const READ_CLOSED_FLAGS: u32 =
    afd::POLL_DISCONNECT | afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;
pub(crate) const WRITE_CLOSED_FLAGS: u32 = afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;

pub fn is_readable(event: &Event) -> bool {
    event.flags & READABLE_FLAGS != 0
}

pub fn is_writable(event: &Event) -> bool {
    event.flags & WRITABLE_FLAGS != 0
}

pub fn is_error(event: &Event) -> bool {
    event.flags & ERROR_FLAGS != 0
}

pub fn is_read_closed(event: &Event) -> bool {
    event.flags & READ_CLOSED_FLAGS != 0
}

pub fn is_write_closed(event: &Event) -> bool {
    event.flags & WRITE_CLOSED_FLAGS != 0
}

pub fn is_priority(event: &Event) -> bool {
    event.flags & afd::POLL_RECEIVE_EXPEDITED != 0
}

pub fn is_aio(_: &Event) -> bool {
    // Not supported.
    false
}

pub fn is_lio(_: &Event) -> bool {
    // Not supported.
    false
}

pub fn debug_details(f: &mut fmt::Formatter<'_>, event: &Event) -> fmt::Result {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn check_flags(got: &u32, want: &u32) -> bool {
        (got & want) != 0
    }
    debug_detail!(
        FlagsDetails(u32),
        check_flags,
        afd::POLL_RECEIVE,
        afd::POLL_RECEIVE_EXPEDITED,
        afd::POLL_SEND,
        afd::POLL_DISCONNECT,
        afd::POLL_ABORT,
        afd::POLL_LOCAL_CLOSE,
        afd::POLL_CONNECT,
        afd::POLL_ACCEPT,
        afd::POLL_CONNECT_FAIL,
    );

    f.debug_struct("event")
        .field("flags", &FlagsDetails(event.flags))
        .field("data", &event.data)
        .finish()
}

pub struct Events {
    /// Raw I/O event completions are filled in here by the call to `get_many`
    /// on the completion port above. These are then processed to run callbacks
    /// which figure out what to do after the event is done.
    pub statuses: Box<[CompletionStatus]>,

    /// Literal events returned by `get` to the upwards `EventLoop`. This file
    /// doesn't really modify this (except for the waker), instead almost all
    /// events are filled in by the `ReadinessQueue` from the `poll` module.
    pub events: Vec<Event>,
}

impl Events {
    pub fn with_capacity(cap: usize) -> Events {
        // Note that it's possible for the output `events` to grow beyond the
        // capacity as it can also include deferred events, but that's certainly
        // not the end of the world!
        Events {
            statuses: vec![CompletionStatus::zero(); cap].into_boxed_slice(),
            events: Vec::with_capacity(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn get(&self, idx: usize) -> Option<&Event> {
        self.events.get(idx)
    }

    pub fn clear(&mut self) {
        self.events.clear();
        for status in self.statuses.iter_mut() {
            *status = CompletionStatus::zero();
        }
    }
}
