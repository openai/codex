/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/configuration.h>
#include <internal/thread_arch.h>

CRYPTO_THREAD *ossl_crypto_thread_native_start(CRYPTO_THREAD_ROUTINE routine,
                                               void *data, int joinable)
{
    CRYPTO_THREAD *handle;

    if (routine == NULL)
        return NULL;

    handle = OPENSSL_zalloc(sizeof(*handle));
    if (handle == NULL)
        return NULL;

    if ((handle->lock = ossl_crypto_mutex_new()) == NULL)
        goto fail;
    if ((handle->statelock = ossl_crypto_mutex_new()) == NULL)
        goto fail;
    if ((handle->condvar = ossl_crypto_condvar_new()) == NULL)
        goto fail;

    handle->data = data;
    handle->routine = routine;
    handle->joinable = joinable;

    if (ossl_crypto_thread_native_spawn(handle) == 1)
        return handle;

fail:
    ossl_crypto_condvar_free(&handle->condvar);
    ossl_crypto_mutex_free(&handle->statelock);
    ossl_crypto_mutex_free(&handle->lock);
    OPENSSL_free(handle);
    return NULL;
}

int ossl_crypto_thread_native_join(CRYPTO_THREAD *thread, CRYPTO_THREAD_RETVAL *retval)
{
    uint64_t req_state_mask;

    if (thread == NULL)
        return 0;

    ossl_crypto_mutex_lock(thread->statelock);
    req_state_mask = CRYPTO_THREAD_FINISHED | CRYPTO_THREAD_JOINED;
    while (!CRYPTO_THREAD_GET_STATE(thread, req_state_mask))
        ossl_crypto_condvar_wait(thread->condvar, thread->statelock);

    if (CRYPTO_THREAD_GET_STATE(thread, CRYPTO_THREAD_JOINED))
        goto pass;

    /* Await concurrent join completion, if any. */
    while (CRYPTO_THREAD_GET_STATE(thread, CRYPTO_THREAD_JOIN_AWAIT)) {
        if (!CRYPTO_THREAD_GET_STATE(thread, CRYPTO_THREAD_JOINED))
            ossl_crypto_condvar_wait(thread->condvar, thread->statelock);
        if (CRYPTO_THREAD_GET_STATE(thread, CRYPTO_THREAD_JOINED))
            goto pass;
    }
    CRYPTO_THREAD_SET_STATE(thread, CRYPTO_THREAD_JOIN_AWAIT);
    ossl_crypto_mutex_unlock(thread->statelock);

    if (ossl_crypto_thread_native_perform_join(thread, retval) == 0)
        goto fail;

    ossl_crypto_mutex_lock(thread->statelock);
pass:
    CRYPTO_THREAD_UNSET_ERROR(thread, CRYPTO_THREAD_JOINED);
    CRYPTO_THREAD_SET_STATE(thread, CRYPTO_THREAD_JOINED);

    /*
     * Signal join completion. It is important to signal even if we haven't
     * performed an actual join. Multiple threads could be awaiting the
     * CRYPTO_THREAD_JOIN_AWAIT -> CRYPTO_THREAD_JOINED transition, but signal
     * on actual join would wake only one. Signalling here will always wake one.
     */
    ossl_crypto_condvar_signal(thread->condvar);
    ossl_crypto_mutex_unlock(thread->statelock);

    if (retval != NULL)
        *retval = thread->retval;
    return 1;

fail:
    ossl_crypto_mutex_lock(thread->statelock);
    CRYPTO_THREAD_SET_ERROR(thread, CRYPTO_THREAD_JOINED);

    /* Have another thread that's awaiting join retry to avoid that
     * thread deadlock. */
    CRYPTO_THREAD_UNSET_STATE(thread, CRYPTO_THREAD_JOIN_AWAIT);
    ossl_crypto_condvar_signal(thread->condvar);

    ossl_crypto_mutex_unlock(thread->statelock);
    return 0;
}

int ossl_crypto_thread_native_clean(CRYPTO_THREAD *handle)
{
    uint64_t req_state_mask;

    if (handle == NULL)
        return 0;

    req_state_mask = 0;
    req_state_mask |= CRYPTO_THREAD_FINISHED;
    req_state_mask |= CRYPTO_THREAD_JOINED;

    ossl_crypto_mutex_lock(handle->statelock);
    if (CRYPTO_THREAD_GET_STATE(handle, req_state_mask) == 0) {
        ossl_crypto_mutex_unlock(handle->statelock);
        return 0;
    }
    ossl_crypto_mutex_unlock(handle->statelock);

    ossl_crypto_mutex_free(&handle->lock);
    ossl_crypto_mutex_free(&handle->statelock);
    ossl_crypto_condvar_free(&handle->condvar);

    OPENSSL_free(handle->handle);
    OPENSSL_free(handle);

    return 1;
}
