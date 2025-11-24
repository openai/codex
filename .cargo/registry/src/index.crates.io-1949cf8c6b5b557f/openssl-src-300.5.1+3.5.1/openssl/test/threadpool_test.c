/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <internal/cryptlib.h>
#include <internal/thread_arch.h>
#include <internal/thread.h>
#include <openssl/thread.h>
#include "testutil.h"

static int test_thread_reported_flags(void)
{
    uint32_t flags = OSSL_get_thread_support_flags();

#if !defined(OPENSSL_THREADS)
    if (!TEST_int_eq(flags, 0))
        return 0;
#endif

#if defined(OPENSSL_NO_THREAD_POOL)
    if (!TEST_int_eq(flags & OSSL_THREAD_SUPPORT_FLAG_THREAD_POOL, 0))
        return 0;
#else
    if (!TEST_int_eq(flags & OSSL_THREAD_SUPPORT_FLAG_THREAD_POOL,
                     OSSL_THREAD_SUPPORT_FLAG_THREAD_POOL))
        return 0;
#endif

#if defined(OPENSSL_NO_DEFAULT_THREAD_POOL)
    if (!TEST_int_eq(flags & OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN, 0))
        return 0;
#else
    if (!TEST_int_eq(flags & OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN,
                     OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN))
        return 0;
#endif

    return 1;
}

#ifndef OPENSSL_NO_THREAD_POOL

# define TEST_THREAD_NATIVE_FN_SET_VALUE 1
static uint32_t test_thread_native_fn(void *data)
{
    uint32_t *ldata = (uint32_t*) data;
    *ldata = *ldata + 1;
    return *ldata - 1;
}
/* Tests of native threads */

static int test_thread_native(void)
{
    uint32_t retval;
    uint32_t local;
    CRYPTO_THREAD *t;

    /* thread spawn, join */

    local = 1;
    t = ossl_crypto_thread_native_start(test_thread_native_fn, &local, 1);
    if (!TEST_ptr(t))
        return 0;

    /*
     * pthread_join results in undefined behaviour if called on a joined
     * thread. We do not impose such restrictions, so it's up to us to
     * ensure that this does not happen (thread sanitizer will warn us
     * if we do).
     */
    if (!TEST_int_eq(ossl_crypto_thread_native_join(t, &retval), 1))
        return 0;
    if (!TEST_int_eq(ossl_crypto_thread_native_join(t, &retval), 1))
        return 0;

    if (!TEST_int_eq(retval, 1) || !TEST_int_eq(local, 2))
        return 0;

    if (!TEST_int_eq(ossl_crypto_thread_native_clean(t), 1))
        return 0;
    t = NULL;

    if (!TEST_int_eq(ossl_crypto_thread_native_clean(t), 0))
        return 0;

    return 1;
}

# if !defined(OPENSSL_NO_DEFAULT_THREAD_POOL)
static int test_thread_internal(void)
{
    uint32_t retval[3];
    uint32_t local[3] = { 0 };
    uint32_t threads_supported;
    size_t i;
    void *t[3];
    int status = 0;
    OSSL_LIB_CTX *cust_ctx = OSSL_LIB_CTX_new();

    threads_supported = OSSL_get_thread_support_flags();
    threads_supported &= OSSL_THREAD_SUPPORT_FLAG_DEFAULT_SPAWN;

    if (threads_supported == 0) {
        if (!TEST_uint64_t_eq(OSSL_get_max_threads(NULL), 0))
            goto cleanup;
        if (!TEST_uint64_t_eq(OSSL_get_max_threads(cust_ctx), 0))
            goto cleanup;

        if (!TEST_int_eq(OSSL_set_max_threads(NULL, 1), 0))
            goto cleanup;
        if (!TEST_int_eq(OSSL_set_max_threads(cust_ctx, 1), 0))
            goto cleanup;

        if (!TEST_uint64_t_eq(OSSL_get_max_threads(NULL), 0))
            goto cleanup;
        if (!TEST_uint64_t_eq(OSSL_get_max_threads(cust_ctx), 0))
            goto cleanup;

        t[0] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[0]);
        if (!TEST_ptr_null(t[0]))
            goto cleanup;

        status = 1;
        goto cleanup;
    }

    /* fail when not allowed to use threads */

    if (!TEST_uint64_t_eq(OSSL_get_max_threads(NULL), 0))
        goto cleanup;
    t[0] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[0]);
    if (!TEST_ptr_null(t[0]))
        goto cleanup;

    /* fail when enabled on a different context */
    if (!TEST_uint64_t_eq(OSSL_get_max_threads(cust_ctx), 0))
        goto cleanup;
    if (!TEST_int_eq(OSSL_set_max_threads(cust_ctx, 1), 1))
        goto cleanup;
    if (!TEST_uint64_t_eq(OSSL_get_max_threads(NULL), 0))
        goto cleanup;
    if (!TEST_uint64_t_eq(OSSL_get_max_threads(cust_ctx), 1))
        goto cleanup;
    t[0] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[0]);
    if (!TEST_ptr_null(t[0]))
        goto cleanup;
    if (!TEST_int_eq(OSSL_set_max_threads(cust_ctx, 0), 1))
        goto cleanup;

    /* sequential startup */

    if (!TEST_int_eq(OSSL_set_max_threads(NULL, 1), 1))
        goto cleanup;
    if (!TEST_uint64_t_eq(OSSL_get_max_threads(NULL), 1))
        goto cleanup;
    if (!TEST_uint64_t_eq(OSSL_get_max_threads(cust_ctx), 0))
        goto cleanup;

    for (i = 0; i < OSSL_NELEM(t); ++i) {
        local[0] = i + 1;

        t[i] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[0]);
        if (!TEST_ptr(t[i]))
            goto cleanup;

        /*
         * pthread_join results in undefined behaviour if called on a joined
         * thread. We do not impose such restrictions, so it's up to us to
         * ensure that this does not happen (thread sanitizer will warn us
         * if we do).
         */
        if (!TEST_int_eq(ossl_crypto_thread_join(t[i], &retval[0]), 1))
            goto cleanup;
        if (!TEST_int_eq(ossl_crypto_thread_join(t[i], &retval[0]), 1))
            goto cleanup;

        if (!TEST_int_eq(retval[0], i + 1) || !TEST_int_eq(local[0], i + 2))
            goto cleanup;

        if (!TEST_int_eq(ossl_crypto_thread_clean(t[i]), 1))
            goto cleanup;
        t[i] = NULL;

        if (!TEST_int_eq(ossl_crypto_thread_clean(t[i]), 0))
            goto cleanup;
    }

    /* parallel startup */

    if (!TEST_int_eq(OSSL_set_max_threads(NULL, OSSL_NELEM(t)), 1))
        goto cleanup;

    for (i = 0; i < OSSL_NELEM(t); ++i) {
        local[i] = i + 1;
        t[i] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[i]);
        if (!TEST_ptr(t[i]))
            goto cleanup;
    }
    for (i = 0; i < OSSL_NELEM(t); ++i) {
        if (!TEST_int_eq(ossl_crypto_thread_join(t[i], &retval[i]), 1))
            goto cleanup;
    }
    for (i = 0; i < OSSL_NELEM(t); ++i) {
        if (!TEST_int_eq(retval[i], i + 1) || !TEST_int_eq(local[i], i + 2))
            goto cleanup;
        if (!TEST_int_eq(ossl_crypto_thread_clean(t[i]), 1))
            goto cleanup;
    }

    /* parallel startup, bottleneck */

    if (!TEST_int_eq(OSSL_set_max_threads(NULL, OSSL_NELEM(t) - 1), 1))
        goto cleanup;

    for (i = 0; i < OSSL_NELEM(t); ++i) {
        local[i] = i + 1;
        t[i] = ossl_crypto_thread_start(NULL, test_thread_native_fn, &local[i]);
        if (!TEST_ptr(t[i]))
            goto cleanup;
    }
    for (i = 0; i < OSSL_NELEM(t); ++i) {
        if (!TEST_int_eq(ossl_crypto_thread_join(t[i], &retval[i]), 1))
            goto cleanup;
    }
    for (i = 0; i < OSSL_NELEM(t); ++i) {
        if (!TEST_int_eq(retval[i], i + 1) || !TEST_int_eq(local[i], i + 2))
            goto cleanup;
        if (!TEST_int_eq(ossl_crypto_thread_clean(t[i]), 1))
            goto cleanup;
    }

    if (!TEST_int_eq(OSSL_set_max_threads(NULL, 0), 1))
        goto cleanup;

    status = 1;
cleanup:
    OSSL_LIB_CTX_free(cust_ctx);
    return status;
}
# endif

static uint32_t test_thread_native_multiple_joins_fn1(void *data)
{
    return 0;
}

static uint32_t test_thread_native_multiple_joins_fn2(void *data)
{
    ossl_crypto_thread_native_join((CRYPTO_THREAD *)data, NULL);
    return 0;
}

static uint32_t test_thread_native_multiple_joins_fn3(void *data)
{
    ossl_crypto_thread_native_join((CRYPTO_THREAD *)data, NULL);
    return 0;
}

static int test_thread_native_multiple_joins(void)
{
    CRYPTO_THREAD *t, *t1, *t2;

    t = ossl_crypto_thread_native_start(test_thread_native_multiple_joins_fn1, NULL, 1);
    t1 = ossl_crypto_thread_native_start(test_thread_native_multiple_joins_fn2, t, 1);
    t2 = ossl_crypto_thread_native_start(test_thread_native_multiple_joins_fn3, t, 1);

    if (!TEST_ptr(t) || !TEST_ptr(t1) || !TEST_ptr(t2))
        return 0;

    if (!TEST_int_eq(ossl_crypto_thread_native_join(t2, NULL), 1))
        return 0;
    if (!TEST_int_eq(ossl_crypto_thread_native_join(t1, NULL), 1))
        return 0;

    if (!TEST_int_eq(ossl_crypto_thread_native_clean(t2), 1))
        return 0;

    if (!TEST_int_eq(ossl_crypto_thread_native_clean(t1), 1))
        return 0;

    if (!TEST_int_eq(ossl_crypto_thread_native_clean(t), 1))
        return 0;

    return 1;
}

#endif

int setup_tests(void)
{
    ADD_TEST(test_thread_reported_flags);
#if !defined(OPENSSL_NO_THREAD_POOL)
    ADD_TEST(test_thread_native);
    ADD_TEST(test_thread_native_multiple_joins);
# if !defined(OPENSSL_NO_DEFAULT_THREAD_POOL)
    ADD_TEST(test_thread_internal);
# endif
#endif

    return 1;
}
