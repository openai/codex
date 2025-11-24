//! TLS utilities.
//!
//! # Safety
//!
//! This file contains code that reads the raw phdr array pointed to by the
//! kernel-provided AUXV values.
#![allow(unsafe_code)]

use crate::backend::c;
use crate::backend::param::auxv::exe_phdrs;
use core::arch::global_asm;
use core::ptr::{null, NonNull};
use linux_raw_sys::elf::*;

/// For use with [`set_thread_area`].
///
/// [`set_thread_area`]: crate::runtime::set_thread_area
#[cfg(target_arch = "x86")]
pub type UserDesc = linux_raw_sys::general::user_desc;

pub(crate) fn startup_tls_info() -> StartupTlsInfo {
    let mut base = null();
    let mut tls_phdr = null();
    let mut stack_size = 0;

    let (first_phdr, phent, phnum) = exe_phdrs();
    let mut current_phdr = first_phdr.cast::<Elf_Phdr>();

    // The dynamic address of the dynamic section, which we can compare with
    // the `PT_DYNAMIC` header's static address, if present.
    let dynamic_addr: *const c::c_void = unsafe { &_DYNAMIC };

    // SAFETY: We assume the phdr array pointer and length the kernel provided
    // to the process describe a valid phdr array.
    unsafe {
        let phdrs_end = current_phdr.cast::<u8>().add(phnum * phent).cast();
        while current_phdr != phdrs_end {
            let phdr = &*current_phdr;
            current_phdr = current_phdr.cast::<u8>().add(phent).cast();

            match phdr.p_type {
                // Compute the offset from the static virtual addresses
                // in the `p_vaddr` fields to the dynamic addresses. We don't
                // always get a `PT_PHDR` or `PT_DYNAMIC` header, so use
                // whichever one we get.
                PT_PHDR => base = first_phdr.cast::<u8>().wrapping_sub(phdr.p_vaddr),
                PT_DYNAMIC => base = dynamic_addr.cast::<u8>().wrapping_sub(phdr.p_vaddr),

                PT_TLS => tls_phdr = phdr,
                PT_GNU_STACK => stack_size = phdr.p_memsz,
                _ => {}
            }
        }

        if tls_phdr.is_null() {
            StartupTlsInfo {
                addr: NonNull::dangling().as_ptr(),
                mem_size: 0,
                file_size: 0,
                align: 1,
                stack_size: 0,
            }
        } else {
            StartupTlsInfo {
                addr: base.cast::<u8>().wrapping_add((*tls_phdr).p_vaddr).cast(),
                mem_size: (*tls_phdr).p_memsz,
                file_size: (*tls_phdr).p_filesz,
                align: (*tls_phdr).p_align,
                stack_size,
            }
        }
    }
}

extern "C" {
    /// Declare the `_DYNAMIC` symbol so that we can compare its address with
    /// the static address in the `PT_DYNAMIC` header to learn our offset. Use
    /// a weak symbol because `_DYNAMIC` is not always present.
    static _DYNAMIC: c::c_void;
}
// Rust has `extern_weak` but it isn't stable, so use a `global_asm`.
global_asm!(".weak _DYNAMIC");

/// The values returned from [`startup_tls_info`].
///
/// [`startup_tls_info`]: crate::runtime::startup_tls_info
pub struct StartupTlsInfo {
    /// The base address of the TLS segment.
    pub addr: *const c::c_void,
    /// The size of the memory region.
    pub mem_size: usize,
    /// The size beyond which all memory is zero-initialized.
    pub file_size: usize,
    /// The required alignment for the TLS segment.
    pub align: usize,
    /// The requested minimum size for stacks.
    pub stack_size: usize,
}
