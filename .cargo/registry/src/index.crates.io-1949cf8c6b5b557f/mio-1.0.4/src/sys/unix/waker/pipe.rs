use std::fs::File;
use std::io::{self, Read, Write};
#[cfg(not(target_os = "hermit"))]
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::{AsRawFd, FromRawFd, RawFd};

use crate::sys::unix::pipe;
use crate::sys::Selector;
use crate::{Interest, Token};

/// Waker backed by a unix pipe.
///
/// Waker controls both the sending and receiving ends and empties the pipe
/// if writing to it (waking) fails.
#[derive(Debug)]
pub(crate) struct Waker {
    sender: File,
    receiver: File,
}

impl Waker {
    #[allow(dead_code)] // Not used by the `poll(2)` implementation.
    pub(crate) fn new(selector: &Selector, token: Token) -> io::Result<Waker> {
        let waker = Waker::new_unregistered()?;
        selector.register(waker.receiver.as_raw_fd(), token, Interest::READABLE)?;
        Ok(waker)
    }

    pub(crate) fn new_unregistered() -> io::Result<Waker> {
        let [receiver, sender] = pipe::new_raw()?;
        let sender = unsafe { File::from_raw_fd(sender) };
        let receiver = unsafe { File::from_raw_fd(receiver) };
        Ok(Waker { sender, receiver })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        // The epoll emulation on some illumos systems currently requires
        // the pipe buffer to be completely empty for an edge-triggered
        // wakeup on the pipe read side.
        // See https://www.illumos.org/issues/13436.
        #[cfg(target_os = "illumos")]
        self.empty();

        match (&self.sender).write(&[1]) {
            Ok(_) => Ok(()),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                // The reading end is full so we'll empty the buffer and try
                // again.
                self.empty();
                self.wake()
            }
            Err(ref err) if err.kind() == io::ErrorKind::Interrupted => self.wake(),
            Err(err) => Err(err),
        }
    }

    #[allow(dead_code)] // Only used by the `poll(2)` implementation.
    pub(crate) fn ack_and_reset(&self) {
        self.empty();
    }

    /// Empty the pipe's buffer, only need to call this if `wake` fails.
    /// This ignores any errors.
    fn empty(&self) {
        let mut buf = [0; 4096];
        loop {
            match (&self.receiver).read(&mut buf) {
                Ok(n) if n > 0 => continue,
                _ => return,
            }
        }
    }
}

impl AsRawFd for Waker {
    fn as_raw_fd(&self) -> RawFd {
        self.receiver.as_raw_fd()
    }
}
