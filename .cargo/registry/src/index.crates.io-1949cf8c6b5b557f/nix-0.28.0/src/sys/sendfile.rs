//! Send data from a file to a socket, bypassing userland.

use cfg_if::cfg_if;
use std::os::unix::io::{AsFd, AsRawFd};
use std::ptr;

use libc::{self, off_t};

use crate::errno::Errno;
use crate::Result;

/// Copy up to `count` bytes to `out_fd` from `in_fd` starting at `offset`.
///
/// Returns a `Result` with the number of bytes written.
///
/// If `offset` is `None`, `sendfile` will begin reading at the current offset of `in_fd`and will
/// update the offset of `in_fd`. If `offset` is `Some`, `sendfile` will begin at the specified
/// offset and will not update the offset of `in_fd`. Instead, it will mutate `offset` to point to
/// the byte after the last byte copied.
///
/// `in_fd` must support `mmap`-like operations and therefore cannot be a socket.
///
/// For more information, see [the sendfile(2) man page.](https://man7.org/linux/man-pages/man2/sendfile.2.html) for Linux,
/// see [the sendfile(2) man page.](https://docs.oracle.com/cd/E88353_01/html/E37843/sendfile-3c.html) for Solaris.
#[cfg(any(linux_android, solarish))]
pub fn sendfile<F1: AsFd, F2: AsFd>(
    out_fd: F1,
    in_fd: F2,
    offset: Option<&mut off_t>,
    count: usize,
) -> Result<usize> {
    let offset = offset
        .map(|offset| offset as *mut _)
        .unwrap_or(ptr::null_mut());
    let ret = unsafe {
        libc::sendfile(
            out_fd.as_fd().as_raw_fd(),
            in_fd.as_fd().as_raw_fd(),
            offset,
            count,
        )
    };
    Errno::result(ret).map(|r| r as usize)
}

/// Copy up to `count` bytes to `out_fd` from `in_fd` starting at `offset`.
///
/// Returns a `Result` with the number of bytes written.
///
/// If `offset` is `None`, `sendfile` will begin reading at the current offset of `in_fd`and will
/// update the offset of `in_fd`. If `offset` is `Some`, `sendfile` will begin at the specified
/// offset and will not update the offset of `in_fd`. Instead, it will mutate `offset` to point to
/// the byte after the last byte copied.
///
/// `in_fd` must support `mmap`-like operations and therefore cannot be a socket.
///
/// For more information, see [the sendfile(2) man page.](https://man7.org/linux/man-pages/man2/sendfile.2.html)
#[cfg(target_os = "linux")]
pub fn sendfile64<F1: AsFd, F2: AsFd>(
    out_fd: F1,
    in_fd: F2,
    offset: Option<&mut libc::off64_t>,
    count: usize,
) -> Result<usize> {
    let offset = offset
        .map(|offset| offset as *mut _)
        .unwrap_or(ptr::null_mut());
    let ret = unsafe {
        libc::sendfile64(
            out_fd.as_fd().as_raw_fd(),
            in_fd.as_fd().as_raw_fd(),
            offset,
            count,
        )
    };
    Errno::result(ret).map(|r| r as usize)
}

cfg_if! {
    if #[cfg(any(freebsdlike, apple_targets))] {
        use std::io::IoSlice;

        #[derive(Clone, Debug)]
        struct SendfileHeaderTrailer<'a> {
            raw: libc::sf_hdtr,
            _headers: Option<Vec<IoSlice<'a>>>,
            _trailers: Option<Vec<IoSlice<'a>>>,
        }

        impl<'a> SendfileHeaderTrailer<'a> {
            fn new(
                headers: Option<&'a [&'a [u8]]>,
                trailers: Option<&'a [&'a [u8]]>
            ) -> SendfileHeaderTrailer<'a> {
                let mut header_iovecs: Option<Vec<IoSlice<'_>>> =
                    headers.map(|s| s.iter().map(|b| IoSlice::new(b)).collect());
                let mut trailer_iovecs: Option<Vec<IoSlice<'_>>> =
                    trailers.map(|s| s.iter().map(|b| IoSlice::new(b)).collect());

                SendfileHeaderTrailer {
                    raw: libc::sf_hdtr {
                        headers: {
                            header_iovecs
                                .as_mut()
                                .map_or(ptr::null_mut(), |v| v.as_mut_ptr())
                                .cast()
                        },
                        hdr_cnt: header_iovecs.as_ref().map(|v| v.len()).unwrap_or(0) as i32,
                        trailers: {
                            trailer_iovecs
                                .as_mut()
                                .map_or(ptr::null_mut(), |v| v.as_mut_ptr())
                                .cast()
                        },
                        trl_cnt: trailer_iovecs.as_ref().map(|v| v.len()).unwrap_or(0) as i32
                    },
                    _headers: header_iovecs,
                    _trailers: trailer_iovecs,
                }
            }
        }
    } else if #[cfg(solarish)] {
        use std::os::unix::io::BorrowedFd;
        use std::marker::PhantomData;

        #[derive(Debug, Copy, Clone)]
        /// Mapping of the raw C sendfilevec_t struct
        pub struct SendfileVec<'fd> {
            raw: libc::sendfilevec_t,
            phantom: PhantomData<BorrowedFd<'fd>>
        }

        impl<'fd> SendfileVec<'fd> {
            /// initialises SendfileVec to send data directly from the process's address space
            /// same in C with sfv_fd set to SFV_FD_SELF.
            pub fn newself(
                off: off_t,
                len: usize
            ) -> Self {
                Self{raw: libc::sendfilevec_t{sfv_fd: libc::SFV_FD_SELF, sfv_flag: 0, sfv_off: off, sfv_len: len}, phantom: PhantomData}
            }

            /// initialises SendfileVec to send data from `fd`.
            pub fn new(
                fd: BorrowedFd<'fd>,
                off: off_t,
                len: usize
            ) -> SendfileVec<'fd> {
                Self{raw: libc::sendfilevec_t{sfv_fd: fd.as_raw_fd(), sfv_flag: 0, sfv_off:off, sfv_len: len}, phantom: PhantomData}
            }
        }

        impl From<SendfileVec<'_>> for libc::sendfilevec_t {
            fn from<'fd>(vec: SendfileVec) -> libc::sendfilevec_t {
                vec.raw
            }
        }
    }
}

cfg_if! {
    if #[cfg(target_os = "freebsd")] {
        use libc::c_int;

        libc_bitflags!{
            /// Configuration options for [`sendfile`.](fn.sendfile.html)
            pub struct SfFlags: c_int {
                /// Causes `sendfile` to return EBUSY instead of blocking when attempting to read a
                /// busy page.
                SF_NODISKIO;
                /// Causes `sendfile` to sleep until the network stack releases its reference to the
                /// VM pages read. When `sendfile` returns, the data is not guaranteed to have been
                /// sent, but it is safe to modify the file.
                SF_SYNC;
                /// Causes `sendfile` to cache exactly the number of pages specified in the
                /// `readahead` parameter, disabling caching heuristics.
                SF_USER_READAHEAD;
                /// Causes `sendfile` not to cache the data read.
                SF_NOCACHE;
            }
        }

        /// Read up to `count` bytes from `in_fd` starting at `offset` and write to `out_sock`.
        ///
        /// Returns a `Result` and a count of bytes written. Bytes written may be non-zero even if
        /// an error occurs.
        ///
        /// `in_fd` must describe a regular file or shared memory object. `out_sock` must describe a
        /// stream socket.
        ///
        /// If `offset` falls past the end of the file, the function returns success and zero bytes
        /// written.
        ///
        /// If `count` is `None` or 0, bytes will be read from `in_fd` until reaching the end of
        /// file (EOF).
        ///
        /// `headers` and `trailers` specify optional slices of byte slices to be sent before and
        /// after the data read from `in_fd`, respectively. The length of headers and trailers sent
        /// is included in the returned count of bytes written. The values of `offset` and `count`
        /// do not apply to headers or trailers.
        ///
        /// `readahead` specifies the minimum number of pages to cache in memory ahead of the page
        /// currently being sent.
        ///
        /// For more information, see
        /// [the sendfile(2) man page.](https://www.freebsd.org/cgi/man.cgi?query=sendfile&sektion=2)
        #[allow(clippy::too_many_arguments)]
        pub fn sendfile<F1: AsFd, F2: AsFd>(
            in_fd: F1,
            out_sock: F2,
            offset: off_t,
            count: Option<usize>,
            headers: Option<&[&[u8]]>,
            trailers: Option<&[&[u8]]>,
            flags: SfFlags,
            readahead: u16
        ) -> (Result<()>, off_t) {
            // Readahead goes in upper 16 bits
            // Flags goes in lower 16 bits
            // see `man 2 sendfile`
            let ra32 = u32::from(readahead);
            let flags: u32 = (ra32 << 16) | (flags.bits() as u32);
            let mut bytes_sent: off_t = 0;
            let hdtr = headers.or(trailers).map(|_| SendfileHeaderTrailer::new(headers, trailers));
            let hdtr_ptr = hdtr.as_ref().map_or(ptr::null(), |s| &s.raw as *const libc::sf_hdtr);
            let return_code = unsafe {
                libc::sendfile(in_fd.as_fd().as_raw_fd(),
                               out_sock.as_fd().as_raw_fd(),
                               offset,
                               count.unwrap_or(0),
                               hdtr_ptr as *mut libc::sf_hdtr,
                               &mut bytes_sent as *mut off_t,
                               flags as c_int)
            };
            (Errno::result(return_code).and(Ok(())), bytes_sent)
        }
    } else if #[cfg(target_os = "dragonfly")] {
        /// Read up to `count` bytes from `in_fd` starting at `offset` and write to `out_sock`.
        ///
        /// Returns a `Result` and a count of bytes written. Bytes written may be non-zero even if
        /// an error occurs.
        ///
        /// `in_fd` must describe a regular file. `out_sock` must describe a stream socket.
        ///
        /// If `offset` falls past the end of the file, the function returns success and zero bytes
        /// written.
        ///
        /// If `count` is `None` or 0, bytes will be read from `in_fd` until reaching the end of
        /// file (EOF).
        ///
        /// `headers` and `trailers` specify optional slices of byte slices to be sent before and
        /// after the data read from `in_fd`, respectively. The length of headers and trailers sent
        /// is included in the returned count of bytes written. The values of `offset` and `count`
        /// do not apply to headers or trailers.
        ///
        /// For more information, see
        /// [the sendfile(2) man page.](https://leaf.dragonflybsd.org/cgi/web-man?command=sendfile&section=2)
        pub fn sendfile<F1: AsFd, F2: AsFd>(
            in_fd: F1,
            out_sock: F2,
            offset: off_t,
            count: Option<usize>,
            headers: Option<&[&[u8]]>,
            trailers: Option<&[&[u8]]>,
        ) -> (Result<()>, off_t) {
            let mut bytes_sent: off_t = 0;
            let hdtr = headers.or(trailers).map(|_| SendfileHeaderTrailer::new(headers, trailers));
            let hdtr_ptr = hdtr.as_ref().map_or(ptr::null(), |s| &s.raw as *const libc::sf_hdtr);
            let return_code = unsafe {
                libc::sendfile(in_fd.as_fd().as_raw_fd(),
                               out_sock.as_fd().as_raw_fd(),
                               offset,
                               count.unwrap_or(0),
                               hdtr_ptr as *mut libc::sf_hdtr,
                               &mut bytes_sent as *mut off_t,
                               0)
            };
            (Errno::result(return_code).and(Ok(())), bytes_sent)
        }
    } else if #[cfg(apple_targets)] {
        /// Read bytes from `in_fd` starting at `offset` and write up to `count` bytes to
        /// `out_sock`.
        ///
        /// Returns a `Result` and a count of bytes written. Bytes written may be non-zero even if
        /// an error occurs.
        ///
        /// `in_fd` must describe a regular file. `out_sock` must describe a stream socket.
        ///
        /// If `offset` falls past the end of the file, the function returns success and zero bytes
        /// written.
        ///
        /// If `count` is `None` or 0, bytes will be read from `in_fd` until reaching the end of
        /// file (EOF).
        ///
        /// `hdtr` specifies an optional list of headers and trailers to be sent before and after
        /// the data read from `in_fd`, respectively. The length of headers and trailers sent is
        /// included in the returned count of bytes written. If any headers are specified and
        /// `count` is non-zero, the length of the headers will be counted in the limit of total
        /// bytes sent. Trailers do not count toward the limit of bytes sent and will always be sent
        /// regardless. The value of `offset` does not affect headers or trailers.
        ///
        /// For more information, see
        /// [the sendfile(2) man page.](https://developer.apple.com/legacy/library/documentation/Darwin/Reference/ManPages/man2/sendfile.2.html)
        pub fn sendfile<F1: AsFd, F2: AsFd>(
            in_fd: F1,
            out_sock: F2,
            offset: off_t,
            count: Option<off_t>,
            headers: Option<&[&[u8]]>,
            trailers: Option<&[&[u8]]>
        ) -> (Result<()>, off_t) {
            let mut len = count.unwrap_or(0);
            let hdtr = headers.or(trailers).map(|_| SendfileHeaderTrailer::new(headers, trailers));
            let hdtr_ptr = hdtr.as_ref().map_or(ptr::null(), |s| &s.raw as *const libc::sf_hdtr);
            let return_code = unsafe {
                libc::sendfile(in_fd.as_fd().as_raw_fd(),
                               out_sock.as_fd().as_raw_fd(),
                               offset,
                               &mut len as *mut off_t,
                               hdtr_ptr as *mut libc::sf_hdtr,
                               0)
            };
            (Errno::result(return_code).and(Ok(())), len)
        }
    } else if #[cfg(solarish)] {
        /// Write data from the vec arrays to `out_sock` and returns a `Result` and a
        /// count of bytes written.
        ///
        /// Each `SendfileVec` set needs to be instantiated either with `SendfileVec::new` or
        /// `SendfileVec::newself`.
        ///
        /// The former allows to send data from a file descriptor through `fd`,
        ///  from an offset `off` and for a given amount of data `len`.
        ///
        /// The latter allows to send data from the process's address space, from an offset `off`
        /// and for a given amount of data `len`.
        ///
        /// For more information, see
        /// [the sendfilev(3) man page.](https://illumos.org/man/3EXT/sendfilev)
        pub fn sendfilev<F: AsFd>(
            out_sock: F,
            vec: &[SendfileVec]
        ) -> (Result<()>, usize) {
            let mut len = 0usize;
            let return_code = unsafe {
                libc::sendfilev(out_sock.as_fd().as_raw_fd(), vec.as_ptr() as *const libc::sendfilevec_t, vec.len() as i32, &mut len)
            };
            (Errno::result(return_code).and(Ok(())), len)
        }
    }
}
