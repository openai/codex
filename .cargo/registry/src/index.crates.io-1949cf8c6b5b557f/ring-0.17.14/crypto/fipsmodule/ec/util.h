// Copyright 2015 The BoringSSL Authors
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

#include <ring-core/base.h>

#include "../../internal.h"

#if defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wconversion"
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif


// This function looks at 5+1 scalar bits (5 current, 1 adjacent less
// significant bit), and recodes them into a signed digit for use in fast point
// multiplication: the use of signed rather than unsigned digits means that
// fewer points need to be precomputed, given that point inversion is easy (a
// precomputed point dP makes -dP available as well).
//
// BACKGROUND:
//
// Signed digits for multiplication were introduced by Booth ("A signed binary
// multiplication technique", Quart. Journ. Mech. and Applied Math., vol. IV,
// pt. 2 (1951), pp. 236-240), in that case for multiplication of integers.
// Booth's original encoding did not generally improve the density of nonzero
// digits over the binary representation, and was merely meant to simplify the
// handling of signed factors given in two's complement; but it has since been
// shown to be the basis of various signed-digit representations that do have
// further advantages, including the wNAF, using the following general
// approach:
//
// (1) Given a binary representation
//
//       b_k  ...  b_2  b_1  b_0,
//
//     of a nonnegative integer (b_k in {0, 1}), rewrite it in digits 0, 1, -1
//     by using bit-wise subtraction as follows:
//
//        b_k     b_(k-1)  ...  b_2  b_1  b_0
//      -         b_k      ...  b_3  b_2  b_1  b_0
//       -----------------------------------------
//        s_(k+1) s_k      ...  s_3  s_2  s_1  s_0
//
//     A left-shift followed by subtraction of the original value yields a new
//     representation of the same value, using signed bits s_i = b_(i-1) - b_i.
//     This representation from Booth's paper has since appeared in the
//     literature under a variety of different names including "reversed binary
//     form", "alternating greedy expansion", "mutual opposite form", and
//     "sign-alternating {+-1}-representation".
//
//     An interesting property is that among the nonzero bits, values 1 and -1
//     strictly alternate.
//
// (2) Various window schemes can be applied to the Booth representation of
//     integers: for example, right-to-left sliding windows yield the wNAF
//     (a signed-digit encoding independently discovered by various researchers
//     in the 1990s), and left-to-right sliding windows yield a left-to-right
//     equivalent of the wNAF (independently discovered by various researchers
//     around 2004).
//
// To prevent leaking information through side channels in point multiplication,
// we need to recode the given integer into a regular pattern: sliding windows
// as in wNAFs won't do, we need their fixed-window equivalent -- which is a few
// decades older: we'll be using the so-called "modified Booth encoding" due to
// MacSorley ("High-speed arithmetic in binary computers", Proc. IRE, vol. 49
// (1961), pp. 67-91), in a radix-2^5 setting.  That is, we always combine five
// signed bits into a signed digit:
//
//       s_(5j + 4) s_(5j + 3) s_(5j + 2) s_(5j + 1) s_(5j)
//
// The sign-alternating property implies that the resulting digit values are
// integers from -16 to 16.
//
// Of course, we don't actually need to compute the signed digits s_i as an
// intermediate step (that's just a nice way to see how this scheme relates
// to the wNAF): a direct computation obtains the recoded digit from the
// six bits b_(5j + 4) ... b_(5j - 1).
//
// This function takes those six bits as an integer (0 .. 63), writing the
// recoded digit to *sign (0 for positive, 1 for negative) and *digit (absolute
// value, in the range 0 .. 16).  Note that this integer essentially provides
// the input bits "shifted to the left" by one position: for example, the input
// to compute the least significant recoded digit, given that there's no bit
// b_-1, has to be b_4 b_3 b_2 b_1 b_0 0.
//
// DOUBLING CASE:
//
// Point addition formulas for short Weierstrass curves are often incomplete.
// Edge cases such as P + P or P + ∞ must be handled separately. This
// complicates constant-time requirements. P + ∞ cannot be avoided (any window
// may be zero) and is handled with constant-time selects. P + P (where P is not
// ∞) usually is not. Instead, windowing strategies are chosen to avoid this
// case. Whether this happens depends on the group order.
//
// Let w be the window width (in this function, w = 5). The non-trivial doubling
// case in single-point scalar multiplication may occur if and only if the
// 2^(w-1) bit of the group order is zero.
//
// Note the above only holds if the scalar is fully reduced and the group order
// is a prime that is much larger than 2^w. It also only holds when windows
// are applied from most significant to least significant, doubling between each
// window. It does not apply to more complex table strategies such as
// |EC_nistz256_method|.
//
// PROOF:
//
// Let n be the group order. Let l be the number of bits needed to represent n.
// Assume there exists some 0 <= k < n such that signed w-bit windowed
// multiplication hits the doubling case.
//
// Windowed multiplication consists of iterating over groups of s_i (defined
// above based on k's binary representation) from most to least significant. At
// iteration i (for i = ..., 3w, 2w, w, 0, starting from the most significant
// window), we:
//
//  1. Double the accumulator A, w times. Let A_i be the value of A at this
//     point.
//
//  2. Set A to T_i + A_i, where T_i is a precomputed multiple of P
//     corresponding to the window s_(i+w-1) ... s_i.
//
// Let j be the index such that A_j = T_j ≠ ∞. Looking at A_i and T_i as
// multiples of P, define a_i and t_i to be scalar coefficients of A_i and T_i.
// Thus a_j = t_j ≠ 0 (mod n). Note a_i and t_i may not be reduced mod n. t_i is
// the value of the w signed bits s_(i+w-1) ... s_i. a_i is computed as a_i =
// 2^w * (a_(i+w) + t_(i+w)).
//
// t_i is bounded by -2^(w-1) <= t_i <= 2^(w-1). Additionally, we may write it
// in terms of unsigned bits b_i. t_i consists of signed bits s_(i+w-1) ... s_i.
// This is computed as:
//
//         b_(i+w-2) b_(i+w-3)  ...  b_i      b_(i-1)
//      -  b_(i+w-1) b_(i+w-2)  ...  b_(i+1)  b_i
//       --------------------------------------------
//   t_i = s_(i+w-1) s_(i+w-2)  ...  s_(i+1)  s_i
//
// Observe that b_(i+w-2) through b_i occur in both terms. Let x be the integer
// represented by that bit string, i.e. 2^(w-2)*b_(i+w-2) + ... + b_i.
//
//   t_i = (2*x + b_(i-1)) - (2^(w-1)*b_(i+w-1) + x)
//       = x - 2^(w-1)*b_(i+w-1) + b_(i-1)
//
// Or, using C notation for bit operations:
//
//   t_i = (k>>i) & ((1<<(w-1)) - 1) - (k>>i) & (1<<(w-1)) + (k>>(i-1)) & 1
//
// Note b_(i-1) is added in left-shifted by one (or doubled) from its place.
// This is compensated by t_(i-w)'s subtraction term. Thus, a_i may be computed
// by adding b_l b_(l-1) ... b_(i+1) b_i and an extra copy of b_(i-1). In C
// notation, this is:
//
//   a_i = (k>>(i+w)) << w + ((k>>(i+w-1)) & 1) << w
//
// Observe that, while t_i may be positive or negative, a_i is bounded by
// 0 <= a_i < n + 2^w. Additionally, a_i can only be zero if b_(i+w-1) and up
// are all zero. (Note this implies a non-trivial P + (-P) is unreachable for
// all groups. That would imply the subsequent a_i is zero, which means all
// terms thus far were zero.)
//
// Returning to our doubling position, we have a_j = t_j (mod n). We now
// determine the value of a_j - t_j, which must be divisible by n. Our bounds on
// a_j and t_j imply a_j - t_j is 0 or n. If it is 0, a_j = t_j. However, 2^w
// divides a_j and -2^(w-1) <= t_j <= 2^(w-1), so this can only happen if
// a_j = t_j = 0, which is a trivial doubling. Therefore, a_j - t_j = n.
//
// Now we determine j. Suppose j > 0. w divides j, so j >= w. Then,
//
//   n = a_j - t_j = (k>>(j+w)) << w + ((k>>(j+w-1)) & 1) << w - t_j
//                <= k/2^j + 2^w - t_j
//                 < n/2^w + 2^w + 2^(w-1)
//
// n is much larger than 2^w, so this is impossible. Thus, j = 0: only the final
// addition may hit the doubling case.
//
// Finally, we consider bit patterns for n and k. Divide k into k_H + k_M + k_L
// such that k_H is the contribution from b_(l-1) .. b_w, k_M is the
// contribution from b_(w-1), and k_L is the contribution from b_(w-2) ... b_0.
// That is:
//
// - 2^w divides k_H
// - k_M is 0 or 2^(w-1)
// - 0 <= k_L < 2^(w-1)
//
// Divide n into n_H + n_M + n_L similarly. We thus have:
//
//   t_0 = (k>>0) & ((1<<(w-1)) - 1) - (k>>0) & (1<<(w-1)) + (k>>(0-1)) & 1
//       = k & ((1<<(w-1)) - 1) - k & (1<<(w-1))
//       = k_L - k_M
//
//   a_0 = (k>>(0+w)) << w + ((k>>(0+w-1)) & 1) << w
//       = (k>>w) << w + ((k>>(w-1)) & 1) << w
//       = k_H + 2*k_M
//
//                 n = a_0 - t_0
//   n_H + n_M + n_L = (k_H + 2*k_M) - (k_L - k_M)
//                   = k_H + 3*k_M - k_L
//
// k_H - k_L < k and k < n, so k_H - k_L ≠ n. Therefore k_M is not 0 and must be
// 2^(w-1). Now we consider k_H and n_H. We know k_H <= n_H. Suppose k_H = n_H.
// Then,
//
//   n_M + n_L = 3*(2^(w-1)) - k_L
//             > 3*(2^(w-1)) - 2^(w-1)
//             = 2^w
//
// Contradiction (n_M + n_L is the bottom w bits of n). Thus k_H < n_H. Suppose
// k_H < n_H - 2*2^w. Then,
//
//   n_H + n_M + n_L = k_H + 3*(2^(w-1)) - k_L
//                   < n_H - 2*2^w + 3*(2^(w-1)) - k_L
//         n_M + n_L < -2^(w-1) - k_L
//
// Contradiction. Thus, k_H = n_H - 2^w. (Note 2^w divides n_H and k_H.) Thus,
//
//   n_H + n_M + n_L = k_H + 3*(2^(w-1)) - k_L
//                   = n_H - 2^w + 3*(2^(w-1)) - k_L
//         n_M + n_L = 2^(w-1) - k_L
//                  <= 2^(w-1)
//
// Equality would mean 2^(w-1) divides n, which is impossible if n is prime.
// Thus n_M + n_L < 2^(w-1), so n_M is zero, proving our condition.
//
// This proof constructs k, so, to show the converse, let k_H = n_H - 2^w,
// k_M = 2^(w-1), k_L = 2^(w-1) - n_L. This will result in a non-trivial point
// doubling in the final addition and is the only such scalar.
//
// COMMON CURVES:
//
// The group orders for common curves end in the following bit patterns:
//
//   P-521: ...00001001; w = 4 is okay
//   P-384: ...01110011; w = 2, 5, 6, 7 are okay
//   P-256: ...01010001; w = 5, 7 are okay
//   P-224: ...00111101; w = 3, 4, 5, 6 are okay
static inline void recode_scalar_bits(crypto_word_t *sign, crypto_word_t *digit,
                                      crypto_word_t in) {
  crypto_word_t s, d;

  s = ~((in >> 5) - 1); /* sets all bits to MSB(in), 'in' seen as
                          * 6-bit value */
  d = (1 << 6) - in - 1;
  d = (d & s) | (in & ~s);
  d = (d >> 1) + (d & 1);

  *sign = s & 1;
  *digit = d;
}
