/* Copyright 2016 Brian Smith.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
 * SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */

#include "../../limbs/limbs.h"

#include "ecp_nistz384.h"
#include "../bn/internal.h"
#include "../../internal.h"

#include "../../limbs/limbs.inl"

 /* XXX: Here we assume that the conversion from |Carry| to |Limb| is
  * constant-time, but we haven't verified that assumption. TODO: Fix it so
  * we don't need to make that assumption. */


typedef Limb Elem[P384_LIMBS];
typedef Limb ScalarMont[P384_LIMBS];
typedef Limb Scalar[P384_LIMBS];

static const BN_ULONG Q[P384_LIMBS] = {
#if defined(OPENSSL_64_BIT)
  0xffffffff, 0xffffffff00000000, 0xfffffffffffffffe, 0xffffffffffffffff,
  0xffffffffffffffff, 0xffffffffffffffff
#else
  0xffffffff, 0, 0, 0xffffffff, 0xfffffffe, 0xffffffff, 0xffffffff, 0xffffffff,
  0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff
#endif
};

static const BN_ULONG N[P384_LIMBS] = {
#if defined(OPENSSL_64_BIT)
  0xecec196accc52973, 0x581a0db248b0a77a, 0xc7634d81f4372ddf, 0xffffffffffffffff,
  0xffffffffffffffff, 0xffffffffffffffff
#else
  0xccc52973, 0xecec196a, 0x48b0a77a, 0x581a0db2, 0xf4372ddf, 0xc7634d81,
  0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff
#endif
};

static const BN_ULONG ONE[P384_LIMBS] = {
#if defined(OPENSSL_64_BIT)
  0xffffffff00000001, 0xffffffff, 1, 0, 0
#else
  1, 0xffffffff, 0xffffffff, 0, 1, 0, 0, 0, 0, 0
#endif
};

static const Elem Q_PLUS_1_SHR_1 = {
#if defined(OPENSSL_64_BIT)
  0x80000000, 0x7fffffff80000000, 0xffffffffffffffff, 0xffffffffffffffff,
  0xffffffffffffffff, 0x7fffffffffffffff
#else
  0x80000000, 0, 0x80000000, 0x7fffffff, 0xffffffff, 0xffffffff, 0xffffffff,
  0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x7fffffff
#endif
};

static const BN_ULONG Q_N0[] = {
  BN_MONT_CTX_N0(1, 1)
};

static const BN_ULONG N_N0[] = {
  BN_MONT_CTX_N0(0x6ed46089, 0xe88fdc45)
};

/* XXX: MSVC for x86 warns when it fails to inline these functions it should
 * probably inline. */
#if defined(_MSC_VER) && !defined(__clang__) && defined(OPENSSL_X86)
#define INLINE_IF_POSSIBLE __forceinline
#else
#define INLINE_IF_POSSIBLE inline
#endif

static inline Limb is_equal(const Elem a, const Elem b) {
  return LIMBS_equal(a, b, P384_LIMBS);
}

static inline Limb is_zero(const BN_ULONG a[P384_LIMBS]) {
  return LIMBS_are_zero(a, P384_LIMBS);
}

static inline void copy_conditional(Elem r, const Elem a,
                                                const Limb condition) {
  for (size_t i = 0; i < P384_LIMBS; ++i) {
    r[i] = constant_time_select_w(condition, a[i], r[i]);
  }
}


static inline void elem_add(Elem r, const Elem a, const Elem b) {
  LIMBS_add_mod(r, a, b, Q, P384_LIMBS);
}

static inline void elem_sub(Elem r, const Elem a, const Elem b) {
  LIMBS_sub_mod(r, a, b, Q, P384_LIMBS);
}

static void elem_div_by_2(Elem r, const Elem a) {
  /* Consider the case where `a` is even. Then we can shift `a` right one bit
   * and the result will still be valid because we didn't lose any bits and so
   * `(a >> 1) * 2 == a (mod q)`, which is the invariant we must satisfy.
   *
   * The remainder of this comment is considering the case where `a` is odd.
   *
   * Since `a` is odd, it isn't the case that `(a >> 1) * 2 == a (mod q)`
   * because the lowest bit is lost during the shift. For example, consider:
   *
   * ```python
   * q = 2**384 - 2**128 - 2**96 + 2**32 - 1
   * a = 2**383
   * two_a = a * 2 % q
   * assert two_a == 0x100000000ffffffffffffffff00000001
   * ```
   *
   * Notice there how `(2 * a) % q` wrapped around to a smaller odd value. When
   * we divide `two_a` by two (mod q), we need to get the value `2**383`, which
   * we obviously can't get with just a right shift.
   *
   * `q` is odd, and `a` is odd, so `a + q` is even. We could calculate
   * `(a + q) >> 1` and then reduce it mod `q`. However, then we would have to
   * keep track of an extra most significant bit. We can avoid that by instead
   * calculating `(a >> 1) + ((q + 1) >> 1)`. The `1` in `q + 1` is the least
   * significant bit of `a`. `q + 1` is even, which means it can be shifted
   * without losing any bits. Since `q` is odd, `q - 1` is even, so the largest
   * odd field element is `q - 2`. Thus we know that `a <= q - 2`. We know
   * `(q + 1) >> 1` is `(q + 1) / 2` since (`q + 1`) is even. The value of
   * `a >> 1` is `(a - 1)/2` since the shift will drop the least significant
   * bit of `a`, which is 1. Thus:
   *
   * sum  =  ((q + 1) >> 1) + (a >> 1)
   * sum  =  (q + 1)/2 + (a >> 1)       (substituting (q + 1)/2)
   *     <=  (q + 1)/2 + (q - 2 - 1)/2  (substituting a <= q - 2)
   *     <=  (q + 1)/2 + (q - 3)/2      (simplifying)
   *     <=  (q + 1 + q - 3)/2          (factoring out the common divisor)
   *     <=  (2q - 2)/2                 (simplifying)
   *     <=  q - 1                      (simplifying)
   *
   * Thus, no reduction of the sum mod `q` is necessary. */

  Limb is_odd = constant_time_is_nonzero_w(a[0] & 1);

  /* r = a >> 1. */
  Limb carry = a[P384_LIMBS - 1] & 1;
  r[P384_LIMBS - 1] = a[P384_LIMBS - 1] >> 1;
  for (size_t i = 1; i < P384_LIMBS; ++i) {
    Limb new_carry = a[P384_LIMBS - i - 1];
    r[P384_LIMBS - i - 1] =
        (a[P384_LIMBS - i - 1] >> 1) | (carry << (LIMB_BITS - 1));
    carry = new_carry;
  }

  Elem adjusted;
  BN_ULONG carry2 = limbs_add(adjusted, r, Q_PLUS_1_SHR_1, P384_LIMBS);
  dev_assert_secret(carry2 == 0);
  (void)carry2;
  copy_conditional(r, adjusted, is_odd);
}

static inline void elem_mul_mont(Elem r, const Elem a, const Elem b) {
  /* XXX: Not (clearly) constant-time; inefficient.*/
  bn_mul_mont_small(r, a, b, Q, Q_N0, P384_LIMBS);
}

static inline void elem_mul_by_2(Elem r, const Elem a) {
  LIMBS_shl_mod(r, a, Q, P384_LIMBS);
}

static INLINE_IF_POSSIBLE void elem_mul_by_3(Elem r, const Elem a) {
  /* XXX: inefficient. TODO: Replace with an integrated shift + add. */
  Elem doubled;
  elem_add(doubled, a, a);
  elem_add(r, doubled, a);
}

static inline void elem_sqr_mont(Elem r, const Elem a) {
  /* XXX: Inefficient. TODO: Add a dedicated squaring routine. */
  elem_mul_mont(r, a, a);
}

void p384_elem_sub(Elem r, const Elem a, const Elem b) {
  elem_sub(r, a, b);
}

void p384_elem_div_by_2(Elem r, const Elem a) {
  elem_div_by_2(r, a);
}

void p384_elem_mul_mont(Elem r, const Elem a, const Elem b) {
  elem_mul_mont(r, a, b);
}

void p384_elem_neg(Elem r, const Elem a) {
  Limb is_zero = LIMBS_are_zero(a, P384_LIMBS);
  Carry borrow = limbs_sub(r, Q, a, P384_LIMBS);
  dev_assert_secret(borrow == 0);
  (void)borrow;
  for (size_t i = 0; i < P384_LIMBS; ++i) {
    r[i] = constant_time_select_w(is_zero, 0, r[i]);
  }
}


void p384_scalar_mul_mont(ScalarMont r, const ScalarMont a,
                              const ScalarMont b) {
  /* XXX: Inefficient. TODO: Add dedicated multiplication routine. */
  bn_mul_mont_small(r, a, b, N, N_N0, P384_LIMBS);
}


/* TODO(perf): Optimize this. */

static void p384_point_select_w5(P384_POINT *out,
                                     const P384_POINT table[16], size_t index) {
  Elem x; limbs_zero(x, P384_LIMBS);
  Elem y; limbs_zero(y, P384_LIMBS);
  Elem z; limbs_zero(z, P384_LIMBS);

  // TODO: Rewrite in terms of |limbs_select|.
  for (size_t i = 0; i < 16; ++i) {
    crypto_word_t equal = constant_time_eq_w(index, (crypto_word_t)i + 1);
    for (size_t j = 0; j < P384_LIMBS; ++j) {
      x[j] = constant_time_select_w(equal, table[i].X[j], x[j]);
      y[j] = constant_time_select_w(equal, table[i].Y[j], y[j]);
      z[j] = constant_time_select_w(equal, table[i].Z[j], z[j]);
    }
  }

  limbs_copy(out->X, x, P384_LIMBS);
  limbs_copy(out->Y, y, P384_LIMBS);
  limbs_copy(out->Z, z, P384_LIMBS);
}


#include "ecp_nistz384.inl"
