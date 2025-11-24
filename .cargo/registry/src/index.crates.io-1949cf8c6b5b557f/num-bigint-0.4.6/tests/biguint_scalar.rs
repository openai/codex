use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};

use std::panic::catch_unwind;

mod consts;
use crate::consts::*;

#[macro_use]
mod macros;

#[test]
fn test_scalar_add() {
    fn check(x: &BigUint, y: &BigUint, z: &BigUint) {
        let (x, y, z) = (x.clone(), y.clone(), z.clone());
        assert_unsigned_scalar_op!(x + y == z);
        assert_unsigned_scalar_assign_op!(x += y == z);
    }

    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        check(&a, &b, &c);
        check(&b, &a, &c);
    }
}

#[test]
fn test_scalar_sub() {
    fn check(x: &BigUint, y: &BigUint, z: &BigUint) {
        let (x, y, z) = (x.clone(), y.clone(), z.clone());
        assert_unsigned_scalar_op!(x - y == z);
        assert_unsigned_scalar_assign_op!(x -= y == z);
    }

    for elm in SUM_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        check(&c, &a, &b);
        check(&c, &b, &a);
    }
}

#[test]
fn test_scalar_mul() {
    fn check(x: &BigUint, y: &BigUint, z: &BigUint) {
        let (x, y, z) = (x.clone(), y.clone(), z.clone());
        assert_unsigned_scalar_op!(x * y == z);
        assert_unsigned_scalar_assign_op!(x *= y == z);
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        check(&a, &b, &c);
        check(&b, &a, &c);
    }
}

#[test]
fn test_scalar_rem_noncommutative() {
    assert_eq!(5u8 % BigUint::from(7u8), BigUint::from(5u8));
    assert_eq!(BigUint::from(5u8) % 7u8, BigUint::from(5u8));
}

#[test]
fn test_scalar_div_rem() {
    fn check(x: &BigUint, y: &BigUint, z: &BigUint, r: &BigUint) {
        let (x, y, z, r) = (x.clone(), y.clone(), z.clone(), r.clone());
        assert_unsigned_scalar_op!(x / y == z);
        assert_unsigned_scalar_op!(x % y == r);
        assert_unsigned_scalar_assign_op!(x /= y == z);
        assert_unsigned_scalar_assign_op!(x %= y == r);
    }

    for elm in MUL_TRIPLES.iter() {
        let (a_vec, b_vec, c_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);

        if !a.is_zero() {
            check(&c, &a, &b, &Zero::zero());
        }

        if !b.is_zero() {
            check(&c, &b, &a, &Zero::zero());
        }
    }

    for elm in DIV_REM_QUADRUPLES.iter() {
        let (a_vec, b_vec, c_vec, d_vec) = *elm;
        let a = BigUint::from_slice(a_vec);
        let b = BigUint::from_slice(b_vec);
        let c = BigUint::from_slice(c_vec);
        let d = BigUint::from_slice(d_vec);

        if !b.is_zero() {
            check(&a, &b, &c, &d);
            assert_unsigned_scalar_op!(a / b == c);
            assert_unsigned_scalar_op!(a % b == d);
            assert_unsigned_scalar_assign_op!(a /= b == c);
            assert_unsigned_scalar_assign_op!(a %= b == d);
        }
    }
}

#[test]
fn test_scalar_div_rem_zero() {
    catch_unwind(|| BigUint::zero() / 0u32).unwrap_err();
    catch_unwind(|| BigUint::zero() % 0u32).unwrap_err();
    catch_unwind(|| BigUint::one() / 0u32).unwrap_err();
    catch_unwind(|| BigUint::one() % 0u32).unwrap_err();
}
