//! Verify preserved file descriptors remain inheritable across exec.

use super::*;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

#[test]
fn preserved_files_are_made_inheritable() {
    let file = NamedTempFile::new().expect("temp file");
    set_cloexec(file.as_file().as_raw_fd());

    make_files_inheritable(std::slice::from_ref(file.as_file()));

    assert_eq!(fd_flags(file.as_file().as_raw_fd()) & libc::FD_CLOEXEC, 0);
}

fn set_cloexec(fd: libc::c_int) {
    let flags = fd_flags(fd);
    // SAFETY: `fd` is valid for the duration of the test.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to set CLOEXEC for test fd {fd}: {err}");
    }
}

fn fd_flags(fd: libc::c_int) -> libc::c_int {
    // SAFETY: `fd` is valid for the duration of the test.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to read fd flags for test fd {fd}: {err}");
    }
    flags
}
