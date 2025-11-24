/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/e_os2.h>

#define SLH_DSA_MAX_N 32
#define SLH_DSA_SK_SEED(key) ((key)->priv)
#define SLH_DSA_SK_PRF(key)  ((key)->priv + (key)->params->n)
#define SLH_DSA_PK_SEED(key) ((key)->priv + (key)->params->n * 2)
#define SLH_DSA_PK_ROOT(key) ((key)->priv + (key)->params->n * 3)
#define SLH_DSA_PUB(key) SLH_DSA_PK_SEED(key)
#define SLH_DSA_PRIV(key) SLH_DSA_SK_SEED(key)

/*
 * NOTE: Any changes to this structure may require updating ossl_slh_dsa_key_dup().
 */
struct slh_dsa_key_st {
    /*
     * A private key consists of
     *  Private SEED and PRF values of size |n|
     *  Public SEED and ROOT values of size |n|
     *  (Unlike X25519 the public key is not (fully) constructed from the
     *  private key so when encoded the private key must contain the public key)
     */
    uint8_t priv[4 * SLH_DSA_MAX_N];
    /*
     * pub will be NULL initially.
     * When either a private or public key is loaded it will then point
     * to &priv[n * 2]
     */
    uint8_t *pub;
    OSSL_LIB_CTX *libctx;
    char *propq;
    int has_priv; /* Set to 1 if there is a private key component */

    const SLH_DSA_PARAMS *params;
    const SLH_ADRS_FUNC *adrs_func;
    const SLH_HASH_FUNC *hash_func;
    /* See FIPS 205 Section 11.1 */

    EVP_MD *md; /* Used for SHAKE and SHA-256 */
    EVP_MD *md_big; /* Used for SHA-256 or SHA-512 */
    EVP_MAC *hmac;
};
