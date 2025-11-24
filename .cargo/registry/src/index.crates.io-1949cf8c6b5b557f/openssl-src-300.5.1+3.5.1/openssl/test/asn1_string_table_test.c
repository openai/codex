/*
 * Copyright 2017 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Tests for the ASN1_STRING_TABLE_* functions */

#include <stdio.h>
#include <string.h>

#include <openssl/asn1.h>
#include "testutil.h"

static int test_string_tbl(void)
{
    const ASN1_STRING_TABLE *tmp = NULL;
    int nid = 12345678, nid2 = 87654321, rv = 0, ret = 0;

    tmp = ASN1_STRING_TABLE_get(nid);
    if (!TEST_ptr_null(tmp)) {
        TEST_info("asn1 string table: ASN1_STRING_TABLE_get non-exist nid");
        goto out;
    }

    ret = ASN1_STRING_TABLE_add(nid, -1, -1, MBSTRING_ASC, 0);
    if (!TEST_true(ret)) {
        TEST_info("asn1 string table: add NID(%d) failed", nid);
        goto out;
    }

    ret = ASN1_STRING_TABLE_add(nid2, -1, -1, MBSTRING_ASC, 0);
    if (!TEST_true(ret)) {
        TEST_info("asn1 string table: add NID(%d) failed", nid2);
        goto out;
    }

    tmp = ASN1_STRING_TABLE_get(nid);
    if (!TEST_ptr(tmp)) {
        TEST_info("asn1 string table: get NID(%d) failed", nid);
        goto out;
    }

    tmp = ASN1_STRING_TABLE_get(nid2);
    if (!TEST_ptr(tmp)) {
        TEST_info("asn1 string table: get NID(%d) failed", nid2);
        goto out;
    }

    ASN1_STRING_TABLE_cleanup();

    /* check if all newly added NIDs are cleaned up */
    tmp = ASN1_STRING_TABLE_get(nid);
    if (!TEST_ptr_null(tmp)) {
        TEST_info("asn1 string table: get NID(%d) failed", nid);
        goto out;
    }

    tmp = ASN1_STRING_TABLE_get(nid2);
    if (!TEST_ptr_null(tmp)) {
        TEST_info("asn1 string table: get NID(%d) failed", nid2);
        goto out;
    }

    rv = 1;
 out:
    return rv;
}

int setup_tests(void)
{
    ADD_TEST(test_string_tbl);
    return 1;
}
