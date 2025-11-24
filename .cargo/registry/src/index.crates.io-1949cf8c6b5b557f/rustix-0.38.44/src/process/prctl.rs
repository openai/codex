//! Bindings for the Linux `prctl` system call.
//!
//! There are similarities (but also differences) with FreeBSD's `procctl`
//! system call, whose interface is located in the `procctl.rs` file.

#![allow(unsafe_code)]

use core::mem::size_of;
use core::ptr::{null, null_mut, NonNull};

use bitflags::bitflags;

use crate::backend::c::{c_int, c_uint, c_void};
use crate::backend::prctl::syscalls;
use crate::fd::{AsRawFd, BorrowedFd};
use crate::ffi::CStr;
use crate::io;
use crate::prctl::*;
use crate::process::{Pid, RawPid};
use crate::signal::Signal;
use crate::utils::{as_mut_ptr, as_ptr};

//
// PR_GET_PDEATHSIG/PR_SET_PDEATHSIG
//

const PR_GET_PDEATHSIG: c_int = 2;

/// Get the current value of the parent process death signal.
///
/// # References
///  - [Linux: `prctl(PR_GET_PDEATHSIG,…)`]
///  - [FreeBSD: `procctl(PROC_PDEATHSIG_STATUS,…)`]
///
/// [Linux: `prctl(PR_GET_PDEATHSIG,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_PDEATHSIG_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
#[doc(alias = "PR_GET_PDEATHSIG")]
pub fn parent_process_death_signal() -> io::Result<Option<Signal>> {
    unsafe { prctl_get_at_arg2_optional::<c_int>(PR_GET_PDEATHSIG) }.map(Signal::from_raw)
}

const PR_SET_PDEATHSIG: c_int = 1;

/// Set the parent-death signal of the calling process.
///
/// # References
///  - [Linux: `prctl(PR_SET_PDEATHSIG,…)`]
///  - [FreeBSD: `procctl(PROC_PDEATHSIG_CTL,…)`]
///
/// [Linux: `prctl(PR_SET_PDEATHSIG,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_PDEATHSIG_CTL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
#[doc(alias = "PR_SET_PDEATHSIG")]
pub fn set_parent_process_death_signal(signal: Option<Signal>) -> io::Result<()> {
    let signal = signal.map_or(0_usize, |signal| signal as usize);
    unsafe { prctl_2args(PR_SET_PDEATHSIG, signal as *mut _) }.map(|_r| ())
}

//
// PR_GET_DUMPABLE/PR_SET_DUMPABLE
//

const PR_GET_DUMPABLE: c_int = 3;

const SUID_DUMP_DISABLE: i32 = 0;
const SUID_DUMP_USER: i32 = 1;
const SUID_DUMP_ROOT: i32 = 2;

/// `SUID_DUMP_*` values for use with [`dumpable_behavior`] and
/// [`set_dumpable_behavior`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DumpableBehavior {
    /// Not dumpable.
    #[doc(alias = "SUID_DUMP_DISABLE")]
    NotDumpable = SUID_DUMP_DISABLE,
    /// Dumpable.
    #[doc(alias = "SUID_DUMP_USER")]
    Dumpable = SUID_DUMP_USER,
    /// Dumpable but only readable by root.
    #[doc(alias = "SUID_DUMP_ROOT")]
    DumpableReadableOnlyByRoot = SUID_DUMP_ROOT,
}

impl TryFrom<i32> for DumpableBehavior {
    type Error = io::Errno;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            SUID_DUMP_DISABLE => Ok(Self::NotDumpable),
            SUID_DUMP_USER => Ok(Self::Dumpable),
            SUID_DUMP_ROOT => Ok(Self::DumpableReadableOnlyByRoot),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the current state of the calling process' `dumpable` attribute.
///
/// # References
///  - [`prctl(PR_GET_DUMPABLE,…)`]
///
/// [`prctl(PR_GET_DUMPABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_DUMPABLE")]
pub fn dumpable_behavior() -> io::Result<DumpableBehavior> {
    unsafe { prctl_1arg(PR_GET_DUMPABLE) }.and_then(TryInto::try_into)
}

const PR_SET_DUMPABLE: c_int = 4;

/// Set the state of the `dumpable` attribute.
///
/// This attribute determines whether the process can be traced and whether
/// core dumps are produced for the calling process upon delivery of a signal
/// whose default behavior is to produce a core dump.
///
/// A similar function with the same name is available on FreeBSD (as part of
/// the `procctl` interface), but it has an extra argument which allows to
/// select a process other then the current process.
///
/// # References
///  - [`prctl(PR_SET_DUMPABLE,…)`]
///
/// [`prctl(PR_SET_DUMPABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_DUMPABLE")]
pub fn set_dumpable_behavior(config: DumpableBehavior) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_DUMPABLE, config as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_UNALIGN/PR_SET_UNALIGN
//

const PR_GET_UNALIGN: c_int = 5;

bitflags! {
    /// `PR_UNALIGN_*` flags for use with [`unaligned_access_control`] and
    /// [`set_unaligned_access_control`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct UnalignedAccessControl: u32 {
        /// Silently fix up unaligned user accesses.
        #[doc(alias = "NOPRINT")]
        #[doc(alias = "PR_UNALIGN_NOPRINT")]
        const NO_PRINT = 1;
        /// Generate a [`Signal::Bus`] signal on unaligned user access.
        #[doc(alias = "PR_UNALIGN_SIGBUS")]
        const SIGBUS = 2;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// Get unaligned access control bits.
///
/// # References
///  - [`prctl(PR_GET_UNALIGN,…)`]
///
/// [`prctl(PR_GET_UNALIGN,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_UNALIGN")]
pub fn unaligned_access_control() -> io::Result<UnalignedAccessControl> {
    let r = unsafe { prctl_get_at_arg2_optional::<c_uint>(PR_GET_UNALIGN)? };
    UnalignedAccessControl::from_bits(r).ok_or(io::Errno::RANGE)
}

const PR_SET_UNALIGN: c_int = 6;

/// Set unaligned access control bits.
///
/// # References
///  - [`prctl(PR_SET_UNALIGN,…)`]
///
/// [`prctl(PR_SET_UNALIGN,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_UNALIGN")]
pub fn set_unaligned_access_control(config: UnalignedAccessControl) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_UNALIGN, config.bits() as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_FPEMU/PR_SET_FPEMU
//

const PR_GET_FPEMU: c_int = 9;

bitflags! {
    /// `PR_FPEMU_*` flags for use with [`floating_point_emulation_control`]
    /// and [`set_floating_point_emulation_control`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FloatingPointEmulationControl: u32 {
        /// Silently emulate floating point operations accesses.
        #[doc(alias = "PR_UNALIGN_NOPRINT")]
        const NO_PRINT = 1;
        /// Don't emulate floating point operations, send a [`Signal::Fpe`]
        /// signal instead.
        #[doc(alias = "PR_UNALIGN_SIGFPE")]
        const SIGFPE = 2;
    }
}

/// Get floating point emulation control bits.
///
/// # References
///  - [`prctl(PR_GET_FPEMU,…)`]
///
/// [`prctl(PR_GET_FPEMU,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_FPEMU")]
pub fn floating_point_emulation_control() -> io::Result<FloatingPointEmulationControl> {
    let r = unsafe { prctl_get_at_arg2_optional::<c_uint>(PR_GET_FPEMU)? };
    FloatingPointEmulationControl::from_bits(r).ok_or(io::Errno::RANGE)
}

const PR_SET_FPEMU: c_int = 10;

/// Set floating point emulation control bits.
///
/// # References
///  - [`prctl(PR_SET_FPEMU,…)`]
///
/// [`prctl(PR_SET_FPEMU,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_FPEMU")]
pub fn set_floating_point_emulation_control(
    config: FloatingPointEmulationControl,
) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_FPEMU, config.bits() as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_FPEXC/PR_SET_FPEXC
//

const PR_GET_FPEXC: c_int = 11;

bitflags! {
    /// Zero means floating point exceptions are disabled.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FloatingPointExceptionMode: u32 {
        /// Async non-recoverable exception mode.
        const NONRECOV = 1;
        /// Async recoverable exception mode.
        const ASYNC = 2;
        /// Precise exception mode.
        const PRECISE = 3;

        /// Use FPEXC for floating point exception enables.
        const SW_ENABLE = 0x80;
        /// Floating point divide by zero.
        const DIV = 0x01_0000;
        /// Floating point overflow.
        const OVF = 0x02_0000;
        /// Floating point underflow.
        const UND = 0x04_0000;
        /// Floating point inexact result.
        const RES = 0x08_0000;
        /// Floating point invalid operation.
        const INV = 0x10_0000;
    }
}

/// Get floating point exception mode.
///
/// # References
///  - [`prctl(PR_GET_FPEXC,…)`]
///
/// [`prctl(PR_GET_FPEXC,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_FPEXEC")]
pub fn floating_point_exception_mode() -> io::Result<Option<FloatingPointExceptionMode>> {
    unsafe { prctl_get_at_arg2_optional::<c_uint>(PR_GET_FPEXC) }
        .map(FloatingPointExceptionMode::from_bits)
}

const PR_SET_FPEXC: c_int = 12;

/// Set floating point exception mode.
///
/// # References
///  - [`prctl(PR_SET_FPEXC,…)`]
///
/// [`prctl(PR_SET_FPEXC,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_FPEXEC")]
pub fn set_floating_point_exception_mode(
    config: Option<FloatingPointExceptionMode>,
) -> io::Result<()> {
    let config = config.as_ref().map_or(0, FloatingPointExceptionMode::bits);
    unsafe { prctl_2args(PR_SET_FPEXC, config as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_TIMING/PR_SET_TIMING
//

const PR_GET_TIMING: c_int = 13;

const PR_TIMING_STATISTICAL: i32 = 0;
const PR_TIMING_TIMESTAMP: i32 = 1;

/// `PR_TIMING_*` values for use with [`timing_method`] and
/// [`set_timing_method`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum TimingMethod {
    /// Normal, traditional, statistical process timing.
    Statistical = PR_TIMING_STATISTICAL,
    /// Accurate timestamp based process timing.
    TimeStamp = PR_TIMING_TIMESTAMP,
}

impl TryFrom<i32> for TimingMethod {
    type Error = io::Errno;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            PR_TIMING_STATISTICAL => Ok(Self::Statistical),
            PR_TIMING_TIMESTAMP => Ok(Self::TimeStamp),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get which process timing method is currently in use.
///
/// # References
///  - [`prctl(PR_GET_TIMING,…)`]
///
/// [`prctl(PR_GET_TIMING,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_TIMING")]
pub fn timing_method() -> io::Result<TimingMethod> {
    unsafe { prctl_1arg(PR_GET_TIMING) }.and_then(TryInto::try_into)
}

const PR_SET_TIMING: c_int = 14;

/// Set whether to use (normal, traditional) statistical process timing or
/// accurate timestamp-based process timing.
///
/// # References
///  - [`prctl(PR_SET_TIMING,…)`]
///
/// [`prctl(PR_SET_TIMING,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_TIMING")]
pub fn set_timing_method(method: TimingMethod) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_TIMING, method as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_ENDIAN/PR_SET_ENDIAN
//

const PR_GET_ENDIAN: c_int = 19;

const PR_ENDIAN_BIG: u32 = 0;
const PR_ENDIAN_LITTLE: u32 = 1;
const PR_ENDIAN_PPC_LITTLE: u32 = 2;

/// `PR_ENDIAN_*` values for use with [`endian_mode`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EndianMode {
    /// Big endian mode.
    Big = PR_ENDIAN_BIG,
    /// True little endian mode.
    Little = PR_ENDIAN_LITTLE,
    /// `PowerPC` pseudo little endian.
    PowerPCLittle = PR_ENDIAN_PPC_LITTLE,
}

impl TryFrom<u32> for EndianMode {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_ENDIAN_BIG => Ok(Self::Big),
            PR_ENDIAN_LITTLE => Ok(Self::Little),
            PR_ENDIAN_PPC_LITTLE => Ok(Self::PowerPCLittle),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the endianness of the calling process.
///
/// # References
///  - [`prctl(PR_GET_ENDIAN,…)`]
///
/// [`prctl(PR_GET_ENDIAN,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_ENDIAN")]
pub fn endian_mode() -> io::Result<EndianMode> {
    unsafe { prctl_get_at_arg2::<c_uint, _>(PR_GET_ENDIAN) }
}

const PR_SET_ENDIAN: c_int = 20;

/// Set the endianness of the calling process.
///
/// # References
///  - [`prctl(PR_SET_ENDIAN,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_ENDIAN,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_ENDIAN")]
pub unsafe fn set_endian_mode(mode: EndianMode) -> io::Result<()> {
    prctl_2args(PR_SET_ENDIAN, mode as usize as *mut _).map(|_r| ())
}

//
// PR_GET_TSC/PR_SET_TSC
//

const PR_GET_TSC: c_int = 25;

const PR_TSC_ENABLE: u32 = 1;
const PR_TSC_SIGSEGV: u32 = 2;

/// `PR_TSC_*` values for use with [`time_stamp_counter_readability`] and
/// [`set_time_stamp_counter_readability`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TimeStampCounterReadability {
    /// Allow the use of the timestamp counter.
    Readable = PR_TSC_ENABLE,
    /// Throw a [`Signal::Segv`] signal instead of reading the TSC.
    RaiseSIGSEGV = PR_TSC_SIGSEGV,
}

impl TryFrom<u32> for TimeStampCounterReadability {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_TSC_ENABLE => Ok(Self::Readable),
            PR_TSC_SIGSEGV => Ok(Self::RaiseSIGSEGV),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the state of the flag determining if the timestamp counter can be read.
///
/// # References
///  - [`prctl(PR_GET_TSC,…)`]
///
/// [`prctl(PR_GET_TSC,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_TSC")]
pub fn time_stamp_counter_readability() -> io::Result<TimeStampCounterReadability> {
    unsafe { prctl_get_at_arg2::<c_uint, _>(PR_GET_TSC) }
}

const PR_SET_TSC: c_int = 26;

/// Set the state of the flag determining if the timestamp counter can be read
/// by the process.
///
/// # References
///  - [`prctl(PR_SET_TSC,…)`]
///
/// [`prctl(PR_SET_TSC,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_TSC")]
pub fn set_time_stamp_counter_readability(
    readability: TimeStampCounterReadability,
) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_TSC, readability as usize as *mut _) }.map(|_r| ())
}

//
// PR_TASK_PERF_EVENTS_DISABLE/PR_TASK_PERF_EVENTS_ENABLE
//

const PR_TASK_PERF_EVENTS_DISABLE: c_int = 31;
const PR_TASK_PERF_EVENTS_ENABLE: c_int = 32;

/// Enable or disable all performance counters attached to the calling process.
///
/// # References
///  - [`prctl(PR_TASK_PERF_EVENTS_ENABLE,…)`]
///  - [`prctl(PR_TASK_PERF_EVENTS_DISABLE,…)`]
///
/// [`prctl(PR_TASK_PERF_EVENTS_ENABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [`prctl(PR_TASK_PERF_EVENTS_DISABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_TASK_PERF_EVENTS_ENABLE")]
#[doc(alias = "PR_TASK_PERF_EVENTS_DISABLE")]
pub fn configure_performance_counters(enable: bool) -> io::Result<()> {
    let option = if enable {
        PR_TASK_PERF_EVENTS_ENABLE
    } else {
        PR_TASK_PERF_EVENTS_DISABLE
    };

    unsafe { prctl_1arg(option) }.map(|_r| ())
}

//
// PR_MCE_KILL_GET/PR_MCE_KILL
//

const PR_MCE_KILL_GET: c_int = 34;

const PR_MCE_KILL_LATE: u32 = 0;
const PR_MCE_KILL_EARLY: u32 = 1;
const PR_MCE_KILL_DEFAULT: u32 = 2;

/// `PR_MCE_KILL_*` values for use with
/// [`machine_check_memory_corruption_kill_policy`] and
/// [`set_machine_check_memory_corruption_kill_policy`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MachineCheckMemoryCorruptionKillPolicy {
    /// Late kill policy.
    #[doc(alias = "PR_MCE_KILL_LATE")]
    Late = PR_MCE_KILL_LATE,
    /// Early kill policy.
    #[doc(alias = "PR_MCE_KILL_EARLY")]
    Early = PR_MCE_KILL_EARLY,
    /// System-wide default policy.
    #[doc(alias = "PR_MCE_KILL_DEFAULT")]
    Default = PR_MCE_KILL_DEFAULT,
}

impl TryFrom<u32> for MachineCheckMemoryCorruptionKillPolicy {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_MCE_KILL_LATE => Ok(Self::Late),
            PR_MCE_KILL_EARLY => Ok(Self::Early),
            PR_MCE_KILL_DEFAULT => Ok(Self::Default),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the current per-process machine check kill policy.
///
/// # References
///  - [`prctl(PR_MCE_KILL_GET,…)`]
///
/// [`prctl(PR_MCE_KILL_GET,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_MCE_KILL_GET")]
pub fn machine_check_memory_corruption_kill_policy(
) -> io::Result<MachineCheckMemoryCorruptionKillPolicy> {
    let r = unsafe { prctl_1arg(PR_MCE_KILL_GET)? } as c_uint;
    MachineCheckMemoryCorruptionKillPolicy::try_from(r)
}

const PR_MCE_KILL: c_int = 33;

const PR_MCE_KILL_CLEAR: usize = 0;
const PR_MCE_KILL_SET: usize = 1;

/// Set the machine check memory corruption kill policy for the calling thread.
///
/// # References
///  - [`prctl(PR_MCE_KILL,…)`]
///
/// [`prctl(PR_MCE_KILL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_MCE_KILL")]
pub fn set_machine_check_memory_corruption_kill_policy(
    policy: Option<MachineCheckMemoryCorruptionKillPolicy>,
) -> io::Result<()> {
    let (sub_operation, policy) = if let Some(policy) = policy {
        (PR_MCE_KILL_SET, policy as usize as *mut _)
    } else {
        (PR_MCE_KILL_CLEAR, null_mut())
    };

    unsafe { prctl_3args(PR_MCE_KILL, sub_operation as *mut _, policy) }.map(|_r| ())
}

//
// PR_SET_MM
//

const PR_SET_MM: c_int = 35;

const PR_SET_MM_START_CODE: u32 = 1;
const PR_SET_MM_END_CODE: u32 = 2;
const PR_SET_MM_START_DATA: u32 = 3;
const PR_SET_MM_END_DATA: u32 = 4;
const PR_SET_MM_START_STACK: u32 = 5;
const PR_SET_MM_START_BRK: u32 = 6;
const PR_SET_MM_BRK: u32 = 7;
const PR_SET_MM_ARG_START: u32 = 8;
const PR_SET_MM_ARG_END: u32 = 9;
const PR_SET_MM_ENV_START: u32 = 10;
const PR_SET_MM_ENV_END: u32 = 11;
const PR_SET_MM_AUXV: usize = 12;
const PR_SET_MM_EXE_FILE: usize = 13;
const PR_SET_MM_MAP: usize = 14;
const PR_SET_MM_MAP_SIZE: usize = 15;

/// `PR_SET_MM_*` values for use with [`set_virtual_memory_map_address`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VirtualMemoryMapAddress {
    /// Set the address above which the program text can run.
    CodeStart = PR_SET_MM_START_CODE,
    /// Set the address below which the program text can run.
    CodeEnd = PR_SET_MM_END_CODE,
    /// Set the address above which initialized and uninitialized (bss) data
    /// are placed.
    DataStart = PR_SET_MM_START_DATA,
    /// Set the address below which initialized and uninitialized (bss) data
    /// are placed.
    DataEnd = PR_SET_MM_END_DATA,
    /// Set the start address of the stack.
    StackStart = PR_SET_MM_START_STACK,
    /// Set the address above which the program heap can be expanded with `brk`
    /// call.
    BrkStart = PR_SET_MM_START_BRK,
    /// Set the current `brk` value.
    BrkCurrent = PR_SET_MM_BRK,
    /// Set the address above which the program command line is placed.
    ArgStart = PR_SET_MM_ARG_START,
    /// Set the address below which the program command line is placed.
    ArgEnd = PR_SET_MM_ARG_END,
    /// Set the address above which the program environment is placed.
    EnvironmentStart = PR_SET_MM_ENV_START,
    /// Set the address below which the program environment is placed.
    EnvironmentEnd = PR_SET_MM_ENV_END,
}

/// Modify certain kernel memory map descriptor addresses of the calling
/// process.
///
/// # References
///  - [`prctl(PR_SET_MM,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_MM,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_MM")]
pub unsafe fn set_virtual_memory_map_address(
    option: VirtualMemoryMapAddress,
    address: Option<NonNull<c_void>>,
) -> io::Result<()> {
    let address = address.map_or_else(null_mut, NonNull::as_ptr);
    prctl_3args(PR_SET_MM, option as usize as *mut _, address).map(|_r| ())
}

/// Supersede the `/proc/pid/exe` symbolic link with a new one pointing to a
/// new executable file.
///
/// # References
///  - [`prctl(PR_SET_MM,PR_SET_MM_EXE_FILE,…)`]
///
/// [`prctl(PR_SET_MM,PR_SET_MM_EXE_FILE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_MM")]
#[doc(alias = "PR_SET_MM_EXE_FILE")]
pub fn set_executable_file(fd: BorrowedFd<'_>) -> io::Result<()> {
    let fd = usize::try_from(fd.as_raw_fd()).map_err(|_r| io::Errno::RANGE)?;
    unsafe { prctl_3args(PR_SET_MM, PR_SET_MM_EXE_FILE as *mut _, fd as *mut _) }.map(|_r| ())
}

/// Set a new auxiliary vector.
///
/// # References
///  - [`prctl(PR_SET_MM,PR_SET_MM_AUXV,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_MM,PR_SET_MM_AUXV,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_MM")]
#[doc(alias = "PR_SET_MM_AUXV")]
pub unsafe fn set_auxiliary_vector(auxv: &[*const c_void]) -> io::Result<()> {
    syscalls::prctl(
        PR_SET_MM,
        PR_SET_MM_AUXV as *mut _,
        auxv.as_ptr() as *mut _,
        auxv.len() as *mut _,
        null_mut(),
    )
    .map(|_r| ())
}

/// Get the size of the [`PrctlMmMap`] the kernel expects.
///
/// # References
///  - [`prctl(PR_SET_MM,PR_SET_MM_MAP_SIZE,…)`]
///
/// [`prctl(PR_SET_MM,PR_SET_MM_MAP_SIZE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_MM")]
#[doc(alias = "PR_SET_MM_MAP_SIZE")]
pub fn virtual_memory_map_config_struct_size() -> io::Result<usize> {
    let mut value: c_uint = 0;
    let value_ptr = as_mut_ptr(&mut value);
    unsafe { prctl_3args(PR_SET_MM, PR_SET_MM_MAP_SIZE as *mut _, value_ptr.cast())? };
    Ok(value as usize)
}

/// This structure provides new memory descriptor map which mostly modifies
/// `/proc/pid/stat[m]` output for a task.
/// This mostly done in a sake of checkpoint/restore functionality.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PrctlMmMap {
    /// Code section start address.
    pub start_code: u64,
    /// Code section end address.
    pub end_code: u64,
    /// Data section start address.
    pub start_data: u64,
    /// Data section end address.
    pub end_data: u64,
    /// `brk` start address.
    pub start_brk: u64,
    /// `brk` current address.
    pub brk: u64,
    /// Stack start address.
    pub start_stack: u64,
    /// Program command line start address.
    pub arg_start: u64,
    /// Program command line end address.
    pub arg_end: u64,
    /// Program environment start address.
    pub env_start: u64,
    /// Program environment end address.
    pub env_end: u64,
    /// Auxiliary vector start address.
    pub auxv: *mut u64,
    /// Auxiliary vector size.
    pub auxv_size: u32,
    /// File descriptor of executable file that was used to create this
    /// process.
    pub exe_fd: u32,
}

/// Provides one-shot access to all the addresses by passing in a
/// [`PrctlMmMap`].
///
/// # References
///  - [`prctl(PR_SET_MM,PR_SET_MM_MAP,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_MM,PR_SET_MM_MAP,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_MM")]
#[doc(alias = "PR_SET_MM_MAP")]
pub unsafe fn configure_virtual_memory_map(config: &PrctlMmMap) -> io::Result<()> {
    syscalls::prctl(
        PR_SET_MM,
        PR_SET_MM_MAP as *mut _,
        as_ptr(config) as *mut _,
        size_of::<PrctlMmMap>() as *mut _,
        null_mut(),
    )
    .map(|_r| ())
}

//
// PR_SET_PTRACER
//

const PR_SET_PTRACER: c_int = 0x59_61_6d_61;

const PR_SET_PTRACER_ANY: usize = usize::MAX;

/// Process ptracer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PTracer {
    /// None.
    None,
    /// Disable `ptrace` restrictions for the calling process.
    Any,
    /// Specific process.
    ProcessID(Pid),
}

/// Declare that the ptracer process can `ptrace` the calling process as if it
/// were a direct process ancestor.
///
/// # References
///  - [`prctl(PR_SET_PTRACER,…)`]
///
/// [`prctl(PR_SET_PTRACER,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_PTRACER")]
pub fn set_ptracer(tracer: PTracer) -> io::Result<()> {
    let pid = match tracer {
        PTracer::None => null_mut(),
        PTracer::Any => PR_SET_PTRACER_ANY as *mut _,
        PTracer::ProcessID(pid) => pid.as_raw_nonzero().get() as usize as *mut _,
    };

    unsafe { prctl_2args(PR_SET_PTRACER, pid) }.map(|_r| ())
}

//
// PR_GET_CHILD_SUBREAPER/PR_SET_CHILD_SUBREAPER
//

const PR_GET_CHILD_SUBREAPER: c_int = 37;

/// Get the `child subreaper` setting of the calling process.
///
/// # References
///  - [`prctl(PR_GET_CHILD_SUBREAPER,…)`]
///
/// [`prctl(PR_GET_CHILD_SUBREAPER,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_CHILD_SUBREAPER")]
pub fn child_subreaper() -> io::Result<Option<Pid>> {
    unsafe {
        let r = prctl_get_at_arg2_optional::<c_uint>(PR_GET_CHILD_SUBREAPER)?;
        Ok(Pid::from_raw(r as RawPid))
    }
}

const PR_SET_CHILD_SUBREAPER: c_int = 36;

/// Set the `child subreaper` attribute of the calling process.
///
/// # References
///  - [`prctl(PR_SET_CHILD_SUBREAPER,…)`]
///
/// [`prctl(PR_SET_CHILD_SUBREAPER,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_CHILD_SUBREAPER")]
pub fn set_child_subreaper(pid: Option<Pid>) -> io::Result<()> {
    let pid = pid.map_or(0_usize, |pid| pid.as_raw_nonzero().get() as usize);
    unsafe { prctl_2args(PR_SET_CHILD_SUBREAPER, pid as *mut _) }.map(|_r| ())
}

//
// PR_GET_FP_MODE/PR_SET_FP_MODE
//

const PR_GET_FP_MODE: c_int = 46;

const PR_FP_MODE_FR: u32 = 1_u32 << 0;
const PR_FP_MODE_FRE: u32 = 1_u32 << 1;

/// `PR_FP_MODE_*` values for use with [`floating_point_mode`] and
/// [`set_floating_point_mode`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FloatingPointMode {
    /// 64-bit floating point registers.
    FloatingPointRegisters = PR_FP_MODE_FR,
    /// Enable emulation of 32-bit floating-point mode.
    FloatingPointEmulation = PR_FP_MODE_FRE,
}

impl TryFrom<u32> for FloatingPointMode {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_FP_MODE_FR => Ok(Self::FloatingPointRegisters),
            PR_FP_MODE_FRE => Ok(Self::FloatingPointEmulation),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the current floating point mode.
///
/// # References
///  - [`prctl(PR_GET_FP_MODE,…)`]
///
/// [`prctl(PR_GET_FP_MODE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_FP_MODE")]
pub fn floating_point_mode() -> io::Result<FloatingPointMode> {
    let r = unsafe { prctl_1arg(PR_GET_FP_MODE)? } as c_uint;
    FloatingPointMode::try_from(r)
}

const PR_SET_FP_MODE: c_int = 45;

/// Allow control of the floating point mode from user space.
///
/// # References
///  - [`prctl(PR_SET_FP_MODE,…)`]
///
/// [`prctl(PR_SET_FP_MODE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_FP_MODE")]
pub fn set_floating_point_mode(mode: FloatingPointMode) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_FP_MODE, mode as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_SPECULATION_CTRL/PR_SET_SPECULATION_CTRL
//

const PR_GET_SPECULATION_CTRL: c_int = 52;

const PR_SPEC_STORE_BYPASS: u32 = 0;
const PR_SPEC_INDIRECT_BRANCH: u32 = 1;
const PR_SPEC_L1D_FLUSH: u32 = 2;

/// `PR_SPEC_*` values for use with [`speculative_feature_state`] and
/// [`control_speculative_feature`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SpeculationFeature {
    /// Set the state of the speculative store bypass misfeature.
    SpeculativeStoreBypass = PR_SPEC_STORE_BYPASS,
    /// Set the state of the indirect branch speculation misfeature.
    IndirectBranchSpeculation = PR_SPEC_INDIRECT_BRANCH,
    /// Flush L1D Cache on context switch out of the task.
    FlushL1DCacheOnContextSwitchOutOfTask = PR_SPEC_L1D_FLUSH,
}

impl TryFrom<u32> for SpeculationFeature {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_SPEC_STORE_BYPASS => Ok(Self::SpeculativeStoreBypass),
            PR_SPEC_INDIRECT_BRANCH => Ok(Self::IndirectBranchSpeculation),
            PR_SPEC_L1D_FLUSH => Ok(Self::FlushL1DCacheOnContextSwitchOutOfTask),
            _ => Err(io::Errno::RANGE),
        }
    }
}

bitflags! {
    /// `PR_SPEC_*` flags for use with [`control_speculative_feature`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SpeculationFeatureControl: u32 {
        /// The speculation feature is enabled, mitigation is disabled.
        const ENABLE = 1_u32 << 1;
        /// The speculation feature is disabled, mitigation is enabled.
        const DISABLE = 1_u32 << 2;
        /// The speculation feature is disabled, mitigation is enabled, and it
        /// cannot be undone.
        const FORCE_DISABLE = 1_u32 << 3;
        /// The speculation feature is disabled, mitigation is enabled, and the
        /// state will be cleared on `execve`.
        const DISABLE_NOEXEC = 1_u32 << 4;
    }
}

bitflags! {
    /// Zero means the processors are not vulnerable.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SpeculationFeatureState: u32 {
        /// Mitigation can be controlled per thread by
        /// `PR_SET_SPECULATION_CTRL`.
        const PRCTL = 1_u32 << 0;
        /// The speculation feature is enabled, mitigation is disabled.
        const ENABLE = 1_u32 << 1;
        /// The speculation feature is disabled, mitigation is enabled.
        const DISABLE = 1_u32 << 2;
        /// The speculation feature is disabled, mitigation is enabled, and it
        /// cannot be undone.
        const FORCE_DISABLE = 1_u32 << 3;
        /// The speculation feature is disabled, mitigation is enabled, and the
        /// state will be cleared on `execve`.
        const DISABLE_NOEXEC = 1_u32 << 4;
    }
}

/// Get the state of the speculation misfeature.
///
/// # References
///  - [`prctl(PR_GET_SPECULATION_CTRL,…)`]
///
/// [`prctl(PR_GET_SPECULATION_CTRL,…)`]: https://www.kernel.org/doc/html/v6.10/userspace-api/spec_ctrl.html
#[inline]
#[doc(alias = "PR_GET_SPECULATION_CTRL")]
pub fn speculative_feature_state(
    feature: SpeculationFeature,
) -> io::Result<Option<SpeculationFeatureState>> {
    let r = unsafe { prctl_2args(PR_GET_SPECULATION_CTRL, feature as usize as *mut _)? } as c_uint;
    Ok(SpeculationFeatureState::from_bits(r))
}

const PR_SET_SPECULATION_CTRL: c_int = 53;

/// Sets the state of the speculation misfeature.
///
/// # References
///  - [`prctl(PR_SET_SPECULATION_CTRL,…)`]
///
/// [`prctl(PR_SET_SPECULATION_CTRL,…)`]: https://www.kernel.org/doc/html/v6.10/userspace-api/spec_ctrl.html
#[inline]
#[doc(alias = "PR_SET_SPECULATION_CTRL")]
pub fn control_speculative_feature(
    feature: SpeculationFeature,
    config: SpeculationFeatureControl,
) -> io::Result<()> {
    let feature = feature as usize as *mut _;
    let config = config.bits() as usize as *mut _;
    unsafe { prctl_3args(PR_SET_SPECULATION_CTRL, feature, config) }.map(|_r| ())
}

//
// PR_GET_IO_FLUSHER/PR_SET_IO_FLUSHER
//

const PR_GET_IO_FLUSHER: c_int = 58;

/// Get the `IO_FLUSHER` state of the caller.
///
/// # References
///  - [`prctl(PR_GET_IO_FLUSHER,…)`]
///
/// [`prctl(PR_GET_IO_FLUSHER,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_GET_IO_FLUSHER")]
pub fn is_io_flusher() -> io::Result<bool> {
    unsafe { prctl_1arg(PR_GET_IO_FLUSHER) }.map(|r| r != 0)
}

const PR_SET_IO_FLUSHER: c_int = 57;

/// Put the process in the `IO_FLUSHER` state, allowing it to make progress
/// when allocating memory.
///
/// # References
///  - [`prctl(PR_SET_IO_FLUSHER,…)`]
///
/// [`prctl(PR_SET_IO_FLUSHER,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[doc(alias = "PR_SET_IO_FLUSHER")]
pub fn configure_io_flusher_behavior(enable: bool) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_IO_FLUSHER, usize::from(enable) as *mut _) }.map(|_r| ())
}

//
// PR_PAC_GET_ENABLED_KEYS/PR_PAC_SET_ENABLED_KEYS
//

const PR_PAC_GET_ENABLED_KEYS: c_int = 61;

/// Get enabled pointer authentication keys.
///
/// # References
///  - [`prctl(PR_PAC_GET_ENABLED_KEYS,…)`]
///
/// [`prctl(PR_PAC_GET_ENABLED_KEYS,…)`]: https://www.kernel.org/doc/html/v6.10/arch/arm64/pointer-authentication.html
#[inline]
#[doc(alias = "PR_PAC_GET_ENABLED_KEYS")]
pub fn enabled_pointer_authentication_keys() -> io::Result<PointerAuthenticationKeys> {
    let r = unsafe { prctl_1arg(PR_PAC_GET_ENABLED_KEYS)? } as c_uint;
    PointerAuthenticationKeys::from_bits(r).ok_or(io::Errno::RANGE)
}

const PR_PAC_SET_ENABLED_KEYS: c_int = 60;

/// Set enabled pointer authentication keys.
///
/// # References
///  - [`prctl(PR_PAC_SET_ENABLED_KEYS,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_PAC_SET_ENABLED_KEYS,…)`]: https://www.kernel.org/doc/html/v6.10/arch/arm64/pointer-authentication.html
#[inline]
#[doc(alias = "PR_PAC_SET_ENABLED_KEYS")]
pub unsafe fn configure_pointer_authentication_keys(
    config: impl Iterator<Item = (PointerAuthenticationKeys, bool)>,
) -> io::Result<()> {
    let mut affected_keys: u32 = 0;
    let mut enabled_keys: u32 = 0;

    for (key, enable) in config {
        let key = key.bits();
        affected_keys |= key;

        if enable {
            enabled_keys |= key;
        } else {
            enabled_keys &= !key;
        }
    }

    if affected_keys == 0 {
        return Ok(()); // Nothing to do.
    }

    prctl_3args(
        PR_PAC_SET_ENABLED_KEYS,
        affected_keys as usize as *mut _,
        enabled_keys as usize as *mut _,
    )
    .map(|_r| ())
}

//
// PR_SET_VMA
//

const PR_SET_VMA: c_int = 0x53_56_4d_41;

const PR_SET_VMA_ANON_NAME: usize = 0;

/// Set the name for a virtual memory region.
///
/// # References
///  - [`prctl(PR_SET_VMA,PR_SET_VMA_ANON_NAME,…)`]
///
/// [`prctl(PR_SET_VMA,PR_SET_VMA_ANON_NAME,…)`]: https://lwn.net/Articles/867818/
#[inline]
#[doc(alias = "PR_SET_VMA")]
#[doc(alias = "PR_SET_VMA_ANON_NAME")]
pub fn set_virtual_memory_region_name(region: &[u8], name: Option<&CStr>) -> io::Result<()> {
    unsafe {
        syscalls::prctl(
            PR_SET_VMA,
            PR_SET_VMA_ANON_NAME as *mut _,
            region.as_ptr() as *mut _,
            region.len() as *mut _,
            name.map_or_else(null, CStr::as_ptr) as *mut _,
        )
        .map(|_r| ())
    }
}
