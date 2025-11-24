#![warn(rust_2018_idioms)]
#![cfg(all(unix, feature = "full"))]

use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use std::{
    future::Future,
    io::{self, ErrorKind, Read, Write},
    task::{Context, Waker},
};

use nix::unistd::{read, write};

use futures::poll;

use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};
use tokio::io::Interest;
use tokio_test::{assert_err, assert_pending};

struct TestWaker {
    inner: Arc<TestWakerInner>,
    waker: Waker,
}

#[derive(Default)]
struct TestWakerInner {
    awoken: AtomicBool,
}

impl futures::task::ArcWake for TestWakerInner {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.awoken.store(true, Ordering::SeqCst);
    }
}

impl TestWaker {
    fn new() -> Self {
        let inner: Arc<TestWakerInner> = Default::default();

        Self {
            inner: inner.clone(),
            waker: futures::task::waker(inner),
        }
    }

    fn awoken(&self) -> bool {
        self.inner.awoken.swap(false, Ordering::SeqCst)
    }

    fn context(&self) -> Context<'_> {
        Context::from_waker(&self.waker)
    }
}

#[derive(Debug)]
struct FileDescriptor {
    fd: std::os::fd::OwnedFd,
}

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl Read for &FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        read(self.fd.as_raw_fd(), buf).map_err(io::Error::from)
    }
}

impl Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (self as &Self).read(buf)
    }
}

impl Write for &FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(&self.fd, buf).map_err(io::Error::from)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (self as &Self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (self as &Self).flush()
    }
}

fn set_nonblocking(fd: RawFd) {
    use nix::fcntl::{OFlag, F_GETFL, F_SETFL};

    let flags = nix::fcntl::fcntl(fd, F_GETFL).expect("fcntl(F_GETFD)");

    if flags < 0 {
        panic!(
            "bad return value from fcntl(F_GETFL): {} ({:?})",
            flags,
            nix::Error::last()
        );
    }

    let flags = OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK;

    nix::fcntl::fcntl(fd, F_SETFL(flags)).expect("fcntl(F_SETFD)");
}

fn socketpair() -> (FileDescriptor, FileDescriptor) {
    use nix::sys::socket::{self, AddressFamily, SockFlag, SockType};

    let (fd_a, fd_b) = socket::socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .expect("socketpair");
    let fds = (FileDescriptor { fd: fd_a }, FileDescriptor { fd: fd_b });

    set_nonblocking(fds.0.fd.as_raw_fd());
    set_nonblocking(fds.1.fd.as_raw_fd());

    fds
}

fn drain(mut fd: &FileDescriptor, mut amt: usize) {
    let mut buf = [0u8; 512];
    while amt > 0 {
        match fd.read(&mut buf[..]) {
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Ok(0) => panic!("unexpected EOF"),
            Err(e) => panic!("unexpected error: {e:?}"),
            Ok(x) => amt -= x,
        }
    }
}

#[tokio::test]
async fn initially_writable() {
    let (a, b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();
    let afd_b = AsyncFd::new(b).unwrap();

    afd_a.writable().await.unwrap().clear_ready();
    afd_b.writable().await.unwrap().clear_ready();

    tokio::select! {
        biased;
        _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        _ = afd_a.readable() => panic!("Unexpected readable state"),
        _ = afd_b.readable() => panic!("Unexpected readable state"),
    }
}

#[tokio::test]
async fn reset_readable() {
    let (a, mut b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();

    let readable = afd_a.readable();
    tokio::pin!(readable);

    tokio::select! {
        _ = readable.as_mut() => panic!(),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    b.write_all(b"0").unwrap();

    let mut guard = readable.await.unwrap();

    guard
        .try_io(|_| afd_a.get_ref().read(&mut [0]))
        .unwrap()
        .unwrap();

    // `a` is not readable, but the reactor still thinks it is
    // (because we have not observed a not-ready error yet)
    afd_a.readable().await.unwrap().retain_ready();

    // Explicitly clear the ready state
    guard.clear_ready();

    let readable = afd_a.readable();
    tokio::pin!(readable);

    tokio::select! {
        _ = readable.as_mut() => panic!(),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    b.write_all(b"0").unwrap();

    // We can observe the new readable event
    afd_a.readable().await.unwrap().clear_ready();
}

#[tokio::test]
async fn reset_writable() {
    let (a, b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();

    let mut guard = afd_a.writable().await.unwrap();

    // Write until we get a WouldBlock. This also clears the ready state.
    let mut bytes = 0;
    while let Ok(Ok(amt)) = guard.try_io(|_| afd_a.get_ref().write(&[0; 512][..])) {
        bytes += amt;
    }

    // Writable state should be cleared now.
    let writable = afd_a.writable();
    tokio::pin!(writable);

    tokio::select! {
        _ = writable.as_mut() => panic!(),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    // Read from the other side; we should become writable now.
    drain(&b, bytes);

    let _ = writable.await.unwrap();
}

#[derive(Debug)]
struct ArcFd<T>(Arc<T>);
impl<T: AsRawFd> AsRawFd for ArcFd<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[tokio::test]
async fn drop_closes() {
    let (a, mut b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();

    assert_eq!(
        ErrorKind::WouldBlock,
        b.read(&mut [0]).err().unwrap().kind()
    );

    std::mem::drop(afd_a);

    assert_eq!(0, b.read(&mut [0]).unwrap());

    // into_inner does not close the fd

    let (a, mut b) = socketpair();
    let afd_a = AsyncFd::new(a).unwrap();
    let _a: FileDescriptor = afd_a.into_inner();

    assert_eq!(
        ErrorKind::WouldBlock,
        b.read(&mut [0]).err().unwrap().kind()
    );

    // Drop closure behavior is delegated to the inner object
    let (a, mut b) = socketpair();
    let arc_fd = Arc::new(a);
    let afd_a = AsyncFd::new(ArcFd(arc_fd.clone())).unwrap();
    std::mem::drop(afd_a);

    assert_eq!(
        ErrorKind::WouldBlock,
        b.read(&mut [0]).err().unwrap().kind()
    );

    std::mem::drop(arc_fd); // suppress unnecessary clone clippy warning
}

#[tokio::test]
async fn reregister() {
    let (a, _b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();
    let a = afd_a.into_inner();
    AsyncFd::new(a).unwrap();
}

#[tokio::test]
async fn guard_try_io() {
    let (a, mut b) = socketpair();

    b.write_all(b"0").unwrap();

    let afd_a = AsyncFd::new(a).unwrap();

    let mut guard = afd_a.readable().await.unwrap();

    afd_a.get_ref().read_exact(&mut [0]).unwrap();

    // Should not clear the readable state
    let _ = guard.try_io(|_| Ok(()));

    // Still readable...
    let _ = afd_a.readable().await.unwrap();

    // Should clear the readable state
    let _ = guard.try_io(|_| io::Result::<()>::Err(ErrorKind::WouldBlock.into()));

    // Assert not readable
    let readable = afd_a.readable();
    tokio::pin!(readable);

    tokio::select! {
        _ = readable.as_mut() => panic!(),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    // Write something down b again and make sure we're reawoken
    b.write_all(b"0").unwrap();
    let _ = readable.await.unwrap();
}

#[tokio::test]
async fn try_io_readable() {
    let (a, mut b) = socketpair();
    let mut afd_a = AsyncFd::new(a).unwrap();

    // Give the runtime some time to update bookkeeping.
    tokio::task::yield_now().await;

    {
        let mut called = false;
        let _ = afd_a.try_io_mut(Interest::READABLE, |_| {
            called = true;
            Ok(())
        });
        assert!(
            !called,
            "closure should not have been called, since socket should not be readable"
        );
    }

    // Make `a` readable by writing to `b`.
    // Give the runtime some time to update bookkeeping.
    b.write_all(&[0]).unwrap();
    tokio::task::yield_now().await;

    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::READABLE, |_| {
            called = true;
            Ok(())
        });
        assert!(
            called,
            "closure should have been called, since socket should have data available to read"
        );
    }

    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::READABLE, |_| {
            called = true;
            io::Result::<()>::Err(ErrorKind::WouldBlock.into())
        });
        assert!(
            called,
            "closure should have been called, since socket should have data available to read"
        );
    }

    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::READABLE, |_| {
            called = true;
            Ok(())
        });
        assert!(!called, "closure should not have been called, since socket readable state should have been cleared");
    }
}

#[tokio::test]
async fn try_io_writable() {
    let (a, _b) = socketpair();
    let afd_a = AsyncFd::new(a).unwrap();

    // Give the runtime some time to update bookkeeping.
    tokio::task::yield_now().await;

    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::WRITABLE, |_| {
            called = true;
            Ok(())
        });
        assert!(
            called,
            "closure should have been called, since socket should still be marked as writable"
        );
    }
    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::WRITABLE, |_| {
            called = true;
            io::Result::<()>::Err(ErrorKind::WouldBlock.into())
        });
        assert!(
            called,
            "closure should have been called, since socket should still be marked as writable"
        );
    }

    {
        let mut called = false;
        let _ = afd_a.try_io(Interest::WRITABLE, |_| {
            called = true;
            Ok(())
        });
        assert!(!called, "closure should not have been called, since socket writable state should have been cleared");
    }
}

#[tokio::test]
async fn multiple_waiters() {
    let (a, mut b) = socketpair();
    let afd_a = Arc::new(AsyncFd::new(a).unwrap());

    let barrier = Arc::new(tokio::sync::Barrier::new(11));

    let mut tasks = Vec::new();
    for _ in 0..10 {
        let afd_a = afd_a.clone();
        let barrier = barrier.clone();

        let f = async move {
            let notify_barrier = async {
                barrier.wait().await;
                futures::future::pending::<()>().await;
            };

            tokio::select! {
                biased;
                guard = afd_a.readable() => {
                    tokio::task::yield_now().await;
                    guard.unwrap().clear_ready()
                },
                _ = notify_barrier => unreachable!(),
            }

            std::mem::drop(afd_a);
        };

        tasks.push(tokio::spawn(f));
    }

    let mut all_tasks = futures::future::try_join_all(tasks);

    tokio::select! {
        r = std::pin::Pin::new(&mut all_tasks) => {
            r.unwrap(); // propagate panic
            panic!("Tasks exited unexpectedly")
        },
        _ = barrier.wait() => {}
    }

    b.write_all(b"0").unwrap();

    all_tasks.await.unwrap();
}

#[tokio::test]
// Block on https://github.com/rust-lang/miri/issues/4374
#[cfg_attr(miri, ignore)]
async fn poll_fns() {
    let (a, b) = socketpair();
    let afd_a = Arc::new(AsyncFd::new(a).unwrap());
    let afd_b = Arc::new(AsyncFd::new(b).unwrap());

    // Fill up the write side of A
    let mut bytes = 0;
    while let Ok(amt) = afd_a.get_ref().write(&[0; 512]) {
        bytes += amt;
    }

    let waker = TestWaker::new();

    assert_pending!(afd_a.as_ref().poll_read_ready(&mut waker.context()));

    let afd_a_2 = afd_a.clone();
    let r_barrier = Arc::new(tokio::sync::Barrier::new(2));
    let barrier_clone = r_barrier.clone();

    let read_fut = tokio::spawn(async move {
        // Move waker onto this task first
        assert_pending!(poll!(std::future::poll_fn(|cx| afd_a_2
            .as_ref()
            .poll_read_ready(cx))));
        barrier_clone.wait().await;

        let _ = std::future::poll_fn(|cx| afd_a_2.as_ref().poll_read_ready(cx)).await;
    });

    let afd_a_2 = afd_a.clone();
    let w_barrier = Arc::new(tokio::sync::Barrier::new(2));
    let barrier_clone = w_barrier.clone();

    let mut write_fut = tokio::spawn(async move {
        // Move waker onto this task first
        assert_pending!(poll!(std::future::poll_fn(|cx| afd_a_2
            .as_ref()
            .poll_write_ready(cx))));
        barrier_clone.wait().await;

        let _ = std::future::poll_fn(|cx| afd_a_2.as_ref().poll_write_ready(cx)).await;
    });

    r_barrier.wait().await;
    w_barrier.wait().await;

    let readable = afd_a.readable();
    tokio::pin!(readable);

    tokio::select! {
        _ = &mut readable => unreachable!(),
        _ = tokio::task::yield_now() => {}
    }

    // Make A readable. We expect that 'readable' and 'read_fut' will both complete quickly
    afd_b.get_ref().write_all(b"0").unwrap();

    let _ = tokio::join!(readable, read_fut);

    // Our original waker should _not_ be awoken (poll_read_ready retains only the last context)
    assert!(!waker.awoken());

    // The writable side should not be awoken
    tokio::select! {
        _ = &mut write_fut => unreachable!(),
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }

    // Make it writable now
    drain(afd_b.get_ref(), bytes);

    // now we should be writable (ie - the waker for poll_write should still be registered after we wake the read side)
    let _ = write_fut.await;
}

fn assert_pending<T: std::fmt::Debug, F: Future<Output = T>>(f: F) -> std::pin::Pin<Box<F>> {
    let mut pinned = Box::pin(f);

    assert_pending!(pinned
        .as_mut()
        .poll(&mut Context::from_waker(futures::task::noop_waker_ref())));

    pinned
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn driver_shutdown_wakes_currently_pending() {
    let rt = rt();

    let (a, _b) = socketpair();
    let afd_a = {
        let _enter = rt.enter();
        AsyncFd::new(a).unwrap()
    };

    let readable = assert_pending(afd_a.readable());

    std::mem::drop(rt);

    // The future was initialized **before** dropping the rt
    assert_err!(futures::executor::block_on(readable));

    // The future is initialized **after** dropping the rt.
    assert_err!(futures::executor::block_on(afd_a.readable()));
}

#[test]
fn driver_shutdown_wakes_future_pending() {
    let rt = rt();

    let (a, _b) = socketpair();
    let afd_a = {
        let _enter = rt.enter();
        AsyncFd::new(a).unwrap()
    };

    std::mem::drop(rt);

    assert_err!(futures::executor::block_on(afd_a.readable()));
}

#[test]
fn driver_shutdown_wakes_pending_race() {
    // TODO: make this a loom test
    for _ in 0..100 {
        let rt = rt();

        let (a, _b) = socketpair();
        let afd_a = {
            let _enter = rt.enter();
            AsyncFd::new(a).unwrap()
        };

        let _ = std::thread::spawn(move || std::mem::drop(rt));

        // This may or may not return an error (but will be awoken)
        let _ = futures::executor::block_on(afd_a.readable());

        // However retrying will always return an error
        assert_err!(futures::executor::block_on(afd_a.readable()));
    }
}

async fn poll_readable<T: AsRawFd>(fd: &AsyncFd<T>) -> std::io::Result<AsyncFdReadyGuard<'_, T>> {
    std::future::poll_fn(|cx| fd.poll_read_ready(cx)).await
}

async fn poll_writable<T: AsRawFd>(fd: &AsyncFd<T>) -> std::io::Result<AsyncFdReadyGuard<'_, T>> {
    std::future::poll_fn(|cx| fd.poll_write_ready(cx)).await
}

#[test]
fn driver_shutdown_wakes_currently_pending_polls() {
    let rt = rt();

    let (a, _b) = socketpair();
    let afd_a = {
        let _enter = rt.enter();
        AsyncFd::new(a).unwrap()
    };

    while afd_a.get_ref().write(&[0; 512]).is_ok() {} // make not writable

    let readable = assert_pending(poll_readable(&afd_a));
    let writable = assert_pending(poll_writable(&afd_a));

    std::mem::drop(rt);

    // Attempting to poll readiness when the rt is dropped is an error
    assert_err!(futures::executor::block_on(readable));
    assert_err!(futures::executor::block_on(writable));
}

#[test]
fn driver_shutdown_wakes_poll() {
    let rt = rt();

    let (a, _b) = socketpair();
    let afd_a = {
        let _enter = rt.enter();
        AsyncFd::new(a).unwrap()
    };

    std::mem::drop(rt);

    assert_err!(futures::executor::block_on(poll_readable(&afd_a)));
    assert_err!(futures::executor::block_on(poll_writable(&afd_a)));
}

#[test]
fn driver_shutdown_then_clear_readiness() {
    let rt = rt();

    let (a, _b) = socketpair();
    let afd_a = {
        let _enter = rt.enter();
        AsyncFd::new(a).unwrap()
    };

    let mut write_ready = rt.block_on(afd_a.writable()).unwrap();

    std::mem::drop(rt);

    write_ready.clear_ready();
}

#[test]
fn driver_shutdown_wakes_poll_race() {
    // TODO: make this a loom test
    for _ in 0..100 {
        let rt = rt();

        let (a, _b) = socketpair();
        let afd_a = {
            let _enter = rt.enter();
            AsyncFd::new(a).unwrap()
        };

        while afd_a.get_ref().write(&[0; 512]).is_ok() {} // make not writable

        let _ = std::thread::spawn(move || std::mem::drop(rt));

        // The poll variants will always return an error in this case
        assert_err!(futures::executor::block_on(poll_readable(&afd_a)));
        assert_err!(futures::executor::block_on(poll_writable(&afd_a)));
    }
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No socket in miri.
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn priority_event_on_oob_data() {
    use std::net::SocketAddr;

    use tokio::io::Interest;

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let listener = std::net::TcpListener::bind(addr).unwrap();
    let addr = listener.local_addr().unwrap();

    let client = std::net::TcpStream::connect(addr).unwrap();
    let client = AsyncFd::with_interest(client, Interest::PRIORITY).unwrap();

    let (stream, _) = listener.accept().unwrap();

    // Sending out of band data should trigger priority event.
    send_oob_data(&stream, b"hello").unwrap();

    let _ = client.ready(Interest::PRIORITY).await.unwrap();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn send_oob_data<S: AsRawFd>(stream: &S, data: &[u8]) -> io::Result<usize> {
    unsafe {
        let res = libc::send(
            stream.as_raw_fd(),
            data.as_ptr().cast(),
            data.len(),
            libc::MSG_OOB,
        );
        if res == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }
}

#[tokio::test]
async fn clear_ready_matching_clears_ready() {
    use tokio::io::{Interest, Ready};

    let (a, mut b) = socketpair();

    let afd_a = AsyncFd::new(a).unwrap();
    b.write_all(b"0").unwrap();

    let mut guard = afd_a
        .ready(Interest::READABLE | Interest::WRITABLE)
        .await
        .unwrap();

    assert_eq!(guard.ready(), Ready::READABLE | Ready::WRITABLE);

    guard.clear_ready_matching(Ready::READABLE);
    assert_eq!(guard.ready(), Ready::WRITABLE);

    guard.clear_ready_matching(Ready::WRITABLE);
    assert_eq!(guard.ready(), Ready::EMPTY);
}

#[tokio::test]
async fn clear_ready_matching_clears_ready_mut() {
    use tokio::io::{Interest, Ready};

    let (a, mut b) = socketpair();

    let mut afd_a = AsyncFd::new(a).unwrap();
    b.write_all(b"0").unwrap();

    let mut guard = afd_a
        .ready_mut(Interest::READABLE | Interest::WRITABLE)
        .await
        .unwrap();

    assert_eq!(guard.ready(), Ready::READABLE | Ready::WRITABLE);

    guard.clear_ready_matching(Ready::READABLE);
    assert_eq!(guard.ready(), Ready::WRITABLE);

    guard.clear_ready_matching(Ready::WRITABLE);
    assert_eq!(guard.ready(), Ready::EMPTY);
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No socket in miri.
#[cfg(target_os = "linux")]
async fn await_error_readiness_timestamping() {
    use std::net::{Ipv4Addr, SocketAddr};

    use tokio::io::{Interest, Ready};

    let address_a = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let address_b = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));

    let socket = std::net::UdpSocket::bind(address_a).unwrap();

    socket.set_nonblocking(true).unwrap();

    // configure send timestamps
    configure_timestamping_socket(&socket).unwrap();

    socket.connect(address_b).unwrap();

    let fd = AsyncFd::new(socket).unwrap();

    tokio::select! {
        _ = fd.ready(Interest::ERROR) => panic!(),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    let buf = b"hello there";
    fd.get_ref().send(buf).unwrap();

    // the send timestamp should now be in the error queue
    let guard = fd.ready(Interest::ERROR).await.unwrap();
    assert_eq!(guard.ready(), Ready::ERROR);
}

#[cfg(target_os = "linux")]
fn configure_timestamping_socket(udp_socket: &std::net::UdpSocket) -> std::io::Result<libc::c_int> {
    // enable software timestamping, and specifically software send timestamping
    let options = libc::SOF_TIMESTAMPING_SOFTWARE | libc::SOF_TIMESTAMPING_TX_SOFTWARE;

    let res = unsafe {
        libc::setsockopt(
            udp_socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_TIMESTAMP,
            &options as *const _ as *const libc::c_void,
            std::mem::size_of_val(&options) as libc::socklen_t,
        )
    };

    if res == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(res)
    }
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No socket in miri.
#[cfg(target_os = "linux")]
async fn await_error_readiness_invalid_address() {
    use std::net::{Ipv4Addr, SocketAddr};
    use tokio::io::{Interest, Ready};

    let socket_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let socket = std::net::UdpSocket::bind(socket_addr).unwrap();
    let socket_fd = socket.as_raw_fd();

    // Enable IP_RECVERR option to receive error messages
    // https://man7.org/linux/man-pages/man7/ip.7.html has some extra information
    let recv_err: libc::c_int = 1;
    unsafe {
        let res = libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_IP,
            libc::IP_RECVERR,
            &recv_err as *const _ as *const libc::c_void,
            std::mem::size_of_val(&recv_err) as libc::socklen_t,
        );
        if res == -1 {
            panic!("{:?}", std::io::Error::last_os_error());
        }
    }

    // Spawn a separate thread for sending messages
    tokio::spawn(async move {
        // Set the destination address. This address is invalid in this context. the OS will notice
        // that nobody is listening on port this port. Normally this is ignored (UDP is "fire and forget"),
        // but because IP_RECVERR is enabled, the error will actually be reported to the sending socket
        let mut dest_addr =
            unsafe { std::mem::MaybeUninit::<libc::sockaddr_in>::zeroed().assume_init() };
        dest_addr.sin_family = libc::AF_INET as _;
        // based on https://en.wikipedia.org/wiki/Ephemeral_port, we should pick a port number
        // below 1024 to guarantee that other tests don't select this port by accident when they
        // use port 0 to select an ephemeral port.
        dest_addr.sin_port = 512u16.to_be(); // Destination port

        // Prepare the message data
        let message = "Hello, Socket!";

        // Prepare the message structure for sendmsg
        let mut iov = libc::iovec {
            iov_base: message.as_ptr() as *mut libc::c_void,
            iov_len: message.len(),
        };

        // Prepare the destination address for the sendmsg call
        let dest_sockaddr: *const libc::sockaddr = &dest_addr as *const _ as *const libc::sockaddr;
        let dest_addrlen: libc::socklen_t = std::mem::size_of_val(&dest_addr) as libc::socklen_t;

        let mut msg: libc::msghdr = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        msg.msg_name = dest_sockaddr as *mut libc::c_void;
        msg.msg_namelen = dest_addrlen;
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;

        if unsafe { libc::sendmsg(socket_fd, &msg, 0) } == -1 {
            panic!("{:?}", std::io::Error::last_os_error())
        }
    });

    let fd = AsyncFd::new(socket).unwrap();

    let guard = fd.ready(Interest::ERROR).await.unwrap();
    assert_eq!(guard.ready(), Ready::ERROR);
}

#[derive(Debug, PartialEq, Eq)]
struct InvalidSource;

impl AsRawFd for InvalidSource {
    fn as_raw_fd(&self) -> RawFd {
        -1
    }
}

#[tokio::test]
async fn try_new() {
    let original = Arc::new(InvalidSource);

    let error = AsyncFd::try_new(original.clone()).unwrap_err();
    let (returned, _cause) = error.into_parts();

    assert!(Arc::ptr_eq(&original, &returned));
}

#[tokio::test]
async fn try_with_interest() {
    let original = Arc::new(InvalidSource);

    let error = AsyncFd::try_with_interest(original.clone(), Interest::READABLE).unwrap_err();
    let (returned, _cause) = error.into_parts();

    assert!(Arc::ptr_eq(&original, &returned));
}
