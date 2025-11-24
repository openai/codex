/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Tests for X509 time functions */

#include <string.h>
#include <time.h>

#include <openssl/asn1.h>
#include <openssl/x509.h>
#include "testutil.h"
#include "internal/nelem.h"

typedef struct {
    const char *data;
    int type;
    time_t cmp_time;
    /* -1 if asn1_time <= cmp_time, 1 if asn1_time > cmp_time, 0 if error. */
    int expected;
} TESTDATA;

typedef struct {
    const char *data;
    /* 0 for check-only mode, 1 for set-string mode */
    int set_string;
    /* 0 for error, 1 if succeed */
    int expected;
    /*
     * The following 2 fields are ignored if set_string field is set to '0'
     * (in check only mode).
     *
     * But they can still be ignored explicitly in set-string mode by:
     * setting -1 to expected_type and setting NULL to expected_string.
     *
     * It's useful in a case of set-string mode but the expected result
     * is a 'parsing error'.
     */
    int expected_type;
    const char *expected_string;
} TESTDATA_FORMAT;

/*
 * Actually, the "loose" mode has been tested in
 * those time-compare-cases, so we may not test it again.
 */
static TESTDATA_FORMAT x509_format_tests[] = {
    /* GeneralizedTime */
    {
        /* good format, check only */
        "20170217180105Z", 0, 1, -1, NULL,
    },
    {
        /* not leap year, check only */
        "20170229180105Z", 0, 0, -1, NULL,
    },
    {
        /* leap year, check only */
        "20160229180105Z", 0, 1, -1, NULL,
    },
    {
        /* SS is missing, check only */
        "201702171801Z", 0, 0, -1, NULL,
    },
    {
        /* fractional seconds, check only */
        "20170217180105.001Z", 0, 0, -1, NULL,
    },
    {
        /* timezone, check only */
        "20170217180105+0800", 0, 0, -1, NULL,
    },
    {
        /* SS is missing, set string */
        "201702171801Z", 1, 0, -1, NULL,
    },
    {
        /* fractional seconds, set string */
        "20170217180105.001Z", 1, 0, -1, NULL,
    },
    {
        /* timezone, set string */
        "20170217180105+0800", 1, 0, -1, NULL,
    },
    {
        /* good format, check returned 'turned' string */
        "20170217180154Z", 1, 1, V_ASN1_UTCTIME, "170217180154Z",
    },
    {
        /* good format, check returned string */
        "20510217180154Z", 1, 1, V_ASN1_GENERALIZEDTIME, "20510217180154Z",
    },
    {
        /* good format but out of UTC range, check returned string */
        "19230419180154Z", 1, 1, V_ASN1_GENERALIZEDTIME, "19230419180154Z",
    },
    /* UTC */
    {
        /* SS is missing, check only */
        "1702171801Z", 0, 0, -1, NULL,
    },
    {
        /* not leap year, check only */
        "050229180101Z", 0, 0, -1, NULL,
    },
    {
        /* leap year, check only */
        "040229180101Z", 0, 1, -1, NULL,
    },
    {
        /* timezone, check only */
        "170217180154+0800", 0, 0, -1, NULL,
    },
    {
        /* SS is missing, set string */
        "1702171801Z", 1, 0, -1, NULL,
    },
    {
        /* timezone, set string */
        "170217180154+0800", 1, 0, -1, NULL,
    },
    {
        /* 2017, good format, check returned string */
        "170217180154Z", 1, 1, V_ASN1_UTCTIME, "170217180154Z",
    },
    {
        /* 1998, good format, check returned string */
        "981223180154Z", 1, 1, V_ASN1_UTCTIME, "981223180154Z",
    },
};

static TESTDATA x509_cmp_tests[] = {
    {
        "20170217180154Z", V_ASN1_GENERALIZEDTIME,
        /* The same in seconds since epoch. */
        1487354514, -1,
    },
    {
        "20170217180154Z", V_ASN1_GENERALIZEDTIME,
        /* One second more. */
        1487354515, -1,
    },
    {
        "20170217180154Z", V_ASN1_GENERALIZEDTIME,
        /* One second less. */
        1487354513, 1,
    },
    /* Same as UTC time. */
    {
        "170217180154Z", V_ASN1_UTCTIME,
        /* The same in seconds since epoch. */
        1487354514, -1,
    },
    {
        "170217180154Z", V_ASN1_UTCTIME,
        /* One second more. */
        1487354515, -1,
    },
    {
        "170217180154Z", V_ASN1_UTCTIME,
        /* One second less. */
        1487354513, 1,
    },
    /* UTCTime from the 20th century. */
    {
        "990217180154Z", V_ASN1_UTCTIME,
        /* The same in seconds since epoch. */
        919274514, -1,
    },
    {
        "990217180154Z", V_ASN1_UTCTIME,
        /* One second more. */
        919274515, -1,
    },
    {
        "990217180154Z", V_ASN1_UTCTIME,
        /* One second less. */
        919274513, 1,
    },
    /* Various invalid formats. */
    {
        /* No trailing Z. */
        "20170217180154", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* No trailing Z, UTCTime. */
        "170217180154", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* No seconds. */
        "201702171801Z", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* No seconds, UTCTime. */
        "1702171801Z", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Fractional seconds. */
        "20170217180154.001Z", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Fractional seconds, UTCTime. */
        "170217180154.001Z", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Timezone offset. */
        "20170217180154+0100", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Timezone offset, UTCTime. */
        "170217180154+0100", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Extra digits. */
        "2017021718015400Z", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Extra digits, UTCTime. */
        "17021718015400Z", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Non-digits. */
        "2017021718015aZ", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Non-digits, UTCTime. */
        "17021718015aZ", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Trailing garbage. */
        "20170217180154Zlongtrailinggarbage", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Trailing garbage, UTCTime. */
        "170217180154Zlongtrailinggarbage", V_ASN1_UTCTIME, 0, 0,
    },
    {
         /* Swapped type. */
        "20170217180154Z", V_ASN1_UTCTIME, 0, 0,
    },
    {
        /* Swapped type. */
        "170217180154Z", V_ASN1_GENERALIZEDTIME, 0, 0,
    },
    {
        /* Bad type. */
        "20170217180154Z", V_ASN1_OCTET_STRING, 0, 0,
    },
};

static int test_x509_cmp_time(int idx)
{
    ASN1_TIME t;
    int result;

    memset(&t, 0, sizeof(t));
    t.type = x509_cmp_tests[idx].type;
    t.data = (unsigned char*)(x509_cmp_tests[idx].data);
    t.length = strlen(x509_cmp_tests[idx].data);
    t.flags = 0;

    result = X509_cmp_time(&t, &x509_cmp_tests[idx].cmp_time);
    if (!TEST_int_eq(result, x509_cmp_tests[idx].expected)) {
        TEST_info("test_x509_cmp_time(%d) failed: expected %d, got %d\n",
                idx, x509_cmp_tests[idx].expected, result);
        return 0;
    }
    return 1;
}

static int test_x509_cmp_time_current(void)
{
    time_t now = time(NULL);
    /* Pick a day earlier and later, relative to any system clock. */
    ASN1_TIME *asn1_before = NULL, *asn1_after = NULL;
    int cmp_result, failed = 0;

    asn1_before = ASN1_TIME_adj(NULL, now, -1, 0);
    asn1_after = ASN1_TIME_adj(NULL, now, 1, 0);

    cmp_result  = X509_cmp_time(asn1_before, NULL);
    if (!TEST_int_eq(cmp_result, -1))
        failed = 1;

    cmp_result = X509_cmp_time(asn1_after, NULL);
    if (!TEST_int_eq(cmp_result, 1))
        failed = 1;

    ASN1_TIME_free(asn1_before);
    ASN1_TIME_free(asn1_after);

    return failed == 0;
}

static int test_X509_cmp_timeframe_vpm(const X509_VERIFY_PARAM *vpm,
                                       ASN1_TIME *asn1_before,
                                       ASN1_TIME *asn1_mid,
                                       ASN1_TIME *asn1_after)
{
    int always_0 = vpm != NULL
        && (X509_VERIFY_PARAM_get_flags(vpm) & X509_V_FLAG_USE_CHECK_TIME) == 0
        && (X509_VERIFY_PARAM_get_flags(vpm) & X509_V_FLAG_NO_CHECK_TIME) != 0;

    return asn1_before != NULL && asn1_mid != NULL && asn1_after != NULL
        && TEST_int_eq(X509_cmp_timeframe(vpm, asn1_before, asn1_after), 0)
        && TEST_int_eq(X509_cmp_timeframe(vpm, asn1_before, NULL), 0)
        && TEST_int_eq(X509_cmp_timeframe(vpm, NULL, asn1_after), 0)
        && TEST_int_eq(X509_cmp_timeframe(vpm, NULL, NULL), 0)
        && TEST_int_eq(X509_cmp_timeframe(vpm, asn1_after, asn1_after),
                       always_0 ? 0 : -1)
        && TEST_int_eq(X509_cmp_timeframe(vpm, asn1_before, asn1_before),
                       always_0 ? 0 : 1)
        && TEST_int_eq(X509_cmp_timeframe(vpm, asn1_after, asn1_before),
                       always_0 ? 0 : 1);
}

static int test_X509_cmp_timeframe(void)
{
    time_t now = time(NULL);
    ASN1_TIME *asn1_mid = ASN1_TIME_adj(NULL, now, 0, 0);
    /* Pick a day earlier and later, relative to any system clock. */
    ASN1_TIME *asn1_before = ASN1_TIME_adj(NULL, now, -1, 0);
    ASN1_TIME *asn1_after = ASN1_TIME_adj(NULL, now, 1, 0);
    X509_VERIFY_PARAM *vpm = X509_VERIFY_PARAM_new();
    int res = 0;

    if (vpm == NULL)
        goto finish;
    res = test_X509_cmp_timeframe_vpm(NULL, asn1_before, asn1_mid, asn1_after)
        && test_X509_cmp_timeframe_vpm(vpm, asn1_before, asn1_mid, asn1_after);

    X509_VERIFY_PARAM_set_time(vpm, now);
    res = res
        && test_X509_cmp_timeframe_vpm(vpm, asn1_before, asn1_mid, asn1_after)
        && X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_NO_CHECK_TIME)
        && test_X509_cmp_timeframe_vpm(vpm, asn1_before, asn1_mid, asn1_after);

    X509_VERIFY_PARAM_free(vpm);
finish:
    ASN1_TIME_free(asn1_mid);
    ASN1_TIME_free(asn1_before);
    ASN1_TIME_free(asn1_after);

    return res;
}

static int test_x509_time(int idx)
{
    ASN1_TIME *t = NULL;
    int result, rv = 0;

    if (x509_format_tests[idx].set_string) {
        /* set-string mode */
        t = ASN1_TIME_new();
        if (t == NULL) {
            TEST_info("test_x509_time(%d) failed: internal error\n", idx);
            return 0;
        }
    }

    result = ASN1_TIME_set_string_X509(t, x509_format_tests[idx].data);
    /* time string parsing result is always checked against what's expected */
    if (!TEST_int_eq(result, x509_format_tests[idx].expected)) {
        TEST_info("test_x509_time(%d) failed: expected %d, got %d\n",
                idx, x509_format_tests[idx].expected, result);
        goto out;
    }

    /* if t is not NULL but expected_type is ignored(-1), it is an 'OK' case */
    if (t != NULL && x509_format_tests[idx].expected_type != -1) {
        if (!TEST_int_eq(t->type, x509_format_tests[idx].expected_type)) {
            TEST_info("test_x509_time(%d) failed: expected_type %d, got %d\n",
                    idx, x509_format_tests[idx].expected_type, t->type);
            goto out;
        }
    }

    /* if t is not NULL but expected_string is NULL, it is an 'OK' case too */
    if (t != NULL && x509_format_tests[idx].expected_string) {
        if (!TEST_mem_eq((const char *)t->data, t->length,
                    x509_format_tests[idx].expected_string,
                    strlen(x509_format_tests[idx].expected_string))) {
            TEST_info("test_x509_time(%d) failed: expected_string %s, got %.*s\n",
                    idx, x509_format_tests[idx].expected_string, t->length,
                    t->data);
            goto out;
        }
    }

    rv = 1;
out:
    if (t != NULL)
        ASN1_TIME_free(t);
    return rv;
}

static const struct {
    int y, m, d;
    int yd, wd;
} day_of_week_tests[] = {
    /*YYYY  MM  DD  DoY  DoW */
    { 1900,  1,  1,   0, 1 },
    { 1900,  2, 28,  58, 3 },
    { 1900,  3,  1,  59, 4 },
    { 1900, 12, 31, 364, 1 },
    { 1901,  1,  1,   0, 2 },
    { 1970,  1,  1,   0, 4 },
    { 1999,  1, 10,   9, 0 },
    { 1999, 12, 31, 364, 5 },
    { 2000,  1,  1,   0, 6 },
    { 2000,  2, 28,  58, 1 },
    { 2000,  2, 29,  59, 2 },
    { 2000,  3,  1,  60, 3 },
    { 2000, 12, 31, 365, 0 },
    { 2001,  1,  1,   0, 1 },
    { 2008,  1,  1,   0, 2 },
    { 2008,  2, 28,  58, 4 },
    { 2008,  2, 29,  59, 5 },
    { 2008,  3,  1,  60, 6 },
    { 2008, 12, 31, 365, 3 },
    { 2009,  1,  1,   0, 4 },
    { 2011,  1,  1,   0, 6 },
    { 2011,  2, 28,  58, 1 },
    { 2011,  3,  1,  59, 2 },
    { 2011, 12, 31, 364, 6 },
    { 2012,  1,  1,   0, 0 },
    { 2019,  1,  2,   1, 3 },
    { 2019,  2,  2,  32, 6 },
    { 2019,  3,  2,  60, 6 },
    { 2019,  4,  2,  91, 2 },
    { 2019,  5,  2, 121, 4 },
    { 2019,  6,  2, 152, 0 },
    { 2019,  7,  2, 182, 2 },
    { 2019,  8,  2, 213, 5 },
    { 2019,  9,  2, 244, 1 },
    { 2019, 10,  2, 274, 3 },
    { 2019, 11,  2, 305, 6 },
    { 2019, 12,  2, 335, 1 },
    { 2020,  1,  2,   1, 4 },
    { 2020,  2,  2,  32, 0 },
    { 2020,  3,  2,  61, 1 },
    { 2020,  4,  2,  92, 4 },
    { 2020,  5,  2, 122, 6 },
    { 2020,  6,  2, 153, 2 },
    { 2020,  7,  2, 183, 4 },
    { 2020,  8,  2, 214, 0 },
    { 2020,  9,  2, 245, 3 },
    { 2020, 10,  2, 275, 5 },
    { 2020, 11,  2, 306, 1 },
    { 2020, 12,  2, 336, 3 }
};

static int test_days(int n)
{
    char d[16];
    ASN1_TIME *a = NULL;
    struct tm t;
    int r;

    BIO_snprintf(d, sizeof(d), "%04d%02d%02d050505Z",
                 day_of_week_tests[n].y, day_of_week_tests[n].m,
                 day_of_week_tests[n].d);

    if (!TEST_ptr(a = ASN1_TIME_new()))
        return 0;

    r = TEST_true(ASN1_TIME_set_string(a, d))
        && TEST_true(ASN1_TIME_to_tm(a, &t))
        && TEST_int_eq(t.tm_yday, day_of_week_tests[n].yd)
        && TEST_int_eq(t.tm_wday, day_of_week_tests[n].wd);

    ASN1_TIME_free(a);
    return r;
}

#define construct_asn1_time(s, t, e) \
    { { sizeof(s) - 1, t, (unsigned char*)s, 0 }, e }

static const struct {
    ASN1_TIME asn1;
    const char *readable;
} x509_print_tests_rfc_822 [] = {
    /* Generalized Time */
    construct_asn1_time("20170731222050Z", V_ASN1_GENERALIZEDTIME,
            "Jul 31 22:20:50 2017 GMT"),
    /* Generalized Time, no seconds */
    construct_asn1_time("201707312220Z", V_ASN1_GENERALIZEDTIME,
            "Bad time value"),
    /* Generalized Time, fractional seconds (3 digits) */
    construct_asn1_time("20170731222050.123Z", V_ASN1_GENERALIZEDTIME,
            "Jul 31 22:20:50.123 2017 GMT"),
    /* Generalized Time, fractional seconds (1 digit) */
    construct_asn1_time("20170731222050.1Z", V_ASN1_GENERALIZEDTIME,
            "Jul 31 22:20:50.1 2017 GMT"),
    /* Generalized Time, fractional seconds (0 digit) */
    construct_asn1_time("20170731222050.Z", V_ASN1_GENERALIZEDTIME,
            "Bad time value"),
    /* UTC Time */
    construct_asn1_time("170731222050Z", V_ASN1_UTCTIME,
            "Jul 31 22:20:50 2017 GMT"),
    /* UTC Time, no seconds */
    construct_asn1_time("1707312220Z", V_ASN1_UTCTIME,
            "Bad time value"),
};

static const struct {
    ASN1_TIME asn1;
    const char *readable;
} x509_print_tests_iso_8601 [] = {
    /* Generalized Time */
    construct_asn1_time("20170731222050Z", V_ASN1_GENERALIZEDTIME,
            "2017-07-31 22:20:50Z"),
    /* Generalized Time, no seconds */
    construct_asn1_time("201707312220Z", V_ASN1_GENERALIZEDTIME,
            "Bad time value"),
    /* Generalized Time, fractional seconds (3 digits) */
    construct_asn1_time("20170731222050.123Z", V_ASN1_GENERALIZEDTIME,
            "2017-07-31 22:20:50.123Z"),
    /* Generalized Time, fractional seconds (1 digit) */
    construct_asn1_time("20170731222050.1Z", V_ASN1_GENERALIZEDTIME,
            "2017-07-31 22:20:50.1Z"),
    /* Generalized Time, fractional seconds (0 digit) */
    construct_asn1_time("20170731222050.Z", V_ASN1_GENERALIZEDTIME,
            "Bad time value"),
    /* UTC Time */
    construct_asn1_time("170731222050Z", V_ASN1_UTCTIME,
            "2017-07-31 22:20:50Z"),
    /* UTC Time, no seconds */
    construct_asn1_time("1707312220Z", V_ASN1_UTCTIME,
            "Bad time value"),
};

static int test_x509_time_print_rfc_822(int idx)
{
    BIO *m;
    int ret = 0, rv;
    char *pp;
    const char *readable;

    if (!TEST_ptr(m = BIO_new(BIO_s_mem())))
        goto err;

    rv = ASN1_TIME_print_ex(m, &x509_print_tests_rfc_822[idx].asn1, ASN1_DTFLGS_RFC822);
    readable = x509_print_tests_rfc_822[idx].readable;

    if (rv == 0 && !TEST_str_eq(readable, "Bad time value")) {
        /* only if the test case intends to fail... */
        goto err;
    }
    if (!TEST_int_ne(rv = BIO_get_mem_data(m, &pp), 0)
        || !TEST_int_eq(rv, (int)strlen(readable))
        || !TEST_strn_eq(pp, readable, rv))
        goto err;

    ret = 1;
 err:
    BIO_free(m);
    return ret;
}

static int test_x509_time_print_iso_8601(int idx)
{
    BIO *m;
    int ret = 0, rv;
    char *pp;
    const char *readable;

    if (!TEST_ptr(m = BIO_new(BIO_s_mem())))
        goto err;

    rv = ASN1_TIME_print_ex(m, &x509_print_tests_iso_8601[idx].asn1, ASN1_DTFLGS_ISO8601);
    readable = x509_print_tests_iso_8601[idx].readable;

    if (rv == 0 && !TEST_str_eq(readable, "Bad time value")) {
        /* only if the test case intends to fail... */
        goto err;
    }
    if (!TEST_int_ne(rv = BIO_get_mem_data(m, &pp), 0)
        || !TEST_int_eq(rv, (int)strlen(readable))
        || !TEST_strn_eq(pp, readable, rv))
        goto err;

    ret = 1;
 err:
    BIO_free(m);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_x509_cmp_time_current);
    ADD_TEST(test_X509_cmp_timeframe);
    ADD_ALL_TESTS(test_x509_cmp_time, OSSL_NELEM(x509_cmp_tests));
    ADD_ALL_TESTS(test_x509_time, OSSL_NELEM(x509_format_tests));
    ADD_ALL_TESTS(test_days, OSSL_NELEM(day_of_week_tests));
    ADD_ALL_TESTS(test_x509_time_print_rfc_822, OSSL_NELEM(x509_print_tests_rfc_822));
    ADD_ALL_TESTS(test_x509_time_print_iso_8601, OSSL_NELEM(x509_print_tests_iso_8601));
    return 1;
}
