// Copyright 2020 The BoringSSL Authors
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

#ifndef OPENSSL_HEADER_CURVE25519_INTERNAL_H
#define OPENSSL_HEADER_CURVE25519_INTERNAL_H

#include <ring-core/base.h>

#include "../internal.h"


#if defined(OPENSSL_ARM) && !defined(OPENSSL_NO_ASM) && !defined(OPENSSL_APPLE)
#define BORINGSSL_X25519_NEON

// x25519_NEON is defined in asm/x25519-arm.S.
void x25519_NEON(uint8_t out[32], const uint8_t scalar[32],
                 const uint8_t point[32]);
#endif

#if !defined(OPENSSL_NO_ASM) && !defined(OPENSSL_SMALL) && \
    defined(__GNUC__) && defined(__x86_64__) && !defined(OPENSSL_WINDOWS)
#define BORINGSSL_FE25519_ADX

// fiat_curve25519_adx_mul is defined in
// third_party/fiat/asm/fiat_curve25519_adx_mul.S
void __attribute__((sysv_abi))
fiat_curve25519_adx_mul(uint64_t out[4], const uint64_t in1[4],
                        const uint64_t in2[4]);

// fiat_curve25519_adx_square is defined in
// third_party/fiat/asm/fiat_curve25519_adx_square.S
void __attribute__((sysv_abi))
fiat_curve25519_adx_square(uint64_t out[4], const uint64_t in[4]);

// x25519_scalar_mult_adx is defined in third_party/fiat/curve25519_64_adx.h
void x25519_scalar_mult_adx(uint8_t out[32], const uint8_t scalar[32],
                            const uint8_t point[32]);
void x25519_ge_scalarmult_base_adx(uint8_t h[4][32], const uint8_t a[32]);
#endif

#if defined(OPENSSL_64_BIT)
// An element t,
// entries t[0]...t[4], represents the integer t[0]+2^51 t[1]+2^102 t[2]+2^153
// t[3]+2^204 t[4].
// fe limbs are bounded by 1.125*2^51.
// fe_loose limbs are bounded by 3.375*2^51.
typedef uint64_t fe_limb_t;
#define FE_NUM_LIMBS 5
#else
// An element t,
// entries t[0]...t[9], represents the integer t[0]+2^26 t[1]+2^51 t[2]+2^77
// t[3]+2^102 t[4]+...+2^230 t[9].
// fe limbs are bounded by 1.125*2^26,1.125*2^25,1.125*2^26,1.125*2^25,etc.
// fe_loose limbs are bounded by 3.375*2^26,3.375*2^25,3.375*2^26,3.375*2^25,etc.
typedef uint32_t fe_limb_t;
#define FE_NUM_LIMBS 10
#endif

// fe means field element. Here the field is \Z/(2^255-19).
// Multiplication and carrying produce fe from fe_loose.
// Keep in sync with `Elem` and `ELEM_LIMBS` in curve25519/ops.rs.
typedef struct fe { fe_limb_t v[FE_NUM_LIMBS]; } fe;

// Addition and subtraction produce fe_loose from (fe, fe).
// Keep in sync with `Elem` and `ELEM_LIMBS` in curve25519/ops.rs.
typedef struct fe_loose { fe_limb_t v[FE_NUM_LIMBS]; } fe_loose;

static inline void fe_limbs_copy(fe_limb_t r[], const fe_limb_t a[]) {
  for (size_t i = 0; i < FE_NUM_LIMBS; ++i) {
    r[i] = a[i];
  }
}

// ge means group element.
//
// Here the group is the set of pairs (x,y) of field elements (see fe.h)
// satisfying -x^2 + y^2 = 1 + d x^2y^2
// where d = -121665/121666.
//
// Representations:
//   ge_p2 (projective): (X:Y:Z) satisfying x=X/Z, y=Y/Z
//   ge_p3 (extended): (X:Y:Z:T) satisfying x=X/Z, y=Y/Z, XY=ZT
//   ge_p1p1 (completed): ((X:Z),(Y:T)) satisfying x=X/Z, y=Y/T
//   ge_precomp (Duif): (y+x,y-x,2dxy)

// Keep in sync with `Point` in curve25519/ops.rs.
typedef struct {
  fe X;
  fe Y;
  fe Z;
} ge_p2;


// Keep in sync with `ExtPoint` in curve25519/ops.rs.
typedef struct {
  fe X;
  fe Y;
  fe Z;
  fe T;
} ge_p3;

typedef struct {
  fe_loose X;
  fe_loose Y;
  fe_loose Z;
  fe_loose T;
} ge_p1p1;

typedef struct {
  fe_loose yplusx;
  fe_loose yminusx;
  fe_loose xy2d;
} ge_precomp;

typedef struct {
  fe_loose YplusX;
  fe_loose YminusX;
  fe_loose Z;
  fe_loose T2d;
} ge_cached;

extern const uint8_t k25519Precomp[32][8][3][32];

#endif  // OPENSSL_HEADER_CURVE25519_INTERNAL_H
