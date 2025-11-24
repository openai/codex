/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <internal/thread_arch.h>

#if defined(OPENSSL_THREADS_NONE)

int ossl_crypto_thread_native_spawn(CRYPTO_THREAD *thread)
{
    return 0;
}

int ossl_crypto_thread_native_perform_join(CRYPTO_THREAD *thread, CRYPTO_THREAD_RETVAL *retval)
{
    return 0;
}

int ossl_crypto_thread_native_exit(void)
{
    return 0;
}

int ossl_crypto_thread_native_is_self(CRYPTO_THREAD *thread)
{
    return 0;
}

CRYPTO_MUTEX *ossl_crypto_mutex_new(void)
{
    return NULL;
}

void ossl_crypto_mutex_lock(CRYPTO_MUTEX *mutex)
{
}

int ossl_crypto_mutex_try_lock(CRYPTO_MUTEX *mutex)
{
    return 0;
}

void ossl_crypto_mutex_unlock(CRYPTO_MUTEX *mutex)
{
}

void ossl_crypto_mutex_free(CRYPTO_MUTEX **mutex)
{
}

CRYPTO_CONDVAR *ossl_crypto_condvar_new(void)
{
    return NULL;
}

void ossl_crypto_condvar_wait(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex)
{
}

void ossl_crypto_condvar_wait_timeout(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex,
                                      OSSL_TIME deadline)
{
}

void ossl_crypto_condvar_broadcast(CRYPTO_CONDVAR *cv)
{
}

void ossl_crypto_condvar_signal(CRYPTO_CONDVAR *cv)
{
}

void ossl_crypto_condvar_free(CRYPTO_CONDVAR **cv)
{
}

#endif
