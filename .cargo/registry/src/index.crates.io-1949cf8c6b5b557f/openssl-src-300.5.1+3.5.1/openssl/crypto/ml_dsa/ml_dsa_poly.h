/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/crypto.h>

#define ML_DSA_NUM_POLY_COEFFICIENTS 256

/* Polynomial object with 256 coefficients. The coefficients are unsigned 32 bits */
struct poly_st {
    uint32_t coeff[ML_DSA_NUM_POLY_COEFFICIENTS];
};

static ossl_inline ossl_unused void
poly_zero(POLY *p)
{
    memset(p->coeff, 0, sizeof(*p));
}

/**
 * @brief Polynomial addition.
 *
 * @param lhs A polynomial with coefficients in the range (0..q-1)
 * @param rhs A polynomial with coefficients in the range (0..q-1) to add
 *            to the 'lhs'.
 * @param out The returned addition result with the coefficients all in the
 *            range 0..q-1
 */
static ossl_inline ossl_unused void
poly_add(const POLY *lhs, const POLY *rhs, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = reduce_once(lhs->coeff[i] + rhs->coeff[i]);
}

/**
 * @brief Polynomial subtraction.
 *
 * @param lhs A polynomial with coefficients in the range (0..q-1)
 * @param rhs A polynomial with coefficients in the range (0..q-1) to subtract
 *            from the 'lhs'.
 * @param out The returned subtraction result with the coefficients all in the
 *            range 0..q-1
 */
static ossl_inline ossl_unused void
poly_sub(const POLY *lhs, const POLY *rhs, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = mod_sub(lhs->coeff[i], rhs->coeff[i]);
}

/* @returns 1 if the polynomials are equal, or 0 otherwise */
static ossl_inline ossl_unused int
poly_equal(const POLY *a, const POLY *b)
{
    return CRYPTO_memcmp(a, b, sizeof(*a)) == 0;
}

static ossl_inline ossl_unused void
poly_ntt(POLY *p)
{
    ossl_ml_dsa_poly_ntt(p);
}

static ossl_inline ossl_unused int
poly_sample_in_ball_ntt(POLY *out, const uint8_t *seed, int seed_len,
                        EVP_MD_CTX *h_ctx, const EVP_MD *md, uint32_t tau)
{
    if (!ossl_ml_dsa_poly_sample_in_ball(out, seed, seed_len, h_ctx, md, tau))
        return 0;
    poly_ntt(out);
    return 1;
}

static ossl_inline ossl_unused int
poly_expand_mask(POLY *out, const uint8_t *seed, size_t seed_len,
                 uint32_t gamma1, EVP_MD_CTX *h_ctx, const EVP_MD *md)
{
    return ossl_ml_dsa_poly_expand_mask(out, seed, seed_len, gamma1, h_ctx, md);
}

/**
 * @brief Decompose the coefficients of a polynomial into (r1, r0) such that
 * coeff[i] == t1[i] * 2^13 + t0[i] mod q
 * See FIPS 204, Algorithm 35, Power2Round()
 *
 * @param t A polynomial containing coefficients in the range 0..q-1
 * @param t1 The returned polynomial containing coefficients that represent
 *           the top 10 MSB of each coefficient in t (i.e each ranging from 0..1023)
 * @param t0 The remainder coefficients of t in the range (0..4096 or q-4095..q-1)
 *           Each t0 coefficient has an effective range of 8192 (i.e. 13 bits).
 */
static ossl_inline ossl_unused void
poly_power2_round(const POLY *t, POLY *t1, POLY *t0)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        ossl_ml_dsa_key_compress_power2_round(t->coeff[i],
                                              t1->coeff + i, t0->coeff + i);
}

static ossl_inline ossl_unused void
poly_scale_power2_round(POLY *in, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = (in->coeff[i] << ML_DSA_D_BITS);
}

static ossl_inline ossl_unused void
poly_high_bits(const POLY *in, uint32_t gamma2, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = ossl_ml_dsa_key_compress_high_bits(in->coeff[i], gamma2);
}

static ossl_inline ossl_unused void
poly_low_bits(const POLY *in, uint32_t gamma2, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = ossl_ml_dsa_key_compress_low_bits(in->coeff[i], gamma2);
}

static ossl_inline ossl_unused void
poly_make_hint(const POLY *ct0, const POLY *cs2, const POLY *w, uint32_t gamma2,
               POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = ossl_ml_dsa_key_compress_make_hint(ct0->coeff[i],
                                                           cs2->coeff[i],
                                                           gamma2, w->coeff[i]);
}

static ossl_inline ossl_unused void
poly_use_hint(const POLY *h, const POLY *r, uint32_t gamma2, POLY *out)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++)
        out->coeff[i] = ossl_ml_dsa_key_compress_use_hint(h->coeff[i],
                                                          r->coeff[i], gamma2);
}

static ossl_inline ossl_unused void
poly_max(const POLY *p, uint32_t *mx)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++) {
        uint32_t c = p->coeff[i];
        uint32_t abs = abs_mod_prime(c);

        *mx = maximum(*mx, abs);
    }
}

static ossl_inline ossl_unused void
poly_max_signed(const POLY *p, uint32_t *mx)
{
    int i;

    for (i = 0; i < ML_DSA_NUM_POLY_COEFFICIENTS; i++) {
        uint32_t c = p->coeff[i];
        uint32_t abs = abs_signed(c);

        *mx = maximum(*mx, abs);
    }
}
