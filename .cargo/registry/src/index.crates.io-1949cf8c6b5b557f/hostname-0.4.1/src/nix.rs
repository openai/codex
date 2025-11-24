#[cfg(feature = "set")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
#[cfg(feature = "set")]
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;

const _POSIX_HOST_NAME_MAX: libc::c_long = 255;

pub fn get() -> io::Result<OsString> {
    // According to the POSIX specification,
    // host names are limited to `HOST_NAME_MAX` bytes
    //
    // https://pubs.opengroup.org/onlinepubs/9699919799/functions/gethostname.html
    let limit = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) };
    let size = libc::c_long::max(limit, _POSIX_HOST_NAME_MAX) as usize;

    // Reserve additional space for terminating nul byte.
    let mut buffer = vec![0u8; size + 1];

    #[allow(trivial_casts)]
    let result = unsafe { libc::gethostname(buffer.as_mut_ptr() as *mut libc::c_char, size) };

    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(wrap_buffer(buffer))
}

fn wrap_buffer(mut bytes: Vec<u8>) -> OsString {
    // Returned name might be truncated if it does not fit
    // and `buffer` will not contain the trailing \0 in that case.
    // Manually capping the buffer length here.
    let end = bytes
        .iter()
        .position(|&byte| byte == 0x00)
        .unwrap_or(bytes.len());
    bytes.resize(end, 0x00);

    OsString::from_vec(bytes)
}

#[cfg(feature = "set")]
pub fn set(hostname: &OsStr) -> io::Result<()> {
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "solaris",
        target_os = "illumos"
    )))]
    #[allow(non_camel_case_types)]
    type hostname_len_t = libc::size_t;

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "solaris",
        target_os = "illumos"
    ))]
    #[allow(non_camel_case_types)]
    type hostname_len_t = libc::c_int;

    #[allow(clippy::unnecessary_cast)]
    // Cast is needed for the `libc::c_int` type
    if hostname.len() > hostname_len_t::MAX as usize {
        return Err(io::Error::other("hostname too long"));
    }

    let size = hostname.len() as hostname_len_t;

    #[allow(trivial_casts)]
    let result =
        unsafe { libc::sethostname(hostname.as_bytes().as_ptr() as *const libc::c_char, size) };

    if result != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::wrap_buffer;

    // Happy path case: there is a correct null terminated C string in a buffer
    // and a bunch of NULL characters from the pre-allocated buffer
    #[test]
    fn test_non_overflowed_buffer() {
        let buf = b"potato\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_vec();

        assert_eq!(wrap_buffer(buf), OsStr::new("potato"));
    }

    #[test]
    fn test_empty_buffer() {
        let buf = b"".to_vec();

        assert_eq!(wrap_buffer(buf), OsStr::new(""));
    }

    #[test]
    fn test_filled_with_null_buffer() {
        let buf = b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_vec();

        assert_eq!(wrap_buffer(buf), OsStr::new(""));
    }

    // Hostname value had overflowed the buffer, so it was truncated
    // and according to the POSIX documentation of the `gethostname`:
    //
    // > it is unspecified whether the returned name is null-terminated.
    #[test]
    fn test_overflowed_buffer() {
        let buf = b"potat".to_vec();

        assert_eq!(wrap_buffer(buf), OsStr::new("potat"));
    }
}
