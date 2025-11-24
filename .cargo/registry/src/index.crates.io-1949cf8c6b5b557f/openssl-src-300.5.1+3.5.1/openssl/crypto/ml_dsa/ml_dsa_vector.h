/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include "ml_dsa_poly.h"

struct vector_st {
    POLY *poly;
    size_t num_poly;
};

/**
 * @brief Initialize a Vector object.
 *
 * @param v The vector to initialize.
 * @param polys Preallocated storage for an array of Polynomials blocks. |v|
 *              does not own/free this.
 * @param num_polys The number of |polys| blocks (k or l)
 */
static ossl_inline ossl_unused
void vector_init(VECTOR *v, POLY *polys, size_t num_polys)
{
    v->poly = polys;
    v->num_poly = num_polys;
}

static ossl_inline ossl_unused
int vector_alloc(VECTOR *v, size_t num_polys)
{
    v->poly = OPENSSL_malloc(num_polys * sizeof(POLY));
    if (v->poly == NULL)
        return 0;
    v->num_poly = num_polys;
    return 1;
}

static ossl_inline ossl_unused
void vector_free(VECTOR *v)
{
    OPENSSL_free(v->poly);
    v->poly = NULL;
    v->num_poly = 0;
}

/* @brief zeroize a vectors polynomial coefficients */
static ossl_inline ossl_unused
void vector_zero(VECTOR *va)
{
    if (va->poly != NULL)
        memset(va->poly, 0, va->num_poly * sizeof(va->poly[0]));
}

/*
 * @brief copy a vector
 * The assumption is that |dst| has already been initialized
 */
static ossl_inline ossl_unused void
vector_copy(VECTOR *dst, const VECTOR *src)
{
    assert(dst->num_poly == src->num_poly);
    memcpy(dst->poly, src->poly, src->num_poly * sizeof(src->poly[0]));
}

/* @brief return 1 if 2 vectors are equal, or 0 otherwise */
static ossl_inline ossl_unused int
vector_equal(const VECTOR *a, const VECTOR *b)
{
    size_t i;

    if (a->num_poly != b->num_poly)
        return 0;
    for (i = 0; i < a->num_poly; ++i) {
        if (!poly_equal(a->poly + i, b->poly + i))
            return 0;
    }
    return 1;
}

/* @brief add 2 vectors */
static ossl_inline ossl_unused void
vector_add(const VECTOR *lhs, const VECTOR *rhs, VECTOR *out)
{
    size_t i;

    for (i = 0; i < lhs->num_poly; i++)
        poly_add(lhs->poly + i, rhs->poly + i, out->poly + i);
}

/* @brief subtract 2 vectors */
static ossl_inline ossl_unused void
vector_sub(const VECTOR *lhs, const VECTOR *rhs, VECTOR *out)
{
    size_t i;

    for (i = 0; i < lhs->num_poly; i++)
        poly_sub(lhs->poly + i, rhs->poly + i, out->poly + i);
}

/* @brief convert a vector in place into NTT form */
static ossl_inline ossl_unused void
vector_ntt(VECTOR *va)
{
    size_t i;

    for (i = 0; i < va->num_poly; i++)
        ossl_ml_dsa_poly_ntt(va->poly + i);
}

/* @brief convert a vector in place into inverse NTT form */
static ossl_inline ossl_unused void
vector_ntt_inverse(VECTOR *va)
{
    size_t i;

    for (i = 0; i < va->num_poly; i++)
        ossl_ml_dsa_poly_ntt_inverse(va->poly + i);
}

/* @brief multiply a vector by a SCALAR polynomial */
static ossl_inline ossl_unused void
vector_mult_scalar(const VECTOR *lhs, const POLY *rhs, VECTOR *out)
{
    size_t i;

    for (i = 0; i < lhs->num_poly; i++)
        ossl_ml_dsa_poly_ntt_mult(lhs->poly + i, rhs, out->poly + i);
}

static ossl_inline ossl_unused int
vector_expand_S(EVP_MD_CTX *h_ctx, const EVP_MD *md, int eta,
                const uint8_t *seed, VECTOR *s1, VECTOR *s2)
{
    return ossl_ml_dsa_vector_expand_S(h_ctx, md, eta, seed, s1, s2);
}

static ossl_inline ossl_unused void
vector_expand_mask(VECTOR *out, const uint8_t *rho_prime, size_t rho_prime_len,
                   uint32_t kappa, uint32_t gamma1,
                   EVP_MD_CTX *h_ctx, const EVP_MD *md)
{
    size_t i;
    uint8_t derived_seed[ML_DSA_RHO_PRIME_BYTES + 2];

    memcpy(derived_seed, rho_prime, ML_DSA_RHO_PRIME_BYTES);

    for (i = 0; i < out->num_poly; i++) {
        size_t index = kappa + i;

        derived_seed[ML_DSA_RHO_PRIME_BYTES] = index & 0xFF;
        derived_seed[ML_DSA_RHO_PRIME_BYTES + 1] = (index >> 8) & 0xFF;
        poly_expand_mask(out->poly + i, derived_seed, sizeof(derived_seed),
                         gamma1, h_ctx, md);
    }
}

/* Scale back previously rounded value */
static ossl_inline ossl_unused void
vector_scale_power2_round_ntt(const VECTOR *in, VECTOR *out)
{
    size_t i;

    for (i = 0; i < in->num_poly; i++)
        poly_scale_power2_round(in->poly + i, out->poly + i);
    vector_ntt(out);
}

/*
 * @brief Decompose all polynomial coefficients of a vector into (t1, t0) such
 * that coeff[i] == t1[i] * 2^13 + t0[i] mod q.
 * See FIPS 204, Algorithm 35, Power2Round()
 */
static ossl_inline ossl_unused void
vector_power2_round(const VECTOR *t, VECTOR *t1, VECTOR *t0)
{
    size_t i;

    for (i = 0; i < t->num_poly; i++)
        poly_power2_round(t->poly + i, t1->poly + i, t0->poly + i);
}

static ossl_inline ossl_unused void
vector_high_bits(const VECTOR *in, uint32_t gamma2, VECTOR *out)
{
    size_t i;

    for (i = 0; i < out->num_poly; i++)
        poly_high_bits(in->poly + i, gamma2, out->poly + i);
}

static ossl_inline ossl_unused void
vector_low_bits(const VECTOR *in, uint32_t gamma2, VECTOR *out)
{
    size_t i;

    for (i = 0; i < out->num_poly; i++)
        poly_low_bits(in->poly + i, gamma2, out->poly + i);
}

static ossl_inline ossl_unused uint32_t
vector_max(const VECTOR *v)
{
    size_t i;
    uint32_t mx = 0;

    for (i = 0; i < v->num_poly; i++)
        poly_max(v->poly + i, &mx);
    return mx;
}

static ossl_inline ossl_unused uint32_t
vector_max_signed(const VECTOR *v)
{
    size_t i;
    uint32_t mx = 0;

    for (i = 0; i < v->num_poly; i++)
        poly_max_signed(v->poly + i, &mx);
    return mx;
}

static ossl_inline ossl_unused size_t
vector_count_ones(const VECTOR *v)
{
    int j;
    size_t i, count = 0;

    for (i = 0; i < v->num_poly; i++)
        for (j = 0; j < ML_DSA_NUM_POLY_COEFFICIENTS; j++)
            count += v->poly[i].coeff[j];
    return count;
}

static ossl_inline ossl_unused void
vector_make_hint(const VECTOR *ct0, const VECTOR *cs2, const VECTOR *w,
                 uint32_t gamma2, VECTOR *out)
{
    size_t i;

    for (i = 0; i < out->num_poly; i++)
        poly_make_hint(ct0->poly + i, cs2->poly + i, w->poly + i, gamma2,
                       out->poly + i);
}

static ossl_inline ossl_unused void
vector_use_hint(const VECTOR *h, const VECTOR *r, uint32_t gamma2, VECTOR *out)
{
    size_t i;

    for (i = 0; i < out->num_poly; i++)
        poly_use_hint(h->poly + i, r->poly + i, gamma2, out->poly + i);
}
