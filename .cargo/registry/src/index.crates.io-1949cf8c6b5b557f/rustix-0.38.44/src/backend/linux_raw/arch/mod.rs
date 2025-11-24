//! Architecture-specific syscall code.
//!
//! This module also has a `choose` submodule which chooses a scheme and is
//! what most of the `rustix` syscalls use.
//!
//! Compilers should really have intrinsics for making system calls. They're
//! much like regular calls, with custom calling conventions, and calling
//! conventions are otherwise the compiler's job. But for now, use inline asm.
//!
//! The calling conventions for Linux syscalls are [documented here].
//!
//! [documented here]: https://man7.org/linux/man-pages/man2/syscall.2.html
//!
//! # Safety
//!
//! This contains the inline `asm` statements performing the syscall
//! instructions.

#![allow(unsafe_code)]
#![cfg_attr(not(feature = "all-apis"), allow(unused_imports))]
// We'll use as many arguments as syscalls need.
#![allow(clippy::too_many_arguments)]

// These functions always use the machine's syscall instruction, even when it
// isn't the fastest option available.
#[cfg_attr(target_arch = "aarch64", path = "aarch64.rs")]
#[cfg_attr(all(target_arch = "arm", not(thumb_mode)), path = "arm.rs")]
#[cfg_attr(all(target_arch = "arm", thumb_mode), path = "thumb.rs")]
#[cfg_attr(target_arch = "mips", path = "mips.rs")]
#[cfg_attr(target_arch = "mips32r6", path = "mips32r6.rs")]
#[cfg_attr(target_arch = "mips64", path = "mips64.rs")]
#[cfg_attr(target_arch = "mips64r6", path = "mips64r6.rs")]
#[cfg_attr(target_arch = "powerpc64", path = "powerpc64.rs")]
#[cfg_attr(target_arch = "riscv64", path = "riscv64.rs")]
#[cfg_attr(target_arch = "s390x", path = "s390x.rs")]
#[cfg_attr(target_arch = "x86", path = "x86.rs")]
#[cfg_attr(target_arch = "x86_64", path = "x86_64.rs")]
pub(in crate::backend) mod asm;

// On most architectures, the architecture syscall instruction is fast, so use
// it directly.
#[cfg(any(
    target_arch = "arm",
    target_arch = "aarch64",
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_arch = "powerpc64",
    target_arch = "riscv64",
    target_arch = "s390x",
    target_arch = "x86_64",
))]
pub(in crate::backend) use self::asm as choose;

// On 32-bit x86, use vDSO wrappers for all syscalls. We could use the
// architecture syscall instruction (`int 0x80`), but the vDSO kernel_vsyscall
// mechanism is much faster.
#[cfg(target_arch = "x86")]
pub(in crate::backend) use super::vdso_wrappers::x86_via_vdso as choose;

// This would be the code for always using `int 0x80` on 32-bit x86.
//#[cfg(target_arch = "x86")]
//pub(in crate::backend) use self::asm as choose;

// Macros for invoking system calls.
//
// These factor out:
//  - Calling `nr` on the syscall number to convert it into `SyscallNumber`.
//  - Calling `.into()` on each of the arguments to convert them into `ArgReg`.
//  - Qualifying the `syscall*` and `__NR_*` identifiers.
//  - Counting the number of arguments.
macro_rules! syscall {
    ($nr:ident) => {
        $crate::backend::arch::choose::syscall0($crate::backend::reg::nr(
            linux_raw_sys::general::$nr,
        ))
    };

    ($nr:ident, $a0:expr) => {
        $crate::backend::arch::choose::syscall1(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr) => {
        $crate::backend::arch::choose::syscall2(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr) => {
        $crate::backend::arch::choose::syscall3(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {
        $crate::backend::arch::choose::syscall4(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::backend::arch::choose::syscall5(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        $crate::backend::arch::choose::syscall6(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        $crate::backend::arch::choose::syscall7(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
            $a6.into(),
        )
    };
}

// Macro to invoke a syscall that always uses direct assembly, rather than the
// vDSO. Useful when still finding the vDSO.
#[allow(unused_macros)]
macro_rules! syscall_always_asm {
    ($nr:ident) => {
        $crate::backend::arch::asm::syscall0($crate::backend::reg::nr(linux_raw_sys::general::$nr))
    };

    ($nr:ident, $a0:expr) => {
        $crate::backend::arch::asm::syscall1(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr) => {
        $crate::backend::arch::asm::syscall2(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr) => {
        $crate::backend::arch::asm::syscall3(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {
        $crate::backend::arch::asm::syscall4(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::backend::arch::asm::syscall5(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        $crate::backend::arch::asm::syscall6(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        $crate::backend::arch::asm::syscall7(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
            $a6.into(),
        )
    };
}

/// Like `syscall`, but adds the `readonly` attribute to the inline asm, which
/// indicates that the syscall does not mutate any memory.
macro_rules! syscall_readonly {
    ($nr:ident) => {
        $crate::backend::arch::choose::syscall0_readonly($crate::backend::reg::nr(
            linux_raw_sys::general::$nr,
        ))
    };

    ($nr:ident, $a0:expr) => {
        $crate::backend::arch::choose::syscall1_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr) => {
        $crate::backend::arch::choose::syscall2_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr) => {
        $crate::backend::arch::choose::syscall3_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {
        $crate::backend::arch::choose::syscall4_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::backend::arch::choose::syscall5_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        $crate::backend::arch::choose::syscall6_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
        )
    };

    ($nr:ident, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        $crate::backend::arch::choose::syscall7_readonly(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
            $a1.into(),
            $a2.into(),
            $a3.into(),
            $a4.into(),
            $a5.into(),
            $a6.into(),
        )
    };
}

/// Like `syscall`, but indicates that the syscall does not return.
#[cfg(feature = "runtime")]
macro_rules! syscall_noreturn {
    ($nr:ident, $a0:expr) => {
        $crate::backend::arch::choose::syscall1_noreturn(
            $crate::backend::reg::nr(linux_raw_sys::general::$nr),
            $a0.into(),
        )
    };
}
