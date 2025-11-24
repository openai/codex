/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_cc.h"
#include "internal/quic_types.h"

typedef struct ossl_cc_dummy_st {
    size_t max_dgram_len;
    size_t *p_diag_max_dgram_len;
} OSSL_CC_DUMMY;

static void dummy_update_diag(OSSL_CC_DUMMY *d);

static OSSL_CC_DATA *dummy_new(OSSL_TIME (*now_cb)(void *arg),
                               void *now_cb_arg)
{
    OSSL_CC_DUMMY *d = OPENSSL_zalloc(sizeof(*d));

    if (d == NULL)
        return NULL;

    d->max_dgram_len = QUIC_MIN_INITIAL_DGRAM_LEN;
    return (OSSL_CC_DATA *)d;
}

static void dummy_free(OSSL_CC_DATA *cc)
{
    OPENSSL_free(cc);
}

static void dummy_reset(OSSL_CC_DATA *cc)
{

}

static int dummy_set_input_params(OSSL_CC_DATA *cc, const OSSL_PARAM *params)
{
    OSSL_CC_DUMMY *d = (OSSL_CC_DUMMY *)cc;
    const OSSL_PARAM *p;
    size_t value;

    p = OSSL_PARAM_locate_const(params, OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &value))
            return 0;
        if (value < QUIC_MIN_INITIAL_DGRAM_LEN)
            return 0;

        d->max_dgram_len = value;
        dummy_update_diag(d);
    }

    return 1;
}

static int dummy_bind_diagnostic(OSSL_CC_DATA *cc, OSSL_PARAM *params)
{
    OSSL_CC_DUMMY *d = (OSSL_CC_DUMMY *)cc;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UNSIGNED_INTEGER
            || p->data_size != sizeof(size_t))
            return 0;

        d->p_diag_max_dgram_len = p->data;
    }

    dummy_update_diag(d);
    return 1;
}

static int dummy_unbind_diagnostic(OSSL_CC_DATA *cc, OSSL_PARAM *params)
{
    OSSL_CC_DUMMY *d = (OSSL_CC_DUMMY *)cc;

    if (OSSL_PARAM_locate_const(params, OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN)
        != NULL)
        d->p_diag_max_dgram_len = NULL;

    return 1;
}

static void dummy_update_diag(OSSL_CC_DUMMY *d)
{
    if (d->p_diag_max_dgram_len != NULL)
        *d->p_diag_max_dgram_len = d->max_dgram_len;
}

static uint64_t dummy_get_tx_allowance(OSSL_CC_DATA *cc)
{
    return SIZE_MAX;
}

static OSSL_TIME dummy_get_wakeup_deadline(OSSL_CC_DATA *cc)
{
    return ossl_time_infinite();
}

static int dummy_on_data_sent(OSSL_CC_DATA *cc,
                              uint64_t num_bytes)
{
    return 1;
}

static int dummy_on_data_acked(OSSL_CC_DATA *cc,
                               const OSSL_CC_ACK_INFO *info)
{
    return 1;
}

static int dummy_on_data_lost(OSSL_CC_DATA *cc,
                              const OSSL_CC_LOSS_INFO *info)
{
    return 1;
}

static int dummy_on_data_lost_finished(OSSL_CC_DATA *cc,
                                       uint32_t flags)
{
    return 1;
}

static int dummy_on_data_invalidated(OSSL_CC_DATA *cc,
                                     uint64_t num_bytes)
{
    return 1;
}

const OSSL_CC_METHOD ossl_cc_dummy_method = {
    dummy_new,
    dummy_free,
    dummy_reset,
    dummy_set_input_params,
    dummy_bind_diagnostic,
    dummy_unbind_diagnostic,
    dummy_get_tx_allowance,
    dummy_get_wakeup_deadline,
    dummy_on_data_sent,
    dummy_on_data_acked,
    dummy_on_data_lost,
    dummy_on_data_lost_finished,
    dummy_on_data_invalidated,
};
