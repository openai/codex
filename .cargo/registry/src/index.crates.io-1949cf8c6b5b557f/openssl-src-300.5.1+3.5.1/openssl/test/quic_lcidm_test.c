/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_lcidm.h"
#include "testutil.h"

static char ptrs[8];

static const QUIC_CONN_ID cid8_1 = { 8, { 1 } };
static const QUIC_CONN_ID cid8_2 = { 8, { 2 } };
static const QUIC_CONN_ID cid8_3 = { 8, { 3 } };
static const QUIC_CONN_ID cid8_4 = { 8, { 4 } };
static const QUIC_CONN_ID cid8_5 = { 8, { 5 } };

static int test_lcidm(void)
{
    int testresult = 0;
    QUIC_LCIDM *lcidm;
    size_t lcid_len = 10; /* != ODCID len */
    QUIC_CONN_ID lcid_1, lcid_dummy, lcid_init = {0};
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid_frame_1, ncid_frame_2, ncid_frame_3;
    void *opaque = NULL;
    uint64_t seq_num = UINT64_MAX;
    int did_retire = 0;

    if (!TEST_ptr(lcidm = ossl_quic_lcidm_new(NULL, lcid_len)))
        goto err;

    if (!TEST_size_t_eq(ossl_quic_lcidm_get_lcid_len(lcidm), lcid_len))
        goto err;

    if (!TEST_true(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 0, &cid8_1))
        || !TEST_false(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 0, &cid8_2))
        || !TEST_false(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 1, &cid8_1))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 1), 0)
        || !TEST_true(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 1, &cid8_3))
        || !TEST_false(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 1, &cid8_4))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 0), 1)
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 1), 1)
        || !TEST_true(ossl_quic_lcidm_retire_odcid(lcidm, ptrs + 0))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 0), 0)
        || !TEST_false(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 0, &cid8_1))
        || !TEST_false(ossl_quic_lcidm_enrol_odcid(lcidm, ptrs + 0, &cid8_5))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 0), 0)

        || !TEST_true(ossl_quic_lcidm_generate_initial(lcidm, ptrs + 2, &lcid_1))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 1)
        || !TEST_false(ossl_quic_lcidm_generate_initial(lcidm, ptrs + 2, &lcid_init))
        || !TEST_true(ossl_quic_lcidm_generate(lcidm, ptrs + 2, &ncid_frame_1))
        || !TEST_true(ossl_quic_lcidm_generate(lcidm, ptrs + 2, &ncid_frame_2))
        || !TEST_true(ossl_quic_lcidm_generate(lcidm, ptrs + 2, &ncid_frame_3))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 4)
        || !TEST_true(ossl_quic_lcidm_lookup(lcidm, &lcid_1, &seq_num, &opaque))
        || !TEST_ptr_eq(opaque, ptrs + 2)
        || !TEST_uint64_t_eq(seq_num, 0)
        || !TEST_true(ossl_quic_lcidm_lookup(lcidm, &ncid_frame_1.conn_id,
                                             &seq_num, &opaque))
        || !TEST_ptr_eq(opaque, ptrs + 2)
        || !TEST_uint64_t_eq(seq_num, 1)
        || !TEST_true(ossl_quic_lcidm_lookup(lcidm, &ncid_frame_2.conn_id,
                                             &seq_num, &opaque))
        || !TEST_ptr_eq(opaque, ptrs + 2)
        || !TEST_uint64_t_eq(seq_num, 2)
        || !TEST_true(ossl_quic_lcidm_lookup(lcidm, &ncid_frame_3.conn_id,
                                             &seq_num, &opaque))
        || !TEST_ptr_eq(opaque, ptrs + 2)
        || !TEST_uint64_t_eq(seq_num, 3)
        || !TEST_true(ossl_quic_lcidm_retire(lcidm, ptrs + 2, 2, NULL,
                                             &lcid_dummy, &seq_num, &did_retire))
        || !TEST_true(did_retire)
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 3)
        || !TEST_true(ossl_quic_lcidm_retire(lcidm, ptrs + 2, 2, NULL,
                                             &lcid_dummy, &seq_num, &did_retire))
        || !TEST_true(did_retire)
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 2)
        || !TEST_true(ossl_quic_lcidm_retire(lcidm, ptrs + 2, 2, NULL,
                                             &lcid_dummy, &seq_num, &did_retire))
        || !TEST_false(did_retire)
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 2)
        || !TEST_false(ossl_quic_lcidm_lookup(lcidm, &lcid_init,
                                              &seq_num, &opaque))
        || !TEST_false(ossl_quic_lcidm_lookup(lcidm, &ncid_frame_1.conn_id,
                                              &seq_num, &opaque))
        || !TEST_true(ossl_quic_lcidm_lookup(lcidm, &ncid_frame_2.conn_id,
                                             &seq_num, &opaque))
        || !TEST_true(ossl_quic_lcidm_cull(lcidm, ptrs + 2))
        || !TEST_size_t_eq(ossl_quic_lcidm_get_num_active_lcid(lcidm, ptrs + 2), 0))
        goto err;

    testresult = 1;
err:
    ossl_quic_lcidm_free(lcidm);
    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_lcidm);
    return 1;
}
