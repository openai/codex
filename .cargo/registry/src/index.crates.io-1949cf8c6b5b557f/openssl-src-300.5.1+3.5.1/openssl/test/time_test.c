/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "testutil.h"
#include "internal/time.h"

static int test_time_to_timeval(void)
{
    OSSL_TIME a;
    struct timeval tv;

    a = ossl_time_zero();

    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 0))
        return 0;

    /* Test that zero round trips */
    if (!TEST_true(ossl_time_is_zero(ossl_time_from_timeval(tv))))
        return 0;

    /* We should round up nano secs to the next usec */
    a = ossl_ticks2time(1);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 1))
        return 0;
    a = ossl_ticks2time(999);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 1))
        return 0;
    a = ossl_ticks2time(1000);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 1))
        return 0;
    a = ossl_ticks2time(1001);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 2))
        return 0;
    a = ossl_ticks2time(999000);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 0) || !TEST_long_eq(tv.tv_usec, 999))
        return 0;
    a = ossl_ticks2time(999999001);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 1) || !TEST_long_eq(tv.tv_usec, 0))
        return 0;
    a = ossl_ticks2time(999999999);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 1) || !TEST_long_eq(tv.tv_usec, 0))
        return 0;
    a = ossl_ticks2time(1000000000);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 1) || !TEST_long_eq(tv.tv_usec, 0))
        return 0;
    a = ossl_ticks2time(1000000001);
    tv = ossl_time_to_timeval(a);
    if (!TEST_long_eq(tv.tv_sec, 1) || !TEST_long_eq(tv.tv_usec, 1))
        return 0;

    /*
     * Note that we don't currently support infinity round tripping. Instead
     * callers need to explicitly test for infinity.
     */

    return 1;
}

int setup_tests(void)
{
    ADD_TEST(test_time_to_timeval);

    return 1;
}
