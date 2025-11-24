#[cfg(all(feature = "alloc", not(any(target_os = "espidf", target_os = "redox"))))]
pub(crate) mod dir;
#[cfg(linux_kernel)]
pub mod inotify;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "haiku",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) mod makedev;
#[cfg(not(windows))]
pub(crate) mod syscalls;
pub(crate) mod types;

// TODO: Fix linux-raw-sys to define ioctl codes for sparc.
#[cfg(all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")))]
pub(crate) const EXT4_IOC_RESIZE_FS: crate::ioctl::RawOpcode = 0x8008_6610;

#[cfg(all(linux_kernel, not(any(target_arch = "sparc", target_arch = "sparc64"))))]
pub(crate) const EXT4_IOC_RESIZE_FS: crate::ioctl::RawOpcode =
    linux_raw_sys::ioctl::EXT4_IOC_RESIZE_FS as crate::ioctl::RawOpcode;
