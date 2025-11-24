//! Bindings to IOCP, I/O Completion Ports

use super::{Handle, Overlapped};
use std::cmp;
use std::fmt;
use std::io;
use std::mem;
use std::os::windows::io::*;
use std::ptr::null_mut;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatusEx, PostQueuedCompletionStatus, OVERLAPPED,
    OVERLAPPED_ENTRY,
};

/// A handle to an Windows I/O Completion Port.
#[derive(Debug)]
pub(crate) struct CompletionPort {
    handle: Handle,
}

// SAFETY: `Handles`s are, in general, not thread-safe. However, we only used `Handle`s for
// resources that are thread-safe in `CompletionPort`.
unsafe impl Send for CompletionPort {}

// SAFETY: `Handles`s are, in general, not thread-safe. However, we only used `Handle`s for
// resources that are thread-safe in `CompletionPort`.
unsafe impl Sync for CompletionPort {}

/// A status message received from an I/O completion port.
///
/// These statuses can be created via the `new` or `empty` constructors and then
/// provided to a completion port, or they are read out of a completion port.
/// The fields of each status are read through its accessor methods.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CompletionStatus(OVERLAPPED_ENTRY);

impl fmt::Debug for CompletionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompletionStatus(OVERLAPPED_ENTRY)")
    }
}

unsafe impl Send for CompletionStatus {}
unsafe impl Sync for CompletionStatus {}

impl CompletionPort {
    /// Creates a new I/O completion port with the specified concurrency value.
    ///
    /// The number of threads given corresponds to the level of concurrency
    /// allowed for threads associated with this port. Consult the Windows
    /// documentation for more information about this value.
    pub fn new(threads: u32) -> io::Result<CompletionPort> {
        let ret = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, null_mut(), 0, threads) };
        if ret.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(CompletionPort {
                handle: Handle::new(ret),
            })
        }
    }

    /// Associates a new `HANDLE` to this I/O completion port.
    ///
    /// This function will associate the given handle to this port with the
    /// given `token` to be returned in status messages whenever it receives a
    /// notification.
    ///
    /// Any object which is convertible to a `HANDLE` via the `AsRawHandle`
    /// trait can be provided to this function, such as `std::fs::File` and
    /// friends.
    #[cfg(any(feature = "net", feature = "os-ext"))]
    pub fn add_handle<T: AsRawHandle + ?Sized>(&self, token: usize, t: &T) -> io::Result<()> {
        let ret = unsafe {
            CreateIoCompletionPort(t.as_raw_handle() as HANDLE, self.handle.raw(), token, 0)
        };
        if ret.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Dequeues a number of completion statuses from this I/O completion port.
    ///
    /// This function is the same as `get` except that it may return more than
    /// one status. A buffer of "zero" statuses is provided (the contents are
    /// not read) and then on success this function will return a sub-slice of
    /// statuses which represent those which were dequeued from this port. This
    /// function does not wait to fill up the entire list of statuses provided.
    ///
    /// Like with `get`, a timeout may be specified for this operation.
    pub fn get_many<'a>(
        &self,
        list: &'a mut [CompletionStatus],
        timeout: Option<Duration>,
    ) -> io::Result<&'a mut [CompletionStatus]> {
        debug_assert_eq!(
            mem::size_of::<CompletionStatus>(),
            mem::size_of::<OVERLAPPED_ENTRY>()
        );
        let mut removed = 0;
        let timeout = duration_millis(timeout);
        let len = cmp::min(list.len(), u32::MAX as usize) as u32;
        let ret = unsafe {
            GetQueuedCompletionStatusEx(
                self.handle.raw(),
                list.as_ptr() as *mut _,
                len,
                &mut removed,
                timeout,
                0,
            )
        };

        if ret == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(&mut list[..removed as usize])
        }
    }

    /// Posts a new completion status onto this I/O completion port.
    ///
    /// This function will post the given status, with custom parameters, to the
    /// port. Threads blocked in `get` or `get_many` will eventually receive
    /// this status.
    pub fn post(&self, status: CompletionStatus) -> io::Result<()> {
        let ret = unsafe {
            PostQueuedCompletionStatus(
                self.handle.raw(),
                status.0.dwNumberOfBytesTransferred,
                status.0.lpCompletionKey,
                status.0.lpOverlapped,
            )
        };

        if ret == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl AsRawHandle for CompletionPort {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.raw() as RawHandle
    }
}

impl FromRawHandle for CompletionPort {
    unsafe fn from_raw_handle(handle: RawHandle) -> CompletionPort {
        CompletionPort {
            handle: Handle::new(handle as HANDLE),
        }
    }
}

impl IntoRawHandle for CompletionPort {
    fn into_raw_handle(self) -> RawHandle {
        self.handle.into_raw()
    }
}

impl CompletionStatus {
    /// Creates a new completion status with the provided parameters.
    ///
    /// This function is useful when creating a status to send to a port with
    /// the `post` method. The parameters are opaquely passed through and not
    /// interpreted by the system at all.
    pub(crate) fn new(bytes: u32, token: usize, overlapped: *mut Overlapped) -> Self {
        CompletionStatus(OVERLAPPED_ENTRY {
            dwNumberOfBytesTransferred: bytes,
            lpCompletionKey: token,
            lpOverlapped: overlapped as *mut _,
            Internal: 0,
        })
    }

    /// Creates a new borrowed completion status from the borrowed
    /// `OVERLAPPED_ENTRY` argument provided.
    ///
    /// This method will wrap the `OVERLAPPED_ENTRY` in a `CompletionStatus`,
    /// returning the wrapped structure.
    #[cfg(feature = "os-ext")]
    pub fn from_entry(entry: &OVERLAPPED_ENTRY) -> &Self {
        // Safety: CompletionStatus is repr(transparent) w/ OVERLAPPED_ENTRY, so
        // a reference to one is guaranteed to be layout compatible with the
        // reference to another.
        unsafe { &*(entry as *const _ as *const _) }
    }

    /// Creates a new "zero" completion status.
    ///
    /// This function is useful when creating a stack buffer or vector of
    /// completion statuses to be passed to the `get_many` function.
    pub fn zero() -> Self {
        Self::new(0, 0, null_mut())
    }

    /// Returns the number of bytes that were transferred for the I/O operation
    /// associated with this completion status.
    pub fn bytes_transferred(&self) -> u32 {
        self.0.dwNumberOfBytesTransferred
    }

    /// Returns the completion key value associated with the file handle whose
    /// I/O operation has completed.
    ///
    /// A completion key is a per-handle key that is specified when it is added
    /// to an I/O completion port via `add_handle` or `add_socket`.
    pub fn token(&self) -> usize {
        self.0.lpCompletionKey as usize
    }

    /// Returns a pointer to the `Overlapped` structure that was specified when
    /// the I/O operation was started.
    pub fn overlapped(&self) -> *mut OVERLAPPED {
        self.0.lpOverlapped
    }

    /// Returns a pointer to the internal `OVERLAPPED_ENTRY` object.
    pub fn entry(&self) -> &OVERLAPPED_ENTRY {
        &self.0
    }
}

#[inline]
fn duration_millis(dur: Option<Duration>) -> u32 {
    if let Some(dur) = dur {
        // `Duration::as_millis` truncates, so round up. This avoids
        // turning sub-millisecond timeouts into a zero timeout, unless
        // the caller explicitly requests that by specifying a zero
        // timeout.
        let dur_ms = dur
            .checked_add(Duration::from_nanos(999_999))
            .unwrap_or(dur)
            .as_millis();
        cmp::min(dur_ms, u32::MAX as u128) as u32
    } else {
        u32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionPort, CompletionStatus};

    #[test]
    fn is_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<CompletionPort>();
    }

    #[test]
    fn get_many() {
        let c = CompletionPort::new(1).unwrap();

        c.post(CompletionStatus::new(1, 2, 3 as *mut _)).unwrap();
        c.post(CompletionStatus::new(4, 5, 6 as *mut _)).unwrap();

        let mut s = vec![CompletionStatus::zero(); 4];
        {
            let s = c.get_many(&mut s, None).unwrap();
            assert_eq!(s.len(), 2);
            assert_eq!(s[0].bytes_transferred(), 1);
            assert_eq!(s[0].token(), 2);
            assert_eq!(s[0].overlapped(), 3 as *mut _);
            assert_eq!(s[1].bytes_transferred(), 4);
            assert_eq!(s[1].token(), 5);
            assert_eq!(s[1].overlapped(), 6 as *mut _);
        }
        assert_eq!(s[2].bytes_transferred(), 0);
        assert_eq!(s[2].token(), 0);
        assert_eq!(s[2].overlapped(), 0 as *mut _);
    }
}
