//! Low-level Windows thread attribute helpers used by ConPTY spawn.
//!
//! This module wraps the Win32 `PROC_THREAD_ATTRIBUTE_LIST` APIs so ConPTY handles can
//! be attached to a child process. It is ConPTY‑specific and used in both legacy and
//! elevated unified_exec paths when spawning a PTY‑backed process.

use std::io;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::DeleteProcThreadAttributeList;
use windows_sys::Win32::System::Threading::InitializeProcThreadAttributeList;
use windows_sys::Win32::System::Threading::UpdateProcThreadAttribute;
use windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST;

const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;

/// RAII wrapper for Windows PROC_THREAD_ATTRIBUTE_LIST.
pub struct ProcThreadAttributeList {
    buffer: Vec<u8>,
}

impl ProcThreadAttributeList {
    /// Allocate and initialize a thread attribute list.
    pub fn new(attr_count: u32) -> io::Result<Self> {
        let mut size: usize = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), attr_count, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        let mut buffer = vec![0u8; size];
        let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        let ok = unsafe { InitializeProcThreadAttributeList(list, attr_count, 0, &mut size) };
        if ok == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        Ok(Self { buffer })
    }

    /// Return a mutable pointer to the attribute list for Win32 APIs.
    pub fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST
    }

    /// Attach a ConPTY handle to the attribute list.
    pub fn set_pseudoconsole(&mut self, hpc: isize) -> io::Result<()> {
        let list = self.as_mut_ptr();
        let mut hpc_value = hpc;
        let ok = unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                (&mut hpc_value as *mut isize).cast(),
                std::mem::size_of::<isize>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        Ok(())
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.as_mut_ptr());
        }
    }
}
