//! arm Linux system calls, using thumb-mode.
//!
//! In thumb-mode, r7 is the frame pointer and is not permitted to be used in
//! an inline asm operand, so we have to use a different register and copy it
//! into r7 inside the inline asm.

use crate::backend::reg::{
    ArgReg, FromAsm, RetReg, SyscallNumber, ToAsm, A0, A1, A2, A3, A4, A5, R0,
};
use core::arch::asm;

#[inline]
pub(in crate::backend) unsafe fn syscall0_readonly(nr: SyscallNumber<'_>) -> RetReg<R0> {
    let r0;
    asm!(
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        lateout("r0") r0,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> RetReg<R0> {
    let r0;
    asm!(
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_noreturn(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> ! {
    asm!(
        "mov r7, {nr}",
        "svc 0",
        nr = in(reg) nr.to_asm(),
        in("r0") a0.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
        in("r4") a4.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
        in("r4") a4.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
        in("r4") a4.to_asm(),
        in("r5") a5.to_asm(),
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
        "mov {tmp}, r7",
        "mov r7, {nr}",
        "svc 0",
        "mov r7, {tmp}",
        nr = in(reg) nr.to_asm(),
        tmp = out(reg) _,
        inlateout("r0") a0.to_asm() => r0,
        in("r1") a1.to_asm(),
        in("r2") a2.to_asm(),
        in("r3") a3.to_asm(),
        in("r4") a4.to_asm(),
        in("r5") a5.to_asm(),
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}
