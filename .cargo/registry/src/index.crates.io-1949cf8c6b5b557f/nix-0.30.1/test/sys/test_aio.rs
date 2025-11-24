use std::{
    io::{Read, Seek, Write},
    ops::Deref,
    os::unix::io::{AsFd, AsRawFd, BorrowedFd},
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    thread, time,
};

use libc::c_int;
use nix::{
    errno::*,
    sys::{
        aio::*,
        signal::{
            sigaction, SaFlags, SigAction, SigHandler, SigSet, SigevNotify,
            Signal,
        },
        time::{TimeSpec, TimeValLike},
    },
};
use tempfile::tempfile;

pub static SIGNALED: AtomicBool = AtomicBool::new(false);

extern "C" fn sigfunc(_: c_int) {
    SIGNALED.store(true, Ordering::Relaxed);
}

// Helper that polls an AioCb for completion or error
macro_rules! poll_aio {
    ($aiocb: expr) => {
        loop {
            let err = $aiocb.as_mut().error();
            if err != Err(Errno::EINPROGRESS) {
                break err;
            };
            thread::sleep(time::Duration::from_millis(10));
        }
    };
}

mod aio_fsync {
    use super::*;

    #[test]
    fn test_accessors() {
        let f = tempfile().unwrap();
        let aiocb = AioFsync::new(
            f.as_fd(),
            AioFsyncMode::O_SYNC,
            42,
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 99,
            },
        );
        assert_eq!(f.as_raw_fd(), aiocb.fd().as_raw_fd());
        assert_eq!(AioFsyncMode::O_SYNC, aiocb.mode());
        assert_eq!(42, aiocb.priority());
        let sev = aiocb.sigevent().sigevent();
        assert_eq!(Signal::SIGUSR2 as i32, sev.sigev_signo);
        assert_eq!(99, sev.sigev_value.sival_ptr as i64);
    }

    /// `AioFsync::submit` should not modify the `AioCb` object if
    /// `libc::aio_fsync` returns an error
    // Skip on Linux, because Linux's AIO implementation can't detect errors
    // synchronously
    #[test]
    #[cfg_attr(any(target_os = "android", target_os = "linux"), ignore)]
    fn error() {
        use std::mem;

        const INITIAL: &[u8] = b"abcdef123456";
        // Create an invalid AioFsyncMode
        let mode = unsafe { mem::transmute::<i32, AioFsyncMode>(666) };
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        let mut aiof =
            Box::pin(AioFsync::new(f.as_fd(), mode, 0, SigevNotify::SigevNone));
        let err = aiof.as_mut().submit();
        err.expect_err("assertion failed");
    }

    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn ok() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        let mut aiof = Box::pin(AioFsync::new(
            f.as_fd(),
            AioFsyncMode::O_SYNC,
            0,
            SigevNotify::SigevNone,
        ));
        aiof.as_mut().submit().unwrap();
        poll_aio!(&mut aiof).unwrap();
        aiof.as_mut().aio_return().unwrap();
    }
}

mod aio_read {
    use super::*;

    #[test]
    fn test_accessors() {
        let f = tempfile().unwrap();
        let mut rbuf = vec![0; 4];
        let aiocb = AioRead::new(
            f.as_fd(),
            2, //offset
            &mut rbuf,
            42, //priority
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 99,
            },
        );
        assert_eq!(f.as_raw_fd(), aiocb.fd().as_raw_fd());
        assert_eq!(4, aiocb.nbytes());
        assert_eq!(2, aiocb.offset());
        assert_eq!(42, aiocb.priority());
        let sev = aiocb.sigevent().sigevent();
        assert_eq!(Signal::SIGUSR2 as i32, sev.sigev_signo);
        assert_eq!(99, sev.sigev_value.sival_ptr as i64);
    }

    // Tests AioWrite.cancel.  We aren't trying to test the OS's implementation,
    // only our bindings.  So it's sufficient to check that cancel
    // returned any AioCancelStat value.
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn cancel() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut rbuf = vec![0; 4];
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        let fd = f.as_fd();
        let mut aior =
            Box::pin(AioRead::new(fd, 2, &mut rbuf, 0, SigevNotify::SigevNone));
        aior.as_mut().submit().unwrap();

        aior.as_mut().cancel().unwrap();

        // Wait for aiow to complete, but don't care whether it succeeded
        let _ = poll_aio!(&mut aior);
        let _ = aior.as_mut().aio_return();
    }

    /// `AioRead::submit` should not modify the `AioCb` object if
    /// `libc::aio_read` returns an error
    // Skip on Linux, because Linux's AIO implementation can't detect errors
    // synchronously
    #[test]
    #[cfg(any(target_os = "freebsd", apple_targets))]
    fn error() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut rbuf = vec![0; 4];
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        let mut aior = Box::pin(AioRead::new(
            f.as_fd(),
            -1, //an invalid offset
            &mut rbuf,
            0, //priority
            SigevNotify::SigevNone,
        ));
        aior.as_mut().submit().expect_err("assertion failed");
    }

    // Test a simple aio operation with no completion notification.  We must
    // poll for completion
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn ok() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut rbuf = vec![0; 4];
        const EXPECT: &[u8] = b"cdef";
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let fd = f.as_fd();
            let mut aior = Box::pin(AioRead::new(
                fd,
                2,
                &mut rbuf,
                0,
                SigevNotify::SigevNone,
            ));
            aior.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aior);
            assert_eq!(err, Ok(()));
            assert_eq!(aior.as_mut().aio_return().unwrap(), EXPECT.len());
        }
        assert_eq!(EXPECT, rbuf.deref());
    }

    // Like ok, but allocates the structure on the stack.
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn on_stack() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut rbuf = vec![0; 4];
        const EXPECT: &[u8] = b"cdef";
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let fd = f.as_fd();
            let mut aior =
                AioRead::new(fd, 2, &mut rbuf, 0, SigevNotify::SigevNone);
            let mut aior = unsafe { Pin::new_unchecked(&mut aior) };
            aior.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aior);
            assert_eq!(err, Ok(()));
            assert_eq!(aior.as_mut().aio_return().unwrap(), EXPECT.len());
        }
        assert_eq!(EXPECT, rbuf.deref());
    }
}

#[cfg(target_os = "freebsd")]
#[cfg(fbsd14)]
mod aio_readv {
    use std::io::IoSliceMut;

    use super::*;

    #[test]
    fn test_accessors() {
        let f = tempfile().unwrap();
        let mut rbuf0 = vec![0; 4];
        let mut rbuf1 = vec![0; 8];
        let mut rbufs =
            [IoSliceMut::new(&mut rbuf0), IoSliceMut::new(&mut rbuf1)];
        let aiocb = AioReadv::new(
            f.as_fd(),
            2, //offset
            &mut rbufs,
            42, //priority
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 99,
            },
        );
        assert_eq!(f.as_raw_fd(), aiocb.fd().as_raw_fd());
        assert_eq!(2, aiocb.iovlen());
        assert_eq!(2, aiocb.offset());
        assert_eq!(42, aiocb.priority());
        let sev = aiocb.sigevent().sigevent();
        assert_eq!(Signal::SIGUSR2 as i32, sev.sigev_signo);
        assert_eq!(99, sev.sigev_value.sival_ptr as i64);
    }

    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn ok() {
        const INITIAL: &[u8] = b"abcdef123456";
        let mut rbuf0 = vec![0; 4];
        let mut rbuf1 = vec![0; 2];
        let mut rbufs =
            [IoSliceMut::new(&mut rbuf0), IoSliceMut::new(&mut rbuf1)];
        const EXPECT0: &[u8] = b"cdef";
        const EXPECT1: &[u8] = b"12";
        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let fd = f.as_fd();
            let mut aior = Box::pin(AioReadv::new(
                fd,
                2,
                &mut rbufs,
                0,
                SigevNotify::SigevNone,
            ));
            aior.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aior);
            assert_eq!(err, Ok(()));
            assert_eq!(
                aior.as_mut().aio_return().unwrap(),
                EXPECT0.len() + EXPECT1.len()
            );
        }
        assert_eq!(&EXPECT0, &rbuf0);
        assert_eq!(&EXPECT1, &rbuf1);
    }
}

mod aio_write {
    use super::*;

    #[test]
    fn test_accessors() {
        let f = tempfile().unwrap();
        let wbuf = vec![0; 4];
        let aiocb = AioWrite::new(
            f.as_fd(),
            2, //offset
            &wbuf,
            42, //priority
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 99,
            },
        );
        assert_eq!(f.as_raw_fd(), aiocb.fd().as_raw_fd());
        assert_eq!(4, aiocb.nbytes());
        assert_eq!(2, aiocb.offset());
        assert_eq!(42, aiocb.priority());
        let sev = aiocb.sigevent().sigevent();
        assert_eq!(Signal::SIGUSR2 as i32, sev.sigev_signo);
        assert_eq!(99, sev.sigev_value.sival_ptr as i64);
    }

    // Tests AioWrite.cancel.  We aren't trying to test the OS's implementation,
    // only our bindings.  So it's sufficient to check that cancel
    // returned any AioCancelStat value.
    #[test]
    #[cfg_attr(target_env = "musl", ignore)]
    fn cancel() {
        let wbuf: &[u8] = b"CDEF";

        let f = tempfile().unwrap();
        let mut aiow = Box::pin(AioWrite::new(
            f.as_fd(),
            0,
            wbuf,
            0,
            SigevNotify::SigevNone,
        ));
        aiow.as_mut().submit().unwrap();
        let err = aiow.as_mut().error();
        assert!(err == Ok(()) || err == Err(Errno::EINPROGRESS));

        aiow.as_mut().cancel().unwrap();

        // Wait for aiow to complete, but don't care whether it succeeded
        let _ = poll_aio!(&mut aiow);
        let _ = aiow.as_mut().aio_return();
    }

    // Test a simple aio operation with no completion notification.  We must
    // poll for completion.
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn ok() {
        const INITIAL: &[u8] = b"abcdef123456";
        let wbuf = "CDEF".to_string().into_bytes();
        let mut rbuf = Vec::new();
        const EXPECT: &[u8] = b"abCDEF123456";

        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let mut aiow = Box::pin(AioWrite::new(
                f.as_fd(),
                2,
                &wbuf,
                0,
                SigevNotify::SigevNone,
            ));
            aiow.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aiow);
            assert_eq!(err, Ok(()));
            assert_eq!(aiow.as_mut().aio_return().unwrap(), wbuf.len());
        }

        f.rewind().unwrap();
        let len = f.read_to_end(&mut rbuf).unwrap();
        assert_eq!(len, EXPECT.len());
        assert_eq!(rbuf, EXPECT);
    }

    // Like ok, but allocates the structure on the stack.
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn on_stack() {
        const INITIAL: &[u8] = b"abcdef123456";
        let wbuf = "CDEF".to_string().into_bytes();
        let mut rbuf = Vec::new();
        const EXPECT: &[u8] = b"abCDEF123456";

        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let mut aiow = AioWrite::new(
                f.as_fd(),
                2, //offset
                &wbuf,
                0, //priority
                SigevNotify::SigevNone,
            );
            let mut aiow = unsafe { Pin::new_unchecked(&mut aiow) };
            aiow.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aiow);
            assert_eq!(err, Ok(()));
            assert_eq!(aiow.as_mut().aio_return().unwrap(), wbuf.len());
        }

        f.rewind().unwrap();
        let len = f.read_to_end(&mut rbuf).unwrap();
        assert_eq!(len, EXPECT.len());
        assert_eq!(rbuf, EXPECT);
    }

    /// `AioWrite::write` should not modify the `AioCb` object if
    /// `libc::aio_write` returns an error.
    // Skip on Linux, because Linux's AIO implementation can't detect errors
    // synchronously
    #[test]
    #[cfg_attr(any(target_os = "android", target_os = "linux"), ignore)]
    fn error() {
        // Not I/O safe!  Deliberately create an invalid fd.
        let fd = unsafe { BorrowedFd::borrow_raw(666) };
        let wbuf = "CDEF".to_string().into_bytes();
        let mut aiow = Box::pin(AioWrite::new(
            fd,
            0, //offset
            &wbuf,
            0, //priority
            SigevNotify::SigevNone,
        ));
        aiow.as_mut().submit().expect_err("assertion failed");
        // Dropping the AioWrite at this point should not panic
    }
}

#[cfg(target_os = "freebsd")]
#[cfg(fbsd14)]
mod aio_writev {
    use std::io::IoSlice;

    use super::*;

    #[test]
    fn test_accessors() {
        let f = tempfile().unwrap();
        let wbuf0 = vec![0; 4];
        let wbuf1 = vec![0; 8];
        let wbufs = [IoSlice::new(&wbuf0), IoSlice::new(&wbuf1)];
        let aiocb = AioWritev::new(
            f.as_fd(),
            2, //offset
            &wbufs,
            42, //priority
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 99,
            },
        );
        assert_eq!(f.as_raw_fd(), aiocb.fd().as_raw_fd());
        assert_eq!(2, aiocb.iovlen());
        assert_eq!(2, aiocb.offset());
        assert_eq!(42, aiocb.priority());
        let sev = aiocb.sigevent().sigevent();
        assert_eq!(Signal::SIGUSR2 as i32, sev.sigev_signo);
        assert_eq!(99, sev.sigev_value.sival_ptr as i64);
    }

    // Test a simple aio operation with no completion notification.  We must
    // poll for completion.
    #[test]
    #[cfg_attr(all(target_env = "musl", target_arch = "x86_64"), ignore)]
    fn ok() {
        const INITIAL: &[u8] = b"abcdef123456";
        let wbuf0 = b"BC";
        let wbuf1 = b"DEF";
        let wbufs = [IoSlice::new(wbuf0), IoSlice::new(wbuf1)];
        let wlen = wbuf0.len() + wbuf1.len();
        let mut rbuf = Vec::new();
        const EXPECT: &[u8] = b"aBCDEF123456";

        let mut f = tempfile().unwrap();
        f.write_all(INITIAL).unwrap();
        {
            let mut aiow = Box::pin(AioWritev::new(
                f.as_fd(),
                1,
                &wbufs,
                0,
                SigevNotify::SigevNone,
            ));
            aiow.as_mut().submit().unwrap();

            let err = poll_aio!(&mut aiow);
            assert_eq!(err, Ok(()));
            assert_eq!(aiow.as_mut().aio_return().unwrap(), wlen);
        }

        f.rewind().unwrap();
        let len = f.read_to_end(&mut rbuf).unwrap();
        assert_eq!(len, EXPECT.len());
        assert_eq!(rbuf, EXPECT);
    }
}

// Test an aio operation with completion delivered by a signal
#[test]
#[cfg_attr(
    any(
        all(target_env = "musl", target_arch = "x86_64"),
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    ),
    ignore
)]
fn sigev_signal() {
    let _m = crate::SIGNAL_MTX.lock();
    let sa = SigAction::new(
        SigHandler::Handler(sigfunc),
        SaFlags::SA_RESETHAND,
        SigSet::empty(),
    );
    SIGNALED.store(false, Ordering::Relaxed);
    unsafe { sigaction(Signal::SIGUSR2, &sa) }.unwrap();

    const INITIAL: &[u8] = b"abcdef123456";
    const WBUF: &[u8] = b"CDEF";
    let mut rbuf = Vec::new();
    const EXPECT: &[u8] = b"abCDEF123456";

    let mut f = tempfile().unwrap();
    f.write_all(INITIAL).unwrap();
    {
        let mut aiow = Box::pin(AioWrite::new(
            f.as_fd(),
            2, //offset
            WBUF,
            0, //priority
            SigevNotify::SigevSignal {
                signal: Signal::SIGUSR2,
                si_value: 0, //TODO: validate in sigfunc
            },
        ));
        aiow.as_mut().submit().unwrap();
        while !SIGNALED.load(Ordering::Relaxed) {
            thread::sleep(time::Duration::from_millis(10));
        }

        assert_eq!(aiow.as_mut().aio_return().unwrap(), WBUF.len());
    }

    f.rewind().unwrap();
    let len = f.read_to_end(&mut rbuf).unwrap();
    assert_eq!(len, EXPECT.len());
    assert_eq!(rbuf, EXPECT);
}

// Tests using aio_cancel_all for all outstanding IOs.
#[test]
#[cfg_attr(target_env = "musl", ignore)]
fn test_aio_cancel_all() {
    let wbuf: &[u8] = b"CDEF";

    let f = tempfile().unwrap();
    let mut aiocb = Box::pin(AioWrite::new(
        f.as_fd(),
        0, //offset
        wbuf,
        0, //priority
        SigevNotify::SigevNone,
    ));
    aiocb.as_mut().submit().unwrap();
    let err = aiocb.as_mut().error();
    assert!(err == Ok(()) || err == Err(Errno::EINPROGRESS));

    aio_cancel_all(f.as_fd()).unwrap();

    // Wait for aiocb to complete, but don't care whether it succeeded
    let _ = poll_aio!(&mut aiocb);
    let _ = aiocb.as_mut().aio_return();
}

#[test]
fn test_aio_suspend() {
    const INITIAL: &[u8] = b"abcdef123456";
    const WBUF: &[u8] = b"CDEFG";
    let timeout = TimeSpec::seconds(10);
    let mut rbuf = vec![0; 4];
    let rlen = rbuf.len();
    let mut f = tempfile().unwrap();
    f.write_all(INITIAL).unwrap();

    let mut wcb = Box::pin(AioWrite::new(
        f.as_fd(),
        2, //offset
        WBUF,
        0, //priority
        SigevNotify::SigevNone,
    ));

    let mut rcb = Box::pin(AioRead::new(
        f.as_fd(),
        8, //offset
        &mut rbuf,
        0, //priority
        SigevNotify::SigevNone,
    ));
    wcb.as_mut().submit().unwrap();
    rcb.as_mut().submit().unwrap();
    loop {
        {
            let cbbuf = [
                &*wcb as &dyn AsRef<libc::aiocb>,
                &*rcb as &dyn AsRef<libc::aiocb>,
            ];
            let r = aio_suspend(&cbbuf[..], Some(timeout));
            match r {
                Err(Errno::EINTR) => continue,
                Err(e) => panic!("aio_suspend returned {e:?}"),
                Ok(_) => (),
            };
        }
        if rcb.as_mut().error() != Err(Errno::EINPROGRESS)
            && wcb.as_mut().error() != Err(Errno::EINPROGRESS)
        {
            break;
        }
    }

    assert_eq!(wcb.as_mut().aio_return().unwrap(), WBUF.len());
    assert_eq!(rcb.as_mut().aio_return().unwrap(), rlen);
}

/// aio_suspend relies on casting Rust Aio* struct pointers to libc::aiocb
/// pointers.  This test ensures that such casts are valid.
#[test]
fn casting() {
    let sev = SigevNotify::SigevNone;
    // Only safe because we'll never await the futures
    let fd = unsafe { BorrowedFd::borrow_raw(666) };
    let aiof = AioFsync::new(fd, AioFsyncMode::O_SYNC, 0, sev);
    assert_eq!(
        aiof.as_ref() as *const libc::aiocb,
        &aiof as *const AioFsync as *const libc::aiocb
    );

    let mut rbuf = [];
    let aior = AioRead::new(fd, 0, &mut rbuf, 0, sev);
    assert_eq!(
        aior.as_ref() as *const libc::aiocb,
        &aior as *const AioRead as *const libc::aiocb
    );

    let wbuf = [];
    let aiow = AioWrite::new(fd, 0, &wbuf, 0, sev);
    assert_eq!(
        aiow.as_ref() as *const libc::aiocb,
        &aiow as *const AioWrite as *const libc::aiocb
    );
}

#[cfg(target_os = "freebsd")]
#[test]
fn casting_vectored() {
    use std::io::{IoSlice, IoSliceMut};

    let sev = SigevNotify::SigevNone;

    let mut rbuf = [];
    let mut rbufs = [IoSliceMut::new(&mut rbuf)];
    // Only safe because we'll never await the futures
    let fd = unsafe { BorrowedFd::borrow_raw(666) };
    let aiorv = AioReadv::new(fd, 0, &mut rbufs[..], 0, sev);
    assert_eq!(
        aiorv.as_ref() as *const libc::aiocb,
        &aiorv as *const AioReadv as *const libc::aiocb
    );

    let wbuf = [];
    let wbufs = [IoSlice::new(&wbuf)];
    let aiowv = AioWritev::new(fd, 0, &wbufs, 0, sev);
    assert_eq!(
        aiowv.as_ref() as *const libc::aiocb,
        &aiowv as *const AioWritev as *const libc::aiocb
    );
}
