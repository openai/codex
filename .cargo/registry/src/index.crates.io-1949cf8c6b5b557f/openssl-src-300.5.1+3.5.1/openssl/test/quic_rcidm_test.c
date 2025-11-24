/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_rcidm.h"
#include "testutil.h"

static const QUIC_CONN_ID cid8_1 = { 8, { 1 } };
static const QUIC_CONN_ID cid8_2 = { 8, { 2 } };
static const QUIC_CONN_ID cid8_3 = { 8, { 3 } };
static const QUIC_CONN_ID cid8_4 = { 8, { 4 } };
static const QUIC_CONN_ID cid8_5 = { 8, { 5 } };

/*
 * 0: Client, Initial ODCID
 * 1: Client, Initial ODCID + Retry ODCID
 * 2: Server, doesn't start with Initial ODCID
 */
static int test_rcidm(int idx)
{
    int testresult = 0;
    QUIC_RCIDM *rcidm;
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid_frame_1 = {0}, ncid_frame_2 = {0};
    QUIC_CONN_ID dcid_out;
    const QUIC_CONN_ID *odcid = NULL;
    uint64_t seq_num_out;

    ncid_frame_1.seq_num        = 2;
    ncid_frame_1.conn_id.id_len = 8;
    ncid_frame_1.conn_id.id[0]  = 3;

    ncid_frame_2.seq_num        = 3;
    ncid_frame_2.conn_id.id_len = 8;
    ncid_frame_2.conn_id.id[0]  = 4;

    odcid = ((idx == 2) ? NULL : &cid8_1);
    if (!TEST_ptr(rcidm = ossl_quic_rcidm_new(odcid)))
        goto err;

    if (idx != 2) {
        if (/* ODCID not counted */
            !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
            || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 0))
            || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))

            || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_1))
            || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 0))
            goto err;
    } else {
        if (!TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
            || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 0))
            goto err;
    }

    if (idx == 1) {
        if (!TEST_true(ossl_quic_rcidm_add_from_server_retry(rcidm, &cid8_5))
            || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
            || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 0))
            || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))

            || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_5))
            || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 0))
            goto err;
    }

    if (!TEST_true(ossl_quic_rcidm_add_from_initial(rcidm, &cid8_2))
        /* Initial SCID (seq=0) is counted */
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 1)
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
        || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 0))
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
        || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_2))

        || !TEST_true(ossl_quic_rcidm_add_from_ncid(rcidm, &ncid_frame_1))
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 2)
        /* Not changed over yet - handshake not confirmed */
        || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 0))
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
        || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_2))

        || !TEST_true(ossl_quic_rcidm_add_from_ncid(rcidm, &ncid_frame_2))
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_active(rcidm), 3)
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_retiring(rcidm), 0)
        || !TEST_false(ossl_quic_rcidm_pop_retire_seq_num(rcidm, &seq_num_out))
        /* Not changed over yet - handshake not confirmed */
        || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 0))
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
        || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_2)))
        goto err;

    ossl_quic_rcidm_on_handshake_complete(rcidm);

    if (!TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
        || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
        || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_3))
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_retiring(rcidm), 1)
        || !TEST_true(ossl_quic_rcidm_peek_retire_seq_num(rcidm, &seq_num_out))
        || !TEST_uint64_t_eq(seq_num_out, 0))
        goto err;

    ossl_quic_rcidm_request_roll(rcidm);

    if (!TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
        || !TEST_false(ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, 1))
        || !TEST_true(ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &dcid_out))
        || !TEST_true(ossl_quic_conn_id_eq(&dcid_out, &cid8_4))
        || !TEST_size_t_eq(ossl_quic_rcidm_get_num_retiring(rcidm), 2)
        || !TEST_true(ossl_quic_rcidm_peek_retire_seq_num(rcidm, &seq_num_out))
        || !TEST_uint64_t_eq(seq_num_out, 0)
        || !TEST_true(ossl_quic_rcidm_pop_retire_seq_num(rcidm, &seq_num_out))
        || !TEST_uint64_t_eq(seq_num_out, 0)
        || !TEST_true(ossl_quic_rcidm_pop_retire_seq_num(rcidm, &seq_num_out))
        || !TEST_uint64_t_eq(seq_num_out, 2))
        goto err;

    testresult = 1;
err:
    ossl_quic_rcidm_free(rcidm);
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_rcidm, 3);
    return 1;
}
