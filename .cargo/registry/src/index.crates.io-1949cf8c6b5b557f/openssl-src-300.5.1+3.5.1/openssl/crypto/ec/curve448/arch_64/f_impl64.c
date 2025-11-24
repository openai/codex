/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2014 Cryptography Research, Inc.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Mike Hamburg
 */

#include "internal/e_os.h"
#include <openssl/macros.h>
#include "internal/numbers.h"

#ifndef UINT128_MAX
/* No support for 128 bit ints, so do nothing here */
NON_EMPTY_TRANSLATION_UNIT
#else

# include "../field.h"

void ossl_gf_mul(gf_s * RESTRICT cs, const gf as, const gf bs)
{
    const uint64_t *a = as->limb, *b = bs->limb;
    uint64_t *c = cs->limb;
    uint128_t accum0 = 0, accum1 = 0, accum2;
    uint64_t mask = (1ULL << 56) - 1;
    uint64_t aa[4], bb[4], bbb[4];
    unsigned int i, j;

    for (i = 0; i < 4; i++) {
        aa[i] = a[i] + a[i + 4];
        bb[i] = b[i] + b[i + 4];
        bbb[i] = bb[i] + b[i + 4];
    }

    for (i = 0; i < 4; i++) {
        accum2 = 0;

        for (j = 0; j <= i; j++) {
            accum2 += widemul(a[j], b[i - j]);
            accum1 += widemul(aa[j], bb[i - j]);
            accum0 += widemul(a[j + 4], b[i - j + 4]);
        }
        for (; j < 4; j++) {
            accum2 += widemul(a[j], b[i + 8 - j]);
            accum1 += widemul(aa[j], bbb[i + 4 - j]);
            accum0 += widemul(a[j + 4], bb[i + 4 - j]);
        }

        accum1 -= accum2;
        accum0 += accum2;

        c[i] = ((uint64_t)(accum0)) & mask;
        c[i + 4] = ((uint64_t)(accum1)) & mask;

        accum0 >>= 56;
        accum1 >>= 56;
    }

    accum0 += accum1;
    accum0 += c[4];
    accum1 += c[0];
    c[4] = ((uint64_t)(accum0)) & mask;
    c[0] = ((uint64_t)(accum1)) & mask;

    accum0 >>= 56;
    accum1 >>= 56;

    c[5] += ((uint64_t)(accum0));
    c[1] += ((uint64_t)(accum1));
}

void ossl_gf_mulw_unsigned(gf_s * RESTRICT cs, const gf as, uint32_t b)
{
    const uint64_t *a = as->limb;
    uint64_t *c = cs->limb;
    uint128_t accum0 = 0, accum4 = 0;
    uint64_t mask = (1ULL << 56) - 1;
    int i;

    for (i = 0; i < 4; i++) {
        accum0 += widemul(b, a[i]);
        accum4 += widemul(b, a[i + 4]);
        c[i] = accum0 & mask;
        accum0 >>= 56;
        c[i + 4] = accum4 & mask;
        accum4 >>= 56;
    }

    accum0 += accum4 + c[4];
    c[4] = accum0 & mask;
    c[5] += accum0 >> 56;

    accum4 += c[0];
    c[0] = accum4 & mask;
    c[1] += accum4 >> 56;
}

void ossl_gf_sqr(gf_s * RESTRICT cs, const gf as)
{
    const uint64_t *a = as->limb;
    uint64_t *c = cs->limb;
    uint128_t accum0 = 0, accum1 = 0, accum2;
    uint64_t mask = (1ULL << 56) - 1;
    uint64_t aa[4];
    unsigned int i;

    /* For some reason clang doesn't vectorize this without prompting? */
    for (i = 0; i < 4; i++)
        aa[i] = a[i] + a[i + 4];

    accum2 = widemul(a[0], a[3]);
    accum0 = widemul(aa[0], aa[3]);
    accum1 = widemul(a[4], a[7]);

    accum2 += widemul(a[1], a[2]);
    accum0 += widemul(aa[1], aa[2]);
    accum1 += widemul(a[5], a[6]);

    accum0 -= accum2;
    accum1 += accum2;

    c[3] = ((uint64_t)(accum1)) << 1 & mask;
    c[7] = ((uint64_t)(accum0)) << 1 & mask;

    accum0 >>= 55;
    accum1 >>= 55;

    accum0 += widemul(2 * aa[1], aa[3]);
    accum1 += widemul(2 * a[5], a[7]);
    accum0 += widemul(aa[2], aa[2]);
    accum1 += accum0;

    accum0 -= widemul(2 * a[1], a[3]);
    accum1 += widemul(a[6], a[6]);

    accum2 = widemul(a[0], a[0]);
    accum1 -= accum2;
    accum0 += accum2;

    accum0 -= widemul(a[2], a[2]);
    accum1 += widemul(aa[0], aa[0]);
    accum0 += widemul(a[4], a[4]);

    c[0] = ((uint64_t)(accum0)) & mask;
    c[4] = ((uint64_t)(accum1)) & mask;

    accum0 >>= 56;
    accum1 >>= 56;

    accum2 = widemul(2 * aa[2], aa[3]);
    accum0 -= widemul(2 * a[2], a[3]);
    accum1 += widemul(2 * a[6], a[7]);

    accum1 += accum2;
    accum0 += accum2;

    accum2 = widemul(2 * a[0], a[1]);
    accum1 += widemul(2 * aa[0], aa[1]);
    accum0 += widemul(2 * a[4], a[5]);

    accum1 -= accum2;
    accum0 += accum2;

    c[1] = ((uint64_t)(accum0)) & mask;
    c[5] = ((uint64_t)(accum1)) & mask;

    accum0 >>= 56;
    accum1 >>= 56;

    accum2 = widemul(aa[3], aa[3]);
    accum0 -= widemul(a[3], a[3]);
    accum1 += widemul(a[7], a[7]);

    accum1 += accum2;
    accum0 += accum2;

    accum2 = widemul(2 * a[0], a[2]);
    accum1 += widemul(2 * aa[0], aa[2]);
    accum0 += widemul(2 * a[4], a[6]);

    accum2 += widemul(a[1], a[1]);
    accum1 += widemul(aa[1], aa[1]);
    accum0 += widemul(a[5], a[5]);

    accum1 -= accum2;
    accum0 += accum2;

    c[2] = ((uint64_t)(accum0)) & mask;
    c[6] = ((uint64_t)(accum1)) & mask;

    accum0 >>= 56;
    accum1 >>= 56;

    accum0 += c[3];
    accum1 += c[7];
    c[3] = ((uint64_t)(accum0)) & mask;
    c[7] = ((uint64_t)(accum1)) & mask;

    /* we could almost stop here, but it wouldn't be stable, so... */

    accum0 >>= 56;
    accum1 >>= 56;
    c[4] += ((uint64_t)(accum0)) + ((uint64_t)(accum1));
    c[0] += ((uint64_t)(accum1));
}
#endif
