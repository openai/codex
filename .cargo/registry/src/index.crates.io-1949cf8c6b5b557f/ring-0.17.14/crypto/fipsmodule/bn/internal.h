// Copyright 1995-2016 The OpenSSL Project Authors. All Rights Reserved.
// Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved.
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

#ifndef OPENSSL_HEADER_BN_INTERNAL_H
#define OPENSSL_HEADER_BN_INTERNAL_H

#include <ring-core/base.h>

#if defined(OPENSSL_X86_64) && defined(_MSC_VER) && !defined(__clang__)
#pragma warning(push, 3)
#include <intrin.h>
#pragma warning(pop)
#pragma intrinsic(_umul128)
#endif

#include "../../internal.h"

typedef crypto_word_t BN_ULONG;

#if defined(OPENSSL_64_BIT)

#if defined(BORINGSSL_HAS_UINT128)
// MSVC doesn't support two-word integers on 64-bit.
#define BN_ULLONG uint128_t
#endif

#define BN_BITS2 64
#define BN_MONT_CTX_N0_LIMBS 1
#define BN_MONT_CTX_N0(hi, lo) TOBN(hi, lo), 0
#define TOBN(hi, lo) ((BN_ULONG)(hi) << 32 | (lo))

#elif defined(OPENSSL_32_BIT)

#define BN_ULLONG uint64_t
#define BN_BITS2 32
// On some 32-bit platforms, Montgomery multiplication is done using 64-bit
// arithmetic with SIMD instructions. On such platforms, |BN_MONT_CTX::n0|
// needs to be two words long. Only certain 32-bit platforms actually make use
// of n0[1] and shorter R value would suffice for the others. However,
// currently only the assembly files know which is which.
#define BN_MONT_CTX_N0_LIMBS 2
#define BN_MONT_CTX_N0(hi, lo) TOBN(hi, lo)
#define TOBN(hi, lo) (lo), (hi)

#else
#error "Must define either OPENSSL_32_BIT or OPENSSL_64_BIT"
#endif



// BN_MONTGOMERY_MAX_WORDS is the maximum numer of words allowed in a |BIGNUM|
// used with Montgomery reduction. Ideally this limit would be applied to all
// |BIGNUM|s, in |bn_wexpand|, but the exactfloat library needs to create 8 MiB
// values for other operations.
// #define BN_MONTGOMERY_MAX_WORDS (8 * 1024 / sizeof(BN_ULONG))

// bn_mul_mont writes |ap| * |bp| mod |np| to |rp|, each |num| words
// long. Inputs and outputs are in Montgomery form. |n0| is a pointer to
// an |N0|.
//
// If at least one of |ap| or |bp| is fully reduced, |rp| will be fully reduced.
// If neither is fully-reduced, the output may not be either.
//
// This function allocates |num| words on the stack, so |num| should be at most
// |BN_MONTGOMERY_MAX_WORDS|.
//
// TODO(davidben): The x86_64 implementation expects a 32-bit input and masks
// off upper bits. The aarch64 implementation expects a 64-bit input and does
// not. |size_t| is the safer option but not strictly correct for x86_64. But
// the |BN_MONTGOMERY_MAX_WORDS| bound makes this moot.
//
// See also discussion in |ToWord| in abi_test.h for notes on smaller-than-word
// inputs.
//
// |num| must be at least 4, at least on x86.
//
// In other forks, |bn_mul_mont| returns an |int| indicating whether it
// actually did the multiplication. All our implementations always do the
// multiplication, and forcing callers to deal with the possibility of it
// failing just leads to further problems.
OPENSSL_STATIC_ASSERT(sizeof(int) == sizeof(size_t) ||
                      (sizeof(int) == 4 && sizeof(size_t) == 8),
                      "int and size_t ABI mismatch");
#if defined(OPENSSL_X86_64)
void bn_mul_mont_nohw(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                      const BN_ULONG *np, const BN_ULONG *n0, size_t num);
static inline void bn_mul_mont_small(
    BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
    const BN_ULONG *np, const BN_ULONG *n0, size_t num) {
    bn_mul_mont_nohw(rp, ap, bp, np, n0, num);
}
#elif defined(OPENSSL_AARCH64)
void bn_mul_mont_nohw(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                      const BN_ULONG *np, const BN_ULONG *n0, size_t num);
static inline void bn_mul_mont_small(
    BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
    const BN_ULONG *np, const BN_ULONG *n0, size_t num) {
    // No point in optimizing for P-256 because P-256 doesn't call into
    // this on AArch64.
    bn_mul_mont_nohw(rp, ap, bp, np, n0, num);
}
#elif defined(OPENSSL_ARM)
void bn_mul8x_mont_neon(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                        const BN_ULONG *np, const BN_ULONG *n0, size_t num);
void bn_mul_mont_nohw(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                      const BN_ULONG *np, const BN_ULONG *n0, size_t num);
static inline void bn_mul_mont_small(
    BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
    const BN_ULONG *np, const BN_ULONG *n0, size_t num) {
    // Approximate what `bn_mul_mont` did so that the NEON version for P-256
    // when practical.
    if (num == 8) {
        // XXX: This should not be accessing `neon_available` directly.
        if (neon_available) {
            bn_mul8x_mont_neon(rp, ap, bp, np, n0, num);
            return;
        }
    }
    bn_mul_mont_nohw(rp, ap, bp, np, n0, num);
}
#else
void bn_mul_mont(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                 const BN_ULONG *np, const BN_ULONG *n0, size_t num);
static inline void bn_mul_mont_small(
    BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
    const BN_ULONG *np, const BN_ULONG *n0, size_t num) {
    bn_mul_mont(rp, ap, bp, np, n0, num);
}
#endif

static inline void bn_umult_lohi(BN_ULONG *low_out, BN_ULONG *high_out,
                                 BN_ULONG a, BN_ULONG b) {
#if defined(OPENSSL_X86_64) && defined(_MSC_VER) && !defined(__clang__)
  *low_out = _umul128(a, b, high_out);
#else
  BN_ULLONG result = (BN_ULLONG)a * b;
  *low_out = (BN_ULONG)result;
  *high_out = (BN_ULONG)(result >> BN_BITS2);
#endif
}

#endif  // OPENSSL_HEADER_BN_INTERNAL_H
