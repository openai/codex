//! Linux auxv support, using libc.
//!
//! # Safety
//!
//! This uses raw pointers to locate and read the kernel-provided auxv array.
#![allow(unsafe_code)]

use crate::backend::c;
#[cfg(feature = "param")]
use crate::ffi::CStr;
#[cfg(not(feature = "runtime"))]
use core::ptr::null;
use linux_raw_sys::elf::*;

// `getauxval` wasn't supported in glibc until 2.16. Also this lets us use
// `*mut` as the return type to preserve strict provenance.
#[cfg(not(feature = "runtime"))]
weak!(fn getauxval(c::c_ulong) -> *mut c::c_void);

// With the "runtime" feature, go ahead and depend on `getauxval` existing so
// that we never fail.
#[cfg(feature = "runtime")]
extern "C" {
    fn getauxval(type_: c::c_ulong) -> *mut c::c_void;
}

#[cfg(feature = "runtime")]
const AT_PHDR: c::c_ulong = 3;
#[cfg(feature = "runtime")]
const AT_PHENT: c::c_ulong = 4;
#[cfg(feature = "runtime")]
const AT_PHNUM: c::c_ulong = 5;
#[cfg(feature = "runtime")]
const AT_ENTRY: c::c_ulong = 9;
const AT_HWCAP: c::c_ulong = 16;
#[cfg(feature = "runtime")]
const AT_RANDOM: c::c_ulong = 25;
const AT_HWCAP2: c::c_ulong = 26;
const AT_SECURE: c::c_ulong = 23;
const AT_EXECFN: c::c_ulong = 31;
const AT_SYSINFO_EHDR: c::c_ulong = 33;
const AT_MINSIGSTKSZ: c::c_ulong = 51;

// Declare `sysconf` ourselves so that we don't depend on all of libc just for
// this.
extern "C" {
    fn sysconf(name: c::c_int) -> c::c_long;
}

#[cfg(target_os = "android")]
const _SC_PAGESIZE: c::c_int = 39;
#[cfg(target_os = "linux")]
const _SC_PAGESIZE: c::c_int = 30;
#[cfg(target_os = "android")]
const _SC_CLK_TCK: c::c_int = 6;
#[cfg(target_os = "linux")]
const _SC_CLK_TCK: c::c_int = 2;

#[cfg(feature = "param")]
#[inline]
pub(crate) fn page_size() -> usize {
    unsafe { sysconf(_SC_PAGESIZE) as usize }
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn clock_ticks_per_second() -> u64 {
    unsafe { sysconf(_SC_CLK_TCK) as u64 }
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_hwcap() -> (usize, usize) {
    #[cfg(not(feature = "runtime"))]
    unsafe {
        if let Some(libc_getauxval) = getauxval.get() {
            let hwcap = libc_getauxval(AT_HWCAP) as usize;
            let hwcap2 = libc_getauxval(AT_HWCAP2) as usize;
            (hwcap, hwcap2)
        } else {
            (0, 0)
        }
    }

    #[cfg(feature = "runtime")]
    unsafe {
        let hwcap = getauxval(AT_HWCAP) as usize;
        let hwcap2 = getauxval(AT_HWCAP2) as usize;
        (hwcap, hwcap2)
    }
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_minsigstksz() -> usize {
    #[cfg(not(feature = "runtime"))]
    if let Some(libc_getauxval) = getauxval.get() {
        unsafe { libc_getauxval(AT_MINSIGSTKSZ) as usize }
    } else {
        0
    }

    #[cfg(feature = "runtime")]
    unsafe {
        getauxval(AT_MINSIGSTKSZ) as usize
    }
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_execfn() -> &'static CStr {
    #[cfg(not(feature = "runtime"))]
    unsafe {
        if let Some(libc_getauxval) = getauxval.get() {
            CStr::from_ptr(libc_getauxval(AT_EXECFN).cast())
        } else {
            cstr!("")
        }
    }

    #[cfg(feature = "runtime")]
    unsafe {
        CStr::from_ptr(getauxval(AT_EXECFN).cast())
    }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn linux_secure() -> bool {
    unsafe { getauxval(AT_SECURE) as usize != 0 }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn exe_phdrs() -> (*const c::c_void, usize, usize) {
    unsafe {
        let phdr: *const c::c_void = getauxval(AT_PHDR);
        let phent = getauxval(AT_PHENT) as usize;
        let phnum = getauxval(AT_PHNUM) as usize;
        (phdr, phent, phnum)
    }
}

/// `AT_SYSINFO_EHDR` isn't present on all platforms in all configurations, so
/// if we don't see it, this function returns a null pointer.
#[inline]
pub(in super::super) fn sysinfo_ehdr() -> *const Elf_Ehdr {
    #[cfg(not(feature = "runtime"))]
    unsafe {
        if let Some(libc_getauxval) = getauxval.get() {
            libc_getauxval(AT_SYSINFO_EHDR) as *const Elf_Ehdr
        } else {
            null()
        }
    }

    #[cfg(feature = "runtime")]
    unsafe {
        getauxval(AT_SYSINFO_EHDR) as *const Elf_Ehdr
    }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn entry() -> usize {
    unsafe { getauxval(AT_ENTRY) as usize }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn random() -> *const [u8; 16] {
    unsafe { getauxval(AT_RANDOM) as *const [u8; 16] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi() {
        const_assert_eq!(self::_SC_PAGESIZE, ::libc::_SC_PAGESIZE);
        const_assert_eq!(self::_SC_CLK_TCK, ::libc::_SC_CLK_TCK);
        const_assert_eq!(self::AT_HWCAP, ::libc::AT_HWCAP);
        const_assert_eq!(self::AT_HWCAP2, ::libc::AT_HWCAP2);
        const_assert_eq!(self::AT_EXECFN, ::libc::AT_EXECFN);
        const_assert_eq!(self::AT_SECURE, ::libc::AT_SECURE);
        const_assert_eq!(self::AT_SYSINFO_EHDR, ::libc::AT_SYSINFO_EHDR);
        const_assert_eq!(self::AT_MINSIGSTKSZ, ::libc::AT_MINSIGSTKSZ);
        #[cfg(feature = "runtime")]
        const_assert_eq!(self::AT_PHDR, ::libc::AT_PHDR);
        #[cfg(feature = "runtime")]
        const_assert_eq!(self::AT_PHNUM, ::libc::AT_PHNUM);
        #[cfg(feature = "runtime")]
        const_assert_eq!(self::AT_ENTRY, ::libc::AT_ENTRY);
        #[cfg(feature = "runtime")]
        const_assert_eq!(self::AT_RANDOM, ::libc::AT_RANDOM);
    }
}
