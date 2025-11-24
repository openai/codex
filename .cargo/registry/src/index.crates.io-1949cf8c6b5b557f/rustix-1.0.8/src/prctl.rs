//! Helper functions for `prctl` syscalls.

#![allow(unsafe_code)]

use crate::backend::prctl::syscalls;
use crate::ffi::{c_int, c_void};
use crate::io;
use crate::utils::as_mut_ptr;
use bitflags::bitflags;
use core::mem::MaybeUninit;
use core::ptr::null_mut;

bitflags! {
    /// `PR_PAC_AP*`
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PointerAuthenticationKeys: u32 {
        /// `PR_PAC_APIAKEY`—Instruction authentication key `A`.
        const INSTRUCTION_AUTHENTICATION_KEY_A = linux_raw_sys::prctl::PR_PAC_APIAKEY;
        /// `PR_PAC_APIBKEY`—Instruction authentication key `B`.
        const INSTRUCTION_AUTHENTICATION_KEY_B = linux_raw_sys::prctl::PR_PAC_APIBKEY;
        /// `PR_PAC_APDAKEY`—Data authentication key `A`.
        const DATA_AUTHENTICATION_KEY_A = linux_raw_sys::prctl::PR_PAC_APDAKEY;
        /// `PR_PAC_APDBKEY`—Data authentication key `B`.
        const DATA_AUTHENTICATION_KEY_B = linux_raw_sys::prctl::PR_PAC_APDBKEY;
        /// `PR_PAC_APGAKEY`—Generic authentication `A` key.
        const GENERIC_AUTHENTICATION_KEY_A = linux_raw_sys::prctl::PR_PAC_APGAKEY;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[inline]
pub(crate) unsafe fn prctl_1arg(option: c_int) -> io::Result<c_int> {
    const NULL: *mut c_void = null_mut();
    syscalls::prctl(option, NULL, NULL, NULL, NULL)
}

#[inline]
pub(crate) unsafe fn prctl_2args(option: c_int, arg2: *mut c_void) -> io::Result<c_int> {
    const NULL: *mut c_void = null_mut();
    syscalls::prctl(option, arg2, NULL, NULL, NULL)
}

#[inline]
pub(crate) unsafe fn prctl_3args(
    option: c_int,
    arg2: *mut c_void,
    arg3: *mut c_void,
) -> io::Result<c_int> {
    syscalls::prctl(option, arg2, arg3, null_mut(), null_mut())
}

#[inline]
pub(crate) unsafe fn prctl_get_at_arg2_optional<P>(option: i32) -> io::Result<P> {
    let mut value: MaybeUninit<P> = MaybeUninit::uninit();
    prctl_2args(option, value.as_mut_ptr().cast())?;
    Ok(value.assume_init())
}

#[inline]
pub(crate) unsafe fn prctl_get_at_arg2<P, T>(option: i32) -> io::Result<T>
where
    P: Default,
    T: TryFrom<P, Error = io::Errno>,
{
    let mut value: P = Default::default();
    prctl_2args(option, as_mut_ptr(&mut value).cast())?;
    TryFrom::try_from(value)
}
