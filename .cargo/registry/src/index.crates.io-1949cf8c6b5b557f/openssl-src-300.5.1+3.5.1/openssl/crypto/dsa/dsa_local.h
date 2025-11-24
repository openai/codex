/*
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/dsa.h>
#include "internal/refcount.h"
#include "internal/ffc.h"

struct dsa_st {
    /*
     * This first variable is used to pick up errors where a DSA is passed
     * instead of an EVP_PKEY
     */
    int pad;
    int32_t version;
    FFC_PARAMS params;
    BIGNUM *pub_key;            /* y public key */
    BIGNUM *priv_key;           /* x private key */
    int flags;
    /* Normally used to cache montgomery values */
    BN_MONT_CTX *method_mont_p;
    CRYPTO_REF_COUNT references;
#ifndef FIPS_MODULE
    CRYPTO_EX_DATA ex_data;
#endif
    const DSA_METHOD *meth;
    /* functional reference if 'meth' is ENGINE-provided */
    ENGINE *engine;
    CRYPTO_RWLOCK *lock;
    OSSL_LIB_CTX *libctx;

    /* Provider data */
    size_t dirty_cnt; /* If any key material changes, increment this */
};

struct DSA_SIG_st {
    BIGNUM *r;
    BIGNUM *s;
};

struct dsa_method {
    char *name;
    DSA_SIG *(*dsa_do_sign) (const unsigned char *dgst, int dlen, DSA *dsa);
    int (*dsa_sign_setup) (DSA *dsa, BN_CTX *ctx_in, BIGNUM **kinvp,
                           BIGNUM **rp);
    int (*dsa_do_verify) (const unsigned char *dgst, int dgst_len,
                          DSA_SIG *sig, DSA *dsa);
    int (*dsa_mod_exp) (DSA *dsa, BIGNUM *rr, const BIGNUM *a1,
                        const BIGNUM *p1, const BIGNUM *a2, const BIGNUM *p2,
                        const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *in_mont);
    /* Can be null */
    int (*bn_mod_exp) (DSA *dsa, BIGNUM *r, const BIGNUM *a, const BIGNUM *p,
                       const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx);
    int (*init) (DSA *dsa);
    int (*finish) (DSA *dsa);
    int flags;
    void *app_data;
    /* If this is non-NULL, it is used to generate DSA parameters */
    int (*dsa_paramgen) (DSA *dsa, int bits,
                         const unsigned char *seed, int seed_len,
                         int *counter_ret, unsigned long *h_ret,
                         BN_GENCB *cb);
    /* If this is non-NULL, it is used to generate DSA keys */
    int (*dsa_keygen) (DSA *dsa);
};

DSA_SIG *ossl_dsa_do_sign_int(const unsigned char *dgst, int dlen, DSA *dsa,
                              unsigned int nonce_type, const char *digestname,
                              OSSL_LIB_CTX *libctx, const char *propq);
