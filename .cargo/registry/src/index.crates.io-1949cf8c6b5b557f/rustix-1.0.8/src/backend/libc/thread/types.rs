#[cfg(all(
    any(freebsdlike, linux_kernel, target_os = "fuchsia"),
    not(any(target_os = "espidf", target_os = "vita"))
))]
use crate::backend::c;

/// A command for use with [`membarrier`] and [`membarrier_cpu`].
///
/// For `MEMBARRIER_CMD_QUERY`, see [`membarrier_query`].
///
/// [`membarrier`]: crate::thread::membarrier
/// [`membarrier_cpu`]: crate::thread::membarrier_cpu
/// [`membarrier_query`]: crate::thread::membarrier_query
#[cfg(linux_kernel)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
#[non_exhaustive]
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

/// A CPU identifier as a raw integer.
#[cfg(linux_kernel)]
pub type RawCpuid = u32;

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
