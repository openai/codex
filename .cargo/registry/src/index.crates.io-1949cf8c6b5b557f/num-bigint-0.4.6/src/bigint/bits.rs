use super::BigInt;
use super::Sign::{Minus, NoSign, Plus};

use crate::big_digit::{self, BigDigit, DoubleBigDigit};
use crate::biguint::IntDigits;

use alloc::vec::Vec;
use core::cmp::Ordering::{Equal, Greater, Less};
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};
use num_traits::{ToPrimitive, Zero};

// Negation in two's complement.
// acc must be initialized as 1 for least-significant digit.
//
// When negating, a carry (acc == 1) means that all the digits
// considered to this point were zero. This means that if all the
// digits of a negative BigInt have been considered, carry must be
// zero as we cannot have negative zero.
//
//    01 -> ...f    ff
//    ff -> ...f    01
// 01 00 -> ...f ff 00
// 01 01 -> ...f fe ff
// 01 ff -> ...f fe 01
// ff 00 -> ...f 01 00
// ff 01 -> ...f 00 ff
// ff ff -> ...f 00 01
#[inline]
fn negate_carry(a: BigDigit, acc: &mut DoubleBigDigit) -> BigDigit {
    *acc += DoubleBigDigit::from(!a);
    let lo = *acc as BigDigit;
    *acc >>= big_digit::BITS;
    lo
}

// + 1 & -ff = ...0 01 & ...f 01 = ...0 01 = + 1
// +ff & - 1 = ...0 ff & ...f ff = ...0 ff = +ff
// answer is pos, has length of a
fn bitand_pos_neg(a: &mut [BigDigit], b: &[BigDigit]) {
    let mut carry_b = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai &= twos_b;
    }
    debug_assert!(b.len() > a.len() || carry_b == 0);
}

// - 1 & +ff = ...f ff & ...0 ff = ...0 ff = +ff
// -ff & + 1 = ...f 01 & ...0 01 = ...0 01 = + 1
// answer is pos, has length of b
fn bitand_neg_pos(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_a = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        *ai = twos_a & bi;
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => a.truncate(b.len()),
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().cloned());
        }
    }
}

// - 1 & -ff = ...f ff & ...f 01 = ...f 01 = - ff
// -ff & - 1 = ...f 01 & ...f ff = ...f 01 = - ff
// -ff & -fe = ...f 01 & ...f 02 = ...f 00 = -100
// answer is neg, has length of longest with a possible carry
fn bitand_neg_neg(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_a = 1;
    let mut carry_b = 1;
    let mut carry_and = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai = negate_carry(twos_a & twos_b, &mut carry_and);
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    debug_assert!(b.len() > a.len() || carry_b == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => {
            for ai in a[b.len()..].iter_mut() {
                let twos_a = negate_carry(*ai, &mut carry_a);
                *ai = negate_carry(twos_a, &mut carry_and);
            }
            debug_assert!(carry_a == 0);
        }
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().map(|&bi| {
                let twos_b = negate_carry(bi, &mut carry_b);
                negate_carry(twos_b, &mut carry_and)
            }));
            debug_assert!(carry_b == 0);
        }
    }
    if carry_and != 0 {
        a.push(1);
    }
}

forward_val_val_binop!(impl BitAnd for BigInt, bitand);
forward_ref_val_binop!(impl BitAnd for BigInt, bitand);

// do not use forward_ref_ref_binop_commutative! for bitand so that we can
// clone as needed, avoiding over-allocation
impl BitAnd<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn bitand(self, other: &BigInt) -> BigInt {
        match (self.sign, other.sign) {
            (NoSign, _) | (_, NoSign) => BigInt::ZERO,
            (Plus, Plus) => BigInt::from(&self.data & &other.data),
            (Plus, Minus) => self.clone() & other,
            (Minus, Plus) => other.clone() & self,
            (Minus, Minus) => {
                // forward to val-ref, choosing the larger to clone
                if self.len() >= other.len() {
                    self.clone() & other
                } else {
                    other.clone() & self
                }
            }
        }
    }
}

impl BitAnd<&BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn bitand(mut self, other: &BigInt) -> BigInt {
        self &= other;
        self
    }
}

forward_val_assign!(impl BitAndAssign for BigInt, bitand_assign);

impl BitAndAssign<&BigInt> for BigInt {
    fn bitand_assign(&mut self, other: &BigInt) {
        match (self.sign, other.sign) {
            (NoSign, _) => {}
            (_, NoSign) => self.set_zero(),
            (Plus, Plus) => {
                self.data &= &other.data;
                if self.data.is_zero() {
                    self.sign = NoSign;
                }
            }
            (Plus, Minus) => {
                bitand_pos_neg(self.digits_mut(), other.digits());
                self.normalize();
            }
            (Minus, Plus) => {
                bitand_neg_pos(self.digits_mut(), other.digits());
                self.sign = Plus;
                self.normalize();
            }
            (Minus, Minus) => {
                bitand_neg_neg(self.digits_mut(), other.digits());
                self.normalize();
            }
        }
    }
}

// + 1 | -ff = ...0 01 | ...f 01 = ...f 01 = -ff
// +ff | - 1 = ...0 ff | ...f ff = ...f ff = - 1
// answer is neg, has length of b
fn bitor_pos_neg(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_b = 1;
    let mut carry_or = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai = negate_carry(*ai | twos_b, &mut carry_or);
    }
    debug_assert!(b.len() > a.len() || carry_b == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => {
            a.truncate(b.len());
        }
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().map(|&bi| {
                let twos_b = negate_carry(bi, &mut carry_b);
                negate_carry(twos_b, &mut carry_or)
            }));
            debug_assert!(carry_b == 0);
        }
    }
    // for carry_or to be non-zero, we would need twos_b == 0
    debug_assert!(carry_or == 0);
}

// - 1 | +ff = ...f ff | ...0 ff = ...f ff = - 1
// -ff | + 1 = ...f 01 | ...0 01 = ...f 01 = -ff
// answer is neg, has length of a
fn bitor_neg_pos(a: &mut [BigDigit], b: &[BigDigit]) {
    let mut carry_a = 1;
    let mut carry_or = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        *ai = negate_carry(twos_a | bi, &mut carry_or);
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    if a.len() > b.len() {
        for ai in a[b.len()..].iter_mut() {
            let twos_a = negate_carry(*ai, &mut carry_a);
            *ai = negate_carry(twos_a, &mut carry_or);
        }
        debug_assert!(carry_a == 0);
    }
    // for carry_or to be non-zero, we would need twos_a == 0
    debug_assert!(carry_or == 0);
}

// - 1 | -ff = ...f ff | ...f 01 = ...f ff = -1
// -ff | - 1 = ...f 01 | ...f ff = ...f ff = -1
// answer is neg, has length of shortest
fn bitor_neg_neg(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_a = 1;
    let mut carry_b = 1;
    let mut carry_or = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai = negate_carry(twos_a | twos_b, &mut carry_or);
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    debug_assert!(b.len() > a.len() || carry_b == 0);
    if a.len() > b.len() {
        a.truncate(b.len());
    }
    // for carry_or to be non-zero, we would need twos_a == 0 or twos_b == 0
    debug_assert!(carry_or == 0);
}

forward_val_val_binop!(impl BitOr for BigInt, bitor);
forward_ref_val_binop!(impl BitOr for BigInt, bitor);

// do not use forward_ref_ref_binop_commutative! for bitor so that we can
// clone as needed, avoiding over-allocation
impl BitOr<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn bitor(self, other: &BigInt) -> BigInt {
        match (self.sign, other.sign) {
            (NoSign, _) => other.clone(),
            (_, NoSign) => self.clone(),
            (Plus, Plus) => BigInt::from(&self.data | &other.data),
            (Plus, Minus) => other.clone() | self,
            (Minus, Plus) => self.clone() | other,
            (Minus, Minus) => {
                // forward to val-ref, choosing the smaller to clone
                if self.len() <= other.len() {
                    self.clone() | other
                } else {
                    other.clone() | self
                }
            }
        }
    }
}

impl BitOr<&BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn bitor(mut self, other: &BigInt) -> BigInt {
        self |= other;
        self
    }
}

forward_val_assign!(impl BitOrAssign for BigInt, bitor_assign);

impl BitOrAssign<&BigInt> for BigInt {
    fn bitor_assign(&mut self, other: &BigInt) {
        match (self.sign, other.sign) {
            (_, NoSign) => {}
            (NoSign, _) => self.clone_from(other),
            (Plus, Plus) => self.data |= &other.data,
            (Plus, Minus) => {
                bitor_pos_neg(self.digits_mut(), other.digits());
                self.sign = Minus;
                self.normalize();
            }
            (Minus, Plus) => {
                bitor_neg_pos(self.digits_mut(), other.digits());
                self.normalize();
            }
            (Minus, Minus) => {
                bitor_neg_neg(self.digits_mut(), other.digits());
                self.normalize();
            }
        }
    }
}

// + 1 ^ -ff = ...0 01 ^ ...f 01 = ...f 00 = -100
// +ff ^ - 1 = ...0 ff ^ ...f ff = ...f 00 = -100
// answer is neg, has length of longest with a possible carry
fn bitxor_pos_neg(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_b = 1;
    let mut carry_xor = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai = negate_carry(*ai ^ twos_b, &mut carry_xor);
    }
    debug_assert!(b.len() > a.len() || carry_b == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => {
            for ai in a[b.len()..].iter_mut() {
                let twos_b = !0;
                *ai = negate_carry(*ai ^ twos_b, &mut carry_xor);
            }
        }
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().map(|&bi| {
                let twos_b = negate_carry(bi, &mut carry_b);
                negate_carry(twos_b, &mut carry_xor)
            }));
            debug_assert!(carry_b == 0);
        }
    }
    if carry_xor != 0 {
        a.push(1);
    }
}

// - 1 ^ +ff = ...f ff ^ ...0 ff = ...f 00 = -100
// -ff ^ + 1 = ...f 01 ^ ...0 01 = ...f 00 = -100
// answer is neg, has length of longest with a possible carry
fn bitxor_neg_pos(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_a = 1;
    let mut carry_xor = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        *ai = negate_carry(twos_a ^ bi, &mut carry_xor);
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => {
            for ai in a[b.len()..].iter_mut() {
                let twos_a = negate_carry(*ai, &mut carry_a);
                *ai = negate_carry(twos_a, &mut carry_xor);
            }
            debug_assert!(carry_a == 0);
        }
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().map(|&bi| {
                let twos_a = !0;
                negate_carry(twos_a ^ bi, &mut carry_xor)
            }));
        }
    }
    if carry_xor != 0 {
        a.push(1);
    }
}

// - 1 ^ -ff = ...f ff ^ ...f 01 = ...0 fe = +fe
// -ff & - 1 = ...f 01 ^ ...f ff = ...0 fe = +fe
// answer is pos, has length of longest
fn bitxor_neg_neg(a: &mut Vec<BigDigit>, b: &[BigDigit]) {
    let mut carry_a = 1;
    let mut carry_b = 1;
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        let twos_a = negate_carry(*ai, &mut carry_a);
        let twos_b = negate_carry(bi, &mut carry_b);
        *ai = twos_a ^ twos_b;
    }
    debug_assert!(a.len() > b.len() || carry_a == 0);
    debug_assert!(b.len() > a.len() || carry_b == 0);
    match Ord::cmp(&a.len(), &b.len()) {
        Greater => {
            for ai in a[b.len()..].iter_mut() {
                let twos_a = negate_carry(*ai, &mut carry_a);
                let twos_b = !0;
                *ai = twos_a ^ twos_b;
            }
            debug_assert!(carry_a == 0);
        }
        Equal => {}
        Less => {
            let extra = &b[a.len()..];
            a.extend(extra.iter().map(|&bi| {
                let twos_a = !0;
                let twos_b = negate_carry(bi, &mut carry_b);
                twos_a ^ twos_b
            }));
            debug_assert!(carry_b == 0);
        }
    }
}

forward_all_binop_to_val_ref_commutative!(impl BitXor for BigInt, bitxor);

impl BitXor<&BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn bitxor(mut self, other: &BigInt) -> BigInt {
        self ^= other;
        self
    }
}

forward_val_assign!(impl BitXorAssign for BigInt, bitxor_assign);

impl BitXorAssign<&BigInt> for BigInt {
    fn bitxor_assign(&mut self, other: &BigInt) {
        match (self.sign, other.sign) {
            (_, NoSign) => {}
            (NoSign, _) => self.clone_from(other),
            (Plus, Plus) => {
                self.data ^= &other.data;
                if self.data.is_zero() {
                    self.sign = NoSign;
                }
            }
            (Plus, Minus) => {
                bitxor_pos_neg(self.digits_mut(), other.digits());
                self.sign = Minus;
                self.normalize();
            }
            (Minus, Plus) => {
                bitxor_neg_pos(self.digits_mut(), other.digits());
                self.normalize();
            }
            (Minus, Minus) => {
                bitxor_neg_neg(self.digits_mut(), other.digits());
                self.sign = Plus;
                self.normalize();
            }
        }
    }
}

pub(super) fn set_negative_bit(x: &mut BigInt, bit: u64, value: bool) {
    debug_assert_eq!(x.sign, Minus);
    let data = &mut x.data;

    let bits_per_digit = u64::from(big_digit::BITS);
    if bit >= bits_per_digit * data.len() as u64 {
        if !value {
            data.set_bit(bit, true);
        }
    } else {
        // If the Uint number is
        //   ... 0  x 1 0 ... 0
        // then the two's complement is
        //   ... 1 !x 1 0 ... 0
        //            |-- bit at position 'trailing_zeros'
        // where !x is obtained from x by flipping each bit
        let trailing_zeros = data.trailing_zeros().unwrap();
        if bit > trailing_zeros {
            data.set_bit(bit, !value);
        } else if bit == trailing_zeros && !value {
            // Clearing the bit at position `trailing_zeros` is dealt with by doing
            // similarly to what `bitand_neg_pos` does, except we start at digit
            // `bit_index`. All digits below `bit_index` are guaranteed to be zero,
            // so initially we have `carry_in` = `carry_out` = 1. Furthermore, we
            // stop traversing the digits when there are no more carries.
            let bit_index = (bit / bits_per_digit).to_usize().unwrap();
            let bit_mask = (1 as BigDigit) << (bit % bits_per_digit);
            let mut digit_iter = data.digits_mut().iter_mut().skip(bit_index);
            let mut carry_in = 1;
            let mut carry_out = 1;

            let digit = digit_iter.next().unwrap();
            let twos_in = negate_carry(*digit, &mut carry_in);
            let twos_out = twos_in & !bit_mask;
            *digit = negate_carry(twos_out, &mut carry_out);

            for digit in digit_iter {
                if carry_in == 0 && carry_out == 0 {
                    // Exit the loop since no more digits can change
                    break;
                }
                let twos = negate_carry(*digit, &mut carry_in);
                *digit = negate_carry(twos, &mut carry_out);
            }

            if carry_out != 0 {
                // All digits have been traversed and there is a carry
                debug_assert_eq!(carry_in, 0);
                data.digits_mut().push(1);
            }
        } else if bit < trailing_zeros && value {
            // Flip each bit from position 'bit' to 'trailing_zeros', both inclusive
            //       ... 1 !x 1 0 ... 0 ... 0
            //                        |-- bit at position 'bit'
            //                |-- bit at position 'trailing_zeros'
            // bit_mask:      1 1 ... 1 0 .. 0
            // This is done by xor'ing with the bit_mask
            let index_lo = (bit / bits_per_digit).to_usize().unwrap();
            let index_hi = (trailing_zeros / bits_per_digit).to_usize().unwrap();
            let bit_mask_lo = big_digit::MAX << (bit % bits_per_digit);
            let bit_mask_hi =
                big_digit::MAX >> (bits_per_digit - 1 - (trailing_zeros % bits_per_digit));
            let digits = data.digits_mut();

            if index_lo == index_hi {
                digits[index_lo] ^= bit_mask_lo & bit_mask_hi;
            } else {
                digits[index_lo] = bit_mask_lo;
                for digit in &mut digits[index_lo + 1..index_hi] {
                    *digit = big_digit::MAX;
                }
                digits[index_hi] ^= bit_mask_hi;
            }
        } else {
            // We end up here in two cases:
            //   bit == trailing_zeros && value: Bit is already set
            //   bit < trailing_zeros && !value: Bit is already cleared
        }
    }
}
