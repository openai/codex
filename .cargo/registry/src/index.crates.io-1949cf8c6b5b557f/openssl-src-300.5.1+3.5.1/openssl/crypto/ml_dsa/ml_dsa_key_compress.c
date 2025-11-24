/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "ml_dsa_local.h"

/* Key Compression related functions (Rounding & hints) */

/**
 * @brief Decompose r into (r1, r0) such that r == r1 * 2^13 + r0 mod q
 * See FIPS 204, Algorithm 35, Power2Round()
 *
 * Note: that this code is more complex than the FIPS 204 spec since it keeps
 * r0 as a positive number
 *
 * r mod +- 2^13 is defined as having a range of -4095..4096
 *
 * i.e for r = 0..4096 r1 = 0 and r0 = 0..4096
 * at r = 4097..8191 r1 = 1 and r0 = -4095..0
 * (but since r0 is kept positive it effectively adds q and then reduces by q if needed)
 * Similarly for the range r = 8192..8192+4096 r1=1 and r0=0..4096
 * & 12289..16383 r1=2 and r0=-4095..0
 *
 * @param r is in the range 0..q-1
 * @param r1 The returned top 10 MSB (i.e it ranges from 0..1023)
 * @param r0 The remainder in the range (0..4096 or q-4095..q-1)
 *           So r0 has an effective range of 8192 (i.e. 13 bits).
 */
void ossl_ml_dsa_key_compress_power2_round(uint32_t r, uint32_t *r1, uint32_t *r0)
{
    unsigned int mask;
    uint32_t r0_adjusted, r1_adjusted;

    *r1 = r >> ML_DSA_D_BITS;         /* top 13 bits */
    *r0 = r - (*r1 << ML_DSA_D_BITS); /* The remainder mod q */

    r0_adjusted = mod_sub(*r0, 1 << ML_DSA_D_BITS);
    r1_adjusted = *r1 + 1;

    /* Mask is set iff r0 > (2^(dropped_bits))/2. */
    mask = constant_time_lt((uint32_t)(1 << (ML_DSA_D_BITS - 1)), *r0);
    /* r0 = mask ? r0_adjusted : r0 */
    *r0 = constant_time_select_int(mask, r0_adjusted, *r0);
    /* r1 = mask ? r1_adjusted : r1 */
    *r1 = constant_time_select_int(mask, r1_adjusted, *r1);
}

/*
 * @brief return the r1 component of Decomposing r into (r1, r0) such that
 * r == r1 * (2 * gamma2) + r0 mod q
 * See FIPS 204, Algorithm 37, HighBits()
 *
 * @param r A value to decompose in the range (0..q-1)
 * @param gamma2 Depending on the algorithm gamma2 is either (q-1)/32 or (q-1)/88
 * @returns r1 (The high order bits)
 */
uint32_t ossl_ml_dsa_key_compress_high_bits(uint32_t r, uint32_t gamma2)
{
    int32_t r1 = (r + 127) >> 7;

    if (gamma2 == ML_DSA_GAMMA2_Q_MINUS1_DIV32) {
        r1 = (r1 * 1025 + (1 << 21)) >> 22;
        r1 &= 15; /* mod 16 */
        return r1;
    } else {
        r1 = (r1 * 11275 + (1 << 23)) >> 24;
        r1 ^= ((43 - r1) >> 31) & r1;
        return r1;
    }
}

/**
 * @brief Decomposes r into (r1, r0) such that r == r1 * (2*gamma2) + r0 mod q.
 * See FIPS 204, Algorithm 36, Decompose()
 *
 * @param r A value to decompose in the range (0..q-1)
 * @param gamma2 Depending on the algorithm gamma2 is either (q-1)/32 or (q-1)/88
 * @param r1 The returned high order bits
 * @param r0 The returned low order bits
 */
void ossl_ml_dsa_key_compress_decompose(uint32_t r, uint32_t gamma2,
                                        uint32_t *r1, int32_t *r0)
{
    *r1 = ossl_ml_dsa_key_compress_high_bits(r, gamma2);

    *r0 = r - *r1 * 2 * (int32_t)gamma2;
    *r0 -= (((int32_t)ML_DSA_Q_MINUS1_DIV2 - *r0) >> 31) & (int32_t)ML_DSA_Q;
}

/**
 * @brief return the r0 component of Decomposing r into (r1, r0) such that
 * r == r1 * (2 * gamma2) + r0 mod q
 * See FIPS 204, Algorithm 38, LowBits()
 *
 * @param r A value to decompose in the range (0..q-1)
 * @param gamma2 Depending on the algorithm gamma2 is either (q-1)/32 or (q-1)/88
 * @param r0 The returned low order bits
 */
int32_t ossl_ml_dsa_key_compress_low_bits(uint32_t r, uint32_t gamma2)
{
    uint32_t r1;
    int32_t r0;

    ossl_ml_dsa_key_compress_decompose(r, gamma2, &r1, &r0);
    return r0;
}

/*
 * @brief Computes hint bit indicating whether adding z to r alters the high
 * bits of r
 * See FIPS 204, Algorithm 39, MakeHint().
 *
 * In the spec this takes two arguments, z and r, and is called with
 *   z = -ct0
 *   r = w - cs2 + ct0
 *
 * It then computes HighBits (algorithm 37) of z and z+r.
 * But z + r is just w - cs2, so this takes three arguments and saves an addition.
 *
 * @params ct0 A polynomial c (with coefficients of (-1,0,1)) multiplied by the
 *             polynomial vector t0 (which encodes the least significant bits of each coefficient of the
               uncompressed public-key polynomial t)
 * @params cs2 A polynomial c (with coefficients of (-1,0,1)) multiplied by s2 (a secret polynomial)
 * @params gamma2 Depending on the algorithm gamma2 is either (q-1)/32 or (q-1)/88
 * @params w  (A * y)
 * @returns The hint bit.
 */
int32_t ossl_ml_dsa_key_compress_make_hint(uint32_t ct0, uint32_t cs2,
                                           uint32_t gamma2, uint32_t w)
{
    uint32_t r_plus_z = mod_sub(w, cs2);
    uint32_t r = reduce_once(r_plus_z + ct0);

    return  ossl_ml_dsa_key_compress_high_bits(r, gamma2)
        !=  ossl_ml_dsa_key_compress_high_bits(r_plus_z, gamma2);
}

/*
 * @brief Returns the high bits of |r| adjusted according to hint |h|.
 * FIPS 204, Algorithm 40, UseHint().
 * This is not constant time.
 *
 * @param hint The hint bit which is either 0 or 1
 * @param r A value to decompose in the range (0..q-1)
 * @param gamma2 Depending on the algorithm gamma2 is either (q-1)/32 or (q-1)/88
 *
 * @returns The adjusted high bits or r.
 */
uint32_t ossl_ml_dsa_key_compress_use_hint(uint32_t hint, uint32_t r,
                                           uint32_t gamma2)
{
    uint32_t r1;
    int32_t r0;

    ossl_ml_dsa_key_compress_decompose(r, gamma2, &r1, &r0);

    if (hint == 0)
        return r1;

    if (gamma2 == ((ML_DSA_Q - 1) / 32)) {
        /* m = 16, thus |mod m| in the spec turns into |& 15| */
        return r0 > 0 ? (r1 + 1) & 15 : (r1 - 1) & 15;
    } else {
        /* m = 44 if gamma2 = ((q - 1) / 88) */
        if (r0 > 0)
            return (r1 == 43) ? 0 : r1 + 1;
        else
            return (r1 == 0) ? 43 : r1 - 1;
    }
}
