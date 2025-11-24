#![warn(rust_2018_idioms)]
#![cfg(all(target_os = "freebsd", feature = "net"))]

use mio_aio::{AioFsyncMode, SourceApi};
use std::{
    future::Future,
    io, mem,
    os::fd::AsFd,
    os::unix::io::{AsRawFd, RawFd},
    pin::{pin, Pin},
    task::{Context, Poll},
};
use tempfile::tempfile;
use tokio::io::bsd::{Aio, AioSource};
use tokio_test::assert_pending;

mod aio {
    use super::*;

    #[derive(Debug)]
    struct TokioSource<'fd>(mio_aio::Source<nix::sys::aio::AioFsync<'fd>>);

    impl<'fd> AioSource for TokioSource<'fd> {
        fn register(&mut self, kq: RawFd, token: usize) {
            self.0.register_raw(kq, token)
        }
        fn deregister(&mut self) {
            self.0.deregister_raw()
        }
    }

    /// A very crude implementation of an AIO-based future
    struct FsyncFut<'fd>(Aio<TokioSource<'fd>>);

    impl<'fd> FsyncFut<'fd> {
        pub fn submit(self: Pin<&mut Self>) -> io::Result<()> {
            let p = unsafe { self.map_unchecked_mut(|s| &mut s.0 .0) };
            match p.submit() {
                Ok(()) => Ok(()),
                Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
            }
        }
    }

    impl<'fd> Future for FsyncFut<'fd> {
        type Output = io::Result<()>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let poll_result = self.0.poll_ready(cx);
            match poll_result {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(_ev)) => {
                    // At this point, we could clear readiness.  But there's no
                    // point, since we're about to drop the Aio.
                    let p = unsafe { self.map_unchecked_mut(|s| &mut s.0 .0) };
                    let result = p.aio_return();
                    match result {
                        Ok(r) => Poll::Ready(Ok(r)),
                        Err(e) => Poll::Ready(Err(io::Error::from_raw_os_error(e as i32))),
                    }
                }
            }
        }
    }

    /// Low-level AIO Source
    ///
    /// An example bypassing mio_aio and Nix to demonstrate how the kevent
    /// registration actually works, under the hood.
    struct LlSource(Pin<Box<libc::aiocb>>);

    impl LlSource {
        fn fsync(mut self: Pin<&mut Self>) {
            let r = unsafe {
                let p = self.0.as_mut().get_unchecked_mut();
                libc::aio_fsync(libc::O_SYNC, p)
            };
            assert_eq!(0, r);
        }
    }

    impl AioSource for LlSource {
        fn register(&mut self, kq: RawFd, token: usize) {
            let mut sev: libc::sigevent = unsafe { mem::MaybeUninit::zeroed().assume_init() };
            sev.sigev_notify = libc::SIGEV_KEVENT;
            sev.sigev_signo = kq;
            sev.sigev_value = libc::sigval {
                sival_ptr: token as *mut libc::c_void,
            };
            self.0.aio_sigevent = sev;
        }

        fn deregister(&mut self) {
            unsafe {
                self.0.aio_sigevent = mem::zeroed();
            }
        }
    }

    struct LlFut(Aio<LlSource>);

    impl LlFut {
        pub fn fsync(self: Pin<&mut Self>) {
            let p = unsafe { self.map_unchecked_mut(|s| &mut *(s.0)) };
            p.fsync();
        }
    }

    impl Future for LlFut {
        type Output = std::io::Result<usize>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let poll_result = self.0.poll_ready(cx);
            match poll_result {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(ev)) => {
                    // Clearing readiness makes the future non-idempotent; the
                    // caller can't poll it repeatedly after it has already
                    // returned Ready.  But that's ok; most futures behave this
                    // way.
                    self.0.clear_ready(ev);
                    let r = unsafe { libc::aio_return(self.0 .0.as_mut().get_unchecked_mut()) };
                    if r >= 0 {
                        Poll::Ready(Ok(r as usize))
                    } else {
                        Poll::Ready(Err(io::Error::last_os_error()))
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn fsync() {
        let f = tempfile().unwrap();
        let fd = f.as_fd();
        let mode = AioFsyncMode::O_SYNC;
        let source = TokioSource(mio_aio::Fsync::fsync(fd, mode, 0));
        let poll_aio = Aio::new_for_aio(source).unwrap();
        let mut fut = pin!(FsyncFut(poll_aio));
        fut.as_mut().submit().unwrap();
        fut.await.unwrap();
    }

    #[tokio::test]
    async fn ll_fsync() {
        let f = tempfile().unwrap();
        let fd = f.as_raw_fd();
        let mut aiocb: libc::aiocb = unsafe { mem::MaybeUninit::zeroed().assume_init() };
        aiocb.aio_fildes = fd;
        let source = LlSource(Box::pin(aiocb));
        let mut poll_aio = Aio::new_for_aio(source).unwrap();
        let r = unsafe {
            let p = poll_aio.0.as_mut().get_unchecked_mut();
            libc::aio_fsync(libc::O_SYNC, p)
        };
        assert_eq!(0, r);
        let fut = LlFut(poll_aio);
        fut.await.unwrap();
    }

    /// A suitably crafted future type can reuse an Aio object
    #[tokio::test]
    async fn reuse() {
        let f = tempfile().unwrap();
        let fd = f.as_raw_fd();
        let mut aiocb: libc::aiocb = unsafe { mem::MaybeUninit::zeroed().assume_init() };
        aiocb.aio_fildes = fd;
        let source = LlSource(Box::pin(aiocb));
        let poll_aio = Aio::new_for_aio(source).unwrap();

        // Send the operation to the kernel the first time
        let mut fut = LlFut(poll_aio);
        {
            let mut pfut = Pin::new(&mut fut);
            pfut.as_mut().fsync();
            pfut.as_mut().await.unwrap();
        }

        // Check that readiness was cleared
        let mut ctx = Context::from_waker(futures::task::noop_waker_ref());
        assert_pending!(fut.0.poll_ready(&mut ctx));

        // and reuse the future and its Aio object
        {
            let mut pfut = Pin::new(&mut fut);
            pfut.as_mut().fsync();
            pfut.as_mut().await.unwrap();
        }
    }
}

mod lio {
    use super::*;

    /// Low-level source based on lio_listio
    ///
    /// An example demonstrating using AIO with `Interest::Lio`.  mio_aio 0.8
    /// doesn't include any bindings for lio_listio, so we've got to go
    /// low-level.
    struct LioSource<'a> {
        aiocb: Pin<&'a mut [&'a mut libc::aiocb; 1]>,
        sev: libc::sigevent,
    }

    impl<'a> LioSource<'a> {
        fn new(aiocb: Pin<&'a mut [&'a mut libc::aiocb; 1]>) -> Self {
            LioSource {
                aiocb,
                sev: unsafe { mem::zeroed() },
            }
        }

        fn submit(mut self: Pin<&mut Self>) {
            let p: *const *mut libc::aiocb =
                unsafe { self.aiocb.as_mut().get_unchecked_mut() } as *const _ as *const *mut _;
            let r = unsafe { libc::lio_listio(libc::LIO_NOWAIT, p, 1, &mut self.sev) };
            assert_eq!(r, 0);
        }
    }

    impl<'a> AioSource for LioSource<'a> {
        fn register(&mut self, kq: RawFd, token: usize) {
            let mut sev: libc::sigevent = unsafe { mem::MaybeUninit::zeroed().assume_init() };
            sev.sigev_notify = libc::SIGEV_KEVENT;
            sev.sigev_signo = kq;
            sev.sigev_value = libc::sigval {
                sival_ptr: token as *mut libc::c_void,
            };
            self.sev = sev;
        }

        fn deregister(&mut self) {
            unsafe {
                self.sev = mem::zeroed();
            }
        }
    }

    struct LioFut<'a>(Aio<LioSource<'a>>);

    impl<'a> LioFut<'a> {
        pub fn submit(self: Pin<&mut Self>) {
            let p = unsafe { self.map_unchecked_mut(|s| &mut *(s.0)) };
            p.submit();
        }
    }

    impl<'a> Future for LioFut<'a> {
        type Output = std::io::Result<usize>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let poll_result = self.0.poll_ready(cx);
            match poll_result {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(ev)) => {
                    // Clearing readiness makes the future non-idempotent; the
                    // caller can't poll it repeatedly after it has already
                    // returned Ready.  But that's ok; most futures behave this
                    // way.  Clearing readiness is especially useful for
                    // lio_listio, because sometimes some operations will be
                    // ready but not all.
                    self.0.clear_ready(ev);
                    let r = unsafe {
                        let p1 = self.get_unchecked_mut();
                        let p2: &mut [&mut libc::aiocb; 1] =
                            p1.0.aiocb.as_mut().get_unchecked_mut();
                        let p3: &mut libc::aiocb = p2[0];
                        libc::aio_return(p3)
                    };
                    if r >= 0 {
                        Poll::Ready(Ok(r as usize))
                    } else {
                        Poll::Ready(Err(io::Error::last_os_error()))
                    }
                }
            }
        }
    }

    /// An lio_listio operation with one fsync element
    #[tokio::test]
    async fn onewrite() {
        const WBUF: &[u8] = b"abcdef";
        let f = tempfile().unwrap();

        let mut aiocb: libc::aiocb = unsafe { mem::zeroed() };
        aiocb.aio_fildes = f.as_raw_fd();
        aiocb.aio_lio_opcode = libc::LIO_WRITE;
        aiocb.aio_nbytes = WBUF.len();
        aiocb.aio_buf = WBUF.as_ptr() as *mut _;
        let aiocb = pin!([&mut aiocb]);
        let source = LioSource::new(aiocb);
        let poll_aio = Aio::new_for_lio(source).unwrap();

        // Send the operation to the kernel
        let mut fut = pin!(LioFut(poll_aio));
        fut.as_mut().submit();
        fut.await.unwrap();
    }

    /// A suitably crafted future type can reuse an Aio object
    #[tokio::test]
    async fn reuse() {
        const WBUF: &[u8] = b"abcdef";
        let f = tempfile().unwrap();

        let mut aiocb: libc::aiocb = unsafe { mem::zeroed() };
        aiocb.aio_fildes = f.as_raw_fd();
        aiocb.aio_lio_opcode = libc::LIO_WRITE;
        aiocb.aio_nbytes = WBUF.len();
        aiocb.aio_buf = WBUF.as_ptr() as *mut _;
        let aiocb = pin!([&mut aiocb]);
        let source = LioSource::new(aiocb);
        let poll_aio = Aio::new_for_lio(source).unwrap();

        // Send the operation to the kernel the first time
        let mut fut = LioFut(poll_aio);
        {
            let mut pfut = Pin::new(&mut fut);
            pfut.as_mut().submit();
            pfut.as_mut().await.unwrap();
        }

        // Check that readiness was cleared
        let mut ctx = Context::from_waker(futures::task::noop_waker_ref());
        assert_pending!(fut.0.poll_ready(&mut ctx));

        // and reuse the future and its Aio object
        {
            let mut pfut = Pin::new(&mut fut);
            pfut.as_mut().submit();
            pfut.as_mut().await.unwrap();
        }
    }
}
