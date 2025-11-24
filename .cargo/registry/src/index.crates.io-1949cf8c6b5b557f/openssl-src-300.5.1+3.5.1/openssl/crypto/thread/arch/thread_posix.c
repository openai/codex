/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <internal/thread_arch.h>

#if defined(OPENSSL_THREADS_POSIX)
# define _GNU_SOURCE
# include <errno.h>
# include <sys/types.h>
# include <unistd.h>

static void *thread_start_thunk(void *vthread)
{
    CRYPTO_THREAD *thread;
    CRYPTO_THREAD_RETVAL ret;

    thread = (CRYPTO_THREAD *)vthread;

    ret = thread->routine(thread->data);
    ossl_crypto_mutex_lock(thread->statelock);
    CRYPTO_THREAD_SET_STATE(thread, CRYPTO_THREAD_FINISHED);
    thread->retval = ret;
    ossl_crypto_condvar_broadcast(thread->condvar);
    ossl_crypto_mutex_unlock(thread->statelock);

    return NULL;
}

int ossl_crypto_thread_native_spawn(CRYPTO_THREAD *thread)
{
    int ret;
    pthread_attr_t attr;
    pthread_t *handle;

    handle = OPENSSL_zalloc(sizeof(*handle));
    if (handle == NULL)
        goto fail;

    pthread_attr_init(&attr);
    if (!thread->joinable)
        pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    ret = pthread_create(handle, &attr, thread_start_thunk, thread);
    pthread_attr_destroy(&attr);

    if (ret != 0)
        goto fail;

    thread->handle = handle;
    return 1;

fail:
    thread->handle = NULL;
    OPENSSL_free(handle);
    return 0;
}

int ossl_crypto_thread_native_perform_join(CRYPTO_THREAD *thread, CRYPTO_THREAD_RETVAL *retval)
{
    void *thread_retval;
    pthread_t *handle;

    if (thread == NULL || thread->handle == NULL)
        return 0;

    handle = (pthread_t *) thread->handle;
    if (pthread_join(*handle, &thread_retval) != 0)
        return 0;

    /*
     * Join return value may be non-NULL when the thread has been cancelled,
     * as indicated by thread_retval set to PTHREAD_CANCELLED.
     */
    if (thread_retval != NULL)
        return 0;

    return 1;
}

int ossl_crypto_thread_native_exit(void)
{
    pthread_exit(NULL);
    return 1;
}

int ossl_crypto_thread_native_is_self(CRYPTO_THREAD *thread)
{
    return pthread_equal(*(pthread_t *)thread->handle, pthread_self());
}

CRYPTO_MUTEX *ossl_crypto_mutex_new(void)
{
    pthread_mutex_t *mutex;

    if ((mutex = OPENSSL_zalloc(sizeof(*mutex))) == NULL)
        return NULL;
    if (pthread_mutex_init(mutex, NULL) != 0) {
        OPENSSL_free(mutex);
        return NULL;
    }
    return (CRYPTO_MUTEX *)mutex;
}

int ossl_crypto_mutex_try_lock(CRYPTO_MUTEX *mutex)
{
    pthread_mutex_t *mutex_p;

    mutex_p = (pthread_mutex_t *)mutex;

    if (pthread_mutex_trylock(mutex_p) == EBUSY)
        return 0;

    return 1;
}

void ossl_crypto_mutex_lock(CRYPTO_MUTEX *mutex)
{
    int rc;
    pthread_mutex_t *mutex_p;

    mutex_p = (pthread_mutex_t *)mutex;
    rc = pthread_mutex_lock(mutex_p);
    OPENSSL_assert(rc == 0);
}

void ossl_crypto_mutex_unlock(CRYPTO_MUTEX *mutex)
{
    int rc;
    pthread_mutex_t *mutex_p;

    mutex_p = (pthread_mutex_t *)mutex;
    rc = pthread_mutex_unlock(mutex_p);
    OPENSSL_assert(rc == 0);
}

void ossl_crypto_mutex_free(CRYPTO_MUTEX **mutex)
{
    pthread_mutex_t **mutex_p;

    if (mutex == NULL)
        return;

    mutex_p = (pthread_mutex_t **)mutex;
    if (*mutex_p != NULL)
        pthread_mutex_destroy(*mutex_p);
    OPENSSL_free(*mutex_p);
    *mutex = NULL;
}

CRYPTO_CONDVAR *ossl_crypto_condvar_new(void)
{
    pthread_cond_t *cv_p;

    if ((cv_p = OPENSSL_zalloc(sizeof(*cv_p))) == NULL)
        return NULL;
    if (pthread_cond_init(cv_p, NULL) != 0) {
        OPENSSL_free(cv_p);
        return NULL;
    }
    return (CRYPTO_CONDVAR *) cv_p;
}

void ossl_crypto_condvar_wait(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex)
{
    pthread_cond_t *cv_p;
    pthread_mutex_t *mutex_p;

    cv_p = (pthread_cond_t *)cv;
    mutex_p = (pthread_mutex_t *)mutex;
    pthread_cond_wait(cv_p, mutex_p);
}

void ossl_crypto_condvar_wait_timeout(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex,
                                      OSSL_TIME deadline)
{
    pthread_cond_t *cv_p = (pthread_cond_t *)cv;
    pthread_mutex_t *mutex_p = (pthread_mutex_t *)mutex;

    if (ossl_time_is_infinite(deadline)) {
        /*
         * No deadline. Some pthread implementations allow
         * pthread_cond_timedwait to work the same as pthread_cond_wait when
         * abstime is NULL, but it is unclear whether this is POSIXly correct.
         */
        pthread_cond_wait(cv_p, mutex_p);
    } else {
        struct timespec deadline_ts;

        deadline_ts.tv_sec
            = ossl_time2seconds(deadline);
        deadline_ts.tv_nsec
            = (ossl_time2ticks(deadline) % OSSL_TIME_SECOND) / OSSL_TIME_NS;

        pthread_cond_timedwait(cv_p, mutex_p, &deadline_ts);
    }
}

void ossl_crypto_condvar_broadcast(CRYPTO_CONDVAR *cv)
{
    pthread_cond_t *cv_p;

    cv_p = (pthread_cond_t *)cv;
    pthread_cond_broadcast(cv_p);
}

void ossl_crypto_condvar_signal(CRYPTO_CONDVAR *cv)
{
    pthread_cond_t *cv_p;

    cv_p = (pthread_cond_t *)cv;
    pthread_cond_signal(cv_p);
}

void ossl_crypto_condvar_free(CRYPTO_CONDVAR **cv)
{
    pthread_cond_t **cv_p;

    if (cv == NULL)
        return;

    cv_p = (pthread_cond_t **)cv;
    if (*cv_p != NULL)
        pthread_cond_destroy(*cv_p);
    OPENSSL_free(*cv_p);
    *cv_p = NULL;
}

#endif
