/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Tests of the EVP_PKEY_CTX_set_* macro family */

#include <stdio.h>
#include <string.h>

#include <openssl/evp.h>
#include <openssl/kdf.h>
#include "testutil.h"

static int test_kdf_tls1_prf(int index)
{
    int ret = 0;
    EVP_PKEY_CTX *pctx;
    unsigned char out[16];
    size_t outlen = sizeof(out);

    if ((pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_TLS1_PRF, NULL)) == NULL) {
        TEST_error("EVP_PKEY_TLS1_PRF");
        goto err;
    }
    if (EVP_PKEY_derive_init(pctx) <= 0) {
        TEST_error("EVP_PKEY_derive_init");
        goto err;
    }
    if (EVP_PKEY_CTX_set_tls1_prf_md(pctx, EVP_sha256()) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_tls1_prf_md");
        goto err;
    }
    if (EVP_PKEY_CTX_set1_tls1_prf_secret(pctx,
                                          (unsigned char *)"secret", 6) <= 0) {
        TEST_error("EVP_PKEY_CTX_set1_tls1_prf_secret");
        goto err;
    }
    if (index == 0) {
        if (EVP_PKEY_CTX_add1_tls1_prf_seed(pctx,
                                            (unsigned char *)"seed", 4) <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_tls1_prf_seed");
            goto err;
        }
    } else {
        if (EVP_PKEY_CTX_add1_tls1_prf_seed(pctx,
                                            (unsigned char *)"se", 2) <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_tls1_prf_seed");
            goto err;
        }
        if (EVP_PKEY_CTX_add1_tls1_prf_seed(pctx,
                                            (unsigned char *)"ed", 2) <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_tls1_prf_seed");
            goto err;
        }
    }
    if (EVP_PKEY_derive(pctx, out, &outlen) <= 0) {
        TEST_error("EVP_PKEY_derive");
        goto err;
    }

    {
        const unsigned char expected[sizeof(out)] = {
            0x8e, 0x4d, 0x93, 0x25, 0x30, 0xd7, 0x65, 0xa0,
            0xaa, 0xe9, 0x74, 0xc3, 0x04, 0x73, 0x5e, 0xcc
        };
        if (!TEST_mem_eq(out, sizeof(out), expected, sizeof(expected))) {
            goto err;
        }
    }
    ret = 1;
err:
    EVP_PKEY_CTX_free(pctx);
    return ret;
}

static int test_kdf_hkdf(int index)
{
    int ret = 0;
    EVP_PKEY_CTX *pctx;
    unsigned char out[10];
    size_t outlen = sizeof(out);

    if ((pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_HKDF, NULL)) == NULL) {
        TEST_error("EVP_PKEY_HKDF");
        goto err;
    }
    if (EVP_PKEY_derive_init(pctx) <= 0) {
        TEST_error("EVP_PKEY_derive_init");
        goto err;
    }
    if (EVP_PKEY_CTX_set_hkdf_md(pctx, EVP_sha256()) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_hkdf_md");
        goto err;
    }
    if (EVP_PKEY_CTX_set1_hkdf_salt(pctx, (const unsigned char *)"salt", 4)
            <= 0) {
        TEST_error("EVP_PKEY_CTX_set1_hkdf_salt");
        goto err;
    }
    if (EVP_PKEY_CTX_set1_hkdf_key(pctx, (const unsigned char *)"secret", 6)
            <= 0) {
        TEST_error("EVP_PKEY_CTX_set1_hkdf_key");
        goto err;
    }
    if (index == 0) {
        if (EVP_PKEY_CTX_add1_hkdf_info(pctx, (const unsigned char *)"label", 5)
            <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_hkdf_info");
            goto err;
        }
    } else {
        if (EVP_PKEY_CTX_add1_hkdf_info(pctx, (const unsigned char *)"lab", 3)
            <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_hkdf_info");
            goto err;
        }
        if (EVP_PKEY_CTX_add1_hkdf_info(pctx, (const unsigned char *)"el", 2)
            <= 0) {
            TEST_error("EVP_PKEY_CTX_add1_hkdf_info");
            goto err;
        }
    }
    if (EVP_PKEY_derive(pctx, out, &outlen) <= 0) {
        TEST_error("EVP_PKEY_derive");
        goto err;
    }

    {
        const unsigned char expected[sizeof(out)] = {
            0x2a, 0xc4, 0x36, 0x9f, 0x52, 0x59, 0x96, 0xf8, 0xde, 0x13
        };
        if (!TEST_mem_eq(out, sizeof(out), expected, sizeof(expected))) {
            goto err;
        }
    }
    ret = 1;
err:
    EVP_PKEY_CTX_free(pctx);
    return ret;
}

#ifndef OPENSSL_NO_SCRYPT
static int test_kdf_scrypt(void)
{
    int ret = 0;
    EVP_PKEY_CTX *pctx;
    unsigned char out[64];
    size_t outlen = sizeof(out);

    if ((pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_SCRYPT, NULL)) == NULL) {
        TEST_error("EVP_PKEY_SCRYPT");
        goto err;
    }
    if (EVP_PKEY_derive_init(pctx) <= 0) {
        TEST_error("EVP_PKEY_derive_init");
        goto err;
    }
    if (EVP_PKEY_CTX_set1_pbe_pass(pctx, "password", 8) <= 0) {
        TEST_error("EVP_PKEY_CTX_set1_pbe_pass");
        goto err;
    }
    if (EVP_PKEY_CTX_set1_scrypt_salt(pctx, (unsigned char *)"NaCl", 4) <= 0) {
        TEST_error("EVP_PKEY_CTX_set1_scrypt_salt");
        goto err;
    }
    if (EVP_PKEY_CTX_set_scrypt_N(pctx, 1024) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_scrypt_N");
        goto err;
    }
    if (EVP_PKEY_CTX_set_scrypt_r(pctx, 8) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_scrypt_r");
        goto err;
    }
    if (EVP_PKEY_CTX_set_scrypt_p(pctx, 16) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_scrypt_p");
        goto err;
    }
    if (EVP_PKEY_CTX_set_scrypt_maxmem_bytes(pctx, 16) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_maxmem_bytes");
        goto err;
    }
    if (EVP_PKEY_derive(pctx, out, &outlen) > 0) {
        TEST_error("EVP_PKEY_derive should have failed");
        goto err;
    }
    if (EVP_PKEY_CTX_set_scrypt_maxmem_bytes(pctx, 10 * 1024 * 1024) <= 0) {
        TEST_error("EVP_PKEY_CTX_set_maxmem_bytes");
        goto err;
    }
    if (EVP_PKEY_derive(pctx, out, &outlen) <= 0) {
        TEST_error("EVP_PKEY_derive");
        goto err;
    }

    {
        const unsigned char expected[sizeof(out)] = {
            0xfd, 0xba, 0xbe, 0x1c, 0x9d, 0x34, 0x72, 0x00,
            0x78, 0x56, 0xe7, 0x19, 0x0d, 0x01, 0xe9, 0xfe,
            0x7c, 0x6a, 0xd7, 0xcb, 0xc8, 0x23, 0x78, 0x30,
            0xe7, 0x73, 0x76, 0x63, 0x4b, 0x37, 0x31, 0x62,
            0x2e, 0xaf, 0x30, 0xd9, 0x2e, 0x22, 0xa3, 0x88,
            0x6f, 0xf1, 0x09, 0x27, 0x9d, 0x98, 0x30, 0xda,
            0xc7, 0x27, 0xaf, 0xb9, 0x4a, 0x83, 0xee, 0x6d,
            0x83, 0x60, 0xcb, 0xdf, 0xa2, 0xcc, 0x06, 0x40
        };
        if (!TEST_mem_eq(out, sizeof(out), expected, sizeof(expected))) {
            goto err;
        }
    }
    ret = 1;
err:
    EVP_PKEY_CTX_free(pctx);
    return ret;
}
#endif

int setup_tests(void)
{
    int tests = 1;

    if (fips_provider_version_ge(NULL, 3, 3, 1))
        tests = 2;

    ADD_ALL_TESTS(test_kdf_tls1_prf, tests);
    ADD_ALL_TESTS(test_kdf_hkdf, tests);
#ifndef OPENSSL_NO_SCRYPT
    ADD_TEST(test_kdf_scrypt);
#endif
    return 1;
}
