/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/evp.h>
#include "testutil.h"

static int test_evp_get_digestbyname(void)
{
    const EVP_MD *md;

    if (!TEST_ptr(md = EVP_get_digestbyname("SHA2-256")))
        return 0;
    return 1;
}

static int test_evp_get_cipherbyname(void)
{
    const EVP_CIPHER *cipher;

    if (!TEST_ptr(cipher = EVP_get_cipherbyname("AES-256-WRAP")))
        return 0;
    return 1;
}

int setup_tests(void)
{
    ADD_TEST(test_evp_get_digestbyname);
    ADD_TEST(test_evp_get_cipherbyname);
    return 1;
}
