// Copyright 2014-2016 The OpenSSL Project Authors. All Rights Reserved.
// Copyright (c) 2014, Intel Corporation. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Originally written by Shay Gueron (1, 2), and Vlad Krasnov (1)
// (1) Intel Corporation, Israel Development Center, Haifa, Israel
// (2) University of Haifa, Israel
//
// Reference:
// S.Gueron and V.Krasnov, "Fast Prime Field Elliptic Curve Cryptography with
//                          256 Bit Primes"

#include <ring-core/base.h>

#include "../../limbs/limbs.inl"

#include <stdint.h>

#include "p256-nistz.h"

#if defined(OPENSSL_USE_NISTZ256)

typedef P256_POINT_AFFINE PRECOMP256_ROW[64];

// One converted into the Montgomery domain
static const BN_ULONG ONE_MONT[P256_LIMBS] = {
    TOBN(0x00000000, 0x00000001),
    TOBN(0xffffffff, 0x00000000),
    TOBN(0xffffffff, 0xffffffff),
    TOBN(0x00000000, 0xfffffffe),
};

// Precomputed tables for the default generator
#include "p256-nistz-table.h"

// Recode window to a signed digit, see |ec_GFp_nistp_recode_scalar_bits| in
// util.c for details
static crypto_word_t booth_recode_w5(crypto_word_t in) {
  crypto_word_t s, d;

  s = ~((in >> 5) - 1);
  d = (1 << 6) - in - 1;
  d = (d & s) | (in & ~s);
  d = (d >> 1) + (d & 1);

  return (d << 1) + (s & 1);
}

static crypto_word_t booth_recode_w7(crypto_word_t in) {
  crypto_word_t s, d;

  s = ~((in >> 7) - 1);
  d = (1 << 8) - in - 1;
  d = (d & s) | (in & ~s);
  d = (d >> 1) + (d & 1);

  return (d << 1) + (s & 1);
}

// The `(P256_LIMBS == 8)` case is unreachable for 64-bit targets.
#if defined(OPENSSL_64_BIT) && defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunreachable-code"
#endif

// copy_conditional copies |src| to |dst| if |move| is one and leaves it as-is
// if |move| is zero.
//
// WARNING: this breaks the usual convention of constant-time functions
// returning masks.
static void copy_conditional(BN_ULONG dst[P256_LIMBS],
                             const BN_ULONG src[P256_LIMBS], BN_ULONG move) {
  BN_ULONG mask1 = ((BN_ULONG)0) - move;
  BN_ULONG mask2 = ~mask1;

  dst[0] = (src[0] & mask1) ^ (dst[0] & mask2);
  dst[1] = (src[1] & mask1) ^ (dst[1] & mask2);
  dst[2] = (src[2] & mask1) ^ (dst[2] & mask2);
  dst[3] = (src[3] & mask1) ^ (dst[3] & mask2);
  if (P256_LIMBS == 8) {
    dst[4] = (src[4] & mask1) ^ (dst[4] & mask2);
    dst[5] = (src[5] & mask1) ^ (dst[5] & mask2);
    dst[6] = (src[6] & mask1) ^ (dst[6] & mask2);
    dst[7] = (src[7] & mask1) ^ (dst[7] & mask2);
  }
}

#if defined(__clang__)
#pragma GCC diagnostic pop
#endif

// is_not_zero returns one iff in != 0 and zero otherwise.
//
// WARNING: this breaks the usual convention of constant-time functions
// returning masks.
//
// (define-fun is_not_zero ((in (_ BitVec 64))) (_ BitVec 64)
//   (bvlshr (bvor in (bvsub #x0000000000000000 in)) #x000000000000003f)
// )
//
// (declare-fun x () (_ BitVec 64))
//
// (assert (and (= x #x0000000000000000) (= (is_not_zero x)
// #x0000000000000001))) (check-sat)
//
// (assert (and (not (= x #x0000000000000000)) (= (is_not_zero x)
// #x0000000000000000))) (check-sat)
//
static BN_ULONG is_not_zero(BN_ULONG in) {
  in |= (0 - in);
  in >>= BN_BITS2 - 1;
  return in;
}

#if defined(OPENSSL_X86_64)
// Dispatch between CPU variations. The "_adx" suffixed functions use MULX in
// addition to ADCX/ADOX. MULX is part of BMI2, not ADX, so we must check both
// capabilities.
       void ecp_nistz256_mul_mont(BN_ULONG res[P256_LIMBS],
                                  const BN_ULONG a[P256_LIMBS],
                                  const BN_ULONG b[P256_LIMBS]) {
  if (adx_bmi2_available) {
    ecp_nistz256_mul_mont_adx(res, a, b);
  } else {
    ecp_nistz256_mul_mont_nohw(res, a, b);
  }
}

       void ecp_nistz256_sqr_mont(BN_ULONG res[P256_LIMBS],
                                  const BN_ULONG a[P256_LIMBS]) {
  if (adx_bmi2_available) {
    ecp_nistz256_sqr_mont_adx(res, a);
  } else {
    ecp_nistz256_sqr_mont_nohw(res, a);
  }
}

       void ecp_nistz256_ord_mul_mont(BN_ULONG res[P256_LIMBS],
                                      const BN_ULONG a[P256_LIMBS],
                                      const BN_ULONG b[P256_LIMBS]) {
  if (adx_bmi2_available) {
    ecp_nistz256_ord_mul_mont_adx(res, a, b);
  } else {
    ecp_nistz256_ord_mul_mont_nohw(res, a, b);
  }
}

       void ecp_nistz256_ord_sqr_mont(BN_ULONG res[P256_LIMBS],
                                      const BN_ULONG a[P256_LIMBS],
                                      BN_ULONG rep) {
  if (adx_bmi2_available) {
    ecp_nistz256_ord_sqr_mont_adx(res, a, rep);
  } else {
    ecp_nistz256_ord_sqr_mont_nohw(res, a, rep);
  }
}

static void ecp_nistz256_select_w5(P256_POINT *val, const P256_POINT in_t[16],
                                   int index) {
  if (avx2_available) {
    ecp_nistz256_select_w5_avx2(val, in_t, index);
  } else {
    ecp_nistz256_select_w5_nohw(val, in_t, index);
  }
}

static void ecp_nistz256_select_w7(P256_POINT_AFFINE *val,
                                   const P256_POINT_AFFINE in_t[64],
                                   int index) {
  if (avx2_available) {
    ecp_nistz256_select_w7_avx2(val, in_t, index);
  } else {
    ecp_nistz256_select_w7_nohw(val, in_t, index);
  }
}

       void ecp_nistz256_point_double(P256_POINT *r, const P256_POINT *a) {
  if (adx_bmi2_available) {
    ecp_nistz256_point_double_adx(r, a);
  } else {
    ecp_nistz256_point_double_nohw(r, a);
  }
}

       void ecp_nistz256_point_add(P256_POINT *r, const P256_POINT *a,
                                   const P256_POINT *b) {
  if (adx_bmi2_available) {
    ecp_nistz256_point_add_adx(r, a, b);
  } else {
    ecp_nistz256_point_add_nohw(r, a, b);
  }
}

       void ecp_nistz256_point_add_affine(P256_POINT *r, const P256_POINT *a,
                                          const P256_POINT_AFFINE *b) {
  if (adx_bmi2_available) {
    ecp_nistz256_point_add_affine_adx(r, a, b);
  } else {
    ecp_nistz256_point_add_affine_nohw(r, a, b);
  }
}
#endif  // OPENSSL_X86_64

// r = p * p_scalar
static void ecp_nistz256_windowed_mul(P256_POINT *r,
                                      const BN_ULONG p_scalar[P256_LIMBS],
                                      const BN_ULONG p_x[P256_LIMBS],
                                      const BN_ULONG p_y[P256_LIMBS]) {
  debug_assert_nonsecret(r != NULL);
  debug_assert_nonsecret(p_scalar != NULL);
  debug_assert_nonsecret(p_x != NULL);
  debug_assert_nonsecret(p_y != NULL);

  static const size_t kWindowSize = 5;
  static const crypto_word_t kMask = (1 << (5 /* kWindowSize */ + 1)) - 1;

  // A |P256_POINT| is (3 * 32) = 96 bytes, and the 64-byte alignment should
  // add no more than 63 bytes of overhead. Thus, |table| should require
  // ~1599 ((96 * 16) + 63) bytes of stack space.
  alignas(64) P256_POINT table[16];
  P256_SCALAR_BYTES p_str;
  p256_scalar_bytes_from_limbs(p_str, p_scalar);

  // table[0] is implicitly (0,0,0) (the point at infinity), therefore it is
  // not stored. All other values are actually stored with an offset of -1 in
  // table.
  P256_POINT *row = table;

  limbs_copy(row[1 - 1].X, p_x, P256_LIMBS);
  limbs_copy(row[1 - 1].Y, p_y, P256_LIMBS);
  limbs_copy(row[1 - 1].Z, ONE_MONT, P256_LIMBS);

  ecp_nistz256_point_double(&row[2 - 1], &row[1 - 1]);
  ecp_nistz256_point_add(&row[3 - 1], &row[2 - 1], &row[1 - 1]);
  ecp_nistz256_point_double(&row[4 - 1], &row[2 - 1]);
  ecp_nistz256_point_double(&row[6 - 1], &row[3 - 1]);
  ecp_nistz256_point_double(&row[8 - 1], &row[4 - 1]);
  ecp_nistz256_point_double(&row[12 - 1], &row[6 - 1]);
  ecp_nistz256_point_add(&row[5 - 1], &row[4 - 1], &row[1 - 1]);
  ecp_nistz256_point_add(&row[7 - 1], &row[6 - 1], &row[1 - 1]);
  ecp_nistz256_point_add(&row[9 - 1], &row[8 - 1], &row[1 - 1]);
  ecp_nistz256_point_add(&row[13 - 1], &row[12 - 1], &row[1 - 1]);
  ecp_nistz256_point_double(&row[14 - 1], &row[7 - 1]);
  ecp_nistz256_point_double(&row[10 - 1], &row[5 - 1]);
  ecp_nistz256_point_add(&row[15 - 1], &row[14 - 1], &row[1 - 1]);
  ecp_nistz256_point_add(&row[11 - 1], &row[10 - 1], &row[1 - 1]);
  ecp_nistz256_point_double(&row[16 - 1], &row[8 - 1]);

  BN_ULONG tmp[P256_LIMBS];
  alignas(32) P256_POINT h;
  size_t index = 255;
  crypto_word_t wvalue = p_str[(index - 1) / 8];
  wvalue = (wvalue >> ((index - 1) % 8)) & kMask;

  ecp_nistz256_select_w5(r, table, (int)(booth_recode_w5(wvalue) >> 1));

  while (index >= 5) {
    if (index != 255) {
      size_t off = (index - 1) / 8;

      wvalue = (crypto_word_t)p_str[off] | (crypto_word_t)p_str[off + 1] << 8;
      wvalue = (wvalue >> ((index - 1) % 8)) & kMask;

      wvalue = booth_recode_w5(wvalue);

      ecp_nistz256_select_w5(&h, table, (int)(wvalue >> 1));

      ecp_nistz256_neg(tmp, h.Y);
      copy_conditional(h.Y, tmp, (wvalue & 1));

      ecp_nistz256_point_add(r, r, &h);
    }

    index -= kWindowSize;

    ecp_nistz256_point_double(r, r);
    ecp_nistz256_point_double(r, r);
    ecp_nistz256_point_double(r, r);
    ecp_nistz256_point_double(r, r);
    ecp_nistz256_point_double(r, r);
  }

  // Final window
  wvalue = p_str[0];
  wvalue = (wvalue << 1) & kMask;

  wvalue = booth_recode_w5(wvalue);

  ecp_nistz256_select_w5(&h, table, (int)(wvalue >> 1));

  ecp_nistz256_neg(tmp, h.Y);
  copy_conditional(h.Y, tmp, wvalue & 1);

  ecp_nistz256_point_add(r, r, &h);
}

static crypto_word_t calc_first_wvalue(size_t *index, const uint8_t p_str[33]) {
  static const size_t kWindowSize = 7;
  static const crypto_word_t kMask = (1 << (7 /* kWindowSize */ + 1)) - 1;
  *index = kWindowSize;

  crypto_word_t wvalue = ((crypto_word_t)p_str[0] << 1) & kMask;
  return booth_recode_w7(wvalue);
}

static crypto_word_t calc_wvalue(size_t *index, const uint8_t p_str[33]) {
  static const size_t kWindowSize = 7;
  static const crypto_word_t kMask = (1 << (7 /* kWindowSize */ + 1)) - 1;

  const size_t off = (*index - 1) / 8;
  crypto_word_t wvalue =
      (crypto_word_t)p_str[off] | (crypto_word_t)p_str[off + 1] << 8;
  wvalue = (wvalue >> ((*index - 1) % 8)) & kMask;
  *index += kWindowSize;

  return booth_recode_w7(wvalue);
}

void p256_point_mul(Limb r[3][P256_LIMBS], const Limb p_scalar[P256_LIMBS],
                        const Limb p_x[P256_LIMBS],
                        const Limb p_y[P256_LIMBS]) {
  alignas(32) P256_POINT out;
  ecp_nistz256_windowed_mul(&out, p_scalar, p_x, p_y);

  limbs_copy(r[0], out.X, P256_LIMBS);
  limbs_copy(r[1], out.Y, P256_LIMBS);
  limbs_copy(r[2], out.Z, P256_LIMBS);
}

void p256_point_mul_base(Limb r[3][P256_LIMBS], const Limb scalar[P256_LIMBS]) {
  P256_SCALAR_BYTES p_str;
  p256_scalar_bytes_from_limbs(p_str, scalar);

  // First window
  size_t index = 0;
  crypto_word_t wvalue = calc_first_wvalue(&index, p_str);

  alignas(32) P256_POINT_AFFINE t;
  alignas(32) P256_POINT p;
  ecp_nistz256_select_w7(&t, ecp_nistz256_precomputed[0], (int)(wvalue >> 1));
  ecp_nistz256_neg(p.Z, t.Y);
  copy_conditional(t.Y, p.Z, wvalue & 1);

  // Convert |t| from affine to Jacobian coordinates. We set Z to zero if |t|
  // is infinity and |ONE| otherwise. |t| was computed from the table, so it
  // is infinity iff |wvalue >> 1| is zero.
  limbs_copy(p.X, t.X, P256_LIMBS);
  limbs_copy(p.Y, t.Y, P256_LIMBS);
  limbs_zero(p.Z, P256_LIMBS);
  copy_conditional(p.Z, ONE_MONT, is_not_zero(wvalue >> 1));

  for (int i = 1; i < 37; i++) {
    wvalue = calc_wvalue(&index, p_str);

    ecp_nistz256_select_w7(&t, ecp_nistz256_precomputed[i], (int)(wvalue >> 1));

    alignas(32) BN_ULONG neg_Y[P256_LIMBS];
    ecp_nistz256_neg(neg_Y, t.Y);
    copy_conditional(t.Y, neg_Y, wvalue & 1);

    // Note |ecp_nistz256_point_add_affine| does not work if |p| and |t| are the
    // same non-infinity point.
    ecp_nistz256_point_add_affine(&p, &p, &t);
  }

  limbs_copy(r[0], p.X, P256_LIMBS);
  limbs_copy(r[1], p.Y, P256_LIMBS);
  limbs_copy(r[2], p.Z, P256_LIMBS);
}

void p256_point_mul_base_vartime(Limb r[3][P256_LIMBS],
                                 const Limb g_scalar[P256_LIMBS]) {
  alignas(32) P256_POINT p;
  uint8_t p_str[33];
  OPENSSL_memcpy(p_str, g_scalar, 32);
  p_str[32] = 0;

  // First window
  size_t index = 0;
  size_t wvalue = calc_first_wvalue(&index, p_str);

  // Convert |p| from affine to Jacobian coordinates. We set Z to zero if |p|
  // is infinity and |ONE_MONT| otherwise. |p| was computed from the table, so
  // it is infinity iff |wvalue >> 1| is zero.
  if ((wvalue >> 1) != 0) {
    OPENSSL_memcpy(p.X, &ecp_nistz256_precomputed[0][(wvalue >> 1) - 1].X,
                   sizeof(p.X));
    OPENSSL_memcpy(p.Y, &ecp_nistz256_precomputed[0][(wvalue >> 1) - 1].Y,
                   sizeof(p.Y));
    OPENSSL_memcpy(p.Z, ONE_MONT, sizeof(p.Z));
  } else {
    OPENSSL_memset(p.X, 0, sizeof(p.X));
    OPENSSL_memset(p.Y, 0, sizeof(p.Y));
    OPENSSL_memset(p.Z, 0, sizeof(p.Z));
  }

  if ((wvalue & 1) == 1) {
    ecp_nistz256_neg(p.Y, p.Y);
  }

  for (int i = 1; i < 37; i++) {
    wvalue = calc_wvalue(&index, p_str);
    if ((wvalue >> 1) == 0) {
      continue;
    }

    alignas(32) P256_POINT_AFFINE t;
    OPENSSL_memcpy(&t, &ecp_nistz256_precomputed[i][(wvalue >> 1) - 1],
                   sizeof(t));
    if ((wvalue & 1) == 1) {
      ecp_nistz256_neg(t.Y, t.Y);
    }

    // Note |ecp_nistz256_point_add_affine| does not work if |p| and |t| are
    // the same non-infinity point, so it is important that we compute the
    // |g_scalar| term before the |p_scalar| term.
    ecp_nistz256_point_add_affine(&p, &p, &t);
  }


  limbs_copy(r[0], p.X, P256_LIMBS);
  limbs_copy(r[1], p.Y, P256_LIMBS);
  limbs_copy(r[2], p.Z, P256_LIMBS);
}

#endif /* defined(OPENSSL_USE_NISTZ256) */
