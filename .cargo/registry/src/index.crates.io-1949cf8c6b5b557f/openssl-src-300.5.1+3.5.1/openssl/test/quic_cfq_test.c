/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet.h"
#include "internal/quic_cfq.h"
#include "internal/quic_wire.h"
#include "testutil.h"

static const unsigned char ref_buf[] = {
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19
};

static const uint32_t ref_priority[] = {
    90, 80, 70, 60, 95, 40, 94, 20, 10, 0
};

static const uint32_t ref_pn_space[] = {
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_HANDSHAKE,
    QUIC_PN_SPACE_HANDSHAKE,
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_INITIAL,
    QUIC_PN_SPACE_APP,
    QUIC_PN_SPACE_APP,
};

static const uint64_t ref_frame_type[] = {
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
    OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID,
};

static const uint32_t expect[QUIC_PN_SPACE_NUM][11] = {
    { 4, 6, 0, 3, 5, 7, UINT32_MAX },
    { 1, 2, UINT32_MAX },
    { 8, 9, UINT32_MAX },
};

static QUIC_CFQ_ITEM *items[QUIC_PN_SPACE_NUM][10];

static unsigned char *g_free;
static size_t g_free_len;

static void free_cb(unsigned char *buf, size_t buf_len, void *arg)
{
    g_free      = buf;
    g_free_len  = buf_len;
}

static int check(QUIC_CFQ *cfq)
{
    int testresult = 0;
    QUIC_CFQ_ITEM *item;
    size_t i;
    uint32_t pn_space;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        for (i = 0, item = ossl_quic_cfq_get_priority_head(cfq, pn_space);;
             ++i, item = ossl_quic_cfq_item_get_priority_next(item, pn_space)) {

            if (expect[pn_space][i] == UINT32_MAX) {
                if (!TEST_ptr_null(item))
                    goto err;

                break;
            }

            items[pn_space][i] = item;

            if (!TEST_ptr(item)
                || !TEST_ptr_eq(ossl_quic_cfq_item_get_encoded(item),
                                ref_buf + expect[pn_space][i])
                || !TEST_int_eq(ossl_quic_cfq_item_get_pn_space(item), pn_space)
                || !TEST_int_eq(ossl_quic_cfq_item_get_state(item),
                                QUIC_CFQ_STATE_NEW))
                goto err;
        }

    testresult = 1;
err:
    return testresult;
}

static int test_cfq(void)
{
    int testresult = 0;
    QUIC_CFQ *cfq = NULL;
    QUIC_CFQ_ITEM *item, *inext;
    size_t i;
    uint32_t pn_space;

    if (!TEST_ptr(cfq = ossl_quic_cfq_new()))
        goto err;

    g_free      = NULL;
    g_free_len  = 0;

    for (i = 0; i < OSSL_NELEM(ref_buf); ++i) {
        if (!TEST_ptr(item = ossl_quic_cfq_add_frame(cfq, ref_priority[i],
                                                     ref_pn_space[i],
                                                     ref_frame_type[i], 0,
                                                     ref_buf + i,
                                                     1,
                                                     free_cb,
                                                     NULL))
            || !TEST_int_eq(ossl_quic_cfq_item_get_state(item),
                            QUIC_CFQ_STATE_NEW)
            || !TEST_uint_eq(ossl_quic_cfq_item_get_pn_space(item),
                             ref_pn_space[i])
            || !TEST_uint64_t_eq(ossl_quic_cfq_item_get_frame_type(item),
                                 ref_frame_type[i])
            || !TEST_ptr_eq(ossl_quic_cfq_item_get_encoded(item),
                            ref_buf + i)
            || !TEST_size_t_eq(ossl_quic_cfq_item_get_encoded_len(item),
                               1))
            goto err;
    }

    if (!check(cfq))
        goto err;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        for (item = ossl_quic_cfq_get_priority_head(cfq, pn_space);
             item != NULL; item = inext) {
            inext = ossl_quic_cfq_item_get_priority_next(item, pn_space);

            ossl_quic_cfq_mark_tx(cfq, item);
        }

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        if (!TEST_ptr_null(ossl_quic_cfq_get_priority_head(cfq, pn_space)))
            goto err;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        for (i = 0; i < OSSL_NELEM(items[0]); ++i)
            if (items[pn_space][i] != NULL)
                ossl_quic_cfq_mark_lost(cfq, items[pn_space][i], UINT32_MAX);

    if (!check(cfq))
        goto err;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        for (i = 0; i < OSSL_NELEM(items[0]); ++i)
            if (items[pn_space][i] != NULL)
                ossl_quic_cfq_release(cfq, items[pn_space][i]);

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        if (!TEST_ptr_null(ossl_quic_cfq_get_priority_head(cfq, pn_space)))
            goto err;

    testresult = 1;
err:
    ossl_quic_cfq_free(cfq);
    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_cfq);
    return 1;
}
