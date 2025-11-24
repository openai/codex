/*
 * Copyright 2015-2017 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include "testutil.h"

/*
 * Password based encryption (PBE) table ordering test.
 * Attempt to look up all supported algorithms.
 */

static int test_pbelu(void)
{
    int i, failed = 0;
    int pbe_type, pbe_nid, last_type = -1, last_nid = -1;

    for (i = 0; EVP_PBE_get(&pbe_type, &pbe_nid, i) != 0; i++) {
        if (!TEST_true(EVP_PBE_find(pbe_type, pbe_nid, NULL, NULL, 0))) {
            TEST_note("i=%d, pbe_type=%d, pbe_nid=%d", i, pbe_type, pbe_nid);
            failed = 1;
            break;
        }
    }

    if (!failed)
        return 1;

    /* Error: print out whole table */
    for (i = 0; EVP_PBE_get(&pbe_type, &pbe_nid, i) != 0; i++) {
        failed = pbe_type < last_type
                 || (pbe_type == last_type && pbe_nid < last_nid);
        TEST_note("PBE type=%d %d (%s): %s\n", pbe_type, pbe_nid,
                  OBJ_nid2sn(pbe_nid), failed ? "ERROR" : "OK");
        last_type = pbe_type;
        last_nid = pbe_nid;
    }
    return 0;
}

int setup_tests(void)
{
    ADD_TEST(test_pbelu);
    return 1;
}
