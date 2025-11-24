/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "rand_local.h"
#include "crypto/evp.h"
#include "crypto/rand.h"
#include "crypto/rand_pool.h"
#include "internal/core.h"
#include <openssl/core_dispatch.h>
#include <openssl/err.h>

size_t ossl_rand_get_entropy(ossl_unused OSSL_LIB_CTX *ctx,
                             unsigned char **pout, int entropy,
                             size_t min_len, size_t max_len)
{
    size_t ret = 0;
    size_t entropy_available;
    RAND_POOL *pool;

    pool = ossl_rand_pool_new(entropy, 1, min_len, max_len);
    if (pool == NULL) {
        ERR_raise(ERR_LIB_RAND, ERR_R_RAND_LIB);
        return 0;
    }

    /* Get entropy by polling system entropy sources. */
    entropy_available = ossl_pool_acquire_entropy(pool);

    if (entropy_available > 0) {
        ret   = ossl_rand_pool_length(pool);
        *pout = ossl_rand_pool_detach(pool);
    }

    ossl_rand_pool_free(pool);
    return ret;
}

size_t ossl_rand_get_user_entropy(OSSL_LIB_CTX *ctx,
                                  unsigned char **pout, int entropy,
                                  size_t min_len, size_t max_len)
{
    EVP_RAND_CTX *rng = ossl_rand_get0_seed_noncreating(ctx);

    if (rng != NULL && evp_rand_can_seed(rng))
        return evp_rand_get_seed(rng, pout, entropy, min_len, max_len,
                                 0, NULL, 0);
    else
        return ossl_rand_get_entropy(ctx, pout, entropy, min_len, max_len);
}

void ossl_rand_cleanup_entropy(ossl_unused OSSL_LIB_CTX *ctx,
                               unsigned char *buf, size_t len)
{
    OPENSSL_secure_clear_free(buf, len);
}

void ossl_rand_cleanup_user_entropy(OSSL_LIB_CTX *ctx,
                                    unsigned char *buf, size_t len)
{
    EVP_RAND_CTX *rng = ossl_rand_get0_seed_noncreating(ctx);

    if (rng != NULL && evp_rand_can_seed(rng))
        evp_rand_clear_seed(rng, buf, len);
    else
        OPENSSL_secure_clear_free(buf, len);
}

size_t ossl_rand_get_nonce(ossl_unused OSSL_LIB_CTX *ctx,
                           unsigned char **pout,
                           size_t min_len, ossl_unused size_t max_len,
                           const void *salt, size_t salt_len)
{
    size_t ret = 0;
    RAND_POOL *pool;

    pool = ossl_rand_pool_new(0, 0, min_len, max_len);
    if (pool == NULL) {
        ERR_raise(ERR_LIB_RAND, ERR_R_RAND_LIB);
        return 0;
    }

    if (!ossl_pool_add_nonce_data(pool))
        goto err;

    if (salt != NULL && !ossl_rand_pool_add(pool, salt, salt_len, 0))
        goto err;
    ret   = ossl_rand_pool_length(pool);
    *pout = ossl_rand_pool_detach(pool);
 err:
    ossl_rand_pool_free(pool);
    return ret;
}

size_t ossl_rand_get_user_nonce(OSSL_LIB_CTX *ctx,
                                unsigned char **pout,
                                size_t min_len, size_t max_len,
                                const void *salt, size_t salt_len)
{
    unsigned char *buf;
    EVP_RAND_CTX *rng = ossl_rand_get0_seed_noncreating(ctx);

    if (rng == NULL)
        return ossl_rand_get_nonce(ctx, pout, min_len, max_len, salt, salt_len);

    if ((buf = OPENSSL_malloc(min_len)) == NULL)
        return 0;

    if (!EVP_RAND_generate(rng, buf, min_len, 0, 0, salt, salt_len)) {
        OPENSSL_free(buf);
        return 0;
    }
    *pout = buf;
    return min_len;
}

void ossl_rand_cleanup_nonce(ossl_unused OSSL_LIB_CTX *ctx,
                             unsigned char *buf, size_t len)
{
    OPENSSL_clear_free(buf, len);
}

void ossl_rand_cleanup_user_nonce(ossl_unused OSSL_LIB_CTX *ctx,
                                  unsigned char *buf, size_t len)
{
    OPENSSL_clear_free(buf, len);
}
