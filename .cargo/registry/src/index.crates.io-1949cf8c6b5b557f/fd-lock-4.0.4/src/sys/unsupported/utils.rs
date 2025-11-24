use std::io;
use std::os::raw::c_int;

pub(crate) fn syscall(int: c_int) -> io::Result<()> {
    match int {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}
