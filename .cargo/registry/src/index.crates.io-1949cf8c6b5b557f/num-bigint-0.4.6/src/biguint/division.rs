use super::addition::__add2;
use super::{cmp_slice, BigUint};

use crate::big_digit::{self, BigDigit, DoubleBigDigit};
use crate::UsizePromotion;

use core::cmp::Ordering::{Equal, Greater, Less};
use core::mem;
use core::ops::{Div, DivAssign, Rem, RemAssign};
use num_integer::Integer;
use num_traits::{CheckedDiv, CheckedEuclid, Euclid, One, ToPrimitive, Zero};

pub(super) const FAST_DIV_WIDE: bool = cfg!(any(target_arch = "x86", target_arch = "x86_64"));

/// Divide a two digit numerator by a one digit divisor, returns quotient and remainder:
///
/// Note: the caller must ensure that both the quotient and remainder will fit into a single digit.
/// This is _not_ true for an arbitrary numerator/denominator.
///
/// (This function also matches what the x86 divide instruction does).
#[cfg(any(miri, not(any(target_arch = "x86", target_arch = "x86_64"))))]
#[inline]
fn div_wide(hi: BigDigit, lo: BigDigit, divisor: BigDigit) -> (BigDigit, BigDigit) {
    debug_assert!(hi < divisor);

    let lhs = big_digit::to_doublebigdigit(hi, lo);
    let rhs = DoubleBigDigit::from(divisor);
    ((lhs / rhs) as BigDigit, (lhs % rhs) as BigDigit)
}

/// x86 and x86_64 can use a real `div` instruction.
#[cfg(all(not(miri), any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn div_wide(hi: BigDigit, lo: BigDigit, divisor: BigDigit) -> (BigDigit, BigDigit) {
    // This debug assertion covers the potential #DE for divisor==0 or a quotient too large for one
    // register, otherwise in release mode it will become a target-specific fault like SIGFPE.
    // This should never occur with the inputs from our few `div_wide` callers.
    debug_assert!(hi < divisor);

    // SAFETY: The `div` instruction only affects registers, reading the explicit operand as the
    // divisor, and implicitly reading RDX:RAX or EDX:EAX as the dividend. The result is implicitly
    // written back to RAX or EAX for the quotient and RDX or EDX for the remainder. No memory is
    // used, and flags are not preserved.
    unsafe {
        let (div, rem);

        cfg_digit!(
            macro_rules! div {
                () => {
                    "div {0:e}"
                };
            }
            macro_rules! div {
                () => {
                    "div {0:r}"
                };
            }
        );

        core::arch::asm!(
            div!(),
            in(reg) divisor,
            inout("dx") hi => rem,
            inout("ax") lo => div,
            options(pure, nomem, nostack),
        );

        (div, rem)
    }
}

/// For small divisors, we can divide without promoting to `DoubleBigDigit` by
/// using half-size pieces of digit, like long-division.
#[inline]
fn div_half(rem: BigDigit, digit: BigDigit, divisor: BigDigit) -> (BigDigit, BigDigit) {
    use crate::big_digit::{HALF, HALF_BITS};

    debug_assert!(rem < divisor && divisor <= HALF);
    let (hi, rem) = ((rem << HALF_BITS) | (digit >> HALF_BITS)).div_rem(&divisor);
    let (lo, rem) = ((rem << HALF_BITS) | (digit & HALF)).div_rem(&divisor);
    ((hi << HALF_BITS) | lo, rem)
}

#[inline]
pub(super) fn div_rem_digit(mut a: BigUint, b: BigDigit) -> (BigUint, BigDigit) {
    if b == 0 {
        panic!("attempt to divide by zero")
    }

    let mut rem = 0;

    if !FAST_DIV_WIDE && b <= big_digit::HALF {
        for d in a.data.iter_mut().rev() {
            let (q, r) = div_half(rem, *d, b);
            *d = q;
            rem = r;
        }
    } else {
        for d in a.data.iter_mut().rev() {
            let (q, r) = div_wide(rem, *d, b);
            *d = q;
            rem = r;
        }
    }

    (a.normalized(), rem)
}

#[inline]
fn rem_digit(a: &BigUint, b: BigDigit) -> BigDigit {
    if b == 0 {
        panic!("attempt to divide by zero")
    }

    let mut rem = 0;

    if !FAST_DIV_WIDE && b <= big_digit::HALF {
        for &digit in a.data.iter().rev() {
            let (_, r) = div_half(rem, digit, b);
            rem = r;
        }
    } else {
        for &digit in a.data.iter().rev() {
            let (_, r) = div_wide(rem, digit, b);
            rem = r;
        }
    }

    rem
}

/// Subtract a multiple.
/// a -= b * c
/// Returns a borrow (if a < b then borrow > 0).
fn sub_mul_digit_same_len(a: &mut [BigDigit], b: &[BigDigit], c: BigDigit) -> BigDigit {
    debug_assert!(a.len() == b.len());

    // carry is between -big_digit::MAX and 0, so to avoid overflow we store
    // offset_carry = carry + big_digit::MAX
    let mut offset_carry = big_digit::MAX;

    for (x, y) in a.iter_mut().zip(b) {
        // We want to calculate sum = x - y * c + carry.
        // sum >= -(big_digit::MAX * big_digit::MAX) - big_digit::MAX
        // sum <= big_digit::MAX
        // Offsetting sum by (big_digit::MAX << big_digit::BITS) puts it in DoubleBigDigit range.
        let offset_sum = big_digit::to_doublebigdigit(big_digit::MAX, *x)
            - big_digit::MAX as DoubleBigDigit
            + offset_carry as DoubleBigDigit
            - *y as DoubleBigDigit * c as DoubleBigDigit;

        let (new_offset_carry, new_x) = big_digit::from_doublebigdigit(offset_sum);
        offset_carry = new_offset_carry;
        *x = new_x;
    }

    // Return the borrow.
    big_digit::MAX - offset_carry
}

fn div_rem(mut u: BigUint, mut d: BigUint) -> (BigUint, BigUint) {
    if d.is_zero() {
        panic!("attempt to divide by zero")
    }
    if u.is_zero() {
        return (BigUint::ZERO, BigUint::ZERO);
    }

    if d.data.len() == 1 {
        if d.data == [1] {
            return (u, BigUint::ZERO);
        }
        let (div, rem) = div_rem_digit(u, d.data[0]);
        // reuse d
        d.data.clear();
        d += rem;
        return (div, d);
    }

    // Required or the q_len calculation below can underflow:
    match u.cmp(&d) {
        Less => return (BigUint::ZERO, u),
        Equal => {
            u.set_one();
            return (u, BigUint::ZERO);
        }
        Greater => {} // Do nothing
    }

    // This algorithm is from Knuth, TAOCP vol 2 section 4.3, algorithm D:
    //
    // First, normalize the arguments so the highest bit in the highest digit of the divisor is
    // set: the main loop uses the highest digit of the divisor for generating guesses, so we
    // want it to be the largest number we can efficiently divide by.
    //
    let shift = d.data.last().unwrap().leading_zeros() as usize;

    if shift == 0 {
        // no need to clone d
        div_rem_core(u, &d.data)
    } else {
        let (q, r) = div_rem_core(u << shift, &(d << shift).data);
        // renormalize the remainder
        (q, r >> shift)
    }
}

pub(super) fn div_rem_ref(u: &BigUint, d: &BigUint) -> (BigUint, BigUint) {
    if d.is_zero() {
        panic!("attempt to divide by zero")
    }
    if u.is_zero() {
        return (BigUint::ZERO, BigUint::ZERO);
    }

    if d.data.len() == 1 {
        if d.data == [1] {
            return (u.clone(), BigUint::ZERO);
        }

        let (div, rem) = div_rem_digit(u.clone(), d.data[0]);
        return (div, rem.into());
    }

    // Required or the q_len calculation below can underflow:
    match u.cmp(d) {
        Less => return (BigUint::ZERO, u.clone()),
        Equal => return (One::one(), BigUint::ZERO),
        Greater => {} // Do nothing
    }

    // This algorithm is from Knuth, TAOCP vol 2 section 4.3, algorithm D:
    //
    // First, normalize the arguments so the highest bit in the highest digit of the divisor is
    // set: the main loop uses the highest digit of the divisor for generating guesses, so we
    // want it to be the largest number we can efficiently divide by.
    //
    let shift = d.data.last().unwrap().leading_zeros() as usize;

    if shift == 0 {
        // no need to clone d
        div_rem_core(u.clone(), &d.data)
    } else {
        let (q, r) = div_rem_core(u << shift, &(d << shift).data);
        // renormalize the remainder
        (q, r >> shift)
    }
}

/// An implementation of the base division algorithm.
/// Knuth, TAOCP vol 2 section 4.3.1, algorithm D, with an improvement from exercises 19-21.
fn div_rem_core(mut a: BigUint, b: &[BigDigit]) -> (BigUint, BigUint) {
    debug_assert!(a.data.len() >= b.len() && b.len() > 1);
    debug_assert!(b.last().unwrap().leading_zeros() == 0);

    // The algorithm works by incrementally calculating "guesses", q0, for the next digit of the
    // quotient. Once we have any number q0 such that (q0 << j) * b <= a, we can set
    //
    //     q += q0 << j
    //     a -= (q0 << j) * b
    //
    // and then iterate until a < b. Then, (q, a) will be our desired quotient and remainder.
    //
    // q0, our guess, is calculated by dividing the last three digits of a by the last two digits of
    // b - this will give us a guess that is close to the actual quotient, but is possibly greater.
    // It can only be greater by 1 and only in rare cases, with probability at most
    // 2^-(big_digit::BITS-1) for random a, see TAOCP 4.3.1 exercise 21.
    //
    // If the quotient turns out to be too large, we adjust it by 1:
    // q -= 1 << j
    // a += b << j

    // a0 stores an additional extra most significant digit of the dividend, not stored in a.
    let mut a0 = 0;

    // [b1, b0] are the two most significant digits of the divisor. They never change.
    let b0 = b[b.len() - 1];
    let b1 = b[b.len() - 2];

    let q_len = a.data.len() - b.len() + 1;
    let mut q = BigUint {
        data: vec![0; q_len],
    };

    for j in (0..q_len).rev() {
        debug_assert!(a.data.len() == b.len() + j);

        let a1 = *a.data.last().unwrap();
        let a2 = a.data[a.data.len() - 2];

        // The first q0 estimate is [a1,a0] / b0. It will never be too small, it may be too large
        // by at most 2.
        let (mut q0, mut r) = if a0 < b0 {
            let (q0, r) = div_wide(a0, a1, b0);
            (q0, r as DoubleBigDigit)
        } else {
            debug_assert!(a0 == b0);
            // Avoid overflowing q0, we know the quotient fits in BigDigit.
            // [a1,a0] = b0 * (1<<BITS - 1) + (a0 + a1)
            (big_digit::MAX, a0 as DoubleBigDigit + a1 as DoubleBigDigit)
        };

        // r = [a1,a0] - q0 * b0
        //
        // Now we want to compute a more precise estimate [a2,a1,a0] / [b1,b0] which can only be
        // less or equal to the current q0.
        //
        // q0 is too large if:
        // [a2,a1,a0] < q0 * [b1,b0]
        // (r << BITS) + a2 < q0 * b1
        while r <= big_digit::MAX as DoubleBigDigit
            && big_digit::to_doublebigdigit(r as BigDigit, a2)
                < q0 as DoubleBigDigit * b1 as DoubleBigDigit
        {
            q0 -= 1;
            r += b0 as DoubleBigDigit;
        }

        // q0 is now either the correct quotient digit, or in rare cases 1 too large.
        // Subtract (q0 << j) from a. This may overflow, in which case we will have to correct.

        let mut borrow = sub_mul_digit_same_len(&mut a.data[j..], b, q0);
        if borrow > a0 {
            // q0 is too large. We need to add back one multiple of b.
            q0 -= 1;
            borrow -= __add2(&mut a.data[j..], b);
        }
        // The top digit of a, stored in a0, has now been zeroed.
        debug_assert!(borrow == a0);

        q.data[j] = q0;

        // Pop off the next top digit of a.
        a0 = a.data.pop().unwrap();
    }

    a.data.push(a0);
    a.normalize();

    debug_assert_eq!(cmp_slice(&a.data, b), Less);

    (q.normalized(), a)
}

forward_val_ref_binop!(impl Div for BigUint, div);
forward_ref_val_binop!(impl Div for BigUint, div);
forward_val_assign!(impl DivAssign for BigUint, div_assign);

impl Div<BigUint> for BigUint {
    type Output = BigUint;

    #[inline]
    fn div(self, other: BigUint) -> BigUint {
        let (q, _) = div_rem(self, other);
        q
    }
}

impl Div<&BigUint> for &BigUint {
    type Output = BigUint;

    #[inline]
    fn div(self, other: &BigUint) -> BigUint {
        let (q, _) = self.div_rem(other);
        q
    }
}
impl DivAssign<&BigUint> for BigUint {
    #[inline]
    fn div_assign(&mut self, other: &BigUint) {
        *self = &*self / other;
    }
}

promote_unsigned_scalars!(impl Div for BigUint, div);
promote_unsigned_scalars_assign!(impl DivAssign for BigUint, div_assign);
forward_all_scalar_binop_to_val_val!(impl Div<u32> for BigUint, div);
forward_all_scalar_binop_to_val_val!(impl Div<u64> for BigUint, div);
forward_all_scalar_binop_to_val_val!(impl Div<u128> for BigUint, div);

impl Div<u32> for BigUint {
    type Output = BigUint;

    #[inline]
    fn div(self, other: u32) -> BigUint {
        let (q, _) = div_rem_digit(self, other as BigDigit);
        q
    }
}
impl DivAssign<u32> for BigUint {
    #[inline]
    fn div_assign(&mut self, other: u32) {
        *self = &*self / other;
    }
}

impl Div<BigUint> for u32 {
    type Output = BigUint;

    #[inline]
    fn div(self, other: BigUint) -> BigUint {
        match other.data.len() {
            0 => panic!("attempt to divide by zero"),
            1 => From::from(self as BigDigit / other.data[0]),
            _ => BigUint::ZERO,
        }
    }
}

impl Div<u64> for BigUint {
    type Output = BigUint;

    #[inline]
    fn div(self, other: u64) -> BigUint {
        let (q, _) = div_rem(self, From::from(other));
        q
    }
}
impl DivAssign<u64> for BigUint {
    #[inline]
    fn div_assign(&mut self, other: u64) {
        // a vec of size 0 does not allocate, so this is fairly cheap
        let temp = mem::replace(self, Self::ZERO);
        *self = temp / other;
    }
}

impl Div<BigUint> for u64 {
    type Output = BigUint;

    cfg_digit!(
        #[inline]
        fn div(self, other: BigUint) -> BigUint {
            match other.data.len() {
                0 => panic!("attempt to divide by zero"),
                1 => From::from(self / u64::from(other.data[0])),
                2 => From::from(self / big_digit::to_doublebigdigit(other.data[1], other.data[0])),
                _ => BigUint::ZERO,
            }
        }

        #[inline]
        fn div(self, other: BigUint) -> BigUint {
            match other.data.len() {
                0 => panic!("attempt to divide by zero"),
                1 => From::from(self / other.data[0]),
                _ => BigUint::ZERO,
            }
        }
    );
}

impl Div<u128> for BigUint {
    type Output = BigUint;

    #[inline]
    fn div(self, other: u128) -> BigUint {
        let (q, _) = div_rem(self, From::from(other));
        q
    }
}

impl DivAssign<u128> for BigUint {
    #[inline]
    fn div_assign(&mut self, other: u128) {
        *self = &*self / other;
    }
}

impl Div<BigUint> for u128 {
    type Output = BigUint;

    cfg_digit!(
        #[inline]
        fn div(self, other: BigUint) -> BigUint {
            use super::u32_to_u128;
            match other.data.len() {
                0 => panic!("attempt to divide by zero"),
                1 => From::from(self / u128::from(other.data[0])),
                2 => From::from(
                    self / u128::from(big_digit::to_doublebigdigit(other.data[1], other.data[0])),
                ),
                3 => From::from(self / u32_to_u128(0, other.data[2], other.data[1], other.data[0])),
                4 => From::from(
                    self / u32_to_u128(other.data[3], other.data[2], other.data[1], other.data[0]),
                ),
                _ => BigUint::ZERO,
            }
        }

        #[inline]
        fn div(self, other: BigUint) -> BigUint {
            match other.data.len() {
                0 => panic!("attempt to divide by zero"),
                1 => From::from(self / other.data[0] as u128),
                2 => From::from(self / big_digit::to_doublebigdigit(other.data[1], other.data[0])),
                _ => BigUint::ZERO,
            }
        }
    );
}

forward_val_ref_binop!(impl Rem for BigUint, rem);
forward_ref_val_binop!(impl Rem for BigUint, rem);
forward_val_assign!(impl RemAssign for BigUint, rem_assign);

impl Rem<BigUint> for BigUint {
    type Output = BigUint;

    #[inline]
    fn rem(self, other: BigUint) -> BigUint {
        if let Some(other) = other.to_u32() {
            &self % other
        } else {
            let (_, r) = div_rem(self, other);
            r
        }
    }
}

impl Rem<&BigUint> for &BigUint {
    type Output = BigUint;

    #[inline]
    fn rem(self, other: &BigUint) -> BigUint {
        if let Some(other) = other.to_u32() {
            self % other
        } else {
            let (_, r) = self.div_rem(other);
            r
        }
    }
}
impl RemAssign<&BigUint> for BigUint {
    #[inline]
    fn rem_assign(&mut self, other: &BigUint) {
        *self = &*self % other;
    }
}

promote_unsigned_scalars!(impl Rem for BigUint, rem);
promote_unsigned_scalars_assign!(impl RemAssign for BigUint, rem_assign);
forward_all_scalar_binop_to_ref_val!(impl Rem<u32> for BigUint, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<u64> for BigUint, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<u128> for BigUint, rem);

impl Rem<u32> for &BigUint {
    type Output = BigUint;

    #[inline]
    fn rem(self, other: u32) -> BigUint {
        rem_digit(self, other as BigDigit).into()
    }
}
impl RemAssign<u32> for BigUint {
    #[inline]
    fn rem_assign(&mut self, other: u32) {
        *self = &*self % other;
    }
}

impl Rem<&BigUint> for u32 {
    type Output = BigUint;

    #[inline]
    fn rem(mut self, other: &BigUint) -> BigUint {
        self %= other;
        From::from(self)
    }
}

macro_rules! impl_rem_assign_scalar {
    ($scalar:ty, $to_scalar:ident) => {
        forward_val_assign_scalar!(impl RemAssign for BigUint, $scalar, rem_assign);
        impl RemAssign<&BigUint> for $scalar {
            #[inline]
            fn rem_assign(&mut self, other: &BigUint) {
                *self = match other.$to_scalar() {
                    None => *self,
                    Some(0) => panic!("attempt to divide by zero"),
                    Some(v) => *self % v
                };
            }
        }
    }
}

// we can scalar %= BigUint for any scalar, including signed types
impl_rem_assign_scalar!(u128, to_u128);
impl_rem_assign_scalar!(usize, to_usize);
impl_rem_assign_scalar!(u64, to_u64);
impl_rem_assign_scalar!(u32, to_u32);
impl_rem_assign_scalar!(u16, to_u16);
impl_rem_assign_scalar!(u8, to_u8);
impl_rem_assign_scalar!(i128, to_i128);
impl_rem_assign_scalar!(isize, to_isize);
impl_rem_assign_scalar!(i64, to_i64);
impl_rem_assign_scalar!(i32, to_i32);
impl_rem_assign_scalar!(i16, to_i16);
impl_rem_assign_scalar!(i8, to_i8);

impl Rem<u64> for BigUint {
    type Output = BigUint;

    #[inline]
    fn rem(self, other: u64) -> BigUint {
        let (_, r) = div_rem(self, From::from(other));
        r
    }
}
impl RemAssign<u64> for BigUint {
    #[inline]
    fn rem_assign(&mut self, other: u64) {
        *self = &*self % other;
    }
}

impl Rem<BigUint> for u64 {
    type Output = BigUint;

    #[inline]
    fn rem(mut self, other: BigUint) -> BigUint {
        self %= other;
        From::from(self)
    }
}

impl Rem<u128> for BigUint {
    type Output = BigUint;

    #[inline]
    fn rem(self, other: u128) -> BigUint {
        let (_, r) = div_rem(self, From::from(other));
        r
    }
}

impl RemAssign<u128> for BigUint {
    #[inline]
    fn rem_assign(&mut self, other: u128) {
        *self = &*self % other;
    }
}

impl Rem<BigUint> for u128 {
    type Output = BigUint;

    #[inline]
    fn rem(mut self, other: BigUint) -> BigUint {
        self %= other;
        From::from(self)
    }
}

impl CheckedDiv for BigUint {
    #[inline]
    fn checked_div(&self, v: &BigUint) -> Option<BigUint> {
        if v.is_zero() {
            return None;
        }
        Some(self.div(v))
    }
}

impl CheckedEuclid for BigUint {
    #[inline]
    fn checked_div_euclid(&self, v: &BigUint) -> Option<BigUint> {
        if v.is_zero() {
            return None;
        }
        Some(self.div_euclid(v))
    }

    #[inline]
    fn checked_rem_euclid(&self, v: &BigUint) -> Option<BigUint> {
        if v.is_zero() {
            return None;
        }
        Some(self.rem_euclid(v))
    }

    fn checked_div_rem_euclid(&self, v: &Self) -> Option<(Self, Self)> {
        Some(self.div_rem_euclid(v))
    }
}

impl Euclid for BigUint {
    #[inline]
    fn div_euclid(&self, v: &BigUint) -> BigUint {
        // trivially same as regular division
        self / v
    }

    #[inline]
    fn rem_euclid(&self, v: &BigUint) -> BigUint {
        // trivially same as regular remainder
        self % v
    }

    fn div_rem_euclid(&self, v: &Self) -> (Self, Self) {
        // trivially same as regular division and remainder
        self.div_rem(v)
    }
}
