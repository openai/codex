use cfg_if::cfg_if;

/// The datatype used for the ioctl number
#[cfg(any(
    target_os = "android",
    target_os = "fuchsia",
    target_env = "musl",
    target_env = "ohos"
))]
#[doc(hidden)]
pub type ioctl_num_type = ::libc::c_int;
#[cfg(not(any(
    target_os = "android",
    target_os = "fuchsia",
    target_env = "musl",
    target_env = "ohos"
)))]
#[doc(hidden)]
pub type ioctl_num_type = ::libc::c_ulong;
/// The datatype used for the 3rd argument
#[doc(hidden)]
pub type ioctl_param_type = ::libc::c_ulong;

#[doc(hidden)]
pub const NRBITS: ioctl_num_type = 8;
#[doc(hidden)]
pub const TYPEBITS: ioctl_num_type = 8;

cfg_if! {
    if #[cfg(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc64"
    ))] {
        mod consts {
            #[doc(hidden)]
            pub const NONE: u8 = 1;
            #[doc(hidden)]
            pub const READ: u8 = 2;
            #[doc(hidden)]
            pub const WRITE: u8 = 4;
            #[doc(hidden)]
            pub const SIZEBITS: u8 = 13;
            #[doc(hidden)]
            pub const DIRBITS: u8 = 3;
        }
    } else {
        // "Generic" ioctl protocol
        mod consts {
            #[doc(hidden)]
            pub const NONE: u8 = 0;
            #[doc(hidden)]
            pub const READ: u8 = 2;
            #[doc(hidden)]
            pub const WRITE: u8 = 1;
            #[doc(hidden)]
            pub const SIZEBITS: u8 = 14;
            #[doc(hidden)]
            pub const DIRBITS: u8 = 2;
        }
    }
}

pub use self::consts::*;

#[doc(hidden)]
pub const NRSHIFT: ioctl_num_type = 0;
#[doc(hidden)]
pub const TYPESHIFT: ioctl_num_type = NRSHIFT + NRBITS as ioctl_num_type;
#[doc(hidden)]
pub const SIZESHIFT: ioctl_num_type = TYPESHIFT + TYPEBITS as ioctl_num_type;
#[doc(hidden)]
pub const DIRSHIFT: ioctl_num_type = SIZESHIFT + SIZEBITS as ioctl_num_type;

#[doc(hidden)]
pub const NRMASK: ioctl_num_type = (1 << NRBITS) - 1;
#[doc(hidden)]
pub const TYPEMASK: ioctl_num_type = (1 << TYPEBITS) - 1;
#[doc(hidden)]
pub const SIZEMASK: ioctl_num_type = (1 << SIZEBITS) - 1;
#[doc(hidden)]
pub const DIRMASK: ioctl_num_type = (1 << DIRBITS) - 1;

/// Encode an ioctl command.
#[macro_export]
#[doc(hidden)]
macro_rules! ioc {
    ($dir:expr, $ty:expr, $nr:expr, $sz:expr) => {
        (($dir as $crate::sys::ioctl::ioctl_num_type
            & $crate::sys::ioctl::DIRMASK)
            << $crate::sys::ioctl::DIRSHIFT)
            | (($ty as $crate::sys::ioctl::ioctl_num_type
                & $crate::sys::ioctl::TYPEMASK)
                << $crate::sys::ioctl::TYPESHIFT)
            | (($nr as $crate::sys::ioctl::ioctl_num_type
                & $crate::sys::ioctl::NRMASK)
                << $crate::sys::ioctl::NRSHIFT)
            | (($sz as $crate::sys::ioctl::ioctl_num_type
                & $crate::sys::ioctl::SIZEMASK)
                << $crate::sys::ioctl::SIZESHIFT)
    };
}

/// Generate an ioctl request code for a command that passes no data.
///
/// This is equivalent to the `_IO()` macro exposed by the C ioctl API.
///
/// You should only use this macro directly if the `ioctl` you're working
/// with is "bad" and you cannot use `ioctl_none!()` directly.
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate nix;
/// const KVMIO: u8 = 0xAE;
/// ioctl_write_int_bad!(kvm_create_vm, request_code_none!(KVMIO, 0x03));
/// # fn main() {}
/// ```
#[macro_export(local_inner_macros)]
macro_rules! request_code_none {
    ($ty:expr, $nr:expr) => {
        ioc!($crate::sys::ioctl::NONE, $ty, $nr, 0)
    };
}

/// Generate an ioctl request code for a command that reads.
///
/// This is equivalent to the `_IOR()` macro exposed by the C ioctl API.
///
/// You should only use this macro directly if the `ioctl` you're working
/// with is "bad" and you cannot use `ioctl_read!()` directly.
///
/// The read/write direction is relative to userland, so this
/// command would be userland is reading and the kernel is
/// writing.
#[macro_export(local_inner_macros)]
macro_rules! request_code_read {
    ($ty:expr, $nr:expr, $sz:expr) => {
        ioc!($crate::sys::ioctl::READ, $ty, $nr, $sz)
    };
}

/// Generate an ioctl request code for a command that writes.
///
/// This is equivalent to the `_IOW()` macro exposed by the C ioctl API.
///
/// You should only use this macro directly if the `ioctl` you're working
/// with is "bad" and you cannot use `ioctl_write!()` directly.
///
/// The read/write direction is relative to userland, so this
/// command would be userland is writing and the kernel is
/// reading.
#[macro_export(local_inner_macros)]
macro_rules! request_code_write {
    ($ty:expr, $nr:expr, $sz:expr) => {
        ioc!($crate::sys::ioctl::WRITE, $ty, $nr, $sz)
    };
}

/// Generate an ioctl request code for a command that reads and writes.
///
/// This is equivalent to the `_IOWR()` macro exposed by the C ioctl API.
///
/// You should only use this macro directly if the `ioctl` you're working
/// with is "bad" and you cannot use `ioctl_readwrite!()` directly.
#[macro_export(local_inner_macros)]
macro_rules! request_code_readwrite {
    ($ty:expr, $nr:expr, $sz:expr) => {
        ioc!(
            $crate::sys::ioctl::READ | $crate::sys::ioctl::WRITE,
            $ty,
            $nr,
            $sz
        )
    };
}
