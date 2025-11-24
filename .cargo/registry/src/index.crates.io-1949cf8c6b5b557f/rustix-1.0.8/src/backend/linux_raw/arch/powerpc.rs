//! powerpc Linux system calls.
//!
//! On powerpc, Linux indicates success or failure using `cr0.SO` rather
//! than by returning a negative error code as most other architectures do. In
//! theory we could immediately translate this into a `Result`, and it'd save a
//! few branches. And in theory we could have specialized sequences for use
//! with syscalls that are known to never fail. However, those would require
//! more extensive changes in rustix's platform-independent code. For now, we
//! check the flag and negate the error value to make PowerPC look like other
//! architectures.

use crate::backend::reg::{
    ArgReg, FromAsm, RetReg, SyscallNumber, ToAsm, A0, A1, A2, A3, A4, A5, R0,
};
use core::arch::asm;

#[inline]
pub(in crate::backend) unsafe fn syscall0_readonly(nr: SyscallNumber<'_>) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        lateout("r3") r0,
        lateout("r4") _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        lateout("r4") _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        lateout("r4") _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_noreturn(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> ! {
    asm!(
        "sc",
        "trap",
        in("r0") nr.to_asm(),
        in("r3") a0.to_asm(),
        options(nostack, noreturn)
    )
}

#[inline]
pub(in crate::backend) unsafe fn syscall2(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall2_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall3(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall3_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall4(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall4_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall5(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
    a4: ArgReg<'_, A4>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        inlateout("r7") a4.to_asm() => _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall5_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
    a4: ArgReg<'_, A4>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        inlateout("r7") a4.to_asm() => _,
        lateout("r8") _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall6(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
    a4: ArgReg<'_, A4>,
    a5: ArgReg<'_, A5>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        inlateout("r7") a4.to_asm() => _,
        inlateout("r8") a5.to_asm() => _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall6_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
    a4: ArgReg<'_, A4>,
    a5: ArgReg<'_, A5>,
) -> RetReg<R0> {
    let r0;
    asm!(
        "sc",
        "bns 0f",
        "neg 3, 3",
        "0:",
        inlateout("r0") nr.to_asm() => _,
        inlateout("r3") a0.to_asm() => r0,
        inlateout("r4") a1.to_asm() => _,
        inlateout("r5") a2.to_asm() => _,
        inlateout("r6") a3.to_asm() => _,
        inlateout("r7") a4.to_asm() => _,
        inlateout("r8") a5.to_asm() => _,
        lateout("r9") _,
        lateout("r10") _,
        lateout("r11") _,
        lateout("r12") _,
        lateout("cr0") _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}
