//! Channel that delivers messages periodically.
//!
//! Messages cannot be sent into this kind of channel; they are materialized on demand.

use std::thread;
use std::time::{Duration, Instant};

use crossbeam_utils::atomic::AtomicCell;

use crate::context::Context;
use crate::err::{RecvTimeoutError, TryRecvError};
use crate::select::{Operation, SelectHandle, Token};

/// Result of a receive operation.
pub(crate) type TickToken = Option<Instant>;

/// Channel that delivers messages periodically.
pub(crate) struct Channel {
    /// The instant at which the next message will be delivered.
    delivery_time: AtomicCell<Instant>,

    /// The time interval in which messages get delivered.
    duration: Duration,
}

impl Channel {
    /// Creates a channel that delivers messages periodically.
    #[inline]
    pub(crate) fn new(delivery_time: Instant, dur: Duration) -> Self {
        Channel {
            delivery_time: AtomicCell::new(delivery_time),
            duration: dur,
        }
    }

    /// Attempts to receive a message without blocking.
    #[inline]
    pub(crate) fn try_recv(&self) -> Result<Instant, TryRecvError> {
        loop {
            let now = Instant::now();
            let delivery_time = self.delivery_time.load();

            if now < delivery_time {
                return Err(TryRecvError::Empty);
            }

            if self
                .delivery_time
                .compare_exchange(delivery_time, now + self.duration)
                .is_ok()
            {
                return Ok(delivery_time);
            }
        }
    }

    /// Receives a message from the channel.
    #[inline]
    pub(crate) fn recv(&self, deadline: Option<Instant>) -> Result<Instant, RecvTimeoutError> {
        loop {
            let delivery_time = self.delivery_time.load();
            let now = Instant::now();

            if let Some(d) = deadline {
                if d < delivery_time {
                    if now < d {
                        thread::sleep(d - now);
                    }
                    return Err(RecvTimeoutError::Timeout);
                }
            }

            if self
                .delivery_time
                .compare_exchange(delivery_time, delivery_time.max(now) + self.duration)
                .is_ok()
            {
                if now < delivery_time {
                    thread::sleep(delivery_time - now);
                }
                return Ok(delivery_time);
            }
        }
    }

    /// Reads a message from the channel.
    #[inline]
    pub(crate) unsafe fn read(&self, token: &mut Token) -> Result<Instant, ()> {
        token.tick.ok_or(())
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        Instant::now() < self.delivery_time.load()
    }

    /// Returns `true` if the channel is full.
    #[inline]
    pub(crate) fn is_full(&self) -> bool {
        !self.is_empty()
    }

    /// Returns the number of messages in the channel.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            1
        }
    }

    /// Returns the capacity of the channel.
    #[inline]
    pub(crate) fn capacity(&self) -> Option<usize> {
        Some(1)
    }
}

impl SelectHandle for Channel {
    #[inline]
    fn try_select(&self, token: &mut Token) -> bool {
        match self.try_recv() {
            Ok(msg) => {
                token.tick = Some(msg);
                true
            }
            Err(TryRecvError::Disconnected) => {
                token.tick = None;
                true
            }
            Err(TryRecvError::Empty) => false,
        }
    }

    #[inline]
    fn deadline(&self) -> Option<Instant> {
        Some(self.delivery_time.load())
    }

    #[inline]
    fn register(&self, _oper: Operation, _cx: &Context) -> bool {
        self.is_ready()
    }

    #[inline]
    fn unregister(&self, _oper: Operation) {}

    #[inline]
    fn accept(&self, token: &mut Token, _cx: &Context) -> bool {
        self.try_select(token)
    }

    #[inline]
    fn is_ready(&self) -> bool {
        !self.is_empty()
    }

    #[inline]
    fn watch(&self, _oper: Operation, _cx: &Context) -> bool {
        self.is_ready()
    }

    #[inline]
    fn unwatch(&self, _oper: Operation) {}
}
