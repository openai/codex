/// A resource value for use with [`getrlimit`], [`setrlimit`], and
/// [`prlimit`].
///
/// [`getrlimit`]: crate::process::getrlimit
/// [`setrlimit`]: crate::process::setrlimit
/// [`prlimit`]: crate::process::prlimit
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
#[non_exhaustive]
pub enum Resource {
    /// `RLIMIT_CPU`
    Cpu = linux_raw_sys::general::RLIMIT_CPU,
    /// `RLIMIT_FSIZE`
    Fsize = linux_raw_sys::general::RLIMIT_FSIZE,
    /// `RLIMIT_DATA`
    Data = linux_raw_sys::general::RLIMIT_DATA,
    /// `RLIMIT_STACK`
    Stack = linux_raw_sys::general::RLIMIT_STACK,
    /// `RLIMIT_CORE`
    Core = linux_raw_sys::general::RLIMIT_CORE,
    /// `RLIMIT_RSS`
    Rss = linux_raw_sys::general::RLIMIT_RSS,
    /// `RLIMIT_NPROC`
    Nproc = linux_raw_sys::general::RLIMIT_NPROC,
    /// `RLIMIT_NOFILE`
    Nofile = linux_raw_sys::general::RLIMIT_NOFILE,
    /// `RLIMIT_MEMLOCK`
    Memlock = linux_raw_sys::general::RLIMIT_MEMLOCK,
    /// `RLIMIT_AS`
    As = linux_raw_sys::general::RLIMIT_AS,
    /// `RLIMIT_LOCKS`
    Locks = linux_raw_sys::general::RLIMIT_LOCKS,
    /// `RLIMIT_SIGPENDING`
    Sigpending = linux_raw_sys::general::RLIMIT_SIGPENDING,
    /// `RLIMIT_MSGQUEUE`
    Msgqueue = linux_raw_sys::general::RLIMIT_MSGQUEUE,
    /// `RLIMIT_NICE`
    Nice = linux_raw_sys::general::RLIMIT_NICE,
    /// `RLIMIT_RTPRIO`
    Rtprio = linux_raw_sys::general::RLIMIT_RTPRIO,
    /// `RLIMIT_RTTIME`
    Rttime = linux_raw_sys::general::RLIMIT_RTTIME,
}
