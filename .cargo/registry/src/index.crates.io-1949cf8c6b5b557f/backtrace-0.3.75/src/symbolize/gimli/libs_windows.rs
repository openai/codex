use super::super::super::windows_sys::*;
use super::mystd::ffi::OsString;
use super::{coff, mmap, Library, LibrarySegment};
use alloc::vec;
use alloc::vec::Vec;
use core::mem;
use core::mem::MaybeUninit;

// For loading native libraries on Windows, see some discussion on
// rust-lang/rust#71060 for the various strategies here.
pub(super) fn native_libraries() -> Vec<Library> {
    let mut ret = Vec::new();
    unsafe {
        add_loaded_images(&mut ret);
    }
    return ret;
}

unsafe fn add_loaded_images(ret: &mut Vec<Library>) {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0);
        if snap == INVALID_HANDLE_VALUE {
            return;
        }

        // huge struct, probably should avoid manually initializing it even if we can
        let mut me = MaybeUninit::<MODULEENTRY32W>::zeroed().assume_init();
        me.dwSize = mem::size_of_val(&me) as u32;
        if Module32FirstW(snap, &mut me) == TRUE {
            loop {
                if let Some(lib) = load_library(&me) {
                    ret.push(lib);
                }

                if Module32NextW(snap, &mut me) != TRUE {
                    break;
                }
            }
        }

        CloseHandle(snap);
    }
}

// Safety: long_path should be null-terminated
#[cfg(target_os = "cygwin")]
unsafe fn get_posix_path(long_path: &[u16]) -> Option<OsString> {
    use super::mystd::os::unix::ffi::OsStringExt;

    unsafe extern "C" {
        // Doc: https://cygwin.com/cygwin-api/func-cygwin-conv-path.html
        // Src: https://github.com/cygwin/cygwin/blob/718a15ba50e0d01c79800bd658c2477f9a603540/winsup/cygwin/path.cc#L3902
        // Safety:
        // * `what` should be `CCP_WIN_W_TO_POSIX` here
        // * `from` is null-terminated UTF-16 path
        // * `to` is buffer, the buffer size is `size`.
        fn cygwin_conv_path(
            what: libc::c_uint,
            from: *const u16,
            to: *mut u8,
            size: libc::size_t,
        ) -> libc::ssize_t;
    }
    const CCP_WIN_W_TO_POSIX: libc::c_uint = 3;

    // If `size` is 0, returns needed buffer size, including null terminator;
    // or -1 if error.
    // Safety: if `size` is 0, `to` is not used.
    let name_len = unsafe {
        cygwin_conv_path(
            CCP_WIN_W_TO_POSIX,
            long_path.as_ptr(),
            core::ptr::null_mut(),
            0,
        )
    };
    // Expect at least 1 for null terminator.
    // It's not likely to return error here.
    if name_len < 1 {
        return None;
    }
    let name_len = name_len as usize;
    let mut name_buffer = Vec::with_capacity(name_len);
    // Safety: `name_buffer` is large enough.
    let res = unsafe {
        cygwin_conv_path(
            CCP_WIN_W_TO_POSIX,
            long_path.as_ptr(),
            name_buffer.as_mut_ptr(),
            name_len,
        )
    };
    // It's not likely to return error here.
    if res != 0 {
        return None;
    }
    // Remove the null terminator.
    unsafe { name_buffer.set_len(name_len - 1) };
    let name = OsString::from_vec(name_buffer);
    Some(name)
}

unsafe fn load_library(me: &MODULEENTRY32W) -> Option<Library> {
    #[cfg(windows)]
    let name = {
        use super::mystd::os::windows::prelude::*;
        let pos = me
            .szExePath
            .iter()
            .position(|i| *i == 0)
            .unwrap_or(me.szExePath.len());
        OsString::from_wide(&me.szExePath[..pos])
    };
    #[cfg(target_os = "cygwin")]
    // Safety: the path with max length MAX_PATH always contains a null
    // terminator. Don't slice it.
    let name = unsafe { get_posix_path(&me.szExePath[..])? };

    // MinGW libraries currently don't support ASLR
    // (rust-lang/rust#16514), but DLLs can still be relocated around in
    // the address space. It appears that addresses in debug info are
    // all as-if this library was loaded at its "image base", which is a
    // field in its COFF file headers. Since this is what debuginfo
    // seems to list we parse the symbol table and store addresses as if
    // the library was loaded at "image base" as well.
    //
    // The library may not be loaded at "image base", however.
    // (presumably something else may be loaded there?) This is where
    // the `bias` field comes into play, and we need to figure out the
    // value of `bias` here. Unfortunately though it's not clear how to
    // acquire this from a loaded module. What we do have, however, is
    // the actual load address (`modBaseAddr`).
    //
    // As a bit of a cop-out for now we mmap the file, read the file
    // header information, then drop the mmap. This is wasteful because
    // we'll probably reopen the mmap later, but this should work well
    // enough for now.
    //
    // Once we have the `image_base` (desired load location) and the
    // `base_addr` (actual load location) we can fill in the `bias`
    // (difference between the actual and desired) and then the stated
    // address of each segment is the `image_base` since that's what the
    // file says.
    //
    // For now it appears that unlike ELF/MachO we can make do with one
    // segment per library, using `modBaseSize` as the whole size.
    let mmap = mmap(name.as_ref())?;
    let image_base = coff::get_image_base(&mmap)?;
    let base_addr = me.modBaseAddr as usize;
    Some(Library {
        name,
        bias: base_addr.wrapping_sub(image_base),
        segments: vec![LibrarySegment {
            stated_virtual_memory_address: image_base,
            len: me.modBaseSize as usize,
        }],
    })
}
