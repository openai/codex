use super::mystd::env;
use super::mystd::ffi::OsStr;
use super::mystd::os::unix::prelude::*;
use super::xcoff;
use super::{Library, LibrarySegment};
use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::{c_int, CStr};
use core::mem;

const EXE_IMAGE_BASE: u64 = 0x100000000;

unsafe extern "C" {
    #[link_name = "_Errno"]
    fn errno_location() -> *mut c_int;
}

fn errno() -> i32 {
    unsafe { (*errno_location()) as i32 }
}

/// On AIX, we use `loadquery` with `L_GETINFO` flag to query libraries mmapped.
/// See https://www.ibm.com/docs/en/aix/7.2?topic=l-loadquery-subroutine for
/// detailed information of `loadquery`.
pub(super) fn native_libraries() -> Vec<Library> {
    let mut ret = Vec::new();
    unsafe {
        let mut buffer = vec![mem::zeroed::<libc::ld_info>(); 64];
        loop {
            if libc::loadquery(
                libc::L_GETINFO,
                buffer.as_mut_ptr().cast::<libc::c_char>(),
                (mem::size_of::<libc::ld_info>() * buffer.len()) as u32,
            ) != -1
            {
                break;
            } else {
                match errno() {
                    libc::ENOMEM => {
                        buffer.resize(buffer.len() * 2, mem::zeroed::<libc::ld_info>());
                    }
                    _ => {
                        // If other error occurs, return empty libraries.
                        return Vec::new();
                    }
                }
            }
        }
        let mut current = buffer.as_mut_ptr();
        loop {
            let text_base = (*current).ldinfo_textorg as usize;
            let filename_ptr: *const libc::c_char = &(*current).ldinfo_filename[0];
            let bytes = CStr::from_ptr(filename_ptr).to_bytes();
            let member_name_ptr = filename_ptr.offset((bytes.len() + 1) as isize);
            let mut filename = OsStr::from_bytes(bytes).to_owned();
            if text_base == EXE_IMAGE_BASE as usize {
                if let Ok(exe) = env::current_exe() {
                    filename = exe.into_os_string();
                }
            }
            let bytes = CStr::from_ptr(member_name_ptr).to_bytes();
            let member_name = OsStr::from_bytes(bytes).to_owned();
            if let Some(image) = xcoff::parse_image(filename.as_ref(), &member_name) {
                ret.push(Library {
                    name: filename,
                    member_name,
                    segments: vec![LibrarySegment {
                        stated_virtual_memory_address: image.base as usize,
                        len: image.size,
                    }],
                    bias: (text_base + image.offset).wrapping_sub(image.base as usize),
                });
            }
            if (*current).ldinfo_next == 0 {
                break;
            }
            current = current
                .cast::<libc::c_char>()
                .offset((*current).ldinfo_next as isize)
                .cast::<libc::ld_info>();
        }
    }
    return ret;
}
