/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * The test_multi_downgrade_shared_pkey function tests the thread safety of a
 * deprecated function.
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
# define OPENSSL_SUPPRESS_DEPRECATED
#endif

#if defined(_WIN32)
# include <windows.h>
#endif

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/rsa.h>
#include <openssl/aes.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/pem.h>
#include <openssl/evp.h>
#include "internal/tsan_assist.h"
#include "internal/nelem.h"
#include "internal/time.h"
#include "internal/rcu.h"
#include "testutil.h"
#include "threadstest.h"

#ifdef __SANITIZE_THREAD__
#include <sanitizer/tsan_interface.h>
#define TSAN_ACQUIRE(s) __tsan_acquire(s)
#else
#define TSAN_ACQUIRE(s)
#endif

/* Limit the maximum number of threads */
#define MAXIMUM_THREADS     10

/* Limit the maximum number of providers loaded into a library context */
#define MAXIMUM_PROVIDERS   4

static int do_fips = 0;
static char *privkey;
static char *config_file = NULL;
static int multidefault_run = 0;

static const char *default_provider[] = { "default", NULL };
static const char *fips_provider[] = { "fips", NULL };
static const char *fips_and_default_providers[] = { "default", "fips", NULL };

static CRYPTO_RWLOCK *global_lock;

#ifdef TSAN_REQUIRES_LOCKING
static CRYPTO_RWLOCK *tsan_lock;
#endif

/* Grab a globally unique integer value, return 0 on failure */
static int get_new_uid(void)
{
    /*
     * Start with a nice large number to avoid potential conflicts when
     * we generate a new OID.
     */
    static TSAN_QUALIFIER int current_uid = 1 << (sizeof(int) * 8 - 2);
#ifdef TSAN_REQUIRES_LOCKING
    int r;

    if (!TEST_true(CRYPTO_THREAD_write_lock(tsan_lock)))
        return 0;
    r = ++current_uid;
    if (!TEST_true(CRYPTO_THREAD_unlock(tsan_lock)))
        return 0;
    return r;

#else
    return tsan_counter(&current_uid);
#endif
}

static int test_lock(void)
{
    CRYPTO_RWLOCK *lock = CRYPTO_THREAD_lock_new();
    int res;

    if (!TEST_ptr(lock))
        return 0;

    res = TEST_true(CRYPTO_THREAD_read_lock(lock))
          && TEST_true(CRYPTO_THREAD_unlock(lock))
          && TEST_true(CRYPTO_THREAD_write_lock(lock))
          && TEST_true(CRYPTO_THREAD_unlock(lock));

    CRYPTO_THREAD_lock_free(lock);

    return res;
}

#if defined(OPENSSL_THREADS)
static int contention = 0;
static int rwwriter1_done = 0;
static int rwwriter2_done = 0;
static int rwreader1_iterations = 0;
static int rwreader2_iterations = 0;
static int rwwriter1_iterations = 0;
static int rwwriter2_iterations = 0;
static int *rwwriter_ptr = NULL;
static int rw_torture_result = 1;
static CRYPTO_RWLOCK *rwtorturelock = NULL;
static CRYPTO_RWLOCK *atomiclock = NULL;

static void rwwriter_fn(int id, int *iterations)
{
    int count;
    int *old, *new;
    OSSL_TIME t1, t2;
    t1 = ossl_time_now();

    for (count = 0; ; count++) {
        new = CRYPTO_zalloc(sizeof (int), NULL, 0);
        if (contention == 0)
            OSSL_sleep(1000);
        if (!CRYPTO_THREAD_write_lock(rwtorturelock))
            abort();
        if (rwwriter_ptr != NULL) {
            *new = *rwwriter_ptr + 1;
        } else {
            *new = 0;
        }
        old = rwwriter_ptr;
        rwwriter_ptr = new;
        if (!CRYPTO_THREAD_unlock(rwtorturelock))
            abort();
        if (old != NULL)
            CRYPTO_free(old, __FILE__, __LINE__);
        t2 = ossl_time_now();
        if ((ossl_time2seconds(t2) - ossl_time2seconds(t1)) >= 4)
            break;
    }
    *iterations = count;
    return;
}

static void rwwriter1_fn(void)
{
    int local;

    TEST_info("Starting writer1");
    rwwriter_fn(1, &rwwriter1_iterations);
    CRYPTO_atomic_add(&rwwriter1_done, 1, &local, atomiclock);
}

static void rwwriter2_fn(void)
{
    int local;

    TEST_info("Starting writer 2");
    rwwriter_fn(2, &rwwriter2_iterations);
    CRYPTO_atomic_add(&rwwriter2_done, 1, &local, atomiclock);
}

static void rwreader_fn(int *iterations)
{
    unsigned int count = 0;

    int old = 0;
    int lw1 = 0;
    int lw2 = 0;

    if (CRYPTO_THREAD_read_lock(rwtorturelock) == 0)
            abort();

    while (lw1 != 1 || lw2 != 1) {
        CRYPTO_atomic_add(&rwwriter1_done, 0, &lw1, atomiclock);
        CRYPTO_atomic_add(&rwwriter2_done, 0, &lw2, atomiclock);

        count++;
        if (rwwriter_ptr != NULL && old > *rwwriter_ptr) {
            TEST_info("rwwriter pointer went backwards\n");
            rw_torture_result = 0;
        }
        if (CRYPTO_THREAD_unlock(rwtorturelock) == 0)
            abort();
        *iterations = count;
        if (rw_torture_result == 0) {
            *iterations = count;
            return;
        }
        if (CRYPTO_THREAD_read_lock(rwtorturelock) == 0)
            abort();
    }
    *iterations = count;
    if (CRYPTO_THREAD_unlock(rwtorturelock) == 0)
            abort();
}

static void rwreader1_fn(void)
{
    TEST_info("Starting reader 1");
    rwreader_fn(&rwreader1_iterations);
}

static void rwreader2_fn(void)
{
    TEST_info("Starting reader 2");
    rwreader_fn(&rwreader2_iterations);
}

static thread_t rwwriter1;
static thread_t rwwriter2;
static thread_t rwreader1;
static thread_t rwreader2;

static int _torture_rw(void)
{
    double tottime = 0;
    int ret = 0;
    double avr, avw;
    OSSL_TIME t1, t2;
    struct timeval dtime;

    rwtorturelock = CRYPTO_THREAD_lock_new();
    atomiclock = CRYPTO_THREAD_lock_new();
    if (!TEST_ptr(rwtorturelock) || !TEST_ptr(atomiclock))
        goto out;

    rwwriter1_iterations = 0;
    rwwriter2_iterations = 0;
    rwreader1_iterations = 0;
    rwreader2_iterations = 0;
    rwwriter1_done = 0;
    rwwriter2_done = 0;
    rw_torture_result = 1;

    memset(&rwwriter1, 0, sizeof(thread_t));
    memset(&rwwriter2, 0, sizeof(thread_t));
    memset(&rwreader1, 0, sizeof(thread_t));
    memset(&rwreader2, 0, sizeof(thread_t));

    TEST_info("Staring rw torture");
    t1 = ossl_time_now();
    if (!TEST_true(run_thread(&rwreader1, rwreader1_fn))
        || !TEST_true(run_thread(&rwreader2, rwreader2_fn))
        || !TEST_true(run_thread(&rwwriter1, rwwriter1_fn))
        || !TEST_true(run_thread(&rwwriter2, rwwriter2_fn))
        || !TEST_true(wait_for_thread(rwwriter1))
        || !TEST_true(wait_for_thread(rwwriter2))
        || !TEST_true(wait_for_thread(rwreader1))
        || !TEST_true(wait_for_thread(rwreader2)))
        goto out;

    t2 = ossl_time_now();
    dtime = ossl_time_to_timeval(ossl_time_subtract(t2, t1));
    tottime = dtime.tv_sec + (dtime.tv_usec / 1e6);
    TEST_info("rw_torture_result is %d\n", rw_torture_result);
    TEST_info("performed %d reads and %d writes over 2 read and 2 write threads in %e seconds",
              rwreader1_iterations + rwreader2_iterations,
              rwwriter1_iterations + rwwriter2_iterations, tottime);
    if ((rwreader1_iterations + rwreader2_iterations == 0)
        || (rwwriter1_iterations + rwwriter2_iterations == 0)) {
        TEST_info("Threads did not iterate\n");
        goto out;
    }
    avr = tottime / (rwreader1_iterations + rwreader2_iterations);
    avw = (tottime / (rwwriter1_iterations + rwwriter2_iterations));
    TEST_info("Average read time %e/read", avr);
    TEST_info("Averate write time %e/write", avw);

    if (TEST_int_eq(rw_torture_result, 1))
        ret = 1;
out:
    CRYPTO_THREAD_lock_free(rwtorturelock);
    CRYPTO_THREAD_lock_free(atomiclock);
    rwtorturelock = NULL;
    return ret;
}

static int torture_rw_low(void)
{
    contention = 0;
    return _torture_rw();
}

static int torture_rw_high(void)
{
    contention = 1;
    return _torture_rw();
}


static CRYPTO_RCU_LOCK *rcu_lock = NULL;

static int writer1_done = 0;
static int writer2_done = 0;
static int reader1_iterations = 0;
static int reader2_iterations = 0;
static int writer1_iterations = 0;
static int writer2_iterations = 0;
static uint64_t *writer_ptr = NULL;
static uint64_t global_ctr = 0;
static int rcu_torture_result = 1;
static void free_old_rcu_data(void *data)
{
    CRYPTO_free(data, NULL, 0);
}

static void writer_fn(int id, int *iterations)
{
    int count;
    OSSL_TIME t1, t2;
    uint64_t *old, *new;

    t1 = ossl_time_now();

    for (count = 0; ; count++) {
        new = CRYPTO_zalloc(sizeof(uint64_t), NULL, 0);
        if (contention == 0)
            OSSL_sleep(1000);
        ossl_rcu_write_lock(rcu_lock);
        old = ossl_rcu_deref(&writer_ptr);
        TSAN_ACQUIRE(&writer_ptr);
        *new = global_ctr++;
        ossl_rcu_assign_ptr(&writer_ptr, &new);
        if (contention == 0)
            ossl_rcu_call(rcu_lock, free_old_rcu_data, old);
        ossl_rcu_write_unlock(rcu_lock);
        if (contention != 0) {
            ossl_synchronize_rcu(rcu_lock);
            CRYPTO_free(old, NULL, 0);
        }
        t2 = ossl_time_now();
        if ((ossl_time2seconds(t2) - ossl_time2seconds(t1)) >= 4)
            break;
    }
    *iterations = count;
    return;
}

static void writer1_fn(void)
{
    int local;

    TEST_info("Starting writer1");
    writer_fn(1, &writer1_iterations);
    CRYPTO_atomic_add(&writer1_done, 1, &local, atomiclock);
}

static void writer2_fn(void)
{
    int local;

    TEST_info("Starting writer2");
    writer_fn(2, &writer2_iterations);
    CRYPTO_atomic_add(&writer2_done, 1, &local, atomiclock);
}

static void reader_fn(int *iterations)
{
    unsigned int count = 0;
    uint64_t *valp;
    uint64_t val;
    uint64_t oldval = 0;
    int lw1 = 0;
    int lw2 = 0;

    while (lw1 != 1 || lw2 != 1) {
        CRYPTO_atomic_add(&writer1_done, 0, &lw1, atomiclock);
        CRYPTO_atomic_add(&writer2_done, 0, &lw2, atomiclock);
        count++;
        ossl_rcu_read_lock(rcu_lock);
        valp = ossl_rcu_deref(&writer_ptr);
        val = (valp == NULL) ? 0 : *valp;

        if (oldval > val) {
            TEST_info("rcu torture value went backwards! %llu : %llu", (unsigned long long)oldval, (unsigned long long)val);
            rcu_torture_result = 0;
        }
        oldval = val; /* just try to deref the pointer */
        ossl_rcu_read_unlock(rcu_lock);
        if (rcu_torture_result == 0) {
            *iterations = count;
            return;
        }
    }
    *iterations = count;
}

static void reader1_fn(void)
{
    TEST_info("Starting reader 1");
    reader_fn(&reader1_iterations);
}

static void reader2_fn(void)
{
    TEST_info("Starting reader 2");
    reader_fn(&reader2_iterations);
}

static thread_t writer1;
static thread_t writer2;
static thread_t reader1;
static thread_t reader2;

static int _torture_rcu(void)
{
    OSSL_TIME t1, t2;
    struct timeval dtime;
    double tottime;
    double avr, avw;
    int rc = 0;

    atomiclock = CRYPTO_THREAD_lock_new();
    if (!TEST_ptr(atomiclock))
        goto out;

    memset(&writer1, 0, sizeof(thread_t));
    memset(&writer2, 0, sizeof(thread_t));
    memset(&reader1, 0, sizeof(thread_t));
    memset(&reader2, 0, sizeof(thread_t));

    writer1_iterations = 0;
    writer2_iterations = 0;
    reader1_iterations = 0;
    reader2_iterations = 0;
    writer1_done = 0;
    writer2_done = 0;
    rcu_torture_result = 1;

    rcu_lock = ossl_rcu_lock_new(contention == 2 ? 4 : 1, NULL);
    if (rcu_lock == NULL)
        goto out;

    TEST_info("Staring rcu torture");
    t1 = ossl_time_now();
    if (!TEST_true(run_thread(&reader1, reader1_fn))
        || !TEST_true(run_thread(&reader2, reader2_fn))
        || !TEST_true(run_thread(&writer1, writer1_fn))
        || !TEST_true(run_thread(&writer2, writer2_fn))
        || !TEST_true(wait_for_thread(writer1))
        || !TEST_true(wait_for_thread(writer2))
        || !TEST_true(wait_for_thread(reader1))
        || !TEST_true(wait_for_thread(reader2)))
        goto out;

    t2 = ossl_time_now();
    dtime = ossl_time_to_timeval(ossl_time_subtract(t2, t1));
    tottime = dtime.tv_sec + (dtime.tv_usec / 1e6);
    TEST_info("rcu_torture_result is %d\n", rcu_torture_result);
    TEST_info("performed %d reads and %d writes over 2 read and 2 write threads in %e seconds",
              reader1_iterations + reader2_iterations,
              writer1_iterations + writer2_iterations, tottime);
    if ((reader1_iterations + reader2_iterations == 0)
        || (writer1_iterations + writer2_iterations == 0)) {
        TEST_info("Threads did not iterate\n");
        goto out;
    }
    avr = tottime / (reader1_iterations + reader2_iterations);
    avw = tottime / (writer1_iterations + writer2_iterations);
    TEST_info("Average read time %e/read", avr);
    TEST_info("Average write time %e/write", avw);

    if (!TEST_int_eq(rcu_torture_result, 1))
        goto out;

    rc = 1;
out:
    ossl_rcu_lock_free(rcu_lock);
    CRYPTO_THREAD_lock_free(atomiclock);
    if (!TEST_int_eq(rcu_torture_result, 1))
        return 0;

    return rc;
}

static int torture_rcu_low(void)
{
    contention = 0;
    return _torture_rcu();
}

static int torture_rcu_high(void)
{
    contention = 1;
    return _torture_rcu();
}

static int torture_rcu_high2(void)
{
    contention = 2;
    return _torture_rcu();
}
#endif

static CRYPTO_ONCE once_run = CRYPTO_ONCE_STATIC_INIT;
static unsigned once_run_count = 0;

static void once_do_run(void)
{
    once_run_count++;
}

static void once_run_thread_cb(void)
{
    CRYPTO_THREAD_run_once(&once_run, once_do_run);
}

static int test_once(void)
{
    thread_t thread;

    if (!TEST_true(run_thread(&thread, once_run_thread_cb))
        || !TEST_true(wait_for_thread(thread))
        || !CRYPTO_THREAD_run_once(&once_run, once_do_run)
        || !TEST_int_eq(once_run_count, 1))
        return 0;
    return 1;
}

static CRYPTO_THREAD_LOCAL thread_local_key;
static unsigned destructor_run_count = 0;
static int thread_local_thread_cb_ok = 0;

static void thread_local_destructor(void *arg)
{
    unsigned *count;

    if (arg == NULL)
        return;

    count = arg;

    (*count)++;
}

static void thread_local_thread_cb(void)
{
    void *ptr;

    ptr = CRYPTO_THREAD_get_local(&thread_local_key);
    if (!TEST_ptr_null(ptr)
        || !TEST_true(CRYPTO_THREAD_set_local(&thread_local_key,
                                              &destructor_run_count)))
        return;

    ptr = CRYPTO_THREAD_get_local(&thread_local_key);
    if (!TEST_ptr_eq(ptr, &destructor_run_count))
        return;

    thread_local_thread_cb_ok = 1;
}

static int test_thread_local(void)
{
    thread_t thread;
    void *ptr = NULL;

    if (!TEST_true(CRYPTO_THREAD_init_local(&thread_local_key,
                                            thread_local_destructor)))
        return 0;

    ptr = CRYPTO_THREAD_get_local(&thread_local_key);
    if (!TEST_ptr_null(ptr)
        || !TEST_true(run_thread(&thread, thread_local_thread_cb))
        || !TEST_true(wait_for_thread(thread))
        || !TEST_int_eq(thread_local_thread_cb_ok, 1))
        return 0;

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)

    ptr = CRYPTO_THREAD_get_local(&thread_local_key);
    if (!TEST_ptr_null(ptr))
        return 0;

# if !defined(OPENSSL_SYS_WINDOWS)
    if (!TEST_int_eq(destructor_run_count, 1))
        return 0;
# endif
#endif

    if (!TEST_true(CRYPTO_THREAD_cleanup_local(&thread_local_key)))
        return 0;
    return 1;
}

/*
 * Basic test to ensure that we can repeatedly create and
 * destroy local keys without leaking anything
 */
static int test_thread_local_multi_key(void)
{
    int dummy;
    int i;

    for (i = 0; i < 1000; i++) {
        if (!TEST_true(CRYPTO_THREAD_init_local(&thread_local_key,
                                                thread_local_destructor)))
            return 0;

        if (!TEST_true(CRYPTO_THREAD_set_local(&thread_local_key, &dummy)))
            return 0;

        if (!TEST_true(CRYPTO_THREAD_cleanup_local(&thread_local_key)))
            return 0;
    }
    return 1;
}

static int test_atomic(void)
{
    int val = 0, ret = 0, testresult = 0;
    uint64_t val64 = 1, ret64 = 0;
    CRYPTO_RWLOCK *lock = CRYPTO_THREAD_lock_new();

    if (!TEST_ptr(lock))
        return 0;

    if (CRYPTO_atomic_add(&val, 1, &ret, NULL)) {
        /* This succeeds therefore we're on a platform with lockless atomics */
        if (!TEST_int_eq(val, 1) || !TEST_int_eq(val, ret))
            goto err;
    } else {
        /* This failed therefore we're on a platform without lockless atomics */
        if (!TEST_int_eq(val, 0) || !TEST_int_eq(val, ret))
            goto err;
    }
    val = 0;
    ret = 0;

    if (!TEST_true(CRYPTO_atomic_add(&val, 1, &ret, lock)))
        goto err;
    if (!TEST_int_eq(val, 1) || !TEST_int_eq(val, ret))
        goto err;

    if (CRYPTO_atomic_or(&val64, 2, &ret64, NULL)) {
        /* This succeeds therefore we're on a platform with lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 3)
                || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
            goto err;
    } else {
        /* This failed therefore we're on a platform without lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 1)
                || !TEST_int_eq((unsigned int)ret64, 0))
            goto err;
    }
    val64 = 1;
    ret64 = 0;

    if (!TEST_true(CRYPTO_atomic_or(&val64, 2, &ret64, lock)))
        goto err;

    if (!TEST_uint_eq((unsigned int)val64, 3)
            || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
        goto err;

    ret64 = 0;
    if (CRYPTO_atomic_load(&val64, &ret64, NULL)) {
        /* This succeeds therefore we're on a platform with lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 3)
                || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
            goto err;
    } else {
        /* This failed therefore we're on a platform without lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 3)
                || !TEST_int_eq((unsigned int)ret64, 0))
            goto err;
    }

    ret64 = 0;
    if (!TEST_true(CRYPTO_atomic_load(&val64, &ret64, lock)))
        goto err;

    if (!TEST_uint_eq((unsigned int)val64, 3)
            || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
        goto err;

    ret64 = 0;

    if (CRYPTO_atomic_and(&val64, 5, &ret64, NULL)) {
        /* This succeeds therefore we're on a platform with lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 1)
                || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
            goto err;
    } else {
        /* This failed therefore we're on a platform without lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 3)
                || !TEST_int_eq((unsigned int)ret64, 0))
            goto err;
    }
    val64 = 3;
    ret64 = 0;

    if (!TEST_true(CRYPTO_atomic_and(&val64, 5, &ret64, lock)))
        goto err;

    if (!TEST_uint_eq((unsigned int)val64, 1)
            || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
        goto err;

    ret64 = 0;

    if (CRYPTO_atomic_add64(&val64, 2, &ret64, NULL)) {
        /* This succeeds therefore we're on a platform with lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 3)
                || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
            goto err;
    } else {
        /* This failed therefore we're on a platform without lockless atomics */
        if (!TEST_uint_eq((unsigned int)val64, 1)
                || !TEST_int_eq((unsigned int)ret64, 0))
            goto err;
    }
    val64 = 1;
    ret64 = 0;

    if (!TEST_true(CRYPTO_atomic_add64(&val64, 2, &ret64, lock)))
        goto err;

    if (!TEST_uint_eq((unsigned int)val64, 3)
            || !TEST_uint_eq((unsigned int)val64, (unsigned int)ret64))
        goto err;

    testresult = 1;
 err:
    CRYPTO_THREAD_lock_free(lock);
    return testresult;
}

static OSSL_LIB_CTX *multi_libctx = NULL;
static int multi_success;
static OSSL_PROVIDER *multi_provider[MAXIMUM_PROVIDERS + 1];
static size_t multi_num_threads;
static thread_t multi_threads[MAXIMUM_THREADS];

static void multi_intialise(void)
{
    multi_success = 1;
    multi_libctx = NULL;
    multi_num_threads = 0;
    memset(multi_threads, 0, sizeof(multi_threads));
    memset(multi_provider, 0, sizeof(multi_provider));
}

static void multi_set_success(int ok)
{
    if (CRYPTO_THREAD_write_lock(global_lock) == 0) {
        /* not synchronized, but better than not reporting failure */
        multi_success = ok;
        return;
    }

    multi_success = ok;

    CRYPTO_THREAD_unlock(global_lock);
}

static void thead_teardown_libctx(void)
{
    OSSL_PROVIDER **p;

    for (p = multi_provider; *p != NULL; p++)
        OSSL_PROVIDER_unload(*p);
    OSSL_LIB_CTX_free(multi_libctx);
    multi_intialise();
}

static int thread_setup_libctx(int libctx, const char *providers[])
{
    size_t n;

    if (libctx && !TEST_true(test_get_libctx(&multi_libctx, NULL, config_file,
                                             NULL, NULL)))
        return 0;

    if (providers != NULL)
        for (n = 0; providers[n] != NULL; n++)
            if (!TEST_size_t_lt(n, MAXIMUM_PROVIDERS)
                || !TEST_ptr(multi_provider[n] = OSSL_PROVIDER_load(multi_libctx,
                                                                    providers[n]))) {
                thead_teardown_libctx();
                return 0;
            }
    return 1;
}

static int teardown_threads(void)
{
    size_t i;

    for (i = 0; i < multi_num_threads; i++)
        if (!TEST_true(wait_for_thread(multi_threads[i])))
            return 0;
    return 1;
}

static int start_threads(size_t n, void (*thread_func)(void))
{
    size_t i;

    if (!TEST_size_t_le(multi_num_threads + n, MAXIMUM_THREADS))
        return 0;

    for (i = 0 ; i < n; i++)
        if (!TEST_true(run_thread(multi_threads + multi_num_threads++, thread_func)))
            return 0;
    return 1;
}

/* Template multi-threaded test function */
static int thread_run_test(void (*main_func)(void),
                           size_t num_threads, void (*thread_func)(void),
                           int libctx, const char *providers[])
{
    int testresult = 0;

    multi_intialise();
    if (!thread_setup_libctx(libctx, providers)
            || !start_threads(num_threads, thread_func))
        goto err;

    if (main_func != NULL)
        main_func();

    if (!teardown_threads()
            || !TEST_true(multi_success))
        goto err;
    testresult = 1;
 err:
    thead_teardown_libctx();
    return testresult;
}

static void thread_general_worker(void)
{
    EVP_MD_CTX *mdctx = EVP_MD_CTX_new();
    EVP_MD *md = EVP_MD_fetch(multi_libctx, "SHA2-256", NULL);
    EVP_CIPHER_CTX *cipherctx = EVP_CIPHER_CTX_new();
    EVP_CIPHER *ciph = EVP_CIPHER_fetch(multi_libctx, "AES-128-CBC", NULL);
    const char *message = "Hello World";
    size_t messlen = strlen(message);
    /* Should be big enough for encryption output too */
    unsigned char out[EVP_MAX_MD_SIZE];
    const unsigned char key[AES_BLOCK_SIZE] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0x0c, 0x0d, 0x0e, 0x0f
    };
    const unsigned char iv[AES_BLOCK_SIZE] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0x0c, 0x0d, 0x0e, 0x0f
    };
    unsigned int mdoutl;
    int ciphoutl;
    EVP_PKEY *pkey = NULL;
    int testresult = 0;
    int i, isfips;

    isfips = OSSL_PROVIDER_available(multi_libctx, "fips");

    if (!TEST_ptr(mdctx)
            || !TEST_ptr(md)
            || !TEST_ptr(cipherctx)
            || !TEST_ptr(ciph))
        goto err;

    /* Do some work */
    for (i = 0; i < 5; i++) {
        if (!TEST_true(EVP_DigestInit_ex(mdctx, md, NULL))
                || !TEST_true(EVP_DigestUpdate(mdctx, message, messlen))
                || !TEST_true(EVP_DigestFinal(mdctx, out, &mdoutl)))
            goto err;
    }
    for (i = 0; i < 5; i++) {
        if (!TEST_true(EVP_EncryptInit_ex(cipherctx, ciph, NULL, key, iv))
                || !TEST_true(EVP_EncryptUpdate(cipherctx, out, &ciphoutl,
                                                (unsigned char *)message,
                                                messlen))
                || !TEST_true(EVP_EncryptFinal(cipherctx, out, &ciphoutl)))
            goto err;
    }

    /*
     * We want the test to run quickly - not securely.
     * Therefore we use an insecure bit length where we can (512).
     * In the FIPS module though we must use a longer length.
     */
    pkey = EVP_PKEY_Q_keygen(multi_libctx, NULL, "RSA", (size_t)(isfips ? 2048 : 512));
    if (!TEST_ptr(pkey))
        goto err;

    testresult = 1;
 err:
    EVP_MD_CTX_free(mdctx);
    EVP_MD_free(md);
    EVP_CIPHER_CTX_free(cipherctx);
    EVP_CIPHER_free(ciph);
    EVP_PKEY_free(pkey);
    if (!testresult)
        multi_set_success(0);
}

static void thread_multi_simple_fetch(void)
{
    EVP_MD *md = EVP_MD_fetch(multi_libctx, "SHA2-256", NULL);

    if (md != NULL)
        EVP_MD_free(md);
    else
        multi_set_success(0);
}

static EVP_PKEY *shared_evp_pkey = NULL;

static void thread_shared_evp_pkey(void)
{
    char *msg = "Hello World";
    unsigned char ctbuf[256];
    unsigned char ptbuf[256];
    size_t ptlen, ctlen = sizeof(ctbuf);
    EVP_PKEY_CTX *ctx = NULL;
    int success = 0;
    int i;

    for (i = 0; i < 1 + do_fips; i++) {
        if (i > 0)
            EVP_PKEY_CTX_free(ctx);
        ctx = EVP_PKEY_CTX_new_from_pkey(multi_libctx, shared_evp_pkey,
                                         i == 0 ? "provider=default"
                                                : "provider=fips");
        if (!TEST_ptr(ctx))
            goto err;

        if (!TEST_int_ge(EVP_PKEY_encrypt_init(ctx), 0)
                || !TEST_int_ge(EVP_PKEY_encrypt(ctx, ctbuf, &ctlen,
                                                (unsigned char *)msg, strlen(msg)),
                                                0))
            goto err;

        EVP_PKEY_CTX_free(ctx);
        ctx = EVP_PKEY_CTX_new_from_pkey(multi_libctx, shared_evp_pkey, NULL);

        if (!TEST_ptr(ctx))
            goto err;

        ptlen = sizeof(ptbuf);
        if (!TEST_int_ge(EVP_PKEY_decrypt_init(ctx), 0)
                || !TEST_int_gt(EVP_PKEY_decrypt(ctx, ptbuf, &ptlen, ctbuf, ctlen),
                                                0)
                || !TEST_mem_eq(msg, strlen(msg), ptbuf, ptlen))
            goto err;
    }

    success = 1;

 err:
    EVP_PKEY_CTX_free(ctx);
    if (!success)
        multi_set_success(0);
}

static void thread_provider_load_unload(void)
{
    OSSL_PROVIDER *deflt = OSSL_PROVIDER_load(multi_libctx, "default");

    if (!TEST_ptr(deflt)
            || !TEST_true(OSSL_PROVIDER_available(multi_libctx, "default")))
        multi_set_success(0);

    OSSL_PROVIDER_unload(deflt);
}

static int test_multi_general_worker_default_provider(void)
{
    return thread_run_test(&thread_general_worker, 2, &thread_general_worker,
                           1, default_provider);
}

static int test_multi_general_worker_fips_provider(void)
{
    if (!do_fips)
        return TEST_skip("FIPS not supported");
    return thread_run_test(&thread_general_worker, 2, &thread_general_worker,
                           1, fips_provider);
}

static int test_multi_fetch_worker(void)
{
    return thread_run_test(&thread_multi_simple_fetch,
                           2, &thread_multi_simple_fetch, 1, default_provider);
}

static int test_multi_shared_pkey_common(void (*worker)(void))
{
    int testresult = 0;

    multi_intialise();
    if (!thread_setup_libctx(1, do_fips ? fips_and_default_providers
                                        : default_provider)
            || !TEST_ptr(shared_evp_pkey = load_pkey_pem(privkey, multi_libctx))
            || !start_threads(1, &thread_shared_evp_pkey)
            || !start_threads(1, worker))
        goto err;

    thread_shared_evp_pkey();

    if (!teardown_threads()
            || !TEST_true(multi_success))
        goto err;
    testresult = 1;
 err:
    EVP_PKEY_free(shared_evp_pkey);
    thead_teardown_libctx();
    return testresult;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
static void thread_downgrade_shared_evp_pkey(void)
{
    /*
     * This test is only relevant for deprecated functions that perform
     * downgrading
     */
    if (EVP_PKEY_get0_RSA(shared_evp_pkey) == NULL)
        multi_set_success(0);
}

static int test_multi_downgrade_shared_pkey(void)
{
    return test_multi_shared_pkey_common(&thread_downgrade_shared_evp_pkey);
}
#endif

static int test_multi_shared_pkey(void)
{
    return test_multi_shared_pkey_common(&thread_shared_evp_pkey);
}

static void thread_release_shared_pkey(void)
{
    OSSL_sleep(0);
    EVP_PKEY_free(shared_evp_pkey);
}

static int test_multi_shared_pkey_release(void)
{
    int testresult = 0;
    size_t i = 1;

    multi_intialise();
    shared_evp_pkey = NULL;
    if (!thread_setup_libctx(1, do_fips ? fips_and_default_providers
                                        : default_provider)
            || !TEST_ptr(shared_evp_pkey = load_pkey_pem(privkey, multi_libctx)))
        goto err;
    for (; i < 10; ++i) {
        if (!TEST_true(EVP_PKEY_up_ref(shared_evp_pkey)))
            goto err;
    }

    if (!start_threads(10, &thread_release_shared_pkey))
        goto err;
    i = 0;

    if (!teardown_threads()
            || !TEST_true(multi_success))
        goto err;
    testresult = 1;
 err:
    while (i > 0) {
        EVP_PKEY_free(shared_evp_pkey);
        --i;
    }
    thead_teardown_libctx();
    return testresult;
}

static int test_multi_load_unload_provider(void)
{
    EVP_MD *sha256 = NULL;
    OSSL_PROVIDER *prov = NULL;
    int testresult = 0;

    multi_intialise();
    if (!thread_setup_libctx(1, NULL)
            || !TEST_ptr(prov = OSSL_PROVIDER_load(multi_libctx, "default"))
            || !TEST_ptr(sha256 = EVP_MD_fetch(multi_libctx, "SHA2-256", NULL))
            || !TEST_true(OSSL_PROVIDER_unload(prov)))
        goto err;
    prov = NULL;

    if (!start_threads(2, &thread_provider_load_unload))
        goto err;

    thread_provider_load_unload();

    if (!teardown_threads()
            || !TEST_true(multi_success))
        goto err;
    testresult = 1;
 err:
    OSSL_PROVIDER_unload(prov);
    EVP_MD_free(sha256);
    thead_teardown_libctx();
    return testresult;
}

static char *multi_load_provider = "legacy";
/*
 * This test attempts to load several providers at the same time, and if
 * run with a thread sanitizer, should crash if the core provider code
 * doesn't synchronize well enough.
 */
static void test_multi_load_worker(void)
{
    OSSL_PROVIDER *prov;

    if (!TEST_ptr(prov = OSSL_PROVIDER_load(multi_libctx, multi_load_provider))
            || !TEST_true(OSSL_PROVIDER_unload(prov)))
        multi_set_success(0);
}

static int test_multi_default(void)
{
    /* Avoid running this test twice */
    if (multidefault_run) {
        TEST_skip("multi default test already run");
        return 1;
    }
    multidefault_run = 1;

    return thread_run_test(&thread_multi_simple_fetch,
                           2, &thread_multi_simple_fetch, 0, default_provider);
}

static int test_multi_load(void)
{
    int res = 1;
    OSSL_PROVIDER *prov;

    /* The multidefault test must run prior to this test */
    if (!multidefault_run) {
        TEST_info("Running multi default test first");
        res = test_multi_default();
    }

    /*
     * We use the legacy provider in test_multi_load_worker because it uses a
     * child libctx that might hit more codepaths that might be sensitive to
     * threading issues. But in a no-legacy build that won't be loadable so
     * we use the default provider instead.
     */
    prov = OSSL_PROVIDER_load(NULL, "legacy");
    if (prov == NULL) {
        TEST_info("Cannot load legacy provider - assuming this is a no-legacy build");
        multi_load_provider = "default";
    }
    OSSL_PROVIDER_unload(prov);

    return thread_run_test(NULL, MAXIMUM_THREADS, &test_multi_load_worker, 0,
                          NULL) && res;
}

static void test_obj_create_one(void)
{
    char tids[12], oid[40], sn[30], ln[30];
    int id = get_new_uid();

    BIO_snprintf(tids, sizeof(tids), "%d", id);
    BIO_snprintf(oid, sizeof(oid), "1.3.6.1.4.1.16604.%s", tids);
    BIO_snprintf(sn, sizeof(sn), "short-name-%s", tids);
    BIO_snprintf(ln, sizeof(ln), "long-name-%s", tids);
    if (!TEST_int_ne(id, 0)
            || !TEST_true(id = OBJ_create(oid, sn, ln))
            || !TEST_true(OBJ_add_sigid(id, NID_sha3_256, NID_rsa)))
        multi_set_success(0);
}

static int test_obj_add(void)
{
    return thread_run_test(&test_obj_create_one,
                           MAXIMUM_THREADS, &test_obj_create_one,
                           1, default_provider);
}

#if !defined(OPENSSL_NO_DGRAM) && !defined(OPENSSL_NO_SOCK)
static BIO *multi_bio1, *multi_bio2;

static void test_bio_dgram_pair_worker(void)
{
    ossl_unused int r;
    int ok = 0;
    uint8_t ch = 0;
    uint8_t scratch[64];
    BIO_MSG msg = {0};
    size_t num_processed = 0;

    if (!TEST_int_eq(RAND_bytes_ex(multi_libctx, &ch, 1, 64), 1))
        goto err;

    msg.data     = scratch;
    msg.data_len = sizeof(scratch);

    /*
     * We do not test for failure here as recvmmsg may fail if no sendmmsg
     * has been called yet. The purpose of this code is to exercise tsan.
     */
    if (ch & 2)
        r = BIO_sendmmsg(ch & 1 ? multi_bio2 : multi_bio1, &msg,
                         sizeof(BIO_MSG), 1, 0, &num_processed);
    else
        r = BIO_recvmmsg(ch & 1 ? multi_bio2 : multi_bio1, &msg,
                         sizeof(BIO_MSG), 1, 0, &num_processed);

    ok = 1;
err:
    if (ok == 0)
        multi_set_success(0);
}

static int test_bio_dgram_pair(void)
{
    int r;
    BIO *bio1 = NULL, *bio2 = NULL;

    r = BIO_new_bio_dgram_pair(&bio1, 0, &bio2, 0);
    if (!TEST_int_eq(r, 1))
        goto err;

    multi_bio1 = bio1;
    multi_bio2 = bio2;

    r  = thread_run_test(&test_bio_dgram_pair_worker,
                         MAXIMUM_THREADS, &test_bio_dgram_pair_worker,
                         1, default_provider);

err:
    BIO_free(bio1);
    BIO_free(bio2);
    return r;
}
#endif

static const char *pemdataraw[] = {
    "-----BEGIN RSA PRIVATE KEY-----\n",
    "MIIBOgIBAAJBAMFcGsaxxdgiuuGmCkVImy4h99CqT7jwY3pexPGcnUFtR2Fh36Bp\n",
    "oncwtkZ4cAgtvd4Qs8PkxUdp6p/DlUmObdkCAwEAAQJAUR44xX6zB3eaeyvTRzms\n",
    "kHADrPCmPWnr8dxsNwiDGHzrMKLN+i/HAam+97HxIKVWNDH2ba9Mf1SA8xu9dcHZ\n",
    "AQIhAOHPCLxbtQFVxlnhSyxYeb7O323c3QulPNn3bhOipElpAiEA2zZpBE8ZXVnL\n",
    "74QjG4zINlDfH+EOEtjJJ3RtaYDugvECIBtsQDxXytChsRgDQ1TcXdStXPcDppie\n",
    "dZhm8yhRTTBZAiAZjE/U9rsIDC0ebxIAZfn3iplWh84yGB3pgUI3J5WkoQIhAInE\n",
    "HTUY5WRj5riZtkyGnbm3DvF+1eMtO2lYV+OuLcfE\n",
    "-----END RSA PRIVATE KEY-----\n",
    NULL
};

static void test_pem_read_one(void)
{
    EVP_PKEY *key = NULL;
    BIO *pem = NULL;
    char *pemdata;
    size_t len;

    pemdata = glue_strings(pemdataraw, &len);
    if (pemdata == NULL) {
        multi_set_success(0);
        goto err;
    }

    pem = BIO_new_mem_buf(pemdata, len);
    if (pem == NULL) {
        multi_set_success(0);
        goto err;
    }

    key = PEM_read_bio_PrivateKey(pem, NULL, NULL, NULL);
    if (key == NULL)
        multi_set_success(0);

 err:
    EVP_PKEY_free(key);
    BIO_free(pem);
    OPENSSL_free(pemdata);
}

/* Test reading PEM files in multiple threads */
static int test_pem_read(void)
{
    return thread_run_test(&test_pem_read_one, MAXIMUM_THREADS,
                           &test_pem_read_one, 1, default_provider);
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_FIPS, OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "fips", OPT_FIPS, '-', "Test the FIPS provider" },
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    char *datadir;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_FIPS:
            do_fips = 1;
            break;
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    if (!TEST_ptr(datadir = test_get_argument(0)))
        return 0;

    privkey = test_mk_file_path(datadir, "rsakey.pem");
    if (!TEST_ptr(privkey))
        return 0;

    if (!TEST_ptr(global_lock = CRYPTO_THREAD_lock_new()))
        return 0;

#ifdef TSAN_REQUIRES_LOCKING
    if (!TEST_ptr(tsan_lock = CRYPTO_THREAD_lock_new()))
        return 0;
#endif

    /* Keep first to validate auto creation of default library context */
    ADD_TEST(test_multi_default);

    ADD_TEST(test_lock);
#if defined(OPENSSL_THREADS)
    ADD_TEST(torture_rw_low);
    ADD_TEST(torture_rw_high);
    ADD_TEST(torture_rcu_low);
    ADD_TEST(torture_rcu_high);
    ADD_TEST(torture_rcu_high2);
#endif
    ADD_TEST(test_once);
    ADD_TEST(test_thread_local);
    ADD_TEST(test_thread_local_multi_key);
    ADD_TEST(test_atomic);
    ADD_TEST(test_multi_load);
    ADD_TEST(test_multi_general_worker_default_provider);
    ADD_TEST(test_multi_general_worker_fips_provider);
    ADD_TEST(test_multi_fetch_worker);
    ADD_TEST(test_multi_shared_pkey);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_multi_downgrade_shared_pkey);
#endif
    ADD_TEST(test_multi_shared_pkey_release);
    ADD_TEST(test_multi_load_unload_provider);
    ADD_TEST(test_obj_add);
#if !defined(OPENSSL_NO_DGRAM) && !defined(OPENSSL_NO_SOCK)
    ADD_TEST(test_bio_dgram_pair);
#endif
    ADD_TEST(test_pem_read);
    return 1;
}

void cleanup_tests(void)
{
    OPENSSL_free(privkey);
#ifdef TSAN_REQUIRES_LOCKING
    CRYPTO_THREAD_lock_free(tsan_lock);
#endif
    CRYPTO_THREAD_lock_free(global_lock);
}
