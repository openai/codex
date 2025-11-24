//! [`Zeroize`] impls for ARM64 SIMD registers.

use crate::{atomic_fence, volatile_write, Zeroize};

use core::arch::aarch64::*;

macro_rules! impl_zeroize_for_simd_register {
    ($($type:ty),* $(,)?) => {
        $(
            impl Zeroize for $type {
                #[inline]
                fn zeroize(&mut self) {
                    volatile_write(self, unsafe { core::mem::zeroed() });
                    atomic_fence();
                }
            }
        )+
    };
}

// TODO(tarcieri): other NEON register types?
impl_zeroize_for_simd_register! {
    uint8x8_t,
    uint8x16_t,
    uint16x4_t,
    uint16x8_t,
    uint32x2_t,
    uint32x4_t,
    uint64x1_t,
    uint64x2_t,
}
