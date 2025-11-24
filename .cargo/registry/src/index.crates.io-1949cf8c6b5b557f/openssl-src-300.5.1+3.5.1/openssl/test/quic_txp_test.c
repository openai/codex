/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "internal/packet.h"
#include "internal/quic_txp.h"
#include "internal/quic_statm.h"
#include "internal/quic_demux.h"
#include "internal/quic_record_rx.h"
#include "testutil.h"
#include "quic_record_test_util.h"

static const QUIC_CONN_ID scid_1 = {
    1, { 0x5f }
};

static const QUIC_CONN_ID dcid_1 = {
    8, { 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8 }
};

static const QUIC_CONN_ID cid_1 = {
    8, { 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8 }
};

static const unsigned char reset_token_1[16] = {
    0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11,
    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x12,
};

static const unsigned char secret_1[32] = {
    0x01
};

static OSSL_TIME fake_now(void *arg)
{
    return ossl_time_now(); /* TODO */
}

struct helper {
    OSSL_QUIC_TX_PACKETISER         *txp;
    OSSL_QUIC_TX_PACKETISER_ARGS    args;
    OSSL_QTX_ARGS                   qtx_args;
    BIO                             *bio1, *bio2;
    QUIC_TXFC                       conn_txfc;
    QUIC_RXFC                       conn_rxfc, stream_rxfc;
    QUIC_RXFC                       max_streams_bidi_rxfc, max_streams_uni_rxfc;
    OSSL_STATM                      statm;
    OSSL_CC_DATA                    *cc_data;
    const OSSL_CC_METHOD            *cc_method;
    QUIC_STREAM_MAP                 qsm;
    char                            have_statm, have_qsm;
    QUIC_DEMUX                      *demux;
    OSSL_QRX                        *qrx;
    OSSL_QRX_ARGS                   qrx_args;
    OSSL_QRX_PKT                    *qrx_pkt;
    PACKET                          pkt;
    uint64_t                        frame_type;
    union {
        uint64_t                        max_data;
        OSSL_QUIC_FRAME_NEW_CONN_ID     new_conn_id;
        OSSL_QUIC_FRAME_ACK             ack;
        struct {
            const unsigned char *token;
            size_t              token_len;
        } new_token;
        OSSL_QUIC_FRAME_CRYPTO          crypto;
        OSSL_QUIC_FRAME_STREAM          stream;
        OSSL_QUIC_FRAME_STOP_SENDING    stop_sending;
        OSSL_QUIC_FRAME_RESET_STREAM    reset_stream;
        OSSL_QUIC_FRAME_CONN_CLOSE      conn_close;
    } frame;
    OSSL_QUIC_ACK_RANGE     ack_ranges[16];
};

static void helper_cleanup(struct helper *h)
{
    size_t i;
    uint32_t pn_space;

    ossl_qrx_pkt_release(h->qrx_pkt);
    h->qrx_pkt = NULL;

    for (pn_space = QUIC_PN_SPACE_INITIAL;
         pn_space < QUIC_PN_SPACE_NUM;
         ++pn_space)
        ossl_ackm_on_pkt_space_discarded(h->args.ackm, pn_space);

    ossl_quic_tx_packetiser_free(h->txp);
    ossl_qtx_free(h->args.qtx);
    ossl_quic_txpim_free(h->args.txpim);
    ossl_quic_cfq_free(h->args.cfq);
    if (h->cc_data != NULL)
        h->cc_method->free(h->cc_data);
    if (h->have_statm)
        ossl_statm_destroy(&h->statm);
    if (h->have_qsm)
        ossl_quic_stream_map_cleanup(&h->qsm);
    for (i = 0; i < QUIC_PN_SPACE_NUM; ++i)
        ossl_quic_sstream_free(h->args.crypto[i]);
    ossl_ackm_free(h->args.ackm);
    ossl_qrx_free(h->qrx);
    ossl_quic_demux_free(h->demux);
    BIO_free(h->bio1);
    BIO_free(h->bio2);
}

static void demux_default_handler(QUIC_URXE *e, void *arg,
                                  const QUIC_CONN_ID *dcid)
{
    struct helper *h = arg;

    if (dcid == NULL || !ossl_quic_conn_id_eq(dcid, &dcid_1))
        return;

    ossl_qrx_inject_urxe(h->qrx, e);
}

static int helper_init(struct helper *h)
{
    int rc = 0;
    size_t i;

    memset(h, 0, sizeof(*h));

    /* Initialisation */
    if (!TEST_true(BIO_new_bio_dgram_pair(&h->bio1, 0, &h->bio2, 0)))
        goto err;

    h->qtx_args.bio    = h->bio1;
    h->qtx_args.mdpl   = 1200;

    if (!TEST_ptr(h->args.qtx = ossl_qtx_new(&h->qtx_args)))
        goto err;

    if (!TEST_ptr(h->args.txpim = ossl_quic_txpim_new()))
        goto err;

    if (!TEST_ptr(h->args.cfq = ossl_quic_cfq_new()))
        goto err;

    if (!TEST_true(ossl_quic_txfc_init(&h->conn_txfc, NULL)))
        goto err;

    if (!TEST_true(ossl_quic_rxfc_init(&h->conn_rxfc, NULL,
                                       2 * 1024 * 1024,
                                       10 * 1024 * 1024,
                                       fake_now,
                                       NULL)))
        goto err;

    if (!TEST_true(ossl_quic_rxfc_init(&h->stream_rxfc, &h->conn_rxfc,
                                       1 * 1024 * 1024,
                                       5 * 1024 * 1024,
                                       fake_now,
                                       NULL)))
        goto err;

    if (!TEST_true(ossl_quic_rxfc_init(&h->max_streams_bidi_rxfc, NULL,
                                       100, 100,
                                       fake_now,
                                       NULL)))
        goto err;

    if (!TEST_true(ossl_quic_rxfc_init(&h->max_streams_uni_rxfc, NULL,
                                       100, 100,
                                       fake_now,
                                       NULL)))

    if (!TEST_true(ossl_statm_init(&h->statm)))
        goto err;

    h->have_statm = 1;

    h->cc_method = &ossl_cc_dummy_method;
    if (!TEST_ptr(h->cc_data = h->cc_method->new(fake_now, NULL)))
        goto err;

    if (!TEST_ptr(h->args.ackm = ossl_ackm_new(fake_now, NULL,
                                               &h->statm,
                                               h->cc_method,
                                               h->cc_data)))
        goto err;

    if (!TEST_true(ossl_quic_stream_map_init(&h->qsm, NULL, NULL,
                                             &h->max_streams_bidi_rxfc,
                                             &h->max_streams_uni_rxfc,
                                             /*is_server=*/0)))
        goto err;

    h->have_qsm = 1;

    for (i = 0; i < QUIC_PN_SPACE_NUM; ++i)
        if (!TEST_ptr(h->args.crypto[i] = ossl_quic_sstream_new(4096)))
            goto err;

    h->args.cur_scid                = scid_1;
    h->args.cur_dcid                = dcid_1;
    h->args.qsm                     = &h->qsm;
    h->args.conn_txfc               = &h->conn_txfc;
    h->args.conn_rxfc               = &h->conn_rxfc;
    h->args.max_streams_bidi_rxfc   = &h->max_streams_bidi_rxfc;
    h->args.max_streams_uni_rxfc    = &h->max_streams_uni_rxfc;
    h->args.cc_method               = h->cc_method;
    h->args.cc_data                 = h->cc_data;
    h->args.now                     = fake_now;
    h->args.protocol_version        = QUIC_VERSION_1;

    if (!TEST_ptr(h->txp = ossl_quic_tx_packetiser_new(&h->args)))
        goto err;

    /*
     * Our helper should always skip validation
     * as the tests are not written to expect delayed connections
     */
    ossl_quic_tx_packetiser_set_validated(h->txp);

    if (!TEST_ptr(h->demux = ossl_quic_demux_new(h->bio2, 8,
                                                 fake_now, NULL)))
        goto err;

    ossl_quic_demux_set_default_handler(h->demux, demux_default_handler, h);

    h->qrx_args.demux                  = h->demux;
    h->qrx_args.short_conn_id_len      = 8;
    h->qrx_args.max_deferred           = 32;

    if (!TEST_ptr(h->qrx = ossl_qrx_new(&h->qrx_args)))
        goto err;

    ossl_qrx_allow_1rtt_processing(h->qrx);

    rc = 1;
err:
    if (!rc)
        helper_cleanup(h);

    return rc;
}

#define OPK_END                     0   /* End of Script */
#define OPK_TXP_GENERATE            1   /* Call generate, expect packet output */
#define OPK_TXP_GENERATE_NONE       2   /* Call generate, expect no packet output */
#define OPK_RX_PKT                  3   /* Receive, expect packet */
#define OPK_RX_PKT_NONE             4   /* Receive, expect no packet */
#define OPK_EXPECT_DGRAM_LEN        5   /* Expect received datagram length in range */
#define OPK_EXPECT_FRAME            6   /* Expect next frame is of type */
#define OPK_EXPECT_INITIAL_TOKEN    7   /* Expect initial token buffer match */
#define OPK_EXPECT_HDR              8   /* Expect header structure match */
#define OPK_CHECK                   9   /* Call check function */
#define OPK_NEXT_FRAME              10  /* Next frame */
#define OPK_EXPECT_NO_FRAME         11  /* Expect no further frames */
#define OPK_PROVIDE_SECRET          12  /* Provide secret to QTX and QRX */
#define OPK_DISCARD_EL              13  /* Discard QTX EL */
#define OPK_CRYPTO_SEND             14  /* Push data into crypto send stream */
#define OPK_STREAM_NEW              15  /* Create new application stream */
#define OPK_STREAM_SEND             16  /* Push data into application send stream */
#define OPK_STREAM_FIN              17  /* Mark stream as finished */
#define OPK_STOP_SENDING            18  /* Mark stream for STOP_SENDING */
#define OPK_RESET_STREAM            19  /* Mark stream for RESET_STREAM */
#define OPK_CONN_TXFC_BUMP          20  /* Bump connection TXFC CWM */
#define OPK_STREAM_TXFC_BUMP        21  /* Bump stream TXFC CWM */
#define OPK_HANDSHAKE_COMPLETE      22  /* Mark handshake as complete */
#define OPK_NOP                     23  /* No-op */

struct script_op {
    uint32_t opcode;
    uint64_t arg0, arg1;
    const void *buf;
    size_t buf_len;
    int (*check_func)(struct helper *h);
};

#define OP_END      \
    { OPK_END }
#define OP_TXP_GENERATE() \
    { OPK_TXP_GENERATE },
#define OP_TXP_GENERATE_NONE() \
    { OPK_TXP_GENERATE_NONE },
#define OP_RX_PKT() \
    { OPK_RX_PKT },
#define OP_RX_PKT_NONE() \
    { OPK_RX_PKT_NONE },
#define OP_EXPECT_DGRAM_LEN(lo, hi) \
    { OPK_EXPECT_DGRAM_LEN, (lo), (hi) },
#define OP_EXPECT_FRAME(frame_type) \
    { OPK_EXPECT_FRAME, (frame_type) },
#define OP_EXPECT_INITIAL_TOKEN(buf) \
    { OPK_EXPECT_INITIAL_TOKEN, sizeof(buf), 0, buf },
#define OP_EXPECT_HDR(hdr) \
    { OPK_EXPECT_HDR, 0, 0, &(hdr) },
#define OP_CHECK(func) \
    { OPK_CHECK, 0, 0, NULL, 0, (func) },
#define OP_NEXT_FRAME() \
    { OPK_NEXT_FRAME },
#define OP_EXPECT_NO_FRAME() \
    { OPK_EXPECT_NO_FRAME },
#define OP_PROVIDE_SECRET(el, suite, secret) \
    { OPK_PROVIDE_SECRET, (el), (suite), (secret), sizeof(secret) },
#define OP_DISCARD_EL(el) \
    { OPK_DISCARD_EL, (el) },
#define OP_CRYPTO_SEND(pn_space, buf) \
    { OPK_CRYPTO_SEND, (pn_space), 0, (buf), sizeof(buf) },
#define OP_STREAM_NEW(id) \
    { OPK_STREAM_NEW, (id) },
#define OP_STREAM_SEND(id, buf) \
    { OPK_STREAM_SEND, (id), 0, (buf), sizeof(buf) },
#define OP_STREAM_FIN(id) \
    { OPK_STREAM_FIN, (id) },
#define OP_STOP_SENDING(id, aec) \
    { OPK_STOP_SENDING, (id), (aec) },
#define OP_RESET_STREAM(id, aec) \
    { OPK_RESET_STREAM, (id), (aec) },
#define OP_CONN_TXFC_BUMP(cwm) \
    { OPK_CONN_TXFC_BUMP, (cwm) },
#define OP_STREAM_TXFC_BUMP(id, cwm) \
    { OPK_STREAM_TXFC_BUMP, (cwm), (id) },
#define OP_HANDSHAKE_COMPLETE() \
    { OPK_HANDSHAKE_COMPLETE },
#define OP_NOP() \
    { OPK_NOP },

static int schedule_handshake_done(struct helper *h)
{
    ossl_quic_tx_packetiser_schedule_handshake_done(h->txp);
    return 1;
}

static int schedule_ack_eliciting_app(struct helper *h)
{
    ossl_quic_tx_packetiser_schedule_ack_eliciting(h->txp, QUIC_PN_SPACE_APP);
    return 1;
}

/* 1. 1-RTT, Single Handshake Done Frame */
static const struct script_op script_1[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_handshake_done)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    /* Should not be long */
    OP_EXPECT_DGRAM_LEN(21, 32)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 2. 1-RTT, Forced ACK-Eliciting Frame */
static const struct script_op script_2[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_ack_eliciting_app)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    /* Should not be long */
    OP_EXPECT_DGRAM_LEN(21, 32)
    /* A PING frame should have been added */
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_PING)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 3. 1-RTT, MAX_DATA */
static int schedule_max_data(struct helper *h)
{
    uint64_t cwm;

    cwm = ossl_quic_rxfc_get_cwm(&h->stream_rxfc);

    if (!TEST_true(ossl_quic_rxfc_on_rx_stream_frame(&h->stream_rxfc, cwm, 0))
        || !TEST_true(ossl_quic_rxfc_on_retire(&h->stream_rxfc, cwm,
                                               ossl_ticks2time(OSSL_TIME_MS))))
        return 0;

    return 1;
}

static const struct script_op script_3[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_max_data)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    /* Should not be long */
    OP_EXPECT_DGRAM_LEN(21, 40)
    /* A PING frame should have been added */
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_MAX_DATA)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 4. 1-RTT, CFQ (NEW_CONN_ID) */
static void free_buf_mem(unsigned char *buf, size_t buf_len, void *arg)
{
    BUF_MEM_free((BUF_MEM *)arg);
}

static int schedule_cfq_new_conn_id(struct helper *h)
{
    int rc = 0;
    QUIC_CFQ_ITEM *cfq_item;
    WPACKET wpkt;
    BUF_MEM *buf_mem = NULL;
    size_t l = 0;
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid = {0};

    ncid.seq_num         = 2345;
    ncid.retire_prior_to = 1234;
    ncid.conn_id         = cid_1;
    memcpy(ncid.stateless_reset.token, reset_token_1, sizeof(reset_token_1));

    if (!TEST_ptr(buf_mem = BUF_MEM_new()))
        goto err;

    if (!TEST_true(WPACKET_init(&wpkt, buf_mem)))
        goto err;

    if (!TEST_true(ossl_quic_wire_encode_frame_new_conn_id(&wpkt, &ncid))) {
        WPACKET_cleanup(&wpkt);
        goto err;
    }

    WPACKET_finish(&wpkt);

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &l)))
        goto err;

    if (!TEST_ptr(cfq_item = ossl_quic_cfq_add_frame(h->args.cfq, 1,
                                                     QUIC_PN_SPACE_APP,
                                                     OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID, 0,
                                                     (unsigned char *)buf_mem->data, l,
                                                     free_buf_mem,
                                                     buf_mem)))
        goto err;

    rc = 1;
err:
    if (!rc)
        BUF_MEM_free(buf_mem);
    return rc;
}

static int check_cfq_new_conn_id(struct helper *h)
{
    if (!TEST_uint64_t_eq(h->frame.new_conn_id.seq_num, 2345)
        || !TEST_uint64_t_eq(h->frame.new_conn_id.retire_prior_to, 1234)
        || !TEST_mem_eq(&h->frame.new_conn_id.conn_id, sizeof(cid_1),
                        &cid_1, sizeof(cid_1))
        || !TEST_mem_eq(&h->frame.new_conn_id.stateless_reset.token,
                        sizeof(reset_token_1),
                        reset_token_1,
                        sizeof(reset_token_1)))
        return 0;

    return 1;
}

static const struct script_op script_4[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_cfq_new_conn_id)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 128)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID)
    OP_CHECK(check_cfq_new_conn_id)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 5. 1-RTT, CFQ (NEW_TOKEN) */
static const unsigned char token_1[] = {
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15
};

static int schedule_cfq_new_token(struct helper *h)
{
    int rc = 0;
    QUIC_CFQ_ITEM *cfq_item;
    WPACKET wpkt;
    BUF_MEM *buf_mem = NULL;
    size_t l = 0;

    if (!TEST_ptr(buf_mem = BUF_MEM_new()))
        goto err;

    if (!TEST_true(WPACKET_init(&wpkt, buf_mem)))
        goto err;

    if (!TEST_true(ossl_quic_wire_encode_frame_new_token(&wpkt, token_1,
                                                         sizeof(token_1)))) {
        WPACKET_cleanup(&wpkt);
        goto err;
    }

    WPACKET_finish(&wpkt);

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &l)))
        goto err;

    if (!TEST_ptr(cfq_item = ossl_quic_cfq_add_frame(h->args.cfq, 1,
                                                     QUIC_PN_SPACE_APP,
                                                     OSSL_QUIC_FRAME_TYPE_NEW_TOKEN, 0,
                                                     (unsigned char *)buf_mem->data, l,
                                                     free_buf_mem,
                                                     buf_mem)))
        goto err;

    rc = 1;
err:
    if (!rc)
        BUF_MEM_free(buf_mem);
    return rc;
}

static int check_cfq_new_token(struct helper *h)
{
    if (!TEST_mem_eq(h->frame.new_token.token,
                     h->frame.new_token.token_len,
                     token_1,
                     sizeof(token_1)))
        return 0;

    return 1;
}

static const struct script_op script_5[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_cfq_new_token)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_NEW_TOKEN)
    OP_CHECK(check_cfq_new_token)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 6. 1-RTT, ACK */
static int schedule_ack(struct helper *h)
{
    size_t i;
    OSSL_ACKM_RX_PKT rx_pkt = {0};

    /* Stimulate ACK emission by simulating a few received packets. */
    for (i = 0; i < 5; ++i) {
        rx_pkt.pkt_num          = i;
        rx_pkt.time             = fake_now(NULL);
        rx_pkt.pkt_space        = QUIC_PN_SPACE_APP;
        rx_pkt.is_ack_eliciting = 1;

        if (!TEST_true(ossl_ackm_on_rx_packet(h->args.ackm, &rx_pkt)))
            return 0;
    }

    return 1;
}

static const struct script_op script_6[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_ack)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 7. 1-RTT, ACK, NEW_TOKEN */
static const struct script_op script_7[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(schedule_cfq_new_token)
    OP_CHECK(schedule_ack)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    /* ACK must come before NEW_TOKEN */
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_NEW_TOKEN)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 8. 1-RTT, CRYPTO */
static const unsigned char crypto_1[] = {
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09
};

static const struct script_op script_8[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CRYPTO_SEND(QUIC_PN_SPACE_APP, crypto_1)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_CRYPTO)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 9. 1-RTT, STREAM */
static const unsigned char stream_9[] = {
    0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x7a, 0x7b
};

static int check_stream_9(struct helper *h)
{
    if (!TEST_mem_eq(h->frame.stream.data, (size_t)h->frame.stream.len,
                     stream_9, sizeof(stream_9)))
        return 0;

    return 1;
}

static const struct script_op script_9[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_HANDSHAKE_COMPLETE()
    OP_TXP_GENERATE_NONE()
    OP_STREAM_NEW(42)
    OP_STREAM_SEND(42, stream_9)
    /* Still no output because of TXFC */
    OP_TXP_GENERATE_NONE()
    /* Now grant a TXFC budget */
    OP_CONN_TXFC_BUMP(1000)
    OP_STREAM_TXFC_BUMP(42, 1000)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STREAM)
    OP_CHECK(check_stream_9)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 10. 1-RTT, STREAM, round robin */
/* The data below is randomly generated data. */
static const unsigned char stream_10a[1300] = {
    0x40, 0x0d, 0xb6, 0x0d, 0x25, 0x5f, 0xdd, 0xb9, 0x05, 0x79, 0xa8, 0xe3,
    0x79, 0x32, 0xb2, 0xa7, 0x30, 0x6d, 0x29, 0xf6, 0xba, 0x50, 0xbe, 0x83,
    0xcb, 0x56, 0xec, 0xd6, 0xc7, 0x80, 0x84, 0xa2, 0x2f, 0xeb, 0xc4, 0x37,
    0x40, 0x44, 0xef, 0xd8, 0x78, 0xbb, 0x92, 0x80, 0x22, 0x33, 0xc0, 0xce,
    0x33, 0x5b, 0x75, 0x8c, 0xa5, 0x1a, 0x7a, 0x2a, 0xa9, 0x88, 0xaf, 0xf6,
    0x3a, 0xe2, 0x5e, 0x60, 0x52, 0x6d, 0xef, 0x7f, 0x2a, 0x9a, 0xaa, 0x17,
    0x0e, 0x12, 0x51, 0x82, 0x08, 0x2f, 0x0f, 0x5b, 0xff, 0xf5, 0x7c, 0x7c,
    0x89, 0x04, 0xfb, 0xa7, 0x80, 0x4e, 0xda, 0x12, 0x89, 0x01, 0x4a, 0x81,
    0x84, 0x78, 0x15, 0xa9, 0x12, 0x28, 0x69, 0x4a, 0x25, 0xe5, 0x8b, 0x69,
    0xc2, 0x9f, 0xb6, 0x59, 0x49, 0xe3, 0x53, 0x90, 0xef, 0xc9, 0xb8, 0x40,
    0xdd, 0x62, 0x5f, 0x99, 0x68, 0xd2, 0x0a, 0x77, 0xde, 0xf3, 0x11, 0x39,
    0x7f, 0x93, 0x8b, 0x81, 0x69, 0x36, 0xa7, 0x76, 0xa4, 0x10, 0x56, 0x51,
    0xe5, 0x45, 0x3a, 0x42, 0x49, 0x6c, 0xc6, 0xa0, 0xb4, 0x13, 0x46, 0x59,
    0x0e, 0x48, 0x60, 0xc9, 0xff, 0x70, 0x10, 0x8d, 0x6a, 0xf9, 0x5b, 0x94,
    0xc2, 0x9e, 0x49, 0x19, 0x56, 0xf2, 0xc1, 0xff, 0x08, 0x3f, 0x9e, 0x26,
    0x8e, 0x99, 0x71, 0xc4, 0x25, 0xb1, 0x4e, 0xcc, 0x7e, 0x5f, 0xf0, 0x4e,
    0x25, 0xa2, 0x2f, 0x3f, 0x68, 0xaa, 0xcf, 0xbd, 0x19, 0x19, 0x1c, 0x92,
    0xa0, 0xb6, 0xb8, 0x32, 0xb1, 0x0b, 0x91, 0x05, 0xa9, 0xf8, 0x1a, 0x4b,
    0x74, 0x09, 0xf9, 0x57, 0xd0, 0x1c, 0x38, 0x10, 0x05, 0x54, 0xd8, 0x4e,
    0x12, 0x67, 0xcc, 0x43, 0xa3, 0x81, 0xa9, 0x3a, 0x12, 0x57, 0xe7, 0x4b,
    0x0e, 0xe5, 0x51, 0xf9, 0x5f, 0xd4, 0x46, 0x73, 0xa2, 0x78, 0xb7, 0x00,
    0x24, 0x69, 0x35, 0x10, 0x1e, 0xb8, 0xa7, 0x4a, 0x9b, 0xbc, 0xfc, 0x04,
    0x6f, 0x1a, 0xb0, 0x4f, 0x12, 0xc9, 0x2b, 0x3b, 0x94, 0x85, 0x1b, 0x8e,
    0xba, 0xac, 0xfd, 0x10, 0x22, 0x68, 0x90, 0x17, 0x13, 0x44, 0x18, 0x2f,
    0x33, 0x37, 0x1a, 0x89, 0xc0, 0x2c, 0x14, 0x59, 0xb2, 0xaf, 0xc0, 0x6b,
    0xdc, 0x28, 0xe1, 0xe9, 0xc1, 0x0c, 0xb4, 0x80, 0x90, 0xb9, 0x1f, 0x45,
    0xb4, 0x63, 0x9a, 0x0e, 0xfa, 0x33, 0xf5, 0x75, 0x3a, 0x4f, 0xc3, 0x8c,
    0x70, 0xdb, 0xd7, 0xbf, 0xf6, 0xb8, 0x7f, 0xcc, 0xe5, 0x85, 0xb6, 0xae,
    0x25, 0x60, 0x18, 0x5b, 0xf1, 0x51, 0x1a, 0x85, 0xc1, 0x7f, 0xf3, 0xbe,
    0xb6, 0x82, 0x38, 0xe3, 0xd2, 0xff, 0x8a, 0xc4, 0xdb, 0x08, 0xe6, 0x96,
    0xd5, 0x3d, 0x1f, 0xc5, 0x12, 0x35, 0x45, 0x75, 0x5d, 0x17, 0x4e, 0xe1,
    0xb8, 0xc9, 0xf0, 0x45, 0x95, 0x0b, 0x03, 0xcb, 0x85, 0x47, 0xaf, 0xc7,
    0x88, 0xb6, 0xc1, 0x2c, 0xb8, 0x9b, 0xe6, 0x8b, 0x51, 0xd5, 0x2e, 0x71,
    0xba, 0xc9, 0xa9, 0x37, 0x5e, 0x1c, 0x2c, 0x03, 0xf0, 0xc7, 0xc1, 0xd3,
    0x72, 0xaa, 0x4d, 0x19, 0xd6, 0x51, 0x64, 0x12, 0xeb, 0x39, 0xeb, 0x45,
    0xe9, 0xb4, 0x84, 0x08, 0xb6, 0x6c, 0xc7, 0x3e, 0xf0, 0x88, 0x64, 0xc2,
    0x91, 0xb7, 0xa5, 0x86, 0x66, 0x83, 0xd5, 0xd3, 0x41, 0x24, 0xb2, 0x1c,
    0x9a, 0x18, 0x10, 0x0e, 0xa5, 0xc9, 0xef, 0xcd, 0x06, 0xce, 0xa8, 0xaf,
    0x22, 0x52, 0x25, 0x0b, 0x99, 0x3d, 0xe9, 0x26, 0xda, 0xa9, 0x47, 0xd1,
    0x4b, 0xa6, 0x4c, 0xfc, 0x80, 0xaf, 0x6a, 0x59, 0x4b, 0x35, 0xa4, 0x93,
    0x39, 0x5b, 0xfa, 0x91, 0x9d, 0xdf, 0x9d, 0x3c, 0xfb, 0x53, 0xca, 0x18,
    0x19, 0xe4, 0xda, 0x95, 0x47, 0x5a, 0x37, 0x59, 0xd7, 0xd2, 0xe4, 0x75,
    0x45, 0x0d, 0x03, 0x7f, 0xa0, 0xa9, 0xa0, 0x71, 0x06, 0xb1, 0x9d, 0x46,
    0xbd, 0xcf, 0x4a, 0x8b, 0x73, 0xc1, 0x45, 0x5c, 0x00, 0x61, 0xfd, 0xd1,
    0xa4, 0xa2, 0x3e, 0xaa, 0xbe, 0x72, 0xf1, 0x7a, 0x1a, 0x76, 0x88, 0x5c,
    0x9e, 0x74, 0x6d, 0x2a, 0x34, 0xfc, 0xf7, 0x41, 0x28, 0xe8, 0xa3, 0x43,
    0x4d, 0x43, 0x1d, 0x6c, 0x36, 0xb1, 0x45, 0x71, 0x5a, 0x3c, 0xd3, 0x28,
    0x44, 0xe4, 0x9b, 0xbf, 0x54, 0x16, 0xc3, 0x99, 0x6c, 0x42, 0xd8, 0x20,
    0xb6, 0x20, 0x5f, 0x6e, 0xbc, 0xba, 0x88, 0x5e, 0x2f, 0xa5, 0xd1, 0x82,
    0x5c, 0x92, 0xd0, 0x79, 0xfd, 0xcc, 0x61, 0x49, 0xd0, 0x73, 0x92, 0xe6,
    0x98, 0xe3, 0x80, 0x7a, 0xf9, 0x56, 0x63, 0x33, 0x19, 0xda, 0x54, 0x13,
    0xf0, 0x21, 0xa8, 0x15, 0xf6, 0xb7, 0x43, 0x7c, 0x1c, 0x1e, 0xb1, 0x89,
    0x8d, 0xce, 0x20, 0x54, 0x81, 0x80, 0xb5, 0x8f, 0x9b, 0xb1, 0x09, 0x92,
    0xdb, 0x25, 0x6f, 0x30, 0x29, 0x08, 0x1a, 0x05, 0x08, 0xf4, 0x83, 0x8b,
    0x1e, 0x2d, 0xfd, 0xe4, 0xb2, 0x76, 0xc8, 0x4d, 0xf3, 0xa6, 0x49, 0x5f,
    0x2c, 0x99, 0x78, 0xbd, 0x07, 0xef, 0xc8, 0xd9, 0xb5, 0x70, 0x3b, 0x0a,
    0xcb, 0xbd, 0xa0, 0xea, 0x15, 0xfb, 0xd1, 0x6e, 0x61, 0x83, 0xcb, 0x90,
    0xd0, 0xa3, 0x81, 0x28, 0xdc, 0xd5, 0x84, 0xae, 0x55, 0x28, 0x13, 0x9e,
    0xc6, 0xd8, 0xf4, 0x67, 0xd6, 0x0d, 0xd4, 0x69, 0xac, 0xf6, 0x35, 0x95,
    0x99, 0x44, 0x26, 0x72, 0x36, 0x55, 0xf9, 0x42, 0xa6, 0x1b, 0x00, 0x93,
    0x00, 0x19, 0x2f, 0x70, 0xd3, 0x16, 0x66, 0x4e, 0x80, 0xbb, 0xb6, 0x84,
    0xa1, 0x2c, 0x09, 0xfb, 0x41, 0xdf, 0x63, 0xde, 0x62, 0x3e, 0xd0, 0xa8,
    0xd8, 0x0c, 0x03, 0x06, 0xa9, 0x82, 0x17, 0x9c, 0xd2, 0xa9, 0xd5, 0x6f,
    0xcc, 0xc0, 0xf2, 0x5d, 0xb1, 0xba, 0xf8, 0x2e, 0x37, 0x8b, 0xe6, 0x5d,
    0x9f, 0x1b, 0xfb, 0x53, 0x0a, 0x96, 0xbe, 0x69, 0x31, 0x19, 0x8f, 0x44,
    0x1b, 0xc2, 0x42, 0x7e, 0x65, 0x12, 0x1d, 0x52, 0x1e, 0xe2, 0xc0, 0x86,
    0x70, 0x88, 0xe5, 0xf6, 0x87, 0x5d, 0x03, 0x4b, 0x12, 0x3c, 0x2d, 0xaf,
    0x09, 0xf5, 0x4f, 0x82, 0x2e, 0x2e, 0xbe, 0x07, 0xe8, 0x8d, 0x57, 0x6e,
    0xc0, 0xeb, 0xf9, 0x37, 0xac, 0x89, 0x01, 0xb7, 0xc6, 0x52, 0x1c, 0x86,
    0xe5, 0xbc, 0x1f, 0xbd, 0xde, 0xa2, 0x42, 0xb6, 0x73, 0x85, 0x6f, 0x06,
    0x36, 0x56, 0x40, 0x2b, 0xea, 0x16, 0x8c, 0xf4, 0x7b, 0x65, 0x6a, 0xca,
    0x3c, 0x56, 0x68, 0x01, 0xe3, 0x9c, 0xbb, 0xb9, 0x45, 0x54, 0xcd, 0x13,
    0x74, 0xad, 0x80, 0x40, 0xbc, 0xd0, 0x74, 0xb4, 0x31, 0xe4, 0xca, 0xd5,
    0xf8, 0x4f, 0x08, 0x5b, 0xc4, 0x15, 0x1a, 0x51, 0x3b, 0xc6, 0x40, 0xc8,
    0xea, 0x76, 0x30, 0x95, 0xb7, 0x76, 0xa4, 0xda, 0x20, 0xdb, 0x75, 0x1c,
    0xf4, 0x87, 0x24, 0x29, 0x54, 0xc6, 0x59, 0x0c, 0xf0, 0xed, 0xf5, 0x3d,
    0xce, 0x95, 0x23, 0x30, 0x49, 0x91, 0xa7, 0x7b, 0x22, 0xb5, 0xd7, 0x71,
    0xb0, 0x60, 0xe1, 0xf0, 0x84, 0x74, 0x0e, 0x2f, 0xa8, 0x79, 0x35, 0xb9,
    0x03, 0xb5, 0x2c, 0xdc, 0x60, 0x48, 0x12, 0xd9, 0x14, 0x5a, 0x58, 0x5d,
    0x95, 0xc6, 0x47, 0xfd, 0xaf, 0x09, 0xc2, 0x67, 0xa5, 0x09, 0xae, 0xff,
    0x4b, 0xd5, 0x6c, 0x2f, 0x1d, 0x33, 0x31, 0xcb, 0xdb, 0xcf, 0xf5, 0xf6,
    0xbc, 0x90, 0xb2, 0x15, 0xd4, 0x34, 0xeb, 0xde, 0x0e, 0x8f, 0x3d, 0xea,
    0xa4, 0x9b, 0x29, 0x8a, 0xf9, 0x4a, 0xac, 0x38, 0x1e, 0x46, 0xb2, 0x2d,
    0xa2, 0x61, 0xc5, 0x99, 0x5e, 0x85, 0x36, 0x85, 0xb0, 0xb1, 0x6b, 0xc4,
    0x06, 0x68, 0xc7, 0x9b, 0x54, 0xb9, 0xc8, 0x9d, 0xf3, 0x1a, 0xe0, 0x67,
    0x0e, 0x4d, 0x5c, 0x13, 0x54, 0xa4, 0x62, 0x62, 0x6f, 0xae, 0x0e, 0x86,
    0xa2, 0xe0, 0x31, 0xc7, 0x72, 0xa1, 0xbb, 0x87, 0x3e, 0x61, 0x96, 0xb7,
    0x53, 0xf9, 0x34, 0xcb, 0xfd, 0x6c, 0x67, 0x25, 0x73, 0x61, 0x75, 0x4f,
    0xab, 0x37, 0x08, 0xef, 0x35, 0x5a, 0x03, 0xe5, 0x08, 0x43, 0xec, 0xdc,
    0xb5, 0x2c, 0x1f, 0xe6, 0xeb, 0xc6, 0x06, 0x0b, 0xed, 0xad, 0x74, 0xf4,
    0x55, 0xef, 0xe0, 0x2e, 0x83, 0x00, 0xdb, 0x32, 0xde, 0xe9, 0xe4, 0x2f,
    0xf5, 0x20, 0x6d, 0x72, 0x47, 0xf4, 0x68, 0xa6, 0x7f, 0x3e, 0x6a, 0x5a,
    0x21, 0x76, 0x31, 0x97, 0xa0, 0xc6, 0x7d, 0x03, 0xf7, 0x27, 0x45, 0x5a,
    0x75, 0x03, 0xc1, 0x5c, 0x94, 0x2b, 0x37, 0x9f, 0x46, 0x8f, 0xc3, 0xa7,
    0x50, 0xe4, 0xe7, 0x23, 0xf7, 0x20, 0xa2, 0x8e, 0x4b, 0xfd, 0x7a, 0xa7,
    0x8a, 0x54, 0x7b, 0x32, 0xef, 0x0e, 0x82, 0xb9, 0xf9, 0x14, 0x62, 0x68,
    0x32, 0x9e, 0x55, 0xc0, 0xd8, 0xc7, 0x41, 0x9c, 0x67, 0x95, 0xbf, 0xc3,
    0x86, 0x74, 0x70, 0x64, 0x44, 0x23, 0x77, 0x79, 0x82, 0x23, 0x1c, 0xf4,
    0xa1, 0x05, 0xd3, 0x98, 0x89, 0xde, 0x7d, 0xb3, 0x5b, 0xef, 0x38, 0xd2,
    0x07, 0xbc, 0x5a, 0x69, 0xa3, 0xe4, 0x37, 0x9b, 0x53, 0xff, 0x04, 0x6b,
    0xd9, 0xd8, 0x32, 0x89, 0xf7, 0x82, 0x77, 0xcf, 0xe6, 0xff, 0xf4, 0x15,
    0x54, 0x91, 0x65, 0x96, 0x49, 0xd7, 0x0a, 0xa4, 0xf3, 0x55, 0x2b, 0xc1,
    0x48, 0xc1, 0x7e, 0x56, 0x69, 0x27, 0xf4, 0xd1, 0x47, 0x1f, 0xde, 0x86,
    0x15, 0x67, 0x04, 0x9d, 0x41, 0x1f, 0xe8, 0xe1, 0x23, 0xe4, 0x56, 0xb9,
    0xdb, 0x4e, 0xe4, 0x84, 0x6c, 0x63, 0x39, 0xad, 0x44, 0x6d, 0x4e, 0x28,
    0xcd, 0xf6, 0xac, 0xec, 0xc2, 0xad, 0xcd, 0xc3, 0xed, 0x03, 0x63, 0x5d,
    0xef, 0x1d, 0x40, 0x8d, 0x9a, 0x02, 0x67, 0x4b, 0x55, 0xb5, 0xfe, 0x75,
    0xb6, 0x53, 0x34, 0x1d, 0x7b, 0x26, 0x23, 0xfe, 0xb9, 0x21, 0xd3, 0xe0,
    0xa0, 0x1a, 0x85, 0xe5
};

static const unsigned char stream_10b[1300] = {
    0x18, 0x00, 0xd7, 0xfb, 0x12, 0xda, 0xdb, 0x68, 0xeb, 0x38, 0x4d, 0xf6,
    0xb2, 0x45, 0x74, 0x4c, 0xcc, 0xe7, 0xa7, 0xc1, 0x26, 0x84, 0x3d, 0xdf,
    0x7d, 0xc5, 0xe9, 0xd4, 0x31, 0xa2, 0x51, 0x38, 0x95, 0xe2, 0x68, 0x11,
    0x9d, 0xd1, 0x52, 0xb5, 0xef, 0x76, 0xe0, 0x3d, 0x11, 0x50, 0xd7, 0xb2,
    0xc1, 0x7d, 0x12, 0xaf, 0x02, 0x52, 0x97, 0x03, 0xf3, 0x2e, 0x54, 0xdf,
    0xa0, 0x40, 0x76, 0x52, 0x82, 0x23, 0x3c, 0xbd, 0x20, 0x6d, 0x0a, 0x6f,
    0x81, 0xfc, 0x41, 0x9d, 0x2e, 0xa7, 0x2c, 0x78, 0x9c, 0xd8, 0x56, 0xb0,
    0x31, 0x35, 0xc8, 0x53, 0xef, 0xf9, 0x43, 0x17, 0xc0, 0x8c, 0x2c, 0x8f,
    0x4a, 0x68, 0xe8, 0x9f, 0xbd, 0x3f, 0xf2, 0x18, 0xb8, 0xe6, 0x55, 0xea,
    0x2a, 0x37, 0x3e, 0xac, 0xb0, 0x75, 0xd4, 0x75, 0x12, 0x82, 0xec, 0x21,
    0xb9, 0xce, 0xe5, 0xc1, 0x62, 0x49, 0xd5, 0xf1, 0xca, 0xd4, 0x32, 0x76,
    0x34, 0x5f, 0x3e, 0xc9, 0xb3, 0x54, 0xe4, 0xd0, 0xa9, 0x7d, 0x0c, 0x64,
    0x48, 0x0a, 0x74, 0x38, 0x03, 0xd0, 0x20, 0xac, 0xe3, 0x58, 0x3d, 0x4b,
    0xa7, 0x46, 0xac, 0x57, 0x63, 0x12, 0x17, 0xcb, 0x96, 0xed, 0xc9, 0x39,
    0x64, 0xde, 0xff, 0xc6, 0xb2, 0x40, 0x2c, 0xf9, 0x1d, 0xa6, 0x94, 0x2a,
    0x16, 0x4d, 0x7f, 0x22, 0x91, 0x8b, 0xfe, 0x83, 0x77, 0x02, 0x68, 0x62,
    0x27, 0x77, 0x2e, 0xe9, 0xce, 0xbc, 0x20, 0xe8, 0xfb, 0xf8, 0x4e, 0x17,
    0x07, 0xe1, 0xaa, 0x29, 0xb7, 0x50, 0xcf, 0xb0, 0x6a, 0xcf, 0x01, 0xec,
    0xbf, 0xff, 0xb5, 0x9f, 0x00, 0x64, 0x80, 0xbb, 0xa6, 0xe4, 0xa2, 0x1e,
    0xe4, 0xf8, 0xa3, 0x0d, 0xc7, 0x65, 0x45, 0xb7, 0x01, 0x33, 0x80, 0x37,
    0x11, 0x16, 0x34, 0xc1, 0x06, 0xc5, 0xd3, 0xc4, 0x70, 0x62, 0x75, 0xd8,
    0xa3, 0xba, 0x84, 0x9f, 0x81, 0x9f, 0xda, 0x01, 0x83, 0x42, 0x84, 0x05,
    0x69, 0x68, 0xb0, 0x74, 0x73, 0x0f, 0x68, 0x39, 0xd3, 0x11, 0xc5, 0x55,
    0x3e, 0xf2, 0xb7, 0xf4, 0xa6, 0xed, 0x0b, 0x50, 0xbe, 0x44, 0xf8, 0x67,
    0x48, 0x46, 0x5e, 0x71, 0x07, 0xcf, 0xca, 0x8a, 0xbc, 0xa4, 0x3c, 0xd2,
    0x4a, 0x80, 0x2e, 0x4f, 0xc5, 0x3b, 0x61, 0xc1, 0x7e, 0x93, 0x9e, 0xe0,
    0x05, 0xfb, 0x10, 0xe8, 0x53, 0xff, 0x16, 0x5e, 0x18, 0xe0, 0x9f, 0x39,
    0xbf, 0xaa, 0x80, 0x6d, 0xb7, 0x9f, 0x51, 0x91, 0xa0, 0xf6, 0xce, 0xad,
    0xed, 0x56, 0x15, 0xb9, 0x12, 0x57, 0x60, 0xa6, 0xae, 0x54, 0x6e, 0x36,
    0xf3, 0xe0, 0x05, 0xd8, 0x3e, 0x6d, 0x08, 0x36, 0xc9, 0x79, 0x64, 0x51,
    0x63, 0x92, 0xa8, 0xa1, 0xbf, 0x55, 0x26, 0x80, 0x75, 0x44, 0x33, 0x33,
    0xfb, 0xb7, 0xec, 0xf9, 0xc6, 0x01, 0xf9, 0xd5, 0x93, 0xfc, 0xb7, 0x43,
    0xa2, 0x38, 0x0d, 0x17, 0x75, 0x67, 0xec, 0xc9, 0x98, 0xd6, 0x25, 0xe6,
    0xb9, 0xed, 0x61, 0xa4, 0xee, 0x2c, 0xda, 0x27, 0xbd, 0xff, 0x86, 0x1e,
    0x45, 0x64, 0xfe, 0xcf, 0x0c, 0x9b, 0x7b, 0x75, 0x5f, 0xf1, 0xe0, 0xba,
    0x77, 0x8c, 0x03, 0x8f, 0xb4, 0x3a, 0xb6, 0x9c, 0xda, 0x9a, 0x83, 0xcb,
    0xe9, 0xcb, 0x3f, 0xf4, 0x10, 0x99, 0x5b, 0xe1, 0x19, 0x8f, 0x6b, 0x95,
    0x50, 0xe6, 0x78, 0xc9, 0x35, 0xb6, 0x87, 0xd8, 0x9e, 0x17, 0x30, 0x96,
    0x70, 0xa3, 0x04, 0x69, 0x1c, 0xa2, 0x6c, 0xd4, 0x88, 0x48, 0x44, 0x14,
    0x94, 0xd4, 0xc9, 0x4d, 0xe3, 0x82, 0x7e, 0x62, 0xf0, 0x0a, 0x18, 0x4d,
    0xd0, 0xd6, 0x63, 0xa3, 0xdf, 0xea, 0x28, 0xf4, 0x00, 0x75, 0x70, 0x78,
    0x08, 0x70, 0x3f, 0xff, 0x84, 0x86, 0x72, 0xea, 0x4f, 0x15, 0x8c, 0x17,
    0x60, 0x5f, 0xa1, 0x50, 0xa0, 0xfc, 0x6f, 0x8a, 0x46, 0xfc, 0x01, 0x8d,
    0x7c, 0xdc, 0x69, 0x6a, 0xd3, 0x74, 0x69, 0x76, 0x77, 0xdd, 0xe4, 0x9c,
    0x49, 0x1e, 0x6f, 0x7d, 0x31, 0x14, 0xd9, 0xe9, 0xe7, 0x17, 0x66, 0x82,
    0x1b, 0xf1, 0x0f, 0xe2, 0xba, 0xd2, 0x28, 0xd1, 0x6f, 0x48, 0xc7, 0xac,
    0x08, 0x4e, 0xee, 0x94, 0x66, 0x99, 0x34, 0x16, 0x5d, 0x95, 0xae, 0xe3,
    0x59, 0x79, 0x7f, 0x8e, 0x9f, 0xe3, 0xdb, 0xff, 0xdc, 0x4d, 0xb0, 0xbf,
    0xf9, 0xf3, 0x3e, 0xec, 0xcf, 0x50, 0x3d, 0x2d, 0xba, 0x94, 0x1f, 0x1a,
    0xab, 0xa4, 0xf4, 0x67, 0x43, 0x7e, 0xb9, 0x65, 0x20, 0x13, 0xb1, 0xd9,
    0x88, 0x4a, 0x24, 0x13, 0x84, 0x86, 0xae, 0x2b, 0x0c, 0x6c, 0x7e, 0xd4,
    0x25, 0x6e, 0xaa, 0x8d, 0x0c, 0x54, 0x99, 0xde, 0x1d, 0xac, 0x8c, 0x5c,
    0x73, 0x94, 0xd9, 0x75, 0xcb, 0x5a, 0x54, 0x3d, 0xeb, 0xff, 0xc1, 0x95,
    0x53, 0xb5, 0x39, 0xf7, 0xe5, 0xf1, 0x77, 0xd1, 0x42, 0x82, 0x4b, 0xb0,
    0xab, 0x19, 0x28, 0xff, 0x53, 0x28, 0x87, 0x46, 0xc6, 0x6f, 0x05, 0x06,
    0xa6, 0x0c, 0x97, 0x93, 0x68, 0x38, 0xe1, 0x61, 0xed, 0xf8, 0x90, 0x13,
    0xa3, 0x6f, 0xf2, 0x08, 0x37, 0xd7, 0x05, 0x25, 0x34, 0x43, 0x57, 0x72,
    0xfd, 0x6c, 0xc2, 0x19, 0x26, 0xe7, 0x50, 0x30, 0xb8, 0x6d, 0x09, 0x71,
    0x83, 0x75, 0xd4, 0x11, 0x25, 0x29, 0xc6, 0xee, 0xb2, 0x51, 0x1c, 0x1c,
    0x9e, 0x2d, 0x09, 0xb9, 0x73, 0x2b, 0xbf, 0xda, 0xc8, 0x1e, 0x2b, 0xe5,
    0x3f, 0x1e, 0x63, 0xe9, 0xc0, 0x6d, 0x04, 0x3a, 0x48, 0x61, 0xa8, 0xc6,
    0x16, 0x8d, 0x69, 0xc0, 0x67, 0x0c, 0x3b, 0xc4, 0x05, 0x36, 0xa1, 0x30,
    0x62, 0x92, 0x4d, 0x44, 0x31, 0x66, 0x46, 0xda, 0xef, 0x0f, 0x4e, 0xfb,
    0x78, 0x6a, 0xa9, 0x5b, 0xf8, 0x56, 0x26, 0x74, 0x16, 0xab, 0x17, 0x93,
    0x3c, 0x36, 0xbb, 0xa2, 0xbf, 0xad, 0xba, 0xb1, 0xfe, 0xc4, 0x9f, 0x75,
    0x47, 0x1e, 0x99, 0x7e, 0x32, 0xe8, 0xd4, 0x6c, 0xa4, 0xf8, 0xd2, 0xe4,
    0xb2, 0x51, 0xbb, 0xb2, 0xd7, 0xce, 0x94, 0xaf, 0x7f, 0xe6, 0x2c, 0x13,
    0xae, 0xd2, 0x29, 0x30, 0x7b, 0xfd, 0x25, 0x61, 0xf9, 0xe8, 0x35, 0x2d,
    0x1a, 0xc9, 0x81, 0xa5, 0xfe, 0xce, 0xf6, 0x17, 0xc5, 0xfb, 0x8c, 0x79,
    0x67, 0xa8, 0x5f, 0x5c, 0x31, 0xbc, 0xfc, 0xf3, 0x6b, 0xd3, 0x0d, 0xe0,
    0x62, 0xab, 0x86, 0xc3, 0x17, 0x5a, 0xba, 0x97, 0x86, 0x8f, 0x65, 0xd6,
    0xbd, 0x0c, 0xa1, 0xfb, 0x7f, 0x7c, 0xdc, 0xcb, 0x94, 0x30, 0x0b, 0x04,
    0x54, 0xc4, 0x31, 0xa1, 0xca, 0x1e, 0xc5, 0xf0, 0xb6, 0x08, 0xd7, 0x2e,
    0xa1, 0x90, 0x41, 0xce, 0xd9, 0xef, 0x3a, 0x58, 0x01, 0x1a, 0x73, 0x18,
    0xad, 0xdc, 0x20, 0x25, 0x95, 0x1a, 0xfe, 0x61, 0xf1, 0x58, 0x32, 0x8b,
    0x43, 0x59, 0xd6, 0x21, 0xdb, 0xa9, 0x8e, 0x54, 0xe6, 0x21, 0xcf, 0xd3,
    0x6b, 0x59, 0x29, 0x9b, 0x3e, 0x6c, 0x7f, 0xe2, 0x29, 0x72, 0x8c, 0xd1,
    0x3e, 0x9a, 0x84, 0x98, 0xb0, 0xf3, 0x20, 0x30, 0x34, 0x71, 0xa7, 0x5b,
    0xf0, 0x26, 0xe1, 0xf4, 0x76, 0x65, 0xc9, 0xd7, 0xe4, 0xb9, 0x25, 0x48,
    0xc2, 0x7e, 0xa6, 0x0b, 0x0d, 0x05, 0x68, 0xa1, 0x96, 0x61, 0x0b, 0x4c,
    0x2f, 0x1a, 0xe3, 0x56, 0x71, 0x89, 0x48, 0x66, 0xd8, 0xd0, 0x69, 0x37,
    0x7a, 0xdf, 0xdb, 0xed, 0xad, 0x82, 0xaa, 0x40, 0x25, 0x47, 0x3e, 0x75,
    0xa6, 0x0e, 0xf5, 0x2f, 0xa7, 0x4e, 0x97, 0xa2, 0x5f, 0x01, 0x99, 0x48,
    0x3a, 0x63, 0x18, 0x20, 0x61, 0x72, 0xe4, 0xcf, 0x4b, 0x3b, 0x99, 0x36,
    0xe1, 0xf3, 0xbf, 0xae, 0x2b, 0x6b, 0xa1, 0x94, 0xa0, 0x15, 0x94, 0xd6,
    0xe0, 0xba, 0x71, 0xa2, 0x85, 0xa0, 0x8c, 0x5e, 0x58, 0xe2, 0xde, 0x6b,
    0x08, 0x68, 0x90, 0x82, 0x71, 0x8d, 0xfd, 0x12, 0xa2, 0x49, 0x87, 0x70,
    0xee, 0x2a, 0x08, 0xe2, 0x26, 0xaf, 0xeb, 0x85, 0x35, 0xd2, 0x0e, 0xfd,
    0x2b, 0x6f, 0xc0, 0xfe, 0x41, 0xbb, 0xd7, 0x0a, 0xa3, 0x8d, 0x8b, 0xec,
    0x44, 0x9f, 0x46, 0x59, 0x4d, 0xac, 0x04, 0x1e, 0xde, 0x10, 0x7b, 0x17,
    0x0a, 0xb0, 0xcc, 0x26, 0x0c, 0xa9, 0x3c, 0x5f, 0xd8, 0xe6, 0x52, 0xd3,
    0xfd, 0x0b, 0x66, 0x75, 0x06, 0x84, 0x23, 0x64, 0x2b, 0x80, 0x68, 0xf9,
    0xcb, 0xcd, 0x04, 0x07, 0xf7, 0xe0, 0x07, 0xb4, 0xc6, 0xa0, 0x08, 0xd0,
    0x76, 0x16, 0x77, 0xd8, 0x48, 0xf0, 0x45, 0x4e, 0xe2, 0xf2, 0x88, 0xcd,
    0x0f, 0xbd, 0x7d, 0xb6, 0xbe, 0x4e, 0x9e, 0x5d, 0x6c, 0x47, 0x26, 0x34,
    0x94, 0xfb, 0xc5, 0x4f, 0x5c, 0xb5, 0xb5, 0xfc, 0x99, 0x34, 0x71, 0xe5,
    0xe1, 0x36, 0x0c, 0xd2, 0x95, 0xb8, 0x93, 0x3c, 0x5d, 0x2d, 0x71, 0x55,
    0x0b, 0x96, 0x4e, 0x9f, 0x07, 0x9a, 0x38, 0x9a, 0xcc, 0x24, 0xb5, 0xac,
    0x05, 0x8b, 0x1c, 0x61, 0xd4, 0xf2, 0xdf, 0x9e, 0x11, 0xe3, 0x7d, 0x64,
    0x2f, 0xe5, 0x13, 0xd4, 0x0a, 0xe9, 0x32, 0x26, 0xa8, 0x93, 0x21, 0x59,
    0xf3, 0x41, 0x48, 0x0a, 0xbd, 0x59, 0x8f, 0xf8, 0x72, 0xab, 0xd3, 0x65,
    0x8e, 0xdc, 0xaa, 0x0c, 0xc0, 0x01, 0x36, 0xb7, 0xf5, 0x84, 0x27, 0x9a,
    0x98, 0x89, 0x73, 0x3a, 0xeb, 0x55, 0x15, 0xc9, 0x3d, 0xe1, 0xf8, 0xea,
    0xf6, 0x11, 0x28, 0xe0, 0x80, 0x93, 0xcc, 0xba, 0xe1, 0xf1, 0x81, 0xbc,
    0xa4, 0x30, 0xbc, 0x98, 0xe8, 0x9e, 0x8d, 0x17, 0x7e, 0xb7, 0xb1, 0x27,
    0x6f, 0xcf, 0x9c, 0x0d, 0x1d, 0x01, 0xea, 0x45, 0xc0, 0x90, 0xda, 0x53,
    0xf6, 0xde, 0xdf, 0x12, 0xa1, 0x23, 0x3d, 0x92, 0x89, 0x77, 0xa7, 0x2a,
    0xe7, 0x45, 0x24, 0xdd, 0xf2, 0x17, 0x10, 0xca, 0x6e, 0x14, 0xb2, 0x77,
    0x08, 0xc4, 0x18, 0xcd
};

static uint64_t stream_10a_off, stream_10b_off;

static int check_stream_10a(struct helper *h)
{
    /*
     * Must have filled or almost filled the packet (using default MDPL of
     * 1200).
     */
    if (!TEST_uint64_t_ge(h->frame.stream.len, 1150)
        || !TEST_uint64_t_le(h->frame.stream.len, 1200))
        return 0;

    if (!TEST_mem_eq(h->frame.stream.data, (size_t)h->frame.stream.len,
                     stream_10a, (size_t)h->frame.stream.len))
        return 0;

    stream_10a_off = h->frame.stream.offset + h->frame.stream.len;
    return 1;
}

static int check_stream_10b(struct helper *h)
{
    if (!TEST_uint64_t_ge(h->frame.stream.len, 1150)
        || !TEST_uint64_t_le(h->frame.stream.len, 1200))
        return 0;

    if (!TEST_mem_eq(h->frame.stream.data, (size_t)h->frame.stream.len,
                     stream_10b, (size_t)h->frame.stream.len))
        return 0;

    stream_10b_off = h->frame.stream.offset + h->frame.stream.len;
    return 1;
}

static int check_stream_10c(struct helper *h)
{
    if (!TEST_uint64_t_ge(h->frame.stream.len, 5)
        || !TEST_uint64_t_le(h->frame.stream.len, 200))
        return 0;

    if (!TEST_mem_eq(h->frame.stream.data, (size_t)h->frame.stream.len,
                     stream_10a + stream_10a_off, (size_t)h->frame.stream.len))
        return 0;

    return 1;
}

static int check_stream_10d(struct helper *h)
{
    if (!TEST_uint64_t_ge(h->frame.stream.len, 5)
        || !TEST_uint64_t_le(h->frame.stream.len, 200))
        return 0;

    if (!TEST_mem_eq(h->frame.stream.data, (size_t)h->frame.stream.len,
                     stream_10b + stream_10b_off, (size_t)h->frame.stream.len))
        return 0;

    return 1;
}

static const struct script_op script_10[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_HANDSHAKE_COMPLETE()
    OP_TXP_GENERATE_NONE()
    OP_STREAM_NEW(42)
    OP_STREAM_NEW(43)
    OP_CONN_TXFC_BUMP(10000)
    OP_STREAM_TXFC_BUMP(42, 5000)
    OP_STREAM_TXFC_BUMP(43, 5000)
    OP_STREAM_SEND(42, stream_10a)
    OP_STREAM_SEND(43, stream_10b)

    /* First packet containing data from stream 42 */
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1100, 1200)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STREAM)
    OP_CHECK(check_stream_10a)
    OP_EXPECT_NO_FRAME()

    /* Second packet containing data from stream 43 */
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1100, 1200)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STREAM)
    OP_CHECK(check_stream_10b)
    OP_EXPECT_NO_FRAME()

    /* Third packet containing data from stream 42 */
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(200, 500)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN)
    OP_CHECK(check_stream_10c)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STREAM_OFF)
    OP_CHECK(check_stream_10d)
    OP_EXPECT_NO_FRAME()

    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()

    OP_END
};

/* 11. Initial, CRYPTO */
static const struct script_op script_11[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_INITIAL, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CRYPTO_SEND(QUIC_PN_SPACE_INITIAL, crypto_1)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1200, 1200)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_CRYPTO)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 12. 1-RTT, STOP_SENDING */
static int check_stream_12(struct helper *h)
{
    if (!TEST_uint64_t_eq(h->frame.stop_sending.stream_id, 42)
        || !TEST_uint64_t_eq(h->frame.stop_sending.app_error_code, 4568))
        return 0;

    return 1;
}

static const struct script_op script_12[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_HANDSHAKE_COMPLETE()
    OP_TXP_GENERATE_NONE()
    OP_STREAM_NEW(42)
    OP_STOP_SENDING(42, 4568)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 128)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_STOP_SENDING)
    OP_CHECK(check_stream_12)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 13. 1-RTT, RESET_STREAM */
static const unsigned char stream_13[] = {
    0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x7a, 0x7b
};

static ossl_unused int check_stream_13(struct helper *h)
{
    if (!TEST_uint64_t_eq(h->frame.reset_stream.stream_id, 42)
        || !TEST_uint64_t_eq(h->frame.reset_stream.app_error_code, 4568)
        || !TEST_uint64_t_eq(h->frame.reset_stream.final_size, 0))
        return 0;

    return 1;
}

static const struct script_op script_13[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_HANDSHAKE_COMPLETE()
    OP_TXP_GENERATE_NONE()
    OP_STREAM_NEW(42)
    OP_CONN_TXFC_BUMP(8)
    OP_STREAM_TXFC_BUMP(42, 8)
    OP_STREAM_SEND(42, stream_13)
    OP_RESET_STREAM(42, 4568)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 128)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_RESET_STREAM)
    OP_CHECK(check_stream_13)
    OP_NEXT_FRAME()
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 14. 1-RTT, CONNECTION_CLOSE */
static int gen_conn_close(struct helper *h)
{
    OSSL_QUIC_FRAME_CONN_CLOSE f = {0};

    f.error_code     = 2345;
    f.frame_type     = OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE;
    f.reason         = "Reason string";
    f.reason_len     = strlen(f.reason);

    if (!TEST_true(ossl_quic_tx_packetiser_schedule_conn_close(h->txp, &f)))
        return 0;

    return 1;
}

static int check_14(struct helper *h)
{
    if (!TEST_int_eq(h->frame.conn_close.is_app, 0)
        || !TEST_uint64_t_eq(h->frame.conn_close.frame_type,
                             OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE)
        || !TEST_uint64_t_eq(h->frame.conn_close.error_code, 2345)
        || !TEST_mem_eq(h->frame.conn_close.reason, h->frame.conn_close.reason_len,
                        "Reason string", 13))
        return 0;

    return 1;
}

static const struct script_op script_14[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_HANDSHAKE_COMPLETE()
    OP_TXP_GENERATE_NONE()
    OP_CHECK(gen_conn_close)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT)
    OP_CHECK(check_14)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_END
};

/* 15. INITIAL, Anti-Deadlock Probe Simulation */
static int gen_probe_initial(struct helper *h)
{
    OSSL_ACKM_PROBE_INFO *probe = ossl_ackm_get0_probe_request(h->args.ackm);

    /*
     * Pretend the ACKM asked for an anti-deadlock Initial probe.
     * We test output of this in the ACKM unit tests.
     */
    ++probe->anti_deadlock_initial;
    return 1;
}

static const struct script_op script_15[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_INITIAL, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(gen_probe_initial)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1200, 1200)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_PING)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 16. HANDSHAKE, Anti-Deadlock Probe Simulation */
static int gen_probe_handshake(struct helper *h)
{
    OSSL_ACKM_PROBE_INFO *probe = ossl_ackm_get0_probe_request(h->args.ackm);

    /*
     * Pretend the ACKM asked for an anti-deadlock Handshake probe.
     * We test output of this in the ACKM unit tests.
     */
    ++probe->anti_deadlock_handshake;
    return 1;
}

static const struct script_op script_16[] = {
    OP_DISCARD_EL(QUIC_ENC_LEVEL_INITIAL)
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_HANDSHAKE, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(gen_probe_handshake)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_PING)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 17. 1-RTT, Probe Simulation */
static int gen_probe_1rtt(struct helper *h)
{
    OSSL_ACKM_PROBE_INFO *probe = ossl_ackm_get0_probe_request(h->args.ackm);

    /*
     * Pretend the ACKM asked for a 1-RTT PTO probe.
     * We test output of this in the ACKM unit tests.
     */
    ++probe->pto[QUIC_PN_SPACE_APP];
    return 1;
}

static const struct script_op script_17[] = {
    OP_DISCARD_EL(QUIC_ENC_LEVEL_INITIAL)
    OP_DISCARD_EL(QUIC_ENC_LEVEL_HANDSHAKE)
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_1RTT, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(gen_probe_1rtt)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(21, 512)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_PING)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

/* 18. Big Token Rejection */
static const unsigned char big_token[1950];

static int try_big_token(struct helper *h)
{
    size_t i;

    /* Ensure big token is rejected */
    if (!TEST_false(ossl_quic_tx_packetiser_set_initial_token(h->txp,
                                                              big_token,
                                                              sizeof(big_token),
                                                              NULL,
                                                              NULL)))
        return 0;

    /*
     * Keep trying until we find an acceptable size, then make sure
     * that works for generation
     */
    for (i = sizeof(big_token) - 1;; --i) {
        if (!TEST_size_t_gt(i, 0))
            return 0;

        if (ossl_quic_tx_packetiser_set_initial_token(h->txp, big_token, i,
                                                      NULL, NULL))
            break;
    }

    return 1;
}

static const struct script_op script_18[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_INITIAL, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CHECK(try_big_token)
    OP_TXP_GENERATE_NONE()
    OP_CRYPTO_SEND(QUIC_PN_SPACE_INITIAL, crypto_1)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1200, 1200)
    OP_NEXT_FRAME()
    OP_EXPECT_FRAME(OSSL_QUIC_FRAME_TYPE_CRYPTO)
    OP_EXPECT_NO_FRAME()
    OP_RX_PKT_NONE()
    OP_TXP_GENERATE_NONE()
    OP_END
};

static const struct script_op *const scripts[] = {
    script_1,
    script_2,
    script_3,
    script_4,
    script_5,
    script_6,
    script_7,
    script_8,
    script_9,
    script_10,
    script_11,
    script_12,
    script_13,
    script_14,
    script_15,
    script_16,
    script_17,
    script_18
};

static void skip_padding(struct helper *h)
{
    uint64_t frame_type;

    if (!ossl_quic_wire_peek_frame_header(&h->pkt, &frame_type, NULL))
        return; /* EOF */

    if (frame_type == OSSL_QUIC_FRAME_TYPE_PADDING)
        ossl_quic_wire_decode_padding(&h->pkt);
}

static int run_script(int script_idx, const struct script_op *script)
{
    int testresult = 0, have_helper = 0;
    QUIC_TXP_STATUS status;
    struct helper h;
    const struct script_op *op;
    size_t opn = 0;

    if (!helper_init(&h))
        goto err;

    have_helper = 1;
    for (op = script, opn = 0; op->opcode != OPK_END; ++op, ++opn) {
        switch (op->opcode) {
        case OPK_TXP_GENERATE:
            if (!TEST_true(ossl_quic_tx_packetiser_generate(h.txp, &status))
                && !TEST_size_t_gt(status.sent_pkt, 0))
                goto err;

            ossl_qtx_finish_dgram(h.args.qtx);
            ossl_qtx_flush_net(h.args.qtx);
            break;
        case OPK_TXP_GENERATE_NONE:
            if (!TEST_true(ossl_quic_tx_packetiser_generate(h.txp, &status))
                && !TEST_size_t_eq(status.sent_pkt, 0))
                goto err;

            break;
        case OPK_RX_PKT:
            ossl_quic_demux_pump(h.demux);
            ossl_qrx_pkt_release(h.qrx_pkt);
            h.qrx_pkt = NULL;
            if (!TEST_true(ossl_qrx_read_pkt(h.qrx, &h.qrx_pkt)))
                goto err;
            if (!TEST_true(PACKET_buf_init(&h.pkt,
                                           h.qrx_pkt->hdr->data,
                                           h.qrx_pkt->hdr->len)))
                goto err;
            h.frame_type = UINT64_MAX;
            break;
        case OPK_RX_PKT_NONE:
            ossl_quic_demux_pump(h.demux);
            if (!TEST_false(ossl_qrx_read_pkt(h.qrx, &h.qrx_pkt)))
                goto err;
            h.frame_type = UINT64_MAX;
            break;
        case OPK_EXPECT_DGRAM_LEN:
            if (!TEST_size_t_ge(h.qrx_pkt->datagram_len, (size_t)op->arg0)
                || !TEST_size_t_le(h.qrx_pkt->datagram_len, (size_t)op->arg1))
                goto err;
            break;
        case OPK_EXPECT_FRAME:
            if (!TEST_uint64_t_eq(h.frame_type, op->arg0))
                goto err;
            break;
        case OPK_EXPECT_INITIAL_TOKEN:
            if (!TEST_mem_eq(h.qrx_pkt->hdr->token, h.qrx_pkt->hdr->token_len,
                             op->buf, (size_t)op->arg0))
                goto err;
            break;
        case OPK_EXPECT_HDR:
            if (!TEST_true(cmp_pkt_hdr(h.qrx_pkt->hdr, op->buf,
                                       NULL, 0, 0)))
                goto err;
            break;
        case OPK_CHECK:
            if (!TEST_true(op->check_func(&h)))
                goto err;
            break;
        case OPK_NEXT_FRAME:
            skip_padding(&h);
            if (!ossl_quic_wire_peek_frame_header(&h.pkt, &h.frame_type, NULL)) {
                h.frame_type = UINT64_MAX;
                break;
            }

            switch (h.frame_type) {
            case OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE:
                if (!TEST_true(ossl_quic_wire_decode_frame_handshake_done(&h.pkt)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_PING:
                if (!TEST_true(ossl_quic_wire_decode_frame_ping(&h.pkt)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
                if (!TEST_true(ossl_quic_wire_decode_frame_max_data(&h.pkt,
                                                                    &h.frame.max_data)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
                if (!TEST_true(ossl_quic_wire_decode_frame_new_conn_id(&h.pkt,
                                                                       &h.frame.new_conn_id)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
                if (!TEST_true(ossl_quic_wire_decode_frame_new_token(&h.pkt,
                                                                     &h.frame.new_token.token,
                                                                     &h.frame.new_token.token_len)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
            case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
                h.frame.ack.ack_ranges      = h.ack_ranges;
                h.frame.ack.num_ack_ranges  = OSSL_NELEM(h.ack_ranges);
                if (!TEST_true(ossl_quic_wire_decode_frame_ack(&h.pkt,
                                                               h.args.ack_delay_exponent,
                                                               &h.frame.ack,
                                                               NULL)))
                    goto err;
                break;
            case OSSL_QUIC_FRAME_TYPE_CRYPTO:
                if (!TEST_true(ossl_quic_wire_decode_frame_crypto(&h.pkt, 0, &h.frame.crypto)))
                    goto err;
                break;

            case OSSL_QUIC_FRAME_TYPE_STREAM:
            case OSSL_QUIC_FRAME_TYPE_STREAM_FIN:
            case OSSL_QUIC_FRAME_TYPE_STREAM_LEN:
            case OSSL_QUIC_FRAME_TYPE_STREAM_LEN_FIN:
            case OSSL_QUIC_FRAME_TYPE_STREAM_OFF:
            case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_FIN:
            case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN:
            case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN_FIN:
                if (!TEST_true(ossl_quic_wire_decode_frame_stream(&h.pkt, 0, &h.frame.stream)))
                    goto err;
                break;

            case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
                if (!TEST_true(ossl_quic_wire_decode_frame_stop_sending(&h.pkt,
                                                                        &h.frame.stop_sending)))
                    goto err;
                break;

            case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
                if (!TEST_true(ossl_quic_wire_decode_frame_reset_stream(&h.pkt,
                                                                        &h.frame.reset_stream)))
                    goto err;
                break;

            case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
            case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
                if (!TEST_true(ossl_quic_wire_decode_frame_conn_close(&h.pkt,
                                                                      &h.frame.conn_close)))
                    goto err;
                break;

            default:
                TEST_error("unknown frame type");
                goto err;
            }
            break;
        case OPK_EXPECT_NO_FRAME:
            skip_padding(&h);
            if (!TEST_size_t_eq(PACKET_remaining(&h.pkt), 0))
                goto err;
            break;
        case OPK_PROVIDE_SECRET:
            if (!TEST_true(ossl_qtx_provide_secret(h.args.qtx,
                                                   (uint32_t)op->arg0,
                                                   (uint32_t)op->arg1,
                                                   NULL, op->buf, op->buf_len)))
                goto err;
            if (!TEST_true(ossl_qrx_provide_secret(h.qrx,
                                                   (uint32_t)op->arg0,
                                                   (uint32_t)op->arg1,
                                                   NULL, op->buf, op->buf_len)))
                goto err;
            break;
        case OPK_DISCARD_EL:
            if (!TEST_true(ossl_quic_tx_packetiser_discard_enc_level(h.txp,
                                                                     (uint32_t)op->arg0)))
                goto err;
            /*
             * We do not discard on the QRX here, the object is to test the
             * TXP so if the TXP does erroneously send at a discarded EL we
             * want to know about it.
             */
            break;
        case OPK_CRYPTO_SEND:
            {
                size_t consumed = 0;

                if (!TEST_true(ossl_quic_sstream_append(h.args.crypto[op->arg0],
                                                        op->buf, op->buf_len,
                                                        &consumed)))
                    goto err;

                if (!TEST_size_t_eq(consumed, op->buf_len))
                    goto err;
            }
            break;
        case OPK_STREAM_NEW:
            {
                QUIC_STREAM *s;

                if (!TEST_ptr(s = ossl_quic_stream_map_alloc(h.args.qsm, op->arg0,
                                                             QUIC_STREAM_DIR_BIDI)))
                    goto err;

                if (!TEST_ptr(s->sstream = ossl_quic_sstream_new(512 * 1024))
                    || !TEST_true(ossl_quic_txfc_init(&s->txfc, &h.conn_txfc))
                    || !TEST_true(ossl_quic_rxfc_init(&s->rxfc, &h.conn_rxfc,
                                                      1 * 1024 * 1024,
                                                      16 * 1024 * 1024,
                                                      fake_now, NULL))
                    || !TEST_ptr(s->rstream = ossl_quic_rstream_new(&s->rxfc,
                                                                    NULL, 1024))) {
                    ossl_quic_sstream_free(s->sstream);
                    ossl_quic_stream_map_release(h.args.qsm, s);
                    goto err;
                }
            }
            break;
        case OPK_STREAM_SEND:
            {
                QUIC_STREAM *s;
                size_t consumed = 0;

                if (!TEST_ptr(s = ossl_quic_stream_map_get_by_id(h.args.qsm,
                                                                 op->arg0)))
                    goto err;

                if (!TEST_true(ossl_quic_sstream_append(s->sstream, op->buf,
                                                        op->buf_len, &consumed)))
                    goto err;

                if (!TEST_size_t_eq(consumed, op->buf_len))
                    goto err;

                ossl_quic_stream_map_update_state(h.args.qsm, s);
            }
            break;
        case OPK_STREAM_FIN:
            {
                QUIC_STREAM *s;

                if (!TEST_ptr(s = ossl_quic_stream_map_get_by_id(h.args.qsm,
                                                                 op->arg0)))
                    goto err;

                ossl_quic_sstream_fin(s->sstream);
            }
            break;
        case OPK_STOP_SENDING:
            {
                QUIC_STREAM *s;

                if (!TEST_ptr(s = ossl_quic_stream_map_get_by_id(h.args.qsm,
                                                                 op->arg0)))
                    goto err;

                if (!TEST_true(ossl_quic_stream_map_stop_sending_recv_part(h.args.qsm,
                                                                           s, op->arg1)))
                    goto err;

                ossl_quic_stream_map_update_state(h.args.qsm, s);

                if (!TEST_true(s->active))
                    goto err;
            }
            break;
        case OPK_RESET_STREAM:
            {
                QUIC_STREAM *s;

                if (!TEST_ptr(s = ossl_quic_stream_map_get_by_id(h.args.qsm,
                                                                 op->arg0)))
                    goto err;

                if (!TEST_true(ossl_quic_stream_map_reset_stream_send_part(h.args.qsm,
                                                                           s, op->arg1)))
                    goto err;

                ossl_quic_stream_map_update_state(h.args.qsm, s);

                if (!TEST_true(s->active))
                    goto err;
            }
            break;
        case OPK_CONN_TXFC_BUMP:
            if (!TEST_true(ossl_quic_txfc_bump_cwm(h.args.conn_txfc, op->arg0)))
                goto err;

            break;
        case OPK_STREAM_TXFC_BUMP:
            {
                QUIC_STREAM *s;

                if (!TEST_ptr(s = ossl_quic_stream_map_get_by_id(h.args.qsm,
                                                                 op->arg1)))
                    goto err;

                if (!TEST_true(ossl_quic_txfc_bump_cwm(&s->txfc, op->arg0)))
                    goto err;

                ossl_quic_stream_map_update_state(h.args.qsm, s);
            }
            break;
        case OPK_HANDSHAKE_COMPLETE:
            ossl_quic_tx_packetiser_notify_handshake_complete(h.txp);
            break;
        case OPK_NOP:
            break;
        default:
            TEST_error("bad opcode");
            goto err;
        }
    }

    testresult = 1;
err:
    if (!testresult)
        TEST_error("script %d failed at op %zu", script_idx + 1, opn + 1);
    if (have_helper)
        helper_cleanup(&h);
    return testresult;
}

static int test_script(int idx)
{
    return run_script(idx, scripts[idx]);
}

/*
 * Dynamic Script 1.
 *
 * This script exists to test the interactions between multiple packets (ELs) in
 * the same datagram when there is a padding requirement (due to the datagram
 * containing an Initial packet). There are boundary cases which are difficult
 * to get right so it is important to test this entire space. Examples of such
 * edge cases include:
 *
 * - If we are planning on generating both an Initial and Handshake packet in a
 *   datagram ordinarily we would plan on adding the padding frames to meet the
 *   mandatory minimum size to the last packet in the datagram (i.e., the
 *   Handshake packet). But if the amount of room remaining in a datagram is
 *   e.g. only 3 bytes after generating the Initial packet, this is not
 *   enough room for another packet and we have a problem as having finished
 *   the Initial packet we have no way to add the necessary padding.
 *
 * - If we do have room for another packet but it is not enough room to encode
 *   any desired frame.
 *
 * This test confirms we handle these cases correctly for multi-packet datagrams
 * by placing two packets in a datagram and varying the size of the first
 * datagram.
 */
static const unsigned char dyn_script_1_crypto_1a[1200];
static const unsigned char dyn_script_1_crypto_1b[1];

static int check_is_initial(struct helper *h)
{
    return h->qrx_pkt->hdr->type == QUIC_PKT_TYPE_INITIAL;
}

static int check_is_handshake(struct helper *h)
{
    return h->qrx_pkt->hdr->type == QUIC_PKT_TYPE_HANDSHAKE;
}

static struct script_op dyn_script_1[] = {
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_INITIAL, QRL_SUITE_AES128GCM, secret_1)
    OP_PROVIDE_SECRET(QUIC_ENC_LEVEL_HANDSHAKE, QRL_SUITE_AES128GCM, secret_1)
    OP_TXP_GENERATE_NONE()
    OP_CRYPTO_SEND(QUIC_PN_SPACE_INITIAL, dyn_script_1_crypto_1a) /* [crypto_idx] */
    OP_CRYPTO_SEND(QUIC_PN_SPACE_HANDSHAKE, dyn_script_1_crypto_1b)
    OP_TXP_GENERATE()
    OP_RX_PKT()
    OP_EXPECT_DGRAM_LEN(1200, 1200)
    OP_CHECK(check_is_initial)
    OP_NOP() /* [pkt_idx] */
    OP_NOP() /* [check_idx] */
    OP_END
};

static const size_t dyn_script_1_crypto_idx     = 3;
static const size_t dyn_script_1_pkt_idx        = 9;
static const size_t dyn_script_1_check_idx      = 10;
static const size_t dyn_script_1_start_from     = 1000;

static int test_dyn_script_1(int idx)
{
    size_t target_size = dyn_script_1_start_from + (size_t)idx;
    int expect_handshake_pkt_in_same_dgram = (target_size <= 1115);

    dyn_script_1[dyn_script_1_crypto_idx].buf_len = target_size;

    if (expect_handshake_pkt_in_same_dgram) {
        dyn_script_1[dyn_script_1_pkt_idx].opcode       = OPK_RX_PKT;
        dyn_script_1[dyn_script_1_check_idx].opcode     = OPK_CHECK;
        dyn_script_1[dyn_script_1_check_idx].check_func = check_is_handshake;
    } else {
        dyn_script_1[dyn_script_1_pkt_idx].opcode       = OPK_RX_PKT_NONE;
        dyn_script_1[dyn_script_1_check_idx].opcode     = OPK_NOP;
    }

    if (!run_script(idx, dyn_script_1)) {
        TEST_error("failed dyn script 1 with target size %zu", target_size);
        return 0;
    }

    return 1;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_script, OSSL_NELEM(scripts));
    ADD_ALL_TESTS(test_dyn_script_1,
                  OSSL_NELEM(dyn_script_1_crypto_1a)
                  - dyn_script_1_start_from + 1);
    return 1;
}
