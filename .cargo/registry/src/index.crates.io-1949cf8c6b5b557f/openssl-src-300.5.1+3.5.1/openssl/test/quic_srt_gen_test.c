/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include "internal/quic_srt_gen.h"

#include "testutil.h"
#include "testutil/output.h"

struct test_case {
    const unsigned char         *key;
    size_t                      key_len;
    QUIC_CONN_ID                dcid;
    QUIC_STATELESS_RESET_TOKEN  expected;
};

static const unsigned char key_1[] = { 0x01, 0x02, 0x03 };

static const unsigned char key_2[] = {
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01
};

static const struct test_case tests[] = {
    {
      key_1, sizeof(key_1), { 2, { 0x55, 0x66 } },
      {{ 0x02,0x9e,0x8f,0x3d,0x1e,0xa9,0x06,0x23,0xb2,0x43,0xd2,0x19,0x59,0x8a,0xa1,0x66 }}
    },
    {
      key_2, sizeof(key_2), { 0, { 0 } },
      {{ 0x93,0x10,0x2f,0xc7,0xaf,0x9d,0x9b,0x28,0x3f,0x84,0x95,0x6b,0xa3,0xdc,0x07,0x6b }}
    },
    {
      key_2, sizeof(key_2),
      { 20, { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1 } },
      {{ 0x9a,0x98,0x98,0x61,0xbe,0xfd,0xe3,0x05,0x45,0xac,0x66,0xcf,0x3b,0x58,0xfb,0xab }}
    }
};

static int test_srt_gen(int idx)
{
    int testresult = 0;
    const struct test_case *t = &tests[idx];
    QUIC_SRT_GEN *srt_gen = NULL;
    QUIC_STATELESS_RESET_TOKEN token;
    size_t i;

    if (!TEST_ptr(srt_gen = ossl_quic_srt_gen_new(NULL, NULL,
                                                  t->key, t->key_len)))
        goto err;

    for (i = 0; i < 2; ++i) {
        memset(&token, 0xff, sizeof(token));

        if (!TEST_true(ossl_quic_srt_gen_calculate_token(srt_gen, &t->dcid,
                                                         &token)))
            goto err;

        if (!TEST_mem_eq(token.token, sizeof(token.token),
                         &t->expected, sizeof(t->expected)))
            goto err;
    }

    testresult = 1;
err:
    ossl_quic_srt_gen_free(srt_gen);
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_srt_gen, OSSL_NELEM(tests));
    return 1;
}
