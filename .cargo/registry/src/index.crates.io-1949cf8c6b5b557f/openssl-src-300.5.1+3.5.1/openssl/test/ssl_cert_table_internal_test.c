/*
 * Copyright 2017-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal tests for the x509 and x509v3 modules */

#include <stdio.h>
#include <string.h>

#include <openssl/ssl.h>
#include "testutil.h"
#include "internal/nelem.h"
#include "../ssl/ssl_local.h"
#include "../ssl/ssl_cert_table.h"

#define test_cert_table(nid, amask, idx) \
    do_test_cert_table(nid, amask, idx, #idx)

static int do_test_cert_table(int nid, uint32_t amask, size_t idx,
                              const char *idxname)
{
    const SSL_CERT_LOOKUP *clu = &ssl_cert_info[idx];

    if (clu->nid == nid && clu->amask == amask)
        return 1;

    TEST_error("Invalid table entry for certificate type %s, index %zu",
               idxname, idx);
    if (clu->nid != nid)
        TEST_note("Expected %s, got %s\n", OBJ_nid2sn(nid),
                  OBJ_nid2sn(clu->nid));
    if (clu->amask != amask)
        TEST_note("Expected auth mask 0x%x, got 0x%x\n",
                  (unsigned int)amask, (unsigned int)clu->amask);
    return 0;
}

/* Sanity check of ssl_cert_table */

static int test_ssl_cert_table(void)
{
    return TEST_size_t_eq(OSSL_NELEM(ssl_cert_info), SSL_PKEY_NUM)
           && test_cert_table(EVP_PKEY_RSA, SSL_aRSA, SSL_PKEY_RSA)
           && test_cert_table(EVP_PKEY_DSA, SSL_aDSS, SSL_PKEY_DSA_SIGN)
           && test_cert_table(EVP_PKEY_EC, SSL_aECDSA, SSL_PKEY_ECC)
           && test_cert_table(NID_id_GostR3410_2001, SSL_aGOST01,
                              SSL_PKEY_GOST01)
           && test_cert_table(NID_id_GostR3410_2012_256, SSL_aGOST12,
                              SSL_PKEY_GOST12_256)
           && test_cert_table(NID_id_GostR3410_2012_512, SSL_aGOST12,
                              SSL_PKEY_GOST12_512)
           && test_cert_table(EVP_PKEY_ED25519, SSL_aECDSA, SSL_PKEY_ED25519)
           && test_cert_table(EVP_PKEY_ED448, SSL_aECDSA, SSL_PKEY_ED448);
}

int setup_tests(void)
{
    ADD_TEST(test_ssl_cert_table);
    return 1;
}
