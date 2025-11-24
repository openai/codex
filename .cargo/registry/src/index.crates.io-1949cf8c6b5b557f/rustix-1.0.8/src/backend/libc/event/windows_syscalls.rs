//! Windows system calls in the `event` module.

use crate::backend::c;
use crate::backend::conv::ret_c_int;
use crate::event::{FdSetElement, PollFd, Timespec};
use crate::io;

pub(crate) fn poll(fds: &mut [PollFd<'_>], timeout: Option<&Timespec>) -> io::Result<usize> {
    let nfds = fds
        .len()
        .try_into()
        .map_err(|_convert_err| io::Errno::INVAL)?;

    let timeout = match timeout {
        None => -1,
        Some(timeout) => timeout.as_c_int_millis().ok_or(io::Errno::INVAL)?,
    };

    ret_c_int(unsafe { c::poll(fds.as_mut_ptr().cast(), nfds, timeout) })
        .map(|nready| nready as usize)
}

pub(crate) fn select(
    nfds: i32,
    readfds: Option<&mut [FdSetElement]>,
    writefds: Option<&mut [FdSetElement]>,
    exceptfds: Option<&mut [FdSetElement]>,
    timeout: Option<&crate::timespec::Timespec>,
) -> io::Result<i32> {
    use core::ptr::{null, null_mut};

    let readfds = match readfds {
        Some(readfds) => {
            assert!(readfds.len() >= readfds[0].0 as usize);
            readfds.as_mut_ptr()
        }
        None => null_mut(),
    };
    let writefds = match writefds {
        Some(writefds) => {
            assert!(writefds.len() >= writefds[0].0 as usize);
            writefds.as_mut_ptr()
        }
        None => null_mut(),
    };
    let exceptfds = match exceptfds {
        Some(exceptfds) => {
            assert!(exceptfds.len() >= exceptfds[0].0 as usize);
            exceptfds.as_mut_ptr()
        }
        None => null_mut(),
    };

    let timeout_data;
    let timeout_ptr = match timeout {
        Some(timeout) => {
            // Convert from `Timespec` to `TIMEVAL`.
            timeout_data = c::TIMEVAL {
                tv_sec: timeout
                    .tv_sec
                    .try_into()
                    .map_err(|_| io::Errno::OPNOTSUPP)?,
                tv_usec: ((timeout.tv_nsec + 999) / 1000) as _,
            };
            &timeout_data
        }
        None => null(),
    };

    unsafe {
        ret_c_int(c::select(
            nfds,
            readfds.cast(),
            writefds.cast(),
            exceptfds.cast(),
            timeout_ptr,
        ))
    }
}
