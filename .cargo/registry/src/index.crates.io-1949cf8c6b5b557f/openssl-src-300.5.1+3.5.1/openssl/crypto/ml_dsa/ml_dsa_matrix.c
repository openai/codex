/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "ml_dsa_local.h"
#include "ml_dsa_vector.h"
#include "ml_dsa_matrix.h"

/*
 * Matrix multiply of a k*l matrix of polynomials by a 1 * l vector of
 * polynomials to produce a 1 * k vector of polynomial results.
 * i.e. t = a * s
 *
 * @param a A k * l matrix of polynomials in NTT form
 * @param s A 1 * l vector of polynomials in NTT form
 * @param t 1 * k vector of polynomial results in NTT form
 */
void ossl_ml_dsa_matrix_mult_vector(const MATRIX *a, const VECTOR *s,
                                    VECTOR *t)
{
    size_t i, j;
    POLY *poly = a->m_poly;

    vector_zero(t);

    for (i = 0; i < a->k; i++) {
        for (j = 0; j < a->l; j++) {
            POLY product;

            ossl_ml_dsa_poly_ntt_mult(poly++, &s->poly[j], &product);
            poly_add(&product, &t->poly[i], &t->poly[i]);
        }
    }
}
