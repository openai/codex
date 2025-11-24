/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Time tests for the asn1 module */

#include <limits.h>
#include <stdio.h>
#include <string.h>

#include <crypto/asn1.h>
#include <openssl/asn1.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include "testutil.h"
#include "internal/nelem.h"

struct testdata {
    char *data;             /* TIME string value */
    int type;               /* GENERALIZED OR UTC */
    int expected_type;      /* expected type after set/set_string_gmt */
    int check_result;       /* check result */
    time_t t;               /* expected time_t*/
    int cmp_result;         /* comparison to baseline result */
    int convert_result;     /* conversion result */
};

struct TESTDATA_asn1_to_utc {
    char *input;
    time_t expected;
};

static const struct TESTDATA_asn1_to_utc asn1_to_utc[] = {
    {
        /*
         * last second of standard time in central Europe in 2021
         * specified in GMT
         */
        "210328005959Z",
        1616893199,
    },
    {
        /*
         * first second of daylight saving time in central Europe in 2021
         * specified in GMT
         */
        "210328010000Z",
        1616893200,
    },
    {
        /*
         * last second of standard time in central Europe in 2021
         * specified in offset to GMT
         */
        "20210328015959+0100",
        1616893199,
    },
    {
        /*
         * first second of daylight saving time in central Europe in 2021
         * specified in offset to GMT
         */
        "20210328030000+0200",
        1616893200,
    },
    {
        /*
         * Invalid strings should get -1 as a result
         */
        "INVALID",
        -1,
    },
};

static struct testdata tbl_testdata_pos[] = {
    { "0",                 V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, }, /* Bad time */
    { "ABCD",              V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "0ABCD",             V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1-700101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "`9700101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "19700101000000Z",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 0, },
    { "A00101000000Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 0, },
    { "A9700101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1A700101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "19A00101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "197A0101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1970A101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "19700A01000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "197001A1000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1970010A000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "19700101A00000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "197001010A0000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1970010100A000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "19700101000A00Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "197001010000A0Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "1970010100000AZ",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "700101000000X",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 0, },
    { "19700101000000X",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "209912312359Z",     V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "199912310000Z",     V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 0,           0,  0, 0, },
    { "9912312359Z",       V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 0, },
    { "9912310000Z",       V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 0, },
    { "19700101000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,           0, -1, 1, }, /* Epoch begins */
    { "700101000000Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,           0, -1, 1, }, /* ditto */
    { "20380119031407Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,  0x7FFFFFFF,  1, 1, }, /* Max 32bit time_t */
    { "380119031407Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,  0x7FFFFFFF,  1, 1, },
    { "20371231235959Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,  2145916799,  1, 1, }, /* Just before 2038 */
    { "20371231235959Z",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         0,           0,  0, 1, }, /* Bad UTC time */
    { "371231235959Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,  2145916799,  1, 1, },
    { "19701006121456Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,    24063296, -1, 1, },
    { "701006121456Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,    24063296, -1, 1, },
    { "19991231000000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,   946598400,  0, 1, }, /* Match baseline */
    { "991231000000Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "9912310000+0000",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "199912310000+0000", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "9912310000-0000",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "199912310000-0000", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "199912310100+0100", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "199912302300-0100", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "199912302300-A000", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         0,   946598400,  0, 1, },
    { "199912302300-0A00", V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         0,   946598400,  0, 1, },
    { "9912310100+0100",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
    { "9912302300-0100",   V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,   946598400,  0, 1, },
};

/* ASSUMES SIGNED TIME_T */
static struct testdata tbl_testdata_neg[] = {
    { "19011213204552Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 1,     INT_MIN, -1, 0, },
    { "691006121456Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,    -7472704, -1, 1, },
    { "19691006121456Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,    -7472704, -1, 1, },
};

/* explicit casts to time_t short warnings on systems with 32-bit time_t */
static struct testdata tbl_testdata_pos_64bit[] = {
    { "20380119031408Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,  (time_t)0x80000000,  1, 1, },
    { "20380119031409Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_UTCTIME,         1,  (time_t)0x80000001,  1, 1, },
    { "380119031408Z",     V_ASN1_UTCTIME,         V_ASN1_UTCTIME,         1,  (time_t)0x80000000,  1, 1, },
    { "20500101120000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 1,  (time_t)0x967b1ec0,  1, 0, },
};

/* ASSUMES SIGNED TIME_T */
static struct testdata tbl_testdata_neg_64bit[] = {
    { "19011213204551Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 1, (time_t)-2147483649LL, -1, 0, },
    { "19000101120000Z",   V_ASN1_GENERALIZEDTIME, V_ASN1_GENERALIZEDTIME, 1, (time_t)-2208945600LL, -1, 0, },
};

/* A baseline time to compare to */
static ASN1_TIME gtime = {
    15,
    V_ASN1_GENERALIZEDTIME,
    (unsigned char*)"19991231000000Z",
    0
};
static time_t gtime_t = 946598400;

static int test_table(struct testdata *tbl, int idx)
{
    int error = 0;
    ASN1_TIME atime;
    ASN1_TIME *ptime;
    struct testdata *td = &tbl[idx];
    int day, sec;

    atime.data = (unsigned char*)td->data;
    atime.length = strlen((char*)atime.data);
    atime.type = td->type;
    atime.flags = 0;

    if (!TEST_int_eq(ASN1_TIME_check(&atime), td->check_result)) {
        TEST_info("ASN1_TIME_check(%s) unexpected result", atime.data);
        error = 1;
    }
    if (td->check_result == 0)
        return 1;

    if (!TEST_int_eq(ASN1_TIME_cmp_time_t(&atime, td->t), 0)) {
        TEST_info("ASN1_TIME_cmp_time_t(%s vs %ld) compare failed", atime.data, (long)td->t);
        error = 1;
    }

    if (!TEST_true(ASN1_TIME_diff(&day, &sec, &atime, &atime))) {
        TEST_info("ASN1_TIME_diff(%s) to self failed", atime.data);
        error = 1;
    }
    if (!TEST_int_eq(day, 0) || !TEST_int_eq(sec, 0)) {
        TEST_info("ASN1_TIME_diff(%s) to self not equal", atime.data);
        error = 1;
    }

    if (!TEST_true(ASN1_TIME_diff(&day, &sec, &gtime, &atime))) {
        TEST_info("ASN1_TIME_diff(%s) to baseline failed", atime.data);
        error = 1;
    } else if (!((td->cmp_result == 0 && TEST_true((day == 0 && sec == 0))) ||
                 (td->cmp_result == -1 && TEST_true((day < 0 || sec < 0))) ||
                 (td->cmp_result == 1 && TEST_true((day > 0 || sec > 0))))) {
        TEST_info("ASN1_TIME_diff(%s) to baseline bad comparison", atime.data);
        error = 1;
    }

    if (!TEST_int_eq(ASN1_TIME_cmp_time_t(&atime, gtime_t), td->cmp_result)) {
        TEST_info("ASN1_TIME_cmp_time_t(%s) to baseline bad comparison", atime.data);
        error = 1;
    }

    ptime = ASN1_TIME_set(NULL, td->t);
    if (!TEST_ptr(ptime)) {
        TEST_info("ASN1_TIME_set(%ld) failed", (long)td->t);
        error = 1;
    } else {
        int local_error = 0;
        if (!TEST_int_eq(ASN1_TIME_cmp_time_t(ptime, td->t), 0)) {
            TEST_info("ASN1_TIME_set(%ld) compare failed (%s->%s)",
                    (long)td->t, td->data, ptime->data);
            local_error = error = 1;
        }
        if (!TEST_int_eq(ptime->type, td->expected_type)) {
            TEST_info("ASN1_TIME_set(%ld) unexpected type", (long)td->t);
            local_error = error = 1;
        }
        if (local_error)
            TEST_info("ASN1_TIME_set() = %*s", ptime->length, ptime->data);
        ASN1_TIME_free(ptime);
    }

    ptime = ASN1_TIME_new();
    if (!TEST_ptr(ptime)) {
        TEST_info("ASN1_TIME_new() failed");
        error = 1;
    } else {
        int local_error = 0;
        if (!TEST_int_eq(ASN1_TIME_set_string(ptime, td->data), td->check_result)) {
            TEST_info("ASN1_TIME_set_string_gmt(%s) failed", td->data);
            local_error = error = 1;
        }
        if (!TEST_int_eq(ASN1_TIME_normalize(ptime), td->check_result)) {
            TEST_info("ASN1_TIME_normalize(%s) failed", td->data);
            local_error = error = 1;
        }
        if (!TEST_int_eq(ptime->type, td->expected_type)) {
            TEST_info("ASN1_TIME_set_string_gmt(%s) unexpected type", td->data);
            local_error = error = 1;
        }
        day = sec = 0;
        if (!TEST_true(ASN1_TIME_diff(&day, &sec, ptime, &atime)) || !TEST_int_eq(day, 0) || !TEST_int_eq(sec, 0)) {
            TEST_info("ASN1_TIME_diff(day=%d, sec=%d, %s) after ASN1_TIME_set_string_gmt() failed", day, sec, td->data);
            local_error = error = 1;
        }
        if (!TEST_int_eq(ASN1_TIME_cmp_time_t(ptime, gtime_t), td->cmp_result)) {
            TEST_info("ASN1_TIME_cmp_time_t(%s) after ASN1_TIME_set_string_gnt() to baseline bad comparison", td->data);
            local_error = error = 1;
        }
        if (local_error)
            TEST_info("ASN1_TIME_set_string_gmt() = %*s", ptime->length, ptime->data);
        ASN1_TIME_free(ptime);
    }

    ptime = ASN1_TIME_new();
    if (!TEST_ptr(ptime)) {
        TEST_info("ASN1_TIME_new() failed");
        error = 1;
    } else {
        int local_error = 0;
        if (!TEST_int_eq(ASN1_TIME_set_string(ptime, td->data), td->check_result)) {
            TEST_info("ASN1_TIME_set_string(%s) failed", td->data);
            local_error = error = 1;
        }
        day = sec = 0;
        if (!TEST_true(ASN1_TIME_diff(&day, &sec, ptime, &atime)) || !TEST_int_eq(day, 0) || !TEST_int_eq(sec, 0)) {
            TEST_info("ASN1_TIME_diff(day=%d, sec=%d, %s) after ASN1_TIME_set_string() failed", day, sec, td->data);
            local_error = error = 1;
        }
        if (!TEST_int_eq(ASN1_TIME_cmp_time_t(ptime, gtime_t), td->cmp_result)) {
            TEST_info("ASN1_TIME_cmp_time_t(%s) after ASN1_TIME_set_string() to baseline bad comparison", td->data);
            local_error = error = 1;
        }
        if (local_error)
            TEST_info("ASN1_TIME_set_string() = %*s", ptime->length, ptime->data);
        ASN1_TIME_free(ptime);
    }

    if (td->type == V_ASN1_UTCTIME) {
        ptime = ASN1_TIME_to_generalizedtime(&atime, NULL);
        if (td->convert_result == 1 && !TEST_ptr(ptime)) {
            TEST_info("ASN1_TIME_to_generalizedtime(%s) failed", atime.data);
            error = 1;
        } else if (td->convert_result == 0 && !TEST_ptr_null(ptime)) {
            TEST_info("ASN1_TIME_to_generalizedtime(%s) should have failed", atime.data);
            error = 1;
        }
        if (ptime != NULL && !TEST_int_eq(ASN1_TIME_cmp_time_t(ptime, td->t), 0)) {
            TEST_info("ASN1_TIME_to_generalizedtime(%s->%s) bad result", atime.data, ptime->data);
            error = 1;
        }
        ASN1_TIME_free(ptime);
    }
    /* else cannot simply convert GENERALIZEDTIME to UTCTIME */

    if (error)
        TEST_error("atime=%s", atime.data);

    return !error;
}

static int test_table_pos(int idx)
{
    return test_table(tbl_testdata_pos, idx);
}

static int test_table_neg(int idx)
{
    return test_table(tbl_testdata_neg, idx);
}

static int test_table_pos_64bit(int idx)
{
    return test_table(tbl_testdata_pos_64bit, idx);
}

static int test_table_neg_64bit(int idx)
{
    return test_table(tbl_testdata_neg_64bit, idx);
}

struct compare_testdata {
    ASN1_TIME t1;
    ASN1_TIME t2;
    int result;
};

static unsigned char TODAY_GEN_STR[] = "20170825000000Z";
static unsigned char TOMORROW_GEN_STR[] = "20170826000000Z";
static unsigned char TODAY_UTC_STR[] = "170825000000Z";
static unsigned char TOMORROW_UTC_STR[] = "170826000000Z";

#define TODAY_GEN    { sizeof(TODAY_GEN_STR)-1, V_ASN1_GENERALIZEDTIME, TODAY_GEN_STR, 0 }
#define TOMORROW_GEN { sizeof(TOMORROW_GEN_STR)-1, V_ASN1_GENERALIZEDTIME, TOMORROW_GEN_STR, 0 }
#define TODAY_UTC    { sizeof(TODAY_UTC_STR)-1, V_ASN1_UTCTIME, TODAY_UTC_STR, 0 }
#define TOMORROW_UTC { sizeof(TOMORROW_UTC_STR)-1, V_ASN1_UTCTIME, TOMORROW_UTC_STR, 0 }

static struct compare_testdata tbl_compare_testdata[] = {
    { TODAY_GEN,    TODAY_GEN,     0 },
    { TODAY_GEN,    TODAY_UTC,     0 },
    { TODAY_GEN,    TOMORROW_GEN, -1 },
    { TODAY_GEN,    TOMORROW_UTC, -1 },

    { TODAY_UTC,    TODAY_GEN,     0 },
    { TODAY_UTC,    TODAY_UTC,     0 },
    { TODAY_UTC,    TOMORROW_GEN, -1 },
    { TODAY_UTC,    TOMORROW_UTC, -1 },

    { TOMORROW_GEN, TODAY_GEN,     1 },
    { TOMORROW_GEN, TODAY_UTC,     1 },
    { TOMORROW_GEN, TOMORROW_GEN,  0 },
    { TOMORROW_GEN, TOMORROW_UTC,  0 },

    { TOMORROW_UTC, TODAY_GEN,     1 },
    { TOMORROW_UTC, TODAY_UTC,     1 },
    { TOMORROW_UTC, TOMORROW_GEN,  0 },
    { TOMORROW_UTC, TOMORROW_UTC,  0 }
};

static int test_table_compare(int idx)
{
    struct compare_testdata *td = &tbl_compare_testdata[idx];

    return TEST_int_eq(ASN1_TIME_compare(&td->t1, &td->t2), td->result);
}

static int test_time_dup(void)
{
    int ret = 0;
    ASN1_TIME *asn1_time = NULL;
    ASN1_TIME *asn1_time_dup = NULL;
    ASN1_TIME *asn1_gentime = NULL;

    asn1_time = ASN1_TIME_adj(NULL, time(NULL), 0, 0);
    if (asn1_time == NULL) {
        TEST_info("Internal error.");
        goto err;
    }

    asn1_gentime = ASN1_TIME_to_generalizedtime(asn1_time, NULL);
    if (asn1_gentime == NULL) {
        TEST_info("Internal error.");
        goto err;
    }

    asn1_time_dup = ASN1_TIME_dup(asn1_time);
    if (!TEST_ptr_ne(asn1_time_dup, NULL)) {
        TEST_info("ASN1_TIME_dup() failed.");
        goto err;
    }
    if (!TEST_int_eq(ASN1_TIME_compare(asn1_time, asn1_time_dup), 0)) {
        TEST_info("ASN1_TIME_dup() duplicated non-identical value.");
        goto err;
    }
    ASN1_STRING_free(asn1_time_dup);

    asn1_time_dup = ASN1_UTCTIME_dup(asn1_time);
    if (!TEST_ptr_ne(asn1_time_dup, NULL)) {
        TEST_info("ASN1_UTCTIME_dup() failed.");
        goto err;
    }
    if (!TEST_int_eq(ASN1_TIME_compare(asn1_time, asn1_time_dup), 0)) {
        TEST_info("ASN1_UTCTIME_dup() duplicated non-identical UTCTIME value.");
        goto err;
    }
    ASN1_STRING_free(asn1_time_dup);

    asn1_time_dup = ASN1_GENERALIZEDTIME_dup(asn1_gentime);
    if (!TEST_ptr_ne(asn1_time_dup, NULL)) {
        TEST_info("ASN1_GENERALIZEDTIME_dup() failed.");
        goto err;
    }
    if (!TEST_int_eq(ASN1_TIME_compare(asn1_gentime, asn1_time_dup), 0)) {
        TEST_info("ASN1_GENERALIZEDTIME_dup() dup'ed non-identical value.");
        goto err;
    }

    ret = 1;
 err:
    ASN1_STRING_free(asn1_time);
    ASN1_STRING_free(asn1_gentime);
    ASN1_STRING_free(asn1_time_dup);
    return ret;
}

static int convert_asn1_to_time_t(int idx)
{
    time_t testdateutc;

    testdateutc = test_asn1_string_to_time_t(asn1_to_utc[idx].input);

    if (!TEST_time_t_eq(testdateutc, asn1_to_utc[idx].expected)) {
        TEST_info("test_asn1_string_to_time_t (%s) failed: expected %lli, got %lli\n",
                  asn1_to_utc[idx].input,
                  (long long int)asn1_to_utc[idx].expected,
                  (long long int)testdateutc);
        return 0;
    }
    return 1;
}

/*
 * this test is here to exercise ossl_asn1_time_from_tm
 * with an integer year close to INT_MAX.
 */
static int convert_tm_to_asn1_time(void)
{
    /* we need 64 bit time_t */
#if ((ULONG_MAX >> 31) >> 31) >= 1
    time_t t;
    ASN1_TIME *at;

    if (sizeof(time_t) * CHAR_BIT >= 64) {
        t = 67768011791126057ULL;
        at = ASN1_TIME_set(NULL, t);
        /*
         * If ASN1_TIME_set returns NULL, it means it could not handle the input
         * which is fine for this edge case.
         */
        ASN1_STRING_free(at);
    }
#endif
    return 1;
}

int setup_tests(void)
{
    /*
     * On platforms where |time_t| is an unsigned integer, t will be a
     * positive number.
     *
     * We check if we're on a platform with a signed |time_t| with '!(t > 0)'
     * because some compilers are picky if you do 't < 0', or even 't <= 0'
     * if |t| is unsigned.
     */
    time_t t = -1;
    /*
     * On some platforms, |time_t| is signed, but a negative value is an
     * error, and using it with gmtime() or localtime() generates a NULL.
     * If that is the case, we can't perform tests on negative values.
     */
    struct tm *ptm = localtime(&t);

    ADD_ALL_TESTS(test_table_pos, OSSL_NELEM(tbl_testdata_pos));
    if (!(t > 0) && ptm != NULL) {
        TEST_info("Adding negative-sign time_t tests");
        ADD_ALL_TESTS(test_table_neg, OSSL_NELEM(tbl_testdata_neg));
    }
    if (sizeof(time_t) > sizeof(uint32_t)) {
        TEST_info("Adding 64-bit time_t tests");
        ADD_ALL_TESTS(test_table_pos_64bit, OSSL_NELEM(tbl_testdata_pos_64bit));
#ifndef __hpux
        if (!(t > 0) && ptm != NULL) {
            TEST_info("Adding negative-sign 64-bit time_t tests");
            ADD_ALL_TESTS(test_table_neg_64bit, OSSL_NELEM(tbl_testdata_neg_64bit));
        }
#endif
    }
    ADD_ALL_TESTS(test_table_compare, OSSL_NELEM(tbl_compare_testdata));
    ADD_TEST(test_time_dup);
    ADD_ALL_TESTS(convert_asn1_to_time_t, OSSL_NELEM(asn1_to_utc));
    ADD_TEST(convert_tm_to_asn1_time);
    return 1;
}
