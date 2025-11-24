/// The datatype used for the ioctl number
#[doc(hidden)]
#[cfg(not(solarish))]
pub type ioctl_num_type = ::libc::c_ulong;

#[doc(hidden)]
#[cfg(solarish)]
pub type ioctl_num_type = ::libc::c_int;

/// The datatype used for the 3rd argument
#[doc(hidden)]
pub type ioctl_param_type = ::libc::c_int;

mod consts {
    use crate::sys::ioctl::ioctl_num_type;
    #[doc(hidden)]
    pub const VOID: ioctl_num_type = 0x2000_0000;
    #[doc(hidden)]
    pub const OUT: ioctl_num_type = 0x4000_0000;
    #[doc(hidden)]
    #[allow(overflowing_literals)]
    pub const IN: ioctl_num_type = 0x8000_0000;
    #[doc(hidden)]
    pub const INOUT: ioctl_num_type = IN | OUT;
    #[doc(hidden)]
    pub const IOCPARM_MASK: ioctl_num_type = 0x1fff;
}

pub use self::consts::*;

#[macro_export]
#[doc(hidden)]
macro_rules! ioc {
    ($inout:expr, $group:expr, $num:expr, $len:expr) => {
        $inout
            | (($len as $crate::sys::ioctl::ioctl_num_type
                & $crate::sys::ioctl::IOCPARM_MASK)
                << 16)
            | (($group as $crate::sys::ioctl::ioctl_num_type) << 8)
            | ($num as $crate::sys::ioctl::ioctl_num_type)
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
    ($g:expr, $n:expr) => {
        ioc!($crate::sys::ioctl::VOID, $g, $n, 0)
    };
}

/// Generate an ioctl request code for a command that passes an integer
///
/// This is equivalent to the `_IOWINT()` macro exposed by the C ioctl API.
///
/// You should only use this macro directly if the `ioctl` you're working
/// with is "bad" and you cannot use `ioctl_write_int!()` directly.
#[macro_export(local_inner_macros)]
macro_rules! request_code_write_int {
    ($g:expr, $n:expr) => {
        ioc!(
            $crate::sys::ioctl::VOID,
            $g,
            $n,
            ::std::mem::size_of::<$crate::libc::c_int>()
        )
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
    ($g:expr, $n:expr, $len:expr) => {
        ioc!($crate::sys::ioctl::OUT, $g, $n, $len)
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
    ($g:expr, $n:expr, $len:expr) => {
        ioc!($crate::sys::ioctl::IN, $g, $n, $len)
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
    ($g:expr, $n:expr, $len:expr) => {
        ioc!($crate::sys::ioctl::INOUT, $g, $n, $len)
    };
}
