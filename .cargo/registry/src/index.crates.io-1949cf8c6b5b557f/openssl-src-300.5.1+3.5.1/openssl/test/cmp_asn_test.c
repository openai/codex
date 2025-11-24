/*
 * Copyright 2007-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

static unsigned char rand_data[OSSL_CMP_TRANSACTIONID_LENGTH];

typedef struct test_fixture {
    const char *test_case_name;
    int expected;
    ASN1_OCTET_STRING *src_string;
    ASN1_OCTET_STRING *tgt_string;

} CMP_ASN_TEST_FIXTURE;

static CMP_ASN_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_ASN_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    return fixture;
}

static void tear_down(CMP_ASN_TEST_FIXTURE *fixture)
{
    ASN1_OCTET_STRING_free(fixture->src_string);
    if (fixture->tgt_string != fixture->src_string)
        ASN1_OCTET_STRING_free(fixture->tgt_string);

    OPENSSL_free(fixture);
}

static int execute_cmp_asn1_get_int_test(CMP_ASN_TEST_FIXTURE *fixture)
{
    int res = 0;
    ASN1_INTEGER *asn1integer = ASN1_INTEGER_new();
    const int good_int = 77;
    const int64_t max_int = INT_MAX;

    if (!TEST_ptr(asn1integer))
        return res;

    if (!TEST_true(ASN1_INTEGER_set(asn1integer, good_int))) {
        ASN1_INTEGER_free(asn1integer);
        return 0;
    }
    res = TEST_int_eq(good_int, ossl_cmp_asn1_get_int(asn1integer));
    if (res == 0)
        goto err;

    res = 0;
    if (!TEST_true(ASN1_INTEGER_set_int64(asn1integer, max_int + 1)))
        goto err;
    res = TEST_int_eq(-2, ossl_cmp_asn1_get_int(asn1integer));

 err:
    ASN1_INTEGER_free(asn1integer);
    return res;
}

static int test_cmp_asn1_get_int(void)
{
    SETUP_TEST_FIXTURE(CMP_ASN_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_cmp_asn1_get_int_test, tear_down);
    return result;
}

static int execute_CMP_ASN1_OCTET_STRING_set1_test(CMP_ASN_TEST_FIXTURE *
                                                   fixture)
{
    if (!TEST_int_eq(fixture->expected,
                     ossl_cmp_asn1_octet_string_set1(&fixture->tgt_string,
                                                     fixture->src_string)))
        return 0;
    if (fixture->expected != 0)
        return TEST_int_eq(0, ASN1_OCTET_STRING_cmp(fixture->tgt_string,
                                                    fixture->src_string));
    return 1;
}

static int test_ASN1_OCTET_STRING_set(void)
{
    SETUP_TEST_FIXTURE(CMP_ASN_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    if (!TEST_ptr(fixture->tgt_string = ASN1_OCTET_STRING_new())
            || !TEST_ptr(fixture->src_string = ASN1_OCTET_STRING_new())
            || !TEST_true(ASN1_OCTET_STRING_set(fixture->src_string, rand_data,
                                                sizeof(rand_data)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_CMP_ASN1_OCTET_STRING_set1_test, tear_down);
    return result;
}

static int test_ASN1_OCTET_STRING_set_tgt_is_src(void)
{
    SETUP_TEST_FIXTURE(CMP_ASN_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    if (!TEST_ptr(fixture->src_string = ASN1_OCTET_STRING_new())
            || !(fixture->tgt_string = fixture->src_string)
            || !TEST_true(ASN1_OCTET_STRING_set(fixture->src_string, rand_data,
                                                sizeof(rand_data)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_CMP_ASN1_OCTET_STRING_set1_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    return;
}

int setup_tests(void)
{
    RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH);
    /* ASN.1 related tests */
    ADD_TEST(test_cmp_asn1_get_int);
    ADD_TEST(test_ASN1_OCTET_STRING_set);
    ADD_TEST(test_ASN1_OCTET_STRING_set_tgt_is_src);
    return 1;
}
