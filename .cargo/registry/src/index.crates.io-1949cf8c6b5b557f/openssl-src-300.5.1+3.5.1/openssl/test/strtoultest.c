/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <internal/nelem.h>
#include "testutil.h"

struct strtoul_test_entry {
    char *input; /* the input string */
    int base; /* the base we are converting in */
    unsigned long expect_val; /* the expected value we should get */
    int expect_err;  /* the expected error we expect to receive */
    size_t expect_endptr_offset; /* the expected endptr offset, +1 for NULL */
};

static struct strtoul_test_entry strtoul_tests[] = {
    /* pass on conv "0" to 0 */
    {
        "0", 0, 0, 1, 1
    },
    /* pass on conv "12345" to 12345 */
    {
        "12345", 0, 12345, 1, 5
    },
    /* pass on conv "0x12345" to 0x12345, base 16 */
    {
        "0x12345", 0, 0x12345, 1, 7
    },
    /* pass on base 10 translation, endptr points to 'x' */
    {
        "0x12345", 10, 0, 1, 1
    },
#if ULONG_MAX == 4294967295
    /* pass on ULONG_MAX translation */
    {
        "4294967295", 0, ULONG_MAX, 1, 10
    },
#else
    {
        "18446744073709551615", 0, ULONG_MAX, 1, 20
    },
#endif

    /* fail on negative input */
    {
        "-1", 0, 0, 0, 0
    },
    /* fail on non-numerical input */
    {
        "abcd", 0, 0, 0, 0
    },
    /* pass on decimal input */
    {
        "1.0", 0, 1, 1, 1
    },
    /* Fail on decimal input without leading number */
    {
        ".1", 0, 0, 0, 0
    }
};

static int test_strtoul(int idx)
{
    unsigned long val;
    char *endptr = NULL;
    int err;
    struct strtoul_test_entry *test = &strtoul_tests[idx];

    /*
     * For each test, convert the string to an unsigned long
     */
    err = OPENSSL_strtoul(test->input, &endptr, test->base, &val);

    /*
     * Check to ensure the error returned is expected
     */
    if (!TEST_int_eq(err, test->expect_err))
        return 0;
    /*
     * Confirm that the endptr points to where we expect
     */
    if (!TEST_ptr_eq(endptr, &test->input[test->expect_endptr_offset]))
        return 0;
    /*
     * And check that we received the proper translated value
     * Note, we only check the value if the conversion passed
     */
    if (test->expect_err == 1) {
        if (!TEST_ulong_eq(val, test->expect_val))
            return 0;
    }
    return 1;

}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_strtoul, OSSL_NELEM(strtoul_tests));
    return 1;
}
