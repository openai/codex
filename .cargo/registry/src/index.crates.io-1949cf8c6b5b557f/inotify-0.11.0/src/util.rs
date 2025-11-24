use std::{
    io,
    mem,
    os::unix::io::RawFd,
    path::Path,
};

use inotify_sys as ffi;
use libc::{
    c_void,
    size_t,
};

const INOTIFY_EVENT_SIZE: usize = mem::size_of::<ffi::inotify_event>() + 257;

pub fn read_into_buffer(fd: RawFd, buffer: &mut [u8]) -> isize {
    unsafe {
        ffi::read(
            fd,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len() as size_t
        )
    }
}

/// Get the inotify event buffer size
///
/// The maximum size of an inotify event and thus the buffer size to hold it
/// can be calculated using this formula:
/// `sizeof(struct inotify_event) + NAME_MAX + 1`
///
/// See: <https://man7.org/linux/man-pages/man7/inotify.7.html>
///
/// The NAME_MAX size formula is:
/// `ABSOLUTE_PARENT_PATH_LEN + 1 + 255`
///
/// - `ABSOLUTE_PARENT_PATH_LEN` will be calculated at runtime.
/// - Add 1 to account for a `/`, either in between the parent path and a filename
/// or for the root directory.
/// - Add the maximum number of chars in a filename, 255.
///
/// See: <https://github.com/torvalds/linux/blob/master/include/uapi/linux/limits.h>
///
/// Unfortunately, we can't just do the same with max path length itself.
///
/// See: <https://eklitzke.org/path-max-is-tricky>
///
/// This function is really just a fallible wrapper around `get_absolute_path_buffer_size()`.
///
/// path: A relative or absolute path for the inotify events.
pub fn get_buffer_size(path: &Path) -> io::Result<usize> {
    Ok(get_absolute_path_buffer_size(&path.canonicalize()?))
}

/// Get the inotify event buffer size for an absolute path
///
/// For relative paths, consider using `get_buffer_size()` which provides a fallible wrapper
/// for this function.
///
/// path: An absolute path for the inotify events.
pub fn get_absolute_path_buffer_size(path: &Path) -> usize {
    INOTIFY_EVENT_SIZE
    // Get the length of the absolute parent path, if the path is not the root directory.
    // Because we canonicalize the path, we do not need to worry about prefixes.
    + if let Some(parent_path) = path.parent() {
        parent_path.as_os_str().len()
    } else {
        0
    }
}
