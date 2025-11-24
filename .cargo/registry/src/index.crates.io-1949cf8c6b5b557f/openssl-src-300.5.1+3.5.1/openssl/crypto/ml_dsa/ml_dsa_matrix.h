/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* A 'k' by 'l' Matrix object ('k' rows and 'l' columns) containing polynomial scalars */
struct matrix_st {
    POLY *m_poly;
    size_t k, l;
};

/**
 * @brief Initialize a Matrix object.
 *
 * @param m The matrix object.
 * @param polys A preallocated array of k * l polynomial blocks. |m| does not
 *              own/free this.
 * @param k The number of rows
 * @param l The number of columns
 */
static ossl_inline ossl_unused void
matrix_init(MATRIX *m, POLY *polys, size_t k, size_t l)
{
    m->k = k;
    m->l = l;
    m->m_poly = polys;
}

static ossl_inline ossl_unused void
matrix_mult_vector(const MATRIX *a, const VECTOR *s, VECTOR *t)
{
    ossl_ml_dsa_matrix_mult_vector(a, s, t);
}

static ossl_inline ossl_unused int
matrix_expand_A(EVP_MD_CTX *g_ctx, const EVP_MD *md, const uint8_t *rho,
                MATRIX *out)
{
    return ossl_ml_dsa_matrix_expand_A(g_ctx, md, rho, out);
}
