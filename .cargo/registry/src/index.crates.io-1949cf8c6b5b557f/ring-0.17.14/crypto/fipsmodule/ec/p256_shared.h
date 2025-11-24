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

#ifndef OPENSSL_HEADER_EC_P256_SHARED_H
#define OPENSSL_HEADER_EC_P256_SHARED_H

#include "ring-core/base.h"

#include "../bn/internal.h"

#if !defined(OPENSSL_NO_ASM) && \
    (defined(OPENSSL_X86_64) || defined(OPENSSL_AARCH64)) && \
    !defined(OPENSSL_SMALL)
# define OPENSSL_USE_NISTZ256
#endif

// P-256 field operations.
//
// An element mod P in P-256 is represented as a little-endian array of
// |P256_LIMBS| |BN_ULONG|s, spanning the full range of values.
//
// The following functions take fully-reduced inputs mod P and give
// fully-reduced outputs. They may be used in-place.

#define P256_LIMBS (256 / BN_BITS2)

// A P256_POINT represents a P-256 point in Jacobian coordinates.
typedef struct {
  BN_ULONG X[P256_LIMBS];
  BN_ULONG Y[P256_LIMBS];
  BN_ULONG Z[P256_LIMBS];
} P256_POINT;

typedef unsigned char P256_SCALAR_BYTES[33];

static inline void p256_scalar_bytes_from_limbs(
    P256_SCALAR_BYTES bytes_out, const BN_ULONG limbs[P256_LIMBS]) {
  OPENSSL_memcpy(bytes_out, limbs, 32);
  bytes_out[32] = 0;
}

#endif /* !defined(OPENSSL_USE_NISTZ256) */
