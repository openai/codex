//! Linux auxv support.
//!
//! # Safety
//!
//! This uses raw pointers to locate and read the kernel-provided auxv array.
#![allow(unsafe_code)]

use super::super::conv::{c_int, pass_usize, ret_usize};
use crate::backend::c;
use crate::fd::OwnedFd;
#[cfg(feature = "param")]
use crate::ffi::CStr;
use crate::fs::{Mode, OFlags};
use crate::utils::{as_ptr, check_raw_pointer};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::{null_mut, read_unaligned, NonNull};
#[cfg(feature = "runtime")]
use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::{AtomicPtr, AtomicUsize};
use linux_raw_sys::elf::*;
use linux_raw_sys::general::{
    AT_BASE, AT_CLKTCK, AT_EXECFN, AT_HWCAP, AT_HWCAP2, AT_MINSIGSTKSZ, AT_NULL, AT_PAGESZ,
    AT_SYSINFO_EHDR,
};
#[cfg(feature = "runtime")]
use linux_raw_sys::general::{
    AT_EGID, AT_ENTRY, AT_EUID, AT_GID, AT_PHDR, AT_PHENT, AT_PHNUM, AT_RANDOM, AT_SECURE, AT_UID,
};
#[cfg(feature = "alloc")]
use {alloc::borrow::Cow, alloc::vec};

// TODO: Fix linux-raw-sys to define EM_CURRENT for s390x.
#[cfg(target_arch = "s390x")]
const EM_CURRENT: u16 = 22; // EM_S390

#[cfg(feature = "param")]
#[inline]
pub(crate) fn page_size() -> usize {
    let mut page_size = PAGE_SIZE.load(Relaxed);

    if page_size == 0 {
        init_auxv();
        page_size = PAGE_SIZE.load(Relaxed);
    }

    page_size
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn clock_ticks_per_second() -> u64 {
    let mut ticks = CLOCK_TICKS_PER_SECOND.load(Relaxed);

    if ticks == 0 {
        init_auxv();
        ticks = CLOCK_TICKS_PER_SECOND.load(Relaxed);
    }

    ticks as u64
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_hwcap() -> (usize, usize) {
    let mut hwcap = HWCAP.load(Relaxed);
    let mut hwcap2 = HWCAP2.load(Relaxed);

    if hwcap == 0 || hwcap2 == 0 {
        init_auxv();
        hwcap = HWCAP.load(Relaxed);
        hwcap2 = HWCAP2.load(Relaxed);
    }

    (hwcap, hwcap2)
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_minsigstksz() -> usize {
    let mut minsigstksz = MINSIGSTKSZ.load(Relaxed);

    if minsigstksz == 0 {
        init_auxv();
        minsigstksz = MINSIGSTKSZ.load(Relaxed);
    }

    minsigstksz
}

#[cfg(feature = "param")]
#[inline]
pub(crate) fn linux_execfn() -> &'static CStr {
    let mut execfn = EXECFN.load(Relaxed);

    if execfn.is_null() {
        init_auxv();
        execfn = EXECFN.load(Relaxed);
    }

    // SAFETY: We assume the `AT_EXECFN` value provided by the kernel is a
    // valid pointer to a valid NUL-terminated array of bytes.
    unsafe { CStr::from_ptr(execfn.cast()) }
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn linux_secure() -> bool {
    let mut secure = SECURE.load(Relaxed);

    // 0 means not initialized yet.
    if secure == 0 {
        init_auxv();
        secure = SECURE.load(Relaxed);
    }

    // 0 means not present. Libc `getauxval(AT_SECURE)` would return 0.
    // 1 means not in secure mode.
    // 2 means in secure mode.
    secure > 1
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn exe_phdrs() -> (*const c::c_void, usize, usize) {
    let mut phdr = PHDR.load(Relaxed);
    let mut phent = PHENT.load(Relaxed);
    let mut phnum = PHNUM.load(Relaxed);

    if phdr.is_null() || phnum == 0 {
        init_auxv();
        phdr = PHDR.load(Relaxed);
        phent = PHENT.load(Relaxed);
        phnum = PHNUM.load(Relaxed);
    }

    (phdr.cast(), phent, phnum)
}

/// `AT_SYSINFO_EHDR` isn't present on all platforms in all configurations, so
/// if we don't see it, this function returns a null pointer.
///
/// And, this function returns a null pointer, rather than panicking, if the
/// auxv records can't be read.
#[inline]
pub(in super::super) fn sysinfo_ehdr() -> *const Elf_Ehdr {
    let mut ehdr = SYSINFO_EHDR.load(Relaxed);

    if ehdr.is_null() {
        // Use `maybe_init_auxv` to read the aux vectors if it can, but do
        // nothing if it can't. If it can't, then we'll get a null pointer
        // here, which our callers are prepared to deal with.
        maybe_init_auxv();

        ehdr = SYSINFO_EHDR.load(Relaxed);
    }

    ehdr
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn entry() -> usize {
    let mut entry = ENTRY.load(Relaxed);

    if entry == 0 {
        init_auxv();
        entry = ENTRY.load(Relaxed);
    }

    entry
}

#[cfg(feature = "runtime")]
#[inline]
pub(crate) fn random() -> *const [u8; 16] {
    let mut random = RANDOM.load(Relaxed);

    if random.is_null() {
        init_auxv();
        random = RANDOM.load(Relaxed);
    }

    random
}

static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);
static CLOCK_TICKS_PER_SECOND: AtomicUsize = AtomicUsize::new(0);
static HWCAP: AtomicUsize = AtomicUsize::new(0);
static HWCAP2: AtomicUsize = AtomicUsize::new(0);
static MINSIGSTKSZ: AtomicUsize = AtomicUsize::new(0);
static EXECFN: AtomicPtr<c::c_char> = AtomicPtr::new(null_mut());
static SYSINFO_EHDR: AtomicPtr<Elf_Ehdr> = AtomicPtr::new(null_mut());
#[cfg(feature = "runtime")]
static SECURE: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "runtime")]
static PHDR: AtomicPtr<Elf_Phdr> = AtomicPtr::new(null_mut());
#[cfg(feature = "runtime")]
static PHENT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static PHNUM: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static ENTRY: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "runtime")]
static RANDOM: AtomicPtr<[u8; 16]> = AtomicPtr::new(null_mut());

const PR_GET_AUXV: c::c_int = 0x4155_5856;

/// Use Linux ≥ 6.4's `PR_GET_AUXV` to read the aux records, into a provided
/// statically-sized buffer. Return:
///  - `Ok(…)` if the buffer is big enough.
///  - `Err(Ok(len))` if we need a buffer of length `len`.
///  - `Err(Err(err))` if we failed with `err`.
#[cold]
fn pr_get_auxv_static(buffer: &mut [u8; 512]) -> Result<&mut [u8], crate::io::Result<usize>> {
    let len = unsafe {
        ret_usize(syscall_always_asm!(
            __NR_prctl,
            c_int(PR_GET_AUXV),
            buffer.as_mut_ptr(),
            pass_usize(buffer.len()),
            pass_usize(0),
            pass_usize(0)
        ))
        .map_err(Err)?
    };
    if len <= buffer.len() {
        return Ok(&mut buffer[..len]);
    }
    Err(Ok(len))
}

/// Use Linux ≥ 6.4's `PR_GET_AUXV` to read the aux records, using a provided
/// statically-sized buffer if possible, or a dynamically allocated buffer
/// otherwise. Return:
///  - Ok(…) on success.
///  - Err(err) on failure.
#[cfg(feature = "alloc")]
#[cold]
fn pr_get_auxv_dynamic(buffer: &mut [u8; 512]) -> crate::io::Result<Cow<'_, [u8]>> {
    // First try use the static buffer.
    let len = match pr_get_auxv_static(buffer) {
        Ok(buffer) => return Ok(Cow::Borrowed(buffer)),
        Err(Ok(len)) => len,
        Err(Err(err)) => return Err(err),
    };

    // If that indicates it needs a bigger buffer, allocate one.
    let mut buffer = vec![0u8; len];
    let len = unsafe {
        ret_usize(syscall_always_asm!(
            __NR_prctl,
            c_int(PR_GET_AUXV),
            buffer.as_mut_ptr(),
            pass_usize(buffer.len()),
            pass_usize(0),
            pass_usize(0)
        ))?
    };
    assert_eq!(len, buffer.len());
    Ok(Cow::Owned(buffer))
}

/// Read the auxv records and initialize the various static variables. Panic
/// if an error is encountered.
#[cold]
fn init_auxv() {
    init_auxv_impl().unwrap();
}

/// Like `init_auxv`, but don't panic if an error is encountered. The caller
/// must be prepared for initialization to be skipped.
#[cold]
fn maybe_init_auxv() {
    let _ = init_auxv_impl();
}

/// If we don't have "use-explicitly-provided-auxv" or "use-libc-auxv", we
/// read the aux vector via the `prctl` `PR_GET_AUXV`, with a fallback to
/// /proc/self/auxv for kernels that don't support `PR_GET_AUXV`.
#[cold]
fn init_auxv_impl() -> Result<(), ()> {
    let mut buffer = [0u8; 512];

    // If we don't have "alloc", just try to read into our statically-sized
    // buffer. This might fail due to the buffer being insufficient; we're
    // prepared to cope, though we may do suboptimal things.
    #[cfg(not(feature = "alloc"))]
    let result = pr_get_auxv_static(&mut buffer);

    // If we do have "alloc" then read into our statically-sized buffer if
    // it fits, or fall back to a dynamically-allocated buffer.
    #[cfg(feature = "alloc")]
    let result = pr_get_auxv_dynamic(&mut buffer);

    if let Ok(buffer) = result {
        // SAFETY: We assume the kernel returns a valid auxv.
        unsafe {
            init_from_aux_iter(AuxPointer(buffer.as_ptr().cast())).unwrap();
        }
        return Ok(());
    }

    // If `PR_GET_AUXV` is unavailable, or if we don't have "alloc" and
    // the aux records don't fit in our static buffer, then fall back to trying
    // to open "/proc/self/auxv". We don't use `proc_self_fd` because its extra
    // checking breaks on QEMU.
    if let Ok(file) = crate::fs::open("/proc/self/auxv", OFlags::RDONLY, Mode::empty()) {
        #[cfg(feature = "alloc")]
        init_from_auxv_file(file).unwrap();

        #[cfg(not(feature = "alloc"))]
        unsafe {
            init_from_aux_iter(AuxFile(file)).unwrap();
        }

        return Ok(());
    }

    Err(())
}

/// Process auxv entries from the open file `auxv`.
#[cfg(feature = "alloc")]
#[cold]
#[must_use]
fn init_from_auxv_file(auxv: OwnedFd) -> Option<()> {
    let mut buffer = Vec::<u8>::with_capacity(512);
    loop {
        let cur = buffer.len();

        // Request one extra byte; `Vec` will often allocate more.
        buffer.reserve(1);

        // Use all the space it allocated.
        buffer.resize(buffer.capacity(), 0);

        // Read up to that many bytes.
        let n = match crate::io::read(&auxv, &mut buffer[cur..]) {
            Err(crate::io::Errno::INTR) => 0,
            Err(_err) => panic!(),
            Ok(0) => break,
            Ok(n) => n,
        };

        // Account for the number of bytes actually read.
        buffer.resize(cur + n, 0_u8);
    }

    // SAFETY: We loaded from an auxv file into the buffer.
    unsafe { init_from_aux_iter(AuxPointer(buffer.as_ptr().cast())) }
}

/// Process auxv entries from the auxv array pointed to by `auxp`.
///
/// # Safety
///
/// This must be passed a pointer to an auxv array.
///
/// The buffer contains `Elf_aux_t` elements, though it need not be aligned;
/// function uses `read_unaligned` to read from it.
#[cold]
#[must_use]
unsafe fn init_from_aux_iter(aux_iter: impl Iterator<Item = Elf_auxv_t>) -> Option<()> {
    let mut pagesz = 0;
    let mut clktck = 0;
    let mut hwcap = 0;
    let mut hwcap2 = 0;
    let mut minsigstksz = 0;
    let mut execfn = null_mut();
    let mut sysinfo_ehdr = null_mut();
    #[cfg(feature = "runtime")]
    let mut secure = 0;
    #[cfg(feature = "runtime")]
    let mut phdr = null_mut();
    #[cfg(feature = "runtime")]
    let mut phnum = 0;
    #[cfg(feature = "runtime")]
    let mut phent = 0;
    #[cfg(feature = "runtime")]
    let mut entry = 0;
    #[cfg(feature = "runtime")]
    let mut uid = None;
    #[cfg(feature = "runtime")]
    let mut euid = None;
    #[cfg(feature = "runtime")]
    let mut gid = None;
    #[cfg(feature = "runtime")]
    let mut egid = None;
    #[cfg(feature = "runtime")]
    let mut random = null_mut();

    for Elf_auxv_t { a_type, a_val } in aux_iter {
        match a_type as _ {
            AT_PAGESZ => pagesz = a_val as usize,
            AT_CLKTCK => clktck = a_val as usize,
            AT_HWCAP => hwcap = a_val as usize,
            AT_HWCAP2 => hwcap2 = a_val as usize,
            AT_MINSIGSTKSZ => minsigstksz = a_val as usize,
            AT_EXECFN => execfn = check_raw_pointer::<c::c_char>(a_val as *mut _)?.as_ptr(),
            AT_SYSINFO_EHDR => sysinfo_ehdr = check_elf_base(a_val as *mut _)?.as_ptr(),

            AT_BASE => {
                // The `AT_BASE` value can be NULL in a static executable that
                // doesn't use a dynamic linker. If so, ignore it.
                if !a_val.is_null() {
                    let _ = check_elf_base(a_val.cast())?;
                }
            }

            #[cfg(feature = "runtime")]
            AT_SECURE => secure = (a_val as usize != 0) as u8 + 1,
            #[cfg(feature = "runtime")]
            AT_UID => uid = Some(a_val),
            #[cfg(feature = "runtime")]
            AT_EUID => euid = Some(a_val),
            #[cfg(feature = "runtime")]
            AT_GID => gid = Some(a_val),
            #[cfg(feature = "runtime")]
            AT_EGID => egid = Some(a_val),
            #[cfg(feature = "runtime")]
            AT_PHDR => phdr = check_raw_pointer::<Elf_Phdr>(a_val as *mut _)?.as_ptr(),
            #[cfg(feature = "runtime")]
            AT_PHNUM => phnum = a_val as usize,
            #[cfg(feature = "runtime")]
            AT_PHENT => phent = a_val as usize,
            #[cfg(feature = "runtime")]
            AT_ENTRY => entry = a_val as usize,
            #[cfg(feature = "runtime")]
            AT_RANDOM => random = check_raw_pointer::<[u8; 16]>(a_val as *mut _)?.as_ptr(),

            AT_NULL => break,
            _ => (),
        }
    }

    #[cfg(feature = "runtime")]
    assert_eq!(phent, size_of::<Elf_Phdr>());

    // If we're running set-uid or set-gid, enable “secure execution” mode,
    // which doesn't do much, but users may be depending on the things that
    // it does do.
    #[cfg(feature = "runtime")]
    if uid != euid || gid != egid {
        secure = 2;
    }

    // The base and sysinfo_ehdr (if present) matches our platform. Accept the
    // aux values.
    PAGE_SIZE.store(pagesz, Relaxed);
    CLOCK_TICKS_PER_SECOND.store(clktck, Relaxed);
    HWCAP.store(hwcap, Relaxed);
    HWCAP2.store(hwcap2, Relaxed);
    MINSIGSTKSZ.store(minsigstksz, Relaxed);
    EXECFN.store(execfn, Relaxed);
    SYSINFO_EHDR.store(sysinfo_ehdr, Relaxed);
    #[cfg(feature = "runtime")]
    SECURE.store(secure, Relaxed);
    #[cfg(feature = "runtime")]
    PHDR.store(phdr, Relaxed);
    #[cfg(feature = "runtime")]
    PHNUM.store(phnum, Relaxed);
    #[cfg(feature = "runtime")]
    ENTRY.store(entry, Relaxed);
    #[cfg(feature = "runtime")]
    RANDOM.store(random, Relaxed);

    Some(())
}

/// Check that `base` is a valid pointer to the kernel-provided vDSO.
///
/// `base` is some value we got from a `AT_SYSINFO_EHDR` aux record somewhere,
/// which hopefully holds the value of the kernel-provided vDSO in memory. Do a
/// series of checks to be as sure as we can that it's safe to use.
#[cold]
#[must_use]
unsafe fn check_elf_base(base: *const Elf_Ehdr) -> Option<NonNull<Elf_Ehdr>> {
    // If we're reading a 64-bit auxv on a 32-bit platform, we'll see a zero
    // `a_val` because `AT_*` values are never greater than `u32::MAX`. Zero is
    // used by libc's `getauxval` to indicate errors, so it should never be a
    // valid value.
    if base.is_null() {
        return None;
    }

    let hdr = match check_raw_pointer::<Elf_Ehdr>(base as *mut _) {
        Some(hdr) => hdr,
        None => return None,
    };

    let hdr = hdr.as_ref();
    if hdr.e_ident[..SELFMAG] != ELFMAG {
        return None; // Wrong ELF magic
    }
    if !matches!(hdr.e_ident[EI_OSABI], ELFOSABI_SYSV | ELFOSABI_LINUX) {
        return None; // Unrecognized ELF OS ABI
    }
    if hdr.e_ident[EI_ABIVERSION] != ELFABIVERSION {
        return None; // Unrecognized ELF ABI version
    }
    if hdr.e_type != ET_DYN {
        return None; // Wrong ELF type
    }

    // If ELF is extended, we'll need to adjust.
    if hdr.e_ident[EI_VERSION] != EV_CURRENT
        || hdr.e_ehsize as usize != size_of::<Elf_Ehdr>()
        || hdr.e_phentsize as usize != size_of::<Elf_Phdr>()
    {
        return None;
    }
    // We don't currently support extra-large numbers of segments.
    if hdr.e_phnum == PN_XNUM {
        return None;
    }

    // If `e_phoff` is zero, it's more likely that we're looking at memory that
    // has been zeroed than that the kernel has somehow aliased the `Ehdr` and
    // the `Phdr`.
    if hdr.e_phoff < size_of::<Elf_Ehdr>() {
        return None;
    }

    // Verify that the `EI_CLASS`/`EI_DATA`/`e_machine` fields match the
    // architecture we're running as. This helps catch cases where we're
    // running under QEMU.
    if hdr.e_ident[EI_CLASS] != ELFCLASS {
        return None; // Wrong ELF class
    }
    if hdr.e_ident[EI_DATA] != ELFDATA {
        return None; // Wrong ELF data
    }
    if hdr.e_machine != EM_CURRENT {
        return None; // Wrong machine type
    }

    Some(NonNull::new_unchecked(as_ptr(hdr) as *mut _))
}

// Aux reading utilities

// Read auxv records from an array in memory.
struct AuxPointer(*const Elf_auxv_t);

impl Iterator for AuxPointer {
    type Item = Elf_auxv_t;

    #[cold]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let value = read_unaligned(self.0);
            self.0 = self.0.add(1);
            Some(value)
        }
    }
}

// Read auxv records from a file.
#[cfg(not(feature = "alloc"))]
struct AuxFile(OwnedFd);

#[cfg(not(feature = "alloc"))]
impl Iterator for AuxFile {
    type Item = Elf_auxv_t;

    // This implementation does lots of `read`s and it isn't amazing, but
    // hopefully we won't use it often.
    #[cold]
    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0_u8; size_of::<Self::Item>()];
        let mut slice = &mut buf[..];
        while !slice.is_empty() {
            match crate::io::read(&self.0, slice) {
                Ok(0) => panic!("unexpected end of auxv file"),
                Ok(n) => slice = &mut slice[n..],
                Err(crate::io::Errno::INTR) => continue,
                Err(err) => panic!("{:?}", err),
            }
        }
        Some(unsafe { read_unaligned(buf.as_ptr().cast()) })
    }
}
