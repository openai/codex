/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <limits.h>

#include <openssl/crypto.h>
#include "internal/nelem.h"
#include "crypto/sparse_array.h"
#include "testutil.h"

/* The macros below generate unused functions which error out one of the clang
 * builds.  We disable this check here.
 */
#ifdef __clang__
#pragma clang diagnostic ignored "-Wunused-function"
#endif

DEFINE_SPARSE_ARRAY_OF(char);

static int test_sparse_array(void)
{
    static const struct {
        ossl_uintmax_t n;
        char *v;
    } cases[] = {
        { 22, "a" }, { 0, "z" }, { 1, "b" }, { 290, "c" },
        { INT_MAX, "m" }, { 6666666, "d" }, { (ossl_uintmax_t)-1, "H" },
        { 99, "e" }
    };
    SPARSE_ARRAY_OF(char) *sa;
    size_t i, j;
    int res = 0;

    if (!TEST_ptr(sa = ossl_sa_char_new())
            || !TEST_ptr_null(ossl_sa_char_get(sa, 3))
            || !TEST_ptr_null(ossl_sa_char_get(sa, 0))
            || !TEST_ptr_null(ossl_sa_char_get(sa, UINT_MAX)))
        goto err;

    for (i = 0; i < OSSL_NELEM(cases); i++) {
        if (!TEST_true(ossl_sa_char_set(sa, cases[i].n, cases[i].v))) {
            TEST_note("iteration %zu", i + 1);
            goto err;
        }
        for (j = 0; j <= i; j++)
            if (!TEST_str_eq(ossl_sa_char_get(sa, cases[j].n), cases[j].v)) {
                TEST_note("iteration %zu / %zu", i + 1, j + 1);
                goto err;
            }
    }

    res = 1;
err:
    ossl_sa_char_free(sa);
    return res;
}

static int test_sparse_array_num(void)
{
    static const struct {
        size_t num;
        ossl_uintmax_t n;
        char *v;
    } cases[] = {
        { 1, 22, "a" }, { 2, 1021, "b" }, { 3, 3, "c" }, { 2, 22, NULL },
        { 2, 3, "d" }, { 3, 22, "e" }, { 3, 666, NULL }, { 4, 666, "f" },
        { 3, 3, NULL }, { 2, 22, NULL }, { 1, 666, NULL }, { 2, 64000, "g" },
        { 1, 1021, NULL }, { 0, 64000, NULL }, { 1, 23, "h" }, { 0, 23, NULL }
    };
    SPARSE_ARRAY_OF(char) *sa = NULL;
    size_t i;
    int res = 0;

    if (!TEST_size_t_eq(ossl_sa_char_num(NULL), 0)
            || !TEST_ptr(sa = ossl_sa_char_new())
            || !TEST_size_t_eq(ossl_sa_char_num(sa), 0))
        goto err;
    for (i = 0; i < OSSL_NELEM(cases); i++)
        if (!TEST_true(ossl_sa_char_set(sa, cases[i].n, cases[i].v))
                || !TEST_size_t_eq(ossl_sa_char_num(sa), cases[i].num))
            goto err;
    res = 1;
err:
    ossl_sa_char_free(sa);
    return res;
}

struct index_cases_st {
    ossl_uintmax_t n;
    char *v;
    int del;
};

struct doall_st {
    SPARSE_ARRAY_OF(char) *sa;
    size_t num_cases;
    const struct index_cases_st *cases;
    int res;
    int all;
};

static void leaf_check_all(ossl_uintmax_t n, char *value, void *arg)
{
    struct doall_st *doall_data = (struct doall_st *)arg;
    const struct index_cases_st *cases = doall_data->cases;
    size_t i;

    doall_data->res = 0;
    for (i = 0; i < doall_data->num_cases; i++)
        if ((doall_data->all || !cases[i].del)
            && n == cases[i].n && strcmp(value, cases[i].v) == 0) {
            doall_data->res = 1;
            return;
        }
    TEST_error("Index %ju with value %s not found", n, value);
}

static void leaf_delete(ossl_uintmax_t n, char *value, void *arg)
{
    struct doall_st *doall_data = (struct doall_st *)arg;
    const struct index_cases_st *cases = doall_data->cases;
    size_t i;

    doall_data->res = 0;
    for (i = 0; i < doall_data->num_cases; i++)
        if (n == cases[i].n && strcmp(value, cases[i].v) == 0) {
            doall_data->res = 1;
            ossl_sa_char_set(doall_data->sa, n, NULL);
            return;
        }
    TEST_error("Index %ju with value %s not found", n, value);
}

static int test_sparse_array_doall(void)
{
    static const struct index_cases_st cases[] = {
        { 22, "A", 1 }, { 1021, "b", 0 }, { 3, "c", 0 }, { INT_MAX, "d", 1 },
        { (ossl_uintmax_t)-1, "H", 0 }, { (ossl_uintmax_t)-2, "i", 1 },
        { 666666666, "s", 1 }, { 1234567890, "t", 0 },
    };
    struct doall_st doall_data;
    size_t i;
    SPARSE_ARRAY_OF(char) *sa = NULL;
    int res = 0;

    if (!TEST_ptr(sa = ossl_sa_char_new()))
        goto err;
    doall_data.num_cases = OSSL_NELEM(cases);
    doall_data.cases = cases;
    doall_data.all = 1;
    doall_data.sa = NULL;
    for (i = 0; i <  OSSL_NELEM(cases); i++)
        if (!TEST_true(ossl_sa_char_set(sa, cases[i].n, cases[i].v))) {
            TEST_note("failed at iteration %zu", i + 1);
            goto err;
    }

    ossl_sa_char_doall_arg(sa, &leaf_check_all, &doall_data);
    if (doall_data.res == 0) {
        TEST_info("while checking all elements");
        goto err;
    }
    doall_data.all = 0;
    doall_data.sa = sa;
    ossl_sa_char_doall_arg(sa, &leaf_delete, &doall_data);
    if (doall_data.res == 0) {
        TEST_info("while deleting selected elements");
        goto err;
    }
    ossl_sa_char_doall_arg(sa, &leaf_check_all, &doall_data);
    if (doall_data.res == 0) {
        TEST_info("while checking for deleted elements");
        goto err;
    }
    res = 1;

err:
    ossl_sa_char_free(sa);
    return res;
}

int setup_tests(void)
{
    ADD_TEST(test_sparse_array);
    ADD_TEST(test_sparse_array_num);
    ADD_TEST(test_sparse_array_doall);
    return 1;
}
