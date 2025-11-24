/*
 * Copyright 2021-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2021 UnionTech. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Internal tests for the SM3 module.
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include "testutil.h"

#ifndef OPENSSL_NO_SM3
# include "internal/sm3.h"

static int test_sm3(void)
{
    static const unsigned char input1[] = {
        0x61, 0x62, 0x63
    };

    /*
     * This test vector comes from Example 1 (A.1) of GM/T 0004-2012
     */
    static const unsigned char expected1[SM3_DIGEST_LENGTH] = {
        0x66, 0xc7, 0xf0, 0xf4, 0x62, 0xee, 0xed, 0xd9,
        0xd1, 0xf2, 0xd4, 0x6b, 0xdc, 0x10, 0xe4, 0xe2,
        0x41, 0x67, 0xc4, 0x87, 0x5c, 0xf2, 0xf7, 0xa2,
        0x29, 0x7d, 0xa0, 0x2b, 0x8f, 0x4b, 0xa8, 0xe0
    };

    static const unsigned char input2[] = {
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64,
        0x61, 0x62, 0x63, 0x64, 0x61, 0x62, 0x63, 0x64
    };

    /*
     * This test vector comes from Example 2 (A.2) from GM/T 0004-2012
     */
    static const unsigned char expected2[SM3_DIGEST_LENGTH] = {
        0xde, 0xbe, 0x9f, 0xf9, 0x22, 0x75, 0xb8, 0xa1,
        0x38, 0x60, 0x48, 0x89, 0xc1, 0x8e, 0x5a, 0x4d,
        0x6f, 0xdb, 0x70, 0xe5, 0x38, 0x7e, 0x57, 0x65,
        0x29, 0x3d, 0xcb, 0xa3, 0x9c, 0x0c, 0x57, 0x32
    };

    SM3_CTX ctx1, ctx2;
    unsigned char md1[SM3_DIGEST_LENGTH], md2[SM3_DIGEST_LENGTH];

    if (!TEST_true(ossl_sm3_init(&ctx1))
            || !TEST_true(ossl_sm3_update(&ctx1, input1, sizeof(input1)))
            || !TEST_true(ossl_sm3_final(md1, &ctx1))
            || !TEST_mem_eq(md1, SM3_DIGEST_LENGTH, expected1, SM3_DIGEST_LENGTH))
        return 0;

    if (!TEST_true(ossl_sm3_init(&ctx2))
            || !TEST_true(ossl_sm3_update(&ctx2, input2, sizeof(input2)))
            || !TEST_true(ossl_sm3_final(md2, &ctx2))
            || !TEST_mem_eq(md2, SM3_DIGEST_LENGTH, expected2, SM3_DIGEST_LENGTH))
        return 0;

    return 1;
}
#endif

int setup_tests(void)
{
#ifndef OPENSSL_NO_SM3
    ADD_TEST(test_sm3);
#endif
    return 1;
}
