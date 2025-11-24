//! The Linux `membarrier` syscall.

use crate::process::Cpuid;
use crate::{backend, io};

pub use backend::process::types::MembarrierCommand;

#[cfg(linux_kernel)]
bitflags::bitflags! {
    /// A result from [`membarrier_query`].
    ///
    /// These flags correspond to values of [`MembarrierCommand`] which are
    /// supported in the OS.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MembarrierQuery: u32 {
        /// `MEMBARRIER_CMD_GLOBAL` (also known as `MEMBARRIER_CMD_SHARED`)
        #[doc(alias = "SHARED")]
        #[doc(alias = "MEMBARRIER_CMD_SHARED")]
        const GLOBAL = MembarrierCommand::Global as _;
        /// `MEMBARRIER_CMD_GLOBAL_EXPEDITED`
        const GLOBAL_EXPEDITED = MembarrierCommand::GlobalExpedited as _;
        /// `MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED`
        const REGISTER_GLOBAL_EXPEDITED = MembarrierCommand::RegisterGlobalExpedited as _;
        /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED`
        const PRIVATE_EXPEDITED = MembarrierCommand::PrivateExpedited as _;
        /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED`
        const REGISTER_PRIVATE_EXPEDITED = MembarrierCommand::RegisterPrivateExpedited as _;
        /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE`
        const PRIVATE_EXPEDITED_SYNC_CORE = MembarrierCommand::PrivateExpeditedSyncCore as _;
        /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE`
        const REGISTER_PRIVATE_EXPEDITED_SYNC_CORE = MembarrierCommand::RegisterPrivateExpeditedSyncCore as _;
        /// `MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
        const PRIVATE_EXPEDITED_RSEQ = MembarrierCommand::PrivateExpeditedRseq as _;
        /// `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ` (since Linux 5.10)
        const REGISTER_PRIVATE_EXPEDITED_RSEQ = MembarrierCommand::RegisterPrivateExpeditedRseq as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
impl MembarrierQuery {
    /// Test whether this query result contains the given command.
    #[inline]
    pub fn contains_command(self, cmd: MembarrierCommand) -> bool {
        // `MembarrierCommand` is an enum that only contains values also valid
        // in `MembarrierQuery`.
        self.contains(Self::from_bits_retain(cmd as _))
    }
}

/// `membarrier(MEMBARRIER_CMD_QUERY, 0, 0)`—Query the supported `membarrier`
/// commands.
///
/// This function doesn't return a `Result` because it always succeeds; if the
/// underlying OS doesn't support the `membarrier` syscall, it returns an empty
/// `MembarrierQuery` value.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/membarrier.2.html
#[inline]
#[doc(alias = "MEMBARRIER_CMD_QUERY")]
pub fn membarrier_query() -> MembarrierQuery {
    backend::process::syscalls::membarrier_query()
}

/// `membarrier(cmd, 0, 0)`—Perform a memory barrier.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/membarrier.2.html
#[inline]
pub fn membarrier(cmd: MembarrierCommand) -> io::Result<()> {
    backend::process::syscalls::membarrier(cmd)
}

/// `membarrier(cmd, MEMBARRIER_CMD_FLAG_CPU, cpu)`—Perform a memory barrier
/// with a specific CPU.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/membarrier.2.html
#[inline]
pub fn membarrier_cpu(cmd: MembarrierCommand, cpu: Cpuid) -> io::Result<()> {
    backend::process::syscalls::membarrier_cpu(cmd, cpu)
}
