/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * This program tests the use of OSSL_PARAM, currently in raw form.
 */

#include "internal/nelem.h"
#include "internal/cryptlib.h"
#include "testutil.h"

struct testdata {
    const char *in;
    const unsigned char *expected;
    size_t expected_len;
    const char sep;
};

static const unsigned char test_1[] = { 0xAB, 0xCD, 0xEF, 0xF1 };
static const unsigned char test_2[] = { 0xAB, 0xCD, 0xEF, 0x76, 0x00 };

static struct testdata tbl_testdata[] = {
    {
        "AB:CD:EF:F1",
        test_1, sizeof(test_1),
        ':',
    },
    {
        "AB:CD:EF:76:00",
        test_2, sizeof(test_2),
        ':',
    },
    {
        "AB_CD_EF_F1",
        test_1, sizeof(test_1),
        '_',
    },
    {
        "AB_CD_EF_76_00",
        test_2, sizeof(test_2),
        '_',
    },
    {
        "ABCDEFF1",
        test_1, sizeof(test_1),
        '\0',
    },
    {
        "ABCDEF7600",
        test_2, sizeof(test_2),
        '\0',
    },
};

static int test_hexstr_sep_to_from(int test_index)
{
    int ret = 0;
    long len = 0;
    unsigned char *buf = NULL;
    char *out = NULL;
    struct testdata *test = &tbl_testdata[test_index];

    if (!TEST_ptr(buf = ossl_hexstr2buf_sep(test->in, &len, test->sep))
        || !TEST_mem_eq(buf, len, test->expected, test->expected_len)
        || !TEST_ptr(out = ossl_buf2hexstr_sep(buf, len, test->sep))
        || !TEST_str_eq(out, test->in))
       goto err;

    ret = 1;
err:
    OPENSSL_free(buf);
    OPENSSL_free(out);
    return ret;
}

static int test_hexstr_to_from(int test_index)
{
    int ret = 0;
    long len = 0;
    unsigned char *buf = NULL;
    char *out = NULL;
    struct testdata *test = &tbl_testdata[test_index];

    if (test->sep != '_') {
        if (!TEST_ptr(buf = OPENSSL_hexstr2buf(test->in, &len))
            || !TEST_mem_eq(buf, len, test->expected, test->expected_len)
            || !TEST_ptr(out = OPENSSL_buf2hexstr(buf, len)))
           goto err;
        if (test->sep == ':') {
            if (!TEST_str_eq(out, test->in))
                goto err;
        } else if (!TEST_str_ne(out, test->in)) {
            goto err;
        }
    } else {
        if (!TEST_ptr_null(buf = OPENSSL_hexstr2buf(test->in, &len)))
            goto err;
    }
    ret = 1;
err:
    OPENSSL_free(buf);
    OPENSSL_free(out);
    return ret;
}

static int test_hexstr_ex_to_from(int test_index)
{
    size_t len = 0;
    char out[64];
    unsigned char buf[64];
    struct testdata *test = &tbl_testdata[test_index];

    return TEST_true(OPENSSL_hexstr2buf_ex(buf, sizeof(buf), &len, test->in, ':'))
           && TEST_mem_eq(buf, len, test->expected, test->expected_len)
           && TEST_false(OPENSSL_buf2hexstr_ex(out, 3 * len - 1, NULL, buf, len,
                                               ':'))
           && TEST_true(OPENSSL_buf2hexstr_ex(out, sizeof(out), NULL, buf, len,
                                              ':'))
           && TEST_str_eq(out, test->in)
           && TEST_true(OPENSSL_buf2hexstr_ex(out, sizeof(out), NULL, buf, 0,
                                              ':'))
           && TEST_size_t_eq(strlen(out), 0);
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_hexstr_sep_to_from, OSSL_NELEM(tbl_testdata));
    ADD_ALL_TESTS(test_hexstr_to_from, OSSL_NELEM(tbl_testdata));
    ADD_ALL_TESTS(test_hexstr_ex_to_from, 2);
    return 1;
}
