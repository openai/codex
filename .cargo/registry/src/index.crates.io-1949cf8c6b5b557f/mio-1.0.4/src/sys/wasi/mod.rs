//! # Notes
//!
//! The current implementation is somewhat limited. The `Waker` is not
//! implemented, as at the time of writing there is no way to support to wake-up
//! a thread from calling `poll_oneoff`.
//!
//! Furthermore the (re/de)register functions also don't work while concurrently
//! polling as both registering and polling requires a lock on the
//! `subscriptions`.
//!
//! Finally `Selector::try_clone`, required by `Registry::try_clone`, doesn't
//! work. However this could be implemented by use of an `Arc`.
//!
//! In summary, this only (barely) works using a single thread.

use std::cmp::min;
use std::io;
#[cfg(all(feature = "net", debug_assertions))]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(feature = "net")]
use crate::{Interest, Token};

cfg_net! {
    pub(crate) mod tcp {
        use std::io;
        use std::net::{self, SocketAddr};

        pub(crate) fn accept(listener: &net::TcpListener) -> io::Result<(net::TcpStream, SocketAddr)> {
            let (stream, addr) = listener.accept()?;
            stream.set_nonblocking(true)?;
            Ok((stream, addr))
        }
    }
}

/// Unique id for use as `SelectorId`.
#[cfg(all(debug_assertions, feature = "net"))]
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

pub(crate) struct Selector {
    #[cfg(all(debug_assertions, feature = "net"))]
    id: usize,
    /// Subscriptions (reads events) we're interested in.
    subscriptions: Arc<Mutex<Vec<wasi::Subscription>>>,
}

impl Selector {
    pub(crate) fn new() -> io::Result<Selector> {
        Ok(Selector {
            #[cfg(all(debug_assertions, feature = "net"))]
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            subscriptions: Arc::new(Mutex::new(Vec::new())),
        })
    }

    #[cfg(all(debug_assertions, feature = "net"))]
    pub(crate) fn id(&self) -> usize {
        self.id
    }

    pub(crate) fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();

        let mut subscriptions = self.subscriptions.lock().unwrap();

        // If we want to a use a timeout in the `wasi_poll_oneoff()` function
        // we need another subscription to the list.
        if let Some(timeout) = timeout {
            subscriptions.push(timeout_subscription(timeout));
        }

        // `poll_oneoff` needs the same number of events as subscriptions.
        let length = subscriptions.len();
        events.reserve(length);

        debug_assert!(events.capacity() >= length);
        #[cfg(debug_assertions)]
        if length == 0 {
            warn!(
                "calling mio::Poll::poll with empty subscriptions, this likely not what you want"
            );
        }

        let res = unsafe { wasi::poll_oneoff(subscriptions.as_ptr(), events.as_mut_ptr(), length) };

        // Remove the timeout subscription we possibly added above.
        if timeout.is_some() {
            let timeout_sub = subscriptions.pop();
            debug_assert_eq!(
                timeout_sub.unwrap().u.tag,
                wasi::EVENTTYPE_CLOCK.raw(),
                "failed to remove timeout subscription"
            );
        }

        drop(subscriptions); // Unlock.

        match res {
            Ok(n_events) => {
                // Safety: `poll_oneoff` initialises the `events` for us.
                unsafe { events.set_len(n_events) };

                // Remove the timeout event.
                if timeout.is_some() {
                    if let Some(index) = events.iter().position(is_timeout_event) {
                        events.swap_remove(index);
                    }
                }

                check_errors(&events)
            }
            Err(err) => Err(io_err(err)),
        }
    }

    pub(crate) fn try_clone(&self) -> io::Result<Selector> {
        Ok(Selector {
            #[cfg(all(debug_assertions, feature = "net"))]
            id: self.id,
            subscriptions: self.subscriptions.clone(),
        })
    }

    #[cfg(feature = "net")]
    pub(crate) fn register(
        &self,
        fd: wasi::Fd,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();

        if interests.is_writable() {
            let subscription = wasi::Subscription {
                userdata: token.0 as wasi::Userdata,
                u: wasi::SubscriptionU {
                    tag: wasi::EVENTTYPE_FD_WRITE.raw(),
                    u: wasi::SubscriptionUU {
                        fd_write: wasi::SubscriptionFdReadwrite {
                            file_descriptor: fd,
                        },
                    },
                },
            };
            subscriptions.push(subscription);
        }

        if interests.is_readable() {
            let subscription = wasi::Subscription {
                userdata: token.0 as wasi::Userdata,
                u: wasi::SubscriptionU {
                    tag: wasi::EVENTTYPE_FD_READ.raw(),
                    u: wasi::SubscriptionUU {
                        fd_read: wasi::SubscriptionFdReadwrite {
                            file_descriptor: fd,
                        },
                    },
                },
            };
            subscriptions.push(subscription);
        }

        Ok(())
    }

    #[cfg(feature = "net")]
    pub(crate) fn reregister(
        &self,
        fd: wasi::Fd,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.deregister(fd)
            .and_then(|()| self.register(fd, token, interests))
    }

    #[cfg(feature = "net")]
    pub(crate) fn deregister(&self, fd: wasi::Fd) -> io::Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();

        let predicate = |subscription: &wasi::Subscription| {
            // Safety: `subscription.u.tag` defines the type of the union in
            // `subscription.u.u`.
            match subscription.u.tag {
                t if t == wasi::EVENTTYPE_FD_WRITE.raw() => unsafe {
                    subscription.u.u.fd_write.file_descriptor == fd
                },
                t if t == wasi::EVENTTYPE_FD_READ.raw() => unsafe {
                    subscription.u.u.fd_read.file_descriptor == fd
                },
                _ => false,
            }
        };

        let mut ret = Err(io::ErrorKind::NotFound.into());

        while let Some(index) = subscriptions.iter().position(predicate) {
            subscriptions.swap_remove(index);
            ret = Ok(())
        }

        ret
    }
}

/// Token used to a add a timeout subscription, also used in removing it again.
const TIMEOUT_TOKEN: wasi::Userdata = wasi::Userdata::MAX;

/// Returns a `wasi::Subscription` for `timeout`.
fn timeout_subscription(timeout: Duration) -> wasi::Subscription {
    wasi::Subscription {
        userdata: TIMEOUT_TOKEN,
        u: wasi::SubscriptionU {
            tag: wasi::EVENTTYPE_CLOCK.raw(),
            u: wasi::SubscriptionUU {
                clock: wasi::SubscriptionClock {
                    id: wasi::CLOCKID_MONOTONIC,
                    // Timestamp is in nanoseconds.
                    timeout: min(wasi::Timestamp::MAX as u128, timeout.as_nanos())
                        as wasi::Timestamp,
                    // Give the implementation another millisecond to coalesce
                    // events.
                    precision: Duration::from_millis(1).as_nanos() as wasi::Timestamp,
                    // Zero means the `timeout` is considered relative to the
                    // current time.
                    flags: 0,
                },
            },
        },
    }
}

fn is_timeout_event(event: &wasi::Event) -> bool {
    event.type_ == wasi::EVENTTYPE_CLOCK && event.userdata == TIMEOUT_TOKEN
}

/// Check all events for possible errors, it returns the first error found.
fn check_errors(events: &[Event]) -> io::Result<()> {
    for event in events {
        if event.error != wasi::ERRNO_SUCCESS {
            return Err(io_err(event.error));
        }
    }
    Ok(())
}

/// Convert `wasi::Errno` into an `io::Error`.
fn io_err(errno: wasi::Errno) -> io::Error {
    // TODO: check if this is valid.
    io::Error::from_raw_os_error(errno.raw() as i32)
}

pub(crate) type Events = Vec<Event>;

pub(crate) type Event = wasi::Event;

pub(crate) mod event {
    use std::fmt;

    use crate::sys::Event;
    use crate::Token;

    pub(crate) fn token(event: &Event) -> Token {
        Token(event.userdata as usize)
    }

    pub(crate) fn is_readable(event: &Event) -> bool {
        event.type_ == wasi::EVENTTYPE_FD_READ
    }

    pub(crate) fn is_writable(event: &Event) -> bool {
        event.type_ == wasi::EVENTTYPE_FD_WRITE
    }

    pub(crate) fn is_error(_: &Event) -> bool {
        // Not supported? It could be that `wasi::Event.error` could be used for
        // this, but the docs say `error that occurred while processing the
        // subscription request`, so it's checked in `Select::select` already.
        false
    }

    pub(crate) fn is_read_closed(event: &Event) -> bool {
        event.type_ == wasi::EVENTTYPE_FD_READ
            // Safety: checked the type of the union above.
            && (event.fd_readwrite.flags & wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP) != 0
    }

    pub(crate) fn is_write_closed(event: &Event) -> bool {
        event.type_ == wasi::EVENTTYPE_FD_WRITE
            // Safety: checked the type of the union above.
            && (event.fd_readwrite.flags & wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP) != 0
    }

    pub(crate) fn is_priority(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn is_aio(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn is_lio(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn debug_details(f: &mut fmt::Formatter<'_>, event: &Event) -> fmt::Result {
        debug_detail!(
            TypeDetails(wasi::Eventtype),
            PartialEq::eq,
            wasi::EVENTTYPE_CLOCK,
            wasi::EVENTTYPE_FD_READ,
            wasi::EVENTTYPE_FD_WRITE,
        );

        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn check_flag(got: &wasi::Eventrwflags, want: &wasi::Eventrwflags) -> bool {
            (got & want) != 0
        }
        debug_detail!(
            EventrwflagsDetails(wasi::Eventrwflags),
            check_flag,
            wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP,
        );

        struct EventFdReadwriteDetails(wasi::EventFdReadwrite);

        impl fmt::Debug for EventFdReadwriteDetails {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("EventFdReadwrite")
                    .field("nbytes", &self.0.nbytes)
                    .field("flags", &EventrwflagsDetails(self.0.flags))
                    .finish()
            }
        }

        f.debug_struct("Event")
            .field("userdata", &event.userdata)
            .field("error", &event.error)
            .field("type", &TypeDetails(event.type_))
            .field("fd_readwrite", &EventFdReadwriteDetails(event.fd_readwrite))
            .finish()
    }
}

cfg_os_poll! {
    cfg_io_source! {
        pub(crate) struct IoSourceState;

        impl IoSourceState {
            pub(crate) fn new() -> IoSourceState {
                IoSourceState
            }

            pub(crate) fn do_io<T, F, R>(&self, f: F, io: &T) -> io::Result<R>
            where
                F: FnOnce(&T) -> io::Result<R>,
            {
                // We don't hold state, so we can just call the function and
                // return.
                f(io)
            }
        }
    }
}
