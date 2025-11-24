// Copyright 2016-2023 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use super::{
    elem::{binary_op, binary_op_assign},
    elem_sqr_mul, elem_sqr_mul_acc, PublicModulus, *,
};

pub(super) const NUM_LIMBS: usize = 384 / LIMB_BITS;

pub static COMMON_OPS: CommonOps = CommonOps {
    num_limbs: elem::NumLimbs::P384,

    q: PublicModulus {
        p: limbs_from_hex("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff"),
        rr: PublicElem::from_hex("10000000200000000fffffffe000000000000000200000000fffffffe00000001"),
    },
    n: PublicElem::from_hex("ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf581a0db248b0a77aecec196accc52973"),

    a: PublicElem::from_hex("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffbfffffffc0000000000000003fffffffc"),
    b: PublicElem::from_hex("cd08114b604fbff9b62b21f41f022094e3374bee94938ae277f2209b1920022ef729add87a4c32ec081188719d412dcc"),

    elem_mul_mont: p384_elem_mul_mont,
    elem_sqr_mont: p384_elem_sqr_mont,
};

pub(super) static GENERATOR: (PublicElem<R>, PublicElem<R>) = (
    PublicElem::from_hex("4d3aadc2299e1513812ff723614ede2b6454868459a30eff879c3afc541b4d6e20e378e2a0d6ce383dd0756649c0b528"),
    PublicElem::from_hex("2b78abc25a15c5e9dd8002263969a840c6c3521968f4ffd98bade7562e83b050a1bfa8bf7bb4a9ac23043dad4b03a4fe"),
);

pub static PRIVATE_KEY_OPS: PrivateKeyOps = PrivateKeyOps {
    common: &COMMON_OPS,
    elem_inv_squared: p384_elem_inv_squared,
    point_mul_base_impl: p384_point_mul_base_impl,
    point_mul_impl: p384_point_mul,
    point_add_jacobian_impl: p384_point_add,
};

fn p384_elem_inv_squared(q: &Modulus<Q>, a: &Elem<R>) -> Elem<R> {
    // Calculate a**-2 (mod q) == a**(q - 3) (mod q)
    //
    // The exponent (q - 3) is:
    //
    //    0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe\
    //      ffffffff0000000000000000fffffffc

    #[inline]
    fn sqr_mul(q: &Modulus<Q>, a: &Elem<R>, squarings: LeakyWord, b: &Elem<R>) -> Elem<R> {
        elem_sqr_mul(&COMMON_OPS, a, squarings, b, q.cpu())
    }

    #[inline]
    fn sqr_mul_acc(q: &Modulus<Q>, a: &mut Elem<R>, squarings: LeakyWord, b: &Elem<R>) {
        elem_sqr_mul_acc(&COMMON_OPS, a, squarings, b, q.cpu())
    }

    let b_1 = &a;
    let b_11 = sqr_mul(q, b_1, 1, b_1);
    let b_111 = sqr_mul(q, &b_11, 1, b_1);
    let f_11 = sqr_mul(q, &b_111, 3, &b_111);
    let fff = sqr_mul(q, &f_11, 6, &f_11);
    let fff_111 = sqr_mul(q, &fff, 3, &b_111);
    let fffffff_11 = sqr_mul(q, &fff_111, 15, &fff_111);

    let fffffffffffffff = sqr_mul(q, &fffffff_11, 30, &fffffff_11);

    let ffffffffffffffffffffffffffffff = sqr_mul(q, &fffffffffffffff, 60, &fffffffffffffff);

    // ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
    let mut acc = sqr_mul(
        q,
        &ffffffffffffffffffffffffffffff,
        120,
        &ffffffffffffffffffffffffffffff,
    );

    // fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff_111
    sqr_mul_acc(q, &mut acc, 15, &fff_111);

    // fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff
    sqr_mul_acc(q, &mut acc, 1 + 30, &fffffff_11);
    sqr_mul_acc(q, &mut acc, 2, &b_11);

    // fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff
    // 0000000000000000fffffff_11
    sqr_mul_acc(q, &mut acc, 64 + 30, &fffffff_11);

    // fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff
    // 0000000000000000fffffffc
    q.elem_square(&mut acc);
    q.elem_square(&mut acc);

    acc
}

fn p384_point_mul_base_impl(a: &Scalar, cpu: cpu::Features) -> Point {
    // XXX: Not efficient. TODO: Precompute multiples of the generator.
    let generator = (Elem::from(&GENERATOR.0), Elem::from(&GENERATOR.1));
    PRIVATE_KEY_OPS.point_mul(a, &generator, cpu)
}

pub static PUBLIC_KEY_OPS: PublicKeyOps = PublicKeyOps {
    common: &COMMON_OPS,
};

pub static SCALAR_OPS: ScalarOps = ScalarOps {
    common: &COMMON_OPS,
    scalar_mul_mont: p384_scalar_mul_mont,
};

pub static PUBLIC_SCALAR_OPS: PublicScalarOps = PublicScalarOps {
    scalar_ops: &SCALAR_OPS,
    public_key_ops: &PUBLIC_KEY_OPS,
    twin_mul: |g_scalar, p_scalar, p_xy, cpu| {
        twin_mul_inefficient(&PRIVATE_KEY_OPS, g_scalar, p_scalar, p_xy, cpu)
    },

    q_minus_n: PublicElem::from_hex("389cb27e0bc8d21fa7e5f24cb74f58851313e696333ad68c"),

    // TODO: Use an optimized variable-time implementation.
    scalar_inv_to_mont_vartime: |s, cpu| PRIVATE_SCALAR_OPS.scalar_inv_to_mont(s, cpu),
};

pub static PRIVATE_SCALAR_OPS: PrivateScalarOps = PrivateScalarOps {
    scalar_ops: &SCALAR_OPS,

    oneRR_mod_n: PublicScalar::from_hex("c84ee012b39bf213fb05b7a28266895d40d49174aab1cc5bc3e483afcb82947ff3d81e5df1aa4192d319b2419b409a9"),
    scalar_inv_to_mont: p384_scalar_inv_to_mont,
};

fn p384_scalar_inv_to_mont(a: Scalar<R>, _cpu: cpu::Features) -> Scalar<R> {
    // Calculate the modular inverse of scalar |a| using Fermat's Little
    // Theorem:
    //
    //    a**-1 (mod n) == a**(n - 2) (mod n)
    //
    // The exponent (n - 2) is:
    //
    //    0xffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf\
    //      581a0db248b0a77aecec196accc52971

    fn mul(a: &Scalar<R>, b: &Scalar<R>) -> Scalar<R> {
        binary_op(p384_scalar_mul_mont, a, b)
    }

    fn sqr(a: &Scalar<R>) -> Scalar<R> {
        binary_op(p384_scalar_mul_mont, a, a)
    }

    fn sqr_mut(a: &mut Scalar<R>) {
        unary_op_from_binary_op_assign(p384_scalar_mul_mont, a);
    }

    // Returns (`a` squared `squarings` times) * `b`.
    fn sqr_mul(a: &Scalar<R>, squarings: LeakyWord, b: &Scalar<R>) -> Scalar<R> {
        debug_assert!(squarings >= 1);
        let mut tmp = sqr(a);
        for _ in 1..squarings {
            sqr_mut(&mut tmp);
        }
        mul(&tmp, b)
    }

    // Sets `acc` = (`acc` squared `squarings` times) * `b`.
    fn sqr_mul_acc(acc: &mut Scalar<R>, squarings: LeakyWord, b: &Scalar<R>) {
        debug_assert!(squarings >= 1);
        for _ in 0..squarings {
            sqr_mut(acc);
        }
        binary_op_assign(p384_scalar_mul_mont, acc, b)
    }

    // Indexes into `d`.
    const B_1: usize = 0;
    const B_11: usize = 1;
    const B_101: usize = 2;
    const B_111: usize = 3;
    const B_1001: usize = 4;
    const B_1011: usize = 5;
    const B_1101: usize = 6;
    const B_1111: usize = 7;
    const DIGIT_COUNT: usize = 8;

    let mut d = [Scalar::zero(); DIGIT_COUNT];
    d[B_1] = a;
    let b_10 = sqr(&d[B_1]);
    for i in B_11..DIGIT_COUNT {
        d[i] = mul(&d[i - 1], &b_10);
    }

    let ff = sqr_mul(&d[B_1111], 0 + 4, &d[B_1111]);
    let ffff = sqr_mul(&ff, 0 + 8, &ff);
    let ffffffff = sqr_mul(&ffff, 0 + 16, &ffff);

    let ffffffffffffffff = sqr_mul(&ffffffff, 0 + 32, &ffffffff);

    let ffffffffffffffffffffffff = sqr_mul(&ffffffffffffffff, 0 + 32, &ffffffff);

    // ffffffffffffffffffffffffffffffffffffffffffffffff
    let mut acc = sqr_mul(&ffffffffffffffffffffffff, 0 + 96, &ffffffffffffffffffffffff);

    // The rest of the exponent, in binary, is:
    //
    //    1100011101100011010011011000000111110100001101110010110111011111
    //    0101100000011010000011011011001001001000101100001010011101111010
    //    1110110011101100000110010110101011001100110001010010100101110001

    #[allow(clippy::cast_possible_truncation)]
    static REMAINING_WINDOWS: [(u8, u8); 39] = [
        (2, B_11 as u8),
        (3 + 3, B_111 as u8),
        (1 + 2, B_11 as u8),
        (3 + 2, B_11 as u8),
        (1 + 4, B_1001 as u8),
        (4, B_1011 as u8),
        (6 + 4, B_1111 as u8),
        (3, B_101 as u8),
        (4 + 1, B_1 as u8),
        (4, B_1011 as u8),
        (4, B_1001 as u8),
        (1 + 4, B_1101 as u8),
        (4, B_1101 as u8),
        (4, B_1111 as u8),
        (1 + 4, B_1011 as u8),
        (6 + 4, B_1101 as u8),
        (5 + 4, B_1101 as u8),
        (4, B_1011 as u8),
        (2 + 4, B_1001 as u8),
        (2 + 1, B_1 as u8),
        (3 + 4, B_1011 as u8),
        (4 + 3, B_101 as u8),
        (2 + 3, B_111 as u8),
        (1 + 4, B_1111 as u8),
        (1 + 4, B_1011 as u8),
        (4, B_1011 as u8),
        (2 + 3, B_111 as u8),
        (1 + 2, B_11 as u8),
        (5 + 2, B_11 as u8),
        (2 + 4, B_1011 as u8),
        (1 + 3, B_101 as u8),
        (1 + 2, B_11 as u8),
        (2 + 2, B_11 as u8),
        (2 + 2, B_11 as u8),
        (3 + 3, B_101 as u8),
        (2 + 3, B_101 as u8),
        (2 + 3, B_101 as u8),
        (2, B_11 as u8),
        (3 + 1, B_1 as u8),
    ];

    for &(squarings, digit) in &REMAINING_WINDOWS[..] {
        sqr_mul_acc(&mut acc, LeakyWord::from(squarings), &d[usize::from(digit)]);
    }

    acc
}

unsafe extern "C" fn p384_elem_sqr_mont(
    r: *mut Limb,   // [COMMON_OPS.num_limbs]
    a: *const Limb, // [COMMON_OPS.num_limbs]
) {
    // XXX: Inefficient. TODO: Make a dedicated squaring routine.
    unsafe {
        p384_elem_mul_mont(r, a, a);
    }
}

prefixed_extern! {
    fn p384_elem_mul_mont(
        r: *mut Limb,   // [COMMON_OPS.num_limbs]
        a: *const Limb, // [COMMON_OPS.num_limbs]
        b: *const Limb, // [COMMON_OPS.num_limbs]
    );

    fn p384_point_add(
        r: *mut Limb,   // [3][COMMON_OPS.num_limbs]
        a: *const Limb, // [3][COMMON_OPS.num_limbs]
        b: *const Limb, // [3][COMMON_OPS.num_limbs]
    );
    fn p384_point_mul(
        r: *mut Limb,          // [3][COMMON_OPS.num_limbs]
        p_scalar: *const Limb, // [COMMON_OPS.num_limbs]
        p_x: *const Limb,      // [COMMON_OPS.num_limbs]
        p_y: *const Limb,      // [COMMON_OPS.num_limbs]
    );

    fn p384_scalar_mul_mont(
        r: *mut Limb,   // [COMMON_OPS.num_limbs]
        a: *const Limb, // [COMMON_OPS.num_limbs]
        b: *const Limb, // [COMMON_OPS.num_limbs]
    );
}
