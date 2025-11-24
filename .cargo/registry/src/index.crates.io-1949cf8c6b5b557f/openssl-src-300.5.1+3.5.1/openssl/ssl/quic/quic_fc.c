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
#include "internal/common.h"
#include "internal/safe_math.h"
#include <assert.h>

OSSL_SAFE_MATH_UNSIGNED(uint64_t, uint64_t)

/*
 * TX Flow Controller (TXFC)
 * =========================
 */

int ossl_quic_txfc_init(QUIC_TXFC *txfc, QUIC_TXFC *conn_txfc)
{
    if (conn_txfc != NULL && conn_txfc->parent != NULL)
        return 0;

    txfc->swm                   = 0;
    txfc->cwm                   = 0;
    txfc->parent                = conn_txfc;
    txfc->has_become_blocked    = 0;
    return 1;
}

QUIC_TXFC *ossl_quic_txfc_get_parent(QUIC_TXFC *txfc)
{
    return txfc->parent;
}

int ossl_quic_txfc_bump_cwm(QUIC_TXFC *txfc, uint64_t cwm)
{
    if (cwm <= txfc->cwm)
        return 0;

    txfc->cwm = cwm;
    return 1;
}

uint64_t ossl_quic_txfc_get_credit_local(QUIC_TXFC *txfc, uint64_t consumed)
{
    assert((txfc->swm + consumed) <= txfc->cwm);
    return txfc->cwm - (consumed + txfc->swm);
}

uint64_t ossl_quic_txfc_get_credit(QUIC_TXFC *txfc, uint64_t consumed)
{
    uint64_t r, conn_r;

    r = ossl_quic_txfc_get_credit_local(txfc, 0);

    if (txfc->parent != NULL) {
        assert(txfc->parent->parent == NULL);
        conn_r = ossl_quic_txfc_get_credit_local(txfc->parent, consumed);
        if (conn_r < r)
            r = conn_r;
    }

    return r;
}

int ossl_quic_txfc_consume_credit_local(QUIC_TXFC *txfc, uint64_t num_bytes)
{
    int ok = 1;
    uint64_t credit = ossl_quic_txfc_get_credit_local(txfc, 0);

    if (num_bytes > credit) {
        ok = 0;
        num_bytes = credit;
    }

    if (num_bytes > 0 && num_bytes == credit)
        txfc->has_become_blocked = 1;

    txfc->swm += num_bytes;
    return ok;
}

int ossl_quic_txfc_consume_credit(QUIC_TXFC *txfc, uint64_t num_bytes)
{
    int ok = ossl_quic_txfc_consume_credit_local(txfc, num_bytes);

    if (txfc->parent != NULL) {
        assert(txfc->parent->parent == NULL);
        if (!ossl_quic_txfc_consume_credit_local(txfc->parent, num_bytes))
            return 0;
    }

    return ok;
}

int ossl_quic_txfc_has_become_blocked(QUIC_TXFC *txfc, int clear)
{
    int r = txfc->has_become_blocked;

    if (clear)
        txfc->has_become_blocked = 0;

    return r;
}

uint64_t ossl_quic_txfc_get_cwm(QUIC_TXFC *txfc)
{
    return txfc->cwm;
}

uint64_t ossl_quic_txfc_get_swm(QUIC_TXFC *txfc)
{
    return txfc->swm;
}

/*
 * RX Flow Controller (RXFC)
 * =========================
 */

int ossl_quic_rxfc_init(QUIC_RXFC *rxfc, QUIC_RXFC *conn_rxfc,
                        uint64_t initial_window_size,
                        uint64_t max_window_size,
                        OSSL_TIME (*now)(void *now_arg),
                        void *now_arg)
{
    if (conn_rxfc != NULL && conn_rxfc->parent != NULL)
        return 0;

    rxfc->swm               = 0;
    rxfc->cwm               = initial_window_size;
    rxfc->rwm               = 0;
    rxfc->esrwm             = 0;
    rxfc->hwm               = 0;
    rxfc->cur_window_size   = initial_window_size;
    rxfc->max_window_size   = max_window_size;
    rxfc->parent            = conn_rxfc;
    rxfc->error_code        = 0;
    rxfc->has_cwm_changed   = 0;
    rxfc->epoch_start       = ossl_time_zero();
    rxfc->now               = now;
    rxfc->now_arg           = now_arg;
    rxfc->is_fin            = 0;
    rxfc->standalone        = 0;
    return 1;
}

int ossl_quic_rxfc_init_standalone(QUIC_RXFC *rxfc,
                                   uint64_t initial_window_size,
                                   OSSL_TIME (*now)(void *arg),
                                   void *now_arg)
{
    if (!ossl_quic_rxfc_init(rxfc, NULL,
                             initial_window_size, initial_window_size,
                             now, now_arg))
        return 0;

    rxfc->standalone = 1;
    return 1;
}

QUIC_RXFC *ossl_quic_rxfc_get_parent(QUIC_RXFC *rxfc)
{
    return rxfc->parent;
}

void ossl_quic_rxfc_set_max_window_size(QUIC_RXFC *rxfc,
                                        size_t max_window_size)
{
    rxfc->max_window_size = max_window_size;
}

static void rxfc_start_epoch(QUIC_RXFC *rxfc)
{
    rxfc->epoch_start   = rxfc->now(rxfc->now_arg);
    rxfc->esrwm         = rxfc->rwm;
}

static int on_rx_controlled_bytes(QUIC_RXFC *rxfc, uint64_t num_bytes)
{
    int ok = 1;
    uint64_t credit = rxfc->cwm - rxfc->swm;

    if (num_bytes > credit) {
        ok = 0;
        num_bytes = credit;
        rxfc->error_code = OSSL_QUIC_ERR_FLOW_CONTROL_ERROR;
    }

    rxfc->swm += num_bytes;
    return ok;
}

int ossl_quic_rxfc_on_rx_stream_frame(QUIC_RXFC *rxfc, uint64_t end, int is_fin)
{
    uint64_t delta;

    if (!rxfc->standalone && rxfc->parent == NULL)
        return 0;

    if (rxfc->is_fin && ((is_fin && rxfc->hwm != end) || end > rxfc->hwm)) {
        /* Stream size cannot change after the stream is finished */
        rxfc->error_code = OSSL_QUIC_ERR_FINAL_SIZE_ERROR;
        return 1; /* not a caller error */
    }

    if (is_fin)
        rxfc->is_fin = 1;

    if (end > rxfc->hwm) {
        delta = end - rxfc->hwm;
        rxfc->hwm = end;

        on_rx_controlled_bytes(rxfc, delta);             /* result ignored */
        if (rxfc->parent != NULL)
            on_rx_controlled_bytes(rxfc->parent, delta); /* result ignored */
    } else if (end < rxfc->hwm && is_fin) {
        rxfc->error_code = OSSL_QUIC_ERR_FINAL_SIZE_ERROR;
        return 1; /* not a caller error */
    }

    return 1;
}

/* threshold = 3/4 */
#define WINDOW_THRESHOLD_NUM 3
#define WINDOW_THRESHOLD_DEN 4

static int rxfc_cwm_bump_desired(QUIC_RXFC *rxfc)
{
    int err = 0;
    uint64_t window_rem = rxfc->cwm - rxfc->rwm;
    uint64_t threshold
        = safe_muldiv_uint64_t(rxfc->cur_window_size,
                               WINDOW_THRESHOLD_NUM, WINDOW_THRESHOLD_DEN, &err);

    if (err)
        /*
         * Extremely large window should never occur, but if it does, just use
         * 1/2 as the threshold.
         */
        threshold = rxfc->cur_window_size / 2;

    /*
     * No point emitting a new MAX_STREAM_DATA frame if the stream has a final
     * size.
     */
    return !rxfc->is_fin && window_rem <= threshold;
}

static int rxfc_should_bump_window_size(QUIC_RXFC *rxfc, OSSL_TIME rtt)
{
    /*
     * dt: time since start of epoch
     * b:  bytes of window consumed since start of epoch
     * dw: proportion of window consumed since start of epoch
     * T_window: time it will take to use up the entire window, based on dt, dw
     * RTT: The current estimated RTT.
     *
     * b        = rwm - esrwm
     * dw       = b / window_size
     * T_window = dt / dw
     * T_window = dt / (b / window_size)
     * T_window = (dt * window_size) / b
     *
     * We bump the window size if T_window < 4 * RTT.
     *
     * We leave the division by b on the LHS to reduce the risk of overflowing
     * our 64-bit nanosecond representation, which will afford plenty of
     * precision left over after the division anyway.
     */
    uint64_t  b = rxfc->rwm - rxfc->esrwm;
    OSSL_TIME now, dt, t_window;

    if (b == 0)
        return 0;

    now      = rxfc->now(rxfc->now_arg);
    dt       = ossl_time_subtract(now, rxfc->epoch_start);
    t_window = ossl_time_muldiv(dt, rxfc->cur_window_size, b);

    return ossl_time_compare(t_window, ossl_time_multiply(rtt, 4)) < 0;
}

static void rxfc_adjust_window_size(QUIC_RXFC *rxfc, uint64_t min_window_size,
                                    OSSL_TIME rtt)
{
    /* Are we sending updates too often? */
    uint64_t new_window_size;

    new_window_size = rxfc->cur_window_size;

    if (rxfc_should_bump_window_size(rxfc, rtt))
        new_window_size *= 2;

    if (new_window_size < min_window_size)
        new_window_size = min_window_size;
    if (new_window_size > rxfc->max_window_size) /* takes precedence over min size */
        new_window_size = rxfc->max_window_size;

    rxfc->cur_window_size = new_window_size;
    rxfc_start_epoch(rxfc);
}

static void rxfc_update_cwm(QUIC_RXFC *rxfc, uint64_t min_window_size,
                            OSSL_TIME rtt)
{
    uint64_t new_cwm;

    if (!rxfc_cwm_bump_desired(rxfc))
        return;

    rxfc_adjust_window_size(rxfc, min_window_size, rtt);

    new_cwm = rxfc->rwm + rxfc->cur_window_size;
    if (new_cwm > rxfc->cwm) {
        rxfc->cwm = new_cwm;
        rxfc->has_cwm_changed = 1;
    }
}

static int rxfc_on_retire(QUIC_RXFC *rxfc, uint64_t num_bytes,
                          uint64_t min_window_size,
                          OSSL_TIME rtt)
{
    if (ossl_time_is_zero(rxfc->epoch_start))
        /* This happens when we retire our first ever bytes. */
        rxfc_start_epoch(rxfc);

    rxfc->rwm += num_bytes;
    rxfc_update_cwm(rxfc, min_window_size, rtt);
    return 1;
}

int ossl_quic_rxfc_on_retire(QUIC_RXFC *rxfc,
                             uint64_t num_bytes,
                             OSSL_TIME rtt)
{
    if (rxfc->parent == NULL && !rxfc->standalone)
        return 0;

    if (num_bytes == 0)
        return 1;

    if (rxfc->rwm + num_bytes > rxfc->swm)
        /* Impossible for us to retire more bytes than we have received. */
        return 0;

    rxfc_on_retire(rxfc, num_bytes, 0, rtt);

    if (!rxfc->standalone)
        rxfc_on_retire(rxfc->parent, num_bytes, rxfc->cur_window_size, rtt);

    return 1;
}

uint64_t ossl_quic_rxfc_get_cwm(const QUIC_RXFC *rxfc)
{
    return rxfc->cwm;
}

uint64_t ossl_quic_rxfc_get_swm(const QUIC_RXFC *rxfc)
{
    return rxfc->swm;
}

uint64_t ossl_quic_rxfc_get_rwm(const QUIC_RXFC *rxfc)
{
    return rxfc->rwm;
}

uint64_t ossl_quic_rxfc_get_credit(const QUIC_RXFC *rxfc)
{
    return ossl_quic_rxfc_get_cwm(rxfc) - ossl_quic_rxfc_get_swm(rxfc);
}

int ossl_quic_rxfc_has_cwm_changed(QUIC_RXFC *rxfc, int clear)
{
    int r = rxfc->has_cwm_changed;

    if (clear)
        rxfc->has_cwm_changed = 0;

    return r;
}

int ossl_quic_rxfc_get_error(QUIC_RXFC *rxfc, int clear)
{
    int r = rxfc->error_code;

    if (clear)
        rxfc->error_code = 0;

    return r;
}

int ossl_quic_rxfc_get_final_size(const QUIC_RXFC *rxfc, uint64_t *final_size)
{
    if (!rxfc->is_fin)
        return 0;

    if (final_size != NULL)
        *final_size = rxfc->hwm;

    return 1;
}
