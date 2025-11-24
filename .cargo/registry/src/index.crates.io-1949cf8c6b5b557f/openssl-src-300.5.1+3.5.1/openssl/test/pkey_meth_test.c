/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal tests for EVP_PKEY method ordering */

/* We need to use some deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <string.h>

#include <openssl/evp.h>
#include "testutil.h"

/* Test of EVP_PKEY_ASN1_METHOD ordering */
static int test_asn1_meths(void)
{
    int i;
    int prev = -1;
    int good = 1;
    int pkey_id;
    const EVP_PKEY_ASN1_METHOD *ameth;

    for (i = 0; i < EVP_PKEY_asn1_get_count(); i++) {
        ameth = EVP_PKEY_asn1_get0(i);
        EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL, ameth);
        if (pkey_id < prev)
            good = 0;
        prev = pkey_id;

    }
    if (!good) {
        TEST_error("EVP_PKEY_ASN1_METHOD table out of order");
        for (i = 0; i < EVP_PKEY_asn1_get_count(); i++) {
            const char *info;

            ameth = EVP_PKEY_asn1_get0(i);
            EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, &info, NULL, ameth);
            if (info == NULL)
                info = "<NO NAME>";
            TEST_note("%d : %s : %s", pkey_id, OBJ_nid2ln(pkey_id), info);
        }
    }
    return good;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
/* Test of EVP_PKEY_METHOD ordering */
static int test_pkey_meths(void)
{
    size_t i;
    int prev = -1;
    int good = 1;
    int pkey_id;
    const EVP_PKEY_METHOD *pmeth;

    for (i = 0; i < EVP_PKEY_meth_get_count(); i++) {
        pmeth = EVP_PKEY_meth_get0(i);
        EVP_PKEY_meth_get0_info(&pkey_id, NULL, pmeth);
        if (pkey_id < prev)
            good = 0;
        prev = pkey_id;

    }
    if (!good) {
        TEST_error("EVP_PKEY_METHOD table out of order");
        for (i = 0; i < EVP_PKEY_meth_get_count(); i++) {
            pmeth = EVP_PKEY_meth_get0(i);
            EVP_PKEY_meth_get0_info(&pkey_id, NULL, pmeth);
            TEST_note("%d : %s", pkey_id, OBJ_nid2ln(pkey_id));
        }
    }
    return good;
}
#endif

int setup_tests(void)
{
    ADD_TEST(test_asn1_meths);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ADD_TEST(test_pkey_meths);
#endif
    return 1;
}
