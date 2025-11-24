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

#include "internal.h"
#include "../../internal.h"


OPENSSL_STATIC_ASSERT(BN_MONT_CTX_N0_LIMBS == 1 || BN_MONT_CTX_N0_LIMBS == 2,
                      "BN_MONT_CTX_N0_LIMBS value is invalid");
OPENSSL_STATIC_ASSERT(sizeof(BN_ULONG) * BN_MONT_CTX_N0_LIMBS == sizeof(uint64_t),
                      "uint64_t is insufficient precision for n0");

// LG_LITTLE_R is log_2(r).
#define LG_LITTLE_R (BN_MONT_CTX_N0_LIMBS * BN_BITS2)

// bn_neg_inv_r_mod_n_u64 calculates the -1/n mod r; i.e. it calculates |v|
// such that u*r - v*n == 1. |r| is the constant defined in |bn_mont_n0|. |n|
// must be odd.
//
// This is derived from |xbinGCD| in Henry S. Warren, Jr.'s "Montgomery
// Multiplication" (http://www.hackersdelight.org/MontgomeryMultiplication.pdf).
// It is very similar to the MODULAR-INVERSE function in Stephen R. Dussé's and
// Burton S. Kaliski Jr.'s "A Cryptographic Library for the Motorola DSP56000"
// (http://link.springer.com/chapter/10.1007%2F3-540-46877-3_21).
//
// This is inspired by Joppe W. Bos's "Constant Time Modular Inversion"
// (http://www.joppebos.com/files/CTInversion.pdf) so that the inversion is
// constant-time with respect to |n|. We assume uint64_t additions,
// subtractions, shifts, and bitwise operations are all constant time, which
// may be a large leap of faith on 32-bit targets. We avoid division and
// multiplication, which tend to be the most problematic in terms of timing
// leaks.
//
// Most GCD implementations return values such that |u*r + v*n == 1|, so the
// caller would have to negate the resultant |v| for the purpose of Montgomery
// multiplication. This implementation does the negation implicitly by doing
// the computations as a difference instead of a sum.
uint64_t bn_neg_inv_mod_r_u64(uint64_t n) {
  dev_assert_secret(n % 2 == 1);

  // alpha == 2**(lg r - 1) == r / 2.
  static const uint64_t alpha = UINT64_C(1) << (LG_LITTLE_R - 1);

  const uint64_t beta = n;

  uint64_t u = 1;
  uint64_t v = 0;

  // The invariant maintained from here on is:
  // 2**(lg r - i) == u*2*alpha - v*beta.
  for (size_t i = 0; i < LG_LITTLE_R; ++i) {
#if BN_BITS2 == 64 && defined(BN_ULLONG)
    dev_assert_secret((BN_ULLONG)(1) << (LG_LITTLE_R - i) ==
           ((BN_ULLONG)u * 2 * alpha) - ((BN_ULLONG)v * beta));
#endif

    // Delete a common factor of 2 in u and v if |u| is even. Otherwise, set
    // |u = (u + beta) / 2| and |v = (v / 2) + alpha|.

    uint64_t u_is_odd = UINT64_C(0) - (u & 1);  // Either 0xff..ff or 0.

    // The addition can overflow, so use Dietz's method for it.
    //
    // Dietz calculates (x+y)/2 by (x xor y)>>1 + x&y. This is valid for all
    // (unsigned) x and y, even when x+y overflows. Evidence for 32-bit values
    // (embedded in 64 bits to so that overflow can be ignored):
    //
    // (declare-fun x () (_ BitVec 64))
    // (declare-fun y () (_ BitVec 64))
    // (assert (let (
    //    (one (_ bv1 64))
    //    (thirtyTwo (_ bv32 64)))
    //    (and
    //      (bvult x (bvshl one thirtyTwo))
    //      (bvult y (bvshl one thirtyTwo))
    //      (not (=
    //        (bvadd (bvlshr (bvxor x y) one) (bvand x y))
    //        (bvlshr (bvadd x y) one)))
    // )))
    // (check-sat)
    uint64_t beta_if_u_is_odd = beta & u_is_odd;  // Either |beta| or 0.
    u = ((u ^ beta_if_u_is_odd) >> 1) + (u & beta_if_u_is_odd);

    uint64_t alpha_if_u_is_odd = alpha & u_is_odd; /* Either |alpha| or 0. */
    v = (v >> 1) + alpha_if_u_is_odd;
  }

  // The invariant now shows that u*r - v*n == 1 since r == 2 * alpha.
#if BN_BITS2 == 64 && defined(BN_ULLONG)
  declassify_assert(1 == ((BN_ULLONG)u * 2 * alpha) - ((BN_ULLONG)v * beta));
#endif

  return v;
}
