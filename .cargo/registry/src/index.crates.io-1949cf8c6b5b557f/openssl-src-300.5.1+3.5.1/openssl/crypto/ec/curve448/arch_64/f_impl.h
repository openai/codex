/*
 * Copyright 2017-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2014-2016 Cryptography Research, Inc.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Mike Hamburg
 */

#ifndef OSSL_CRYPTO_EC_CURVE448_ARCH_64_F_IMPL_H
# define OSSL_CRYPTO_EC_CURVE448_ARCH_64_F_IMPL_H

# define GF_HEADROOM 9999        /* Everything is reduced anyway */
# define FIELD_LITERAL(a,b,c,d,e,f,g,h) {{a,b,c,d,e,f,g,h}}

# define LIMB_PLACE_VALUE(i) 56

void gf_add_RAW(gf out, const gf a, const gf b)
{
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out->limb[i] = a->limb[i] + b->limb[i];

    gf_weak_reduce(out);
}

void gf_sub_RAW(gf out, const gf a, const gf b)
{
    uint64_t co1 = ((1ULL << 56) - 1) * 2, co2 = co1 - 2;
    unsigned int i;

    for (i = 0; i < NLIMBS; i++)
        out->limb[i] = a->limb[i] - b->limb[i] + ((i == NLIMBS / 2) ? co2 : co1);

    gf_weak_reduce(out);
}

void gf_bias(gf a, int amt)
{
}

void gf_weak_reduce(gf a)
{
    uint64_t mask = (1ULL << 56) - 1;
    uint64_t tmp = a->limb[NLIMBS - 1] >> 56;
    unsigned int i;

    a->limb[NLIMBS / 2] += tmp;
    for (i = NLIMBS - 1; i > 0; i--)
        a->limb[i] = (a->limb[i] & mask) + (a->limb[i - 1] >> 56);
    a->limb[0] = (a->limb[0] & mask) + tmp;
}

#endif                  /* OSSL_CRYPTO_EC_CURVE448_ARCH_64_F_IMPL_H */
