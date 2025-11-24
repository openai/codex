/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/configuration.h>
#include <openssl/thread.h>
#include <internal/thread.h>

uint32_t OSSL_get_thread_support_flags(void)
{
    int support = 0;

#if !defined(OPENSSL_NO_THREAD_POOL)
    support |= OSSL_THREAD_SUPPORT_FLAG_THREAD_POOL;
#endif
#if !defined(OPENSSL_NO_DEFAULT_THREAD_POOL)
    support |= OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN;
#endif

    return support;
}

#if defined(OPENSSL_NO_THREAD_POOL) || defined(OPENSSL_NO_DEFAULT_THREAD_POOL)

int OSSL_set_max_threads(OSSL_LIB_CTX *ctx, uint64_t max_threads)
{
    return 0;
}

uint64_t OSSL_get_max_threads(OSSL_LIB_CTX *ctx)
{
    return 0;
}

#else

uint64_t OSSL_get_max_threads(OSSL_LIB_CTX *ctx)
{
    uint64_t ret = 0;
    OSSL_LIB_CTX_THREADS *tdata = OSSL_LIB_CTX_GET_THREADS(ctx);

    if (tdata == NULL)
        goto fail;

    ossl_crypto_mutex_lock(tdata->lock);
    ret = tdata->max_threads;
    ossl_crypto_mutex_unlock(tdata->lock);

fail:
    return ret;
}

int OSSL_set_max_threads(OSSL_LIB_CTX *ctx, uint64_t max_threads)
{
    OSSL_LIB_CTX_THREADS *tdata;

    tdata = OSSL_LIB_CTX_GET_THREADS(ctx);
    if (tdata == NULL)
        return 0;

    ossl_crypto_mutex_lock(tdata->lock);
    tdata->max_threads = max_threads;
    ossl_crypto_mutex_unlock(tdata->lock);

    return 1;
}

#endif
