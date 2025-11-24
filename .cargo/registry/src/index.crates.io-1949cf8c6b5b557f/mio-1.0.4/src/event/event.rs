use crate::{sys, Token};

use std::fmt;

/// A readiness event.
///
/// `Event` is a readiness state paired with a [`Token`]. It is returned by
/// [`Poll::poll`].
///
/// For more documentation on polling and events, see [`Poll`].
///
/// [`Poll::poll`]: ../struct.Poll.html#method.poll
/// [`Poll`]: ../struct.Poll.html
/// [`Token`]: ../struct.Token.html
#[derive(Clone)]
#[repr(transparent)]
pub struct Event {
    inner: sys::Event,
}

impl Event {
    /// Returns the event's token.
    pub fn token(&self) -> Token {
        sys::event::token(&self.inner)
    }

    /// Returns true if the event contains readable readiness.
    ///
    /// # Notes
    ///
    /// Out-of-band (OOB) data also triggers readable events. But most
    /// applications don't actually read OOB data, this could leave an
    /// application open to a Denial-of-Service (Dos) attack, see
    /// <https://github.com/sandstorm-io/sandstorm-website/blob/58f93346028c0576e8147627667328eaaf4be9fa/_posts/2015-04-08-osx-security-bug.md>.
    /// However because Mio uses edge-triggers it will not result in an infinite
    /// loop as described in the article above.
    pub fn is_readable(&self) -> bool {
        sys::event::is_readable(&self.inner)
    }

    /// Returns true if the event contains writable readiness.
    pub fn is_writable(&self) -> bool {
        sys::event::is_writable(&self.inner)
    }

    /// Returns true if the event contains error readiness.
    ///
    /// Error events occur when the socket enters an error state. In this case,
    /// the socket will also receive a readable or writable event. Reading or
    /// writing to the socket will result in an error.
    ///
    /// # Notes
    ///
    /// Method is available on all platforms, but not all platforms trigger the
    /// error event.
    ///
    /// The table below shows what flags are checked on what OS.
    ///
    /// | [OS selector] | Flag(s) checked |
    /// |---------------|-----------------|
    /// | [epoll]       | `EPOLLERR`      |
    /// | [kqueue]      | `EV_ERROR` and `EV_EOF` with `fflags` set to `0`. |
    ///
    /// [OS selector]: ../struct.Poll.html#implementation-notes
    /// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
    /// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
    pub fn is_error(&self) -> bool {
        sys::event::is_error(&self.inner)
    }

    /// Returns true if the event contains read closed readiness.
    ///
    /// # Notes
    ///
    /// Read closed readiness can be expected after any of the following have
    /// occurred:
    /// * The local stream has shutdown the read half of its socket
    /// * The local stream has shutdown both the read half and the write half
    ///   of its socket
    /// * The peer stream has shutdown the write half its socket; this sends a
    ///   `FIN` packet that has been received by the local stream
    ///
    /// Method is a best effort implementation. While some platforms may not
    /// return readiness when read half is closed, it is guaranteed that
    /// false-positives will not occur.
    ///
    /// The table below shows what flags are checked on what OS.
    ///
    /// | [OS selector] | Flag(s) checked |
    /// |---------------|-----------------|
    /// | [epoll]       | `EPOLLHUP`, or  |
    /// |               | `EPOLLIN` and `EPOLLRDHUP` |
    /// | [kqueue]      | `EV_EOF`        |
    ///
    /// [OS selector]: ../struct.Poll.html#implementation-notes
    /// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
    /// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
    pub fn is_read_closed(&self) -> bool {
        sys::event::is_read_closed(&self.inner)
    }

    /// Returns true if the event contains write closed readiness.
    ///
    /// # Notes
    ///
    /// On [epoll] this is essentially a check for `EPOLLHUP` flag as the
    /// local stream shutting down its write half does not trigger this event.
    ///
    /// On [kqueue] the local stream shutting down the write half of its
    /// socket will trigger this event.
    ///
    /// Method is a best effort implementation. While some platforms may not
    /// return readiness when write half is closed, it is guaranteed that
    /// false-positives will not occur.
    ///
    /// The table below shows what flags are checked on what OS.
    ///
    /// | [OS selector] | Flag(s) checked |
    /// |---------------|-----------------|
    /// | [epoll]       | `EPOLLHUP`, or  |
    /// |               | only `EPOLLERR`, or |
    /// |               | `EPOLLOUT` and `EPOLLERR` |
    /// | [kqueue]      | `EV_EOF`        |
    ///
    /// [OS selector]: ../struct.Poll.html#implementation-notes
    /// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
    /// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
    pub fn is_write_closed(&self) -> bool {
        sys::event::is_write_closed(&self.inner)
    }

    /// Returns true if the event contains priority readiness.
    ///
    /// # Notes
    ///
    /// Method is available on all platforms, but not all platforms trigger the
    /// priority event.
    ///
    /// The table below shows what flags are checked on what OS.
    ///
    /// | [OS selector] | Flag(s) checked |
    /// |---------------|-----------------|
    /// | [epoll]       | `EPOLLPRI`      |
    /// | [kqueue]      | *Not supported* |
    ///
    /// [OS selector]: ../struct.Poll.html#implementation-notes
    /// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
    /// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
    #[inline]
    pub fn is_priority(&self) -> bool {
        sys::event::is_priority(&self.inner)
    }

    /// Returns true if the event contains AIO readiness.
    ///
    /// # Notes
    ///
    /// Method is available on all platforms, but not all platforms support AIO.
    ///
    /// The table below shows what flags are checked on what OS.
    ///
    /// | [OS selector] | Flag(s) checked |
    /// |---------------|-----------------|
    /// | [epoll]       | *Not supported* |
    /// | [kqueue]<sup>1</sup> | `EVFILT_AIO` |
    ///
    /// 1: Only supported on DragonFly BSD, FreeBSD, iOS and macOS.
    ///
    /// [OS selector]: ../struct.Poll.html#implementation-notes
    /// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
    /// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
    pub fn is_aio(&self) -> bool {
        sys::event::is_aio(&self.inner)
    }

    /// Returns true if the event contains LIO readiness.
    ///
    /// # Notes
    ///
    /// Method is available on all platforms, but only FreeBSD supports LIO. On
    /// FreeBSD this method checks the `EVFILT_LIO` flag.
    pub fn is_lio(&self) -> bool {
        sys::event::is_lio(&self.inner)
    }

    /// Create a reference to an `Event` from a platform specific event.
    pub(crate) fn from_sys_event_ref(sys_event: &sys::Event) -> &Event {
        unsafe {
            // This is safe because the memory layout of `Event` is
            // the same as `sys::Event` due to the `repr(transparent)` attribute.
            &*(sys_event as *const sys::Event as *const Event)
        }
    }
}

/// When the [alternate] flag is enabled this will print platform specific
/// details, for example the fields of the `kevent` structure on platforms that
/// use `kqueue(2)`. Note however that the output of this implementation is
/// **not** consider a part of the stable API.
///
/// [alternate]: fmt::Formatter::alternate
impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alternate = f.alternate();
        let mut d = f.debug_struct("Event");
        d.field("token", &self.token())
            .field("readable", &self.is_readable())
            .field("writable", &self.is_writable())
            .field("error", &self.is_error())
            .field("read_closed", &self.is_read_closed())
            .field("write_closed", &self.is_write_closed())
            .field("priority", &self.is_priority())
            .field("aio", &self.is_aio())
            .field("lio", &self.is_lio());

        if alternate {
            struct EventDetails<'a>(&'a sys::Event);

            impl fmt::Debug for EventDetails<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    sys::event::debug_details(f, self.0)
                }
            }

            d.field("details", &EventDetails(&self.inner)).finish()
        } else {
            d.finish()
        }
    }
}
