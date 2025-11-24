//! s390x Linux system calls.

use crate::backend::reg::{
    ArgReg, FromAsm, RetReg, SyscallNumber, ToAsm as _, A0, A1, A2, A3, A4, A5, R0,
};
use core::arch::asm;

#[inline]
pub(in crate::backend) unsafe fn syscall0_readonly(nr: SyscallNumber<'_>) -> RetReg<R0> {
    let r0;
    asm!(
        "svc 0",
        in("r1") nr.to_asm(),
        lateout("r2") r0,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> RetReg<R0> {
    let r0;
    asm!(
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_noreturn(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> ! {
    asm!(
        "svc 0",
        "j .+2",
        in("r1") nr.to_asm(),
        in("r2") a0.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
        in("r6") a4.to_asm(),
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
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
        in("r6") a4.to_asm(),
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
    if nr.nr == linux_raw_sys::general::__NR_mmap as usize {
        let mut a = [
            a0.to_asm(),
            a1.to_asm(),
            a2.to_asm(),
            a3.to_asm(),
            a4.to_asm(),
            a5.to_asm(),
        ];
        return syscall1(nr, a.as_mut_ptr().into());
    }
    let r0;
    asm!(
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
        in("r6") a4.to_asm(),
        in("r7") a5.to_asm(),
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
    if nr.nr == linux_raw_sys::general::__NR_mmap as usize {
        let a = [
            a0.to_asm(),
            a1.to_asm(),
            a2.to_asm(),
            a3.to_asm(),
            a4.to_asm(),
            a5.to_asm(),
        ];
        return syscall1_readonly(nr, a.as_ptr().into());
    }
    let r0;
    asm!(
        "svc 0",
        in("r1") nr.to_asm(),
        inlateout("r2") a0.to_asm() => r0,
        in("r3") a1.to_asm(),
        in("r4") a2.to_asm(),
        in("r5") a3.to_asm(),
        in("r6") a4.to_asm(),
        in("r7") a5.to_asm(),
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(r0)
}
