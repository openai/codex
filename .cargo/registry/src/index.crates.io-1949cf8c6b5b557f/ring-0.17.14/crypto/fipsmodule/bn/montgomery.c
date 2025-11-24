// Copyright 1995-2016 The OpenSSL Project Authors. All Rights Reserved.
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

#include "internal.h"
#include "../../internal.h"

#include "../../limbs/limbs.h"
#include "../../limbs/limbs.inl"

OPENSSL_STATIC_ASSERT(BN_MONT_CTX_N0_LIMBS == 1 || BN_MONT_CTX_N0_LIMBS == 2,
  "BN_MONT_CTX_N0_LIMBS value is invalid");
OPENSSL_STATIC_ASSERT(
  sizeof(BN_ULONG) * BN_MONT_CTX_N0_LIMBS == sizeof(uint64_t),
  "uint64_t is insufficient precision for n0");

int bn_from_montgomery_in_place(BN_ULONG r[], size_t num_r, BN_ULONG a[],
                                    size_t num_a, const BN_ULONG n[],
                                    size_t num_n,
                                    const BN_ULONG n0_[BN_MONT_CTX_N0_LIMBS]) {
  if (num_n == 0 || num_r != num_n || num_a != 2 * num_n) {
    return 0;
  }

  // Add multiples of |n| to |r| until R = 2^(nl * BN_BITS2) divides it. On
  // input, we had |r| < |n| * R, so now |r| < 2 * |n| * R. Note that |r|
  // includes |carry| which is stored separately.
  BN_ULONG n0 = n0_[0];
  BN_ULONG carry = 0;
  for (size_t i = 0; i < num_n; i++) {
    BN_ULONG v = limbs_mul_add_limb(a + i, n, a[i] * n0, num_n);
    v += carry + a[i + num_n];
    carry |= (v != a[i + num_n]);
    carry &= (v <= a[i + num_n]);
    a[i + num_n] = v;
  }

  // Shift |num_n| words to divide by R. We have |a| < 2 * |n|. Note that |a|
  // includes |carry| which is stored separately.
  a += num_n;

  // |a| thus requires at most one additional subtraction |n| to be reduced.
  // Subtract |n| and select the answer in constant time.
  BN_ULONG v = limbs_sub(r, a, n, num_n) - carry;
  // |v| is one if |a| - |n| underflowed or zero if it did not. Note |v| cannot
  // be -1. That would imply the subtraction did not fit in |num_n| words, and
  // we know at most one subtraction is needed.
  v = 0u - v;
  for (size_t i = 0; i < num_n; i++) {
    r[i] = constant_time_select_w(v, a[i], r[i]);
    a[i] = 0;
  }
  return 1;
}
