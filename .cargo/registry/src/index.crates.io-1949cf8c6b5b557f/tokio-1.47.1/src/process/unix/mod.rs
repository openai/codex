//! Unix handling of child processes.
//!
//! Right now the only "fancy" thing about this is how we implement the
//! `Future` implementation on `Child` to get the exit status. Unix offers
//! no way to register a child with epoll, and the only real way to get a
//! notification when a process exits is the SIGCHLD signal.
//!
//! Signal handling in general is *super* hairy and complicated, and it's even
//! more complicated here with the fact that signals are coalesced, so we may
//! not get a SIGCHLD-per-child.
//!
//! Our best approximation here is to check *all spawned processes* for all
//! SIGCHLD signals received. To do that we create a `Signal`, implemented in
//! the `tokio-net` crate, which is a stream over signals being received.
//!
//! Later when we poll the process's exit status we simply check to see if a
//! SIGCHLD has happened since we last checked, and while that returns "yes" we
//! keep trying.
//!
//! Note that this means that this isn't really scalable, but then again
//! processes in general aren't scalable (e.g. millions) so it shouldn't be that
//! bad in theory...

pub(crate) mod orphan;
use orphan::{OrphanQueue, OrphanQueueImpl, Wait};

mod reap;
use reap::Reaper;

#[cfg(all(target_os = "linux", feature = "rt"))]
mod pidfd_reaper;

use crate::io::{AsyncRead, AsyncWrite, PollEvented, ReadBuf};
use crate::process::kill::Kill;
use crate::process::SpawnedChild;
use crate::runtime::signal::Handle as SignalHandle;
use crate::signal::unix::{signal, Signal, SignalKind};

use mio::event::Source;
use mio::unix::SourceFd;
use std::fmt;
use std::fs::File;
use std::future::Future;
use std::io;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::pin::Pin;
use std::process::{Child as StdChild, ExitStatus, Stdio};
use std::task::Context;
use std::task::Poll;

impl Wait for StdChild {
    fn id(&self) -> u32 {
        self.id()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.try_wait()
    }
}

impl Kill for StdChild {
    fn kill(&mut self) -> io::Result<()> {
        self.kill()
    }
}

cfg_not_has_const_mutex_new! {
    fn get_orphan_queue() -> &'static OrphanQueueImpl<StdChild> {
        use std::sync::OnceLock;

        static ORPHAN_QUEUE: OnceLock<OrphanQueueImpl<StdChild>> = OnceLock::new();

        ORPHAN_QUEUE.get_or_init(OrphanQueueImpl::new)
    }
}

cfg_has_const_mutex_new! {
    fn get_orphan_queue() -> &'static OrphanQueueImpl<StdChild> {
        static ORPHAN_QUEUE: OrphanQueueImpl<StdChild> = OrphanQueueImpl::new();

        &ORPHAN_QUEUE
    }
}

pub(crate) struct GlobalOrphanQueue;

impl fmt::Debug for GlobalOrphanQueue {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        get_orphan_queue().fmt(fmt)
    }
}

impl GlobalOrphanQueue {
    pub(crate) fn reap_orphans(handle: &SignalHandle) {
        get_orphan_queue().reap_orphans(handle);
    }
}

impl OrphanQueue<StdChild> for GlobalOrphanQueue {
    fn push_orphan(&self, orphan: StdChild) {
        get_orphan_queue().push_orphan(orphan);
    }
}

#[must_use = "futures do nothing unless polled"]
pub(crate) enum Child {
    SignalReaper(Reaper<StdChild, GlobalOrphanQueue, Signal>),
    #[cfg(all(target_os = "linux", feature = "rt"))]
    PidfdReaper(pidfd_reaper::PidfdReaper<StdChild, GlobalOrphanQueue>),
}

impl fmt::Debug for Child {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Child").field("pid", &self.id()).finish()
    }
}

pub(crate) fn build_child(mut child: StdChild) -> io::Result<SpawnedChild> {
    let stdin = child.stdin.take().map(stdio).transpose()?;
    let stdout = child.stdout.take().map(stdio).transpose()?;
    let stderr = child.stderr.take().map(stdio).transpose()?;

    #[cfg(all(target_os = "linux", feature = "rt"))]
    match pidfd_reaper::PidfdReaper::new(child, GlobalOrphanQueue) {
        Ok(pidfd_reaper) => {
            return Ok(SpawnedChild {
                child: Child::PidfdReaper(pidfd_reaper),
                stdin,
                stdout,
                stderr,
            })
        }
        Err((Some(err), _child)) => return Err(err),
        Err((None, child_returned)) => child = child_returned,
    }

    let signal = signal(SignalKind::child())?;

    Ok(SpawnedChild {
        child: Child::SignalReaper(Reaper::new(child, GlobalOrphanQueue, signal)),
        stdin,
        stdout,
        stderr,
    })
}

impl Child {
    pub(crate) fn id(&self) -> u32 {
        match self {
            Self::SignalReaper(signal_reaper) => signal_reaper.id(),
            #[cfg(all(target_os = "linux", feature = "rt"))]
            Self::PidfdReaper(pidfd_reaper) => pidfd_reaper.id(),
        }
    }

    fn std_child(&mut self) -> &mut StdChild {
        match self {
            Self::SignalReaper(signal_reaper) => signal_reaper.inner_mut(),
            #[cfg(all(target_os = "linux", feature = "rt"))]
            Self::PidfdReaper(pidfd_reaper) => pidfd_reaper.inner_mut(),
        }
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.std_child().try_wait()
    }
}

impl Kill for Child {
    fn kill(&mut self) -> io::Result<()> {
        self.std_child().kill()
    }
}

impl Future for Child {
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::into_inner(self) {
            Self::SignalReaper(signal_reaper) => Pin::new(signal_reaper).poll(cx),
            #[cfg(all(target_os = "linux", feature = "rt"))]
            Self::PidfdReaper(pidfd_reaper) => Pin::new(pidfd_reaper).poll(cx),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Pipe {
    // Actually a pipe is not a File. However, we are reusing `File` to get
    // close on drop. This is a similar trick as `mio`.
    fd: File,
}

impl<T: IntoRawFd> From<T> for Pipe {
    fn from(fd: T) -> Self {
        let fd = unsafe { File::from_raw_fd(fd.into_raw_fd()) };
        Self { fd }
    }
}

impl io::Read for &Pipe {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        (&self.fd).read(bytes)
    }
}

impl io::Write for &Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&self.fd).write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.fd).flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        (&self.fd).write_vectored(bufs)
    }
}

impl AsRawFd for Pipe {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl AsFd for Pipe {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

fn convert_to_blocking_file(io: ChildStdio) -> io::Result<File> {
    let mut fd = io.inner.into_inner()?.fd;

    // Ensure that the fd to be inherited is set to *blocking* mode, as this
    // is the default that virtually all programs expect to have. Those
    // programs that know how to work with nonblocking stdio will know how to
    // change it to nonblocking mode.
    set_nonblocking(&mut fd, false)?;

    Ok(fd)
}

pub(crate) fn convert_to_stdio(io: ChildStdio) -> io::Result<Stdio> {
    convert_to_blocking_file(io).map(Stdio::from)
}

impl Source for Pipe {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}

pub(crate) struct ChildStdio {
    inner: PollEvented<Pipe>,
}

impl ChildStdio {
    pub(super) fn into_owned_fd(self) -> io::Result<OwnedFd> {
        convert_to_blocking_file(self).map(OwnedFd::from)
    }
}

impl fmt::Debug for ChildStdio {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(fmt)
    }
}

impl AsRawFd for ChildStdio {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl AsFd for ChildStdio {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl AsyncWrite for ChildStdio {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        self.inner.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        true
    }
}

impl AsyncRead for ChildStdio {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Safety: pipes support reading into uninitialized memory
        unsafe { self.inner.poll_read(cx, buf) }
    }
}

fn set_nonblocking<T: AsRawFd>(fd: &mut T, nonblocking: bool) -> io::Result<()> {
    unsafe {
        let fd = fd.as_raw_fd();
        let previous = libc::fcntl(fd, libc::F_GETFL);
        if previous == -1 {
            return Err(io::Error::last_os_error());
        }

        let new = if nonblocking {
            previous | libc::O_NONBLOCK
        } else {
            previous & !libc::O_NONBLOCK
        };

        let r = libc::fcntl(fd, libc::F_SETFL, new);
        if r == -1 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

pub(super) fn stdio<T>(io: T) -> io::Result<ChildStdio>
where
    T: IntoRawFd,
{
    // Set the fd to nonblocking before we pass it to the event loop
    let mut pipe = Pipe::from(io);
    set_nonblocking(&mut pipe, true)?;

    PollEvented::new(pipe).map(|inner| ChildStdio { inner })
}
