use crate::prelude::*;

pub const L_tmpnam: c_uint = 260;
pub const TMP_MAX: c_uint = 0x7fff_ffff;

// POSIX Supplement (from errno.h)
// This particular error code is only currently available in msvc toolchain
pub const EOTHER: c_int = 131;

extern "C" {
    #[link_name = "_stricmp"]
    pub fn stricmp(s1: *const c_char, s2: *const c_char) -> c_int;
    #[link_name = "_strnicmp"]
    pub fn strnicmp(s1: *const c_char, s2: *const c_char, n: size_t) -> c_int;
    #[link_name = "_memccpy"]
    pub fn memccpy(dest: *mut c_void, src: *const c_void, c: c_int, count: size_t) -> *mut c_void;
}
