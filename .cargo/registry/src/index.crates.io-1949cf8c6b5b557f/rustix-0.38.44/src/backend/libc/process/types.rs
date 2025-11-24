#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
use crate::backend::c;

/// A command for use with [`membarrier`] and [`membarrier_cpu`].
///
/// For `MEMBARRIER_CMD_QUERY`, see [`membarrier_query`].
///
/// [`membarrier`]: crate::process::membarrier
/// [`membarrier_cpu`]: crate::process::membarrier_cpu
/// [`membarrier_query`]: crate::process::membarrier_query
#[cfg(linux_kernel)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum MembarrierCommand {
    /// `MEMBARRIER_CMD_GLOBAL`
    #[doc(alias = "Shared")]
    #[doc(alias = "MEMBARRIER_CMD_SHARED")]
    Global = c::MEMBARRIER_CMD_GLOBAL as u32,
    /// `MEMBARRIER_CMD_GLOBAL_EXPEDITED`
    GlobalExpedited = c::MEMBARRIER_CMD_GLOBAL_EXPEDITED as u32,
    /// `MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED`
    RegisterGlobalExpedited = c::MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED as u32,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED`
    PrivateExpedited = c::MEMBARRIER_CMD_PRIVATE_EXPEDITED as u32,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED`
    RegisterPrivateExpedited = c::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as u32,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE`
    PrivateExpeditedSyncCore = c::MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE as u32,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE`
    RegisterPrivateExpeditedSyncCore =
        c::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE as u32,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
    PrivateExpeditedRseq = c::MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ as u32,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
    RegisterPrivateExpeditedRseq = c::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ as u32,
}

/// A resource value for use with [`getrlimit`], [`setrlimit`], and
/// [`prlimit`].
///
/// [`getrlimit`]: crate::process::getrlimit
/// [`setrlimit`]: crate::process::setrlimit
/// [`prlimit`]: crate::process::prlimit
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(target_os = "l4re"), repr(u32))]
#[cfg_attr(target_os = "l4re", repr(u64))]
pub enum Resource {
    /// `RLIMIT_CPU`
    Cpu = bitcast!(c::RLIMIT_CPU),
    /// `RLIMIT_FSIZE`
    Fsize = bitcast!(c::RLIMIT_FSIZE),
    /// `RLIMIT_DATA`
    Data = bitcast!(c::RLIMIT_DATA),
    /// `RLIMIT_STACK`
    Stack = bitcast!(c::RLIMIT_STACK),
    /// `RLIMIT_CORE`
    #[cfg(not(target_os = "haiku"))]
    Core = bitcast!(c::RLIMIT_CORE),
    /// `RLIMIT_RSS`
    // "nto" has `RLIMIT_RSS`, but it has the same value as `RLIMIT_AS`.
    #[cfg(not(any(apple, solarish, target_os = "nto", target_os = "haiku")))]
    Rss = bitcast!(c::RLIMIT_RSS),
    /// `RLIMIT_NPROC`
    #[cfg(not(any(solarish, target_os = "haiku")))]
    Nproc = bitcast!(c::RLIMIT_NPROC),
    /// `RLIMIT_NOFILE`
    Nofile = bitcast!(c::RLIMIT_NOFILE),
    /// `RLIMIT_MEMLOCK`
    #[cfg(not(any(solarish, target_os = "aix", target_os = "haiku")))]
    Memlock = bitcast!(c::RLIMIT_MEMLOCK),
    /// `RLIMIT_AS`
    #[cfg(not(target_os = "openbsd"))]
    As = bitcast!(c::RLIMIT_AS),
    /// `RLIMIT_LOCKS`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto"
    )))]
    Locks = bitcast!(c::RLIMIT_LOCKS),
    /// `RLIMIT_SIGPENDING`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto"
    )))]
    Sigpending = bitcast!(c::RLIMIT_SIGPENDING),
    /// `RLIMIT_MSGQUEUE`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto"
    )))]
    Msgqueue = bitcast!(c::RLIMIT_MSGQUEUE),
    /// `RLIMIT_NICE`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto"
    )))]
    Nice = bitcast!(c::RLIMIT_NICE),
    /// `RLIMIT_RTPRIO`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto"
    )))]
    Rtprio = bitcast!(c::RLIMIT_RTPRIO),
    /// `RLIMIT_RTTIME`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "android",
        target_os = "emscripten",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto",
    )))]
    Rttime = bitcast!(c::RLIMIT_RTTIME),
}

#[cfg(apple)]
#[allow(non_upper_case_globals)]
impl Resource {
    /// `RLIMIT_RSS`
    pub const Rss: Self = Self::As;
}

/// A CPU identifier as a raw integer.
#[cfg(linux_kernel)]
pub type RawCpuid = u32;
#[cfg(freebsdlike)]
pub type RawId = c::id_t;

#[cfg(any(linux_kernel, target_os = "fuchsia"))]
pub(crate) type RawCpuSet = c::cpu_set_t;
#[cfg(freebsdlike)]
pub(crate) type RawCpuSet = c::cpuset_t;

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
#[inline]
pub(crate) fn raw_cpu_set_new() -> RawCpuSet {
    let mut set = unsafe { core::mem::zeroed() };
    super::cpu_set::CPU_ZERO(&mut set);
    set
}

#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
pub(crate) const CPU_SETSIZE: usize = c::CPU_SETSIZE as usize;
