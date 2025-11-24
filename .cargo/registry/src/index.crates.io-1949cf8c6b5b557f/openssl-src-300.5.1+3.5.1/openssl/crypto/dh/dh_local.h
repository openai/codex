/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/dh.h>
#include "internal/refcount.h"
#include "internal/ffc.h"

#define DH_MIN_MODULUS_BITS     512

struct dh_st {
    /*
     * This first argument is used to pick up errors when a DH is passed
     * instead of a EVP_PKEY
     */
    int pad;
    int version;
    FFC_PARAMS params;
    /* max generated private key length (can be less than len(q)) */
    int32_t length;
    BIGNUM *pub_key;            /* g^x % p */
    BIGNUM *priv_key;           /* x */
    int flags;
    BN_MONT_CTX *method_mont_p;
    CRYPTO_REF_COUNT references;
#ifndef FIPS_MODULE
    CRYPTO_EX_DATA ex_data;
    ENGINE *engine;
#endif
    OSSL_LIB_CTX *libctx;
    const DH_METHOD *meth;
    CRYPTO_RWLOCK *lock;

    /* Provider data */
    size_t dirty_cnt; /* If any key material changes, increment this */
};

struct dh_method {
    char *name;
    /* Methods here */
    int (*generate_key) (DH *dh);
    int (*compute_key) (unsigned char *key, const BIGNUM *pub_key, DH *dh);

    /* Can be null */
    int (*bn_mod_exp) (const DH *dh, BIGNUM *r, const BIGNUM *a,
                       const BIGNUM *p, const BIGNUM *m, BN_CTX *ctx,
                       BN_MONT_CTX *m_ctx);
    int (*init) (DH *dh);
    int (*finish) (DH *dh);
    int flags;
    char *app_data;
    /* If this is non-NULL, it will be used to generate parameters */
    int (*generate_params) (DH *dh, int prime_len, int generator,
                            BN_GENCB *cb);
};
