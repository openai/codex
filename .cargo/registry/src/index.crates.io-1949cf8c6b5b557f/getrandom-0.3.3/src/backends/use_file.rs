//! Implementations that just need to read from a file
use crate::Error;
use core::{
    ffi::c_void,
    mem::MaybeUninit,
    sync::atomic::{AtomicI32, Ordering},
};

#[cfg(not(any(target_os = "android", target_os = "linux")))]
pub use crate::util::{inner_u32, inner_u64};

#[path = "../util_libc.rs"]
pub(super) mod util_libc;

/// For all platforms, we use `/dev/urandom` rather than `/dev/random`.
/// For more information see the linked man pages in lib.rs.
///   - On Linux, "/dev/urandom is preferred and sufficient in all use cases".
///   - On Redox, only /dev/urandom is provided.
///   - On AIX, /dev/urandom will "provide cryptographically secure output".
///   - On Haiku and QNX Neutrino they are identical.
const FILE_PATH: &[u8] = b"/dev/urandom\0";

// File descriptor is a "nonnegative integer", so we can safely use negative sentinel values.
const FD_UNINIT: libc::c_int = -1;
const FD_ONGOING_INIT: libc::c_int = -2;

// In theory `libc::c_int` could be something other than `i32`, but for the
// targets we currently support that use `use_file`, it is always `i32`.
// If/when we add support for a target where that isn't the case, we may
// need to use a different atomic type or make other accomodations. The
// compiler will let us know if/when that is the case, because the
// `FD.store(fd)` would fail to compile.
//
// The opening of the file, by libc/libstd/etc. may write some unknown
// state into in-process memory. (Such state may include some sanitizer
// bookkeeping, or we might be operating in a unikernal-like environment
// where all the "kernel" file descriptor bookkeeping is done in our
// process.) `get_fd_locked` stores into FD using `Ordering::Release` to
// ensure any such state is synchronized. `get_fd` loads from `FD` with
// `Ordering::Acquire` to synchronize with it.
static FD: AtomicI32 = AtomicI32::new(FD_UNINIT);

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let mut fd = FD.load(Ordering::Acquire);
    if fd == FD_UNINIT || fd == FD_ONGOING_INIT {
        fd = open_or_wait()?;
    }
    util_libc::sys_fill_exact(dest, |buf| unsafe {
        libc::read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len())
    })
}

/// Open a file in read-only mode.
///
/// # Panics
/// If `path` does not contain any zeros.
// TODO: Move `path` to `CStr` and use `CStr::from_bytes_until_nul` (MSRV 1.69)
// or C-string literals (MSRV 1.77) for statics
fn open_readonly(path: &[u8]) -> Result<libc::c_int, Error> {
    assert!(path.contains(&0));
    loop {
        let fd = unsafe {
            libc::open(
                path.as_ptr().cast::<libc::c_char>(),
                libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd >= 0 {
            return Ok(fd);
        }
        let err = util_libc::last_os_error();
        // We should try again if open() was interrupted.
        if err.raw_os_error() != Some(libc::EINTR) {
            return Err(err);
        }
    }
}

#[cold]
#[inline(never)]
fn open_or_wait() -> Result<libc::c_int, Error> {
    loop {
        match FD.load(Ordering::Acquire) {
            FD_UNINIT => {
                let res = FD.compare_exchange_weak(
                    FD_UNINIT,
                    FD_ONGOING_INIT,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
                if res.is_ok() {
                    break;
                }
            }
            FD_ONGOING_INIT => sync::wait(),
            fd => return Ok(fd),
        }
    }

    let res = open_fd();
    let val = match res {
        Ok(fd) => fd,
        Err(_) => FD_UNINIT,
    };
    FD.store(val, Ordering::Release);

    // On non-Linux targets `wait` is just 1 ms sleep,
    // so we don't need any explicit wake up in addition
    // to updating value of `FD`.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    sync::wake();

    res
}

fn open_fd() -> Result<libc::c_int, Error> {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    sync::wait_until_rng_ready()?;
    let fd = open_readonly(FILE_PATH)?;
    debug_assert!(fd >= 0);
    Ok(fd)
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
mod sync {
    /// Sleep 1 ms before checking `FD` again.
    ///
    /// On non-Linux targets the critical section only opens file,
    /// which should not block, so in the unlikely contended case,
    /// we can sleep-wait for the opening operation to finish.
    pub(super) fn wait() {
        let rqtp = libc::timespec {
            tv_sec: 0,
            tv_nsec: 1_000_000,
        };
        let mut rmtp = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // We do not care if sleep gets interrupted, so the return value is ignored
        unsafe {
            libc::nanosleep(&rqtp, &mut rmtp);
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
mod sync {
    use super::{open_readonly, util_libc::last_os_error, Error, FD, FD_ONGOING_INIT};

    /// Wait for atomic `FD` to change value from `FD_ONGOING_INIT` to something else.
    ///
    /// Futex syscall with `FUTEX_WAIT` op puts the current thread to sleep
    /// until futex syscall with `FUTEX_WAKE` op gets executed for `FD`.
    ///
    /// For more information read: https://www.man7.org/linux/man-pages/man2/futex.2.html
    pub(super) fn wait() {
        let op = libc::FUTEX_WAIT | libc::FUTEX_PRIVATE_FLAG;
        let timeout_ptr = core::ptr::null::<libc::timespec>();
        let ret = unsafe { libc::syscall(libc::SYS_futex, &FD, op, FD_ONGOING_INIT, timeout_ptr) };
        // FUTEX_WAIT should return either 0 or EAGAIN error
        debug_assert!({
            match ret {
                0 => true,
                -1 => last_os_error().raw_os_error() == Some(libc::EAGAIN),
                _ => false,
            }
        });
    }

    /// Wake up all threads which wait for value of atomic `FD` to change.
    pub(super) fn wake() {
        let op = libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG;
        let ret = unsafe { libc::syscall(libc::SYS_futex, &FD, op, libc::INT_MAX) };
        debug_assert!(ret >= 0);
    }

    // Polls /dev/random to make sure it is ok to read from /dev/urandom.
    //
    // Polling avoids draining the estimated entropy from /dev/random;
    // short-lived processes reading even a single byte from /dev/random could
    // be problematic if they are being executed faster than entropy is being
    // collected.
    //
    // OTOH, reading a byte instead of polling is more compatible with
    // sandboxes that disallow `poll()` but which allow reading /dev/random,
    // e.g. sandboxes that assume that `poll()` is for network I/O. This way,
    // fewer applications will have to insert pre-sandbox-initialization logic.
    // Often (blocking) file I/O is not allowed in such early phases of an
    // application for performance and/or security reasons.
    //
    // It is hard to write a sandbox policy to support `libc::poll()` because
    // it may invoke the `poll`, `ppoll`, `ppoll_time64` (since Linux 5.1, with
    // newer versions of glibc), and/or (rarely, and probably only on ancient
    // systems) `select`. depending on the libc implementation (e.g. glibc vs
    // musl), libc version, potentially the kernel version at runtime, and/or
    // the target architecture.
    //
    // BoringSSL and libstd don't try to protect against insecure output from
    // `/dev/urandom'; they don't open `/dev/random` at all.
    //
    // OpenSSL uses `libc::select()` unless the `dev/random` file descriptor
    // is too large; if it is too large then it does what we do here.
    //
    // libsodium uses `libc::poll` similarly to this.
    pub(super) fn wait_until_rng_ready() -> Result<(), Error> {
        let fd = open_readonly(b"/dev/random\0")?;
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let res = loop {
            // A negative timeout means an infinite timeout.
            let res = unsafe { libc::poll(&mut pfd, 1, -1) };
            if res >= 0 {
                // We only used one fd, and cannot timeout.
                debug_assert_eq!(res, 1);
                break Ok(());
            }
            let err = last_os_error();
            // Assuming that `poll` is called correctly,
            // on Linux it can return only EINTR and ENOMEM errors.
            match err.raw_os_error() {
                Some(libc::EINTR) => continue,
                _ => break Err(err),
            }
        };
        unsafe { libc::close(fd) };
        res
    }
}
