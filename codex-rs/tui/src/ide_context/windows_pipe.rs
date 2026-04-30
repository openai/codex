//! Windows named-pipe transport for the IDE context IPC client.

use std::io;
use std::io::Read;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::ERROR_IO_PENDING;
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::WAIT_FAILED;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::Storage::FileSystem::WriteFile;
use windows_sys::Win32::System::IO::CancelIoEx;
use windows_sys::Win32::System::IO::GetOverlappedResult;
use windows_sys::Win32::System::IO::OVERLAPPED;
use windows_sys::Win32::System::Threading::CreateEventW;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const TRUE: BOOL = 1;
const FALSE: BOOL = 0;
const NULL_HANDLE: HANDLE = 0;

pub(super) struct WindowsPipeStream {
    handle: OwnedHandle,
    deadline: Instant,
}

impl WindowsPipeStream {
    pub(super) fn connect(pipe_path: PathBuf, deadline: Instant) -> io::Result<Self> {
        let wide_path = pipe_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
                NULL_HANDLE,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        Ok(Self {
            handle: OwnedHandle(handle),
            deadline,
        })
    }
}

impl Read for WindowsPipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let bytes_to_read = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut operation = OverlappedOperation::new()?;
        let result = unsafe {
            ReadFile(
                self.handle.raw(),
                buf.as_mut_ptr(),
                bytes_to_read,
                ptr::null_mut(),
                operation.as_mut_ptr(),
            )
        };

        operation.complete(self.handle.raw(), result, self.deadline)
    }
}

impl Write for WindowsPipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let bytes_to_write = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut operation = OverlappedOperation::new()?;
        let result = unsafe {
            WriteFile(
                self.handle.raw(),
                buf.as_ptr(),
                bytes_to_write,
                ptr::null_mut(),
                operation.as_mut_ptr(),
            )
        };

        operation.complete(self.handle.raw(), result, self.deadline)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct OverlappedOperation {
    event: OwnedHandle,
    overlapped: OVERLAPPED,
}

impl OverlappedOperation {
    fn new() -> io::Result<Self> {
        let event = unsafe { CreateEventW(ptr::null(), TRUE, FALSE, ptr::null()) };
        if event == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut overlapped = unsafe { std::mem::zeroed::<OVERLAPPED>() };
        overlapped.hEvent = event;
        Ok(Self {
            event: OwnedHandle(event),
            overlapped,
        })
    }

    fn as_mut_ptr(&mut self) -> *mut OVERLAPPED {
        &mut self.overlapped
    }

    fn complete(
        &mut self,
        handle: HANDLE,
        initial_result: BOOL,
        deadline: Instant,
    ) -> io::Result<usize> {
        if initial_result == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(error);
            }

            match unsafe { WaitForSingleObject(self.event.raw(), remaining_timeout_ms(deadline)?) }
            {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => return Err(self.cancel_and_timeout(handle)),
                WAIT_FAILED => return Err(io::Error::last_os_error()),
                other => {
                    return Err(io::Error::other(format!(
                        "unexpected WaitForSingleObject result: {other}"
                    )));
                }
            }
        }

        let mut bytes_transferred = 0;
        let result = unsafe {
            GetOverlappedResult(handle, self.as_mut_ptr(), &mut bytes_transferred, FALSE)
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(bytes_transferred as usize)
    }

    fn cancel_and_timeout(&mut self, handle: HANDLE) -> io::Error {
        unsafe {
            CancelIoEx(handle, self.as_mut_ptr());
        }
        let mut bytes_transferred = 0;
        unsafe {
            GetOverlappedResult(handle, self.as_mut_ptr(), &mut bytes_transferred, TRUE);
        }
        timeout_io_error()
    }
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

fn remaining_timeout_ms(deadline: Instant) -> io::Result<u32> {
    let now = Instant::now();
    if now >= deadline {
        return Err(timeout_io_error());
    }

    let millis = deadline.duration_since(now).as_millis().max(1);
    Ok(u32::try_from(millis).unwrap_or(u32::MAX))
}

fn timeout_io_error() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "timed out waiting for IDE context")
}
