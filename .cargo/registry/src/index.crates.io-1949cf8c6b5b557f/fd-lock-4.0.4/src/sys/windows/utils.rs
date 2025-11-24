use std::io;
use std::mem;

use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::IO::OVERLAPPED;

/// A wrapper around `OVERLAPPED` to provide "rustic" accessors and
/// initializers.
pub(crate) struct Overlapped(OVERLAPPED);

impl Overlapped {
    /// Creates a new zeroed out instance of an overlapped I/O tracking state.
    ///
    /// This is suitable for passing to methods which will then later get
    /// notified via an I/O Completion Port.
    pub(crate) fn zero() -> Overlapped {
        Overlapped(unsafe { mem::zeroed() })
    }

    /// Gain access to the raw underlying data
    pub(crate) fn raw(&self) -> *mut OVERLAPPED {
        &self.0 as *const _ as *mut _
    }
}

/// Convert a system call which returns a `BOOL` to an `io::Result`.
pub(crate) fn syscall(status: BOOL) -> std::io::Result<()> {
    if status == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
