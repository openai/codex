/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <internal/priority_queue.h>
#include <openssl/err.h>
#include <openssl/crypto.h>

#include "internal/nelem.h"
#include "testutil.h"

#define MAX_SAMPLES 500000

DEFINE_PRIORITY_QUEUE_OF(size_t);

static size_t num_rec_freed;

static int size_t_compare(const size_t *a, const size_t *b)
{
    if (*a < *b)
        return -1;
    if (*a > *b)
        return 1;
    return 0;
}

static int qsort_size_t_compare(const void *a, const void *b)
{
    return size_t_compare((size_t *)a, (size_t *)b);
}

static int qsort_size_t_compare_rev(const void *a, const void *b)
{
    return size_t_compare((size_t *)b, (size_t *)a);
}

static void free_checker(ossl_unused size_t *p)
{
    num_rec_freed++;
}

static int test_size_t_priority_queue_int(int reserve, int order, int count,
                                          int remove, int random, int popfree)
{
    PRIORITY_QUEUE_OF(size_t) *pq = NULL;
    static size_t values[MAX_SAMPLES], sorted[MAX_SAMPLES], ref[MAX_SAMPLES];
    size_t n;
    int i, res = 0;
    static const char *orders[3] = { "unordered", "ascending", "descending" };

    TEST_info("testing count %d, %s, %s, values %s, remove %d, %sfree",
              count, orders[order], reserve ? "reserve" : "grow",
              random ? "random" : "deterministic", remove,
              popfree ? "pop " : "");

    if (!TEST_size_t_le(count, MAX_SAMPLES))
        return 0;

    memset(values, 0, sizeof(values));
    memset(sorted, 0, sizeof(sorted));
    memset(ref, 0, sizeof(ref));

    for (i = 0; i < count; i++)
        values[i] = random ? test_random() : (size_t)(count - i);
    memcpy(sorted, values, sizeof(*sorted) * count);
    qsort(sorted, count, sizeof(*sorted), &qsort_size_t_compare);

    if (order == 1)
        memcpy(values, sorted, sizeof(*values) * count);
    else if (order == 2)
        qsort(values, count, sizeof(*values), &qsort_size_t_compare_rev);

    if (!TEST_ptr(pq = ossl_pqueue_size_t_new(&size_t_compare))
            || !TEST_size_t_eq(ossl_pqueue_size_t_num(pq), 0))
        goto err;

    if (reserve && !TEST_true(ossl_pqueue_size_t_reserve(pq, count)))
        goto err;

    for (i = 0; i < count; i++)
        if (!TEST_true(ossl_pqueue_size_t_push(pq, values + i, ref + i)))
            goto err;

    if (!TEST_size_t_eq(*ossl_pqueue_size_t_peek(pq), *sorted)
            || !TEST_size_t_eq(ossl_pqueue_size_t_num(pq), count))
        goto err;

    if (remove) {
        while (remove-- > 0) {
            i = test_random() % count;
            if (values[i] != SIZE_MAX) {
                if (!TEST_ptr_eq(ossl_pqueue_size_t_remove(pq, ref[i]),
                                 values + i))
                    goto err;
                values[i] = SIZE_MAX;
            }
        }
        memcpy(sorted, values, sizeof(*sorted) * count);
        qsort(sorted, count, sizeof(*sorted), &qsort_size_t_compare);
    }
    for (i = 0; ossl_pqueue_size_t_peek(pq) != NULL; i++)
        if (!TEST_size_t_eq(*ossl_pqueue_size_t_peek(pq), sorted[i])
                || !TEST_size_t_eq(*ossl_pqueue_size_t_pop(pq), sorted[i]))
                goto err;

    if (popfree) {
        num_rec_freed = 0;
        n = ossl_pqueue_size_t_num(pq);
        ossl_pqueue_size_t_pop_free(pq, &free_checker);
        pq = NULL;
        if (!TEST_size_t_eq(num_rec_freed, n))
            goto err;
    }
    res = 1;
 err:
    ossl_pqueue_size_t_free(pq);
    return res;
}

static const int test_size_t_priority_counts[] = {
    10, 11, 6, 5, 3, 1, 2, 7500
};

static int test_size_t_priority_queue(int n)
{
    int reserve, order, count, remove, random, popfree;

    count = n % OSSL_NELEM(test_size_t_priority_counts);
    n /= OSSL_NELEM(test_size_t_priority_counts);
    order = n % 3;
    n /= 3;
    random = n % 2;
    n /= 2;
    reserve = n % 2;
    n /= 2;
    remove = n % 6;
    n /= 6;
    popfree = n % 2;

    count = test_size_t_priority_counts[count];
    return test_size_t_priority_queue_int(reserve, order, count, remove,
                                          random, popfree);
}

static int test_large_priority_queue(void)
{
    return test_size_t_priority_queue_int(0, 0, MAX_SAMPLES, MAX_SAMPLES / 100,
                                          1, 1);
}

typedef struct info_st {
    uint64_t seq_num, sub_seq;
    size_t   idx;
} INFO;

DEFINE_PRIORITY_QUEUE_OF(INFO);

static int cmp(const INFO *a, const INFO *b)
{
    if (a->seq_num < b->seq_num)
        return -1;
    if (a->seq_num > b->seq_num)
        return 1;
    if (a->sub_seq < b->sub_seq)
        return -1;
    if (a->sub_seq > b->sub_seq)
        return 1;
    return 0;
}

static int test_22644(void)
{
    size_t i;
    INFO infos[32];
    int res = 0;
    PRIORITY_QUEUE_OF(INFO) *pq = ossl_pqueue_INFO_new(cmp);

    memset(infos, 0, sizeof(infos));
    for (i = 0; i < 32; ++i)
        infos[i].sub_seq = i;

    infos[0].seq_num = 70650219160667140;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[0], &infos[0].idx))
            || !TEST_size_t_eq(infos[0].idx, 7)
            || !TEST_ptr(ossl_pqueue_INFO_remove(pq, infos[0].idx)))
        goto err;

    infos[1].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[1], &infos[1].idx))
            || !TEST_size_t_eq(infos[1].idx, 7)
            || !TEST_ptr(ossl_pqueue_INFO_remove(pq, infos[1].idx)))
        goto err;
    
    infos[2].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[2], &infos[2].idx))
            || !TEST_size_t_eq(infos[2].idx, 7))
        goto err;

    infos[3].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[3], &infos[3].idx))
            || !TEST_size_t_eq(infos[3].idx, 6))
        goto err;

    infos[4].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[4], &infos[4].idx))
            || !TEST_size_t_eq(infos[4].idx, 5))
        goto err;

    infos[5].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[5], &infos[5].idx))
            || !TEST_size_t_eq(infos[5].idx, 4))
        goto err;

    infos[6].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[6], &infos[6].idx))
            || !TEST_size_t_eq(infos[6].idx, 3))
        goto err;

    infos[7].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[7], &infos[7].idx))
            || !TEST_size_t_eq(infos[7].idx, 2))
        goto err;

    infos[8].seq_num = 289360691352306692;
    if (!TEST_true(ossl_pqueue_INFO_push(pq, &infos[8], &infos[8].idx))
            || !TEST_size_t_eq(infos[8].idx, 1))
        goto err;

    if (!TEST_ptr(ossl_pqueue_INFO_pop(pq))
            || !TEST_ptr(ossl_pqueue_INFO_pop(pq)))     /* crash if bug present */
        goto err;
    res = 1;

 err:
    ossl_pqueue_INFO_free(pq);
    return res;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_size_t_priority_queue,
                  OSSL_NELEM(test_size_t_priority_counts)   /* count */
                  * 3                                       /* order */
                  * 2                                       /* random */
                  * 2                                       /* reserve */
                  * 6                                       /* remove */
                  * 2);                                     /* pop & free */
    ADD_TEST(test_large_priority_queue);
    ADD_TEST(test_22644);
    return 1;
}
