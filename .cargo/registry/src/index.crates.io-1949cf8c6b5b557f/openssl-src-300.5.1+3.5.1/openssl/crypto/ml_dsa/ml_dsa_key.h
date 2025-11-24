/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/e_os2.h>
#include "ml_dsa_local.h"
#include "ml_dsa_vector.h"

/* NOTE - any changes to this struct may require updates to ossl_ml_dsa_dup() */
struct ml_dsa_key_st {
    OSSL_LIB_CTX *libctx;
    const ML_DSA_PARAMS *params;

    EVP_MD *shake128_md;
    EVP_MD *shake256_md;

    uint8_t rho[ML_DSA_RHO_BYTES]; /* public random seed */
    uint8_t tr[ML_DSA_TR_BYTES];   /* Pre-cached public key Hash */
    uint8_t K[ML_DSA_K_BYTES];     /* Private random seed for signing */

    /*
     * The encoded public and private keys, these are non NULL if the key
     * components are generated or loaded.
     *
     * For keys that are decoded, but not yet loaded or imported into the
     * provider, the pub_encoding is NULL, while the seed or priv_encoding
     * is not NULL.
     */
    uint8_t *pub_encoding;
    uint8_t *priv_encoding;
    uint8_t *seed;
    int prov_flags;

    /*
     * t1 is the Polynomial encoding of the 10 MSB of each coefficient of the
     * uncompressed public key polynomial t. This is saved as part of the
     * public key. It is column vector of K polynomials.
     * (There are 23 bits in q-modulus.. i.e 10 bits = 23 - 13)
     * t1->poly is allocated.
     */
    VECTOR t1;
    /*
     * t0 is the Polynomial encoding of the 13 LSB of each coefficient of the
     * uncompressed public key polynomial t. This is saved as part of the
     * private key. It is column vector of K polynomials.
     */
    VECTOR t0;
    VECTOR s2; /* private secret of size K with short coefficients (-4..4) or (-2..2) */
    VECTOR s1; /* private secret of size L with short coefficients (-4..4) or (-2..2) */
               /* The s1->poly block is allocated and has space for s2 and t0 also */
};
