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
#include <openssl/err.h>
#include <openssl/crypto.h>

#include "internal/list.h"
#include "internal/nelem.h"
#include "testutil.h"

typedef struct testl_st TESTL;
struct testl_st {
    int n;
    OSSL_LIST_MEMBER(fizz, TESTL);
    OSSL_LIST_MEMBER(buzz, TESTL);
};

DEFINE_LIST_OF(fizz, TESTL);
DEFINE_LIST_OF(buzz, TESTL);

static int test_fizzbuzz(void)
{
    OSSL_LIST(fizz) a;
    OSSL_LIST(buzz) b;
    TESTL elem[20];
    const int nelem = OSSL_NELEM(elem);
    int i, na = 0, nb = 0;

    ossl_list_fizz_init(&a);
    ossl_list_buzz_init(&b);

    if (!TEST_true(ossl_list_fizz_is_empty(&a)))
        return 0;

    for (i = 1; i < nelem; i++) {
        ossl_list_fizz_init_elem(elem + i);
        ossl_list_buzz_init_elem(elem + i);
        elem[i].n = i;
        if (i % 3 == 0) {
            ossl_list_fizz_insert_tail(&a, elem + i);
            na++;
        }
        if (i % 5 == 0) {
            ossl_list_buzz_insert_head(&b, elem + i);
            nb++;
        }
    }

    if (!TEST_false(ossl_list_fizz_is_empty(&a))
            || !TEST_size_t_eq(ossl_list_fizz_num(&a), na)
            || !TEST_size_t_eq(ossl_list_buzz_num(&b), nb)
            || !TEST_ptr(ossl_list_fizz_head(&a))
            || !TEST_ptr(ossl_list_fizz_tail(&a))
            || !TEST_ptr(ossl_list_buzz_head(&b))
            || !TEST_ptr(ossl_list_buzz_tail(&b))
            || !TEST_int_eq(ossl_list_fizz_head(&a)->n, 3)
            || !TEST_int_eq(ossl_list_fizz_tail(&a)->n, na * 3)
            || !TEST_int_eq(ossl_list_buzz_head(&b)->n, nb * 5)
            || !TEST_int_eq(ossl_list_buzz_tail(&b)->n, 5))
        return 0;
    ossl_list_fizz_remove(&a, ossl_list_fizz_head(&a));
    ossl_list_buzz_remove(&b, ossl_list_buzz_tail(&b));
    if (!TEST_size_t_eq(ossl_list_fizz_num(&a), --na)
            || !TEST_size_t_eq(ossl_list_buzz_num(&b), --nb)
            || !TEST_ptr(ossl_list_fizz_head(&a))
            || !TEST_ptr(ossl_list_buzz_tail(&b))
            || !TEST_int_eq(ossl_list_fizz_head(&a)->n, 6)
            || !TEST_int_eq(ossl_list_buzz_tail(&b)->n, 10)
            || !TEST_ptr(ossl_list_fizz_next(ossl_list_fizz_head(&a)))
            || !TEST_ptr(ossl_list_fizz_prev(ossl_list_fizz_tail(&a)))
            || !TEST_int_eq(ossl_list_fizz_next(ossl_list_fizz_head(&a))->n, 9)
            || !TEST_int_eq(ossl_list_fizz_prev(ossl_list_fizz_tail(&a))->n, 15))
        return 0;
    return 1;
}

typedef struct int_st INTL;
struct int_st {
    int n;
    OSSL_LIST_MEMBER(int, INTL);
};

DEFINE_LIST_OF(int, INTL);

static int test_insert(void)
{
    INTL *c, *d;
    OSSL_LIST(int) l;
    INTL elem[20];
    size_t i;
    int n = 1;

    ossl_list_int_init(&l);
    for (i = 0; i < OSSL_NELEM(elem); i++) {
        ossl_list_int_init_elem(elem + i);
        elem[i].n = i;
    }

    /* Check various insert options - head, tail, middle */
    ossl_list_int_insert_head(&l, elem + 3);                /* 3 */
    ossl_list_int_insert_tail(&l, elem + 6);                /* 3 6 */
    ossl_list_int_insert_before(&l, elem + 6, elem + 5);    /* 3 5 6 */
    ossl_list_int_insert_before(&l, elem + 3, elem + 1);    /* 1 3 5 6 */
    ossl_list_int_insert_after(&l, elem + 1, elem + 2);     /* 1 2 3 5 6 */
    ossl_list_int_insert_after(&l, elem + 6, elem + 7);     /* 1 2 3 5 6 7 */
    ossl_list_int_insert_after(&l, elem + 3, elem + 4);     /* 1 2 3 4 5 6 7 */
    if (!TEST_size_t_eq(ossl_list_int_num(&l), 7))
        return 0;
    c = ossl_list_int_head(&l);
    d = ossl_list_int_tail(&l);
    while (c != NULL && d != NULL) {
        if (!TEST_int_eq(c->n, n) || !TEST_int_eq(d->n, 8 - n))
            return 0;
        c = ossl_list_int_next(c);
        d = ossl_list_int_prev(d);
        n++;
    }
    if (!TEST_ptr_null(c) || !TEST_ptr_null(d))
        return 0;

    /* Check removing head, tail and middle */
    ossl_list_int_remove(&l, elem + 1);                     /* 2 3 4 5 6 7 */
    ossl_list_int_remove(&l, elem + 6);                     /* 2 3 4 5 7 */
    ossl_list_int_remove(&l, elem + 7);                     /* 2 3 4 5 */
    n = 2;
    c = ossl_list_int_head(&l);
    d = ossl_list_int_tail(&l);
    while (c != NULL && d != NULL) {
        if (!TEST_int_eq(c->n, n) || !TEST_int_eq(d->n, 7 - n))
            return 0;
        c = ossl_list_int_next(c);
        d = ossl_list_int_prev(d);
        n++;
    }
    if (!TEST_ptr_null(c) || !TEST_ptr_null(d))
        return 0;

    /* Check removing the head of a two element list works */
    ossl_list_int_remove(&l, elem + 2);                     /* 3 4 5 */
    ossl_list_int_remove(&l, elem + 4);                     /* 3 5 */
    ossl_list_int_remove(&l, elem + 3);                     /* 5 */
    if (!TEST_ptr(ossl_list_int_head(&l))
            || !TEST_ptr(ossl_list_int_tail(&l))
            || !TEST_int_eq(ossl_list_int_head(&l)->n, 5)
            || !TEST_int_eq(ossl_list_int_tail(&l)->n, 5))
        return 0;

    /* Check removing the tail of a two element list works */
    ossl_list_int_insert_head(&l, elem);                    /* 0 5 */
    ossl_list_int_remove(&l, elem + 5);                     /* 0 */
    if (!TEST_ptr(ossl_list_int_head(&l))
            || !TEST_ptr(ossl_list_int_tail(&l))
            || !TEST_int_eq(ossl_list_int_head(&l)->n, 0)
            || !TEST_int_eq(ossl_list_int_tail(&l)->n, 0))
        return 0;

    /* Check removing the only element works */
    ossl_list_int_remove(&l, elem);
    if (!TEST_ptr_null(ossl_list_int_head(&l))
            || !TEST_ptr_null(ossl_list_int_tail(&l)))
        return 0;
    return 1;
}

int setup_tests(void)
{
    ADD_TEST(test_fizzbuzz);
    ADD_TEST(test_insert);
    return 1;
}
