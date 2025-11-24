use crate::{
    io::{interest::Interest, PollEvented},
    process::{
        imp::{orphan::Wait, OrphanQueue},
        kill::Kill,
    },
    util::error::RUNTIME_SHUTTING_DOWN_ERROR,
};

use libc::{syscall, SYS_pidfd_open, ENOSYS, PIDFD_NONBLOCK};
use mio::{event::Source, unix::SourceFd};
use std::{
    fs::File,
    future::Future,
    io,
    marker::Unpin,
    ops::Deref,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    pin::Pin,
    process::ExitStatus,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    task::{Context, Poll},
};

#[derive(Debug)]
struct Pidfd {
    fd: File,
}

impl Pidfd {
    fn open(pid: u32) -> Option<Pidfd> {
        // Store false (0) to reduce executable size
        static NO_PIDFD_SUPPORT: AtomicBool = AtomicBool::new(false);

        if NO_PIDFD_SUPPORT.load(Relaxed) {
            return None;
        }

        // Safety: The following function calls invovkes syscall pidfd_open,
        // which takes two parameter: pidfd_open(fd: c_int, flag: c_int)
        let fd = unsafe { syscall(SYS_pidfd_open, pid, PIDFD_NONBLOCK) };
        if fd == -1 {
            let errno = io::Error::last_os_error().raw_os_error().unwrap();

            if errno == ENOSYS {
                NO_PIDFD_SUPPORT.store(true, Relaxed)
            }

            None
        } else {
            // Safety: pidfd_open returns -1 on error or a valid fd with ownership.
            Some(Pidfd {
                fd: unsafe { File::from_raw_fd(fd as i32) },
            })
        }
    }
}

impl AsRawFd for Pidfd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl Source for Pidfd {
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

#[derive(Debug)]
struct PidfdReaperInner<W>
where
    W: Unpin,
{
    inner: W,
    pidfd: PollEvented<Pidfd>,
}

#[allow(deprecated)]
fn is_rt_shutdown_err(err: &io::Error) -> bool {
    if let Some(inner) = err.get_ref() {
        // Using `Error::description()` is more efficient than `format!("{inner}")`,
        // so we use it here even if it is deprecated.
        err.kind() == io::ErrorKind::Other
            && inner.source().is_none()
            && inner.description() == RUNTIME_SHUTTING_DOWN_ERROR
    } else {
        false
    }
}

impl<W> Future for PidfdReaperInner<W>
where
    W: Wait + Unpin,
{
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = Pin::into_inner(self);

        match this.pidfd.registration().poll_read_ready(cx) {
            Poll::Ready(Ok(evt)) => {
                if let Some(exit_code) = this.inner.try_wait()? {
                    return Poll::Ready(Ok(exit_code));
                }
                this.pidfd.registration().clear_readiness(evt);
            }
            Poll::Ready(Err(err)) if is_rt_shutdown_err(&err) => {}
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        };

        this.pidfd.reregister(Interest::READABLE)?;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[derive(Debug)]
pub(crate) struct PidfdReaper<W, Q>
where
    W: Wait + Unpin,
    Q: OrphanQueue<W> + Unpin,
{
    inner: Option<PidfdReaperInner<W>>,
    orphan_queue: Q,
}

impl<W, Q> Deref for PidfdReaper<W, Q>
where
    W: Wait + Unpin,
    Q: OrphanQueue<W> + Unpin,
{
    type Target = W;

    fn deref(&self) -> &Self::Target {
        &self.inner.as_ref().expect("inner has gone away").inner
    }
}

impl<W, Q> PidfdReaper<W, Q>
where
    W: Wait + Unpin,
    Q: OrphanQueue<W> + Unpin,
{
    pub(crate) fn new(inner: W, orphan_queue: Q) -> Result<Self, (Option<io::Error>, W)> {
        if let Some(pidfd) = Pidfd::open(inner.id()) {
            match PollEvented::new_with_interest(pidfd, Interest::READABLE) {
                Ok(pidfd) => Ok(Self {
                    inner: Some(PidfdReaperInner { pidfd, inner }),
                    orphan_queue,
                }),
                Err(io_error) => Err((Some(io_error), inner)),
            }
        } else {
            Err((None, inner))
        }
    }

    pub(crate) fn inner_mut(&mut self) -> &mut W {
        &mut self.inner.as_mut().expect("inner has gone away").inner
    }
}

impl<W, Q> Future for PidfdReaper<W, Q>
where
    W: Wait + Unpin,
    Q: OrphanQueue<W> + Unpin,
{
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(
            Pin::into_inner(self)
                .inner
                .as_mut()
                .expect("inner has gone away"),
        )
        .poll(cx)
    }
}

impl<W, Q> Kill for PidfdReaper<W, Q>
where
    W: Wait + Unpin + Kill,
    Q: OrphanQueue<W> + Unpin,
{
    fn kill(&mut self) -> io::Result<()> {
        self.inner_mut().kill()
    }
}

impl<W, Q> Drop for PidfdReaper<W, Q>
where
    W: Wait + Unpin,
    Q: OrphanQueue<W> + Unpin,
{
    fn drop(&mut self) {
        let mut orphan = self.inner.take().expect("inner has gone away").inner;
        if let Ok(Some(_)) = orphan.try_wait() {
            return;
        }

        self.orphan_queue.push_orphan(orphan);
    }
}

#[cfg(all(test, not(loom), not(miri)))]
mod test {
    use super::*;
    use crate::{
        process::unix::orphan::test::MockQueue,
        runtime::{Builder as RuntimeBuilder, Runtime},
    };
    use std::process::{Command, Output};

    fn create_runtime() -> Runtime {
        RuntimeBuilder::new_current_thread()
            .enable_io()
            .build()
            .unwrap()
    }

    fn run_test(fut: impl Future<Output = ()>) {
        create_runtime().block_on(fut)
    }

    fn is_pidfd_available() -> bool {
        let Output { stdout, status, .. } = Command::new("uname").arg("-r").output().unwrap();
        assert!(status.success());
        let stdout = String::from_utf8_lossy(&stdout);

        let mut kernel_version_iter = match stdout.split_once('-') {
            Some((version, _)) => version,
            _ => &stdout,
        }
        .split('.');

        let major: u32 = kernel_version_iter.next().unwrap().parse().unwrap();
        let minor: u32 = kernel_version_iter.next().unwrap().parse().unwrap();

        major >= 6 || (major == 5 && minor >= 10)
    }

    #[test]
    fn test_pidfd_reaper_poll() {
        if !is_pidfd_available() {
            eprintln!("pidfd is not available on this linux kernel, skip this test");
            return;
        }

        let queue = MockQueue::new();

        run_test(async {
            let child = Command::new("true").spawn().unwrap();
            let pidfd_reaper = PidfdReaper::new(child, &queue).unwrap();

            let exit_status = pidfd_reaper.await.unwrap();
            assert!(exit_status.success());
        });

        assert!(queue.all_enqueued.borrow().is_empty());
    }

    #[test]
    fn test_pidfd_reaper_kill() {
        if !is_pidfd_available() {
            eprintln!("pidfd is not available on this linux kernel, skip this test");
            return;
        }

        let queue = MockQueue::new();

        run_test(async {
            let child = Command::new("sleep").arg("1800").spawn().unwrap();
            let mut pidfd_reaper = PidfdReaper::new(child, &queue).unwrap();

            pidfd_reaper.kill().unwrap();

            let exit_status = pidfd_reaper.await.unwrap();
            assert!(!exit_status.success());
        });

        assert!(queue.all_enqueued.borrow().is_empty());
    }

    #[test]
    fn test_pidfd_reaper_drop() {
        if !is_pidfd_available() {
            eprintln!("pidfd is not available on this linux kernel, skip this test");
            return;
        }

        let queue = MockQueue::new();

        let mut child = Command::new("sleep").arg("1800").spawn().unwrap();

        run_test(async {
            let _pidfd_reaper = PidfdReaper::new(&mut child, &queue).unwrap();
        });

        assert_eq!(queue.all_enqueued.borrow().len(), 1);

        child.kill().unwrap();
        child.wait().unwrap();
    }
}
