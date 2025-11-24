//! mipsel Linux system calls.
//!
//! On mipsel, Linux indicates success or failure using `$a3` rather
//! than by returning a negative error code as most other architectures do.
//!
//! MIPS-family platforms have a special calling convention for `__NR_pipe`,
//! however we use `__NR_pipe2` instead to avoid having to implement it.

use crate::backend::reg::{
    ArgReg, FromAsm, RetReg, SyscallNumber, ToAsm as _, A0, A1, A2, A3, A4, A5, A6, R0,
};
use core::arch::asm;

#[inline]
pub(in crate::backend) unsafe fn syscall0_readonly(nr: SyscallNumber) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall1(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall1_noreturn(nr: SyscallNumber<'_>, a0: ArgReg<'_, A0>) -> ! {
    asm!(
        "syscall",
        "teq $0,$0",
        in("$2" /*$v0*/) nr.to_asm(),
        in("$4" /*$a0*/) a0.to_asm(),
        options(nostack, noreturn)
    )
}

#[inline]
pub(in crate::backend) unsafe fn syscall2(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall2_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall3(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall3_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        lateout("$7" /*$a3*/) err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall4(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall4_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        "syscall",
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(nostack, preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
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
    let x0;
    let err: usize;
    asm!(
        ".set noat",
        "subu $sp, 32",
        "sw {}, 16($sp)",
        "syscall",
        "addu $sp, 32",
        ".set at",
        in(reg) a4.to_asm(),
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
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
    let x0;
    let err: usize;
    asm!(
        ".set noat",
        "subu $sp, 32",
        "sw {}, 16($sp)",
        "syscall",
        "addu $sp, 32",
        ".set at",
        in(reg) a4.to_asm(),
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
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
    let x0;
    let err: usize;
    asm!(
        ".set noat",
        "subu $sp, 32",
        "sw {}, 16($sp)",
        "sw {}, 20($sp)",
        "syscall",
        "addu $sp, 32",
        ".set at",
        in(reg) a4.to_asm(),
        in(reg) a5.to_asm(),
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(preserves_flags)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
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
    let x0;
    let err: usize;
    asm!(
        ".set noat",
        "subu $sp, 32",
        "sw {}, 16($sp)",
        "sw {}, 20($sp)",
        "syscall",
        "addu $sp, 32",
        ".set at",
        in(reg) a4.to_asm(),
        in(reg) a5.to_asm(),
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}

#[inline]
pub(in crate::backend) unsafe fn syscall7_readonly(
    nr: SyscallNumber<'_>,
    a0: ArgReg<'_, A0>,
    a1: ArgReg<'_, A1>,
    a2: ArgReg<'_, A2>,
    a3: ArgReg<'_, A3>,
    a4: ArgReg<'_, A4>,
    a5: ArgReg<'_, A5>,
    a6: ArgReg<'_, A6>,
) -> RetReg<R0> {
    let x0;
    let err: usize;
    asm!(
        ".set noat",
        "subu $sp, 32",
        "sw {}, 16($sp)",
        "sw {}, 20($sp)",
        "sw {}, 24($sp)",
        "syscall",
        "addu $sp, 32",
        ".set at",
        in(reg) a4.to_asm(),
        in(reg) a5.to_asm(),
        in(reg) a6.to_asm(),
        inlateout("$2" /*$v0*/) nr.to_asm() => x0,
        in("$4" /*$a0*/) a0.to_asm(),
        in("$5" /*$a1*/) a1.to_asm(),
        in("$6" /*$a2*/) a2.to_asm(),
        inlateout("$7" /*$a3*/) a3.to_asm() => err,
        lateout("$8" /*$t0*/) _,
        lateout("$9" /*$t1*/) _,
        lateout("$10" /*$t2*/) _,
        lateout("$11" /*$t3*/) _,
        lateout("$12" /*$t4*/) _,
        lateout("$13" /*$t5*/) _,
        lateout("$14" /*$t6*/) _,
        lateout("$15" /*$t7*/) _,
        lateout("$24" /*$t8*/) _,
        lateout("$25" /*$t9*/) _,
        options(preserves_flags, readonly)
    );
    FromAsm::from_asm(if err != 0 {
        (x0 as usize).wrapping_neg() as *mut _
    } else {
        x0
    })
}
