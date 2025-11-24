use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::{fmt, mem, slice};

use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_LISTENING, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES,
};
use windows_sys::Win32::System::IO::{
    CancelIoEx, GetOverlappedResult, OVERLAPPED, OVERLAPPED_ENTRY,
};

use crate::event::Source;
use crate::sys::windows::iocp::{CompletionPort, CompletionStatus};
use crate::sys::windows::{Event, Handle, Overlapped};
use crate::Registry;
use crate::{Interest, Token};

/// Non-blocking windows named pipe.
///
/// This structure internally contains a `HANDLE` which represents the named
/// pipe, and also maintains state associated with the mio event loop and active
/// I/O operations that have been scheduled to translate IOCP to a readiness
/// model.
///
/// Note, IOCP is a *completion* based model whereas mio is a *readiness* based
/// model. To bridge this, `NamedPipe` performs internal buffering. Writes are
/// written to an internal buffer and the buffer is submitted to IOCP. IOCP
/// reads are submitted using internal buffers and `NamedPipe::read` reads from
/// this internal buffer.
///
/// # Trait implementations
///
/// The `Read` and `Write` traits are implemented for `NamedPipe` and for
/// `&NamedPipe`. This represents that a named pipe can be concurrently read and
/// written to and also can be read and written to at all. Typically a named
/// pipe needs to be connected to a client before it can be read or written,
/// however.
///
/// Note that for I/O operations on a named pipe to succeed then the named pipe
/// needs to be associated with an event loop. Until this happens all I/O
/// operations will return a "would block" error.
///
/// # Managing connections
///
/// The `NamedPipe` type supports a `connect` method to connect to a client and
/// a `disconnect` method to disconnect from that client. These two methods only
/// work once a named pipe is associated with an event loop.
///
/// The `connect` method will succeed asynchronously and a completion can be
/// detected once the object receives a writable notification.
///
/// # Named pipe clients
///
/// Currently to create a client of a named pipe server then you can use the
/// `OpenOptions` type in the standard library to create a `File` that connects
/// to a named pipe. Afterwards you can use the `into_raw_handle` method coupled
/// with the `NamedPipe::from_raw_handle` method to convert that to a named pipe
/// that can operate asynchronously. Don't forget to pass the
/// `FILE_FLAG_OVERLAPPED` flag when opening the `File`.
pub struct NamedPipe {
    inner: Arc<Inner>,
}

/// # Notes
///
/// The memory layout of this structure must be fixed as the
/// `ptr_from_*_overlapped` methods depend on it, see the `ptr_from` test.
#[repr(C)]
struct Inner {
    // NOTE: careful modifying the order of these three fields, the `ptr_from_*`
    // methods depend on the layout!
    connect: Overlapped,
    read: Overlapped,
    write: Overlapped,
    event: Overlapped,
    // END NOTE.
    handle: Handle,
    connecting: AtomicBool,
    io: Mutex<Io>,
    pool: Mutex<BufferPool>,
}

// SAFETY: `Handles`s are, in general, not thread-safe. However, we only used `Handle`s for
// resources that are thread-safe in `Inner`.
unsafe impl Send for Inner {}

// SAFETY: `Handles`s are, in general, not thread-safe. However, we only used `Handle`s for
// resources that are thread-safe in `Inner`.
unsafe impl Sync for Inner {}

impl Inner {
    /// Converts a pointer to `Inner.connect` to a pointer to `Inner`.
    ///
    /// # Unsafety
    ///
    /// Caller must ensure `ptr` is pointing to `Inner.connect`.
    unsafe fn ptr_from_conn_overlapped(ptr: *mut OVERLAPPED) -> *const Inner {
        // `connect` is the first field, so the pointer are the same.
        ptr.cast()
    }

    /// Same as [`ptr_from_conn_overlapped`] but for `Inner.read`.
    unsafe fn ptr_from_read_overlapped(ptr: *mut OVERLAPPED) -> *const Inner {
        // `read` is after `connect: Overlapped`.
        (ptr as *mut Overlapped).wrapping_sub(1) as *const Inner
    }

    /// Same as [`ptr_from_conn_overlapped`] but for `Inner.write`.
    unsafe fn ptr_from_write_overlapped(ptr: *mut OVERLAPPED) -> *const Inner {
        // `write` is after `connect: Overlapped` and `read: Overlapped`.
        (ptr as *mut Overlapped).wrapping_sub(2) as *const Inner
    }

    /// Same as [`ptr_from_conn_overlapped`] but for `Inner.event`.
    unsafe fn ptr_from_event_overlapped(ptr: *mut OVERLAPPED) -> *const Inner {
        // `event` is after `connect: Overlapped`, `read: Overlapped`, and `write: Overlapped`.
        (ptr as *mut Overlapped).wrapping_sub(3) as *const Inner
    }

    /// Issue a connection request with the specified overlapped operation.
    ///
    /// This function will issue a request to connect a client to this server,
    /// returning immediately after starting the overlapped operation.
    ///
    /// If this function immediately succeeds then `Ok(true)` is returned. If
    /// the overlapped operation is enqueued and pending, then `Ok(false)` is
    /// returned. Otherwise an error is returned indicating what went wrong.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the
    /// `overlapped` pointer is valid until the end of the I/O operation. The
    /// kernel also requires that `overlapped` is unique for this I/O operation
    /// and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that this pointer is
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn connect_overlapped(&self, overlapped: *mut OVERLAPPED) -> io::Result<bool> {
        if ConnectNamedPipe(self.handle.raw(), overlapped) != 0 {
            return Ok(true);
        }

        let err = io::Error::last_os_error();

        match err.raw_os_error().map(|e| e as u32) {
            Some(ERROR_PIPE_CONNECTED) => Ok(true),
            Some(ERROR_NO_DATA) => Ok(true),
            Some(ERROR_IO_PENDING) => Ok(false),
            _ => Err(err),
        }
    }

    /// Disconnects this named pipe from any connected client.
    pub fn disconnect(&self) -> io::Result<()> {
        if unsafe { DisconnectNamedPipe(self.handle.raw()) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Issues an overlapped read operation to occur on this pipe.
    ///
    /// This function will issue an asynchronous read to occur in an overlapped
    /// fashion, returning immediately. The `buf` provided will be filled in
    /// with data and the request is tracked by the `overlapped` function
    /// provided.
    ///
    /// If the operation succeeds immediately, `Ok(Some(n))` is returned where
    /// `n` is the number of bytes read. If an asynchronous operation is
    /// enqueued, then `Ok(None)` is returned. Otherwise if an error occurred
    /// it is returned.
    ///
    /// When this operation completes (or if it completes immediately), another
    /// mechanism must be used to learn how many bytes were transferred (such as
    /// looking at the filed in the IOCP status message).
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the `buf` and
    /// `overlapped` pointers to be valid until the end of the I/O operation.
    /// The kernel also requires that `overlapped` is unique for this I/O
    /// operation and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that the pointers are
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn read_overlapped(
        &self,
        buf: &mut [u8],
        overlapped: *mut OVERLAPPED,
    ) -> io::Result<Option<usize>> {
        let len = std::cmp::min(buf.len(), u32::MAX as usize) as u32;
        let res = ReadFile(
            self.handle.raw(),
            buf.as_mut_ptr() as *mut _,
            len,
            std::ptr::null_mut(),
            overlapped,
        );
        if res == 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(err);
            }
        }

        let mut bytes = 0;
        let res = GetOverlappedResult(self.handle.raw(), overlapped, &mut bytes, 0);
        if res == 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) {
                Ok(None)
            } else {
                Err(err)
            }
        } else {
            Ok(Some(bytes as usize))
        }
    }

    /// Issues an overlapped write operation to occur on this pipe.
    ///
    /// This function will issue an asynchronous write to occur in an overlapped
    /// fashion, returning immediately. The `buf` provided will be filled in
    /// with data and the request is tracked by the `overlapped` function
    /// provided.
    ///
    /// If the operation succeeds immediately, `Ok(Some(n))` is returned where
    /// `n` is the number of bytes written. If an asynchronous operation is
    /// enqueued, then `Ok(None)` is returned. Otherwise if an error occurred
    /// it is returned.
    ///
    /// When this operation completes (or if it completes immediately), another
    /// mechanism must be used to learn how many bytes were transferred (such as
    /// looking at the filed in the IOCP status message).
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the `buf` and
    /// `overlapped` pointers to be valid until the end of the I/O operation.
    /// The kernel also requires that `overlapped` is unique for this I/O
    /// operation and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that the pointers are
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn write_overlapped(
        &self,
        buf: &[u8],
        overlapped: *mut OVERLAPPED,
    ) -> io::Result<Option<usize>> {
        let len = std::cmp::min(buf.len(), u32::MAX as usize) as u32;
        let res = WriteFile(
            self.handle.raw(),
            buf.as_ptr() as *const _,
            len,
            std::ptr::null_mut(),
            overlapped,
        );
        if res == 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(err);
            }
        }

        let mut bytes = 0;
        let res = GetOverlappedResult(self.handle.raw(), overlapped, &mut bytes, 0);
        if res == 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) {
                Ok(None)
            } else {
                Err(err)
            }
        } else {
            Ok(Some(bytes as usize))
        }
    }

    /// Calls the `GetOverlappedResult` function to get the result of an
    /// overlapped operation for this handle.
    ///
    /// This function takes the `OVERLAPPED` argument which must have been used
    /// to initiate an overlapped I/O operation, and returns either the
    /// successful number of bytes transferred during the operation or an error
    /// if one occurred.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe as `overlapped` must have previously been used
    /// to execute an operation for this handle, and it must also be a valid
    /// pointer to an `Overlapped` instance.
    #[inline]
    unsafe fn result(&self, overlapped: *mut OVERLAPPED) -> io::Result<usize> {
        let mut transferred = 0;
        let r = GetOverlappedResult(self.handle.raw(), overlapped, &mut transferred, 0);
        if r == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(transferred as usize)
        }
    }
}

#[test]
fn ptr_from() {
    use std::mem::ManuallyDrop;
    use std::ptr;

    let pipe = unsafe { ManuallyDrop::new(NamedPipe::from_raw_handle(ptr::null_mut())) };
    let inner: &Inner = &pipe.inner;
    assert_eq!(
        inner as *const Inner,
        unsafe { Inner::ptr_from_conn_overlapped(&inner.connect as *const _ as *mut OVERLAPPED) },
        "`ptr_from_conn_overlapped` incorrect"
    );
    assert_eq!(
        inner as *const Inner,
        unsafe { Inner::ptr_from_read_overlapped(&inner.read as *const _ as *mut OVERLAPPED) },
        "`ptr_from_read_overlapped` incorrect"
    );
    assert_eq!(
        inner as *const Inner,
        unsafe { Inner::ptr_from_write_overlapped(&inner.write as *const _ as *mut OVERLAPPED) },
        "`ptr_from_write_overlapped` incorrect"
    );
}

struct Io {
    // Uniquely identifies the selector associated with this named pipe
    cp: Option<Arc<CompletionPort>>,
    // Token used to identify events
    token: Option<Token>,
    read: State,
    write: State,
    connect_error: Option<io::Error>,
}

#[derive(Debug)]
enum State {
    None,
    Pending(Vec<u8>, usize),
    Ok(Vec<u8>, usize),
    Err(io::Error),
}

// Odd tokens are for named pipes
static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

fn would_block() -> io::Error {
    io::ErrorKind::WouldBlock.into()
}

impl NamedPipe {
    /// Creates a new named pipe at the specified `addr` given a "reasonable
    /// set" of initial configuration options.
    pub fn new<A: AsRef<OsStr>>(addr: A) -> io::Result<NamedPipe> {
        use std::os::windows::ffi::OsStrExt;
        let name: Vec<_> = addr.as_ref().encode_wide().chain(Some(0)).collect();

        // Safety: syscall
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE,
                PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                std::ptr::null_mut(),
            )
        };

        if h == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            // Safety: nothing actually unsafe about this. The trait fn includes
            // `unsafe`.
            Ok(unsafe { Self::from_raw_handle(h as RawHandle) })
        }
    }

    /// Attempts to call `ConnectNamedPipe`, if possible.
    ///
    /// This function will attempt to connect this pipe to a client in an
    /// asynchronous fashion. If the function immediately establishes a
    /// connection to a client then `Ok(())` is returned. Otherwise if a
    /// connection attempt was issued and is now in progress then a "would
    /// block" error is returned.
    ///
    /// When the connection is finished then this object will be flagged as
    /// being ready for a write, or otherwise in the writable state.
    ///
    /// # Errors
    ///
    /// This function will return a "would block" error if the pipe has not yet
    /// been registered with an event loop, if the connection operation has
    /// previously been issued but has not yet completed, or if the connect
    /// itself was issued and didn't finish immediately.
    ///
    /// Normal I/O errors from the call to `ConnectNamedPipe` are returned
    /// immediately.
    pub fn connect(&self) -> io::Result<()> {
        // "Acquire the connecting lock" or otherwise just make sure we're the
        // only operation that's using the `connect` overlapped instance.
        if self.inner.connecting.swap(true, SeqCst) {
            return Err(would_block());
        }

        // Now that we've flagged ourselves in the connecting state, issue the
        // connection attempt. Afterwards interpret the return value and set
        // internal state accordingly.
        let res = unsafe {
            let overlapped = self.inner.connect.as_ptr() as *mut _;
            self.inner.connect_overlapped(overlapped)
        };

        match res {
            // The connection operation finished immediately, so let's schedule
            // reads/writes and such.
            Ok(true) => {
                self.inner.connecting.store(false, SeqCst);
                Inner::post_register(&self.inner, None);
                Ok(())
            }

            // If the overlapped operation was successful and didn't finish
            // immediately then we forget a copy of the arc we hold
            // internally. This ensures that when the completion status comes
            // in for the I/O operation finishing it'll have a reference
            // associated with it and our data will still be valid. The
            // `connect_done` function will "reify" this forgotten pointer to
            // drop the refcount on the other side.
            Ok(false) => {
                mem::forget(self.inner.clone());
                Err(would_block())
            }

            Err(e) => {
                self.inner.connecting.store(false, SeqCst);
                Err(e)
            }
        }
    }

    /// Takes any internal error that has happened after the last I/O operation
    /// which hasn't been retrieved yet.
    ///
    /// This is particularly useful when detecting failed attempts to `connect`.
    /// After a completed `connect` flags this pipe as writable then callers
    /// must invoke this method to determine whether the connection actually
    /// succeeded. If this function returns `None` then a client is connected,
    /// otherwise it returns an error of what happened and a client shouldn't be
    /// connected.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        Ok(self.inner.io.lock().unwrap().connect_error.take())
    }

    /// Disconnects this named pipe from a connected client.
    ///
    /// This function will disconnect the pipe from a connected client, if any,
    /// transitively calling the `DisconnectNamedPipe` function.
    ///
    /// After a `disconnect` is issued, then a `connect` may be called again to
    /// connect to another client.
    pub fn disconnect(&self) -> io::Result<()> {
        self.inner.disconnect()
    }
}

impl FromRawHandle for NamedPipe {
    unsafe fn from_raw_handle(handle: RawHandle) -> NamedPipe {
        NamedPipe {
            inner: Arc::new(Inner {
                handle: Handle::new(handle as HANDLE),
                connect: Overlapped::new(connect_done),
                connecting: AtomicBool::new(false),
                read: Overlapped::new(read_done),
                write: Overlapped::new(write_done),
                event: Overlapped::new(event_done),
                io: Mutex::new(Io {
                    cp: None,
                    token: None,
                    read: State::None,
                    write: State::None,
                    connect_error: None,
                }),
                pool: Mutex::new(BufferPool::with_capacity(2)),
            }),
        }
    }
}

impl Read for NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        <&NamedPipe as Read>::read(&mut &*self, buf)
    }
}

impl Write for NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        <&NamedPipe as Write>::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        <&NamedPipe as Write>::flush(&mut &*self)
    }
}

impl<'a> Read for &'a NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut state = self.inner.io.lock().unwrap();

        if state.token.is_none() {
            return Err(would_block());
        }

        match mem::replace(&mut state.read, State::None) {
            // In theory not possible with `token` checked above,
            // but return would block for now.
            State::None => Err(would_block()),

            // A read is in flight, still waiting for it to finish
            State::Pending(buf, amt) => {
                state.read = State::Pending(buf, amt);
                Err(would_block())
            }

            // We previously read something into `data`, try to copy out some
            // data. If we copy out all the data schedule a new read and
            // otherwise store the buffer to get read later.
            State::Ok(data, cur) => {
                let n = {
                    let mut remaining = &data[cur..];
                    remaining.read(buf)?
                };
                let next = cur + n;
                if next != data.len() {
                    state.read = State::Ok(data, next);
                } else {
                    self.inner.put_buffer(data);
                    Inner::schedule_read(&self.inner, &mut state, None);
                }
                Ok(n)
            }

            // Looks like an in-flight read hit an error, return that here while
            // we schedule a new one.
            State::Err(e) => {
                Inner::schedule_read(&self.inner, &mut state, None);
                if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
                    Ok(0)
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl<'a> Write for &'a NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Make sure there's no writes pending
        let mut io = self.inner.io.lock().unwrap();

        if io.token.is_none() {
            return Err(would_block());
        }

        match io.write {
            State::None => {}
            State::Err(_) => match mem::replace(&mut io.write, State::None) {
                State::Err(e) => return Err(e),
                // `io` is locked, so this branch is unreachable
                _ => unreachable!(),
            },
            // any other state should be handled in `write_done`
            _ => {
                return Err(would_block());
            }
        }

        // Move `buf` onto the heap and fire off the write
        let mut owned_buf = self.inner.get_buffer();
        owned_buf.extend(buf);
        match Inner::maybe_schedule_write(&self.inner, owned_buf, 0, &mut io)? {
            // Some bytes are written immediately
            Some(n) => Ok(n),
            // Write operation is anqueued for whole buffer
            None => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Source for NamedPipe {
    fn register(&mut self, registry: &Registry, token: Token, _: Interest) -> io::Result<()> {
        let mut io = self.inner.io.lock().unwrap();

        io.check_association(registry, false)?;

        if io.token.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "I/O source already registered with a `Registry`",
            ));
        }

        if io.cp.is_none() {
            let selector = registry.selector();

            io.cp = Some(selector.clone_port());

            let inner_token = NEXT_TOKEN.fetch_add(2, Relaxed) + 2;
            selector.inner.cp.add_handle(inner_token, self)?;
        }

        io.token = Some(token);
        drop(io);

        Inner::post_register(&self.inner, None);

        Ok(())
    }

    fn reregister(&mut self, registry: &Registry, token: Token, _: Interest) -> io::Result<()> {
        let mut io = self.inner.io.lock().unwrap();

        io.check_association(registry, true)?;

        io.token = Some(token);
        drop(io);

        Inner::post_register(&self.inner, None);

        Ok(())
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        let mut io = self.inner.io.lock().unwrap();

        io.check_association(registry, true)?;

        if io.token.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "I/O source not registered with `Registry`",
            ));
        }

        io.token = None;
        Ok(())
    }
}

impl AsRawHandle for NamedPipe {
    fn as_raw_handle(&self) -> RawHandle {
        self.inner.handle.raw() as RawHandle
    }
}

impl fmt::Debug for NamedPipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.handle.fmt(f)
    }
}

impl Drop for NamedPipe {
    fn drop(&mut self) {
        // Cancel pending reads/connects, but don't cancel writes to ensure that
        // everything is flushed out.
        unsafe {
            if self.inner.connecting.load(SeqCst) {
                drop(cancel(&self.inner.handle, &self.inner.connect));
            }

            let io = self.inner.io.lock().unwrap();
            if let State::Pending(..) = io.read {
                drop(cancel(&self.inner.handle, &self.inner.read));
            }
        }
    }
}

impl Inner {
    /// Schedules a read to happen in the background, executing an overlapped
    /// operation.
    ///
    /// This function returns `true` if a normal error happens or if the read
    /// is scheduled in the background. If the pipe is no longer connected
    /// (ERROR_PIPE_LISTENING) then `false` is returned and no read is
    /// scheduled.
    fn schedule_read(me: &Arc<Inner>, io: &mut Io, events: Option<&mut Vec<Event>>) -> bool {
        // Check to see if a read is already scheduled/completed
        match io.read {
            State::None => {}
            _ => return true,
        }

        // Allocate a buffer and schedule the read.
        let mut buf = me.get_buffer();
        let e = unsafe {
            let overlapped = me.read.as_ptr() as *mut _;
            let slice = slice::from_raw_parts_mut(buf.as_mut_ptr(), buf.capacity());
            me.read_overlapped(slice, overlapped)
        };

        match e {
            // See `NamedPipe::connect` above for the rationale behind `forget`
            Ok(_) => {
                io.read = State::Pending(buf, 0); // 0 is ignored on read side
                mem::forget(me.clone());
                true
            }

            // If ERROR_PIPE_LISTENING happens then it's not a real read error,
            // we just need to wait for a connect.
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_LISTENING as i32) => false,

            // If some other error happened, though, we're now readable to give
            // out the error.
            Err(e) => {
                io.read = State::Err(e);
                io.notify_readable(me, events);
                true
            }
        }
    }

    /// Maybe schedules overlapped write operation.
    ///
    /// * `None` means that overlapped operation was enqueued
    /// * `Some(n)` means that `n` bytes was immediately written.
    ///   Note, that `write_done` will fire anyway to clean up the state.
    fn maybe_schedule_write(
        me: &Arc<Inner>,
        buf: Vec<u8>,
        pos: usize,
        io: &mut Io,
    ) -> io::Result<Option<usize>> {
        // Very similar to `schedule_read` above, just done for the write half.
        let e = unsafe {
            let overlapped = me.write.as_ptr() as *mut _;
            me.write_overlapped(&buf[pos..], overlapped)
        };

        // See `connect` above for the rationale behind `forget`
        match e {
            // `n` bytes are written immediately
            Ok(Some(n)) => {
                io.write = State::Ok(buf, pos);
                mem::forget(me.clone());
                Ok(Some(n))
            }
            // write operation is enqueued
            Ok(None) => {
                io.write = State::Pending(buf, pos);
                mem::forget(me.clone());
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn schedule_write(
        me: &Arc<Inner>,
        buf: Vec<u8>,
        pos: usize,
        io: &mut Io,
        events: Option<&mut Vec<Event>>,
    ) {
        match Inner::maybe_schedule_write(me, buf, pos, io) {
            Ok(Some(_)) => {
                // immediate result will be handled in `write_done`,
                // so we'll reinterpret the `Ok` state
                let state = mem::replace(&mut io.write, State::None);
                io.write = match state {
                    State::Ok(buf, pos) => State::Pending(buf, pos),
                    // io is locked, so this branch is unreachable
                    _ => unreachable!(),
                };
                mem::forget(me.clone());
            }
            Ok(None) => (),
            Err(e) => {
                io.write = State::Err(e);
                io.notify_writable(me, events);
            }
        }
    }

    fn post_register(me: &Arc<Inner>, mut events: Option<&mut Vec<Event>>) {
        let mut io = me.io.lock().unwrap();
        #[allow(clippy::needless_option_as_deref)]
        if Inner::schedule_read(me, &mut io, events.as_deref_mut()) {
            if let State::None = io.write {
                io.notify_writable(me, events);
            }
        }
    }

    fn get_buffer(&self) -> Vec<u8> {
        self.pool.lock().unwrap().get(4 * 1024)
    }

    fn put_buffer(&self, buf: Vec<u8>) {
        self.pool.lock().unwrap().put(buf)
    }
}

unsafe fn cancel(handle: &Handle, overlapped: &Overlapped) -> io::Result<()> {
    let ret = CancelIoEx(handle.raw(), overlapped.as_ptr());
    // `CancelIoEx` returns 0 on error:
    // https://docs.microsoft.com/en-us/windows/win32/fileio/cancelioex-func
    if ret == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn connect_done(status: &OVERLAPPED_ENTRY, events: Option<&mut Vec<Event>>) {
    let status = CompletionStatus::from_entry(status);

    // Acquire the `Arc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `connect` above.
    let me = unsafe { Arc::from_raw(Inner::ptr_from_conn_overlapped(status.overlapped())) };

    // Flag ourselves as no longer using the `connect` overlapped instances.
    let prev = me.connecting.swap(false, SeqCst);
    assert!(prev, "NamedPipe was not previously connecting");

    // Stash away our connect error if one happened
    debug_assert_eq!(status.bytes_transferred(), 0);
    unsafe {
        match me.result(status.overlapped()) {
            Ok(n) => debug_assert_eq!(n, 0),
            Err(e) => me.io.lock().unwrap().connect_error = Some(e),
        }
    }

    // We essentially just finished a registration, so kick off a
    // read and register write readiness.
    Inner::post_register(&me, events);
}

fn read_done(status: &OVERLAPPED_ENTRY, events: Option<&mut Vec<Event>>) {
    let status = CompletionStatus::from_entry(status);

    // Acquire the `FromRawArc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `schedule_read` above.
    let me = unsafe { Arc::from_raw(Inner::ptr_from_read_overlapped(status.overlapped())) };

    // Move from the `Pending` to `Ok` state.
    let mut io = me.io.lock().unwrap();
    let mut buf = match mem::replace(&mut io.read, State::None) {
        State::Pending(buf, _) => buf,
        _ => unreachable!(),
    };
    unsafe {
        match me.result(status.overlapped()) {
            Ok(n) => {
                debug_assert_eq!(status.bytes_transferred() as usize, n);
                buf.set_len(status.bytes_transferred() as usize);
                io.read = State::Ok(buf, 0);
            }
            Err(e) => {
                debug_assert_eq!(status.bytes_transferred(), 0);
                io.read = State::Err(e);
            }
        }
    }

    // Flag our readiness that we've got data.
    io.notify_readable(&me, events);
}

fn write_done(status: &OVERLAPPED_ENTRY, events: Option<&mut Vec<Event>>) {
    let status = CompletionStatus::from_entry(status);

    // Acquire the `Arc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `schedule_write` above.
    let me = unsafe { Arc::from_raw(Inner::ptr_from_write_overlapped(status.overlapped())) };

    // Make the state change out of `Pending`. If we wrote the entire buffer
    // then we're writable again and otherwise we schedule another write.
    let mut io = me.io.lock().unwrap();
    let (buf, pos) = match mem::replace(&mut io.write, State::None) {
        // `Ok` here means, that the operation was completed immediately
        // `bytes_transferred` is already reported to a client
        State::Ok(..) => {
            io.notify_writable(&me, events);
            return;
        }
        State::Pending(buf, pos) => (buf, pos),
        _ => unreachable!(),
    };

    unsafe {
        match me.result(status.overlapped()) {
            Ok(n) => {
                debug_assert_eq!(status.bytes_transferred() as usize, n);
                let new_pos = pos + (status.bytes_transferred() as usize);
                if new_pos == buf.len() {
                    me.put_buffer(buf);
                    io.notify_writable(&me, events);
                } else {
                    Inner::schedule_write(&me, buf, new_pos, &mut io, events);
                }
            }
            Err(e) => {
                debug_assert_eq!(status.bytes_transferred(), 0);
                io.write = State::Err(e);
                io.notify_writable(&me, events);
            }
        }
    }
}

fn event_done(status: &OVERLAPPED_ENTRY, events: Option<&mut Vec<Event>>) {
    let status = CompletionStatus::from_entry(status);

    // Acquire the `Arc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `schedule_write` above.
    let me = unsafe { Arc::from_raw(Inner::ptr_from_event_overlapped(status.overlapped())) };

    let io = me.io.lock().unwrap();

    // Make sure the I/O handle is still registered with the selector
    if io.token.is_some() {
        // This method is also called during `Selector::drop` to perform
        // cleanup. In this case, `events` is `None` and we don't need to track
        // the event.
        if let Some(events) = events {
            let mut ev = Event::from_completion_status(&status);
            // Reverse the `.data` alteration done in `schedule_event`. This
            // alteration was done so the selector recognized the event as one from
            // a named pipe.
            ev.data >>= 1;
            events.push(ev);
        }
    }
}

impl Io {
    fn check_association(&self, registry: &Registry, required: bool) -> io::Result<()> {
        match self.cp {
            Some(ref cp) if !registry.selector().same_port(cp) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "I/O source already registered with a different `Registry`",
            )),
            None if required => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "I/O source not registered with `Registry`",
            )),
            _ => Ok(()),
        }
    }

    fn notify_readable(&self, me: &Arc<Inner>, events: Option<&mut Vec<Event>>) {
        if let Some(token) = self.token {
            let mut ev = Event::new(token);
            ev.set_readable();

            if let Some(events) = events {
                events.push(ev);
            } else {
                self.schedule_event(me, ev);
            }
        }
    }

    fn notify_writable(&self, me: &Arc<Inner>, events: Option<&mut Vec<Event>>) {
        if let Some(token) = self.token {
            let mut ev = Event::new(token);
            ev.set_writable();

            if let Some(events) = events {
                events.push(ev);
            } else {
                self.schedule_event(me, ev);
            }
        }
    }

    fn schedule_event(&self, me: &Arc<Inner>, mut event: Event) {
        // Alter the token so that the selector will identify the IOCP event as
        // one for a named pipe. This will be reversed in `event_done`
        //
        // `data` for named pipes is an auto-incrementing counter. Because
        // `data` is `u64` we do not risk losing the most-significant bit
        // (unless a user creates 2^62 named pipes during the lifetime of the
        // process).
        event.data <<= 1;
        event.data += 1;

        let completion_status =
            event.to_completion_status_with_overlapped(me.event.as_ptr() as *mut _);

        match self.cp.as_ref().unwrap().post(completion_status) {
            Ok(_) => {
                // Increase the ref count of `Inner` for the completion event.
                mem::forget(me.clone());
            }
            Err(_) => {
                // Nothing to do here
            }
        }
    }
}

struct BufferPool {
    pool: Vec<Vec<u8>>,
}

impl BufferPool {
    fn with_capacity(cap: usize) -> BufferPool {
        BufferPool {
            pool: Vec::with_capacity(cap),
        }
    }

    fn get(&mut self, default_cap: usize) -> Vec<u8> {
        self.pool
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(default_cap))
    }

    fn put(&mut self, mut buf: Vec<u8>) {
        if self.pool.len() < self.pool.capacity() {
            unsafe {
                buf.set_len(0);
            }
            self.pool.push(buf);
        }
    }
}
