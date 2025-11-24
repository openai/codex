//! Both `kqueue(2)` and `epoll(2)` don't need to hold any user space state.

use std::io;
use std::os::fd::RawFd;

use crate::{Interest, Registry, Token};

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

    pub(crate) fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
        fd: RawFd,
    ) -> io::Result<()> {
        // Pass through, we don't have any state.
        registry.selector().register(fd, token, interests)
    }

    pub(crate) fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
        fd: RawFd,
    ) -> io::Result<()> {
        // Pass through, we don't have any state.
        registry.selector().reregister(fd, token, interests)
    }

    pub(crate) fn deregister(&mut self, registry: &Registry, fd: RawFd) -> io::Result<()> {
        // Pass through, we don't have any state.
        registry.selector().deregister(fd)
    }
}
