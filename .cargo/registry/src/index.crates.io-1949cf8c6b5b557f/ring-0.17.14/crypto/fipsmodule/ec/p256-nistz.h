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

#ifndef OPENSSL_HEADER_EC_P256_X86_64_H
#define OPENSSL_HEADER_EC_P256_X86_64_H

#include <ring-core/base.h>

#include "p256_shared.h"

#include "../bn/internal.h"

#if defined(OPENSSL_USE_NISTZ256)

// ecp_nistz256_neg sets |res| to -|a| mod P.
void ecp_nistz256_neg(BN_ULONG res[P256_LIMBS], const BN_ULONG a[P256_LIMBS]);

// ecp_nistz256_mul_mont sets |res| to |a| * |b| * 2^-256 mod P.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_mul_mont_nohw(BN_ULONG res[P256_LIMBS],
                                const BN_ULONG a[P256_LIMBS],
                                const BN_ULONG b[P256_LIMBS]);
void ecp_nistz256_mul_mont_adx(BN_ULONG res[P256_LIMBS],
                               const BN_ULONG a[P256_LIMBS],
                               const BN_ULONG b[P256_LIMBS]);
#else
void ecp_nistz256_mul_mont(BN_ULONG res[P256_LIMBS],
                           const BN_ULONG a[P256_LIMBS],
                           const BN_ULONG b[P256_LIMBS]);
#endif

// ecp_nistz256_sqr_mont sets |res| to |a| * |a| * 2^-256 mod P.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_sqr_mont_nohw(BN_ULONG res[P256_LIMBS],
                                const BN_ULONG a[P256_LIMBS]);
void ecp_nistz256_sqr_mont_adx(BN_ULONG res[P256_LIMBS],
                               const BN_ULONG a[P256_LIMBS]);
#else
void ecp_nistz256_sqr_mont(BN_ULONG res[P256_LIMBS],
                           const BN_ULONG a[P256_LIMBS]);
#endif


// P-256 scalar operations.
//
// The following functions compute modulo N, where N is the order of P-256. They
// take fully-reduced inputs and give fully-reduced outputs.

// ecp_nistz256_ord_mul_mont sets |res| to |a| * |b| where inputs and outputs
// are in Montgomery form. That is, |res| is |a| * |b| * 2^-256 mod N.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_ord_mul_mont_nohw(BN_ULONG res[P256_LIMBS],
                                    const BN_ULONG a[P256_LIMBS],
                                    const BN_ULONG b[P256_LIMBS]);
void ecp_nistz256_ord_mul_mont_adx(BN_ULONG res[P256_LIMBS],
                                   const BN_ULONG a[P256_LIMBS],
                                   const BN_ULONG b[P256_LIMBS]);
#else
void ecp_nistz256_ord_mul_mont(BN_ULONG res[P256_LIMBS],
                               const BN_ULONG a[P256_LIMBS],
                               const BN_ULONG b[P256_LIMBS]);
#endif

// ecp_nistz256_ord_sqr_mont sets |res| to |a|^(2*|rep|) where inputs and
// outputs are in Montgomery form. That is, |res| is
// (|a| * 2^-256)^(2*|rep|) * 2^256 mod N.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_ord_sqr_mont_nohw(BN_ULONG res[P256_LIMBS],
                                    const BN_ULONG a[P256_LIMBS], BN_ULONG rep);
void ecp_nistz256_ord_sqr_mont_adx(BN_ULONG res[P256_LIMBS],
                                   const BN_ULONG a[P256_LIMBS], BN_ULONG rep);
#else
void ecp_nistz256_ord_sqr_mont(BN_ULONG res[P256_LIMBS],
                               const BN_ULONG a[P256_LIMBS], BN_ULONG rep);
#endif



// P-256 point operations.
//
// The following functions may be used in-place. All coordinates are in the
// Montgomery domain.

// A P256_POINT_AFFINE represents a P-256 point in affine coordinates. Infinity
// is encoded as (0, 0).
typedef struct {
  BN_ULONG X[P256_LIMBS];
  BN_ULONG Y[P256_LIMBS];
} P256_POINT_AFFINE;

// ecp_nistz256_select_w5 sets |*val| to |in_t[index-1]| if 1 <= |index| <= 16
// and all zeros (the point at infinity) if |index| is 0. This is done in
// constant time.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_select_w5_nohw(P256_POINT *val, const P256_POINT in_t[16],
                                 int index);
void ecp_nistz256_select_w5_avx2(P256_POINT *val, const P256_POINT in_t[16],
                                 int index);
#else
void ecp_nistz256_select_w5(P256_POINT *val, const P256_POINT in_t[16],
                            int index);
#endif

// ecp_nistz256_select_w7 sets |*val| to |in_t[index-1]| if 1 <= |index| <= 64
// and all zeros (the point at infinity) if |index| is 0. This is done in
// constant time.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_select_w7_nohw(P256_POINT_AFFINE *val,
                                 const P256_POINT_AFFINE in_t[64], int index);
void ecp_nistz256_select_w7_avx2(P256_POINT_AFFINE *val,
                                 const P256_POINT_AFFINE in_t[64], int index);
#else
void ecp_nistz256_select_w7(P256_POINT_AFFINE *val,
                            const P256_POINT_AFFINE in_t[64], int index);
#endif

// ecp_nistz256_point_double sets |r| to |a| doubled.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_point_double_nohw(P256_POINT *r, const P256_POINT *a);
void ecp_nistz256_point_double_adx(P256_POINT *r, const P256_POINT *a);
#else
void ecp_nistz256_point_double(P256_POINT *r, const P256_POINT *a);
#endif

// ecp_nistz256_point_add adds |a| to |b| and places the result in |r|.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_point_add_nohw(P256_POINT *r, const P256_POINT *a,
                                 const P256_POINT *b);
void ecp_nistz256_point_add_adx(P256_POINT *r, const P256_POINT *a,
                                const P256_POINT *b);
#else
void ecp_nistz256_point_add(P256_POINT *r, const P256_POINT *a,
                            const P256_POINT *b);
#endif

// ecp_nistz256_point_add_affine adds |a| to |b| and places the result in
// |r|. |a| and |b| must not represent the same point unless they are both
// infinity.
#if defined(OPENSSL_X86_64)
void ecp_nistz256_point_add_affine_adx(P256_POINT *r, const P256_POINT *a,
                                       const P256_POINT_AFFINE *b);
void ecp_nistz256_point_add_affine_nohw(P256_POINT *r, const P256_POINT *a,
                                        const P256_POINT_AFFINE *b);
#else
void ecp_nistz256_point_add_affine(P256_POINT *r, const P256_POINT *a,
                                   const P256_POINT_AFFINE *b);
#endif

#endif /* defined(OPENSSL_USE_NISTZ256) */

#endif  // OPENSSL_HEADER_EC_P256_X86_64_H
