use super::mystd::fs::File;
use super::mystd::os::unix::prelude::*;
use core::ops::Deref;
use core::ptr;
use core::slice;

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use libc::mmap as mmap64;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
use libc::mmap64;

pub struct Mmap {
    ptr: *mut libc::c_void,
    len: usize,
}

impl Mmap {
    pub unsafe fn map(file: &File, len: usize, offset: u64) -> Option<Mmap> {
        let ptr = unsafe {
            mmap64(
                ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                offset.try_into().ok()?,
            )
        };
        if ptr == libc::MAP_FAILED {
            return None;
        }
        Some(Mmap { ptr, len })
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            let r = libc::munmap(self.ptr, self.len);
            debug_assert_eq!(r, 0);
        }
    }
}
