/*
 * Copyright 2014-2018 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>

#include "internal/nelem.h"
#include "internal/constant_time.h"
#include "testutil.h"
#include "internal/numbers.h"

static const unsigned int CONSTTIME_TRUE = (unsigned)(~0);
static const unsigned int CONSTTIME_FALSE = 0;
static const unsigned char CONSTTIME_TRUE_8 = 0xff;
static const unsigned char CONSTTIME_FALSE_8 = 0;
static const size_t CONSTTIME_TRUE_S = ~((size_t)0);
static const size_t CONSTTIME_FALSE_S = 0;
static uint32_t CONSTTIME_TRUE_32 = (uint32_t)(~(uint32_t)0);
static uint32_t CONSTTIME_FALSE_32 = 0;
static uint64_t CONSTTIME_TRUE_64 = (uint64_t)(~(uint64_t)0);
static uint64_t CONSTTIME_FALSE_64 = 0;

static unsigned int test_values[] = {
    0, 1, 1024, 12345, 32000, UINT_MAX / 2 - 1,
    UINT_MAX / 2, UINT_MAX / 2 + 1, UINT_MAX - 1,
    UINT_MAX
};

static unsigned char test_values_8[] = {
    0, 1, 2, 20, 32, 127, 128, 129, 255
};

static int signed_test_values[] = {
    0, 1, -1, 1024, -1024, 12345, -12345,
    32000, -32000, INT_MAX, INT_MIN, INT_MAX - 1,
    INT_MIN + 1
};

static size_t test_values_s[] = {
    0, 1, 1024, 12345, 32000, SIZE_MAX / 2 - 1,
    SIZE_MAX / 2, SIZE_MAX / 2 + 1, SIZE_MAX - 1,
    SIZE_MAX
};

static uint32_t test_values_32[] = {
    0, 1, 1024, 12345, 32000, UINT32_MAX / 2, UINT32_MAX / 2 + 1,
    UINT32_MAX - 1, UINT32_MAX
};

static uint64_t test_values_64[] = {
    0, 1, 1024, 12345, 32000, 32000000, 32000000001, UINT64_MAX / 2,
    UINT64_MAX / 2 + 1, UINT64_MAX - 1, UINT64_MAX
};

static int test_binary_op(unsigned int (*op) (unsigned int a, unsigned int b),
                          const char *op_name, unsigned int a, unsigned int b,
                          int is_true)
{
    if (is_true && !TEST_uint_eq(op(a, b), CONSTTIME_TRUE))
        return 0;
    if (!is_true && !TEST_uint_eq(op(a, b), CONSTTIME_FALSE))
        return 0;
    return 1;
}

static int test_binary_op_8(unsigned
                            char (*op) (unsigned int a, unsigned int b),
                            const char *op_name, unsigned int a,
                            unsigned int b, int is_true)
{
    if (is_true && !TEST_uint_eq(op(a, b), CONSTTIME_TRUE_8))
        return 0;
    if (!is_true && !TEST_uint_eq(op(a, b), CONSTTIME_FALSE_8))
        return 0;
    return 1;
}

static int test_binary_op_s(size_t (*op) (size_t a, size_t b),
                            const char *op_name, size_t a, size_t b,
                            int is_true)
{
    if (is_true && !TEST_size_t_eq(op(a, b), CONSTTIME_TRUE_S))
        return 0;
    if (!is_true && !TEST_uint_eq(op(a, b), CONSTTIME_FALSE_S))
        return 0;
    return 1;
}

static int test_binary_op_64(uint64_t (*op)(uint64_t a, uint64_t b),
                             const char *op_name, uint64_t a, uint64_t b,
                             int is_true)
{
    uint64_t c = op(a, b);

    if (is_true && c != CONSTTIME_TRUE_64) {
        TEST_error("TRUE %s op failed", op_name);
        BIO_printf(bio_err, "a=%jx b=%jx\n", a, b);
        return 0;
    } else if (!is_true && c != CONSTTIME_FALSE_64) {
        TEST_error("FALSE %s op failed", op_name);
        BIO_printf(bio_err, "a=%jx b=%jx\n", a, b);
        return 0;
    }
    return 1;
}

static int test_is_zero(int i)
{
    unsigned int a = test_values[i];

    if (a == 0 && !TEST_uint_eq(constant_time_is_zero(a), CONSTTIME_TRUE))
        return 0;
    if (a != 0 && !TEST_uint_eq(constant_time_is_zero(a), CONSTTIME_FALSE))
        return 0;
    return 1;
}

static int test_is_zero_8(int i)
{
    unsigned int a = test_values_8[i];

    if (a == 0 && !TEST_uint_eq(constant_time_is_zero_8(a), CONSTTIME_TRUE_8))
        return 0;
    if (a != 0 && !TEST_uint_eq(constant_time_is_zero_8(a), CONSTTIME_FALSE_8))
        return 0;
    return 1;
}

static int test_is_zero_32(int i)
{
    uint32_t a = test_values_32[i];

    if (a == 0 && !TEST_true(constant_time_is_zero_32(a) == CONSTTIME_TRUE_32))
        return 0;
    if (a != 0 && !TEST_true(constant_time_is_zero_32(a) == CONSTTIME_FALSE_32))
        return 0;
    return 1;
}

static int test_is_zero_s(int i)
{
    size_t a = test_values_s[i];

    if (a == 0 && !TEST_size_t_eq(constant_time_is_zero_s(a), CONSTTIME_TRUE_S))
        return 0;
    if (a != 0 && !TEST_uint_eq(constant_time_is_zero_s(a), CONSTTIME_FALSE_S))
        return 0;
    return 1;
}

static int test_select(unsigned int a, unsigned int b)
{
    if (!TEST_uint_eq(constant_time_select(CONSTTIME_TRUE, a, b), a))
        return 0;
    if (!TEST_uint_eq(constant_time_select(CONSTTIME_FALSE, a, b), b))
        return 0;
    return 1;
}

static int test_select_8(unsigned char a, unsigned char b)
{
    if (!TEST_uint_eq(constant_time_select_8(CONSTTIME_TRUE_8, a, b), a))
        return 0;
    if (!TEST_uint_eq(constant_time_select_8(CONSTTIME_FALSE_8, a, b), b))
        return 0;
    return 1;
}

static int test_select_32(uint32_t a, uint32_t b)
{
    if (!TEST_true(constant_time_select_32(CONSTTIME_TRUE_32, a, b) == a))
        return 0;
    if (!TEST_true(constant_time_select_32(CONSTTIME_FALSE_32, a, b) == b))
        return 0;
    return 1;
}

static int test_select_s(size_t a, size_t b)
{
    if (!TEST_uint_eq(constant_time_select_s(CONSTTIME_TRUE_S, a, b), a))
        return 0;
    if (!TEST_uint_eq(constant_time_select_s(CONSTTIME_FALSE_S, a, b), b))
        return 0;
    return 1;
}

static int test_select_64(uint64_t a, uint64_t b)
{
    uint64_t selected = constant_time_select_64(CONSTTIME_TRUE_64, a, b);

    if (selected != a) {
        TEST_error("test_select_64 TRUE failed");
        BIO_printf(bio_err, "a=%jx b=%jx got %jx wanted a\n", a, b, selected);
        return 0;
    }
    selected = constant_time_select_64(CONSTTIME_FALSE_64, a, b);
    if (selected != b) {
        BIO_printf(bio_err, "a=%jx b=%jx got %jx wanted b\n", a, b, selected);
        return 0;
    }
    return 1;
}

static int test_select_int(int a, int b)
{
    if (!TEST_int_eq(constant_time_select_int(CONSTTIME_TRUE, a, b), a))
        return 0;
    if (!TEST_int_eq(constant_time_select_int(CONSTTIME_FALSE, a, b), b))
        return 0;
    return 1;
}

static int test_eq_int_8(int a, int b)
{
    if (a == b && !TEST_int_eq(constant_time_eq_int_8(a, b), CONSTTIME_TRUE_8))
        return 0;
    if (a != b && !TEST_int_eq(constant_time_eq_int_8(a, b), CONSTTIME_FALSE_8))
        return 0;
    return 1;
}

static int test_eq_s(size_t a, size_t b)
{
    if (a == b && !TEST_size_t_eq(constant_time_eq_s(a, b), CONSTTIME_TRUE_S))
        return 0;
    if (a != b && !TEST_int_eq(constant_time_eq_s(a, b), CONSTTIME_FALSE_S))
        return 0;
    return 1;
}

static int test_eq_int(int a, int b)
{
    if (a == b && !TEST_uint_eq(constant_time_eq_int(a, b), CONSTTIME_TRUE))
        return 0;
    if (a != b && !TEST_uint_eq(constant_time_eq_int(a, b), CONSTTIME_FALSE))
        return 0;
    return 1;
}

static int test_sizeofs(void)
{
    if (!TEST_uint_eq(OSSL_NELEM(test_values), OSSL_NELEM(test_values_s)))
        return 0;
    return 1;
}

static int test_binops(int i)
{
    unsigned int a = test_values[i];
    int j;
    int ret = 1;

    for (j = 0; j < (int)OSSL_NELEM(test_values); ++j) {
        unsigned int b = test_values[j];

        if (!test_select(a, b)
                || !test_binary_op(&constant_time_lt, "ct_lt",
                                   a, b, a < b)
                || !test_binary_op(&constant_time_lt, "constant_time_lt",
                                   b, a, b < a)
                || !test_binary_op(&constant_time_ge, "constant_time_ge",
                                   a, b, a >= b)
                || !test_binary_op(&constant_time_ge, "constant_time_ge",
                                   b, a, b >= a)
                || !test_binary_op(&constant_time_eq, "constant_time_eq",
                                   a, b, a == b)
                || !test_binary_op(&constant_time_eq, "constant_time_eq",
                                   b, a, b == a))
            ret = 0;
    }
    return ret;
}

static int test_binops_8(int i)
{
    unsigned int a = test_values_8[i];
    int j;
    int ret = 1;

    for (j = 0; j < (int)OSSL_NELEM(test_values_8); ++j) {
        unsigned int b = test_values_8[j];

        if (!test_binary_op_8(&constant_time_lt_8, "constant_time_lt_8",
                                     a, b, a < b)
                || !test_binary_op_8(&constant_time_lt_8, "constant_time_lt_8",
                                     b, a, b < a)
                || !test_binary_op_8(&constant_time_ge_8, "constant_time_ge_8",
                                     a, b, a >= b)
                || !test_binary_op_8(&constant_time_ge_8, "constant_time_ge_8",
                                     b, a, b >= a)
                || !test_binary_op_8(&constant_time_eq_8, "constant_time_eq_8",
                                     a, b, a == b)
                || !test_binary_op_8(&constant_time_eq_8, "constant_time_eq_8",
                                     b, a, b == a))
            ret = 0;
    }
    return ret;
}

static int test_binops_s(int i)
{
    size_t a = test_values_s[i];
    int j;
    int ret = 1;

    for (j = 0; j < (int)OSSL_NELEM(test_values_s); ++j) {
        size_t b = test_values_s[j];

        if (!test_select_s(a, b)
                || !test_eq_s(a, b)
                || !test_binary_op_s(&constant_time_lt_s, "constant_time_lt_s",
                                     a, b, a < b)
                || !test_binary_op_s(&constant_time_lt_s, "constant_time_lt_s",
                                     b, a, b < a)
                || !test_binary_op_s(&constant_time_ge_s, "constant_time_ge_s",
                                     a, b, a >= b)
                || !test_binary_op_s(&constant_time_ge_s, "constant_time_ge_s",
                                     b, a, b >= a)
                || !test_binary_op_s(&constant_time_eq_s, "constant_time_eq_s",
                                     a, b, a == b)
                || !test_binary_op_s(&constant_time_eq_s, "constant_time_eq_s",
                                     b, a, b == a))
            ret = 0;
    }
    return ret;
}

static int test_signed(int i)
{
    int c = signed_test_values[i];
    unsigned int j;
    int ret = 1;

    for (j = 0; j < OSSL_NELEM(signed_test_values); ++j) {
        int d = signed_test_values[j];

        if (!test_select_int(c, d)
                || !test_eq_int(c, d)
                || !test_eq_int_8(c, d))
            ret = 0;
    }
    return ret;
}

static int test_8values(int i)
{
    unsigned char e = test_values_8[i];
    unsigned int j;
    int ret = 1;

    for (j = 0; j < sizeof(test_values_8); ++j) {
        unsigned char f = test_values_8[j];

        if (!test_select_8(e, f))
            ret = 0;
    }
    return ret;
}

static int test_32values(int i)
{
    uint32_t e = test_values_32[i];
    size_t j;
    int ret = 1;

    for (j = 0; j < OSSL_NELEM(test_values_32); j++) {
        uint32_t f = test_values_32[j];

        if (!test_select_32(e, f))
            ret = 0;
    }
    return ret;
}

static int test_64values(int i)
{
    uint64_t g = test_values_64[i];
    int j, ret = 1;

    for (j = i + 1; j < (int)OSSL_NELEM(test_values_64); j++) {
        uint64_t h = test_values_64[j];

        if (!test_binary_op_64(&constant_time_lt_64, "constant_time_lt_64",
                               g, h, g < h)
                || !test_select_64(g, h)) {
            TEST_info("test_64values failed i=%d j=%d", i, j);
            ret = 0;
        }
    }
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_sizeofs);
    ADD_ALL_TESTS(test_is_zero, OSSL_NELEM(test_values));
    ADD_ALL_TESTS(test_is_zero_8, OSSL_NELEM(test_values_8));
    ADD_ALL_TESTS(test_is_zero_32, OSSL_NELEM(test_values_32));
    ADD_ALL_TESTS(test_is_zero_s, OSSL_NELEM(test_values_s));
    ADD_ALL_TESTS(test_binops, OSSL_NELEM(test_values));
    ADD_ALL_TESTS(test_binops_8, OSSL_NELEM(test_values_8));
    ADD_ALL_TESTS(test_binops_s, OSSL_NELEM(test_values_s));
    ADD_ALL_TESTS(test_signed, OSSL_NELEM(signed_test_values));
    ADD_ALL_TESTS(test_8values, OSSL_NELEM(test_values_8));
    ADD_ALL_TESTS(test_32values, OSSL_NELEM(test_values_32));
    ADD_ALL_TESTS(test_64values, OSSL_NELEM(test_values_64));
    return 1;
}
