use std::os::windows::io::RawHandle;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

/// Wrapper around a Windows HANDLE so that we close it upon drop in all scenarios
#[derive(Debug)]
pub struct Handle(HANDLE);

impl Handle {
    #[inline]
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    pub fn raw(&self) -> HANDLE {
        self.0
    }

    pub fn into_raw(self) -> RawHandle {
        let ret = self.0;
        // This is super important so that drop is not called!
        std::mem::forget(self);
        ret as RawHandle
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
