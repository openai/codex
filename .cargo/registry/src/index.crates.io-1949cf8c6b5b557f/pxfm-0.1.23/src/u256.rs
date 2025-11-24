// /*
//  * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
//  * //
//  * // Redistribution and use in source and binary forms, with or without modification,
//  * // are permitted provided that the following conditions are met:
//  * //
//  * // 1.  Redistributions of source code must retain the above copyright notice, this
//  * // list of conditions and the following disclaimer.
//  * //
//  * // 2.  Redistributions in binary form must reproduce the above copyright notice,
//  * // this list of conditions and the following disclaimer in the documentation
//  * // and/or other materials provided with the distribution.
//  * //
//  * // 3.  Neither the name of the copyright holder nor the names of its
//  * // contributors may be used to endorse or promote products derived from
//  * // this software without specific prior written permission.
//  * //
//  * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//  * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//  * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//  * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//  * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//  * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//  * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//  * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//  * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//  * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//  */
//
// use core::ops;
// use std::cmp::Ordering;
// use std::ops::{Add, BitAnd, BitOrAssign};
//
// const U128_LO_MASK: u128 = u64::MAX as u128;
//
// /// A 256-bit unsigned integer represented as two 128-bit native-endian limbs.
// #[allow(non_camel_case_types)]
// #[derive(Clone, Copy, Debug)]
// pub(crate) struct u256 {
//     pub lo: u128,
//     pub hi: u128,
// }
//
// impl PartialEq for u256 {
//     #[inline]
//     fn eq(&self, other: &Self) -> bool {
//         self.hi == other.hi && self.lo == other.lo
//     }
// }
//
// impl Eq for u256 {}
//
// impl PartialOrd for u256 {
//     #[inline]
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         Some(self.cmp(other))
//     }
// }
//
// impl Ord for u256 {
//     #[inline]
//     fn cmp(&self, other: &Self) -> Ordering {
//         match self.hi.cmp(&other.hi) {
//             Ordering::Equal => self.lo.cmp(&other.lo),
//             ord => ord,
//         }
//     }
// }
//
// pub(crate) fn mulhi_u256(a: u256, b: u256) -> u256 {
//     // // Products
//     let lo_lo = a.lo.expanding_mul(b.lo); // a.lo * b.lo
//     let lo_hi = a.lo.expanding_mul(b.hi); // a.lo * b.hi
//     let hi_lo = a.hi.expanding_mul(b.lo); // a.hi * b.lo
//     let hi_hi = a.hi.expanding_mul(b.hi); // a.hi * b.hi
//
//     let carry = u256::from_u128(lo_lo.hi)
//         .add(u256::from_u128(lo_hi.lo))
//         .add(u256::from_u128(hi_lo.lo));
//     let mid = u256::from_u128(lo_hi.hi)
//         .add(u256::from_u128(hi_lo.hi))
//         .add(u256::from_u128(carry.hi));
//     hi_hi.add(mid)
// }
//
// impl u256 {
//     #[inline]
//     pub(crate) fn view_as_slice(self) -> [u8; 32] {
//         // [(self.hi >> 64) as u64, (self.hi & 0xffff_ffff_ffff_ffff) as u64, (self.lo >> 64) as u64, (self.lo & 0xffff_ffff_ffff_ffff) as u64]
//         let mut out = [0u8; 32];
//         out[16..32].copy_from_slice(&self.hi.to_le_bytes());
//         out[0..16].copy_from_slice(&self.lo.to_le_bytes());
//         out
//     }
//
//     pub(crate) const MAX: Self = Self {
//         lo: u128::MAX,
//         hi: u128::MAX,
//     };
//
//     pub(crate) const ZERO: Self = Self { lo: 0, hi: 0 };
//
//     pub(crate) const ONE: Self = Self { lo: 0, hi: 1 };
//
//     #[inline]
//     pub(crate) const fn from_u128(value: u128) -> Self {
//         Self { lo: value, hi: 0 }
//     }
//
//     #[inline]
//     pub(crate) const fn from_u64(value: u64) -> Self {
//         Self {
//             lo: value as u128,
//             hi: 0,
//         }
//     }
//
//     #[inline]
//     pub(crate) const fn from_u32(value: u32) -> Self {
//         Self {
//             lo: value as u128,
//             hi: 0,
//         }
//     }
//
//     #[inline]
//     pub(crate) const fn leading_zeros(self) -> u32 {
//         let mut leading = self.hi.leading_zeros();
//         if leading > 64 {
//             leading += self.lo.leading_zeros();
//         }
//         leading
//     }
//
//     #[inline]
//     pub(crate) fn lo_u64(self) -> u64 {
//         (self.lo & 0xffff_ffff_ffff_ffff) as u64
//     }
//
//     #[inline]
//     pub(crate) fn to_u64(self) -> u64 {
//         self.lo as u64
//     }
//
//     #[inline]
//     pub(crate) fn to_u32(self) -> u32 {
//         self.lo as u32
//     }
// }
//
// macro_rules! impl_common {
//     ($ty:ty) => {
//         impl ops::BitOr for $ty {
//             type Output = Self;
//
//             fn bitor(mut self, rhs: Self) -> Self::Output {
//                 self.lo |= rhs.lo;
//                 self.hi |= rhs.hi;
//                 self
//             }
//         }
//
//         impl ops::Not for $ty {
//             type Output = Self;
//
//             fn not(mut self) -> Self::Output {
//                 self.lo = !self.lo;
//                 self.hi = !self.hi;
//                 self
//             }
//         }
//
//         impl ops::Shl<u32> for $ty {
//             type Output = Self;
//
//             fn shl(self, _rhs: u32) -> Self::Output {
//                 unimplemented!("only used to meet trait bounds")
//             }
//         }
//     };
// }
//
// impl u256 {
//     #[inline]
//     pub(crate) const fn wrapping_shl(self, shift: u32) -> Self {
//         match shift {
//             0 => self,
//             s if s < 128 => {
//                 let hi = (self.hi << s) | (self.lo >> (128 - s));
//                 let lo = self.lo << s;
//                 u256 { lo, hi }
//             }
//             s if s < 256 => {
//                 let lo = 0;
//                 let hi = self.lo << (s - 128);
//                 u256 { lo, hi }
//             }
//             _ => u256 { lo: 0, hi: 0 },
//         }
//     }
//
//     #[inline]
//     pub(crate) fn wrapping_sub(self, rhs: Self) -> Self {
//         let (lo, carry) = self.lo.overflowing_sub(rhs.lo);
//         let hi = self.hi.wrapping_sub(carry as u128).wrapping_sub(rhs.hi);
//
//         Self { lo, hi }
//     }
//
//     #[inline]
//     pub(crate) fn overflowing_add(self, rhs: Self) -> (Self, bool) {
//         let (lo, carry_lo) = self.lo.overflowing_add(rhs.lo);
//         let (hi_intermediate, carry_hi1) = self.hi.overflowing_add(rhs.hi);
//         let (hi, carry_hi2) = hi_intermediate.overflowing_add(carry_lo as u128);
//
//         let overflow = carry_hi1 || carry_hi2;
//         (Self { lo, hi }, overflow)
//     }
// }
//
// impl_common!(u256);
//
// impl ops::Add<Self> for u256 {
//     type Output = Self;
//
//     #[inline]
//     fn add(self, rhs: Self) -> Self::Output {
//         let (lo, carry) = self.lo.overflowing_add(rhs.lo);
//         let hi = self.hi.wrapping_add(carry as u128).wrapping_add(rhs.hi);
//
//         Self { lo, hi }
//     }
// }
//
// impl BitOrAssign for u256 {
//     #[inline]
//     fn bitor_assign(&mut self, rhs: Self) {
//         self.lo |= rhs.lo;
//         self.hi |= rhs.hi;
//     }
// }
//
// impl ops::Shr<u32> for u256 {
//     type Output = Self;
//
//     #[inline]
//     fn shr(mut self, rhs: u32) -> Self::Output {
//         debug_assert!(rhs < 256, "attempted to shift right with overflow");
//         if rhs >= 256 {
//             return Self { lo: 0, hi: 0 };
//         }
//
//         if rhs == 0 {
//             return self;
//         }
//
//         if rhs < 128 {
//             self.lo >>= rhs;
//             self.lo |= self.hi << (128 - rhs);
//         } else {
//             self.lo = self.hi >> (rhs - 128);
//         }
//
//         if rhs < 128 {
//             self.hi >>= rhs;
//         } else {
//             self.hi = 0;
//         }
//
//         self
//     }
// }
//
// impl BitAnd for u256 {
//     type Output = Self;
//
//     #[inline]
//     fn bitand(self, rhs: Self) -> Self::Output {
//         Self {
//             hi: self.hi & rhs.hi,
//             lo: self.lo & rhs.lo,
//         }
//     }
// }
//
// trait WideningMul {
//     type Output;
//     fn expanding_mul(self, rhs: Self) -> Self::Output;
// }
//
// impl WideningMul for u128 {
//     type Output = u256;
//
//     #[inline]
//     fn expanding_mul(self, rhs: Self) -> u256 {
//         let l0 = self & U128_LO_MASK;
//         let l1 = rhs & U128_LO_MASK;
//         let h0 = self >> 64;
//         let h1 = rhs >> 64;
//
//         let p_ll: u128 = l0.wrapping_mul(l1);
//         let p_lh: u128 = l0.wrapping_mul(h1);
//         let p_hl: u128 = h0.wrapping_mul(l1);
//         let p_hh: u128 = h0.wrapping_mul(h1);
//
//         let s0 = p_hl + (p_ll >> 64);
//         let s1 = (p_ll & U128_LO_MASK) + (s0 << 64);
//         let s2 = p_lh + (s1 >> 64);
//
//         let lo = (p_ll & U128_LO_MASK) + (s2 << 64);
//         let hi = p_hh + (s0 >> 64) + (s2 >> 64);
//
//         u256 { lo, hi }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn test_overflowing_add() {
//         let z0 = u256::MAX;
//         let z1 = u256::MAX;
//         let (k, overflowed) = z0.overflowing_add(z1);
//         assert!(overflowed);
//         assert_eq!(k.lo, u128::MAX - 1);
//     }
//
//     #[test]
//     fn test_mulhi() {
//         let z0 = u256::MAX;
//         let z1 = u256::MAX;
//         let product = mulhi_u256(z0, z1);
//         assert_eq!(product.lo, u128::MAX - 1);
//         assert_eq!(product.hi, u128::MAX);
//     }
// }
