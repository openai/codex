#[cfg(feature = "alloc")]
pub(crate) mod dir;
pub mod inotify;
pub(crate) mod makedev;
pub(crate) mod syscalls;
pub(crate) mod types;

// TODO: Fix linux-raw-sys to define ioctl codes for sparc.
#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
pub(crate) const EXT4_IOC_RESIZE_FS: u32 = 0x8008_6610;

#[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
pub(crate) use linux_raw_sys::ioctl::EXT4_IOC_RESIZE_FS;
