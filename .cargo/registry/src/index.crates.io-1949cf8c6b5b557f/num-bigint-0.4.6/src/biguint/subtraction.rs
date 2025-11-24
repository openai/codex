use super::BigUint;

use crate::big_digit::{self, BigDigit};
use crate::UsizePromotion;

use core::cmp::Ordering::{Equal, Greater, Less};
use core::ops::{Sub, SubAssign};
use num_traits::CheckedSub;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;

// Subtract with borrow:
#[cfg(target_arch = "x86_64")]
cfg_64!(
    #[inline]
    fn sbb(borrow: u8, a: u64, b: u64, out: &mut u64) -> u8 {
        // Safety: There are absolutely no safety concerns with calling `_subborrow_u64`.
        // It's just unsafe for API consistency with other intrinsics.
        unsafe { arch::_subborrow_u64(borrow, a, b, out) }
    }
);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
cfg_32!(
    #[inline]
    fn sbb(borrow: u8, a: u32, b: u32, out: &mut u32) -> u8 {
        // Safety: There are absolutely no safety concerns with calling `_subborrow_u32`.
        // It's just unsafe for API consistency with other intrinsics.
        unsafe { arch::_subborrow_u32(borrow, a, b, out) }
    }
);

// fallback for environments where we don't have a subborrow intrinsic
// (copied from the standard library's `borrowing_sub`)
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn sbb(borrow: u8, lhs: BigDigit, rhs: BigDigit, out: &mut BigDigit) -> u8 {
    let (a, b) = lhs.overflowing_sub(rhs);
    let (c, d) = a.overflowing_sub(borrow as BigDigit);
    *out = c;
    u8::from(b || d)
}

pub(super) fn sub2(a: &mut [BigDigit], b: &[BigDigit]) {
    let mut borrow = 0;

    let len = Ord::min(a.len(), b.len());
    let (a_lo, a_hi) = a.split_at_mut(len);
    let (b_lo, b_hi) = b.split_at(len);

    for (a, b) in a_lo.iter_mut().zip(b_lo) {
        borrow = sbb(borrow, *a, *b, a);
    }

    if borrow != 0 {
        for a in a_hi {
            borrow = sbb(borrow, *a, 0, a);
            if borrow == 0 {
                break;
            }
        }
    }

    // note: we're _required_ to fail on underflow
    assert!(
        borrow == 0 && b_hi.iter().all(|x| *x == 0),
        "Cannot subtract b from a because b is larger than a."
    );
}

// Only for the Sub impl. `a` and `b` must have same length.
#[inline]
fn __sub2rev(a: &[BigDigit], b: &mut [BigDigit]) -> u8 {
    debug_assert!(b.len() == a.len());

    let mut borrow = 0;

    for (ai, bi) in a.iter().zip(b) {
        borrow = sbb(borrow, *ai, *bi, bi);
    }

    borrow
}

fn sub2rev(a: &[BigDigit], b: &mut [BigDigit]) {
    debug_assert!(b.len() >= a.len());

    let len = Ord::min(a.len(), b.len());
    let (a_lo, a_hi) = a.split_at(len);
    let (b_lo, b_hi) = b.split_at_mut(len);

    let borrow = __sub2rev(a_lo, b_lo);

    assert!(a_hi.is_empty());

    // note: we're _required_ to fail on underflow
    assert!(
        borrow == 0 && b_hi.iter().all(|x| *x == 0),
        "Cannot subtract b from a because b is larger than a."
    );
}

forward_val_val_binop!(impl Sub for BigUint, sub);
forward_ref_ref_binop!(impl Sub for BigUint, sub);
forward_val_assign!(impl SubAssign for BigUint, sub_assign);

impl Sub<&BigUint> for BigUint {
    type Output = BigUint;

    fn sub(mut self, other: &BigUint) -> BigUint {
        self -= other;
        self
    }
}
impl SubAssign<&BigUint> for BigUint {
    fn sub_assign(&mut self, other: &BigUint) {
        sub2(&mut self.data[..], &other.data[..]);
        self.normalize();
    }
}

impl Sub<BigUint> for &BigUint {
    type Output = BigUint;

    fn sub(self, mut other: BigUint) -> BigUint {
        let other_len = other.data.len();
        if other_len < self.data.len() {
            let lo_borrow = __sub2rev(&self.data[..other_len], &mut other.data);
            other.data.extend_from_slice(&self.data[other_len..]);
            if lo_borrow != 0 {
                sub2(&mut other.data[other_len..], &[1])
            }
        } else {
            sub2rev(&self.data[..], &mut other.data[..]);
        }
        other.normalized()
    }
}

promote_unsigned_scalars!(impl Sub for BigUint, sub);
promote_unsigned_scalars_assign!(impl SubAssign for BigUint, sub_assign);
forward_all_scalar_binop_to_val_val!(impl Sub<u32> for BigUint, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<u64> for BigUint, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<u128> for BigUint, sub);

impl Sub<u32> for BigUint {
    type Output = BigUint;

    #[inline]
    fn sub(mut self, other: u32) -> BigUint {
        self -= other;
        self
    }
}

impl SubAssign<u32> for BigUint {
    fn sub_assign(&mut self, other: u32) {
        sub2(&mut self.data[..], &[other as BigDigit]);
        self.normalize();
    }
}

impl Sub<BigUint> for u32 {
    type Output = BigUint;

    cfg_digit!(
        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            if other.data.len() == 0 {
                other.data.push(self);
            } else {
                sub2rev(&[self], &mut other.data[..]);
            }
            other.normalized()
        }

        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            if other.data.is_empty() {
                other.data.push(self as BigDigit);
            } else {
                sub2rev(&[self as BigDigit], &mut other.data[..]);
            }
            other.normalized()
        }
    );
}

impl Sub<u64> for BigUint {
    type Output = BigUint;

    #[inline]
    fn sub(mut self, other: u64) -> BigUint {
        self -= other;
        self
    }
}

impl SubAssign<u64> for BigUint {
    cfg_digit!(
        #[inline]
        fn sub_assign(&mut self, other: u64) {
            let (hi, lo) = big_digit::from_doublebigdigit(other);
            sub2(&mut self.data[..], &[lo, hi]);
            self.normalize();
        }

        #[inline]
        fn sub_assign(&mut self, other: u64) {
            sub2(&mut self.data[..], &[other as BigDigit]);
            self.normalize();
        }
    );
}

impl Sub<BigUint> for u64 {
    type Output = BigUint;

    cfg_digit!(
        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            while other.data.len() < 2 {
                other.data.push(0);
            }

            let (hi, lo) = big_digit::from_doublebigdigit(self);
            sub2rev(&[lo, hi], &mut other.data[..]);
            other.normalized()
        }

        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            if other.data.is_empty() {
                other.data.push(self);
            } else {
                sub2rev(&[self], &mut other.data[..]);
            }
            other.normalized()
        }
    );
}

impl Sub<u128> for BigUint {
    type Output = BigUint;

    #[inline]
    fn sub(mut self, other: u128) -> BigUint {
        self -= other;
        self
    }
}

impl SubAssign<u128> for BigUint {
    cfg_digit!(
        #[inline]
        fn sub_assign(&mut self, other: u128) {
            let (a, b, c, d) = super::u32_from_u128(other);
            sub2(&mut self.data[..], &[d, c, b, a]);
            self.normalize();
        }

        #[inline]
        fn sub_assign(&mut self, other: u128) {
            let (hi, lo) = big_digit::from_doublebigdigit(other);
            sub2(&mut self.data[..], &[lo, hi]);
            self.normalize();
        }
    );
}

impl Sub<BigUint> for u128 {
    type Output = BigUint;

    cfg_digit!(
        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            while other.data.len() < 4 {
                other.data.push(0);
            }

            let (a, b, c, d) = super::u32_from_u128(self);
            sub2rev(&[d, c, b, a], &mut other.data[..]);
            other.normalized()
        }

        #[inline]
        fn sub(self, mut other: BigUint) -> BigUint {
            while other.data.len() < 2 {
                other.data.push(0);
            }

            let (hi, lo) = big_digit::from_doublebigdigit(self);
            sub2rev(&[lo, hi], &mut other.data[..]);
            other.normalized()
        }
    );
}

impl CheckedSub for BigUint {
    #[inline]
    fn checked_sub(&self, v: &BigUint) -> Option<BigUint> {
        match self.cmp(v) {
            Less => None,
            Equal => Some(Self::ZERO),
            Greater => Some(self.sub(v)),
        }
    }
}
