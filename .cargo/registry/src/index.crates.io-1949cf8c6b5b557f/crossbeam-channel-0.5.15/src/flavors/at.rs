//! Channel that delivers a message at a certain moment in time.
//!
//! Messages cannot be sent into this kind of channel; they are materialized on demand.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use crate::context::Context;
use crate::err::{RecvTimeoutError, TryRecvError};
use crate::select::{Operation, SelectHandle, Token};
use crate::utils;

/// Result of a receive operation.
pub(crate) type AtToken = Option<Instant>;

/// Channel that delivers a message at a certain moment in time
pub(crate) struct Channel {
    /// The instant at which the message will be delivered.
    delivery_time: Instant,

    /// `true` if the message has been received.
    received: AtomicBool,
}

impl Channel {
    /// Creates a channel that delivers a message at a certain instant in time.
    #[inline]
    pub(crate) fn new_deadline(when: Instant) -> Self {
        Channel {
            delivery_time: when,
            received: AtomicBool::new(false),
        }
    }

    /// Attempts to receive a message without blocking.
    #[inline]
    pub(crate) fn try_recv(&self) -> Result<Instant, TryRecvError> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            // The message has already been received.
            return Err(TryRecvError::Empty);
        }

        if Instant::now() < self.delivery_time {
            // The message was not delivered yet.
            return Err(TryRecvError::Empty);
        }

        // Try receiving the message if it is still available.
        if !self.received.swap(true, Ordering::SeqCst) {
            // Success! Return delivery time as the message.
            Ok(self.delivery_time)
        } else {
            // The message was already received.
            Err(TryRecvError::Empty)
        }
    }

    /// Receives a message from the channel.
    #[inline]
    pub(crate) fn recv(&self, deadline: Option<Instant>) -> Result<Instant, RecvTimeoutError> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            // The message has already been received.
            utils::sleep_until(deadline);
            return Err(RecvTimeoutError::Timeout);
        }

        // Wait until the message is received or the deadline is reached.
        loop {
            let now = Instant::now();

            let deadline = match deadline {
                // Check if we can receive the next message.
                _ if now >= self.delivery_time => break,
                // Check if the timeout deadline has been reached.
                Some(d) if now >= d => return Err(RecvTimeoutError::Timeout),

                // Sleep until one of the above happens
                Some(d) if d < self.delivery_time => d,
                _ => self.delivery_time,
            };

            thread::sleep(deadline - now);
        }

        // Try receiving the message if it is still available.
        if !self.received.swap(true, Ordering::SeqCst) {
            // Success! Return the message, which is the instant at which it was delivered.
            Ok(self.delivery_time)
        } else {
            // The message was already received. Block forever.
            utils::sleep_until(None);
            unreachable!()
        }
    }

    /// Reads a message from the channel.
    #[inline]
    pub(crate) unsafe fn read(&self, token: &mut Token) -> Result<Instant, ()> {
        token.at.ok_or(())
    }

    /// Returns `true` if the channel is empty.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            return true;
        }

        // If the delivery time hasn't been reached yet, the channel is empty.
        if Instant::now() < self.delivery_time {
            return true;
        }

        // The delivery time has been reached. The channel is empty only if the message has already
        // been received.
        self.received.load(Ordering::SeqCst)
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
                token.at = Some(msg);
                true
            }
            Err(TryRecvError::Disconnected) => {
                token.at = None;
                true
            }
            Err(TryRecvError::Empty) => false,
        }
    }

    #[inline]
    fn deadline(&self) -> Option<Instant> {
        // We use relaxed ordering because this is just an optional optimistic check.
        if self.received.load(Ordering::Relaxed) {
            None
        } else {
            Some(self.delivery_time)
        }
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
