/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * CAST low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include <openssl/opensslconf.h> /* To see if OPENSSL_NO_CAST is defined */
#include "internal/nelem.h"
#include "testutil.h"

#ifndef OPENSSL_NO_CAST
# include <openssl/cast.h>

static unsigned char k[16] = {
    0x01, 0x23, 0x45, 0x67, 0x12, 0x34, 0x56, 0x78,
    0x23, 0x45, 0x67, 0x89, 0x34, 0x56, 0x78, 0x9A
};

static unsigned char in[8] = {
    0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF
};

static int k_len[3] = { 16, 10, 5 };

static unsigned char c[3][8] = {
    {0x23, 0x8B, 0x4F, 0xE5, 0x84, 0x7E, 0x44, 0xB2},
    {0xEB, 0x6A, 0x71, 0x1A, 0x2C, 0x02, 0x27, 0x1B},
    {0x7A, 0xC8, 0x16, 0xD1, 0x6E, 0x9B, 0x30, 0x2E},
};

static unsigned char in_a[16] = {
    0x01, 0x23, 0x45, 0x67, 0x12, 0x34, 0x56, 0x78,
    0x23, 0x45, 0x67, 0x89, 0x34, 0x56, 0x78, 0x9A
};

static unsigned char in_b[16] = {
    0x01, 0x23, 0x45, 0x67, 0x12, 0x34, 0x56, 0x78,
    0x23, 0x45, 0x67, 0x89, 0x34, 0x56, 0x78, 0x9A
};

static unsigned char c_a[16] = {
    0xEE, 0xA9, 0xD0, 0xA2, 0x49, 0xFD, 0x3B, 0xA6,
    0xB3, 0x43, 0x6F, 0xB8, 0x9D, 0x6D, 0xCA, 0x92
};

static unsigned char c_b[16] = {
    0xB2, 0xC9, 0x5E, 0xB0, 0x0C, 0x31, 0xAD, 0x71,
    0x80, 0xAC, 0x05, 0xB8, 0xE8, 0x3D, 0x69, 0x6E
};

static int cast_test_vector(int z)
{
    int testresult = 1;
    CAST_KEY key;
    unsigned char out[80];

    CAST_set_key(&key, k_len[z], k);
    CAST_ecb_encrypt(in, out, &key, CAST_ENCRYPT);
    if (!TEST_mem_eq(out, sizeof(c[z]), c[z], sizeof(c[z]))) {
        TEST_info("CAST_ENCRYPT iteration %d failed (len=%d)", z, k_len[z]);
        testresult = 0;
    }

    CAST_ecb_encrypt(out, out, &key, CAST_DECRYPT);
    if (!TEST_mem_eq(out, sizeof(in), in, sizeof(in))) {
        TEST_info("CAST_DECRYPT iteration %d failed (len=%d)", z, k_len[z]);
        testresult = 0;
    }
    return testresult;
}

static int cast_test_iterations(void)
{
    long l;
    int testresult = 1;
    CAST_KEY key, key_b;
    unsigned char out_a[16], out_b[16];

    memcpy(out_a, in_a, sizeof(in_a));
    memcpy(out_b, in_b, sizeof(in_b));

    for (l = 0; l < 1000000L; l++) {
        CAST_set_key(&key_b, 16, out_b);
        CAST_ecb_encrypt(&(out_a[0]), &(out_a[0]), &key_b, CAST_ENCRYPT);
        CAST_ecb_encrypt(&(out_a[8]), &(out_a[8]), &key_b, CAST_ENCRYPT);
        CAST_set_key(&key, 16, out_a);
        CAST_ecb_encrypt(&(out_b[0]), &(out_b[0]), &key, CAST_ENCRYPT);
        CAST_ecb_encrypt(&(out_b[8]), &(out_b[8]), &key, CAST_ENCRYPT);
    }

    if (!TEST_mem_eq(out_a, sizeof(c_a), c_a, sizeof(c_a))
            || !TEST_mem_eq(out_b, sizeof(c_b), c_b, sizeof(c_b)))
        testresult = 0;

    return testresult;
}
#endif

int setup_tests(void)
{
#ifndef OPENSSL_NO_CAST
    ADD_ALL_TESTS(cast_test_vector, OSSL_NELEM(k_len));
    ADD_TEST(cast_test_iterations);
#endif
    return 1;
}
