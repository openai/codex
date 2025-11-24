//! Windows asynchronous process handling.
//!
//! Like with Unix we don't actually have a way of registering a process with an
//! IOCP object. As a result we similarly need another mechanism for getting a
//! signal when a process has exited. For now this is implemented with the
//! `RegisterWaitForSingleObject` function in the kernel32.dll.
//!
//! This strategy is the same that libuv takes and essentially just queues up a
//! wait for the process in a kernel32-specific thread pool. Once the object is
//! notified (e.g. the process exits) then we have a callback that basically
//! just completes a `Oneshot`.
//!
//! The `poll_exit` implementation will attempt to wait for the process in a
//! nonblocking fashion, but failing that it'll fire off a
//! `RegisterWaitForSingleObject` and then wait on the other end of the oneshot
//! from then on out.

use crate::io::{blocking::Blocking, AsyncRead, AsyncWrite, ReadBuf};
use crate::process::kill::Kill;
use crate::process::SpawnedChild;
use crate::sync::oneshot;

use std::fmt;
use std::fs::File as StdFile;
use std::future::Future;
use std::io;
use std::os::windows::prelude::{AsRawHandle, IntoRawHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::process::Stdio;
use std::process::{Child as StdChild, ExitStatus};
use std::ptr::null_mut;
use std::sync::Arc;
use std::task::{Context, Poll};

use windows_sys::{
    Win32::Foundation::{
        DuplicateHandle, BOOLEAN, DUPLICATE_SAME_ACCESS, HANDLE, INVALID_HANDLE_VALUE,
    },
    Win32::System::Threading::{
        GetCurrentProcess, RegisterWaitForSingleObject, UnregisterWaitEx, INFINITE,
        WT_EXECUTEINWAITTHREAD, WT_EXECUTEONLYONCE,
    },
};

#[must_use = "futures do nothing unless polled"]
pub(crate) struct Child {
    child: StdChild,
    waiting: Option<Waiting>,
}

impl fmt::Debug for Child {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Child")
            .field("pid", &self.id())
            .field("child", &self.child)
            .field("waiting", &"..")
            .finish()
    }
}

struct Waiting {
    rx: oneshot::Receiver<()>,
    wait_object: HANDLE,
    tx: *mut Option<oneshot::Sender<()>>,
}

unsafe impl Sync for Waiting {}
unsafe impl Send for Waiting {}

pub(crate) fn build_child(mut child: StdChild) -> io::Result<SpawnedChild> {
    let stdin = child.stdin.take().map(stdio).transpose()?;
    let stdout = child.stdout.take().map(stdio).transpose()?;
    let stderr = child.stderr.take().map(stdio).transpose()?;

    Ok(SpawnedChild {
        child: Child {
            child,
            waiting: None,
        },
        stdin,
        stdout,
        stderr,
    })
}

impl Child {
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}

impl Kill for Child {
    fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }
}

impl Future for Child {
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = Pin::get_mut(self);
        loop {
            if let Some(ref mut w) = inner.waiting {
                match Pin::new(&mut w.rx).poll(cx) {
                    Poll::Ready(Ok(())) => {}
                    Poll::Ready(Err(_)) => panic!("should not be canceled"),
                    Poll::Pending => return Poll::Pending,
                }
                let status = inner.try_wait()?.expect("not ready yet");
                return Poll::Ready(Ok(status));
            }

            if let Some(e) = inner.try_wait()? {
                return Poll::Ready(Ok(e));
            }
            let (tx, rx) = oneshot::channel();
            let ptr = Box::into_raw(Box::new(Some(tx)));
            let mut wait_object = null_mut();
            let rc = unsafe {
                RegisterWaitForSingleObject(
                    &mut wait_object,
                    inner.child.as_raw_handle() as _,
                    Some(callback),
                    ptr as *mut _,
                    INFINITE,
                    WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE,
                )
            };
            if rc == 0 {
                let err = io::Error::last_os_error();
                drop(unsafe { Box::from_raw(ptr) });
                return Poll::Ready(Err(err));
            }
            inner.waiting = Some(Waiting {
                rx,
                wait_object,
                tx: ptr,
            });
        }
    }
}

impl AsRawHandle for Child {
    fn as_raw_handle(&self) -> RawHandle {
        self.child.as_raw_handle()
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        unsafe {
            let rc = UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE);
            if rc == 0 {
                panic!("failed to unregister: {}", io::Error::last_os_error());
            }
            drop(Box::from_raw(self.tx));
        }
    }
}

unsafe extern "system" fn callback(ptr: *mut std::ffi::c_void, _timer_fired: BOOLEAN) {
    let complete = &mut *(ptr as *mut Option<oneshot::Sender<()>>);
    let _ = complete.take().unwrap().send(());
}

#[derive(Debug)]
struct ArcFile(Arc<StdFile>);

impl io::Read for ArcFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(bytes)
    }
}

impl io::Write for ArcFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&*self.0).write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

#[derive(Debug)]
pub(crate) struct ChildStdio {
    // Used for accessing the raw handle, even if the io version is busy
    raw: Arc<StdFile>,
    // For doing I/O operations asynchronously
    io: Blocking<ArcFile>,
}

impl ChildStdio {
    pub(super) fn into_owned_handle(self) -> io::Result<OwnedHandle> {
        convert_to_file(self).map(OwnedHandle::from)
    }
}

impl AsRawHandle for ChildStdio {
    fn as_raw_handle(&self) -> RawHandle {
        self.raw.as_raw_handle()
    }
}

impl AsyncRead for ChildStdio {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildStdio {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

pub(super) fn stdio<T>(io: T) -> io::Result<ChildStdio>
where
    T: IntoRawHandle,
{
    use std::os::windows::prelude::FromRawHandle;

    let raw = Arc::new(unsafe { StdFile::from_raw_handle(io.into_raw_handle()) });
    let io = ArcFile(raw.clone());
    // SAFETY: the `Read` implementation of `io` does not
    // read from the buffer it is borrowing and correctly
    // reports the length of the data written into the buffer.
    let io = unsafe { Blocking::new(io) };
    Ok(ChildStdio { raw, io })
}

fn convert_to_file(child_stdio: ChildStdio) -> io::Result<StdFile> {
    let ChildStdio { raw, io } = child_stdio;
    drop(io); // Try to drop the Arc count here

    Arc::try_unwrap(raw).or_else(|raw| duplicate_handle(&*raw))
}

pub(crate) fn convert_to_stdio(child_stdio: ChildStdio) -> io::Result<Stdio> {
    convert_to_file(child_stdio).map(Stdio::from)
}

fn duplicate_handle<T: AsRawHandle>(io: &T) -> io::Result<StdFile> {
    use std::os::windows::prelude::FromRawHandle;

    unsafe {
        let mut dup_handle = INVALID_HANDLE_VALUE;
        let cur_proc = GetCurrentProcess();

        let status = DuplicateHandle(
            cur_proc,
            io.as_raw_handle() as _,
            cur_proc,
            &mut dup_handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        );

        if status == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(StdFile::from_raw_handle(dup_handle as _))
    }
}
