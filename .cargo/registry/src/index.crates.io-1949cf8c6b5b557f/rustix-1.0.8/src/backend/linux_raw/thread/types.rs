use linux_raw_sys::general::membarrier_cmd;

/// A command for use with [`membarrier`] and [`membarrier_cpu`].
///
/// For `MEMBARRIER_CMD_QUERY`, see [`membarrier_query`].
///
/// [`membarrier`]: crate::thread::membarrier
/// [`membarrier_cpu`]: crate::thread::membarrier_cpu
/// [`membarrier_query`]: crate::thread::membarrier_query
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
#[non_exhaustive]
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

/// A CPU identifier as a raw integer.
pub type RawCpuid = u32;

#[repr(transparent)]
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
