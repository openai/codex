/*
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

typedef struct test_fixture {
    const char *test_case_name;
    int pkistatus;
    const char *str;  /* Not freed by tear_down */
    const char *text; /* Not freed by tear_down */
    int pkifailure;
} CMP_STATUS_TEST_FIXTURE;

static CMP_STATUS_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_STATUS_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    return fixture;
}

static void tear_down(CMP_STATUS_TEST_FIXTURE *fixture)
{
    OPENSSL_free(fixture);
}

/*
 * Tests PKIStatusInfo creation and get-functions
 */
static int execute_PKISI_test(CMP_STATUS_TEST_FIXTURE *fixture)
{
    OSSL_CMP_PKISI *si = NULL;
    int status;
    ASN1_UTF8STRING *statusString = NULL;
    int res = 0, i;

    if (!TEST_ptr(si = OSSL_CMP_STATUSINFO_new(fixture->pkistatus,
                                               fixture->pkifailure,
                                               fixture->text)))
        goto end;

    status = ossl_cmp_pkisi_get_status(si);
    if (!TEST_int_eq(fixture->pkistatus, status)
            || !TEST_str_eq(fixture->str, ossl_cmp_PKIStatus_to_string(status)))
        goto end;

    if (!TEST_ptr(statusString =
                  sk_ASN1_UTF8STRING_value(ossl_cmp_pkisi_get0_statusString(si),
                                           0))
            || !TEST_mem_eq(fixture->text, strlen(fixture->text),
                            (char *)statusString->data, statusString->length))
        goto end;

    if (!TEST_int_eq(fixture->pkifailure,
                     ossl_cmp_pkisi_get_pkifailureinfo(si)))
        goto end;
    for (i = 0; i <= OSSL_CMP_PKIFAILUREINFO_MAX; i++)
        if (!TEST_int_eq((fixture->pkifailure >> i) & 1,
                         ossl_cmp_pkisi_check_pkifailureinfo(si, i)))
            goto end;

    res = 1;

 end:
    OSSL_CMP_PKISI_free(si);
    return res;
}

static int test_PKISI(void)
{
    SETUP_TEST_FIXTURE(CMP_STATUS_TEST_FIXTURE, set_up);
    fixture->pkistatus = OSSL_CMP_PKISTATUS_revocationNotification;
    fixture->str = "PKIStatus: revocation notification - a revocation of the cert has occurred";
    fixture->text = "this is an additional text describing the failure";
    fixture->pkifailure = OSSL_CMP_CTX_FAILINFO_unsupportedVersion |
        OSSL_CMP_CTX_FAILINFO_badDataFormat;
    EXECUTE_TEST(execute_PKISI_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    return;
}

int setup_tests(void)
{
    /*-
     * this tests all of:
     * OSSL_CMP_STATUSINFO_new()
     * ossl_cmp_pkisi_get_status()
     * ossl_cmp_PKIStatus_to_string()
     * ossl_cmp_pkisi_get0_statusString()
     * ossl_cmp_pkisi_get_pkifailureinfo()
     * ossl_cmp_pkisi_check_pkifailureinfo()
     */
    ADD_TEST(test_PKISI);
    return 1;
}
