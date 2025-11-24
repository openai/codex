//! Linux auxv `init` function, for "use-explicitly-provided-auxv" mode.
//!
//! # Safety
//!
//! This uses raw pointers to locate and read the kernel-provided auxv array.
#![allow(unsafe_code)]

use crate::backend::c;
#[cfg(feature = "param")]
use crate::ffi::CStr;
use core::ffi::c_void;
use core::ptr::{null_mut, read, NonNull};
#[cfg(feature = "runtime")]
use core::sync::atomic::AtomicBool;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use linux_raw_sys::elf::*;
use linux_raw_sys::general::{
    AT_CLKTCK, AT_EXECFN, AT_HWCAP, AT_HWCAP2, AT_MINSIGSTKSZ, AT_NULL, AT_PAGESZ, AT_SYSINFO_EHDR,
};
#[cfg(feature = "runtime")]
use linux_raw_sys::general::{AT_ENTRY, AT_PHDR, AT_PHENT, AT_PHNUM, AT_RANDOM, AT_SECURE};

#[cfg(feature = "param")]
#[inline]
pub(crate) fn page_size() -> usize {
    PAGE_SIZE.load(Ordering::Relaxed)
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn clock_ticks_per_second() -> u64 {
    CLOCK_TICKS_PER_SECOND.load(Ordering::Relaxed) as u64
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_hwcap() -> (usize, usize) {
    (
        HWCAP.load(Ordering::Relaxed),
        HWCAP2.load(Ordering::Relaxed),
    )
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_minsigstksz() -> usize {
    MINSIGSTKSZ.load(Ordering::Relaxed)
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_execfn() -> &'static CStr {
    let execfn = EXECFN.load(Ordering::Relaxed);

    // SAFETY: We initialize `EXECFN` to a valid `CStr` pointer, and we assume
    // the `AT_EXECFN` value provided by the kernel points to a valid C string.
    unsafe { CStr::from_ptr(execfn.cast()) }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn linux_secure() -> bool {
    SECURE.load(Ordering::Relaxed)
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn exe_phdrs() -> (*const c_void, usize, usize) {
    (
        PHDR.load(Ordering::Relaxed).cast(),
        PHENT.load(Ordering::Relaxed),
        PHNUM.load(Ordering::Relaxed),
    )
}

/// `AT_SYSINFO_EHDR` isn't present on all platforms in all configurations, so
/// if we don't see it, this function returns a null pointer.
#[inline]
pub(in super::super) fn sysinfo_ehdr() -> *const Elf_Ehdr {
    SYSINFO_EHDR.load(Ordering::Relaxed)
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn entry() -> usize {
    ENTRY.load(Ordering::Relaxed)
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn random() -> *const [u8; 16] {
    RANDOM.load(Ordering::Relaxed)
}

static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);
static CLOCK_TICKS_PER_SECOND: AtomicUsize = AtomicUsize::new(0);
static HWCAP: AtomicUsize = AtomicUsize::new(0);
static HWCAP2: AtomicUsize = AtomicUsize::new(0);
static MINSIGSTKSZ: AtomicUsize = AtomicUsize::new(0);
static SYSINFO_EHDR: AtomicPtr<Elf_Ehdr> = AtomicPtr::new(null_mut());
// Initialize `EXECFN` to a valid `CStr` pointer so that we don't need to check
// for null on every `execfn` call.
static EXECFN: AtomicPtr<c::c_char> = AtomicPtr::new(b"\0".as_ptr() as _);
#[cfg(feature = "runtime")]
static SECURE: AtomicBool = AtomicBool::new(false);
// Use `dangling` so that we can always treat it like an empty slice.
#[cfg(feature = "runtime")]
static PHDR: AtomicPtr<Elf_Phdr> = AtomicPtr::new(NonNull::dangling().as_ptr());
#[cfg(feature = "runtime")]
static PHENT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static PHNUM: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static ENTRY: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static RANDOM: AtomicPtr<[u8; 16]> = AtomicPtr::new(NonNull::dangling().as_ptr());

/// When "use-explicitly-provided-auxv" is enabled, we export a function to be
/// called during initialization, and passed a pointer to the original
/// environment variable block set up by the OS.
pub(crate) unsafe fn init(envp: *mut *mut u8) {
    init_from_envp(envp);
}

/// # Safety
///
/// This must be passed a pointer to the environment variable buffer
/// provided by the kernel, which is followed in memory by the auxv array.
unsafe fn init_from_envp(mut envp: *mut *mut u8) {
    while !(*envp).is_null() {
        envp = envp.add(1);
    }
    init_from_auxp(envp.add(1).cast())
}

/// Process auxv entries from the auxv array pointed to by `auxp`.
///
/// # Safety
///
/// This must be passed a pointer to an auxv array.
///
/// The buffer contains `Elf_aux_t` elements, though it need not be aligned;
/// function uses `read_unaligned` to read from it.
unsafe fn init_from_auxp(mut auxp: *const Elf_auxv_t) {
    loop {
        let Elf_auxv_t { a_type, a_val } = read(auxp);

        match a_type as _ {
            AT_PAGESZ => PAGE_SIZE.store(a_val as usize, Ordering::Relaxed),
            AT_CLKTCK => CLOCK_TICKS_PER_SECOND.store(a_val as usize, Ordering::Relaxed),
            AT_HWCAP => HWCAP.store(a_val as usize, Ordering::Relaxed),
            AT_HWCAP2 => HWCAP2.store(a_val as usize, Ordering::Relaxed),
            AT_MINSIGSTKSZ => MINSIGSTKSZ.store(a_val as usize, Ordering::Relaxed),
            AT_EXECFN => EXECFN.store(a_val.cast::<c::c_char>(), Ordering::Relaxed),
            AT_SYSINFO_EHDR => SYSINFO_EHDR.store(a_val.cast::<Elf_Ehdr>(), Ordering::Relaxed),

            #[cfg(feature = "runtime")]
            AT_SECURE => SECURE.store(a_val as usize != 0, Ordering::Relaxed),
            #[cfg(feature = "runtime")]
            AT_PHDR => PHDR.store(a_val.cast::<Elf_Phdr>(), Ordering::Relaxed),
            #[cfg(feature = "runtime")]
            AT_PHNUM => PHNUM.store(a_val as usize, Ordering::Relaxed),
            #[cfg(feature = "runtime")]
            AT_PHENT => PHENT.store(a_val as usize, Ordering::Relaxed),
            #[cfg(feature = "runtime")]
            AT_ENTRY => ENTRY.store(a_val as usize, Ordering::Relaxed),
            #[cfg(feature = "runtime")]
            AT_RANDOM => RANDOM.store(a_val.cast::<[u8; 16]>(), Ordering::Relaxed),

            AT_NULL => break,
            _ => (),
        }
        auxp = auxp.add(1);
    }
}
