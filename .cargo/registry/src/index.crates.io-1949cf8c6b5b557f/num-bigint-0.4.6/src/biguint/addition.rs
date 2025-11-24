use super::{BigUint, IntDigits};

use crate::big_digit::{self, BigDigit};
use crate::UsizePromotion;

use core::iter::Sum;
use core::ops::{Add, AddAssign};
use num_traits::CheckedAdd;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;

// Add with carry:
#[cfg(target_arch = "x86_64")]
cfg_64!(
    #[inline]
    fn adc(carry: u8, a: u64, b: u64, out: &mut u64) -> u8 {
        // Safety: There are absolutely no safety concerns with calling `_addcarry_u64`.
        // It's just unsafe for API consistency with other intrinsics.
        unsafe { arch::_addcarry_u64(carry, a, b, out) }
    }
);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
cfg_32!(
    #[inline]
    fn adc(carry: u8, a: u32, b: u32, out: &mut u32) -> u8 {
        // Safety: There are absolutely no safety concerns with calling `_addcarry_u32`.
        // It's just unsafe for API consistency with other intrinsics.
        unsafe { arch::_addcarry_u32(carry, a, b, out) }
    }
);

// fallback for environments where we don't have an addcarry intrinsic
// (copied from the standard library's `carrying_add`)
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn adc(carry: u8, lhs: BigDigit, rhs: BigDigit, out: &mut BigDigit) -> u8 {
    let (a, b) = lhs.overflowing_add(rhs);
    let (c, d) = a.overflowing_add(carry as BigDigit);
    *out = c;
    u8::from(b || d)
}

/// Two argument addition of raw slices, `a += b`, returning the carry.
///
/// This is used when the data `Vec` might need to resize to push a non-zero carry, so we perform
/// the addition first hoping that it will fit.
///
/// The caller _must_ ensure that `a` is at least as long as `b`.
#[inline]
pub(super) fn __add2(a: &mut [BigDigit], b: &[BigDigit]) -> BigDigit {
    debug_assert!(a.len() >= b.len());

    let mut carry = 0;
    let (a_lo, a_hi) = a.split_at_mut(b.len());

    for (a, b) in a_lo.iter_mut().zip(b) {
        carry = adc(carry, *a, *b, a);
    }

    if carry != 0 {
        for a in a_hi {
            carry = adc(carry, *a, 0, a);
            if carry == 0 {
                break;
            }
        }
    }

    carry as BigDigit
}

/// Two argument addition of raw slices:
/// a += b
///
/// The caller _must_ ensure that a is big enough to store the result - typically this means
/// resizing a to max(a.len(), b.len()) + 1, to fit a possible carry.
pub(super) fn add2(a: &mut [BigDigit], b: &[BigDigit]) {
    let carry = __add2(a, b);

    debug_assert!(carry == 0);
}

forward_all_binop_to_val_ref_commutative!(impl Add for BigUint, add);
forward_val_assign!(impl AddAssign for BigUint, add_assign);

impl Add<&BigUint> for BigUint {
    type Output = BigUint;

    fn add(mut self, other: &BigUint) -> BigUint {
        self += other;
        self
    }
}
impl AddAssign<&BigUint> for BigUint {
    #[inline]
    fn add_assign(&mut self, other: &BigUint) {
        let self_len = self.data.len();
        let carry = if self_len < other.data.len() {
            let lo_carry = __add2(&mut self.data[..], &other.data[..self_len]);
            self.data.extend_from_slice(&other.data[self_len..]);
            __add2(&mut self.data[self_len..], &[lo_carry])
        } else {
            __add2(&mut self.data[..], &other.data[..])
        };
        if carry != 0 {
            self.data.push(carry);
        }
    }
}

promote_unsigned_scalars!(impl Add for BigUint, add);
promote_unsigned_scalars_assign!(impl AddAssign for BigUint, add_assign);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u32> for BigUint, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u64> for BigUint, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u128> for BigUint, add);

impl Add<u32> for BigUint {
    type Output = BigUint;

    #[inline]
    fn add(mut self, other: u32) -> BigUint {
        self += other;
        self
    }
}

impl AddAssign<u32> for BigUint {
    #[inline]
    fn add_assign(&mut self, other: u32) {
        if other != 0 {
            if self.data.is_empty() {
                self.data.push(0);
            }

            let carry = __add2(&mut self.data, &[other as BigDigit]);
            if carry != 0 {
                self.data.push(carry);
            }
        }
    }
}

impl Add<u64> for BigUint {
    type Output = BigUint;

    #[inline]
    fn add(mut self, other: u64) -> BigUint {
        self += other;
        self
    }
}

impl AddAssign<u64> for BigUint {
    cfg_digit!(
        #[inline]
        fn add_assign(&mut self, other: u64) {
            let (hi, lo) = big_digit::from_doublebigdigit(other);
            if hi == 0 {
                *self += lo;
            } else {
                while self.data.len() < 2 {
                    self.data.push(0);
                }

                let carry = __add2(&mut self.data, &[lo, hi]);
                if carry != 0 {
                    self.data.push(carry);
                }
            }
        }

        #[inline]
        fn add_assign(&mut self, other: u64) {
            if other != 0 {
                if self.data.is_empty() {
                    self.data.push(0);
                }

                let carry = __add2(&mut self.data, &[other as BigDigit]);
                if carry != 0 {
                    self.data.push(carry);
                }
            }
        }
    );
}

impl Add<u128> for BigUint {
    type Output = BigUint;

    #[inline]
    fn add(mut self, other: u128) -> BigUint {
        self += other;
        self
    }
}

impl AddAssign<u128> for BigUint {
    cfg_digit!(
        #[inline]
        fn add_assign(&mut self, other: u128) {
            if other <= u128::from(u64::MAX) {
                *self += other as u64
            } else {
                let (a, b, c, d) = super::u32_from_u128(other);
                let carry = if a > 0 {
                    while self.data.len() < 4 {
                        self.data.push(0);
                    }
                    __add2(&mut self.data, &[d, c, b, a])
                } else {
                    debug_assert!(b > 0);
                    while self.data.len() < 3 {
                        self.data.push(0);
                    }
                    __add2(&mut self.data, &[d, c, b])
                };

                if carry != 0 {
                    self.data.push(carry);
                }
            }
        }

        #[inline]
        fn add_assign(&mut self, other: u128) {
            let (hi, lo) = big_digit::from_doublebigdigit(other);
            if hi == 0 {
                *self += lo;
            } else {
                while self.data.len() < 2 {
                    self.data.push(0);
                }

                let carry = __add2(&mut self.data, &[lo, hi]);
                if carry != 0 {
                    self.data.push(carry);
                }
            }
        }
    );
}

impl CheckedAdd for BigUint {
    #[inline]
    fn checked_add(&self, v: &BigUint) -> Option<BigUint> {
        Some(self.add(v))
    }
}

impl_sum_iter_type!(BigUint);
