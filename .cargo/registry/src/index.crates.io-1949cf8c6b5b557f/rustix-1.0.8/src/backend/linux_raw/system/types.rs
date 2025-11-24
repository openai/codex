use crate::ffi;
use core::mem::size_of;

/// `sysinfo`
#[non_exhaustive]
#[repr(C)]
pub struct Sysinfo {
    /// Seconds since boot
    pub uptime: ffi::c_long,
    /// 1, 5, and 15 minute load averages
    pub loads: [ffi::c_ulong; 3],
    /// Total usable main memory size
    pub totalram: ffi::c_ulong,
    /// Available memory size
    pub freeram: ffi::c_ulong,
    /// Amount of shared memory
    pub sharedram: ffi::c_ulong,
    /// Memory used by buffers
    pub bufferram: ffi::c_ulong,
    /// Total swap space size
    pub totalswap: ffi::c_ulong,
    /// Swap space still available
    pub freeswap: ffi::c_ulong,
    /// Number of current processes
    pub procs: ffi::c_ushort,

    pub(crate) pad: ffi::c_ushort,

    /// Total high memory size
    pub totalhigh: ffi::c_ulong,
    /// Available high memory size
    pub freehigh: ffi::c_ulong,
    /// Memory unit size in bytes
    pub mem_unit: ffi::c_uint,

    pub(crate) f: [u8; 20 - 2 * size_of::<ffi::c_long>() - size_of::<ffi::c_int>()],
}

pub(crate) type RawUname = linux_raw_sys::system::new_utsname;
