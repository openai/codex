/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_srtm.h"
#include "testutil.h"

static char ptrs[8];

static const QUIC_STATELESS_RESET_TOKEN token_1 = {{
    0x01, 0x02, 0x03, 0x04
}};

static const QUIC_STATELESS_RESET_TOKEN token_2 = {{
    0x01, 0x02, 0x03, 0x05
}};

static int test_srtm(void)
{
    int testresult = 0;
    QUIC_SRTM *srtm;
    void *opaque = NULL;
    uint64_t seq_num = 0;

    if (!TEST_ptr(srtm = ossl_quic_srtm_new(NULL, NULL)))
        goto err;

    if (!TEST_true(ossl_quic_srtm_add(srtm, ptrs + 0, 0, &token_1))
        || !TEST_false(ossl_quic_srtm_add(srtm, ptrs + 0, 0, &token_1))
        || !TEST_false(ossl_quic_srtm_remove(srtm, ptrs + 0, 1))
        || !TEST_false(ossl_quic_srtm_remove(srtm, ptrs + 3, 0))
        || !TEST_true(ossl_quic_srtm_cull(srtm, ptrs + 3))
        || !TEST_true(ossl_quic_srtm_cull(srtm, ptrs + 3))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 0, 1, &token_1))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 0, 2, &token_1))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 0, 3, &token_1))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 1, 0, &token_1))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 2, 0, &token_2))
        || !TEST_true(ossl_quic_srtm_add(srtm, ptrs + 3, 3, &token_2))
        || !TEST_true(ossl_quic_srtm_remove(srtm, ptrs + 3, 3))
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 0, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 1)
        || !TEST_uint64_t_eq(seq_num, 0)
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 1, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 0)
        || !TEST_uint64_t_eq(seq_num, 3)
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 2, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 0)
        || !TEST_uint64_t_eq(seq_num, 2)
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 3, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 0)
        || !TEST_uint64_t_eq(seq_num, 1)
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 4, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 0)
        || !TEST_uint64_t_eq(seq_num, 0)
        || !TEST_false(ossl_quic_srtm_lookup(srtm, &token_1, 5, &opaque, &seq_num))
        || !TEST_true(ossl_quic_srtm_cull(srtm, ptrs + 0))
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_1, 0, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 1)
        || !TEST_uint64_t_eq(seq_num, 0)
        || !TEST_true(ossl_quic_srtm_lookup(srtm, &token_2, 0, &opaque, &seq_num))
        || !TEST_ptr_eq(opaque, ptrs + 2)
        || !TEST_uint64_t_eq(seq_num, 0)
        || !TEST_true(ossl_quic_srtm_remove(srtm, ptrs + 2, 0))
        || !TEST_false(ossl_quic_srtm_lookup(srtm, &token_2, 0, &opaque, &seq_num))
       )
        goto err;

    testresult = 1;
err:
    ossl_quic_srtm_free(srtm);
    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_srtm);
    return 1;
}
