//! Mock version of std::fs::File;
use mockall::mock;

use crate::sync::oneshot;
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::{Metadata, Permissions},
    future::Future,
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

mock! {
    #[derive(Debug)]
    pub File {
        pub fn create(pb: PathBuf) -> io::Result<Self>;
        // These inner_ methods exist because std::fs::File has two
        // implementations for each of these methods: one on "&mut self" and
        // one on "&&self".  Defining both of those in terms of an inner_ method
        // allows us to specify the expectation the same way, regardless of
        // which method is used.
        pub fn inner_flush(&self) -> io::Result<()>;
        pub fn inner_read(&self, dst: &mut [u8]) -> io::Result<usize>;
        pub fn inner_seek(&self, pos: SeekFrom) -> io::Result<u64>;
        pub fn inner_write(&self, src: &[u8]) -> io::Result<usize>;
        pub fn metadata(&self) -> io::Result<Metadata>;
        pub fn open(pb: PathBuf) -> io::Result<Self>;
        pub fn set_len(&self, size: u64) -> io::Result<()>;
        pub fn set_permissions(&self, _perm: Permissions) -> io::Result<()>;
        pub fn set_max_buf_size(&self, max_buf_size: usize);
        pub fn sync_all(&self) -> io::Result<()>;
        pub fn sync_data(&self) -> io::Result<()>;
        pub fn try_clone(&self) -> io::Result<Self>;
    }
    #[cfg(windows)]
    impl std::os::windows::io::AsRawHandle for File {
        fn as_raw_handle(&self) -> std::os::windows::io::RawHandle;
    }
    #[cfg(windows)]
    impl std::os::windows::io::FromRawHandle for File {
        unsafe fn from_raw_handle(h: std::os::windows::io::RawHandle) -> Self;
    }
    #[cfg(unix)]
    impl std::os::unix::io::AsRawFd for File {
        fn as_raw_fd(&self) -> std::os::unix::io::RawFd;
    }

    #[cfg(unix)]
    impl std::os::unix::io::FromRawFd for File {
        unsafe fn from_raw_fd(h: std::os::unix::io::RawFd) -> Self;
    }
}

impl Read for MockFile {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.inner_read(dst)
    }
}

impl Read for &'_ MockFile {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        // Placate Miri.  Tokio will call this method with an uninitialized
        // buffer, which is ok because std::io::Read::read implementations don't usually read
        // from their input buffers.  But Mockall 0.12-0.13 will try to Debug::fmt the
        // buffer, even if there is no failure, triggering an uninitialized data access alert from
        // Miri.  Initialize the data here just to prevent those Miri alerts.
        // This can be removed after upgrading to Mockall 0.14.
        dst.fill(0);
        self.inner_read(dst)
    }
}

impl Seek for &'_ MockFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner_seek(pos)
    }
}

impl Write for &'_ MockFile {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        self.inner_write(src)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner_flush()
    }
}

tokio_thread_local! {
    static QUEUE: RefCell<VecDeque<Box<dyn FnOnce() + Send>>> = RefCell::new(VecDeque::new())
}

#[derive(Debug)]
pub(super) struct JoinHandle<T> {
    rx: oneshot::Receiver<T>,
}

pub(super) fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    let task = Box::new(move || {
        let _ = tx.send(f());
    });

    QUEUE.with(|cell| cell.borrow_mut().push_back(task));

    JoinHandle { rx }
}

pub(super) fn spawn_mandatory_blocking<F, R>(f: F) -> Option<JoinHandle<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    let task = Box::new(move || {
        let _ = tx.send(f());
    });

    QUEUE.with(|cell| cell.borrow_mut().push_back(task));

    Some(JoinHandle { rx })
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, io::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use std::task::Poll;

        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
            Poll::Ready(Err(e)) => panic!("error = {e:?}"),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(super) mod pool {
    use super::*;

    pub(in super::super) fn len() -> usize {
        QUEUE.with(|cell| cell.borrow().len())
    }

    pub(in super::super) fn run_one() {
        let task = QUEUE
            .with(|cell| cell.borrow_mut().pop_front())
            .expect("expected task to run, but none ready");

        task();
    }
}
