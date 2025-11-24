/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal tests for the mdc2 module */

/*
 * MDC2 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>

#include <openssl/mdc2.h>
#include "testutil.h"
#include "internal/nelem.h"

typedef struct {
    const char *input;
    const unsigned char expected[MDC2_DIGEST_LENGTH];
} TESTDATA;


/**********************************************************************
 *
 * Test driver
 *
 ***/

static TESTDATA tests[] = {
    {
        "Now is the time for all ",
        {
            0x42, 0xE5, 0x0C, 0xD2, 0x24, 0xBA, 0xCE, 0xBA,
            0x76, 0x0B, 0xDD, 0x2B, 0xD4, 0x09, 0x28, 0x1A
        }
    }
};

/**********************************************************************
 *
 * Test of mdc2 internal functions
 *
 ***/

static int test_mdc2(int idx)
{
    unsigned char md[MDC2_DIGEST_LENGTH];
    MDC2_CTX c;
    const TESTDATA testdata = tests[idx];

    MDC2_Init(&c);
    MDC2_Update(&c, (const unsigned char *)testdata.input,
                strlen(testdata.input));
    MDC2_Final(&(md[0]), &c);

    if (!TEST_mem_eq(testdata.expected, MDC2_DIGEST_LENGTH,
                     md, MDC2_DIGEST_LENGTH)) {
        TEST_info("mdc2 test %d: unexpected output", idx);
        return 0;
    }

    return 1;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_mdc2, OSSL_NELEM(tests));
    return 1;
}
