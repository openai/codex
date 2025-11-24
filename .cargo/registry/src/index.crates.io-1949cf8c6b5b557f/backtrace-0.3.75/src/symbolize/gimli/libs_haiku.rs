// Haiku implements the image_info struct and the get_next_image_info()
// functions to iterate through the loaded executable images. The
// image_info struct contains a pointer to the start of the .text
// section within the virtual address space, as well as the size of
// that section. All the read-only segments of the ELF-binary are in
// that part of the address space.

use super::mystd::ffi::OsStr;
use super::mystd::os::unix::prelude::*;
use super::{Library, LibrarySegment};
use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use core::ffi::CStr;
use core::mem::MaybeUninit;

pub(super) fn native_libraries() -> Vec<Library> {
    let mut libraries: Vec<Library> = Vec::new();

    unsafe {
        let mut info = MaybeUninit::<libc::image_info>::zeroed();
        let mut cookie: i32 = 0;
        // Load the first image to get a valid info struct
        let mut status =
            libc::get_next_image_info(libc::B_CURRENT_TEAM, &mut cookie, info.as_mut_ptr());
        if status != libc::B_OK {
            return libraries;
        }
        let mut info = info.assume_init();

        while status == libc::B_OK {
            let mut segments = Vec::new();
            segments.push(LibrarySegment {
                stated_virtual_memory_address: 0,
                len: info.text_size as usize,
            });

            let bytes = CStr::from_ptr(info.name.as_ptr()).to_bytes();
            let name = OsStr::from_bytes(bytes).to_owned();
            libraries.push(Library {
                name: name,
                segments: segments,
                bias: info.text as usize,
            });

            status = libc::get_next_image_info(libc::B_CURRENT_TEAM, &mut cookie, &mut info);
        }
    }

    libraries
}
