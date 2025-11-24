/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Copyright 2023 IBM Corp.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 *
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 *  Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 */

/*
 * Designed for 56-bit limbs by Rohan McLure <rohan.mclure@linux.ibm.com>.
 * The layout is based on that of ecp_nistp{224,521}.c, allowing even for asm
 * acceleration of felem_{square,mul} as supported in these files.
 */

#include <openssl/e_os2.h>

#include <string.h>
#include <openssl/err.h>
#include "ec_local.h"

#include "internal/numbers.h"

#ifndef INT128_MAX
# error "Your compiler doesn't appear to support 128-bit integer types"
#endif

typedef uint8_t u8;
typedef uint64_t u64;

/*
 * The underlying field. P384 operates over GF(2^384-2^128-2^96+2^32-1). We
 * can serialize an element of this field into 48 bytes. We call this an
 * felem_bytearray.
 */

typedef u8 felem_bytearray[48];

/*
 * These are the parameters of P384, taken from FIPS 186-3, section D.1.2.4.
 * These values are big-endian.
 */
static const felem_bytearray nistp384_curve_params[5] = {
  {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, /* p */
   0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
   0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF,
   0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF},
  {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, /* a = -3 */
   0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
   0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF,
   0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFC},
  {0xB3, 0x31, 0x2F, 0xA7, 0xE2, 0x3E, 0xE7, 0xE4, 0x98, 0x8E, 0x05, 0x6B, /* b */
   0xE3, 0xF8, 0x2D, 0x19, 0x18, 0x1D, 0x9C, 0x6E, 0xFE, 0x81, 0x41, 0x12,
   0x03, 0x14, 0x08, 0x8F, 0x50, 0x13, 0x87, 0x5A, 0xC6, 0x56, 0x39, 0x8D,
   0x8A, 0x2E, 0xD1, 0x9D, 0x2A, 0x85, 0xC8, 0xED, 0xD3, 0xEC, 0x2A, 0xEF},
  {0xAA, 0x87, 0xCA, 0x22, 0xBE, 0x8B, 0x05, 0x37, 0x8E, 0xB1, 0xC7, 0x1E, /* x */
   0xF3, 0x20, 0xAD, 0x74, 0x6E, 0x1D, 0x3B, 0x62, 0x8B, 0xA7, 0x9B, 0x98,
   0x59, 0xF7, 0x41, 0xE0, 0x82, 0x54, 0x2A, 0x38, 0x55, 0x02, 0xF2, 0x5D,
   0xBF, 0x55, 0x29, 0x6C, 0x3A, 0x54, 0x5E, 0x38, 0x72, 0x76, 0x0A, 0xB7},
  {0x36, 0x17, 0xDE, 0x4A, 0x96, 0x26, 0x2C, 0x6F, 0x5D, 0x9E, 0x98, 0xBF, /* y */
   0x92, 0x92, 0xDC, 0x29, 0xF8, 0xF4, 0x1D, 0xBD, 0x28, 0x9A, 0x14, 0x7C,
   0xE9, 0xDA, 0x31, 0x13, 0xB5, 0xF0, 0xB8, 0xC0, 0x0A, 0x60, 0xB1, 0xCE,
   0x1D, 0x7E, 0x81, 0x9D, 0x7A, 0x43, 0x1D, 0x7C, 0x90, 0xEA, 0x0E, 0x5F},
};

/*-
 * The representation of field elements.
 * ------------------------------------
 *
 * We represent field elements with seven values. These values are either 64 or
 * 128 bits and the field element represented is:
 *   v[0]*2^0 + v[1]*2^56 + v[2]*2^112 + ... + v[6]*2^336  (mod p)
 * Each of the seven values is called a 'limb'. Since the limbs are spaced only
 * 56 bits apart, but are greater than 56 bits in length, the most significant
 * bits of each limb overlap with the least significant bits of the next
 *
 * This representation is considered to be 'redundant' in the sense that
 * intermediate values can each contain more than a 56-bit value in each limb.
 * Reduction causes all but the final limb to be reduced to contain a value less
 * than 2^56, with the final value represented allowed to be larger than 2^384,
 * inasmuch as we can be sure that arithmetic overflow remains impossible. The
 * reduced value must of course be congruent to the unreduced value.
 *
 * A field element with 64-bit limbs is an 'felem'. One with 128-bit limbs is a
 * 'widefelem', featuring enough bits to store the result of a multiplication
 * and even some further arithmetic without need for immediate reduction.
 */

#define NLIMBS 7

typedef uint64_t limb;
typedef uint128_t widelimb;
typedef limb limb_aX __attribute((__aligned__(1)));
typedef limb felem[NLIMBS];
typedef widelimb widefelem[2*NLIMBS-1];

static const limb bottom56bits = 0xffffffffffffff;

/* Helper functions (de)serialising reduced field elements in little endian */
static void bin48_to_felem(felem out, const u8 in[48])
{
    memset(out, 0, 56);
    out[0] = (*((limb *) & in[0])) & bottom56bits;
    out[1] = (*((limb_aX *) & in[7])) & bottom56bits;
    out[2] = (*((limb_aX *) & in[14])) & bottom56bits;
    out[3] = (*((limb_aX *) & in[21])) & bottom56bits;
    out[4] = (*((limb_aX *) & in[28])) & bottom56bits;
    out[5] = (*((limb_aX *) & in[35])) & bottom56bits;
    memmove(&out[6], &in[42], 6);
}

static void felem_to_bin48(u8 out[48], const felem in)
{
    memset(out, 0, 48);
    (*((limb *) & out[0]))     |= (in[0] & bottom56bits);
    (*((limb_aX *) & out[7]))  |= (in[1] & bottom56bits);
    (*((limb_aX *) & out[14])) |= (in[2] & bottom56bits);
    (*((limb_aX *) & out[21])) |= (in[3] & bottom56bits);
    (*((limb_aX *) & out[28])) |= (in[4] & bottom56bits);
    (*((limb_aX *) & out[35])) |= (in[5] & bottom56bits);
    memmove(&out[42], &in[6], 6);
}

/* BN_to_felem converts an OpenSSL BIGNUM into an felem */
static int BN_to_felem(felem out, const BIGNUM *bn)
{
    felem_bytearray b_out;
    int num_bytes;

    if (BN_is_negative(bn)) {
        ERR_raise(ERR_LIB_EC, EC_R_BIGNUM_OUT_OF_RANGE);
        return 0;
    }
    num_bytes = BN_bn2lebinpad(bn, b_out, sizeof(b_out));
    if (num_bytes < 0) {
        ERR_raise(ERR_LIB_EC, EC_R_BIGNUM_OUT_OF_RANGE);
        return 0;
    }
    bin48_to_felem(out, b_out);
    return 1;
}

/* felem_to_BN converts an felem into an OpenSSL BIGNUM */
static BIGNUM *felem_to_BN(BIGNUM *out, const felem in)
{
    felem_bytearray b_out;

    felem_to_bin48(b_out, in);
    return BN_lebin2bn(b_out, sizeof(b_out), out);
}

/*-
 * Field operations
 * ----------------
 */

static void felem_one(felem out)
{
    out[0] = 1;
    memset(&out[1], 0, sizeof(limb) * (NLIMBS-1));
}

static void felem_assign(felem out, const felem in)
{
    memcpy(out, in, sizeof(felem));
}

/* felem_sum64 sets out = out + in. */
static void felem_sum64(felem out, const felem in)
{
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out[i] += in[i];
}

/* felem_scalar sets out = in * scalar */
static void felem_scalar(felem out, const felem in, limb scalar)
{
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out[i] = in[i] * scalar;
}

/* felem_scalar64 sets out = out * scalar */
static void felem_scalar64(felem out, limb scalar)
{
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out[i] *= scalar;
}

/* felem_scalar128 sets out = out * scalar */
static void felem_scalar128(widefelem out, limb scalar)
{
    unsigned int i;

    for (i = 0; i < 2*NLIMBS-1; i++)
        out[i] *= scalar;
}

/*-
 * felem_neg sets |out| to |-in|
 * On entry:
 *   in[i] < 2^60 - 2^29
 * On exit:
 *   out[i] < 2^60
 */
static void felem_neg(felem out, const felem in)
{
    /*
     * In order to prevent underflow, we add a multiple of p before subtracting.
     * Use telescopic sums to represent 2^12 * p redundantly with each limb
     * of the form 2^60 + ...
     */
    static const limb two60m52m4 = (((limb) 1) << 60)
                                 - (((limb) 1) << 52)
                                 - (((limb) 1) << 4);
    static const limb two60p44m12 = (((limb) 1) << 60)
                                  + (((limb) 1) << 44)
                                  - (((limb) 1) << 12);
    static const limb two60m28m4 = (((limb) 1) << 60)
                                 - (((limb) 1) << 28)
                                 - (((limb) 1) << 4);
    static const limb two60m4 = (((limb) 1) << 60)
                              - (((limb) 1) << 4);

    out[0] = two60p44m12 - in[0];
    out[1] = two60m52m4 - in[1];
    out[2] = two60m28m4 - in[2];
    out[3] = two60m4 - in[3];
    out[4] = two60m4 - in[4];
    out[5] = two60m4 - in[5];
    out[6] = two60m4 - in[6];
}

#if defined(ECP_NISTP384_ASM)
void p384_felem_diff64(felem out, const felem in);
void p384_felem_diff128(widefelem out, const widefelem in);
void p384_felem_diff_128_64(widefelem out, const felem in);

# define felem_diff64           p384_felem_diff64
# define felem_diff128          p384_felem_diff128
# define felem_diff_128_64      p384_felem_diff_128_64

#else
/*-
 * felem_diff64 subtracts |in| from |out|
 * On entry:
 *   in[i] < 2^60 - 2^52 - 2^4
 * On exit:
 *   out[i] < out_orig[i] + 2^60 + 2^44
 */
static void felem_diff64(felem out, const felem in)
{
    /*
     * In order to prevent underflow, we add a multiple of p before subtracting.
     * Use telescopic sums to represent 2^12 * p redundantly with each limb
     * of the form 2^60 + ...
     */

    static const limb two60m52m4 = (((limb) 1) << 60)
                                 - (((limb) 1) << 52)
                                 - (((limb) 1) << 4);
    static const limb two60p44m12 = (((limb) 1) << 60)
                                  + (((limb) 1) << 44)
                                  - (((limb) 1) << 12);
    static const limb two60m28m4 = (((limb) 1) << 60)
                                 - (((limb) 1) << 28)
                                 - (((limb) 1) << 4);
    static const limb two60m4 = (((limb) 1) << 60)
                              - (((limb) 1) << 4);

    out[0] += two60p44m12 - in[0];
    out[1] += two60m52m4 - in[1];
    out[2] += two60m28m4 - in[2];
    out[3] += two60m4 - in[3];
    out[4] += two60m4 - in[4];
    out[5] += two60m4 - in[5];
    out[6] += two60m4 - in[6];
}

/*
 * in[i] < 2^63
 * out[i] < out_orig[i] + 2^64 + 2^48
 */
static void felem_diff_128_64(widefelem out, const felem in)
{
    /*
     * In order to prevent underflow, we add a multiple of p before subtracting.
     * Use telescopic sums to represent 2^16 * p redundantly with each limb
     * of the form 2^64 + ...
     */

    static const widelimb two64m56m8 = (((widelimb) 1) << 64)
                                     - (((widelimb) 1) << 56)
                                     - (((widelimb) 1) << 8);
    static const widelimb two64m32m8 = (((widelimb) 1) << 64)
                                     - (((widelimb) 1) << 32)
                                     - (((widelimb) 1) << 8);
    static const widelimb two64m8 = (((widelimb) 1) << 64)
                                  - (((widelimb) 1) << 8);
    static const widelimb two64p48m16 = (((widelimb) 1) << 64)
                                      + (((widelimb) 1) << 48)
                                      - (((widelimb) 1) << 16);
    unsigned int i;

    out[0] += two64p48m16;
    out[1] += two64m56m8;
    out[2] += two64m32m8;
    out[3] += two64m8;
    out[4] += two64m8;
    out[5] += two64m8;
    out[6] += two64m8;

    for (i = 0; i < NLIMBS; i++)
        out[i] -= in[i];
}

/*
 * in[i] < 2^127 - 2^119 - 2^71
 * out[i] < out_orig[i] + 2^127 + 2^111
 */
static void felem_diff128(widefelem out, const widefelem in)
{
    /*
     * In order to prevent underflow, we add a multiple of p before subtracting.
     * Use telescopic sums to represent 2^415 * p redundantly with each limb
     * of the form 2^127 + ...
     */

    static const widelimb two127 = ((widelimb) 1) << 127;
    static const widelimb two127m71 = (((widelimb) 1) << 127)
                                    - (((widelimb) 1) << 71);
    static const widelimb two127p111m79m71 = (((widelimb) 1) << 127)
                                           + (((widelimb) 1) << 111)
                                           - (((widelimb) 1) << 79)
                                           - (((widelimb) 1) << 71);
    static const widelimb two127m119m71 = (((widelimb) 1) << 127)
                                        - (((widelimb) 1) << 119)
                                        - (((widelimb) 1) << 71);
    static const widelimb two127m95m71 = (((widelimb) 1) << 127)
                                       - (((widelimb) 1) << 95)
                                       - (((widelimb) 1) << 71);
    unsigned int i;

    out[0]  += two127;
    out[1]  += two127m71;
    out[2]  += two127m71;
    out[3]  += two127m71;
    out[4]  += two127m71;
    out[5]  += two127m71;
    out[6]  += two127p111m79m71;
    out[7]  += two127m119m71;
    out[8]  += two127m95m71;
    out[9]  += two127m71;
    out[10] += two127m71;
    out[11] += two127m71;
    out[12] += two127m71;

    for (i = 0; i < 2*NLIMBS-1; i++)
        out[i] -= in[i];
}
#endif /* ECP_NISTP384_ASM */

static void felem_square_ref(widefelem out, const felem in)
{
    felem inx2;
    felem_scalar(inx2, in, 2);

    out[0] = ((uint128_t) in[0]) * in[0];

    out[1] = ((uint128_t) in[0]) * inx2[1];

    out[2] = ((uint128_t) in[0]) * inx2[2]
           + ((uint128_t) in[1]) * in[1];

    out[3] = ((uint128_t) in[0]) * inx2[3]
           + ((uint128_t) in[1]) * inx2[2];

    out[4] = ((uint128_t) in[0]) * inx2[4]
           + ((uint128_t) in[1]) * inx2[3]
           + ((uint128_t) in[2]) * in[2];

    out[5] = ((uint128_t) in[0]) * inx2[5]
           + ((uint128_t) in[1]) * inx2[4]
           + ((uint128_t) in[2]) * inx2[3];

    out[6] = ((uint128_t) in[0]) * inx2[6]
           + ((uint128_t) in[1]) * inx2[5]
           + ((uint128_t) in[2]) * inx2[4]
           + ((uint128_t) in[3]) * in[3];

    out[7] = ((uint128_t) in[1]) * inx2[6]
           + ((uint128_t) in[2]) * inx2[5]
           + ((uint128_t) in[3]) * inx2[4];

    out[8] = ((uint128_t) in[2]) * inx2[6]
           + ((uint128_t) in[3]) * inx2[5]
           + ((uint128_t) in[4]) * in[4];

    out[9] = ((uint128_t) in[3]) * inx2[6]
           + ((uint128_t) in[4]) * inx2[5];

    out[10] = ((uint128_t) in[4]) * inx2[6]
            + ((uint128_t) in[5]) * in[5];

    out[11] = ((uint128_t) in[5]) * inx2[6];

    out[12] = ((uint128_t) in[6]) * in[6];
}

static void felem_mul_ref(widefelem out, const felem in1, const felem in2)
{
    out[0] = ((uint128_t) in1[0]) * in2[0];

    out[1] = ((uint128_t) in1[0]) * in2[1]
           + ((uint128_t) in1[1]) * in2[0];

    out[2] = ((uint128_t) in1[0]) * in2[2]
           + ((uint128_t) in1[1]) * in2[1]
           + ((uint128_t) in1[2]) * in2[0];

    out[3] = ((uint128_t) in1[0]) * in2[3]
           + ((uint128_t) in1[1]) * in2[2]
           + ((uint128_t) in1[2]) * in2[1]
           + ((uint128_t) in1[3]) * in2[0];

    out[4] = ((uint128_t) in1[0]) * in2[4]
           + ((uint128_t) in1[1]) * in2[3]
           + ((uint128_t) in1[2]) * in2[2]
           + ((uint128_t) in1[3]) * in2[1]
           + ((uint128_t) in1[4]) * in2[0];

    out[5] = ((uint128_t) in1[0]) * in2[5]
           + ((uint128_t) in1[1]) * in2[4]
           + ((uint128_t) in1[2]) * in2[3]
           + ((uint128_t) in1[3]) * in2[2]
           + ((uint128_t) in1[4]) * in2[1]
           + ((uint128_t) in1[5]) * in2[0];

    out[6] = ((uint128_t) in1[0]) * in2[6]
           + ((uint128_t) in1[1]) * in2[5]
           + ((uint128_t) in1[2]) * in2[4]
           + ((uint128_t) in1[3]) * in2[3]
           + ((uint128_t) in1[4]) * in2[2]
           + ((uint128_t) in1[5]) * in2[1]
           + ((uint128_t) in1[6]) * in2[0];

    out[7] = ((uint128_t) in1[1]) * in2[6]
           + ((uint128_t) in1[2]) * in2[5]
           + ((uint128_t) in1[3]) * in2[4]
           + ((uint128_t) in1[4]) * in2[3]
           + ((uint128_t) in1[5]) * in2[2]
           + ((uint128_t) in1[6]) * in2[1];

    out[8] = ((uint128_t) in1[2]) * in2[6]
           + ((uint128_t) in1[3]) * in2[5]
           + ((uint128_t) in1[4]) * in2[4]
           + ((uint128_t) in1[5]) * in2[3]
           + ((uint128_t) in1[6]) * in2[2];

    out[9] = ((uint128_t) in1[3]) * in2[6]
           + ((uint128_t) in1[4]) * in2[5]
           + ((uint128_t) in1[5]) * in2[4]
           + ((uint128_t) in1[6]) * in2[3];

    out[10] = ((uint128_t) in1[4]) * in2[6]
            + ((uint128_t) in1[5]) * in2[5]
            + ((uint128_t) in1[6]) * in2[4];

    out[11] = ((uint128_t) in1[5]) * in2[6]
            + ((uint128_t) in1[6]) * in2[5];

    out[12] = ((uint128_t) in1[6]) * in2[6];
}

/*-
 * Reduce thirteen 128-bit coefficients to seven 64-bit coefficients.
 * in[i] < 2^128 - 2^125
 * out[i] < 2^56 for i < 6,
 * out[6] <= 2^48
 *
 * The technique in use here stems from the format of the prime modulus:
 * P384 = 2^384 - delta
 *
 * Thus we can reduce numbers of the form (X + 2^384 * Y) by substituting
 * them with (X + delta Y), with delta = 2^128 + 2^96 + (-2^32 + 1). These
 * coefficients are still quite large, and so we repeatedly apply this
 * technique on high-order bits in order to guarantee the desired bounds on
 * the size of our output.
 *
 * The three phases of elimination are as follows:
 * [1]: Y = 2^120 (in[12] | in[11] | in[10] | in[9])
 * [2]: Y = 2^8 (acc[8] | acc[7])
 * [3]: Y = 2^48 (acc[6] >> 48)
 * (Where a | b | c | d = (2^56)^3 a + (2^56)^2 b + (2^56) c + d)
 */
static void felem_reduce_ref(felem out, const widefelem in)
{
    /*
     * In order to prevent underflow, we add a multiple of p before subtracting.
     * Use telescopic sums to represent 2^76 * p redundantly with each limb
     * of the form 2^124 + ...
     */
    static const widelimb two124m68 = (((widelimb) 1) << 124)
                                    - (((widelimb) 1) << 68);
    static const widelimb two124m116m68 = (((widelimb) 1) << 124)
                                        - (((widelimb) 1) << 116)
                                        - (((widelimb) 1) << 68);
    static const widelimb two124p108m76 = (((widelimb) 1) << 124)
                                        + (((widelimb) 1) << 108)
                                        - (((widelimb) 1) << 76);
    static const widelimb two124m92m68 = (((widelimb) 1) << 124)
                                       - (((widelimb) 1) << 92)
                                       - (((widelimb) 1) << 68);
    widelimb temp, acc[9];
    unsigned int i;

    memcpy(acc, in, sizeof(widelimb) * 9);

    acc[0] += two124p108m76;
    acc[1] += two124m116m68;
    acc[2] += two124m92m68;
    acc[3] += two124m68;
    acc[4] += two124m68;
    acc[5] += two124m68;
    acc[6] += two124m68;

    /* [1]: Eliminate in[9], ..., in[12] */
    acc[8] += in[12] >> 32;
    acc[7] += (in[12] & 0xffffffff) << 24;
    acc[7] += in[12] >> 8;
    acc[6] += (in[12] & 0xff) << 48;
    acc[6] -= in[12] >> 16;
    acc[5] -= (in[12] & 0xffff) << 40;
    acc[6] += in[12] >> 48;
    acc[5] += (in[12] & 0xffffffffffff) << 8;

    acc[7] += in[11] >> 32;
    acc[6] += (in[11] & 0xffffffff) << 24;
    acc[6] += in[11] >> 8;
    acc[5] += (in[11] & 0xff) << 48;
    acc[5] -= in[11] >> 16;
    acc[4] -= (in[11] & 0xffff) << 40;
    acc[5] += in[11] >> 48;
    acc[4] += (in[11] & 0xffffffffffff) << 8;

    acc[6] += in[10] >> 32;
    acc[5] += (in[10] & 0xffffffff) << 24;
    acc[5] += in[10] >> 8;
    acc[4] += (in[10] & 0xff) << 48;
    acc[4] -= in[10] >> 16;
    acc[3] -= (in[10] & 0xffff) << 40;
    acc[4] += in[10] >> 48;
    acc[3] += (in[10] & 0xffffffffffff) << 8;

    acc[5] += in[9] >> 32;
    acc[4] += (in[9] & 0xffffffff) << 24;
    acc[4] += in[9] >> 8;
    acc[3] += (in[9] & 0xff) << 48;
    acc[3] -= in[9] >> 16;
    acc[2] -= (in[9] & 0xffff) << 40;
    acc[3] += in[9] >> 48;
    acc[2] += (in[9] & 0xffffffffffff) << 8;

    /*
     * [2]: Eliminate acc[7], acc[8], that is the 7 and eighth limbs, as
     * well as the contributions made from eliminating higher limbs.
     * acc[7] < in[7] + 2^120 + 2^56 < in[7] + 2^121
     * acc[8] < in[8] + 2^96
     */
    acc[4] += acc[8] >> 32;
    acc[3] += (acc[8] & 0xffffffff) << 24;
    acc[3] += acc[8] >> 8;
    acc[2] += (acc[8] & 0xff) << 48;
    acc[2] -= acc[8] >> 16;
    acc[1] -= (acc[8] & 0xffff) << 40;
    acc[2] += acc[8] >> 48;
    acc[1] += (acc[8] & 0xffffffffffff) << 8;

    acc[3] += acc[7] >> 32;
    acc[2] += (acc[7] & 0xffffffff) << 24;
    acc[2] += acc[7] >> 8;
    acc[1] += (acc[7] & 0xff) << 48;
    acc[1] -= acc[7] >> 16;
    acc[0] -= (acc[7] & 0xffff) << 40;
    acc[1] += acc[7] >> 48;
    acc[0] += (acc[7] & 0xffffffffffff) << 8;

    /*-
     * acc[k] < in[k] + 2^124 + 2^121
     *        < in[k] + 2^125
     *        < 2^128, for k <= 6
     */

    /*
     * Carry 4 -> 5 -> 6
     * This has the effect of ensuring that these more significant limbs
     * will be small in value after eliminating high bits from acc[6].
     */
    acc[5] += acc[4] >> 56;
    acc[4] &= 0x00ffffffffffffff;

    acc[6] += acc[5] >> 56;
    acc[5] &= 0x00ffffffffffffff;

    /*-
     * acc[6] < in[6] + 2^124 + 2^121 + 2^72 + 2^16
     *        < in[6] + 2^125
     *        < 2^128
     */

    /* [3]: Eliminate high bits of acc[6] */
    temp = acc[6] >> 48;
    acc[6] &= 0x0000ffffffffffff;

    /* temp < 2^80 */

    acc[3] += temp >> 40;
    acc[2] += (temp & 0xffffffffff) << 16;
    acc[2] += temp >> 16;
    acc[1] += (temp & 0xffff) << 40;
    acc[1] -= temp >> 24;
    acc[0] -= (temp & 0xffffff) << 32;
    acc[0] += temp;

    /*-
     * acc[k] < acc_old[k] + 2^64 + 2^56
     *        < in[k] + 2^124 + 2^121 + 2^72 + 2^64 + 2^56 + 2^16 , k < 4
     */

    /* Carry 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6 */
    acc[1] += acc[0] >> 56;   /* acc[1] < acc_old[1] + 2^72 */
    acc[0] &= 0x00ffffffffffffff;

    acc[2] += acc[1] >> 56;   /* acc[2] < acc_old[2] + 2^72 + 2^16 */
    acc[1] &= 0x00ffffffffffffff;

    acc[3] += acc[2] >> 56;   /* acc[3] < acc_old[3] + 2^72 + 2^16 */
    acc[2] &= 0x00ffffffffffffff;

    /*-
     * acc[k] < acc_old[k] + 2^72 + 2^16
     *        < in[k] + 2^124 + 2^121 + 2^73 + 2^64 + 2^56 + 2^17
     *        < in[k] + 2^125
     *        < 2^128 , k < 4
     */

    acc[4] += acc[3] >> 56;   /*-
                               * acc[4] < acc_old[4] + 2^72 + 2^16
                               *        < 2^72 + 2^56 + 2^16
                               */
    acc[3] &= 0x00ffffffffffffff;

    acc[5] += acc[4] >> 56;   /*-
                               * acc[5] < acc_old[5] + 2^16 + 1
                               *        < 2^56 + 2^16 + 1
                               */
    acc[4] &= 0x00ffffffffffffff;

    acc[6] += acc[5] >> 56;   /* acc[6] < 2^48 + 1 <= 2^48 */
    acc[5] &= 0x00ffffffffffffff;

    for (i = 0; i < NLIMBS; i++)
        out[i] = acc[i];
}

static ossl_inline void felem_square_reduce_ref(felem out, const felem in)
{
    widefelem tmp;

    felem_square_ref(tmp, in);
    felem_reduce_ref(out, tmp);
}

static ossl_inline void felem_mul_reduce_ref(felem out, const felem in1, const felem in2)
{
    widefelem tmp;

    felem_mul_ref(tmp, in1, in2);
    felem_reduce_ref(out, tmp);
}

#if defined(ECP_NISTP384_ASM)
static void felem_square_wrapper(widefelem out, const felem in);
static void felem_mul_wrapper(widefelem out, const felem in1, const felem in2);

static void (*felem_square_p)(widefelem out, const felem in) =
    felem_square_wrapper;
static void (*felem_mul_p)(widefelem out, const felem in1, const felem in2) =
    felem_mul_wrapper;

static void (*felem_reduce_p)(felem out, const widefelem in) = felem_reduce_ref;

static void (*felem_square_reduce_p)(felem out, const felem in) =
    felem_square_reduce_ref;
static void (*felem_mul_reduce_p)(felem out, const felem in1, const felem in2) =
    felem_mul_reduce_ref;

void p384_felem_square(widefelem out, const felem in);
void p384_felem_mul(widefelem out, const felem in1, const felem in2);
void p384_felem_reduce(felem out, const widefelem in);

void p384_felem_square_reduce(felem out, const felem in);
void p384_felem_mul_reduce(felem out, const felem in1, const felem in2);

# if defined(_ARCH_PPC64)
#  include "crypto/ppc_arch.h"
# endif

static void felem_select(void)
{
# if defined(_ARCH_PPC64)
    if ((OPENSSL_ppccap_P & PPC_MADD300) && (OPENSSL_ppccap_P & PPC_ALTIVEC)) {
        felem_square_p = p384_felem_square;
        felem_mul_p = p384_felem_mul;
        felem_reduce_p = p384_felem_reduce;
        felem_square_reduce_p = p384_felem_square_reduce;
        felem_mul_reduce_p = p384_felem_mul_reduce;

        return;
    }
# endif

    /* Default */
    felem_square_p = felem_square_ref;
    felem_mul_p = felem_mul_ref;
    felem_reduce_p = felem_reduce_ref;
    felem_square_reduce_p = felem_square_reduce_ref;
    felem_mul_reduce_p = felem_mul_reduce_ref;
}

static void felem_square_wrapper(widefelem out, const felem in)
{
    felem_select();
    felem_square_p(out, in);
}

static void felem_mul_wrapper(widefelem out, const felem in1, const felem in2)
{
    felem_select();
    felem_mul_p(out, in1, in2);
}

# define felem_square felem_square_p
# define felem_mul felem_mul_p
# define felem_reduce felem_reduce_p

# define felem_square_reduce felem_square_reduce_p
# define felem_mul_reduce felem_mul_reduce_p
#else
# define felem_square felem_square_ref
# define felem_mul felem_mul_ref
# define felem_reduce felem_reduce_ref

# define felem_square_reduce felem_square_reduce_ref
# define felem_mul_reduce felem_mul_reduce_ref
#endif

/*-
 * felem_inv calculates |out| = |in|^{-1}
 *
 * Based on Fermat's Little Theorem:
 *   a^p = a (mod p)
 *   a^{p-1} = 1 (mod p)
 *   a^{p-2} = a^{-1} (mod p)
 */
static void felem_inv(felem out, const felem in)
{
    felem ftmp, ftmp2, ftmp3, ftmp4, ftmp5, ftmp6;
    unsigned int i = 0;

    felem_square_reduce(ftmp, in);      /* 2^1 */
    felem_mul_reduce(ftmp, ftmp, in);   /* 2^1 + 2^0 */
    felem_assign(ftmp2, ftmp);

    felem_square_reduce(ftmp, ftmp);    /* 2^2 + 2^1 */
    felem_mul_reduce(ftmp, ftmp, in);   /* 2^2 + 2^1 * 2^0 */
    felem_assign(ftmp3, ftmp);

    for (i = 0; i < 3; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^5 + 2^4 + 2^3 */
    felem_mul_reduce(ftmp, ftmp3, ftmp); /* 2^5 + 2^4 + 2^3 + 2^2 + 2^1 + 2^0 */
    felem_assign(ftmp4, ftmp);

    for (i = 0; i < 6; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^11 + ... + 2^6 */
    felem_mul_reduce(ftmp, ftmp4, ftmp); /* 2^11 + ... + 2^0 */

    for (i = 0; i < 3; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^14 + ... + 2^3 */
    felem_mul_reduce(ftmp, ftmp3, ftmp); /* 2^14 + ... + 2^0 */
    felem_assign(ftmp5, ftmp);

    for (i = 0; i < 15; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^29 + ... + 2^15 */
    felem_mul_reduce(ftmp, ftmp5, ftmp); /* 2^29 + ... + 2^0 */
    felem_assign(ftmp6, ftmp);

    for (i = 0; i < 30; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^59 + ... + 2^30 */
    felem_mul_reduce(ftmp, ftmp6, ftmp); /* 2^59 + ... + 2^0 */
    felem_assign(ftmp4, ftmp);

    for (i = 0; i < 60; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^119 + ... + 2^60 */
    felem_mul_reduce(ftmp, ftmp4, ftmp); /* 2^119 + ... + 2^0 */
    felem_assign(ftmp4, ftmp);

    for (i = 0; i < 120; i++)
      felem_square_reduce(ftmp, ftmp);   /* 2^239 + ... + 2^120 */
    felem_mul_reduce(ftmp, ftmp4, ftmp); /* 2^239 + ... + 2^0 */

    for (i = 0; i < 15; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^254 + ... + 2^15 */
    felem_mul_reduce(ftmp, ftmp5, ftmp); /* 2^254 + ... + 2^0 */

    for (i = 0; i < 31; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^285 + ... + 2^31 */
    felem_mul_reduce(ftmp, ftmp6, ftmp); /* 2^285 + ... + 2^31 + 2^29 + ... + 2^0 */

    for (i = 0; i < 2; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^287 + ... + 2^33 + 2^31 + ... + 2^2 */
    felem_mul_reduce(ftmp, ftmp2, ftmp); /* 2^287 + ... + 2^33 + 2^31 + ... + 2^0 */

    for (i = 0; i < 94; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^381 + ... + 2^127 + 2^125 + ... + 2^94 */
    felem_mul_reduce(ftmp, ftmp6, ftmp); /* 2^381 + ... + 2^127 + 2^125 + ... + 2^94 + 2^29 + ... + 2^0 */

    for (i = 0; i < 2; i++)
        felem_square_reduce(ftmp, ftmp); /* 2^383 + ... + 2^129 + 2^127 + ... + 2^96 + 2^31 + ... + 2^2 */
    felem_mul_reduce(ftmp, in, ftmp);    /* 2^383 + ... + 2^129 + 2^127 + ... + 2^96 + 2^31 + ... + 2^2 + 2^0 */

    memcpy(out, ftmp, sizeof(felem));
}

/*
 * Zero-check: returns a limb with all bits set if |in| == 0 (mod p)
 * and 0 otherwise. We know that field elements are reduced to
 * 0 < in < 2p, so we only need to check two cases:
 * 0 and 2^384 - 2^128 - 2^96 + 2^32 - 1
 *   in[k] < 2^56, k < 6
 *   in[6] <= 2^48
 */
static limb felem_is_zero(const felem in)
{
    limb zero, p384;

    zero = in[0] | in[1] | in[2] | in[3] | in[4] | in[5] | in[6];
    zero = ((int64_t) (zero) - 1) >> 63;
    p384 = (in[0] ^ 0x000000ffffffff) | (in[1] ^ 0xffff0000000000)
         | (in[2] ^ 0xfffffffffeffff) | (in[3] ^ 0xffffffffffffff)
         | (in[4] ^ 0xffffffffffffff) | (in[5] ^ 0xffffffffffffff)
         | (in[6] ^ 0xffffffffffff);
    p384 = ((int64_t) (p384) - 1) >> 63;

    return (zero | p384);
}

static int felem_is_zero_int(const void *in)
{
    return (int)(felem_is_zero(in) & ((limb) 1));
}

/*-
 * felem_contract converts |in| to its unique, minimal representation.
 * Assume we've removed all redundant bits.
 * On entry:
 *   in[k] < 2^56, k < 6
 *   in[6] <= 2^48
 */
static void felem_contract(felem out, const felem in)
{
    static const int64_t two56 = ((limb) 1) << 56;

    /*
     * We know for a fact that 0 <= |in| < 2*p, for p = 2^384 - 2^128 - 2^96 + 2^32 - 1
     * Perform two successive, idempotent subtractions to reduce if |in| >= p.
     */

    int64_t tmp[NLIMBS], cond[5], a;
    unsigned int i;

    memcpy(tmp, in, sizeof(felem));

    /* Case 1: a = 1 iff |in| >= 2^384 */
    a = (in[6] >> 48);
    tmp[0] += a;
    tmp[0] -= a << 32;
    tmp[1] += a << 40;
    tmp[2] += a << 16;
    tmp[6] &= 0x0000ffffffffffff;

    /*
     * eliminate negative coefficients: if tmp[0] is negative, tmp[1] must be
     * non-zero, so we only need one step
     */

    a = tmp[0] >> 63;
    tmp[0] += a & two56;
    tmp[1] -= a & 1;

    /* Carry 1 -> 2 -> 3 -> 4 -> 5 -> 6 */
    tmp[2] += tmp[1] >> 56;
    tmp[1] &= 0x00ffffffffffffff;

    tmp[3] += tmp[2] >> 56;
    tmp[2] &= 0x00ffffffffffffff;

    tmp[4] += tmp[3] >> 56;
    tmp[3] &= 0x00ffffffffffffff;

    tmp[5] += tmp[4] >> 56;
    tmp[4] &= 0x00ffffffffffffff;

    tmp[6] += tmp[5] >> 56; /* tmp[6] < 2^48 */
    tmp[5] &= 0x00ffffffffffffff;

    /*
     * Case 2: a = all ones if p <= |in| < 2^384, 0 otherwise
     */

    /* 0 iff (2^129..2^383) are all one */
    cond[0] = ((tmp[6] | 0xff000000000000) & tmp[5] & tmp[4] & tmp[3] & (tmp[2] | 0x0000000001ffff)) + 1;
    /* 0 iff 2^128 bit is one */
    cond[1] = (tmp[2] | ~0x00000000010000) + 1;
    /* 0 iff (2^96..2^127) bits are all one */
    cond[2] = ((tmp[2] | 0xffffffffff0000) & (tmp[1] | 0x0000ffffffffff)) + 1;
    /* 0 iff (2^32..2^95) bits are all zero */
    cond[3] = (tmp[1] & ~0xffff0000000000) | (tmp[0] & ~((int64_t) 0x000000ffffffff));
    /* 0 iff (2^0..2^31) bits are all one */
    cond[4] = (tmp[0] | 0xffffff00000000) + 1;

    /*
     * In effect, invert our conditions, so that 0 values become all 1's,
     * any non-zero value in the low-order 56 bits becomes all 0's
     */
    for (i = 0; i < 5; i++)
       cond[i] = ((cond[i] & 0x00ffffffffffffff) - 1) >> 63;

    /*
     * The condition for determining whether in is greater than our
     * prime is given by the following condition.
     */

    /* First subtract 2^384 - 2^129 cheaply */
    a = cond[0] & (cond[1] | (cond[2] & (~cond[3] | cond[4])));
    tmp[6] &= ~a;
    tmp[5] &= ~a;
    tmp[4] &= ~a;
    tmp[3] &= ~a;
    tmp[2] &= ~a | 0x0000000001ffff;

    /*
     * Subtract 2^128 - 2^96 by
     * means of disjoint cases.
     */

    /* subtract 2^128 if that bit is present, and add 2^96 */
    a = cond[0] & cond[1];
    tmp[2] &= ~a | 0xfffffffffeffff;
    tmp[1] += a & ((int64_t) 1 << 40);

    /* otherwise, clear bits 2^127 .. 2^96  */
    a = cond[0] & ~cond[1] & (cond[2] & (~cond[3] | cond[4]));
    tmp[2] &= ~a | 0xffffffffff0000;
    tmp[1] &= ~a | 0x0000ffffffffff;

    /* finally, subtract the last 2^32 - 1 */
    a = cond[0] & (cond[1] | (cond[2] & (~cond[3] | cond[4])));
    tmp[0] += a & (-((int64_t) 1 << 32) + 1);

    /*
     * eliminate negative coefficients: if tmp[0] is negative, tmp[1] must be
     * non-zero, so we only need one step
     */
    a = tmp[0] >> 63;
    tmp[0] += a & two56;
    tmp[1] -= a & 1;

    /* Carry 1 -> 2 -> 3 -> 4 -> 5 -> 6 */
    tmp[2] += tmp[1] >> 56;
    tmp[1] &= 0x00ffffffffffffff;

    tmp[3] += tmp[2] >> 56;
    tmp[2] &= 0x00ffffffffffffff;

    tmp[4] += tmp[3] >> 56;
    tmp[3] &= 0x00ffffffffffffff;

    tmp[5] += tmp[4] >> 56;
    tmp[4] &= 0x00ffffffffffffff;

    tmp[6] += tmp[5] >> 56;
    tmp[5] &= 0x00ffffffffffffff;

    memcpy(out, tmp, sizeof(felem));
}

/*-
 * Group operations
 * ----------------
 *
 * Building on top of the field operations we have the operations on the
 * elliptic curve group itself. Points on the curve are represented in Jacobian
 * coordinates
 */

/*-
 * point_double calculates 2*(x_in, y_in, z_in)
 *
 * The method is taken from:
 *   http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#doubling-dbl-2001-b
 *
 * Outputs can equal corresponding inputs, i.e., x_out == x_in is allowed.
 * while x_out == y_in is not (maybe this works, but it's not tested).
 */
static void
point_double(felem x_out, felem y_out, felem z_out,
             const felem x_in, const felem y_in, const felem z_in)
{
    widefelem tmp, tmp2;
    felem delta, gamma, beta, alpha, ftmp, ftmp2;

    felem_assign(ftmp, x_in);
    felem_assign(ftmp2, x_in);

    /* delta = z^2 */
    felem_square_reduce(delta, z_in);     /* delta[i] < 2^56 */

    /* gamma = y^2 */
    felem_square_reduce(gamma, y_in);     /* gamma[i] < 2^56 */

    /* beta = x*gamma */
    felem_mul_reduce(beta, x_in, gamma);  /* beta[i] < 2^56 */

    /* alpha = 3*(x-delta)*(x+delta) */
    felem_diff64(ftmp, delta);            /* ftmp[i] < 2^60 + 2^58 + 2^44 */
    felem_sum64(ftmp2, delta);            /* ftmp2[i] < 2^59 */
    felem_scalar64(ftmp2, 3);             /* ftmp2[i] < 2^61 */
    felem_mul_reduce(alpha, ftmp, ftmp2); /* alpha[i] < 2^56 */

    /* x' = alpha^2 - 8*beta */
    felem_square(tmp, alpha);             /* tmp[i] < 2^115 */
    felem_assign(ftmp, beta);             /* ftmp[i] < 2^56 */
    felem_scalar64(ftmp, 8);              /* ftmp[i] < 2^59 */
    felem_diff_128_64(tmp, ftmp);         /* tmp[i] < 2^115 + 2^64 + 2^48 */
    felem_reduce(x_out, tmp);             /* x_out[i] < 2^56 */

    /* z' = (y + z)^2 - gamma - delta */
    felem_sum64(delta, gamma);     /* delta[i] < 2^57 */
    felem_assign(ftmp, y_in);      /* ftmp[i] < 2^56 */
    felem_sum64(ftmp, z_in);       /* ftmp[i] < 2^56 */
    felem_square(tmp, ftmp);       /* tmp[i] < 2^115 */
    felem_diff_128_64(tmp, delta); /* tmp[i] < 2^115 + 2^64 + 2^48 */
    felem_reduce(z_out, tmp);      /* z_out[i] < 2^56 */

    /* y' = alpha*(4*beta - x') - 8*gamma^2 */
    felem_scalar64(beta, 4);       /* beta[i] < 2^58 */
    felem_diff64(beta, x_out);     /* beta[i] < 2^60 + 2^58 + 2^44 */
    felem_mul(tmp, alpha, beta);   /* tmp[i] < 2^119 */
    felem_square(tmp2, gamma);     /* tmp2[i] < 2^115 */
    felem_scalar128(tmp2, 8);      /* tmp2[i] < 2^118 */
    felem_diff128(tmp, tmp2);      /* tmp[i] < 2^127 + 2^119 + 2^111 */
    felem_reduce(y_out, tmp);      /* tmp[i] < 2^56 */
}

/* copy_conditional copies in to out iff mask is all ones. */
static void copy_conditional(felem out, const felem in, limb mask)
{
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out[i] ^= mask & (in[i] ^ out[i]);
}

/*-
 * point_add calculates (x1, y1, z1) + (x2, y2, z2)
 *
 * The method is taken from
 *   http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#addition-add-2007-bl,
 * adapted for mixed addition (z2 = 1, or z2 = 0 for the point at infinity).
 *
 * This function includes a branch for checking whether the two input points
 * are equal (while not equal to the point at infinity). See comment below
 * on constant-time.
 */
static void point_add(felem x3, felem y3, felem z3,
                      const felem x1, const felem y1, const felem z1,
                      const int mixed, const felem x2, const felem y2,
                      const felem z2)
{
    felem ftmp, ftmp2, ftmp3, ftmp4, ftmp5, ftmp6, x_out, y_out, z_out;
    widefelem tmp, tmp2;
    limb x_equal, y_equal, z1_is_zero, z2_is_zero;
    limb points_equal;

    z1_is_zero = felem_is_zero(z1);
    z2_is_zero = felem_is_zero(z2);

    /* ftmp = z1z1 = z1**2 */
    felem_square_reduce(ftmp, z1);      /* ftmp[i] < 2^56 */

    if (!mixed) {
        /* ftmp2 = z2z2 = z2**2 */
        felem_square_reduce(ftmp2, z2); /* ftmp2[i] < 2^56 */

        /* u1 = ftmp3 = x1*z2z2 */
        felem_mul_reduce(ftmp3, x1, ftmp2); /* ftmp3[i] < 2^56 */

        /* ftmp5 = z1 + z2 */
        felem_assign(ftmp5, z1);       /* ftmp5[i] < 2^56 */
        felem_sum64(ftmp5, z2);        /* ftmp5[i] < 2^57 */

        /* ftmp5 = (z1 + z2)**2 - z1z1 - z2z2 = 2*z1z2 */
        felem_square(tmp, ftmp5);      /* tmp[i] < 2^117 */
        felem_diff_128_64(tmp, ftmp);  /* tmp[i] < 2^117 + 2^64 + 2^48 */
        felem_diff_128_64(tmp, ftmp2); /* tmp[i] < 2^117 + 2^65 + 2^49 */
        felem_reduce(ftmp5, tmp);      /* ftmp5[i] < 2^56 */

        /* ftmp2 = z2 * z2z2 */
        felem_mul_reduce(ftmp2, ftmp2, z2); /* ftmp2[i] < 2^56 */

        /* s1 = ftmp6 = y1 * z2**3 */
        felem_mul_reduce(ftmp6, y1, ftmp2); /* ftmp6[i] < 2^56 */
    } else {
        /*
         * We'll assume z2 = 1 (special case z2 = 0 is handled later)
         */

        /* u1 = ftmp3 = x1*z2z2 */
        felem_assign(ftmp3, x1);     /* ftmp3[i] < 2^56 */

        /* ftmp5 = 2*z1z2 */
        felem_scalar(ftmp5, z1, 2);  /* ftmp5[i] < 2^57 */

        /* s1 = ftmp6 = y1 * z2**3 */
        felem_assign(ftmp6, y1);     /* ftmp6[i] < 2^56 */
    }
    /* ftmp3[i] < 2^56, ftmp5[i] < 2^57, ftmp6[i] < 2^56 */

    /* u2 = x2*z1z1 */
    felem_mul(tmp, x2, ftmp);        /* tmp[i] < 2^115 */

    /* h = ftmp4 = u2 - u1 */
    felem_diff_128_64(tmp, ftmp3);   /* tmp[i] < 2^115 + 2^64 + 2^48 */
    felem_reduce(ftmp4, tmp);        /* ftmp[4] < 2^56 */

    x_equal = felem_is_zero(ftmp4);

    /* z_out = ftmp5 * h */
    felem_mul_reduce(z_out, ftmp5, ftmp4);  /* z_out[i] < 2^56 */

    /* ftmp = z1 * z1z1 */
    felem_mul_reduce(ftmp, ftmp, z1);  /* ftmp[i] < 2^56 */

    /* s2 = tmp = y2 * z1**3 */
    felem_mul(tmp, y2, ftmp);      /* tmp[i] < 2^115 */

    /* r = ftmp5 = (s2 - s1)*2 */
    felem_diff_128_64(tmp, ftmp6); /* tmp[i] < 2^115 + 2^64 + 2^48 */
    felem_reduce(ftmp5, tmp);      /* ftmp5[i] < 2^56 */
    y_equal = felem_is_zero(ftmp5);
    felem_scalar64(ftmp5, 2);      /* ftmp5[i] < 2^57 */

    /*
     * The formulae are incorrect if the points are equal, in affine coordinates
     * (X_1, Y_1) == (X_2, Y_2), so we check for this and do doubling if this
     * happens.
     *
     * We use bitwise operations to avoid potential side-channels introduced by
     * the short-circuiting behaviour of boolean operators.
     *
     * The special case of either point being the point at infinity (z1 and/or
     * z2 are zero), is handled separately later on in this function, so we
     * avoid jumping to point_double here in those special cases.
     *
     * Notice the comment below on the implications of this branching for timing
     * leaks and why it is considered practically irrelevant.
     */
    points_equal = (x_equal & y_equal & (~z1_is_zero) & (~z2_is_zero));

    if (points_equal) {
        /*
         * This is obviously not constant-time but it will almost-never happen
         * for ECDH / ECDSA.
         */
        point_double(x3, y3, z3, x1, y1, z1);
        return;
    }

    /* I = ftmp = (2h)**2 */
    felem_assign(ftmp, ftmp4);        /* ftmp[i] < 2^56 */
    felem_scalar64(ftmp, 2);          /* ftmp[i] < 2^57 */
    felem_square_reduce(ftmp, ftmp);  /* ftmp[i] < 2^56 */

    /* J = ftmp2 = h * I */
    felem_mul_reduce(ftmp2, ftmp4, ftmp); /* ftmp2[i] < 2^56 */

    /* V = ftmp4 = U1 * I */
    felem_mul_reduce(ftmp4, ftmp3, ftmp); /* ftmp4[i] < 2^56 */

    /* x_out = r**2 - J - 2V */
    felem_square(tmp, ftmp5);      /* tmp[i] < 2^117 */
    felem_diff_128_64(tmp, ftmp2); /* tmp[i] < 2^117 + 2^64 + 2^48 */
    felem_assign(ftmp3, ftmp4);    /* ftmp3[i] < 2^56 */
    felem_scalar64(ftmp4, 2);      /* ftmp4[i] < 2^57 */
    felem_diff_128_64(tmp, ftmp4); /* tmp[i] < 2^117 + 2^65 + 2^49 */
    felem_reduce(x_out, tmp);      /* x_out[i] < 2^56 */

    /* y_out = r(V-x_out) - 2 * s1 * J */
    felem_diff64(ftmp3, x_out);    /* ftmp3[i] < 2^60 + 2^56 + 2^44 */
    felem_mul(tmp, ftmp5, ftmp3);  /* tmp[i] < 2^116 */
    felem_mul(tmp2, ftmp6, ftmp2); /* tmp2[i] < 2^115 */
    felem_scalar128(tmp2, 2);      /* tmp2[i] < 2^116 */
    felem_diff128(tmp, tmp2);      /* tmp[i] < 2^127 + 2^116 + 2^111 */
    felem_reduce(y_out, tmp);      /* y_out[i] < 2^56 */

    copy_conditional(x_out, x2, z1_is_zero);
    copy_conditional(x_out, x1, z2_is_zero);
    copy_conditional(y_out, y2, z1_is_zero);
    copy_conditional(y_out, y1, z2_is_zero);
    copy_conditional(z_out, z2, z1_is_zero);
    copy_conditional(z_out, z1, z2_is_zero);
    felem_assign(x3, x_out);
    felem_assign(y3, y_out);
    felem_assign(z3, z_out);
}

/*-
 * Base point pre computation
 * --------------------------
 *
 * Two different sorts of precomputed tables are used in the following code.
 * Each contain various points on the curve, where each point is three field
 * elements (x, y, z).
 *
 * For the base point table, z is usually 1 (0 for the point at infinity).
 * This table has 16 elements:
 * index | bits    | point
 * ------+---------+------------------------------
 *     0 | 0 0 0 0 | 0G
 *     1 | 0 0 0 1 | 1G
 *     2 | 0 0 1 0 | 2^95G
 *     3 | 0 0 1 1 | (2^95 + 1)G
 *     4 | 0 1 0 0 | 2^190G
 *     5 | 0 1 0 1 | (2^190 + 1)G
 *     6 | 0 1 1 0 | (2^190 + 2^95)G
 *     7 | 0 1 1 1 | (2^190 + 2^95 + 1)G
 *     8 | 1 0 0 0 | 2^285G
 *     9 | 1 0 0 1 | (2^285 + 1)G
 *    10 | 1 0 1 0 | (2^285 + 2^95)G
 *    11 | 1 0 1 1 | (2^285 + 2^95 + 1)G
 *    12 | 1 1 0 0 | (2^285 + 2^190)G
 *    13 | 1 1 0 1 | (2^285 + 2^190 + 1)G
 *    14 | 1 1 1 0 | (2^285 + 2^190 + 2^95)G
 *    15 | 1 1 1 1 | (2^285 + 2^190 + 2^95 + 1)G
 *
 * The reason for this is so that we can clock bits into four different
 * locations when doing simple scalar multiplies against the base point.
 *
 * Tables for other points have table[i] = iG for i in 0 .. 16.
 */

/* gmul is the table of precomputed base points */
static const felem gmul[16][3] = {
{{0, 0, 0, 0, 0, 0, 0},
 {0, 0, 0, 0, 0, 0, 0},
 {0, 0, 0, 0, 0, 0, 0}},
{{0x00545e3872760ab7, 0x00f25dbf55296c3a, 0x00e082542a385502, 0x008ba79b9859f741,
  0x0020ad746e1d3b62, 0x0005378eb1c71ef3, 0x0000aa87ca22be8b},
 {0x00431d7c90ea0e5f, 0x00b1ce1d7e819d7a, 0x0013b5f0b8c00a60, 0x00289a147ce9da31,
  0x0092dc29f8f41dbd, 0x002c6f5d9e98bf92, 0x00003617de4a9626},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00024711cc902a90, 0x00acb2e579ab4fe1, 0x00af818a4b4d57b1, 0x00a17c7bec49c3de,
  0x004280482d726a8b, 0x00128dd0f0a90f3b, 0x00004387c1c3fa3c},
 {0x002ce76543cf5c3a, 0x00de6cee5ef58f0a, 0x00403e42fa561ca6, 0x00bc54d6f9cb9731,
  0x007155f925fb4ff1, 0x004a9ce731b7b9bc, 0x00002609076bd7b2},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00e74c9182f0251d, 0x0039bf54bb111974, 0x00b9d2f2eec511d2, 0x0036b1594eb3a6a4,
  0x00ac3bb82d9d564b, 0x00f9313f4615a100, 0x00006716a9a91b10},
 {0x0046698116e2f15c, 0x00f34347067d3d33, 0x008de4ccfdebd002, 0x00e838c6b8e8c97b,
  0x006faf0798def346, 0x007349794a57563c, 0x00002629e7e6ad84},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x0075300e34fd163b, 0x0092e9db4e8d0ad3, 0x00254be9f625f760, 0x00512c518c72ae68,
  0x009bfcf162bede5a, 0x00bf9341566ce311, 0x0000cd6175bd41cf},
 {0x007dfe52af4ac70f, 0x0002159d2d5c4880, 0x00b504d16f0af8d0, 0x0014585e11f5e64c,
  0x0089c6388e030967, 0x00ffb270cbfa5f71, 0x00009a15d92c3947},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x0033fc1278dc4fe5, 0x00d53088c2caa043, 0x0085558827e2db66, 0x00c192bef387b736,
  0x00df6405a2225f2c, 0x0075205aa90fd91a, 0x0000137e3f12349d},
 {0x00ce5b115efcb07e, 0x00abc3308410deeb, 0x005dc6fc1de39904, 0x00907c1c496f36b4,
  0x0008e6ad3926cbe1, 0x00110747b787928c, 0x0000021b9162eb7e},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x008180042cfa26e1, 0x007b826a96254967, 0x0082473694d6b194, 0x007bd6880a45b589,
  0x00c0a5097072d1a3, 0x0019186555e18b4e, 0x000020278190e5ca},
 {0x00b4bef17de61ac0, 0x009535e3c38ed348, 0x002d4aa8e468ceab, 0x00ef40b431036ad3,
  0x00defd52f4542857, 0x0086edbf98234266, 0x00002025b3a7814d},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00b238aa97b886be, 0x00ef3192d6dd3a32, 0x0079f9e01fd62df8, 0x00742e890daba6c5,
  0x008e5289144408ce, 0x0073bbcc8e0171a5, 0x0000c4fd329d3b52},
 {0x00c6f64a15ee23e7, 0x00dcfb7b171cad8b, 0x00039f6cbd805867, 0x00de024e428d4562,
  0x00be6a594d7c64c5, 0x0078467b70dbcd64, 0x0000251f2ed7079b},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x000e5cc25fc4b872, 0x005ebf10d31ef4e1, 0x0061e0ebd11e8256, 0x0076e026096f5a27,
  0x0013e6fc44662e9a, 0x0042b00289d3597e, 0x000024f089170d88},
 {0x001604d7e0effbe6, 0x0048d77cba64ec2c, 0x008166b16da19e36, 0x006b0d1a0f28c088,
  0x000259fcd47754fd, 0x00cc643e4d725f9a, 0x00007b10f3c79c14},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00430155e3b908af, 0x00b801e4fec25226, 0x00b0d4bcfe806d26, 0x009fc4014eb13d37,
  0x0066c94e44ec07e8, 0x00d16adc03874ba2, 0x000030c917a0d2a7},
 {0x00edac9e21eb891c, 0x00ef0fb768102eff, 0x00c088cef272a5f3, 0x00cbf782134e2964,
  0x0001044a7ba9a0e3, 0x00e363f5b194cf3c, 0x00009ce85249e372},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x001dd492dda5a7eb, 0x008fd577be539fd1, 0x002ff4b25a5fc3f1, 0x0074a8a1b64df72f,
  0x002ba3d8c204a76c, 0x009d5cff95c8235a, 0x0000e014b9406e0f},
 {0x008c2e4dbfc98aba, 0x00f30bb89f1a1436, 0x00b46f7aea3e259c, 0x009224454ac02f54,
  0x00906401f5645fa2, 0x003a1d1940eabc77, 0x00007c9351d680e6},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x005a35d872ef967c, 0x0049f1b7884e1987, 0x0059d46d7e31f552, 0x00ceb4869d2d0fb6,
  0x00e8e89eee56802a, 0x0049d806a774aaf2, 0x0000147e2af0ae24},
 {0x005fd1bd852c6e5e, 0x00b674b7b3de6885, 0x003b9ea5eb9b6c08, 0x005c9f03babf3ef7,
  0x00605337fecab3c7, 0x009a3f85b11bbcc8, 0x0000455470f330ec},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x002197ff4d55498d, 0x00383e8916c2d8af, 0x00eb203f34d1c6d2, 0x0080367cbd11b542,
  0x00769b3be864e4f5, 0x0081a8458521c7bb, 0x0000c531b34d3539},
 {0x00e2a3d775fa2e13, 0x00534fc379573844, 0x00ff237d2a8db54a, 0x00d301b2335a8882,
  0x000f75ea96103a80, 0x0018fecb3cdd96fa, 0x0000304bf61e94eb},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00b2afc332a73dbd, 0x0029a0d5bb007bc5, 0x002d628eb210f577, 0x009f59a36dd05f50,
  0x006d339de4eca613, 0x00c75a71addc86bc, 0x000060384c5ea93c},
 {0x00aa9641c32a30b4, 0x00cc73ae8cce565d, 0x00ec911a4df07f61, 0x00aa4b762ea4b264,
  0x0096d395bb393629, 0x004efacfb7632fe0, 0x00006f252f46fa3f},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00567eec597c7af6, 0x0059ba6795204413, 0x00816d4e6f01196f, 0x004ae6b3eb57951d,
  0x00420f5abdda2108, 0x003401d1f57ca9d9, 0x0000cf5837b0b67a},
 {0x00eaa64b8aeeabf9, 0x00246ddf16bcb4de, 0x000e7e3c3aecd751, 0x0008449f04fed72e,
  0x00307b67ccf09183, 0x0017108c3556b7b1, 0x0000229b2483b3bf},
 {1, 0, 0, 0, 0, 0, 0}},
{{0x00e7c491a7bb78a1, 0x00eafddd1d3049ab, 0x00352c05e2bc7c98, 0x003d6880c165fa5c,
  0x00b6ac61cc11c97d, 0x00beeb54fcf90ce5, 0x0000dc1f0b455edc},
 {0x002db2e7aee34d60, 0x0073b5f415a2d8c0, 0x00dd84e4193e9a0c, 0x00d02d873467c572,
  0x0018baaeda60aee5, 0x0013fb11d697c61e, 0x000083aafcc3a973},
 {1, 0, 0, 0, 0, 0, 0}}
};

/*
 * select_point selects the |idx|th point from a precomputation table and
 * copies it to out.
 *
 * pre_comp below is of the size provided in |size|.
 */
static void select_point(const limb idx, unsigned int size,
                         const felem pre_comp[][3], felem out[3])
{
    unsigned int i, j;
    limb *outlimbs = &out[0][0];

    memset(out, 0, sizeof(*out) * 3);

    for (i = 0; i < size; i++) {
        const limb *inlimbs = &pre_comp[i][0][0];
        limb mask = i ^ idx;

        mask |= mask >> 4;
        mask |= mask >> 2;
        mask |= mask >> 1;
        mask &= 1;
        mask--;
        for (j = 0; j < NLIMBS * 3; j++)
            outlimbs[j] |= inlimbs[j] & mask;
    }
}

/* get_bit returns the |i|th bit in |in| */
static char get_bit(const felem_bytearray in, int i)
{
    if (i < 0 || i >= 384)
        return 0;
    return (in[i >> 3] >> (i & 7)) & 1;
}

/*
 * Interleaved point multiplication using precomputed point multiples: The
 * small point multiples 0*P, 1*P, ..., 16*P are in pre_comp[], the scalars
 * in scalars[]. If g_scalar is non-NULL, we also add this multiple of the
 * generator, using certain (large) precomputed multiples in g_pre_comp.
 * Output point (X, Y, Z) is stored in x_out, y_out, z_out
 */
static void batch_mul(felem x_out, felem y_out, felem z_out,
                      const felem_bytearray scalars[],
                      const unsigned int num_points, const u8 *g_scalar,
                      const int mixed, const felem pre_comp[][17][3],
                      const felem g_pre_comp[16][3])
{
    int i, skip;
    unsigned int num, gen_mul = (g_scalar != NULL);
    felem nq[3], tmp[4];
    limb bits;
    u8 sign, digit;

    /* set nq to the point at infinity */
    memset(nq, 0, sizeof(nq));

    /*
     * Loop over all scalars msb-to-lsb, interleaving additions of multiples
     * of the generator (last quarter of rounds) and additions of other
     * points multiples (every 5th round).
     */
    skip = 1;                   /* save two point operations in the first
                                 * round */
    for (i = (num_points ? 380 : 98); i >= 0; --i) {
        /* double */
        if (!skip)
            point_double(nq[0], nq[1], nq[2], nq[0], nq[1], nq[2]);

        /* add multiples of the generator */
        if (gen_mul && (i <= 98)) {
            bits = get_bit(g_scalar, i + 285) << 3;
            if (i < 95) {
                bits |= get_bit(g_scalar, i + 190) << 2;
                bits |= get_bit(g_scalar, i + 95) << 1;
                bits |= get_bit(g_scalar, i);
            }
            /* select the point to add, in constant time */
            select_point(bits, 16, g_pre_comp, tmp);
            if (!skip) {
                /* The 1 argument below is for "mixed" */
                point_add(nq[0],  nq[1],  nq[2],
                          nq[0],  nq[1],  nq[2], 1,
                          tmp[0], tmp[1], tmp[2]);
            } else {
                memcpy(nq, tmp, 3 * sizeof(felem));
                skip = 0;
            }
        }

        /* do other additions every 5 doublings */
        if (num_points && (i % 5 == 0)) {
            /* loop over all scalars */
            for (num = 0; num < num_points; ++num) {
                bits = get_bit(scalars[num], i + 4) << 5;
                bits |= get_bit(scalars[num], i + 3) << 4;
                bits |= get_bit(scalars[num], i + 2) << 3;
                bits |= get_bit(scalars[num], i + 1) << 2;
                bits |= get_bit(scalars[num], i) << 1;
                bits |= get_bit(scalars[num], i - 1);
                ossl_ec_GFp_nistp_recode_scalar_bits(&sign, &digit, bits);

                /*
                 * select the point to add or subtract, in constant time
                 */
                select_point(digit, 17, pre_comp[num], tmp);
                felem_neg(tmp[3], tmp[1]); /* (X, -Y, Z) is the negative
                                            * point */
                copy_conditional(tmp[1], tmp[3], (-(limb) sign));

                if (!skip) {
                    point_add(nq[0],  nq[1],  nq[2],
                              nq[0],  nq[1],  nq[2], mixed,
                              tmp[0], tmp[1], tmp[2]);
                } else {
                    memcpy(nq, tmp, 3 * sizeof(felem));
                    skip = 0;
                }
            }
        }
    }
    felem_assign(x_out, nq[0]);
    felem_assign(y_out, nq[1]);
    felem_assign(z_out, nq[2]);
}

/* Precomputation for the group generator. */
struct nistp384_pre_comp_st {
    felem g_pre_comp[16][3];
    CRYPTO_REF_COUNT references;
};

const EC_METHOD *ossl_ec_GFp_nistp384_method(void)
{
    static const EC_METHOD ret = {
        EC_FLAGS_DEFAULT_OCT,
        NID_X9_62_prime_field,
        ossl_ec_GFp_nistp384_group_init,
        ossl_ec_GFp_simple_group_finish,
        ossl_ec_GFp_simple_group_clear_finish,
        ossl_ec_GFp_nist_group_copy,
        ossl_ec_GFp_nistp384_group_set_curve,
        ossl_ec_GFp_simple_group_get_curve,
        ossl_ec_GFp_simple_group_get_degree,
        ossl_ec_group_simple_order_bits,
        ossl_ec_GFp_simple_group_check_discriminant,
        ossl_ec_GFp_simple_point_init,
        ossl_ec_GFp_simple_point_finish,
        ossl_ec_GFp_simple_point_clear_finish,
        ossl_ec_GFp_simple_point_copy,
        ossl_ec_GFp_simple_point_set_to_infinity,
        ossl_ec_GFp_simple_point_set_affine_coordinates,
        ossl_ec_GFp_nistp384_point_get_affine_coordinates,
        0, /* point_set_compressed_coordinates */
        0, /* point2oct */
        0, /* oct2point */
        ossl_ec_GFp_simple_add,
        ossl_ec_GFp_simple_dbl,
        ossl_ec_GFp_simple_invert,
        ossl_ec_GFp_simple_is_at_infinity,
        ossl_ec_GFp_simple_is_on_curve,
        ossl_ec_GFp_simple_cmp,
        ossl_ec_GFp_simple_make_affine,
        ossl_ec_GFp_simple_points_make_affine,
        ossl_ec_GFp_nistp384_points_mul,
        ossl_ec_GFp_nistp384_precompute_mult,
        ossl_ec_GFp_nistp384_have_precompute_mult,
        ossl_ec_GFp_nist_field_mul,
        ossl_ec_GFp_nist_field_sqr,
        0, /* field_div */
        ossl_ec_GFp_simple_field_inv,
        0, /* field_encode */
        0, /* field_decode */
        0, /* field_set_to_one */
        ossl_ec_key_simple_priv2oct,
        ossl_ec_key_simple_oct2priv,
        0, /* set private */
        ossl_ec_key_simple_generate_key,
        ossl_ec_key_simple_check_key,
        ossl_ec_key_simple_generate_public_key,
        0, /* keycopy */
        0, /* keyfinish */
        ossl_ecdh_simple_compute_key,
        ossl_ecdsa_simple_sign_setup,
        ossl_ecdsa_simple_sign_sig,
        ossl_ecdsa_simple_verify_sig,
        0, /* field_inverse_mod_ord */
        0, /* blind_coordinates */
        0, /* ladder_pre */
        0, /* ladder_step */
        0  /* ladder_post */
    };

    return &ret;
}

/******************************************************************************/
/*
 * FUNCTIONS TO MANAGE PRECOMPUTATION
 */

static NISTP384_PRE_COMP *nistp384_pre_comp_new(void)
{
    NISTP384_PRE_COMP *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return ret;

    if (!CRYPTO_NEW_REF(&ret->references, 1)) {
        OPENSSL_free(ret);
        return NULL;
    }
    return ret;
}

NISTP384_PRE_COMP *ossl_ec_nistp384_pre_comp_dup(NISTP384_PRE_COMP *p)
{
    int i;

    if (p != NULL)
        CRYPTO_UP_REF(&p->references, &i);
    return p;
}

void ossl_ec_nistp384_pre_comp_free(NISTP384_PRE_COMP *p)
{
    int i;

    if (p == NULL)
        return;

    CRYPTO_DOWN_REF(&p->references, &i);
    REF_PRINT_COUNT("ossl_ec_nistp384", i, p);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    CRYPTO_FREE_REF(&p->references);
    OPENSSL_free(p);
}

/******************************************************************************/
/*
 * OPENSSL EC_METHOD FUNCTIONS
 */

int ossl_ec_GFp_nistp384_group_init(EC_GROUP *group)
{
    int ret;

    ret = ossl_ec_GFp_simple_group_init(group);
    group->a_is_minus3 = 1;
    return ret;
}

int ossl_ec_GFp_nistp384_group_set_curve(EC_GROUP *group, const BIGNUM *p,
                                         const BIGNUM *a, const BIGNUM *b,
                                         BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *curve_p, *curve_a, *curve_b;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;

    if (ctx == NULL)
        ctx = new_ctx = BN_CTX_new();
#endif
    if (ctx == NULL)
        return 0;

    BN_CTX_start(ctx);
    curve_p = BN_CTX_get(ctx);
    curve_a = BN_CTX_get(ctx);
    curve_b = BN_CTX_get(ctx);
    if (curve_b == NULL)
        goto err;
    BN_bin2bn(nistp384_curve_params[0], sizeof(felem_bytearray), curve_p);
    BN_bin2bn(nistp384_curve_params[1], sizeof(felem_bytearray), curve_a);
    BN_bin2bn(nistp384_curve_params[2], sizeof(felem_bytearray), curve_b);
    if ((BN_cmp(curve_p, p)) || (BN_cmp(curve_a, a)) || (BN_cmp(curve_b, b))) {
        ERR_raise(ERR_LIB_EC, EC_R_WRONG_CURVE_PARAMETERS);
        goto err;
    }
    group->field_mod_func = BN_nist_mod_384;
    ret = ossl_ec_GFp_simple_group_set_curve(group, p, a, b, ctx);
 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/*
 * Takes the Jacobian coordinates (X, Y, Z) of a point and returns (X', Y') =
 * (X/Z^2, Y/Z^3)
 */
int ossl_ec_GFp_nistp384_point_get_affine_coordinates(const EC_GROUP *group,
                                                      const EC_POINT *point,
                                                      BIGNUM *x, BIGNUM *y,
                                                      BN_CTX *ctx)
{
    felem z1, z2, x_in, y_in, x_out, y_out;
    widefelem tmp;

    if (EC_POINT_is_at_infinity(group, point)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_AT_INFINITY);
        return 0;
    }
    if ((!BN_to_felem(x_in, point->X)) || (!BN_to_felem(y_in, point->Y)) ||
        (!BN_to_felem(z1, point->Z)))
        return 0;
    felem_inv(z2, z1);
    felem_square(tmp, z2);
    felem_reduce(z1, tmp);
    felem_mul(tmp, x_in, z1);
    felem_reduce(x_in, tmp);
    felem_contract(x_out, x_in);
    if (x != NULL) {
        if (!felem_to_BN(x, x_out)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            return 0;
        }
    }
    felem_mul(tmp, z1, z2);
    felem_reduce(z1, tmp);
    felem_mul(tmp, y_in, z1);
    felem_reduce(y_in, tmp);
    felem_contract(y_out, y_in);
    if (y != NULL) {
        if (!felem_to_BN(y, y_out)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            return 0;
        }
    }
    return 1;
}

/* points below is of size |num|, and tmp_felems is of size |num+1/ */
static void make_points_affine(size_t num, felem points[][3],
                               felem tmp_felems[])
{
    /*
     * Runs in constant time, unless an input is the point at infinity (which
     * normally shouldn't happen).
     */
    ossl_ec_GFp_nistp_points_make_affine_internal(num,
                                                  points,
                                                  sizeof(felem),
                                                  tmp_felems,
                                                  (void (*)(void *))felem_one,
                                                  felem_is_zero_int,
                                                  (void (*)(void *, const void *))
                                                  felem_assign,
                                                  (void (*)(void *, const void *))
                                                  felem_square_reduce,
                                                  (void (*)(void *, const void *, const void*))
                                                  felem_mul_reduce,
                                                  (void (*)(void *, const void *))
                                                  felem_inv,
                                                  (void (*)(void *, const void *))
                                                  felem_contract);
}

/*
 * Computes scalar*generator + \sum scalars[i]*points[i], ignoring NULL
 * values Result is stored in r (r can equal one of the inputs).
 */
int ossl_ec_GFp_nistp384_points_mul(const EC_GROUP *group, EC_POINT *r,
                                    const BIGNUM *scalar, size_t num,
                                    const EC_POINT *points[],
                                    const BIGNUM *scalars[], BN_CTX *ctx)
{
    int ret = 0;
    int j;
    int mixed = 0;
    BIGNUM *x, *y, *z, *tmp_scalar;
    felem_bytearray g_secret;
    felem_bytearray *secrets = NULL;
    felem (*pre_comp)[17][3] = NULL;
    felem *tmp_felems = NULL;
    unsigned int i;
    int num_bytes;
    int have_pre_comp = 0;
    size_t num_points = num;
    felem x_in, y_in, z_in, x_out, y_out, z_out;
    NISTP384_PRE_COMP *pre = NULL;
    felem(*g_pre_comp)[3] = NULL;
    EC_POINT *generator = NULL;
    const EC_POINT *p = NULL;
    const BIGNUM *p_scalar = NULL;

    BN_CTX_start(ctx);
    x = BN_CTX_get(ctx);
    y = BN_CTX_get(ctx);
    z = BN_CTX_get(ctx);
    tmp_scalar = BN_CTX_get(ctx);
    if (tmp_scalar == NULL)
        goto err;

    if (scalar != NULL) {
        pre = group->pre_comp.nistp384;
        if (pre)
            /* we have precomputation, try to use it */
            g_pre_comp = &pre->g_pre_comp[0];
        else
            /* try to use the standard precomputation */
            g_pre_comp = (felem(*)[3]) gmul;
        generator = EC_POINT_new(group);
        if (generator == NULL)
            goto err;
        /* get the generator from precomputation */
        if (!felem_to_BN(x, g_pre_comp[1][0]) ||
            !felem_to_BN(y, g_pre_comp[1][1]) ||
            !felem_to_BN(z, g_pre_comp[1][2])) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
        if (!ossl_ec_GFp_simple_set_Jprojective_coordinates_GFp(group,
                                                                generator,
                                                                x, y, z, ctx))
            goto err;
        if (0 == EC_POINT_cmp(group, generator, group->generator, ctx))
            /* precomputation matches generator */
            have_pre_comp = 1;
        else
            /*
             * we don't have valid precomputation: treat the generator as a
             * random point
             */
            num_points++;
    }

    if (num_points > 0) {
        if (num_points >= 2) {
            /*
             * unless we precompute multiples for just one point, converting
             * those into affine form is time well spent
             */
            mixed = 1;
        }
        secrets = OPENSSL_zalloc(sizeof(*secrets) * num_points);
        pre_comp = OPENSSL_zalloc(sizeof(*pre_comp) * num_points);
        if (mixed)
            tmp_felems =
                OPENSSL_malloc(sizeof(*tmp_felems) * (num_points * 17 + 1));
        if ((secrets == NULL) || (pre_comp == NULL)
            || (mixed && (tmp_felems == NULL)))
            goto err;

        /*
         * we treat NULL scalars as 0, and NULL points as points at infinity,
         * i.e., they contribute nothing to the linear combination
         */
        for (i = 0; i < num_points; ++i) {
            if (i == num) {
                /*
                 * we didn't have a valid precomputation, so we pick the
                 * generator
                 */
                p = EC_GROUP_get0_generator(group);
                p_scalar = scalar;
            } else {
                /* the i^th point */
                p = points[i];
                p_scalar = scalars[i];
            }
            if (p_scalar != NULL && p != NULL) {
                /* reduce scalar to 0 <= scalar < 2^384 */
                if ((BN_num_bits(p_scalar) > 384)
                    || (BN_is_negative(p_scalar))) {
                    /*
                     * this is an unusual input, and we don't guarantee
                     * constant-timeness
                     */
                    if (!BN_nnmod(tmp_scalar, p_scalar, group->order, ctx)) {
                        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
                        goto err;
                    }
                    num_bytes = BN_bn2lebinpad(tmp_scalar,
                                               secrets[i], sizeof(secrets[i]));
                } else {
                    num_bytes = BN_bn2lebinpad(p_scalar,
                                               secrets[i], sizeof(secrets[i]));
                }
                if (num_bytes < 0) {
                    ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
                    goto err;
                }
                /* precompute multiples */
                if ((!BN_to_felem(x_out, p->X)) ||
                    (!BN_to_felem(y_out, p->Y)) ||
                    (!BN_to_felem(z_out, p->Z)))
                    goto err;
                memcpy(pre_comp[i][1][0], x_out, sizeof(felem));
                memcpy(pre_comp[i][1][1], y_out, sizeof(felem));
                memcpy(pre_comp[i][1][2], z_out, sizeof(felem));
                for (j = 2; j <= 16; ++j) {
                    if (j & 1) {
                        point_add(pre_comp[i][j][0],     pre_comp[i][j][1],     pre_comp[i][j][2],
                                  pre_comp[i][1][0],     pre_comp[i][1][1],     pre_comp[i][1][2], 0,
                                  pre_comp[i][j - 1][0], pre_comp[i][j - 1][1], pre_comp[i][j - 1][2]);
                    } else {
                        point_double(pre_comp[i][j][0],     pre_comp[i][j][1],     pre_comp[i][j][2],
                                     pre_comp[i][j / 2][0], pre_comp[i][j / 2][1], pre_comp[i][j / 2][2]);
                    }
                }
            }
        }
        if (mixed)
            make_points_affine(num_points * 17, pre_comp[0], tmp_felems);
    }

    /* the scalar for the generator */
    if (scalar != NULL && have_pre_comp) {
        memset(g_secret, 0, sizeof(g_secret));
        /* reduce scalar to 0 <= scalar < 2^384 */
        if ((BN_num_bits(scalar) > 384) || (BN_is_negative(scalar))) {
            /*
             * this is an unusual input, and we don't guarantee
             * constant-timeness
             */
            if (!BN_nnmod(tmp_scalar, scalar, group->order, ctx)) {
                ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
                goto err;
            }
            num_bytes = BN_bn2lebinpad(tmp_scalar, g_secret, sizeof(g_secret));
        } else {
            num_bytes = BN_bn2lebinpad(scalar, g_secret, sizeof(g_secret));
        }
        /* do the multiplication with generator precomputation */
        batch_mul(x_out, y_out, z_out,
                  (const felem_bytearray(*))secrets, num_points,
                  g_secret,
                  mixed, (const felem(*)[17][3])pre_comp,
                  (const felem(*)[3])g_pre_comp);
    } else {
        /* do the multiplication without generator precomputation */
        batch_mul(x_out, y_out, z_out,
                  (const felem_bytearray(*))secrets, num_points,
                  NULL, mixed, (const felem(*)[17][3])pre_comp, NULL);
    }
    /* reduce the output to its unique minimal representation */
    felem_contract(x_in, x_out);
    felem_contract(y_in, y_out);
    felem_contract(z_in, z_out);
    if ((!felem_to_BN(x, x_in)) || (!felem_to_BN(y, y_in)) ||
        (!felem_to_BN(z, z_in))) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    ret = ossl_ec_GFp_simple_set_Jprojective_coordinates_GFp(group, r, x, y, z,
                                                             ctx);

 err:
    BN_CTX_end(ctx);
    EC_POINT_free(generator);
    OPENSSL_free(secrets);
    OPENSSL_free(pre_comp);
    OPENSSL_free(tmp_felems);
    return ret;
}

int ossl_ec_GFp_nistp384_precompute_mult(EC_GROUP *group, BN_CTX *ctx)
{
    int ret = 0;
    NISTP384_PRE_COMP *pre = NULL;
    int i, j;
    BIGNUM *x, *y;
    EC_POINT *generator = NULL;
    felem tmp_felems[16];
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    /* throw away old precomputation */
    EC_pre_comp_free(group);

#ifndef FIPS_MODULE
    if (ctx == NULL)
        ctx = new_ctx = BN_CTX_new();
#endif
    if (ctx == NULL)
        return 0;

    BN_CTX_start(ctx);
    x = BN_CTX_get(ctx);
    y = BN_CTX_get(ctx);
    if (y == NULL)
        goto err;
    /* get the generator */
    if (group->generator == NULL)
        goto err;
    generator = EC_POINT_new(group);
    if (generator == NULL)
        goto err;
    BN_bin2bn(nistp384_curve_params[3], sizeof(felem_bytearray), x);
    BN_bin2bn(nistp384_curve_params[4], sizeof(felem_bytearray), y);
    if (!EC_POINT_set_affine_coordinates(group, generator, x, y, ctx))
        goto err;
    if ((pre = nistp384_pre_comp_new()) == NULL)
        goto err;
    /*
     * if the generator is the standard one, use built-in precomputation
     */
    if (0 == EC_POINT_cmp(group, generator, group->generator, ctx)) {
        memcpy(pre->g_pre_comp, gmul, sizeof(pre->g_pre_comp));
        goto done;
    }
    if ((!BN_to_felem(pre->g_pre_comp[1][0], group->generator->X)) ||
        (!BN_to_felem(pre->g_pre_comp[1][1], group->generator->Y)) ||
        (!BN_to_felem(pre->g_pre_comp[1][2], group->generator->Z)))
        goto err;
    /* compute 2^95*G, 2^190*G, 2^285*G */
    for (i = 1; i <= 4; i <<= 1) {
        point_double(pre->g_pre_comp[2 * i][0], pre->g_pre_comp[2 * i][1], pre->g_pre_comp[2 * i][2],
                     pre->g_pre_comp[i][0],  pre->g_pre_comp[i][1],    pre->g_pre_comp[i][2]);
        for (j = 0; j < 94; ++j) {
            point_double(pre->g_pre_comp[2 * i][0], pre->g_pre_comp[2 * i][1], pre->g_pre_comp[2 * i][2],
                         pre->g_pre_comp[2 * i][0], pre->g_pre_comp[2 * i][1], pre->g_pre_comp[2 * i][2]);
        }
    }
    /* g_pre_comp[0] is the point at infinity */
    memset(pre->g_pre_comp[0], 0, sizeof(pre->g_pre_comp[0]));
    /* the remaining multiples */
    /* 2^95*G + 2^190*G */
    point_add(pre->g_pre_comp[6][0],  pre->g_pre_comp[6][1],  pre->g_pre_comp[6][2],
              pre->g_pre_comp[4][0],  pre->g_pre_comp[4][1],  pre->g_pre_comp[4][2], 0,
              pre->g_pre_comp[2][0],  pre->g_pre_comp[2][1],  pre->g_pre_comp[2][2]);
    /* 2^95*G + 2^285*G */
    point_add(pre->g_pre_comp[10][0], pre->g_pre_comp[10][1], pre->g_pre_comp[10][2],
              pre->g_pre_comp[8][0],  pre->g_pre_comp[8][1],  pre->g_pre_comp[8][2], 0,
              pre->g_pre_comp[2][0],  pre->g_pre_comp[2][1],  pre->g_pre_comp[2][2]);
    /* 2^190*G + 2^285*G */
    point_add(pre->g_pre_comp[12][0], pre->g_pre_comp[12][1], pre->g_pre_comp[12][2],
              pre->g_pre_comp[8][0],  pre->g_pre_comp[8][1],  pre->g_pre_comp[8][2], 0,
              pre->g_pre_comp[4][0],  pre->g_pre_comp[4][1],  pre->g_pre_comp[4][2]);
    /* 2^95*G + 2^190*G + 2^285*G */
    point_add(pre->g_pre_comp[14][0], pre->g_pre_comp[14][1], pre->g_pre_comp[14][2],
              pre->g_pre_comp[12][0], pre->g_pre_comp[12][1], pre->g_pre_comp[12][2], 0,
              pre->g_pre_comp[2][0],  pre->g_pre_comp[2][1],  pre->g_pre_comp[2][2]);
    for (i = 1; i < 8; ++i) {
        /* odd multiples: add G */
        point_add(pre->g_pre_comp[2 * i + 1][0], pre->g_pre_comp[2 * i + 1][1], pre->g_pre_comp[2 * i + 1][2],
                  pre->g_pre_comp[2 * i][0],     pre->g_pre_comp[2 * i][1],     pre->g_pre_comp[2 * i][2], 0,
                  pre->g_pre_comp[1][0],         pre->g_pre_comp[1][1],         pre->g_pre_comp[1][2]);
    }
    make_points_affine(15, &(pre->g_pre_comp[1]), tmp_felems);

 done:
    SETPRECOMP(group, nistp384, pre);
    ret = 1;
    pre = NULL;
 err:
    BN_CTX_end(ctx);
    EC_POINT_free(generator);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    ossl_ec_nistp384_pre_comp_free(pre);
    return ret;
}

int ossl_ec_GFp_nistp384_have_precompute_mult(const EC_GROUP *group)
{
    return HAVEPRECOMP(group, nistp384);
}
