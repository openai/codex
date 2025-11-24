use crate::errno::Errno;
pub use crate::poll_timeout::PollTimeout as EpollTimeout;
pub use crate::poll_timeout::PollTimeoutTryFromError as EpollTimeoutTryFromError;
use crate::Result;
use libc::{self, c_int};
use std::mem;
use std::os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};

libc_bitflags!(
    pub struct EpollFlags: c_int {
        EPOLLIN;
        EPOLLPRI;
        EPOLLOUT;
        EPOLLRDNORM;
        EPOLLRDBAND;
        EPOLLWRNORM;
        EPOLLWRBAND;
        EPOLLMSG;
        EPOLLERR;
        EPOLLHUP;
        EPOLLRDHUP;
        EPOLLEXCLUSIVE;
        #[cfg(not(target_arch = "mips"))]
        EPOLLWAKEUP;
        EPOLLONESHOT;
        EPOLLET;
    }
);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum EpollOp {
    EpollCtlAdd = libc::EPOLL_CTL_ADD,
    EpollCtlDel = libc::EPOLL_CTL_DEL,
    EpollCtlMod = libc::EPOLL_CTL_MOD,
}

libc_bitflags! {
    pub struct EpollCreateFlags: c_int {
        EPOLL_CLOEXEC;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct EpollEvent {
    event: libc::epoll_event,
}

impl EpollEvent {
    pub fn new(events: EpollFlags, data: u64) -> Self {
        EpollEvent {
            event: libc::epoll_event {
                events: events.bits() as u32,
                u64: data,
            },
        }
    }

    pub fn empty() -> Self {
        unsafe { mem::zeroed::<EpollEvent>() }
    }

    pub fn events(&self) -> EpollFlags {
        EpollFlags::from_bits(self.event.events as c_int).unwrap()
    }

    pub fn data(&self) -> u64 {
        self.event.u64
    }
}

/// A safe wrapper around [`epoll`](https://man7.org/linux/man-pages/man7/epoll.7.html).
/// ```
/// # use nix::sys::{epoll::{EpollTimeout, Epoll, EpollEvent, EpollFlags, EpollCreateFlags}, eventfd::{EventFd, EfdFlags}};
/// # use nix::unistd::write;
/// # use std::os::unix::io::{OwnedFd, FromRawFd, AsFd};
/// # use std::time::{Instant, Duration};
/// # fn main() -> nix::Result<()> {
/// const DATA: u64 = 17;
/// const MILLIS: u8 = 100;
///
/// // Create epoll
/// let epoll = Epoll::new(EpollCreateFlags::empty())?;
///
/// // Create eventfd & Add event
/// let eventfd = EventFd::new()?;
/// epoll.add(&eventfd, EpollEvent::new(EpollFlags::EPOLLIN,DATA))?;
///
/// // Arm eventfd & Time wait
/// eventfd.write(1)?;
/// let now = Instant::now();
///
/// // Wait on event
/// let mut events = [EpollEvent::empty()];
/// epoll.wait(&mut events, MILLIS)?;
///
/// // Assert data correct & timeout didn't occur
/// assert_eq!(events[0].data(), DATA);
/// assert!(now.elapsed().as_millis() < MILLIS.into());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Epoll(pub OwnedFd);
impl Epoll {
    /// Creates a new epoll instance and returns a file descriptor referring to that instance.
    ///
    /// [`epoll_create1`](https://man7.org/linux/man-pages/man2/epoll_create1.2.html).
    pub fn new(flags: EpollCreateFlags) -> Result<Self> {
        let res = unsafe { libc::epoll_create1(flags.bits()) };
        let fd = Errno::result(res)?;
        let owned_fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(Self(owned_fd))
    }
    /// Add an entry to the interest list of the epoll file descriptor for
    /// specified in events.
    ///
    /// [`epoll_ctl`](https://man7.org/linux/man-pages/man2/epoll_ctl.2.html) with `EPOLL_CTL_ADD`.
    pub fn add<Fd: AsFd>(&self, fd: Fd, mut event: EpollEvent) -> Result<()> {
        self.epoll_ctl(EpollOp::EpollCtlAdd, fd, &mut event)
    }
    /// Remove (deregister) the target file descriptor `fd` from the interest list.
    ///
    /// [`epoll_ctl`](https://man7.org/linux/man-pages/man2/epoll_ctl.2.html) with `EPOLL_CTL_DEL` .
    pub fn delete<Fd: AsFd>(&self, fd: Fd) -> Result<()> {
        self.epoll_ctl(EpollOp::EpollCtlDel, fd, None)
    }
    /// Change the settings associated with `fd` in the interest list to the new settings specified
    /// in `event`.
    ///
    /// [`epoll_ctl`](https://man7.org/linux/man-pages/man2/epoll_ctl.2.html) with `EPOLL_CTL_MOD`.
    pub fn modify<Fd: AsFd>(
        &self,
        fd: Fd,
        event: &mut EpollEvent,
    ) -> Result<()> {
        self.epoll_ctl(EpollOp::EpollCtlMod, fd, event)
    }
    /// Waits for I/O events, blocking the calling thread if no events are currently available.
    /// (This can be thought of as fetching items from the ready list of the epoll instance.)
    ///
    /// [`epoll_wait`](https://man7.org/linux/man-pages/man2/epoll_wait.2.html)
    pub fn wait<T: Into<EpollTimeout>>(
        &self,
        events: &mut [EpollEvent],
        timeout: T,
    ) -> Result<usize> {
        let res = unsafe {
            libc::epoll_wait(
                self.0.as_raw_fd(),
                events.as_mut_ptr().cast(),
                events.len() as c_int,
                timeout.into().into(),
            )
        };

        Errno::result(res).map(|r| r as usize)
    }
    /// This system call is used to add, modify, or remove entries in the interest list of the epoll
    /// instance referred to by `self`. It requests that the operation `op` be performed for the
    /// target file descriptor, `fd`.
    ///
    /// When possible prefer [`Epoll::add`], [`Epoll::delete`] and [`Epoll::modify`].
    ///
    /// [`epoll_ctl`](https://man7.org/linux/man-pages/man2/epoll_ctl.2.html)
    fn epoll_ctl<'a, Fd: AsFd, T>(
        &self,
        op: EpollOp,
        fd: Fd,
        event: T,
    ) -> Result<()>
    where
        T: Into<Option<&'a mut EpollEvent>>,
    {
        let event: Option<&mut EpollEvent> = event.into();
        let ptr = event
            .map(|x| &mut x.event as *mut libc::epoll_event)
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            Errno::result(libc::epoll_ctl(
                self.0.as_raw_fd(),
                op as c_int,
                fd.as_fd().as_raw_fd(),
                ptr,
            ))
            .map(drop)
        }
    }
}

#[deprecated(since = "0.27.0", note = "Use Epoll::new() instead")]
#[inline]
pub fn epoll_create() -> Result<RawFd> {
    let res = unsafe { libc::epoll_create(1024) };

    Errno::result(res)
}

#[deprecated(since = "0.27.0", note = "Use Epoll::new() instead")]
#[inline]
pub fn epoll_create1(flags: EpollCreateFlags) -> Result<RawFd> {
    let res = unsafe { libc::epoll_create1(flags.bits()) };

    Errno::result(res)
}

#[deprecated(
    since = "0.27.0",
    note = "Use corresponding Epoll methods instead"
)]
#[inline]
pub fn epoll_ctl<'a, T>(
    epfd: RawFd,
    op: EpollOp,
    fd: RawFd,
    event: T,
) -> Result<()>
where
    T: Into<Option<&'a mut EpollEvent>>,
{
    let mut event: Option<&mut EpollEvent> = event.into();
    if event.is_none() && op != EpollOp::EpollCtlDel {
        Err(Errno::EINVAL)
    } else {
        let res = unsafe {
            if let Some(ref mut event) = event {
                libc::epoll_ctl(epfd, op as c_int, fd, &mut event.event)
            } else {
                libc::epoll_ctl(epfd, op as c_int, fd, std::ptr::null_mut())
            }
        };
        Errno::result(res).map(drop)
    }
}

#[deprecated(since = "0.27.0", note = "Use Epoll::wait() instead")]
#[inline]
pub fn epoll_wait(
    epfd: RawFd,
    events: &mut [EpollEvent],
    timeout_ms: isize,
) -> Result<usize> {
    let res = unsafe {
        libc::epoll_wait(
            epfd,
            events.as_mut_ptr().cast(),
            events.len() as c_int,
            timeout_ms as c_int,
        )
    };

    Errno::result(res).map(|r| r as usize)
}
