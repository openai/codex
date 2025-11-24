/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_fc.h"
#include "internal/quic_error.h"
#include "testutil.h"

static int test_txfc(int is_stream)
{
    int testresult = 0;
    QUIC_TXFC conn_txfc, stream_txfc, *txfc, *parent_txfc;

    if (!TEST_true(ossl_quic_txfc_init(&conn_txfc, 0)))
        goto err;

    if (is_stream && !TEST_true(ossl_quic_txfc_init(&stream_txfc, &conn_txfc)))
        goto err;

    txfc = is_stream ? &stream_txfc : &conn_txfc;
    parent_txfc = is_stream ? &conn_txfc : NULL;

    if (!TEST_true(ossl_quic_txfc_bump_cwm(txfc, 2000)))
        goto err;

    if (is_stream && !TEST_true(ossl_quic_txfc_bump_cwm(parent_txfc, 2000)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 0))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_cwm(txfc), 2000))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 0), 2000))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 100), 1900))
        goto err;

    if (is_stream) {
        if ( !TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 0), 2000))
            goto err;

        if ( !TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 100), 1900))
            goto err;
    }

    if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 500)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 0), 1500))
        goto err;

    if (is_stream && !TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 0),
                                       1500))
        goto err;

    if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 500))
        goto err;

    if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 100)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 600))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 0), 1400))
        goto err;

    if (is_stream && !TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 0),
                                       1400))
        goto err;

    if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 1400)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 0), 0))
        goto err;

    if (is_stream && !TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 0),
                                       0))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 2000))
        goto err;

    if (!TEST_true(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_true(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_true(ossl_quic_txfc_has_become_blocked(txfc, 1)))
        goto err;

    if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
        goto err;

    if (!TEST_false(ossl_quic_txfc_consume_credit(txfc, 1)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_cwm(txfc), 2000))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 2000))
        goto err;

    if (!TEST_false(ossl_quic_txfc_bump_cwm(txfc, 2000)))
        goto err;

    if (!TEST_true(ossl_quic_txfc_bump_cwm(txfc, 2500)))
        goto err;

    if (is_stream && !TEST_true(ossl_quic_txfc_bump_cwm(parent_txfc, 2400)))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_cwm(txfc), 2500))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_swm(txfc), 2000))
        goto err;

    if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit_local(txfc, 0), 500))
        goto err;

    if (is_stream)
        ossl_quic_txfc_has_become_blocked(parent_txfc, 1);

    if (is_stream) {
        if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 400), 0))
            goto err;

        if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 399)))
            goto err;

        if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
            goto err;

        if (!TEST_uint64_t_eq(ossl_quic_txfc_get_credit(txfc, 0), 1))
            goto err;

        if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 1)))
            goto err;

        if (!TEST_true(ossl_quic_txfc_has_become_blocked(parent_txfc, 0)))
            goto err;

        if (!TEST_true(ossl_quic_txfc_has_become_blocked(parent_txfc, 1)))
            goto err;

        if (!TEST_false(ossl_quic_txfc_has_become_blocked(parent_txfc, 0)))
            goto err;
    } else {
        if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 499)))
            goto err;

        if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
            goto err;

        if (is_stream && !TEST_false(ossl_quic_txfc_has_become_blocked(parent_txfc, 0)))
            goto err;

        if (!TEST_true(ossl_quic_txfc_consume_credit(txfc, 1)))
            goto err;

        if (!TEST_true(ossl_quic_txfc_has_become_blocked(txfc, 0)))
            goto err;

        if (!TEST_true(ossl_quic_txfc_has_become_blocked(txfc, 1)))
            goto err;

        if (!TEST_false(ossl_quic_txfc_has_become_blocked(txfc, 0)))
            goto err;
    }

    testresult = 1;
err:
    return testresult;
}

static OSSL_TIME cur_time;

static OSSL_TIME fake_now(void *arg)
{
    return cur_time;
}

#define RX_OPC_END                    0
#define RX_OPC_INIT_CONN              1 /* arg0=initial window, arg1=max window */
#define RX_OPC_INIT_STREAM            2 /* arg0=initial window, arg1=max window */
#define RX_OPC_RX                     3 /* arg0=end, arg1=is_fin */
#define RX_OPC_RETIRE                 4 /* arg0=num_bytes, arg1=rtt in OSSL_TIME ticks, expect_fail */
#define RX_OPC_CHECK_CWM_CONN         5 /* arg0=expected */
#define RX_OPC_CHECK_CWM_STREAM       6 /* arg0=expected */
#define RX_OPC_CHECK_SWM_CONN         7 /* arg0=expected */
#define RX_OPC_CHECK_SWM_STREAM       8 /* arg0=expected */
#define RX_OPC_CHECK_RWM_CONN         9 /* arg0=expected */
#define RX_OPC_CHECK_RWM_STREAM      10 /* arg0=expected */
#define RX_OPC_CHECK_CHANGED_CONN    11 /* arg0=expected, arg1=clear */
#define RX_OPC_CHECK_CHANGED_STREAM  12 /* arg0=expected, arg1=clear */
#define RX_OPC_CHECK_ERROR_CONN      13 /* arg0=expected, arg1=clear */
#define RX_OPC_CHECK_ERROR_STREAM    14 /* arg0=expected, arg1=clear */
#define RX_OPC_STEP_TIME             15 /* arg0=OSSL_TIME ticks to advance */
#define RX_OPC_MSG                   16

struct rx_test_op {
    unsigned char   op;
    size_t          stream_idx;
    uint64_t        arg0, arg1;
    unsigned char   expect_fail;
    const char     *msg;
};

#define RX_OP_END \
    { RX_OPC_END }
#define RX_OP_INIT_CONN(init_window_size, max_window_size) \
    { RX_OPC_INIT_CONN, 0, (init_window_size), (max_window_size) },
#define RX_OP_INIT_STREAM(stream_idx, init_window_size, max_window_size) \
    { RX_OPC_INIT_STREAM, (stream_idx), (init_window_size), (max_window_size) },
#define RX_OP_RX(stream_idx, end, is_fin) \
    { RX_OPC_RX, (stream_idx), (end), (is_fin) },
#define RX_OP_RETIRE(stream_idx, num_bytes, rtt, expect_fail) \
    { RX_OPC_RETIRE, (stream_idx), (num_bytes), (rtt), (expect_fail) },
#define RX_OP_CHECK_CWM_CONN(expected) \
    { RX_OPC_CHECK_CWM_CONN, 0, (expected) },
#define RX_OP_CHECK_CWM_STREAM(stream_id, expected) \
    { RX_OPC_CHECK_CWM_STREAM, (stream_id), (expected) },
#define RX_OP_CHECK_SWM_CONN(expected) \
    { RX_OPC_CHECK_SWM_CONN, 0, (expected) },
#define RX_OP_CHECK_SWM_STREAM(stream_id, expected) \
    { RX_OPC_CHECK_SWM_STREAM, (stream_id), (expected) },
#define RX_OP_CHECK_RWM_CONN(expected) \
    { RX_OPC_CHECK_RWM_CONN, 0, (expected) },
#define RX_OP_CHECK_RWM_STREAM(stream_id, expected) \
    { RX_OPC_CHECK_RWM_STREAM, (stream_id), (expected) },
#define RX_OP_CHECK_CHANGED_CONN(expected, clear) \
    { RX_OPC_CHECK_CHANGED_CONN, 0, (expected), (clear) },
#define RX_OP_CHECK_CHANGED_STREAM(stream_id, expected, clear) \
    { RX_OPC_CHECK_CHANGED_STREAM, (stream_id), (expected), (clear) },
#define RX_OP_CHECK_ERROR_CONN(expected, clear) \
    { RX_OPC_CHECK_ERROR_CONN, 0, (expected), (clear) },
#define RX_OP_CHECK_ERROR_STREAM(stream_id, expected, clear) \
    { RX_OPC_CHECK_ERROR_STREAM, (stream_id), (expected), (clear) },
#define RX_OP_STEP_TIME(t) \
    { RX_OPC_STEP_TIME, 0, (t) },
#define RX_OP_MSG(msg) \
    { RX_OPC_MSG, 0, 0, 0, 0, (msg) },

#define RX_OP_INIT(init_window_size, max_window_size) \
    RX_OP_INIT_CONN(init_window_size, max_window_size) \
    RX_OP_INIT_STREAM(0, init_window_size, max_window_size)
#define RX_OP_CHECK_CWM(expected) \
    RX_OP_CHECK_CWM_CONN(expected) \
    RX_OP_CHECK_CWM_STREAM(0, expected)
#define RX_OP_CHECK_SWM(expected) \
    RX_OP_CHECK_SWM_CONN(expected) \
    RX_OP_CHECK_SWM_STREAM(0, expected)
#define RX_OP_CHECK_RWM(expected) \
    RX_OP_CHECK_RWM_CONN(expected) \
    RX_OP_CHECK_RWM_STREAM(0, expected)
#define RX_OP_CHECK_CHANGED(expected, clear) \
    RX_OP_CHECK_CHANGED_CONN(expected, clear) \
    RX_OP_CHECK_CHANGED_STREAM(0, expected, clear)
#define RX_OP_CHECK_ERROR(expected, clear) \
    RX_OP_CHECK_ERROR_CONN(expected, clear) \
    RX_OP_CHECK_ERROR_STREAM(0, expected, clear)

#define INIT_WINDOW_SIZE (1 * 1024 * 1024)
#define INIT_S_WINDOW_SIZE (384 * 1024)

/* 1. Basic RXFC Tests (stream window == connection window) */
static const struct rx_test_op rx_script_1[] = {
    RX_OP_STEP_TIME(1000 * OSSL_TIME_MS)
    RX_OP_INIT(INIT_WINDOW_SIZE, 10 * INIT_WINDOW_SIZE)
    /* Check initial state. */
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE)
    RX_OP_CHECK_ERROR(0, 0)
    RX_OP_CHECK_CHANGED(0, 0)
    /* We cannot retire what we have not received. */
    RX_OP_RETIRE(0, 1, 0, 1)
    /* Zero bytes is a no-op and always valid. */
    RX_OP_RETIRE(0, 0, 0, 0)
    /* Consume some window. */
    RX_OP_RX(0, 50, 0)
    /* CWM has not changed. */
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE)
    RX_OP_CHECK_SWM(50)

    /* RX, Partial retire */
    RX_OP_RX(0, 60, 0)
    RX_OP_CHECK_SWM(60)
    RX_OP_RETIRE(0, 20, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_RWM(20)
    RX_OP_CHECK_SWM(60)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CHANGED(0, 0)
    RX_OP_CHECK_ERROR(0, 0)

    /* Fully retired */
    RX_OP_RETIRE(0, 41, 0, 1)
    RX_OP_RETIRE(0, 40, 0, 0)
    RX_OP_CHECK_SWM(60)
    RX_OP_CHECK_RWM(60)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CHANGED(0, 0)
    RX_OP_CHECK_ERROR(0, 0)

    /* Exhaustion of window - we do not enlarge the window this epoch */
    RX_OP_STEP_TIME(201 * OSSL_TIME_MS)
    RX_OP_RX(0, INIT_WINDOW_SIZE, 0)
    RX_OP_RETIRE(0, INIT_WINDOW_SIZE - 60, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_SWM(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CHANGED(1, 0)
    RX_OP_CHECK_CHANGED(1, 1)
    RX_OP_CHECK_CHANGED(0, 0)
    RX_OP_CHECK_ERROR(0, 0)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE * 2)

    /* Second epoch - we still do not enlarge the window this epoch */
    RX_OP_RX(0, INIT_WINDOW_SIZE + 1, 0)
    RX_OP_STEP_TIME(201 * OSSL_TIME_MS)
    RX_OP_RX(0, INIT_WINDOW_SIZE * 2, 0)
    RX_OP_RETIRE(0, INIT_WINDOW_SIZE, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_SWM(INIT_WINDOW_SIZE * 2)
    RX_OP_CHECK_CHANGED(1, 0)
    RX_OP_CHECK_CHANGED(1, 1)
    RX_OP_CHECK_CHANGED(0, 0)
    RX_OP_CHECK_ERROR(0, 0)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE * 3)

    /* Third epoch - we enlarge the window */
    RX_OP_RX(0, INIT_WINDOW_SIZE * 2 + 1, 0)
    RX_OP_STEP_TIME(199 * OSSL_TIME_MS)
    RX_OP_RX(0, INIT_WINDOW_SIZE * 3, 0)
    RX_OP_RETIRE(0, INIT_WINDOW_SIZE, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_SWM(INIT_WINDOW_SIZE * 3)
    RX_OP_CHECK_CHANGED(1, 0)
    RX_OP_CHECK_CHANGED(1, 1)
    RX_OP_CHECK_CHANGED(0, 0)
    RX_OP_CHECK_ERROR(0, 0)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE * 5)

    /* Fourth epoch - peer violates flow control */
    RX_OP_RX(0, INIT_WINDOW_SIZE * 5 - 5, 0)
    RX_OP_STEP_TIME(250 * OSSL_TIME_MS)
    RX_OP_RX(0, INIT_WINDOW_SIZE * 5 + 1, 0)
    RX_OP_CHECK_SWM(INIT_WINDOW_SIZE * 5)
    RX_OP_CHECK_ERROR(OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 0)
    RX_OP_CHECK_ERROR(OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 1)
    RX_OP_CHECK_ERROR(0, 0)
    RX_OP_CHECK_CWM(INIT_WINDOW_SIZE * 5)
    /*
     * No window expansion due to flow control violation; window expansion is
     * triggered by retirement only.
     */
    RX_OP_CHECK_CHANGED(0, 0)

    RX_OP_END
};

/* 2. Interaction between connection and stream-level flow control */
static const struct rx_test_op rx_script_2[] = {
    RX_OP_STEP_TIME(1000 * OSSL_TIME_MS)
    RX_OP_INIT_CONN(INIT_WINDOW_SIZE, 10 * INIT_WINDOW_SIZE)
    RX_OP_INIT_STREAM(0, INIT_S_WINDOW_SIZE, 30 * INIT_S_WINDOW_SIZE)
    RX_OP_INIT_STREAM(1, INIT_S_WINDOW_SIZE, 30 * INIT_S_WINDOW_SIZE)

    RX_OP_RX(0, 10, 0)
    RX_OP_CHECK_CWM_CONN(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(1, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_SWM_CONN(10)
    RX_OP_CHECK_SWM_STREAM(0, 10)
    RX_OP_CHECK_SWM_STREAM(1, 0)
    RX_OP_CHECK_RWM_CONN(0)
    RX_OP_CHECK_RWM_STREAM(0, 0)
    RX_OP_CHECK_RWM_STREAM(1, 0)

    RX_OP_RX(1, 42, 0)
    RX_OP_RX(1, 42, 0) /* monotonic; equal or lower values ignored */
    RX_OP_RX(1, 35, 0)
    RX_OP_CHECK_CWM_CONN(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(1, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_SWM_CONN(52)
    RX_OP_CHECK_SWM_STREAM(0, 10)
    RX_OP_CHECK_SWM_STREAM(1, 42)
    RX_OP_CHECK_RWM_CONN(0)
    RX_OP_CHECK_RWM_STREAM(0, 0)
    RX_OP_CHECK_RWM_STREAM(1, 0)

    RX_OP_RETIRE(0, 10, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_RWM_CONN(10)
    RX_OP_CHECK_RWM_STREAM(0, 10)
    RX_OP_CHECK_CWM_CONN(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(1, INIT_S_WINDOW_SIZE)

    RX_OP_RETIRE(1, 42, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_RWM_CONN(52)
    RX_OP_CHECK_RWM_STREAM(1, 42)
    RX_OP_CHECK_CWM_CONN(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(1, INIT_S_WINDOW_SIZE)

    RX_OP_CHECK_CHANGED_CONN(0, 0)

    /* FC limited by stream but not connection */
    RX_OP_STEP_TIME(1000 * OSSL_TIME_MS)
    RX_OP_RX(0, INIT_S_WINDOW_SIZE, 0)
    RX_OP_CHECK_SWM_CONN(INIT_S_WINDOW_SIZE + 42)
    RX_OP_CHECK_SWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_SWM_STREAM(1, 42)
    RX_OP_CHECK_CWM_CONN(INIT_WINDOW_SIZE)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE)

    /* We bump CWM when more than 1/4 of the window has been retired */
    RX_OP_RETIRE(0, INIT_S_WINDOW_SIZE - 10, 50 * OSSL_TIME_MS, 0)
    RX_OP_CHECK_CWM_STREAM(0, INIT_S_WINDOW_SIZE * 2)
    RX_OP_CHECK_CHANGED_STREAM(0, 1, 0)
    RX_OP_CHECK_CHANGED_STREAM(0, 1, 1)
    RX_OP_CHECK_CHANGED_STREAM(0, 0, 0)

    /*
     * This is more than 1/4 of the connection window, so CWM will
     * be bumped here too.
     */
    RX_OP_CHECK_CWM_CONN(INIT_S_WINDOW_SIZE + INIT_WINDOW_SIZE + 42)
    RX_OP_CHECK_RWM_CONN(INIT_S_WINDOW_SIZE + 42)
    RX_OP_CHECK_RWM_STREAM(0, INIT_S_WINDOW_SIZE)
    RX_OP_CHECK_RWM_STREAM(1, 42)
    RX_OP_CHECK_CHANGED_CONN(1, 0)
    RX_OP_CHECK_CHANGED_CONN(1, 1)
    RX_OP_CHECK_CHANGED_CONN(0, 0)
    RX_OP_CHECK_ERROR_CONN(0, 0)
    RX_OP_CHECK_ERROR_STREAM(0, 0, 0)
    RX_OP_CHECK_ERROR_STREAM(1, 0, 0)

    /* Test exceeding limit at stream level. */
    RX_OP_RX(0, INIT_S_WINDOW_SIZE * 2 + 1, 0)
    RX_OP_CHECK_ERROR_STREAM(0, OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 0)
    RX_OP_CHECK_ERROR_STREAM(0, OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 1)
    RX_OP_CHECK_ERROR_STREAM(0, 0, 0)
    RX_OP_CHECK_ERROR_CONN(0, 0) /* doesn't affect conn */

    /* Test exceeding limit at connection level. */
    RX_OP_RX(0, INIT_WINDOW_SIZE * 2, 0)
    RX_OP_CHECK_ERROR_CONN(OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 0)
    RX_OP_CHECK_ERROR_CONN(OSSL_QUIC_ERR_FLOW_CONTROL_ERROR, 1)
    RX_OP_CHECK_ERROR_CONN(0, 0)

    RX_OP_END
};

static const struct rx_test_op *rx_scripts[] = {
    rx_script_1,
    rx_script_2
};

static int run_rxfc_script(const struct rx_test_op *script)
{
#define MAX_STREAMS     3
    int testresult = 0;
    const struct rx_test_op *op = script;
    QUIC_RXFC conn_rxfc = {0}, stream_rxfc[MAX_STREAMS] = {0}; /* coverity */
    char stream_init_done[MAX_STREAMS] = {0};
    int conn_init_done = 0;

    cur_time = ossl_time_zero();

    for (; op->op != RX_OPC_END; ++op) {
        switch (op->op) {
            case RX_OPC_INIT_CONN:
                if (!TEST_true(ossl_quic_rxfc_init(&conn_rxfc, 0,
                                                   op->arg0, op->arg1,
                                                   fake_now, NULL)))
                    goto err;

                conn_init_done = 1;
                break;

            case RX_OPC_INIT_STREAM:
                if (!TEST_size_t_lt(op->stream_idx, OSSL_NELEM(stream_rxfc))
                    || !TEST_true(conn_init_done))
                    goto err;

                if (!TEST_true(ossl_quic_rxfc_init(&stream_rxfc[op->stream_idx],
                                                   &conn_rxfc,
                                                   op->arg0, op->arg1,
                                                   fake_now, NULL)))
                    goto err;

                stream_init_done[op->stream_idx] = 1;
                break;

            case RX_OPC_RX:
                if (!TEST_true(conn_init_done && op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;

                if (!TEST_true(ossl_quic_rxfc_on_rx_stream_frame(&stream_rxfc[op->stream_idx],
                                                                 op->arg0,
                                                                 (int)op->arg1)))
                    goto err;

                break;

            case RX_OPC_RETIRE:
                if (!TEST_true(conn_init_done && op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;

                if (!TEST_int_eq(ossl_quic_rxfc_on_retire(&stream_rxfc[op->stream_idx],
                                                          op->arg0,
                                                          ossl_ticks2time(op->arg1)),
                                 !op->expect_fail))
                    goto err;

                break;
            case RX_OPC_CHECK_CWM_CONN:
                if (!TEST_true(conn_init_done))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_cwm(&conn_rxfc),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_CWM_STREAM:
                if (!TEST_true(op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_cwm(&stream_rxfc[op->stream_idx]),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_SWM_CONN:
                if (!TEST_true(conn_init_done))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_swm(&conn_rxfc),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_SWM_STREAM:
                if (!TEST_true(op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_swm(&stream_rxfc[op->stream_idx]),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_RWM_CONN:
                if (!TEST_true(conn_init_done))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_rwm(&conn_rxfc),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_RWM_STREAM:
                if (!TEST_true(op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;
                if (!TEST_uint64_t_eq(ossl_quic_rxfc_get_rwm(&stream_rxfc[op->stream_idx]),
                                      op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_CHANGED_CONN:
                if (!TEST_true(conn_init_done))
                    goto err;
                if (!TEST_int_eq(ossl_quic_rxfc_has_cwm_changed(&conn_rxfc,
                                                                (int)op->arg1),
                                 (int)op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_CHANGED_STREAM:
                if (!TEST_true(op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;
                if (!TEST_int_eq(ossl_quic_rxfc_has_cwm_changed(&stream_rxfc[op->stream_idx],
                                                                (int)op->arg1),
                                 (int)op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_ERROR_CONN:
                if (!TEST_true(conn_init_done))
                    goto err;
                if (!TEST_int_eq(ossl_quic_rxfc_get_error(&conn_rxfc,
                                                          (int)op->arg1),
                                 (int)op->arg0))
                    goto err;
                break;
            case RX_OPC_CHECK_ERROR_STREAM:
                if (!TEST_true(op->stream_idx < OSSL_NELEM(stream_rxfc)
                               && stream_init_done[op->stream_idx]))
                    goto err;
                if (!TEST_int_eq(ossl_quic_rxfc_get_error(&stream_rxfc[op->stream_idx],
                                                          (int)op->arg1),
                                 (int)op->arg0))
                    goto err;
                break;
            case RX_OPC_STEP_TIME:
                cur_time = ossl_time_add(cur_time, ossl_ticks2time(op->arg0));
                break;
            case RX_OPC_MSG:
                fprintf(stderr, "# %s\n", op->msg);
                break;
            default:
                goto err;
        }
    }

    testresult = 1;
err:
    return testresult;
}

static int test_rxfc(int idx)
{
    return run_rxfc_script(rx_scripts[idx]);
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_txfc, 2);
    ADD_ALL_TESTS(test_rxfc, OSSL_NELEM(rx_scripts));
    return 1;
}
