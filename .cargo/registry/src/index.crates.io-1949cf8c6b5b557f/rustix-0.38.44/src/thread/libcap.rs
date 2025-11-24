use bitflags::bitflags;
use core::mem::MaybeUninit;

use crate::pid::Pid;
use crate::{backend, io};

/// `__user_cap_data_struct`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilitySets {
    /// `__user_cap_data_struct.effective`
    pub effective: CapabilityFlags,
    /// `__user_cap_data_struct.permitted`
    pub permitted: CapabilityFlags,
    /// `__user_cap_data_struct.inheritable`
    pub inheritable: CapabilityFlags,
}

bitflags! {
    /// `CAP_*` constants.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CapabilityFlags: u64 {
        /// `CAP_CHOWN`
        const CHOWN = 1 << linux_raw_sys::general::CAP_CHOWN;
        /// `CAP_DAC_OVERRIDE`
        const DAC_OVERRIDE = 1 << linux_raw_sys::general::CAP_DAC_OVERRIDE;
        /// `CAP_DAC_READ_SEARCH`
        const DAC_READ_SEARCH = 1 << linux_raw_sys::general::CAP_DAC_READ_SEARCH;
        /// `CAP_FOWNER`
        const FOWNER = 1 << linux_raw_sys::general::CAP_FOWNER;
        /// `CAP_FSETID`
        const FSETID = 1 << linux_raw_sys::general::CAP_FSETID;
        /// `CAP_KILL`
        const KILL = 1 << linux_raw_sys::general::CAP_KILL;
        /// `CAP_SETGID`
        const SETGID = 1 << linux_raw_sys::general::CAP_SETGID;
        /// `CAP_SETUID`
        const SETUID = 1 << linux_raw_sys::general::CAP_SETUID;
        /// `CAP_SETPCAP`
        const SETPCAP = 1 << linux_raw_sys::general::CAP_SETPCAP;
        /// `CAP_LINUX_IMMUTABLE`
        const LINUX_IMMUTABLE = 1 << linux_raw_sys::general::CAP_LINUX_IMMUTABLE;
        /// `CAP_NET_BIND_SERVICE`
        const NET_BIND_SERVICE = 1 << linux_raw_sys::general::CAP_NET_BIND_SERVICE;
        /// `CAP_NET_BROADCAST`
        const NET_BROADCAST = 1 << linux_raw_sys::general::CAP_NET_BROADCAST;
        /// `CAP_NET_ADMIN`
        const NET_ADMIN = 1 << linux_raw_sys::general::CAP_NET_ADMIN;
        /// `CAP_NET_RAW`
        const NET_RAW = 1 << linux_raw_sys::general::CAP_NET_RAW;
        /// `CAP_IPC_LOCK`
        const IPC_LOCK = 1 << linux_raw_sys::general::CAP_IPC_LOCK;
        /// `CAP_IPC_OWNER`
        const IPC_OWNER = 1 << linux_raw_sys::general::CAP_IPC_OWNER;
        /// `CAP_SYS_MODULE`
        const SYS_MODULE = 1 << linux_raw_sys::general::CAP_SYS_MODULE;
        /// `CAP_SYS_RAWIO`
        const SYS_RAWIO = 1 << linux_raw_sys::general::CAP_SYS_RAWIO;
        /// `CAP_SYS_CHROOT`
        const SYS_CHROOT = 1 << linux_raw_sys::general::CAP_SYS_CHROOT;
        /// `CAP_SYS_PTRACE`
        const SYS_PTRACE = 1 << linux_raw_sys::general::CAP_SYS_PTRACE;
        /// `CAP_SYS_PACCT`
        const SYS_PACCT = 1 << linux_raw_sys::general::CAP_SYS_PACCT;
        /// `CAP_SYS_ADMIN`
        const SYS_ADMIN = 1 << linux_raw_sys::general::CAP_SYS_ADMIN;
        /// `CAP_SYS_BOOT`
        const SYS_BOOT = 1 << linux_raw_sys::general::CAP_SYS_BOOT;
        /// `CAP_SYS_NICE`
        const SYS_NICE = 1 << linux_raw_sys::general::CAP_SYS_NICE;
        /// `CAP_SYS_RESOURCE`
        const SYS_RESOURCE = 1 << linux_raw_sys::general::CAP_SYS_RESOURCE;
        /// `CAP_SYS_TIME`
        const SYS_TIME = 1 << linux_raw_sys::general::CAP_SYS_TIME;
        /// `CAP_SYS_TTY_CONFIG`
        const SYS_TTY_CONFIG = 1 << linux_raw_sys::general::CAP_SYS_TTY_CONFIG;
        /// `CAP_MKNOD`
        const MKNOD = 1 << linux_raw_sys::general::CAP_MKNOD;
        /// `CAP_LEASE`
        const LEASE = 1 << linux_raw_sys::general::CAP_LEASE;
        /// `CAP_AUDIT_WRITE`
        const AUDIT_WRITE = 1 << linux_raw_sys::general::CAP_AUDIT_WRITE;
        /// `CAP_AUDIT_CONTROL`
        const AUDIT_CONTROL = 1 << linux_raw_sys::general::CAP_AUDIT_CONTROL;
        /// `CAP_SETFCAP`
        const SETFCAP = 1 << linux_raw_sys::general::CAP_SETFCAP;
        /// `CAP_MAC_OVERRIDE`
        const MAC_OVERRIDE = 1 << linux_raw_sys::general::CAP_MAC_OVERRIDE;
        /// `CAP_MAC_ADMIN`
        const MAC_ADMIN = 1 << linux_raw_sys::general::CAP_MAC_ADMIN;
        /// `CAP_SYSLOG`
        const SYSLOG = 1 << linux_raw_sys::general::CAP_SYSLOG;
        /// `CAP_WAKE_ALARM`
        const WAKE_ALARM = 1 << linux_raw_sys::general::CAP_WAKE_ALARM;
        /// `CAP_BLOCK_SUSPEND`
        const BLOCK_SUSPEND = 1 << linux_raw_sys::general::CAP_BLOCK_SUSPEND;
        /// `CAP_AUDIT_READ`
        const AUDIT_READ = 1 << linux_raw_sys::general::CAP_AUDIT_READ;
        /// `CAP_PERFMON`
        const PERFMON = 1 << linux_raw_sys::general::CAP_PERFMON;
        /// `CAP_BPF`
        const BPF = 1 << linux_raw_sys::general::CAP_BPF;
        /// `CAP_CHECKPOINT_RESTORE`
        const CHECKPOINT_RESTORE = 1 << linux_raw_sys::general::CAP_CHECKPOINT_RESTORE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `capget(_LINUX_CAPABILITY_VERSION_3, pid)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/capget.2.html
#[inline]
#[doc(alias = "capget")]
pub fn capabilities(pid: Option<Pid>) -> io::Result<CapabilitySets> {
    capget(pid)
}

/// `capset(_LINUX_CAPABILITY_VERSION_3, pid, effective, permitted,
/// inheritable)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/capget.2.html
#[inline]
#[doc(alias = "capset")]
pub fn set_capabilities(pid: Option<Pid>, sets: CapabilitySets) -> io::Result<()> {
    capset(pid, sets)
}

#[inline]
#[allow(unsafe_code)]
fn capget(pid: Option<Pid>) -> io::Result<CapabilitySets> {
    let mut data = [MaybeUninit::<linux_raw_sys::general::__user_cap_data_struct>::uninit(); 2];

    let data = {
        let mut header = linux_raw_sys::general::__user_cap_header_struct {
            version: linux_raw_sys::general::_LINUX_CAPABILITY_VERSION_3,
            pid: Pid::as_raw(pid) as backend::c::c_int,
        };

        backend::thread::syscalls::capget(&mut header, &mut data)?;
        // SAFETY: v3 is a 64-bit implementation, so the kernel filled in both
        // data structs.
        unsafe { (data[0].assume_init(), data[1].assume_init()) }
    };

    let effective = u64::from(data.0.effective) | (u64::from(data.1.effective) << u32::BITS);
    let permitted = u64::from(data.0.permitted) | (u64::from(data.1.permitted) << u32::BITS);
    let inheritable = u64::from(data.0.inheritable) | (u64::from(data.1.inheritable) << u32::BITS);

    // The kernel returns a partitioned bitset that we just combined above.
    Ok(CapabilitySets {
        effective: CapabilityFlags::from_bits_retain(effective),
        permitted: CapabilityFlags::from_bits_retain(permitted),
        inheritable: CapabilityFlags::from_bits_retain(inheritable),
    })
}

#[inline]
fn capset(pid: Option<Pid>, sets: CapabilitySets) -> io::Result<()> {
    let mut header = linux_raw_sys::general::__user_cap_header_struct {
        version: linux_raw_sys::general::_LINUX_CAPABILITY_VERSION_3,
        pid: Pid::as_raw(pid) as backend::c::c_int,
    };
    let data = [
        linux_raw_sys::general::__user_cap_data_struct {
            effective: sets.effective.bits() as u32,
            permitted: sets.permitted.bits() as u32,
            inheritable: sets.inheritable.bits() as u32,
        },
        linux_raw_sys::general::__user_cap_data_struct {
            effective: (sets.effective.bits() >> u32::BITS) as u32,
            permitted: (sets.permitted.bits() >> u32::BITS) as u32,
            inheritable: (sets.inheritable.bits() >> u32::BITS) as u32,
        },
    ];

    backend::thread::syscalls::capset(&mut header, &data)
}
