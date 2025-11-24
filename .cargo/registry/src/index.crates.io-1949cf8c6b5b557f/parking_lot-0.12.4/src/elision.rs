// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::sync::atomic::AtomicUsize;

// Extension trait to add lock elision primitives to atomic types
pub trait AtomicElisionExt {
    type IntType;

    // Perform a compare_exchange and start a transaction
    fn elision_compare_exchange_acquire(
        &self,
        current: Self::IntType,
        new: Self::IntType,
    ) -> Result<Self::IntType, Self::IntType>;

    // Perform a fetch_sub and end a transaction
    fn elision_fetch_sub_release(&self, val: Self::IntType) -> Self::IntType;
}

// Indicates whether the target architecture supports lock elision
#[inline]
pub fn have_elision() -> bool {
    cfg!(all(
        feature = "hardware-lock-elision",
        any(target_arch = "x86", target_arch = "x86_64"),
    ))
}

// This implementation is never actually called because it is guarded by
// have_elision().
#[cfg(not(all(
    feature = "hardware-lock-elision",
    any(target_arch = "x86", target_arch = "x86_64")
)))]
impl AtomicElisionExt for AtomicUsize {
    type IntType = usize;

    #[inline]
    fn elision_compare_exchange_acquire(&self, _: usize, _: usize) -> Result<usize, usize> {
        unreachable!();
    }

    #[inline]
    fn elision_fetch_sub_release(&self, _: usize) -> usize {
        unreachable!();
    }
}

#[cfg(all(
    feature = "hardware-lock-elision",
    any(target_arch = "x86", target_arch = "x86_64")
))]
impl AtomicElisionExt for AtomicUsize {
    type IntType = usize;

    #[inline]
    fn elision_compare_exchange_acquire(&self, current: usize, new: usize) -> Result<usize, usize> {
        unsafe {
            use core::arch::asm;
            let prev: usize;
            #[cfg(target_pointer_width = "32")]
            asm!(
                "xacquire",
                "lock",
                "cmpxchg [{:e}], {:e}",
                in(reg) self,
                in(reg) new,
                inout("eax") current => prev,
            );
            #[cfg(target_pointer_width = "64")]
            asm!(
                "xacquire",
                "lock",
                "cmpxchg [{}], {}",
                in(reg) self,
                in(reg) new,
                inout("rax") current => prev,
            );
            if prev == current {
                Ok(prev)
            } else {
                Err(prev)
            }
        }
    }

    #[inline]
    fn elision_fetch_sub_release(&self, val: usize) -> usize {
        unsafe {
            use core::arch::asm;
            let prev: usize;
            #[cfg(target_pointer_width = "32")]
            asm!(
                "xrelease",
                "lock",
                "xadd [{:e}], {:e}",
                in(reg) self,
                inout(reg) val.wrapping_neg() => prev,
            );
            #[cfg(target_pointer_width = "64")]
            asm!(
                "xrelease",
                "lock",
                "xadd [{}], {}",
                in(reg) self,
                inout(reg) val.wrapping_neg() => prev,
            );
            prev
        }
    }
}
