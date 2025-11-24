use std::io;

use crate::sys::Selector;
use crate::Token;

/// Waker backed by kqueue user space notifications (`EVFILT_USER`).
///
/// The implementation is fairly simple, first the kqueue must be setup to
/// receive waker events this done by calling `Selector.setup_waker`. Next
/// we need access to kqueue, thus we need to duplicate the file descriptor.
/// Now waking is as simple as adding an event to the kqueue.
#[derive(Debug)]
pub(crate) struct Waker {
    selector: Selector,
    token: Token,
}

impl Waker {
    pub(crate) fn new(selector: &Selector, token: Token) -> io::Result<Waker> {
        let selector = selector.try_clone()?;
        selector.setup_waker(token)?;
        Ok(Waker { selector, token })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        self.selector.wake(self.token)
    }
}
