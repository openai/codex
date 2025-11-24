//! Linux `prctl` wrappers.
//!
//! Rustix wraps variadic/dynamic-dispatch functions like `prctl` in type-safe
//! wrappers.
//!
//! # Safety
//!
//! The inner `prctl` calls are dynamically typed and must be called correctly.
#![allow(unsafe_code)]

use core::mem::MaybeUninit;
use core::num::NonZeroU64;
use core::ptr;
use core::ptr::NonNull;
use core::sync::atomic::AtomicU8;

use bitflags::bitflags;

use crate::backend::prctl::syscalls;
#[cfg(feature = "alloc")]
use crate::ffi::CString;
use crate::ffi::{c_int, c_uint, c_void, CStr};
use crate::io;
use crate::pid::Pid;
use crate::prctl::{
    prctl_1arg, prctl_2args, prctl_3args, prctl_get_at_arg2_optional, PointerAuthenticationKeys,
};
use crate::utils::as_ptr;

//
// PR_GET_KEEPCAPS/PR_SET_KEEPCAPS
//

const PR_GET_KEEPCAPS: c_int = 7;

/// Get the current state of the calling thread's `keep capabilities` flag.
///
/// # References
///  - [`prctl(PR_GET_KEEPCAPS,…)`]
///
/// [`prctl(PR_GET_KEEPCAPS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn get_keep_capabilities() -> io::Result<bool> {
    unsafe { prctl_1arg(PR_GET_KEEPCAPS) }.map(|r| r != 0)
}

const PR_SET_KEEPCAPS: c_int = 8;

/// Set the state of the calling thread's `keep capabilities` flag.
///
/// # References
///  - [`prctl(PR_SET_KEEPCAPS,…)`]
///
/// [`prctl(PR_SET_KEEPCAPS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_keep_capabilities(enable: bool) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_KEEPCAPS, usize::from(enable) as *mut _) }.map(|_r| ())
}

//
// PR_GET_NAME/PR_SET_NAME
//

#[cfg(feature = "alloc")]
const PR_GET_NAME: c_int = 16;

/// Get the name of the calling thread.
///
/// # References
///  - [`prctl(PR_GET_NAME,…)`]
///
/// [`prctl(PR_GET_NAME,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn name() -> io::Result<CString> {
    let mut buffer = [0_u8; 16];
    unsafe { prctl_2args(PR_GET_NAME, buffer.as_mut_ptr().cast())? };

    let len = buffer.iter().position(|&x| x == 0_u8).unwrap_or(0);
    CString::new(&buffer[..len]).map_err(|_r| io::Errno::ILSEQ)
}

const PR_SET_NAME: c_int = 15;

/// Set the name of the calling thread.
///
/// Unlike `pthread_setname_np`, this function silently truncates the name to
/// 16 bytes, as the Linux syscall does.
///
/// # References
///  - [`prctl(PR_SET_NAME,…)`]
///
/// [`prctl(PR_SET_NAME,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_name(name: &CStr) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_NAME, name.as_ptr() as *mut _) }.map(|_r| ())
}

//
// PR_GET_SECCOMP/PR_SET_SECCOMP
//

const PR_GET_SECCOMP: c_int = 21;

const SECCOMP_MODE_DISABLED: i32 = 0;
const SECCOMP_MODE_STRICT: i32 = 1;
const SECCOMP_MODE_FILTER: i32 = 2;

/// `SECCOMP_MODE_*`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SecureComputingMode {
    /// Secure computing is not in use.
    Disabled = SECCOMP_MODE_DISABLED,
    /// Use hard-coded filter.
    Strict = SECCOMP_MODE_STRICT,
    /// Use user-supplied filter.
    Filter = SECCOMP_MODE_FILTER,
}

impl TryFrom<i32> for SecureComputingMode {
    type Error = io::Errno;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            SECCOMP_MODE_DISABLED => Ok(Self::Disabled),
            SECCOMP_MODE_STRICT => Ok(Self::Strict),
            SECCOMP_MODE_FILTER => Ok(Self::Filter),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get the secure computing mode of the calling thread.
///
/// If the caller is not in secure computing mode, this returns
/// [`SecureComputingMode::Disabled`]. If the caller is in strict secure
/// computing mode, then this call will cause a [`Signal::KILL`] signal to be
/// sent to the process. If the caller is in filter mode, and this system call
/// is allowed by the seccomp filters, it returns
/// [`SecureComputingMode::Filter`]; otherwise, the process is killed with a
/// [`Signal::KILL`] signal.
///
/// Since Linux 3.8, the Seccomp field of the `/proc/[pid]/status` file
/// provides a method of obtaining the same information, without the risk that
/// the process is killed; see [the `proc` manual page].
///
/// # References
///  - [`prctl(PR_GET_SECCOMP,…)`]
///
/// [`Signal::KILL`]: crate::signal::Signal::KILL
/// [`prctl(PR_GET_SECCOMP,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [the `proc` manual page]: https://man7.org/linux/man-pages/man5/proc.5.html
#[inline]
pub fn secure_computing_mode() -> io::Result<SecureComputingMode> {
    unsafe { prctl_1arg(PR_GET_SECCOMP) }.and_then(TryInto::try_into)
}

const PR_SET_SECCOMP: c_int = 22;

/// Set the secure computing mode for the calling thread, to limit the
/// available system calls.
///
/// # References
///  - [`prctl(PR_SET_SECCOMP,…)`]
///
/// [`prctl(PR_SET_SECCOMP,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_secure_computing_mode(mode: SecureComputingMode) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_SECCOMP, mode as usize as *mut _) }.map(|_r| ())
}

//
// PR_CAPBSET_READ/PR_CAPBSET_DROP
//

const PR_CAPBSET_READ: c_int = 23;

/// Linux per-thread capability.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
#[non_exhaustive]
pub enum Capability {
    /// In a system with the `_POSIX_CHOWN_RESTRICTED` option defined, this
    /// overrides the restriction of changing file ownership and group
    /// ownership.
    ChangeOwnership = linux_raw_sys::general::CAP_CHOWN,
    /// Override all DAC access, including ACL execute access if `_POSIX_ACL`
    /// is defined. Excluding DAC access covered by
    /// [`Capability::LinuxImmutable`].
    DACOverride = linux_raw_sys::general::CAP_DAC_OVERRIDE,
    /// Overrides all DAC restrictions regarding read and search on files and
    /// directories, including ACL restrictions if `_POSIX_ACL` is defined.
    /// Excluding DAC access covered by [`Capability::LinuxImmutable`].
    DACReadSearch = linux_raw_sys::general::CAP_DAC_READ_SEARCH,
    /// Overrides all restrictions about allowed operations on files, where
    /// file owner ID must be equal to the user ID, except where
    /// [`Capability::FileSetID`] is applicable. It doesn't override MAC and
    /// DAC restrictions.
    FileOwner = linux_raw_sys::general::CAP_FOWNER,
    /// Overrides the following restrictions that the effective user ID shall
    /// match the file owner ID when setting the `S_ISUID` and `S_ISGID` bits
    /// on that file; that the effective group ID (or one of the supplementary
    /// group IDs) shall match the file owner ID when setting the `S_ISGID` bit
    /// on that file; that the `S_ISUID` and `S_ISGID` bits are cleared on
    /// successful return from `chown` (not implemented).
    FileSetID = linux_raw_sys::general::CAP_FSETID,
    /// Overrides the restriction that the real or effective user ID of a
    /// process sending a signal must match the real or effective user ID of
    /// the process receiving the signal.
    Kill = linux_raw_sys::general::CAP_KILL,
    /// Allows `setgid` manipulation. Allows `setgroups`. Allows forged gids on
    /// socket credentials passing.
    SetGroupID = linux_raw_sys::general::CAP_SETGID,
    /// Allows `set*uid` manipulation (including fsuid). Allows forged pids on
    /// socket credentials passing.
    SetUserID = linux_raw_sys::general::CAP_SETUID,
    /// Without VFS support for capabilities:
    ///  - Transfer any capability in your permitted set to any pid.
    ///  - remove any capability in your permitted set from any pid. With VFS
    ///    support for capabilities (neither of above, but)
    ///  - Add any capability from current's capability bounding set to the
    ///    current process' inheritable set.
    ///  - Allow taking bits out of capability bounding set.
    ///  - Allow modification of the securebits for a process.
    SetPermittedCapabilities = linux_raw_sys::general::CAP_SETPCAP,
    /// Allow modification of `S_IMMUTABLE` and `S_APPEND` file attributes.
    LinuxImmutable = linux_raw_sys::general::CAP_LINUX_IMMUTABLE,
    /// Allows binding to TCP/UDP sockets below 1024. Allows binding to ATM
    /// VCIs below 32.
    NetBindService = linux_raw_sys::general::CAP_NET_BIND_SERVICE,
    /// Allow broadcasting, listen to multicast.
    NetBroadcast = linux_raw_sys::general::CAP_NET_BROADCAST,
    /// Allow interface configuration. Allow administration of IP firewall,
    /// masquerading and accounting. Allow setting debug option on sockets.
    /// Allow modification of routing tables. Allow setting arbitrary
    /// process / process group ownership on sockets. Allow binding to any
    /// address for transparent proxying (also via [`Capability::NetRaw`]).
    /// Allow setting TOS (type of service). Allow setting promiscuous
    /// mode. Allow clearing driver statistics. Allow multicasting. Allow
    /// read/write of device-specific registers. Allow activation of ATM
    /// control sockets.
    NetAdmin = linux_raw_sys::general::CAP_NET_ADMIN,
    /// Allow use of `RAW` sockets. Allow use of `PACKET` sockets. Allow
    /// binding to any address for transparent proxying (also via
    /// [`Capability::NetAdmin`]).
    NetRaw = linux_raw_sys::general::CAP_NET_RAW,
    /// Allow locking of shared memory segments. Allow mlock and mlockall
    /// (which doesn't really have anything to do with IPC).
    IPCLock = linux_raw_sys::general::CAP_IPC_LOCK,
    /// Override IPC ownership checks.
    IPCOwner = linux_raw_sys::general::CAP_IPC_OWNER,
    /// Insert and remove kernel modules - modify kernel without limit.
    SystemModule = linux_raw_sys::general::CAP_SYS_MODULE,
    /// Allow ioperm/iopl access. Allow sending USB messages to any device via
    /// `/dev/bus/usb`.
    SystemRawIO = linux_raw_sys::general::CAP_SYS_RAWIO,
    /// Allow use of `chroot`.
    SystemChangeRoot = linux_raw_sys::general::CAP_SYS_CHROOT,
    /// Allow `ptrace` of any process.
    SystemProcessTrace = linux_raw_sys::general::CAP_SYS_PTRACE,
    /// Allow configuration of process accounting.
    SystemProcessAccounting = linux_raw_sys::general::CAP_SYS_PACCT,
    /// Allow configuration of the secure attention key. Allow administration
    /// of the random device. Allow examination and configuration of disk
    /// quotas. Allow setting the domainname. Allow setting the hostname.
    /// Allow `mount` and `umount`, setting up new smb connection.
    /// Allow some autofs root ioctls. Allow nfsservctl. Allow
    /// `VM86_REQUEST_IRQ`. Allow to read/write pci config on alpha. Allow
    /// `irix_prctl` on mips (setstacksize). Allow flushing all cache on
    /// m68k (`sys_cacheflush`). Allow removing semaphores. Used instead of
    /// [`Capability::ChangeOwnership`] to "chown" IPC message queues,
    /// semaphores and shared memory. Allow locking/unlocking of shared
    /// memory segment. Allow turning swap on/off. Allow forged pids on
    /// socket credentials passing. Allow setting readahead and
    /// flushing buffers on block devices. Allow setting geometry in floppy
    /// driver. Allow turning DMA on/off in `xd` driver. Allow
    /// administration of md devices (mostly the above, but some
    /// extra ioctls). Allow tuning the ide driver. Allow access to the nvram
    /// device. Allow administration of `apm_bios`, serial and bttv (TV)
    /// device. Allow manufacturer commands in isdn CAPI support driver.
    /// Allow reading non-standardized portions of pci configuration space.
    /// Allow DDI debug ioctl on sbpcd driver. Allow setting up serial ports.
    /// Allow sending raw qic-117 commands. Allow enabling/disabling tagged
    /// queuing on SCSI controllers and sending arbitrary SCSI commands.
    /// Allow setting encryption key on loopback filesystem. Allow setting
    /// zone reclaim policy. Allow everything under
    /// [`Capability::BerkeleyPacketFilters`] and
    /// [`Capability::PerformanceMonitoring`] for backward compatibility.
    SystemAdmin = linux_raw_sys::general::CAP_SYS_ADMIN,
    /// Allow use of `reboot`.
    SystemBoot = linux_raw_sys::general::CAP_SYS_BOOT,
    /// Allow raising priority and setting priority on other (different UID)
    /// processes. Allow use of FIFO and round-robin (realtime) scheduling
    /// on own processes and setting the scheduling algorithm used by
    /// another process. Allow setting cpu affinity on other processes.
    /// Allow setting realtime ioprio class. Allow setting ioprio class on
    /// other processes.
    SystemNice = linux_raw_sys::general::CAP_SYS_NICE,
    /// Override resource limits. Set resource limits. Override quota limits.
    /// Override reserved space on ext2 filesystem. Modify data journaling
    /// mode on ext3 filesystem (uses journaling resources). NOTE: ext2
    /// honors fsuid when checking for resource overrides, so you can
    /// override using fsuid too. Override size restrictions on IPC message
    /// queues. Allow more than 64hz interrupts from the real-time clock.
    /// Override max number of consoles on console allocation. Override max
    /// number of keymaps. Control memory reclaim behavior.
    SystemResource = linux_raw_sys::general::CAP_SYS_RESOURCE,
    /// Allow manipulation of system clock. Allow `irix_stime` on mips. Allow
    /// setting the real-time clock.
    SystemTime = linux_raw_sys::general::CAP_SYS_TIME,
    /// Allow configuration of tty devices. Allow `vhangup` of tty.
    SystemTTYConfig = linux_raw_sys::general::CAP_SYS_TTY_CONFIG,
    /// Allow the privileged aspects of `mknod`.
    MakeNode = linux_raw_sys::general::CAP_MKNOD,
    /// Allow taking of leases on files.
    Lease = linux_raw_sys::general::CAP_LEASE,
    /// Allow writing the audit log via unicast netlink socket.
    AuditWrite = linux_raw_sys::general::CAP_AUDIT_WRITE,
    /// Allow configuration of audit via unicast netlink socket.
    AuditControl = linux_raw_sys::general::CAP_AUDIT_CONTROL,
    /// Set or remove capabilities on files. Map `uid=0` into a child user
    /// namespace.
    SetFileCapabilities = linux_raw_sys::general::CAP_SETFCAP,
    /// Override MAC access. The base kernel enforces no MAC policy. An LSM may
    /// enforce a MAC policy, and if it does and it chooses to implement
    /// capability based overrides of that policy, this is the capability it
    /// should use to do so.
    MACOverride = linux_raw_sys::general::CAP_MAC_OVERRIDE,
    /// Allow MAC configuration or state changes. The base kernel requires no
    /// MAC configuration. An LSM may enforce a MAC policy, and if it does and
    /// it chooses to implement capability based checks on modifications to
    /// that policy or the data required to maintain it, this is the capability
    /// it should use to do so.
    MACAdmin = linux_raw_sys::general::CAP_MAC_ADMIN,
    /// Allow configuring the kernel's `syslog` (`printk` behaviour).
    SystemLog = linux_raw_sys::general::CAP_SYSLOG,
    /// Allow triggering something that will wake the system.
    WakeAlarm = linux_raw_sys::general::CAP_WAKE_ALARM,
    /// Allow preventing system suspends.
    BlockSuspend = linux_raw_sys::general::CAP_BLOCK_SUSPEND,
    /// Allow reading the audit log via multicast netlink socket.
    AuditRead = linux_raw_sys::general::CAP_AUDIT_READ,
    /// Allow system performance and observability privileged operations using
    /// `perf_events`, `i915_perf` and other kernel subsystems.
    PerformanceMonitoring = linux_raw_sys::general::CAP_PERFMON,
    /// This capability allows the following BPF operations:
    ///  - Creating all types of BPF maps
    ///  - Advanced verifier features
    ///     - Indirect variable access
    ///     - Bounded loops
    ///     - BPF to BPF function calls
    ///     - Scalar precision tracking
    ///     - Larger complexity limits
    ///     - Dead code elimination
    ///     - And potentially other features
    ///  - Loading BPF Type Format (BTF) data
    ///  - Retrieve `xlated` and JITed code of BPF programs
    ///  - Use `bpf_spin_lock` helper
    ///
    /// [`Capability::PerformanceMonitoring`] relaxes the verifier checks
    /// further:
    ///  - BPF progs can use of pointer-to-integer conversions
    ///  - speculation attack hardening measures are bypassed
    ///  - `bpf_probe_read` to read arbitrary kernel memory is allowed
    ///  - `bpf_trace_printk` to print kernel memory is allowed
    ///
    /// [`Capability::SystemAdmin`] is required to use `bpf_probe_write_user`.
    ///
    /// [`Capability::SystemAdmin`] is required to iterate system-wide loaded
    /// programs, maps, links, and BTFs, and convert their IDs to file
    /// descriptors.
    ///
    /// [`Capability::PerformanceMonitoring`] and
    /// [`Capability::BerkeleyPacketFilters`] are required to load tracing
    /// programs. [`Capability::NetAdmin`] and
    /// [`Capability::BerkeleyPacketFilters`] are required to load
    /// networking programs.
    BerkeleyPacketFilters = linux_raw_sys::general::CAP_BPF,
    /// Allow checkpoint/restore related operations. Allow PID selection during
    /// `clone3`. Allow writing to `ns_last_pid`.
    CheckpointRestore = linux_raw_sys::general::CAP_CHECKPOINT_RESTORE,
}

/// Check if the specified capability is in the calling thread's capability
/// bounding set.
///
/// # References
///  - [`prctl(PR_CAPBSET_READ,…)`]
///
/// [`prctl(PR_CAPBSET_READ,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn capability_is_in_bounding_set(capability: Capability) -> io::Result<bool> {
    unsafe { prctl_2args(PR_CAPBSET_READ, capability as usize as *mut _) }.map(|r| r != 0)
}

const PR_CAPBSET_DROP: c_int = 24;

/// If the calling thread has the [`Capability::SetPermittedCapabilities`]
/// capability within its user namespace, then drop the specified capability
/// from the thread's capability bounding set.
///
/// # References
///  - [`prctl(PR_CAPBSET_DROP,…)`]
///
/// [`prctl(PR_CAPBSET_DROP,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn remove_capability_from_bounding_set(capability: Capability) -> io::Result<()> {
    unsafe { prctl_2args(PR_CAPBSET_DROP, capability as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_SECUREBITS/PR_SET_SECUREBITS
//

const PR_GET_SECUREBITS: c_int = 27;

bitflags! {
    /// `SECBIT_*`
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CapabilitiesSecureBits: u32 {
        /// If this bit is set, then the kernel does not grant capabilities
        /// when a `set-user-ID-root` program is executed, or when a process
        /// with an effective or real UID of [`Uid::ROOT`] calls `execve`.
        const NO_ROOT = 1_u32 << 0;
        /// Set [`NO_ROOT`] irreversibly.
        ///
        /// [`NO_ROOT`]: Self::NO_ROOT
        const NO_ROOT_LOCKED = 1_u32 << 1;
        /// Setting this flag stops the kernel from adjusting the process'
        /// permitted, effective, and ambient capability sets when the thread's
        /// effective and filesystem UIDs are switched between zero and nonzero
        /// values.
        const NO_SETUID_FIXUP = 1_u32 << 2;
        /// Set [`NO_SETUID_FIXUP`] irreversibly.
        ///
        /// [`NO_SETUID_FIXUP`]: Self::NO_SETUID_FIXUP
        const NO_SETUID_FIXUP_LOCKED = 1_u32 << 3;
        /// Setting this flag allows a thread that has one or more 0 UIDs to
        /// retain capabilities in its permitted set when it switches all of
        /// its UIDs to nonzero values.
        const KEEP_CAPS = 1_u32 << 4;
        /// Set [`KEEP_CAPS`] irreversibly.
        ///
        /// [`KEEP_CAPS`]: Self::KEEP_CAPS
        const KEEP_CAPS_LOCKED = 1_u32 << 5;
        /// Setting this flag disallows raising ambient capabilities via the
        /// `prctl`'s `PR_CAP_AMBIENT_RAISE` operation.
        const NO_CAP_AMBIENT_RAISE = 1_u32 << 6;
        /// Set [`NO_CAP_AMBIENT_RAISE`] irreversibly.
        ///
        /// [`NO_CAP_AMBIENT_RAISE`]: Self::NO_CAP_AMBIENT_RAISE
        const NO_CAP_AMBIENT_RAISE_LOCKED = 1_u32 << 7;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// Get the `securebits` flags of the calling thread.
///
/// # References
///  - [`prctl(PR_GET_SECUREBITS,…)`]
///
/// [`prctl(PR_GET_SECUREBITS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn capabilities_secure_bits() -> io::Result<CapabilitiesSecureBits> {
    let r = unsafe { prctl_1arg(PR_GET_SECUREBITS)? } as c_uint;
    CapabilitiesSecureBits::from_bits(r).ok_or(io::Errno::RANGE)
}

const PR_SET_SECUREBITS: c_int = 28;

/// Set the `securebits` flags of the calling thread.
///
/// # References
///  - [`prctl(PR_SET_SECUREBITS,…)`]
///
/// [`prctl(PR_SET_SECUREBITS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_capabilities_secure_bits(bits: CapabilitiesSecureBits) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_SECUREBITS, bits.bits() as usize as *mut _) }.map(|_r| ())
}

//
// PR_GET_TIMERSLACK/PR_SET_TIMERSLACK
//

const PR_GET_TIMERSLACK: c_int = 30;

/// Get the `current` timer slack value of the calling thread.
///
/// # References
///  - [`prctl(PR_GET_TIMERSLACK,…)`]
///
/// [`prctl(PR_GET_TIMERSLACK,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn current_timer_slack() -> io::Result<u64> {
    unsafe { prctl_1arg(PR_GET_TIMERSLACK) }.map(|r| r as u64)
}

const PR_SET_TIMERSLACK: c_int = 29;

/// Sets the `current` timer slack value for the calling thread.
///
/// # References
///  - [`prctl(PR_SET_TIMERSLACK,…)`]
///
/// [`prctl(PR_SET_TIMERSLACK,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_current_timer_slack(value: Option<NonZeroU64>) -> io::Result<()> {
    let value = usize::try_from(value.map_or(0, NonZeroU64::get)).map_err(|_r| io::Errno::RANGE)?;
    unsafe { prctl_2args(PR_SET_TIMERSLACK, value as *mut _) }.map(|_r| ())
}

//
// PR_GET_NO_NEW_PRIVS/PR_SET_NO_NEW_PRIVS
//

const PR_GET_NO_NEW_PRIVS: c_int = 39;

/// Get the value of the `no_new_privs` attribute for the calling thread.
///
/// # References
///  - [`prctl(PR_GET_NO_NEW_PRIVS,…)`]
///
/// [`prctl(PR_GET_NO_NEW_PRIVS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn no_new_privs() -> io::Result<bool> {
    unsafe { prctl_1arg(PR_GET_NO_NEW_PRIVS) }.map(|r| r != 0)
}

const PR_SET_NO_NEW_PRIVS: c_int = 38;

/// Set the calling thread's `no_new_privs` attribute.
///
/// # References
///  - [`prctl(PR_SET_NO_NEW_PRIVS,…)`]
///
/// [`prctl(PR_SET_NO_NEW_PRIVS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn set_no_new_privs(no_new_privs: bool) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_NO_NEW_PRIVS, usize::from(no_new_privs) as *mut _) }.map(|_r| ())
}

//
// PR_GET_TID_ADDRESS
//

const PR_GET_TID_ADDRESS: c_int = 40;

/// Get the `clear_child_tid` address set by `set_tid_address`
/// and `clone`'s `CLONE_CHILD_CLEARTID` flag.
///
/// # References
///  - [`prctl(PR_GET_TID_ADDRESS,…)`]
///
/// [`prctl(PR_GET_TID_ADDRESS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn get_clear_child_tid_address() -> io::Result<Option<NonNull<c_void>>> {
    unsafe { prctl_get_at_arg2_optional::<*mut c_void>(PR_GET_TID_ADDRESS) }.map(NonNull::new)
}

//
// PR_GET_THP_DISABLE/PR_SET_THP_DISABLE
//

const PR_GET_THP_DISABLE: c_int = 42;

/// Get the current setting of the `THP disable` flag for the calling thread.
///
/// # References
///  - [`prctl(PR_GET_THP_DISABLE,…)`]
///
/// [`prctl(PR_GET_THP_DISABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn transparent_huge_pages_are_disabled() -> io::Result<bool> {
    unsafe { prctl_1arg(PR_GET_THP_DISABLE) }.map(|r| r != 0)
}

const PR_SET_THP_DISABLE: c_int = 41;

/// Set the state of the `THP disable` flag for the calling thread.
///
/// # References
///  - [`prctl(PR_SET_THP_DISABLE,…)`]
///
/// [`prctl(PR_SET_THP_DISABLE,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn disable_transparent_huge_pages(thp_disable: bool) -> io::Result<()> {
    unsafe { prctl_2args(PR_SET_THP_DISABLE, usize::from(thp_disable) as *mut _) }.map(|_r| ())
}

//
// PR_CAP_AMBIENT
//

const PR_CAP_AMBIENT: c_int = 47;

const PR_CAP_AMBIENT_IS_SET: usize = 1;

/// Check if the specified capability is in the ambient set.
///
/// # References
///  - [`prctl(PR_CAP_AMBIENT,PR_CAP_AMBIENT_IS_SET,…)`]
///
/// [`prctl(PR_CAP_AMBIENT,PR_CAP_AMBIENT_IS_SET,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn capability_is_in_ambient_set(capability: Capability) -> io::Result<bool> {
    let cap = capability as usize as *mut _;
    unsafe { prctl_3args(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET as *mut _, cap) }.map(|r| r != 0)
}

const PR_CAP_AMBIENT_CLEAR_ALL: usize = 4;

/// Remove all capabilities from the ambient set.
///
/// # References
///  - [`prctl(PR_CAP_AMBIENT,PR_CAP_AMBIENT_CLEAR_ALL,…)`]
///
/// [`prctl(PR_CAP_AMBIENT,PR_CAP_AMBIENT_CLEAR_ALL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn clear_ambient_capability_set() -> io::Result<()> {
    unsafe { prctl_2args(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL as *mut _) }.map(|_r| ())
}

const PR_CAP_AMBIENT_RAISE: usize = 2;
const PR_CAP_AMBIENT_LOWER: usize = 3;

/// Add or remove the specified capability to the ambient set.
///
/// # References
///  - [`prctl(PR_CAP_AMBIENT,…)`]
///
/// [`prctl(PR_CAP_AMBIENT,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn configure_capability_in_ambient_set(capability: Capability, enable: bool) -> io::Result<()> {
    let sub_operation = if enable {
        PR_CAP_AMBIENT_RAISE
    } else {
        PR_CAP_AMBIENT_LOWER
    };
    let cap = capability as usize as *mut _;

    unsafe { prctl_3args(PR_CAP_AMBIENT, sub_operation as *mut _, cap) }.map(|_r| ())
}

//
// PR_SVE_GET_VL/PR_SVE_SET_VL
//

const PR_SVE_GET_VL: c_int = 51;

const PR_SVE_VL_LEN_MASK: u32 = 0xffff;
const PR_SVE_VL_INHERIT: u32 = 1_u32 << 17;

/// Scalable Vector Extension vector length configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SVEVectorLengthConfig {
    /// Vector length in bytes.
    pub vector_length_in_bytes: u32,
    /// Vector length inherited across `execve`.
    pub vector_length_inherited_across_execve: bool,
}

/// Get the thread's current SVE vector length configuration.
///
/// # References
///  - [`prctl(PR_SVE_GET_VL,…)`]
///
/// [`prctl(PR_SVE_GET_VL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn sve_vector_length_configuration() -> io::Result<SVEVectorLengthConfig> {
    let bits = unsafe { prctl_1arg(PR_SVE_GET_VL)? } as c_uint;
    Ok(SVEVectorLengthConfig {
        vector_length_in_bytes: bits & PR_SVE_VL_LEN_MASK,
        vector_length_inherited_across_execve: (bits & PR_SVE_VL_INHERIT) != 0,
    })
}

const PR_SVE_SET_VL: c_int = 50;

const PR_SVE_SET_VL_ONEXEC: u32 = 1_u32 << 18;

/// Configure the thread's vector length of Scalable Vector Extension.
///
/// # References
///  - [`prctl(PR_SVE_SET_VL,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function,
/// as detailed in the references above.
///
/// [`prctl(PR_SVE_SET_VL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub unsafe fn set_sve_vector_length_configuration(
    vector_length_in_bytes: usize,
    vector_length_inherited_across_execve: bool,
    defer_change_to_next_execve: bool,
) -> io::Result<()> {
    let vector_length_in_bytes =
        u32::try_from(vector_length_in_bytes).map_err(|_r| io::Errno::RANGE)?;

    let mut bits = vector_length_in_bytes & PR_SVE_VL_LEN_MASK;

    if vector_length_inherited_across_execve {
        bits |= PR_SVE_VL_INHERIT;
    }

    if defer_change_to_next_execve {
        bits |= PR_SVE_SET_VL_ONEXEC;
    }

    prctl_2args(PR_SVE_SET_VL, bits as usize as *mut _).map(|_r| ())
}

//
// PR_PAC_RESET_KEYS
//

const PR_PAC_RESET_KEYS: c_int = 54;

/// Securely reset the thread's pointer authentication keys to fresh random
/// values generated by the kernel.
///
/// # References
///  - [`prctl(PR_PAC_RESET_KEYS,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function,
/// as detailed in the references above.
///
/// [`prctl(PR_PAC_RESET_KEYS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub unsafe fn reset_pointer_authentication_keys(
    keys: Option<PointerAuthenticationKeys>,
) -> io::Result<()> {
    let keys = keys.as_ref().map_or(0_u32, PointerAuthenticationKeys::bits);
    prctl_2args(PR_PAC_RESET_KEYS, keys as usize as *mut _).map(|_r| ())
}

//
// PR_GET_TAGGED_ADDR_CTRL/PR_SET_TAGGED_ADDR_CTRL
//

const PR_GET_TAGGED_ADDR_CTRL: c_int = 56;

const PR_MTE_TAG_SHIFT: u32 = 3;
const PR_MTE_TAG_MASK: u32 = 0xffff_u32 << PR_MTE_TAG_SHIFT;

bitflags! {
    /// Zero means addresses that are passed for the purpose of being
    /// dereferenced by the kernel must be untagged.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct TaggedAddressMode: u32 {
        /// Addresses that are passed for the purpose of being dereferenced by
        /// the kernel may be tagged.
        const ENABLED = 1_u32 << 0;
        /// Synchronous tag check fault mode.
        const TCF_SYNC = 1_u32 << 1;
        /// Asynchronous tag check fault mode.
        const TCF_ASYNC = 1_u32 << 2;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// Get the current tagged address mode for the calling thread.
///
/// # References
///  - [`prctl(PR_GET_TAGGED_ADDR_CTRL,…)`]
///
/// [`prctl(PR_GET_TAGGED_ADDR_CTRL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub fn current_tagged_address_mode() -> io::Result<(Option<TaggedAddressMode>, u32)> {
    let r = unsafe { prctl_1arg(PR_GET_TAGGED_ADDR_CTRL)? } as c_uint;
    let mode = r & 0b111_u32;
    let mte_tag = (r & PR_MTE_TAG_MASK) >> PR_MTE_TAG_SHIFT;
    Ok((TaggedAddressMode::from_bits(mode), mte_tag))
}

const PR_SET_TAGGED_ADDR_CTRL: c_int = 55;

/// Controls support for passing tagged user-space addresses to the kernel.
///
/// # References
///  - [`prctl(PR_SET_TAGGED_ADDR_CTRL,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_TAGGED_ADDR_CTRL,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub unsafe fn set_current_tagged_address_mode(
    mode: Option<TaggedAddressMode>,
    mte_tag: u32,
) -> io::Result<()> {
    let config = mode.as_ref().map_or(0_u32, TaggedAddressMode::bits)
        | ((mte_tag << PR_MTE_TAG_SHIFT) & PR_MTE_TAG_MASK);
    prctl_2args(PR_SET_TAGGED_ADDR_CTRL, config as usize as *mut _).map(|_r| ())
}

//
// PR_SET_SYSCALL_USER_DISPATCH
//

const PR_SET_SYSCALL_USER_DISPATCH: c_int = 59;

const PR_SYS_DISPATCH_OFF: usize = 0;

/// Disable Syscall User Dispatch mechanism.
///
/// # References
///  - [`prctl(PR_SET_SYSCALL_USER_DISPATCH,PR_SYS_DISPATCH_OFF,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_SYSCALL_USER_DISPATCH,PR_SYS_DISPATCH_OFF,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub unsafe fn disable_syscall_user_dispatch() -> io::Result<()> {
    prctl_2args(PR_SET_SYSCALL_USER_DISPATCH, PR_SYS_DISPATCH_OFF as *mut _).map(|_r| ())
}

const PR_SYS_DISPATCH_ON: usize = 1;

/// Allow system calls to be executed.
const SYSCALL_DISPATCH_FILTER_ALLOW: u8 = 0;
/// Block system calls from executing.
const SYSCALL_DISPATCH_FILTER_BLOCK: u8 = 1;

/// Value of the fast switch flag controlling system calls user dispatch
/// mechanism without the need to issue a syscall.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SysCallUserDispatchFastSwitch {
    /// System calls are allowed to execute.
    Allow = SYSCALL_DISPATCH_FILTER_ALLOW,
    /// System calls are blocked from executing.
    Block = SYSCALL_DISPATCH_FILTER_BLOCK,
}

impl TryFrom<u8> for SysCallUserDispatchFastSwitch {
    type Error = io::Errno;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            SYSCALL_DISPATCH_FILTER_ALLOW => Ok(Self::Allow),
            SYSCALL_DISPATCH_FILTER_BLOCK => Ok(Self::Block),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Enable Syscall User Dispatch mechanism.
///
/// # References
///  - [`prctl(PR_SET_SYSCALL_USER_DISPATCH,PR_SYS_DISPATCH_ON,…)`]
///
/// # Safety
///
/// Please ensure the conditions necessary to safely call this function, as
/// detailed in the references above.
///
/// [`prctl(PR_SET_SYSCALL_USER_DISPATCH,PR_SYS_DISPATCH_ON,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
#[inline]
pub unsafe fn enable_syscall_user_dispatch(
    always_allowed_region: &[u8],
    fast_switch_flag: &AtomicU8,
) -> io::Result<()> {
    syscalls::prctl(
        PR_SET_SYSCALL_USER_DISPATCH,
        PR_SYS_DISPATCH_ON as *mut _,
        always_allowed_region.as_ptr() as *mut _,
        always_allowed_region.len() as *mut _,
        as_ptr(fast_switch_flag) as *mut _,
    )
    .map(|_r| ())
}

//
// PR_SCHED_CORE
//

const PR_SCHED_CORE: c_int = 62;

const PR_SCHED_CORE_GET: usize = 0;

const PR_SCHED_CORE_SCOPE_THREAD: u32 = 0;
const PR_SCHED_CORE_SCOPE_THREAD_GROUP: u32 = 1;
const PR_SCHED_CORE_SCOPE_PROCESS_GROUP: u32 = 2;

/// `PR_SCHED_CORE_SCOPE_*`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CoreSchedulingScope {
    /// Operation will be performed for the thread.
    Thread = PR_SCHED_CORE_SCOPE_THREAD,
    /// Operation will be performed for all tasks in the task group of the
    /// process.
    ThreadGroup = PR_SCHED_CORE_SCOPE_THREAD_GROUP,
    /// Operation will be performed for all processes in the process group.
    ProcessGroup = PR_SCHED_CORE_SCOPE_PROCESS_GROUP,
}

impl TryFrom<u32> for CoreSchedulingScope {
    type Error = io::Errno;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            PR_SCHED_CORE_SCOPE_THREAD => Ok(Self::Thread),
            PR_SCHED_CORE_SCOPE_THREAD_GROUP => Ok(Self::ThreadGroup),
            PR_SCHED_CORE_SCOPE_PROCESS_GROUP => Ok(Self::ProcessGroup),
            _ => Err(io::Errno::RANGE),
        }
    }
}

/// Get core scheduling cookie of a process.
///
/// # References
///  - [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_GET,…)`]
///
/// [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_GET,…)`]: https://www.kernel.org/doc/html/v6.13/admin-guide/hw-vuln/core-scheduling.html
#[inline]
pub fn core_scheduling_cookie(pid: Pid, scope: CoreSchedulingScope) -> io::Result<u64> {
    let mut value: MaybeUninit<u64> = MaybeUninit::uninit();
    unsafe {
        syscalls::prctl(
            PR_SCHED_CORE,
            PR_SCHED_CORE_GET as *mut _,
            pid.as_raw_nonzero().get() as usize as *mut _,
            scope as usize as *mut _,
            value.as_mut_ptr().cast(),
        )?;
        Ok(value.assume_init())
    }
}

const PR_SCHED_CORE_CREATE: usize = 1;

/// Create unique core scheduling cookie.
///
/// # References
///  - [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_CREATE,…)`]
///
/// [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_CREATE,…)`]: https://www.kernel.org/doc/html/v6.13/admin-guide/hw-vuln/core-scheduling.html
#[inline]
pub fn create_core_scheduling_cookie(pid: Pid, scope: CoreSchedulingScope) -> io::Result<()> {
    unsafe {
        syscalls::prctl(
            PR_SCHED_CORE,
            PR_SCHED_CORE_CREATE as *mut _,
            pid.as_raw_nonzero().get() as usize as *mut _,
            scope as usize as *mut _,
            ptr::null_mut(),
        )
        .map(|_r| ())
    }
}

const PR_SCHED_CORE_SHARE_TO: usize = 2;

/// Push core scheduling cookie to a process.
///
/// # References
///  - [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_SHARE_TO,…)`]
///
/// [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_SHARE_TO,…)`]: https://www.kernel.org/doc/html/v6.13/admin-guide/hw-vuln/core-scheduling.html
#[inline]
pub fn push_core_scheduling_cookie(pid: Pid, scope: CoreSchedulingScope) -> io::Result<()> {
    unsafe {
        syscalls::prctl(
            PR_SCHED_CORE,
            PR_SCHED_CORE_SHARE_TO as *mut _,
            pid.as_raw_nonzero().get() as usize as *mut _,
            scope as usize as *mut _,
            ptr::null_mut(),
        )
        .map(|_r| ())
    }
}

const PR_SCHED_CORE_SHARE_FROM: usize = 3;

/// Pull core scheduling cookie from a process.
///
/// # References
///  - [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_SHARE_FROM,…)`]
///
/// [`prctl(PR_SCHED_CORE,PR_SCHED_CORE_SHARE_FROM,…)`]: https://www.kernel.org/doc/html/v6.13/admin-guide/hw-vuln/core-scheduling.html
#[inline]
pub fn pull_core_scheduling_cookie(pid: Pid, scope: CoreSchedulingScope) -> io::Result<()> {
    unsafe {
        syscalls::prctl(
            PR_SCHED_CORE,
            PR_SCHED_CORE_SHARE_FROM as *mut _,
            pid.as_raw_nonzero().get() as usize as *mut _,
            scope as usize as *mut _,
            ptr::null_mut(),
        )
        .map(|_r| ())
    }
}
