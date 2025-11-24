/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet.h"
#include "internal/quic_txpim.h"
#include "internal/quic_fifd.h"
#include "testutil.h"

static OSSL_TIME cur_time;

static OSSL_TIME fake_now(void *arg) {
    return cur_time;
}

static void step_time(uint64_t ms) {
    cur_time = ossl_time_add(cur_time, ossl_ms2time(ms));
}

static QUIC_SSTREAM *(*get_sstream_by_id_p)(uint64_t stream_id, uint32_t pn_space,
                                            void *arg);

static QUIC_SSTREAM *get_sstream_by_id(uint64_t stream_id, uint32_t pn_space,
                                       void *arg)
{
    return get_sstream_by_id_p(stream_id, pn_space, arg);
}

static void (*regen_frame_p)(uint64_t frame_type, uint64_t stream_id,
                             QUIC_TXPIM_PKT *pkt, void *arg);

static void regen_frame(uint64_t frame_type, uint64_t stream_id,
                        QUIC_TXPIM_PKT *pkt, void *arg)
{
    regen_frame_p(frame_type, stream_id, pkt, arg);
}

static void confirm_frame(uint64_t frame_type, uint64_t stream_id,
                          QUIC_TXPIM_PKT *pkt, void *arg)
{}

static void sstream_updated(uint64_t stream_id, void *arg)
{}

typedef struct info_st {
    QUIC_FIFD fifd;
    OSSL_ACKM *ackm;
    QUIC_CFQ *cfq;
    QUIC_TXPIM *txpim;
    OSSL_STATM statm;
    OSSL_CC_DATA *ccdata;
    QUIC_SSTREAM *sstream[4];
} INFO;

static INFO *cur_info;
static int cb_fail;
static int cfq_freed;

/* ----------------------------------------------------------------------
 * 1. Test that a submitted packet, on ack, acks all streams inside of it
 *    Test that a submitted packet, on ack, calls the get by ID function
 *      correctly
 *    Test that a submitted packet, on ack, acks all fins inside it
 *    Test that a submitted packet, on ack, releases the TXPIM packet
 */
static QUIC_SSTREAM *sstream_expect(uint64_t stream_id, uint32_t pn_space,
                                    void *arg)
{
    if (stream_id == 42 || stream_id == 43)
        return cur_info->sstream[stream_id - 42];

    cb_fail = 1;
    return NULL;
}

static uint64_t regen_frame_type[16];
static uint64_t regen_stream_id[16];
static size_t regen_count;

static void regen_expect(uint64_t frame_type, uint64_t stream_id,
                         QUIC_TXPIM_PKT *pkt, void *arg)
{
    regen_frame_type[regen_count] = frame_type;
    regen_stream_id[regen_count] = stream_id;
    ++regen_count;
}

static const unsigned char placeholder_data[] = "placeholder";

static void cfq_free_cb_(unsigned char *buf, size_t buf_len, void *arg)
{
    if (buf == placeholder_data && buf_len == sizeof(placeholder_data))
        cfq_freed = 1;
}

#define TEST_KIND_ACK       0
#define TEST_KIND_LOSS      1
#define TEST_KIND_DISCARD   2
#define TEST_KIND_NUM       3

static int test_generic(INFO *info, int kind)
{
    int testresult = 0;
    size_t i, consumed = 0;
    QUIC_TXPIM_PKT *pkt = NULL, *pkt2 = NULL;
    OSSL_QUIC_FRAME_STREAM hdr = {0};
    OSSL_QTX_IOVEC iov[2];
    size_t num_iov;
    QUIC_TXPIM_CHUNK chunk = {42, 0, 11, 0};
    OSSL_QUIC_FRAME_ACK ack = {0};
    OSSL_QUIC_ACK_RANGE ack_ranges[1] = {0};
    QUIC_CFQ_ITEM *cfq_item = NULL;
    uint32_t pn_space = (kind == TEST_KIND_DISCARD)
        ? QUIC_PN_SPACE_HANDSHAKE : QUIC_PN_SPACE_APP;

    cur_time            = ossl_seconds2time(1000);
    regen_count         = 0;

    get_sstream_by_id_p = sstream_expect;
    regen_frame_p       = regen_expect;

    if (!TEST_ptr(pkt = ossl_quic_txpim_pkt_alloc(info->txpim)))
        goto err;

    for (i = 0; i < 2; ++i) {
        num_iov = OSSL_NELEM(iov);
        if (!TEST_true(ossl_quic_sstream_append(info->sstream[i],
                                                (unsigned char *)"Test message",
                                                12, &consumed))
            || !TEST_size_t_eq(consumed, 12))
            goto err;

        if (i == 1)
            ossl_quic_sstream_fin(info->sstream[i]);

        if (!TEST_true(ossl_quic_sstream_get_stream_frame(info->sstream[i], 0,
                                                          &hdr, iov, &num_iov))
            || !TEST_int_eq(hdr.is_fin, i == 1)
            || !TEST_uint64_t_eq(hdr.offset, 0)
            || !TEST_uint64_t_eq(hdr.len, 12)
            || !TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(info->sstream[i]), 12)
            || !TEST_true(ossl_quic_sstream_mark_transmitted(info->sstream[i],
                                                             hdr.offset,
                                                             hdr.offset + hdr.len - 1)))
            goto err;

        if (i == 1 && !TEST_true(ossl_quic_sstream_mark_transmitted_fin(info->sstream[i],
                                                                        hdr.offset + hdr.len)))
            goto err;

        chunk.has_fin   = hdr.is_fin;
        chunk.stream_id = 42 + i;
        if (!TEST_true(ossl_quic_txpim_pkt_append_chunk(pkt, &chunk)))
            goto err;
    }

    cfq_freed = 0;
    if (!TEST_ptr(cfq_item = ossl_quic_cfq_add_frame(info->cfq, 10,
                                                     pn_space,
                                                     OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID, 0,
                                                     placeholder_data,
                                                     sizeof(placeholder_data),
                                                     cfq_free_cb_, NULL))
        || !TEST_ptr_eq(cfq_item, ossl_quic_cfq_get_priority_head(info->cfq, pn_space)))
        goto err;

    ossl_quic_txpim_pkt_add_cfq_item(pkt, cfq_item);

    pkt->ackm_pkt.pkt_num           = 0;
    pkt->ackm_pkt.pkt_space         = pn_space;
    pkt->ackm_pkt.largest_acked     = QUIC_PN_INVALID;
    pkt->ackm_pkt.num_bytes         = 50;
    pkt->ackm_pkt.time              = cur_time;
    pkt->ackm_pkt.is_inflight       = 1;
    pkt->ackm_pkt.is_ack_eliciting  = 1;
    if (kind == TEST_KIND_LOSS) {
        pkt->had_handshake_done_frame   = 1;
        pkt->had_max_data_frame         = 1;
        pkt->had_max_streams_bidi_frame = 1;
        pkt->had_max_streams_uni_frame  = 1;
        pkt->had_ack_frame              = 1;
    }

    ack_ranges[0].start = 0;
    ack_ranges[0].end   = 0;
    ack.ack_ranges      = ack_ranges;
    ack.num_ack_ranges  = 1;

    if (!TEST_true(ossl_quic_fifd_pkt_commit(&info->fifd, pkt)))
        goto err;

    /* CFQ item should have been marked as transmitted */
    if (!TEST_ptr_null(ossl_quic_cfq_get_priority_head(info->cfq, pn_space)))
        goto err;

    switch (kind) {
    case TEST_KIND_ACK:
        if (!TEST_true(ossl_ackm_on_rx_ack_frame(info->ackm, &ack,
                                                 pn_space,
                                                 cur_time)))
            goto err;

        for (i = 0; i < 2; ++i)
            if (!TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(info->sstream[i]), 0))
                goto err;

        /* This should fail, which proves the FIN was acked */
        if (!TEST_false(ossl_quic_sstream_mark_lost_fin(info->sstream[1])))
            goto err;

        /* CFQ item must have been released */
        if (!TEST_true(cfq_freed))
            goto err;

        /* No regen calls should have been made */
        if (!TEST_size_t_eq(regen_count, 0))
            goto err;

        break;

    case TEST_KIND_LOSS:
        /* Trigger loss detection via packet threshold. */
        if (!TEST_ptr(pkt2 = ossl_quic_txpim_pkt_alloc(info->txpim)))
            goto err;

        step_time(10000);
        pkt2->ackm_pkt.pkt_num          = 50;
        pkt2->ackm_pkt.pkt_space        = pn_space;
        pkt2->ackm_pkt.largest_acked    = QUIC_PN_INVALID;
        pkt2->ackm_pkt.num_bytes        = 50;
        pkt2->ackm_pkt.time             = cur_time;
        pkt2->ackm_pkt.is_inflight      = 1;
        pkt2->ackm_pkt.is_ack_eliciting = 1;

        ack_ranges[0].start = 50;
        ack_ranges[0].end   = 50;
        ack.ack_ranges      = ack_ranges;
        ack.num_ack_ranges  = 1;

        if (!TEST_true(ossl_quic_fifd_pkt_commit(&info->fifd, pkt2))
            || !TEST_true(ossl_ackm_on_rx_ack_frame(info->ackm, &ack,
                                                    pn_space, cur_time)))
            goto err;

        for (i = 0; i < 2; ++i) {
            num_iov = OSSL_NELEM(iov);
            /*
             * Stream data we sent must have been marked as lost; check by
             * ensuring it is returned again
             */
            if (!TEST_true(ossl_quic_sstream_get_stream_frame(info->sstream[i], 0,
                                                              &hdr, iov, &num_iov))
                || !TEST_uint64_t_eq(hdr.offset, 0)
                || !TEST_uint64_t_eq(hdr.len, 12))
                goto err;
        }

        /* FC frame should have regenerated for each stream */
        if (!TEST_size_t_eq(regen_count, 7)
            || !TEST_uint64_t_eq(regen_stream_id[0], 42)
            || !TEST_uint64_t_eq(regen_frame_type[0], OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
            || !TEST_uint64_t_eq(regen_stream_id[1], 43)
            || !TEST_uint64_t_eq(regen_frame_type[1], OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
            || !TEST_uint64_t_eq(regen_frame_type[2], OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE)
            || !TEST_uint64_t_eq(regen_stream_id[2], UINT64_MAX)
            || !TEST_uint64_t_eq(regen_frame_type[3], OSSL_QUIC_FRAME_TYPE_MAX_DATA)
            || !TEST_uint64_t_eq(regen_stream_id[3], UINT64_MAX)
            || !TEST_uint64_t_eq(regen_frame_type[4], OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI)
            || !TEST_uint64_t_eq(regen_stream_id[4], UINT64_MAX)
            || !TEST_uint64_t_eq(regen_frame_type[5], OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI)
            || !TEST_uint64_t_eq(regen_stream_id[5], UINT64_MAX)
            || !TEST_uint64_t_eq(regen_frame_type[6], OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN)
            || !TEST_uint64_t_eq(regen_stream_id[6], UINT64_MAX))
            goto err;

        /* CFQ item should have been marked as lost */
        if (!TEST_ptr_eq(cfq_item, ossl_quic_cfq_get_priority_head(info->cfq, pn_space)))
            goto err;

        /* FIN should have been marked as lost */
        num_iov = OSSL_NELEM(iov);
        if (!TEST_true(ossl_quic_sstream_get_stream_frame(info->sstream[1], 1,
                                                          &hdr, iov, &num_iov))
            || !TEST_true(hdr.is_fin)
            || !TEST_uint64_t_eq(hdr.len, 0))
            goto err;

        break;

    case TEST_KIND_DISCARD:
        if (!TEST_true(ossl_ackm_on_pkt_space_discarded(info->ackm, pn_space)))
            goto err;

        /* CFQ item must have been released */
        if (!TEST_true(cfq_freed))
            goto err;

        break;

    default:
        goto err;
    }

    /* TXPIM must have been released */
    if (!TEST_size_t_eq(ossl_quic_txpim_get_in_use(info->txpim), 0))
        goto err;

    testresult = 1;
err:
    return testresult;
}

static int test_fifd(int idx)
{
    int testresult = 0;
    INFO info = {0};
    size_t i;

    cur_info = &info;
    cb_fail = 0;

    if (!TEST_true(ossl_statm_init(&info.statm))
        || !TEST_ptr(info.ccdata = ossl_cc_dummy_method.new(fake_now, NULL))
        || !TEST_ptr(info.ackm = ossl_ackm_new(fake_now, NULL,
                                               &info.statm,
                                               &ossl_cc_dummy_method,
                                               info.ccdata))
        || !TEST_true(ossl_ackm_on_handshake_confirmed(info.ackm))
        || !TEST_ptr(info.cfq = ossl_quic_cfq_new())
        || !TEST_ptr(info.txpim = ossl_quic_txpim_new())
        || !TEST_true(ossl_quic_fifd_init(&info.fifd, info.cfq, info.ackm,
                                          info.txpim,
                                          get_sstream_by_id, NULL,
                                          regen_frame, NULL,
                                          confirm_frame, NULL,
                                          sstream_updated, NULL,
                                          NULL, NULL)))
        goto err;

    for (i = 0; i < OSSL_NELEM(info.sstream); ++i)
        if (!TEST_ptr(info.sstream[i] = ossl_quic_sstream_new(1024)))
            goto err;

    ossl_statm_update_rtt(&info.statm, ossl_time_zero(), ossl_ms2time(1));

    if (!TEST_true(test_generic(&info, idx))
        || !TEST_false(cb_fail))
        goto err;

    testresult = 1;
err:
    ossl_quic_fifd_cleanup(&info.fifd);
    ossl_quic_cfq_free(info.cfq);
    ossl_quic_txpim_free(info.txpim);
    ossl_ackm_free(info.ackm);
    ossl_statm_destroy(&info.statm);
    if (info.ccdata != NULL)
        ossl_cc_dummy_method.free(info.ccdata);
    for (i = 0; i < OSSL_NELEM(info.sstream); ++i)
        ossl_quic_sstream_free(info.sstream[i]);
    cur_info = NULL;
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_fifd, TEST_KIND_NUM);
    return 1;
}
