use linux_raw_sys::general::membarrier_cmd;

/// A command for use with [`membarrier`] and [`membarrier_cpu`].
///
/// For `MEMBARRIER_CMD_QUERY`, see [`membarrier_query`].
///
/// [`membarrier`]: crate::process::membarrier
/// [`membarrier_cpu`]: crate::process::membarrier_cpu
/// [`membarrier_query`]: crate::process::membarrier_query
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum MembarrierCommand {
    /// `MEMBARRIER_CMD_GLOBAL`
    #[doc(alias = "Shared")]
    #[doc(alias = "MEMBARRIER_CMD_SHARED")]
    Global = membarrier_cmd::MEMBARRIER_CMD_GLOBAL as _,
    /// `MEMBARRIER_CMD_GLOBAL_EXPEDITED`
    GlobalExpedited = membarrier_cmd::MEMBARRIER_CMD_GLOBAL_EXPEDITED as _,
    /// `MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED`
    RegisterGlobalExpedited = membarrier_cmd::MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED as _,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED`
    PrivateExpedited = membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED as _,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED`
    RegisterPrivateExpedited = membarrier_cmd::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as _,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE`
    PrivateExpeditedSyncCore = membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE as _,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE`
    RegisterPrivateExpeditedSyncCore =
        membarrier_cmd::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE as _,
    /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
    PrivateExpeditedRseq = membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ as _,
    /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
    RegisterPrivateExpeditedRseq =
        membarrier_cmd::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ as _,
}

/// A resource value for use with [`getrlimit`], [`setrlimit`], and
/// [`prlimit`].
///
/// [`getrlimit`]: crate::process::getrlimit
/// [`setrlimit`]: crate::process::setrlimit
/// [`prlimit`]: crate::process::prlimit
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
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

/// A CPU identifier as a raw integer.
pub type RawCpuid = u32;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RawCpuSet {
    #[cfg(all(target_pointer_width = "32", not(target_arch = "x86_64")))]
    pub(crate) bits: [u32; 32],
    #[cfg(not(all(target_pointer_width = "32", not(target_arch = "x86_64"))))]
    pub(crate) bits: [u64; 16],
}

#[inline]
pub(crate) fn raw_cpu_set_new() -> RawCpuSet {
    #[cfg(all(target_pointer_width = "32", not(target_arch = "x86_64")))]
    {
        RawCpuSet { bits: [0; 32] }
    }
    #[cfg(not(all(target_pointer_width = "32", not(target_arch = "x86_64"))))]
    {
        RawCpuSet { bits: [0; 16] }
    }
}

pub(crate) const CPU_SETSIZE: usize = 8 * core::mem::size_of::<RawCpuSet>();
