/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>

#include "testutil.h"

#define SECS_PER_DAY (24 * 60 * 60)

/*
 * Time checking test code. Check times are identical for a wide range of
 * offsets. This should be run on a machine with 64 bit time_t or it will
 * trigger the very errors the routines fix.
 */

static int check_time(long offset)
{
    struct tm tm1, tm2, o1;
    int off_day, off_sec;
    long toffset;
    time_t t1, t2;

    time(&t1);

    t2 = t1 + offset;
    OPENSSL_gmtime(&t2, &tm2);
    OPENSSL_gmtime(&t1, &tm1);
    o1 = tm1;
    if (!TEST_true(OPENSSL_gmtime_adj(&tm1, 0, offset))
        || !TEST_int_eq(tm1.tm_year, tm2.tm_year)
        || !TEST_int_eq(tm1.tm_mon, tm2.tm_mon)
        || !TEST_int_eq(tm1.tm_mday, tm2.tm_mday)
        || !TEST_int_eq(tm1.tm_hour, tm2.tm_hour)
        || !TEST_int_eq(tm1.tm_min, tm2.tm_min)
        || !TEST_int_eq(tm1.tm_sec, tm2.tm_sec)
        || !TEST_true(OPENSSL_gmtime_diff(&off_day, &off_sec, &o1, &tm1)))
        return 0;
    toffset = (long)off_day * SECS_PER_DAY + off_sec;
    if (!TEST_long_eq(offset, toffset))
        return 0;
    return 1;
}

static int test_gmtime(int offset)
{
    return check_time(offset)
           && check_time(-offset)
           && check_time(offset * 1000L)
           && check_time(-offset * 1000L)
           && check_time(offset * 1000000L)
           && check_time(-offset * 1000000L);
}

int setup_tests(void)
{
    if (sizeof(time_t) < 8)
        TEST_info("Skipping; time_t is less than 64-bits");
    else
        ADD_ALL_TESTS_NOSUBTEST(test_gmtime, 1000);
    return 1;
}
