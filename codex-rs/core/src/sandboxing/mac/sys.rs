use std::ffi::CStr;
use std::path::PathBuf;
use std::string::ToString;

use libc;

/// Wraps libc::confstr to return a String.
fn confstr(name: libc::c_int) -> Option<String> {
    let mut buf = vec![0_i8; (libc::PATH_MAX as usize) + 1];
    let len = unsafe { libc::confstr(name, buf.as_mut_ptr(), buf.len()) };
    if len == 0 {
        return None;
    }
    // confstr guarantees NUL-termination when len > 0.
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
    cstr.to_str().ok().map(ToString::to_string)
}

/// Wraps confstr to return a canonicalized PathBuf.
fn confstr_path(name: libc::c_int) -> Option<PathBuf> {
    let s = confstr(name)?;
    let path = PathBuf::from(s);
    path.canonicalize().ok().or(Some(path))
}

pub fn user_cache_dir() -> Option<PathBuf> {
    confstr_path(libc::_CS_DARWIN_USER_CACHE_DIR)
}
