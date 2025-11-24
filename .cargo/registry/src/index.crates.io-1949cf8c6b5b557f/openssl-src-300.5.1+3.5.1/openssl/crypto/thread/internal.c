/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/configuration.h>
#include <openssl/e_os2.h>
#include <openssl/types.h>
#include <openssl/crypto.h>
#include <internal/thread.h>
#include <internal/thread_arch.h>

#if !defined(OPENSSL_NO_DEFAULT_THREAD_POOL)

static ossl_inline uint64_t _ossl_get_avail_threads(OSSL_LIB_CTX_THREADS *tdata)
{
    /* assumes that tdata->lock is taken */
    return tdata->max_threads - tdata->active_threads;
}

uint64_t ossl_get_avail_threads(OSSL_LIB_CTX *ctx)
{
    uint64_t retval = 0;
    OSSL_LIB_CTX_THREADS *tdata = OSSL_LIB_CTX_GET_THREADS(ctx);

    if (tdata == NULL)
        return retval;

    ossl_crypto_mutex_lock(tdata->lock);
    retval = _ossl_get_avail_threads(tdata);
    ossl_crypto_mutex_unlock(tdata->lock);

    return retval;
}

void *ossl_crypto_thread_start(OSSL_LIB_CTX *ctx, CRYPTO_THREAD_ROUTINE start,
                               void *data)
{
    CRYPTO_THREAD *thread;
    OSSL_LIB_CTX_THREADS *tdata = OSSL_LIB_CTX_GET_THREADS(ctx);

    if (tdata == NULL)
        return NULL;

    ossl_crypto_mutex_lock(tdata->lock);
    if (tdata == NULL || tdata->max_threads == 0) {
        ossl_crypto_mutex_unlock(tdata->lock);
        return NULL;
    }

    while (_ossl_get_avail_threads(tdata) == 0)
        ossl_crypto_condvar_wait(tdata->cond_finished, tdata->lock);
    tdata->active_threads++;
    ossl_crypto_mutex_unlock(tdata->lock);

    thread = ossl_crypto_thread_native_start(start, data, 1);
    if (thread == NULL) {
        ossl_crypto_mutex_lock(tdata->lock);
        tdata->active_threads--;
        ossl_crypto_mutex_unlock(tdata->lock);
        goto fail;
    }
    thread->ctx = ctx;

fail:
    return (void *) thread;
}

int ossl_crypto_thread_join(void *vhandle, CRYPTO_THREAD_RETVAL *retval)
{
    CRYPTO_THREAD *handle = vhandle;
    OSSL_LIB_CTX_THREADS *tdata;

    if (vhandle == NULL)
        return 0;

    tdata = OSSL_LIB_CTX_GET_THREADS(handle->ctx);
    if (tdata == NULL)
        return 0;

    if (ossl_crypto_thread_native_join(handle, retval) == 0)
        return 0;

    ossl_crypto_mutex_lock(tdata->lock);
    tdata->active_threads--;
    ossl_crypto_condvar_signal(tdata->cond_finished);
    ossl_crypto_mutex_unlock(tdata->lock);
    return 1;
}

int ossl_crypto_thread_clean(void *vhandle)
{
    CRYPTO_THREAD *handle = vhandle;

    return ossl_crypto_thread_native_clean(handle);
}

#else

ossl_inline uint64_t ossl_get_avail_threads(OSSL_LIB_CTX *ctx)
{
    return 0;
}

void *ossl_crypto_thread_start(OSSL_LIB_CTX *ctx, CRYPTO_THREAD_ROUTINE start,
                               void *data)
{
    return NULL;
}

int ossl_crypto_thread_join(void *vhandle, CRYPTO_THREAD_RETVAL *retval)
{
    return 0;
}

int ossl_crypto_thread_clean(void *vhandle)
{
    return 0;
}

#endif

void *ossl_threads_ctx_new(OSSL_LIB_CTX *ctx)
{
    struct openssl_threads_st *t = OPENSSL_zalloc(sizeof(*t));

    if (t == NULL)
        return NULL;

    t->lock = ossl_crypto_mutex_new();
    t->cond_finished = ossl_crypto_condvar_new();

    if (t->lock == NULL || t->cond_finished == NULL)
        goto fail;

    return t;

fail:
    ossl_threads_ctx_free((void *)t);
    return NULL;
}

void ossl_threads_ctx_free(void *vdata)
{
    OSSL_LIB_CTX_THREADS *t = (OSSL_LIB_CTX_THREADS *) vdata;

    if (t == NULL)
        return;

    ossl_crypto_mutex_free(&t->lock);
    ossl_crypto_condvar_free(&t->cond_finished);
    OPENSSL_free(t);
}
