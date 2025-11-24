/*
 * Copyright 2017-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2017, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/lhash.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/crypto.h>
#include <internal/hashtable.h>

#include "internal/nelem.h"
#include "threadstest.h"
#include "testutil.h"

/*
 * The macros below generate unused functions which error out one of the clang
 * builds.  We disable this check here.
 */
#ifdef __clang__
#pragma clang diagnostic ignored "-Wunused-function"
#endif

DEFINE_LHASH_OF_EX(int);

static int int_tests[] = { 65537, 13, 1, 3, -5, 6, 7, 4, -10, -12, -14, 22, 9,
                           -17, 16, 17, -23, 35, 37, 173, 11 };
static const size_t n_int_tests = OSSL_NELEM(int_tests);
static short int_found[OSSL_NELEM(int_tests)];
static short int_not_found;

static unsigned long int int_hash(const int *p)
{
    return 3 & *p;      /* To force collisions */
}

static int int_cmp(const int *p, const int *q)
{
    return *p != *q;
}

static int int_find(int n)
{
    unsigned int i;

    for (i = 0; i < n_int_tests; i++)
        if (int_tests[i] == n)
            return i;
    return -1;
}

static void int_doall(int *v)
{
    const int n = int_find(*v);

    if (n < 0)
        int_not_found++;
    else
        int_found[n]++;
}

static void int_doall_arg(int *p, short *f)
{
    const int n = int_find(*p);

    if (n < 0)
        int_not_found++;
    else
        f[n]++;
}

IMPLEMENT_LHASH_DOALL_ARG(int, short);

static int test_int_lhash(void)
{
    static struct {
        int data;
        int null;
    } dels[] = {
        { 65537,    0 },
        { 173,      0 },
        { 999,      1 },
        { 37,       0 },
        { 1,        0 },
        { 34,       1 }
    };
    const unsigned int n_dels = OSSL_NELEM(dels);
    LHASH_OF(int) *h = lh_int_new(&int_hash, &int_cmp);
    unsigned int i;
    int testresult = 0, j, *p;

    if (!TEST_ptr(h))
        goto end;

    /* insert */
    for (i = 0; i < n_int_tests; i++)
        if (!TEST_ptr_null(lh_int_insert(h, int_tests + i))) {
            TEST_info("int insert %d", i);
            goto end;
        }

    /* num_items */
    if (!TEST_int_eq((size_t)lh_int_num_items(h), n_int_tests))
        goto end;

    /* retrieve */
    for (i = 0; i < n_int_tests; i++)
        if (!TEST_int_eq(*lh_int_retrieve(h, int_tests + i), int_tests[i])) {
            TEST_info("lhash int retrieve value %d", i);
            goto end;
        }
    for (i = 0; i < n_int_tests; i++)
        if (!TEST_ptr_eq(lh_int_retrieve(h, int_tests + i), int_tests + i)) {
            TEST_info("lhash int retrieve address %d", i);
            goto end;
        }
    j = 1;
    if (!TEST_ptr_eq(lh_int_retrieve(h, &j), int_tests + 2))
        goto end;

    /* replace */
    j = 13;
    if (!TEST_ptr(p = lh_int_insert(h, &j)))
        goto end;
    if (!TEST_ptr_eq(p, int_tests + 1))
        goto end;
    if (!TEST_ptr_eq(lh_int_retrieve(h, int_tests + 1), &j))
        goto end;

    /* do_all */
    memset(int_found, 0, sizeof(int_found));
    int_not_found = 0;
    lh_int_doall(h, &int_doall);
    if (!TEST_int_eq(int_not_found, 0)) {
        TEST_info("lhash int doall encountered a not found condition");
        goto end;
    }
    for (i = 0; i < n_int_tests; i++)
        if (!TEST_int_eq(int_found[i], 1)) {
            TEST_info("lhash int doall %d", i);
            goto end;
        }

    /* do_all_arg */
    memset(int_found, 0, sizeof(int_found));
    int_not_found = 0;
    lh_int_doall_short(h, int_doall_arg, int_found);
    if (!TEST_int_eq(int_not_found, 0)) {
        TEST_info("lhash int doall arg encountered a not found condition");
        goto end;
    }
    for (i = 0; i < n_int_tests; i++)
        if (!TEST_int_eq(int_found[i], 1)) {
            TEST_info("lhash int doall arg %d", i);
            goto end;
        }

    /* delete */
    for (i = 0; i < n_dels; i++) {
        const int b = lh_int_delete(h, &dels[i].data) == NULL;
        if (!TEST_int_eq(b ^ dels[i].null,  0)) {
            TEST_info("lhash int delete %d", i);
            goto end;
        }
    }

    /* error */
    if (!TEST_int_eq(lh_int_error(h), 0))
        goto end;

    testresult = 1;
end:
    lh_int_free(h);
    return testresult;
}


static int int_filter_all(HT_VALUE *v, void *arg)
{
    return 1;
}

HT_START_KEY_DEFN(intkey)
HT_DEF_KEY_FIELD(mykey, int)
HT_END_KEY_DEFN(INTKEY)

IMPLEMENT_HT_VALUE_TYPE_FNS(int, test, static)

static int int_foreach(HT_VALUE *v, void *arg)
{
    int *vd = ossl_ht_test_int_from_value(v);
    const int n = int_find(*vd);

    if (n < 0)
        int_not_found++;
    else
        int_found[n]++;
    return 1;
}

static uint64_t hashtable_hash(uint8_t *key, size_t keylen)
{
    return (uint64_t)(*(uint32_t *)key);
}

static int test_int_hashtable(void)
{
    static struct {
        int data;
        int should_del;
    } dels[] = {
        { 65537 , 1},
        { 173 , 1},
        { 999 , 0 },
        { 37 , 1 },
        { 1 , 1 },
        { 34 , 0 }
    };
    const size_t n_dels = OSSL_NELEM(dels);
    HT_CONFIG hash_conf = {
        NULL,
        NULL,
        NULL,
        0,
        1,
    };
    INTKEY key;
    int rc = 0;
    size_t i;
    HT *ht = NULL;
    int todel;
    HT_VALUE_LIST *list = NULL;

    ht = ossl_ht_new(&hash_conf);

    if (ht == NULL)
        return 0;

    /* insert */
    HT_INIT_KEY(&key);
    for (i = 0; i < n_int_tests; i++) {
        HT_SET_KEY_FIELD(&key, mykey, int_tests[i]);
        if (!TEST_int_eq(ossl_ht_test_int_insert(ht, TO_HT_KEY(&key),
                         &int_tests[i], NULL), 1)) {
            TEST_info("int insert %zu", i);
            goto end;
        }
    }

    /* num_items */
    if (!TEST_int_eq((size_t)ossl_ht_count(ht), n_int_tests))
        goto end;

    /* foreach, no arg */
    memset(int_found, 0, sizeof(int_found));
    int_not_found = 0;
    ossl_ht_foreach_until(ht, int_foreach, NULL);
    if (!TEST_int_eq(int_not_found, 0)) {
        TEST_info("hashtable int foreach encountered a not found condition");
        goto end;
    }

    for (i = 0; i < n_int_tests; i++)
        if (!TEST_int_eq(int_found[i], 1)) {
            TEST_info("hashtable int foreach %zu", i);
            goto end;
    }

    /* filter */
    list = ossl_ht_filter(ht, 64, int_filter_all, NULL);
    if (!TEST_int_eq((size_t)list->list_len, n_int_tests))
        goto end;
    ossl_ht_value_list_free(list);

    /* delete */
    for (i = 0; i < n_dels; i++) {
        HT_SET_KEY_FIELD(&key, mykey, dels[i].data);
        todel = ossl_ht_delete(ht, TO_HT_KEY(&key));
        if (dels[i].should_del) {
            if (!TEST_int_eq(todel, 1)) {
                TEST_info("hashtable couldn't find entry %d to delete\n",
                          dels[i].data);
                goto end;
            }
        } else {
            if (!TEST_int_eq(todel, 0)) {
                TEST_info("%d found an entry that shouldn't be there\n", dels[i].data);
                goto end;
            }
       }
    }

    rc = 1;
end:
    ossl_ht_free(ht);
    return rc;
}

static unsigned long int stress_hash(const int *p)
{
    return *p;
}

#ifdef MEASURE_HASH_PERFORMANCE
static int
timeval_subtract (struct timeval *result, struct timeval *x, struct timeval *y)
{
    /* Perform the carry for the later subtraction by updating y. */
    if (x->tv_usec < y->tv_usec) {
        int nsec = (y->tv_usec - x->tv_usec) / 1000000 + 1;
        y->tv_usec -= 1000000 * nsec;
        y->tv_sec += nsec;
    }
    if (x->tv_usec - y->tv_usec > 1000000) {
        int nsec = (x->tv_usec - y->tv_usec) / 1000000;
        y->tv_usec += 1000000 * nsec;
        y->tv_sec -= nsec;
    }

    /*
     * Compute the time remaining to wait.
     * tv_usec is certainly positive.
     */
    result->tv_sec = x->tv_sec - y->tv_sec;
    result->tv_usec = x->tv_usec - y->tv_usec;

    /* Return 1 if result is negative. */
    return x->tv_sec < y->tv_sec;
}
#endif

static int test_stress(void)
{
    LHASH_OF(int) *h = lh_int_new(&stress_hash, &int_cmp);
    const unsigned int n = 2500000;
    unsigned int i;
    int testresult = 0, *p;
#ifdef MEASURE_HASH_PERFORMANCE
    struct timeval start, end, delta;
#endif

    if (!TEST_ptr(h))
        goto end;
#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&start, NULL);
#endif
    /* insert */
    for (i = 0; i < n; i++) {
        p = OPENSSL_malloc(sizeof(i));
        if (!TEST_ptr(p)) {
            TEST_info("lhash stress out of memory %d", i);
            goto end;
        }
        *p = 3 * i + 1;
        lh_int_insert(h, p);
    }

    /* num_items */
    if (!TEST_int_eq(lh_int_num_items(h), n))
            goto end;

    /* delete in a different order */
    for (i = 0; i < n; i++) {
        const int j = (7 * i + 4) % n * 3 + 1;

        if (!TEST_ptr(p = lh_int_delete(h, &j))) {
            TEST_info("lhash stress delete %d\n", i);
            goto end;
        }
        if (!TEST_int_eq(*p, j)) {
            TEST_info("lhash stress bad value %d", i);
            goto end;
        }
        OPENSSL_free(p);
    }

    testresult = 1;
end:
#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&end, NULL);
    timeval_subtract(&delta, &end, &start);
    TEST_info("lhash stress runs in %ld.%ld seconds", delta.tv_sec, delta.tv_usec);
#endif
    lh_int_free(h);
    return testresult;
}

static void hashtable_intfree(HT_VALUE *v)
{
    OPENSSL_free(v->value);
}

static int test_hashtable_stress(int idx)
{
    const unsigned int n = 2500000;
    unsigned int i;
    int testresult = 0, *p;
    HT_CONFIG hash_conf = {
        NULL,              /* use default context */
        hashtable_intfree, /* our free function */
        hashtable_hash,    /* our hash function */
        625000,            /* preset hash size */
        1,                 /* Check collisions */
        0                  /* Lockless reads */
    };
    HT *h;
    INTKEY key;
    HT_VALUE *v;
#ifdef MEASURE_HASH_PERFORMANCE
    struct timeval start, end, delta;
#endif

    hash_conf.lockless_reads = idx;
    h = ossl_ht_new(&hash_conf);


    if (!TEST_ptr(h))
        goto end;
#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&start, NULL);
#endif

    HT_INIT_KEY(&key);

    /* insert */
    for (i = 0; i < n; i++) {
        p = OPENSSL_malloc(sizeof(i));
        if (!TEST_ptr(p)) {
            TEST_info("hashtable stress out of memory %d", i);
            goto end;
        }
        *p = 3 * i + 1;
        HT_SET_KEY_FIELD(&key, mykey, *p);
        if (!TEST_int_eq(ossl_ht_test_int_insert(h, TO_HT_KEY(&key),
                         p, NULL), 1)) {
            TEST_info("hashtable unable to insert element %d\n", *p);
            goto end;
        }
    }

    /* make sure we stored everything */
    if (!TEST_int_eq((size_t)ossl_ht_count(h), n))
            goto end;

    /* delete or get in a different order */
    for (i = 0; i < n; i++) {
        const int j = (7 * i + 4) % n * 3 + 1;
        HT_SET_KEY_FIELD(&key, mykey, j);

        switch (idx) {
        case 0:
            if (!TEST_int_eq((ossl_ht_delete(h, TO_HT_KEY(&key))), 1)) {
                TEST_info("hashtable didn't delete key %d\n", j);
                goto end;
            }
            break;
        case 1:
            if (!TEST_ptr(p = ossl_ht_test_int_get(h, TO_HT_KEY(&key), &v))
                || !TEST_int_eq(*p, j)) {
                TEST_info("hashtable didn't get key %d\n", j);
                goto end;
            }
            break;
        }
    }

    testresult = 1;
end:
#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&end, NULL);
    timeval_subtract(&delta, &end, &start);
    TEST_info("hashtable stress runs in %ld.%ld seconds", delta.tv_sec, delta.tv_usec);
#endif
    ossl_ht_free(h);
    return testresult;
}

typedef struct test_mt_entry {
    int in_table;
    int pending_delete;
} TEST_MT_ENTRY;

static HT *m_ht = NULL;
#define TEST_MT_POOL_SZ 256
#define TEST_THREAD_ITERATIONS 1000000
#define NUM_WORKERS 16

static struct test_mt_entry test_mt_entries[TEST_MT_POOL_SZ];
static char *worker_exits[NUM_WORKERS];

HT_START_KEY_DEFN(mtkey)
HT_DEF_KEY_FIELD(index, uint32_t)
HT_END_KEY_DEFN(MTKEY)

IMPLEMENT_HT_VALUE_TYPE_FNS(TEST_MT_ENTRY, mt, static)

static int worker_num = 0;
static CRYPTO_RWLOCK *worker_lock;
static CRYPTO_RWLOCK *testrand_lock;
static int free_failure = 0;
static int shutting_down = 0;
static int global_iteration = 0;

static void hashtable_mt_free(HT_VALUE *v)
{
    TEST_MT_ENTRY *m = ossl_ht_mt_TEST_MT_ENTRY_from_value(v);
    int pending_delete;
    int ret;

    CRYPTO_atomic_load_int(&m->pending_delete, &pending_delete, worker_lock);

    if (shutting_down == 1)
        return;

    if (pending_delete == 0) {
        TEST_info("Freeing element which was not scheduled for free");
        free_failure = 1;
    } else {
        CRYPTO_atomic_add(&m->pending_delete, -1,
                          &ret, worker_lock);
    }
}

#define DO_LOOKUP 0
#define DO_INSERT 1
#define DO_REPLACE 2
#define DO_DELETE 3
#define NUM_BEHAVIORS (DO_DELETE + 1)

static void do_mt_hash_work(void)
{
    MTKEY key;
    uint32_t index;
    int num;
    TEST_MT_ENTRY *m;
    TEST_MT_ENTRY *expected_m = NULL;
    HT_VALUE *v = NULL;
    TEST_MT_ENTRY **r = NULL;
    int expected_rc;
    int ret;
    char behavior;
    size_t iter = 0;
    int giter;

    CRYPTO_atomic_add(&worker_num, 1, &num, worker_lock);
    num--; /* atomic_add is an add/fetch operation */

    HT_INIT_KEY(&key);

    for (iter = 0; iter < TEST_THREAD_ITERATIONS; iter++) {
        if (!TEST_true(CRYPTO_THREAD_write_lock(testrand_lock)))
            return;
        index = test_random() % TEST_MT_POOL_SZ;
        behavior = (char)(test_random() % NUM_BEHAVIORS);
        CRYPTO_THREAD_unlock(testrand_lock);

        expected_m = &test_mt_entries[index];
        HT_KEY_RESET(&key);
        HT_SET_KEY_FIELD(&key, index, index);

        if (!CRYPTO_atomic_add(&global_iteration, 1, &giter, worker_lock)) {
            worker_exits[num] = "Unable to increment global iterator";
            return;
        }
        switch(behavior) {
        case DO_LOOKUP:
            ossl_ht_read_lock(m_ht);
            m = ossl_ht_mt_TEST_MT_ENTRY_get(m_ht, TO_HT_KEY(&key), &v);
            if (m != NULL && m != expected_m) {
                worker_exits[num] = "Read unexpected value from hashtable";
                TEST_info("Iteration %d Read unexpected value %p when %p expected",
                          giter, (void *)m, (void *)expected_m);
            }
            ossl_ht_read_unlock(m_ht);
            if (worker_exits[num] != NULL)
                return;
            break;
        case DO_INSERT:
        case DO_REPLACE:
            ossl_ht_write_lock(m_ht);
            if (behavior == DO_REPLACE) {
                expected_rc = 1;
                r = &m;
            } else {
                expected_rc = !expected_m->in_table;
                r = NULL;
            }

            if (expected_rc != ossl_ht_mt_TEST_MT_ENTRY_insert(m_ht,
                                                               TO_HT_KEY(&key),
                                                               expected_m, r)) {
                TEST_info("Iteration %d Expected rc %d on %s of element %u which is %s\n",
                          giter, expected_rc, behavior == DO_REPLACE ? "replace" : "insert",
                          (unsigned int)index,
                          expected_m->in_table ? "in table" : "not in table");
                worker_exits[num] = "Failure on insert";
            }
            if (expected_rc == 1)
                expected_m->in_table = 1;
            ossl_ht_write_unlock(m_ht);
            if (worker_exits[num] != NULL)
                return;
            break;
        case DO_DELETE:
            ossl_ht_write_lock(m_ht);
            expected_rc = expected_m->in_table;
            if (expected_rc == 1) {
                /*
                 * We must set pending_delete before the actual deletion
                 * as another inserting or deleting thread can pick up
                 * the delete callback before the ossl_ht_write_unlock() call.
                 * This can happen only if no read locks are pending and
                 * only on Windows where we do not use the write mutex
                 * to get the callback list.
                 */
                expected_m->in_table = 0;
                CRYPTO_atomic_add(&expected_m->pending_delete, 1, &ret, worker_lock);
            }
            if (expected_rc != ossl_ht_delete(m_ht, TO_HT_KEY(&key))) {
                TEST_info("Iteration %d Expected rc %d on delete of element %u which is %s\n",
                          giter, expected_rc, (unsigned int)index,
                          expected_m->in_table ? "in table" : "not in table");
                worker_exits[num] = "Failure on delete";
            }
            ossl_ht_write_unlock(m_ht);
            if (worker_exits[num] != NULL)
                return;
            break;
        default:
            worker_exits[num] = "Undefined behavior specified";
            return;
        }
    }
}

static int test_hashtable_multithread(void)
{
    HT_CONFIG hash_conf = {
        NULL,              /* use default context */
        hashtable_mt_free, /* our free function */
        NULL,              /* default hash function */
        0,                 /* default hash size */
        1,                 /* Check collisions */
    };
    int ret = 0;
    thread_t workers[NUM_WORKERS];
    int i;
#ifdef MEASURE_HASH_PERFORMANCE
    struct timeval start, end, delta;
#endif

    memset(worker_exits, 0, sizeof(char *) * NUM_WORKERS);
    memset(test_mt_entries, 0, sizeof(TEST_MT_ENTRY) * TEST_MT_POOL_SZ);
    memset(workers, 0, sizeof(thread_t) * NUM_WORKERS);

    m_ht = ossl_ht_new(&hash_conf);

    if (!TEST_ptr(m_ht))
        goto end;

    if (!TEST_ptr(worker_lock = CRYPTO_THREAD_lock_new()))
        goto end_free;
    if (!TEST_ptr(testrand_lock = CRYPTO_THREAD_lock_new()))
        goto end_free;
#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&start, NULL);
#endif

    for (i = 0; i < NUM_WORKERS; i++) {
        if (!run_thread(&workers[i], do_mt_hash_work))
            goto shutdown;
    }

shutdown:
    for (--i; i >= 0; i--) {
        wait_for_thread(workers[i]);
    }


    /*
     * Now that the workers are done, check for any error
     * conditions
     */
    ret = 1;
    for (i = 0; i < NUM_WORKERS; i++) {
        if (worker_exits[i] != NULL) {
            TEST_info("Worker %d failed: %s\n", i, worker_exits[i]);
            ret = 0;
        }
    }
    if (free_failure == 1) {
        TEST_info("Encountered a free failure");
        ret = 0;
    }

#ifdef MEASURE_HASH_PERFORMANCE
    gettimeofday(&end, NULL);
    timeval_subtract(&delta, &end, &start);
    TEST_info("multithread stress runs 40000 ops in %ld.%ld seconds", delta.tv_sec, delta.tv_usec);
#endif

end_free:
    shutting_down = 1;
    ossl_ht_free(m_ht);
    CRYPTO_THREAD_lock_free(worker_lock);
    CRYPTO_THREAD_lock_free(testrand_lock);
end:
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_int_lhash);
    ADD_TEST(test_stress);
    ADD_TEST(test_int_hashtable);
    ADD_ALL_TESTS(test_hashtable_stress, 2);
    ADD_TEST(test_hashtable_multithread);
    return 1;
}
