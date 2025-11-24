//! A safe wrapper around the Windows I/O API.

use super::dur2timeout;

use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::SetFileCompletionNotificationModes;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::WindowsProgramming::FILE_SKIP_SET_EVENT_ON_HANDLE;
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatusEx, PostQueuedCompletionStatus, OVERLAPPED,
    OVERLAPPED_ENTRY,
};

/// A completion block which can be used with I/O completion ports.
///
/// # Safety
///
/// This must be a valid completion block.
pub(super) unsafe trait Completion {
    /// Signal to the completion block that we are about to start an operation.
    fn try_lock(self: Pin<&Self>) -> bool;

    /// Unlock the completion block.
    unsafe fn unlock(self: Pin<&Self>);
}

/// The pointer to a completion block.
///
/// # Safety
///
/// This must be a valid completion block.
pub(super) unsafe trait CompletionHandle: Deref + Sized {
    /// Type of the completion block.
    type Completion: Completion;

    /// Get a pointer to the completion block.
    ///
    /// The pointer is pinned since the underlying object should not be moved
    /// after creation. This prevents it from being invalidated while it's
    /// used in an overlapped operation.
    fn get(&self) -> Pin<&Self::Completion>;

    /// Convert this block into a pointer that can be passed as `*mut OVERLAPPED`.
    fn into_ptr(this: Self) -> *mut OVERLAPPED;

    /// Convert a pointer that was passed as `*mut OVERLAPPED` into a pointer to this block.
    ///
    /// # Safety
    ///
    /// This must be a valid pointer to a completion block.
    unsafe fn from_ptr(ptr: *mut OVERLAPPED) -> Self;

    /// Convert to a pointer without losing ownership.
    fn as_ptr(&self) -> *mut OVERLAPPED;
}

unsafe impl<T: Completion> CompletionHandle for Pin<&T> {
    type Completion = T;

    fn get(&self) -> Pin<&Self::Completion> {
        *self
    }

    fn into_ptr(this: Self) -> *mut OVERLAPPED {
        unsafe { Pin::into_inner_unchecked(this) as *const T as *mut OVERLAPPED }
    }

    unsafe fn from_ptr(ptr: *mut OVERLAPPED) -> Self {
        Pin::new_unchecked(&*(ptr as *const T))
    }

    fn as_ptr(&self) -> *mut OVERLAPPED {
        self.get_ref() as *const T as *mut OVERLAPPED
    }
}

unsafe impl<T: Completion> CompletionHandle for Pin<Arc<T>> {
    type Completion = T;

    fn get(&self) -> Pin<&Self::Completion> {
        self.as_ref()
    }

    fn into_ptr(this: Self) -> *mut OVERLAPPED {
        unsafe { Arc::into_raw(Pin::into_inner_unchecked(this)) as *const T as *mut OVERLAPPED }
    }

    unsafe fn from_ptr(ptr: *mut OVERLAPPED) -> Self {
        Pin::new_unchecked(Arc::from_raw(ptr as *const T))
    }

    fn as_ptr(&self) -> *mut OVERLAPPED {
        self.as_ref().get_ref() as *const T as *mut OVERLAPPED
    }
}

/// A handle to the I/O completion port.
pub(super) struct IoCompletionPort<T> {
    /// The underlying handle.
    handle: HANDLE,

    /// We own the status block.
    _marker: PhantomData<T>,
}

impl<T> Drop for IoCompletionPort<T> {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

impl<T> AsRawHandle for IoCompletionPort<T> {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle as _
    }
}

impl<T> fmt::Debug for IoCompletionPort<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct WriteAsHex(HANDLE);

        impl fmt::Debug for WriteAsHex {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:010x}", self.0 as usize)
            }
        }

        f.debug_struct("IoCompletionPort")
            .field("handle", &WriteAsHex(self.handle))
            .finish()
    }
}

impl<T: CompletionHandle> IoCompletionPort<T> {
    /// Create a new I/O completion port.
    pub(super) fn new(threads: usize) -> io::Result<Self> {
        let handle = unsafe {
            CreateIoCompletionPort(
                INVALID_HANDLE_VALUE,
                ptr::null_mut(),
                0,
                threads.try_into().expect("too many threads"),
            )
        };

        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self {
                handle,
                _marker: PhantomData,
            })
        }
    }

    /// Register a handle with this I/O completion port.
    pub(super) fn register(
        &self,
        handle: &impl AsRawHandle, // TODO change to AsHandle
        skip_set_event_on_handle: bool,
    ) -> io::Result<()> {
        let handle = handle.as_raw_handle();

        let result =
            unsafe { CreateIoCompletionPort(handle as _, self.handle, handle as usize, 0) };

        if result.is_null() {
            return Err(io::Error::last_os_error());
        }

        if skip_set_event_on_handle {
            // Set the skip event on handle.
            let result = unsafe {
                SetFileCompletionNotificationModes(handle as _, FILE_SKIP_SET_EVENT_ON_HANDLE as _)
            };

            if result == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    /// Post a completion packet to this port.
    pub(super) fn post(&self, bytes_transferred: usize, id: usize, packet: T) -> io::Result<()> {
        let result = unsafe {
            PostQueuedCompletionStatus(
                self.handle,
                bytes_transferred
                    .try_into()
                    .expect("too many bytes transferred"),
                id,
                T::into_ptr(packet),
            )
        };

        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Wait for completion packets to arrive.
    pub(super) fn wait(
        &self,
        packets: &mut Vec<OverlappedEntry<T>>,
        timeout: Option<Duration>,
    ) -> io::Result<usize> {
        // Drop the current packets.
        packets.clear();

        let mut count = MaybeUninit::<u32>::uninit();
        let timeout = timeout.map_or(INFINITE, dur2timeout);

        let result = unsafe {
            GetQueuedCompletionStatusEx(
                self.handle,
                packets.as_mut_ptr() as _,
                packets.capacity().try_into().expect("too many packets"),
                count.as_mut_ptr(),
                timeout,
                0,
            )
        };

        if result == 0 {
            let io_error = io::Error::last_os_error();
            if io_error.kind() == io::ErrorKind::TimedOut {
                Ok(0)
            } else {
                Err(io_error)
            }
        } else {
            let count = unsafe { count.assume_init() };
            unsafe {
                packets.set_len(count as _);
            }
            Ok(count as _)
        }
    }
}

/// An `OVERLAPPED_ENTRY` resulting from an I/O completion port.
#[repr(transparent)]
pub(super) struct OverlappedEntry<T: CompletionHandle> {
    /// The underlying entry.
    entry: OVERLAPPED_ENTRY,

    /// We own the status block.
    _marker: PhantomData<T>,
}

impl<T: CompletionHandle> fmt::Debug for OverlappedEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OverlappedEntry { .. }")
    }
}

impl<T: CompletionHandle> OverlappedEntry<T> {
    /// Convert into the completion packet.
    pub(super) fn into_packet(self) -> T {
        let packet = unsafe { self.packet() };
        std::mem::forget(self);
        packet
    }

    /// Get the packet reference that this entry refers to.
    ///
    /// # Safety
    ///
    /// This function should only be called once, since it moves
    /// out the `T` from the `OVERLAPPED_ENTRY`.
    unsafe fn packet(&self) -> T {
        let packet = T::from_ptr(self.entry.lpOverlapped);
        packet.get().unlock();
        packet
    }
}

impl<T: CompletionHandle> Drop for OverlappedEntry<T> {
    fn drop(&mut self) {
        drop(unsafe { self.packet() });
    }
}
