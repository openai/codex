/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "testutil.h"
#include <openssl/ssl.h>
#include "internal/quic_ackm.h"
#include "internal/quic_cc.h"

static OSSL_TIME fake_time = {0};

#define TIME_BASE (ossl_ticks2time(123 * OSSL_TIME_SECOND))

static OSSL_TIME fake_now(void *arg)
{
    return fake_time;
}

struct pkt_info {
    OSSL_ACKM_TX_PKT *pkt;
    int lost, acked, discarded;
};

static void on_lost(void *arg)
{
    struct pkt_info *info = arg;
    ++info->lost;
}

static void on_acked(void *arg)
{
    struct pkt_info *info = arg;
    ++info->acked;
}

static void on_discarded(void *arg)
{
    struct pkt_info *info = arg;
    ++info->discarded;
}

struct helper {
    OSSL_ACKM *ackm;
    struct pkt_info *pkts;
    size_t num_pkts;
    OSSL_CC_DATA *ccdata;
    OSSL_STATM statm;
    int have_statm;
};

static void helper_destroy(struct helper *h)
{
    size_t i;

    if (h->ackm != NULL) {
        ossl_ackm_free(h->ackm);
        h->ackm = NULL;
    }

    if (h->ccdata != NULL) {
        ossl_cc_dummy_method.free(h->ccdata);
        h->ccdata = NULL;
    }

    if (h->have_statm) {
        ossl_statm_destroy(&h->statm);
        h->have_statm = 0;
    }

    if (h->pkts != NULL) {
        for (i = 0; i < h->num_pkts; ++i) {
            OPENSSL_free(h->pkts[i].pkt);
            h->pkts[i].pkt = NULL;
        }

        OPENSSL_free(h->pkts);
        h->pkts = NULL;
    }
}

static int helper_init(struct helper *h, size_t num_pkts)
{
    int rc = 0;

    memset(h, 0, sizeof(*h));

    fake_time = TIME_BASE;

    /* Initialise statistics tracker. */
    if (!TEST_int_eq(ossl_statm_init(&h->statm), 1))
        goto err;

    h->have_statm = 1;

    /* Initialise congestion controller. */
    h->ccdata = ossl_cc_dummy_method.new(fake_now, NULL);
    if (!TEST_ptr(h->ccdata))
        goto err;

    /* Initialise ACK manager. */
    h->ackm = ossl_ackm_new(fake_now, NULL, &h->statm,
                            &ossl_cc_dummy_method, h->ccdata);
    if (!TEST_ptr(h->ackm))
        goto err;

    /* Allocate our array of packet information. */
    h->num_pkts = num_pkts;
    if (num_pkts > 0) {
        h->pkts = OPENSSL_zalloc(sizeof(struct pkt_info) * num_pkts);
        if (!TEST_ptr(h->pkts))
            goto err;
    } else {
        h->pkts = NULL;
    }

    rc = 1;
err:
    if (rc == 0)
        helper_destroy(h);

    return rc;
}

static const QUIC_PN linear_20[] = {
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19
};

static const QUIC_PN high_linear_20[] = {
    1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008,
    1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017,
    1018, 1019
};

/*
 * TX ACK (Packet Threshold) Test Cases
 * ******************************************************************
 */
struct tx_ack_test_case {
    const QUIC_PN              *pn_table;
    size_t                      pn_table_len;
    const OSSL_QUIC_ACK_RANGE  *ack_ranges;
    size_t                      num_ack_ranges;
    const char                 *expect_ack; /* 1=ack, 2=lost, 4=discarded */
};

#define DEFINE_TX_ACK_CASE(n, pntable)                          \
    static const struct tx_ack_test_case tx_ack_case_##n = {    \
        (pntable), OSSL_NELEM(pntable),                         \
        tx_ack_range_##n, OSSL_NELEM(tx_ack_range_##n),         \
        tx_ack_expect_##n                                       \
    }

/* One range, partial coverage of space */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_1[] = {
    { 0, 10 },
};
static const char tx_ack_expect_1[] = {
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(1, linear_20);

/* Two ranges, partial coverage of space, overlapping by 1 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_2[] = {
    { 5, 10 }, { 0, 5 }
};
static const char tx_ack_expect_2[] = {
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(2, linear_20);

/* Two ranges, partial coverage of space, together contiguous */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_3[] = {
    { 6, 10 }, { 0, 5 }
};
static const char tx_ack_expect_3[] = {
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(3, linear_20);

/*
 * Two ranges, partial coverage of space, non-contiguous by 1
 * Causes inferred loss due to packet threshold being exceeded.
 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_4[] = {
    { 7, 10 }, { 0, 5 }
};
static const char tx_ack_expect_4[] = {
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(4, linear_20);

/*
 * Two ranges, partial coverage of space, non-contiguous by 2
 * Causes inferred loss due to packet threshold being exceeded.
 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_5[] = {
    { 7, 10 }, { 0, 4 }
};
static const char tx_ack_expect_5[] = {
    1, 1, 1, 1, 1, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(5, linear_20);

/* One range, covering entire space */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_6[] = {
    { 0, 20 },
};
static const char tx_ack_expect_6[] = {
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1
};
DEFINE_TX_ACK_CASE(6, linear_20);

/* One range, covering more space than exists */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_7[] = {
    { 0, 30 },
};
static const char tx_ack_expect_7[] = {
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1
};
DEFINE_TX_ACK_CASE(7, linear_20);

/* One range, covering nothing (too high) */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_8[] = {
    { 21, 30 },
};
static const char tx_ack_expect_8[] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(8, linear_20);

/* One range, covering nothing (too low) */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_9[] = {
    { 0, 999 },
};
static const char tx_ack_expect_9[] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(9, high_linear_20);

/* One single packet at start of PN set */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_10[] = {
    { 0, 0 },
};
static const char tx_ack_expect_10[] = {
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(10, linear_20);

/*
 * One single packet in middle of PN set
 * Causes inferred loss of one packet due to packet threshold being exceeded,
 * but several other previous packets survive as they are under the threshold.
 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_11[] = {
    { 3, 3 },
};
static const char tx_ack_expect_11[] = {
    2, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(11, linear_20);

/*
 * One single packet at end of PN set
 * Causes inferred loss due to packet threshold being exceeded.
 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_12[] = {
    { 19, 19 },
};
static const char tx_ack_expect_12[] = {
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 0, 1
};
DEFINE_TX_ACK_CASE(12, linear_20);

/*
 * Mixed straddling
 * Causes inferred loss due to packet threshold being exceeded.
 */
static const OSSL_QUIC_ACK_RANGE tx_ack_range_13[] = {
    { 1008, 1008 }, { 1004, 1005 }, { 1001, 1002 }
};
static const char tx_ack_expect_13[] = {
    2, 1, 1, 2, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};
DEFINE_TX_ACK_CASE(13, high_linear_20);

static const struct tx_ack_test_case *const tx_ack_cases[] = {
    &tx_ack_case_1,
    &tx_ack_case_2,
    &tx_ack_case_3,
    &tx_ack_case_4,
    &tx_ack_case_5,
    &tx_ack_case_6,
    &tx_ack_case_7,
    &tx_ack_case_8,
    &tx_ack_case_9,
    &tx_ack_case_10,
    &tx_ack_case_11,
    &tx_ack_case_12,
    &tx_ack_case_13,
};

enum {
    MODE_ACK, MODE_DISCARD, MODE_PTO, MODE_NUM
};

static int test_probe_counts(const OSSL_ACKM_PROBE_INFO *p,
                             uint32_t anti_deadlock_handshake,
                             uint32_t anti_deadlock_initial,
                             uint32_t pto_initial,
                             uint32_t pto_handshake,
                             uint32_t pto_app)
{
    if (!TEST_uint_eq(p->anti_deadlock_handshake, anti_deadlock_handshake))
        return 0;
    if (!TEST_uint_eq(p->anti_deadlock_initial, anti_deadlock_initial))
        return 0;
    if (!TEST_uint_eq(p->pto[QUIC_PN_SPACE_INITIAL], pto_initial))
        return 0;
    if (!TEST_uint_eq(p->pto[QUIC_PN_SPACE_HANDSHAKE], pto_handshake))
        return 0;
    if (!TEST_uint_eq(p->pto[QUIC_PN_SPACE_APP], pto_app))
        return 0;
    return 1;
}

static void on_loss_detection_deadline_callback(OSSL_TIME deadline, void *arg)
{
    *(OSSL_TIME *)arg = deadline;
}

static int test_tx_ack_case_actual(int tidx, int space, int mode)
{
    int testresult = 0;
    struct helper h;
    size_t i;
    OSSL_ACKM_TX_PKT *tx;
    const struct tx_ack_test_case *c = tx_ack_cases[tidx];
    OSSL_QUIC_FRAME_ACK ack = {0};
    OSSL_TIME loss_detection_deadline = ossl_time_zero();

    /* Cannot discard app space, so skip this */
    if (mode == MODE_DISCARD && space == QUIC_PN_SPACE_APP) {
        TEST_skip("skipping test for app space");
        return 1;
    }

    if (!TEST_int_eq(helper_init(&h, c->pn_table_len), 1))
        goto err;

    /* Arm callback. */
    ossl_ackm_set_loss_detection_deadline_callback(h.ackm,
                                                   on_loss_detection_deadline_callback,
                                                   &loss_detection_deadline);

    /* Allocate TX packet structures. */
    for (i = 0; i < c->pn_table_len; ++i) {
        h.pkts[i].pkt = tx = OPENSSL_zalloc(sizeof(*tx));
        if (!TEST_ptr(tx))
            goto err;

        tx->pkt_num             = c->pn_table[i];
        tx->pkt_space           = space;
        tx->is_inflight         = 1;
        tx->is_ack_eliciting    = 1;
        tx->num_bytes           = 123;
        tx->largest_acked       = QUIC_PN_INVALID;
        tx->on_lost             = on_lost;
        tx->on_acked            = on_acked;
        tx->on_discarded        = on_discarded;
        tx->cb_arg              = &h.pkts[i];

        tx->time  = fake_time;

        if (!TEST_int_eq(ossl_ackm_on_tx_packet(h.ackm, tx), 1))
            goto err;
    }

    if (mode == MODE_DISCARD) {
        /* Try discarding. */
        if (!TEST_int_eq(ossl_ackm_on_pkt_space_discarded(h.ackm, space), 1))
            goto err;

        /* Check all discard callbacks were called. */
        for (i  = 0; i < c->pn_table_len; ++i) {
            if (!TEST_int_eq(h.pkts[i].acked, 0))
                goto err;
            if (!TEST_int_eq(h.pkts[i].lost, 0))
                goto err;
            if (!TEST_int_eq(h.pkts[i].discarded, 1))
                goto err;
        }
    } else if (mode == MODE_ACK) {
        /* Try acknowledging. */
        ack.ack_ranges      = (OSSL_QUIC_ACK_RANGE *)c->ack_ranges;
        ack.num_ack_ranges  = c->num_ack_ranges;
        if (!TEST_int_eq(ossl_ackm_on_rx_ack_frame(h.ackm, &ack, space, fake_time), 1))
            goto err;

        /* Check correct ranges were acknowledged. */
        for (i = 0; i < c->pn_table_len; ++i) {
            if (!TEST_int_eq(h.pkts[i].acked,
                             (c->expect_ack[i] & 1) != 0 ? 1 : 0))
                goto err;
            if (!TEST_int_eq(h.pkts[i].lost,
                             (c->expect_ack[i] & 2) != 0 ? 1 : 0))
                goto err;
            if (!TEST_int_eq(h.pkts[i].discarded,
                             (c->expect_ack[i] & 4) != 0 ? 1 : 0))
                goto err;
        }
    } else if (mode == MODE_PTO) {
        OSSL_TIME deadline = ossl_ackm_get_loss_detection_deadline(h.ackm);
        OSSL_ACKM_PROBE_INFO probe;

        if (!TEST_int_eq(ossl_time_compare(deadline, loss_detection_deadline), 0))
            goto err;

        /* We should have a PTO deadline. */
        if (!TEST_int_gt(ossl_time_compare(deadline, fake_time), 0))
            goto err;

        /* Should not have any probe requests yet. */
        probe = *ossl_ackm_get0_probe_request(h.ackm);
        if (!TEST_int_eq(test_probe_counts(&probe, 0, 0, 0, 0, 0), 1))
            goto err;

        /*
         * If in app space, confirm handshake, as this is necessary to enable
         * app space PTO probe requests.
         */
        if (space == QUIC_PN_SPACE_APP)
            if (!TEST_int_eq(ossl_ackm_on_handshake_confirmed(h.ackm), 1))
                goto err;

        /* Advance to the PTO deadline. */
        fake_time = ossl_time_add(deadline, ossl_ticks2time(1));

        if (!TEST_int_eq(ossl_ackm_on_timeout(h.ackm), 1))
            goto err;

        /* Should have a probe request. Not cleared by first call. */
        for (i = 0; i < 3; ++i) {
            probe = *ossl_ackm_get0_probe_request(h.ackm);
            if (i > 0)
                memset(ossl_ackm_get0_probe_request(h.ackm), 0, sizeof(probe));

            if (i == 2) {
                if (!TEST_int_eq(test_probe_counts(&probe, 0, 0, 0, 0, 0), 1))
                    goto err;
            } else {
                if (!TEST_int_eq(test_probe_counts(&probe, 0, 0,
                                                   space == QUIC_PN_SPACE_INITIAL,
                                                   space == QUIC_PN_SPACE_HANDSHAKE,
                                                   space == QUIC_PN_SPACE_APP), 1))
                    goto err;
            }
        }

    } else
        goto err;

    testresult = 1;
err:
    helper_destroy(&h);
    return testresult;
}

/*
 * TX ACK (Time Threshold) Test
 * ******************************************************************
 */
enum {
    TX_ACK_TIME_OP_END,
    TX_ACK_TIME_OP_PKT,     /* TX packets */
    TX_ACK_TIME_OP_ACK,     /* Synthesise incoming ACK of single PN range */
    TX_ACK_TIME_OP_EXPECT   /* Ack/loss assertion */
};

struct tx_ack_time_op {
    int       kind;
    uint64_t  time_advance; /* all ops */
    QUIC_PN   pn;           /* PKT, ACK */
    size_t    num_pn;       /* PKT, ACK */
    const char *expect;     /* 1=ack, 2=lost, 4=discarded */
};

#define TX_OP_PKT(advance, pn, num_pn) \
    { TX_ACK_TIME_OP_PKT, (advance) * OSSL_TIME_MS, (pn), (num_pn), NULL },
#define TX_OP_ACK(advance, pn, num_pn) \
    { TX_ACK_TIME_OP_ACK, (advance) * OSSL_TIME_MS, (pn), (num_pn), NULL },
#define TX_OP_EXPECT(expect) \
    { TX_ACK_TIME_OP_EXPECT, 0, 0, 0, (expect) },
#define TX_OP_END { TX_ACK_TIME_OP_END }

static const char tx_ack_time_script_1_expect[] = {
    2, 1
};

static const struct tx_ack_time_op tx_ack_time_script_1[] = {
    TX_OP_PKT   (      0, 0, 1)
    TX_OP_PKT   (3600000, 1, 1)
    TX_OP_ACK   (   1000, 1, 1)
    TX_OP_EXPECT(tx_ack_time_script_1_expect)
    TX_OP_END
};

static const struct tx_ack_time_op *const tx_ack_time_scripts[] = {
    tx_ack_time_script_1,
};

static int test_tx_ack_time_script(int tidx)
{
    int testresult = 0;
    struct helper h;
    OSSL_ACKM_TX_PKT *tx = NULL;
    OSSL_QUIC_FRAME_ACK ack = {0};
    OSSL_QUIC_ACK_RANGE ack_range = {0};
    size_t i, num_pkts = 0, pkt_idx = 0;
    const struct tx_ack_time_op *script = tx_ack_time_scripts[tidx], *s;

    /* Calculate number of packets. */
    for (s = script; s->kind != TX_ACK_TIME_OP_END; ++s)
        if (s->kind == TX_ACK_TIME_OP_PKT)
            num_pkts += s->num_pn;

    /* Initialise ACK manager and packet structures. */
    if (!TEST_int_eq(helper_init(&h, num_pkts), 1))
        goto err;

    for (i = 0; i < num_pkts; ++i) {
        h.pkts[i].pkt = tx = OPENSSL_zalloc(sizeof(*tx));
        if (!TEST_ptr(tx))
            goto err;
    }

    /* Run script. */
    for (s = script; s->kind != TX_ACK_TIME_OP_END; ++s)
        switch (s->kind) {
            case TX_ACK_TIME_OP_PKT:
                for (i = 0; i < s->num_pn; ++i) {
                    tx = h.pkts[pkt_idx + i].pkt;

                    tx->pkt_num             = s->pn + i;
                    tx->pkt_space           = QUIC_PN_SPACE_INITIAL;
                    tx->num_bytes           = 123;
                    tx->largest_acked       = QUIC_PN_INVALID;
                    tx->is_inflight         = 1;
                    tx->is_ack_eliciting    = 1;
                    tx->on_lost             = on_lost;
                    tx->on_acked            = on_acked;
                    tx->on_discarded        = on_discarded;
                    tx->cb_arg              = &h.pkts[pkt_idx + i];

                    fake_time = ossl_time_add(fake_time,
                                              ossl_ticks2time(s->time_advance));
                    tx->time   = fake_time;

                    if (!TEST_int_eq(ossl_ackm_on_tx_packet(h.ackm, tx), 1))
                        goto err;
                }

                pkt_idx += s->num_pn;
                break;

            case TX_ACK_TIME_OP_ACK:
                ack.ack_ranges      = &ack_range;
                ack.num_ack_ranges  = 1;

                ack_range.start     = s->pn;
                ack_range.end       = s->pn + s->num_pn;

                fake_time = ossl_time_add(fake_time,
                                          ossl_ticks2time(s->time_advance));

                if (!TEST_int_eq(ossl_ackm_on_rx_ack_frame(h.ackm, &ack,
                                                           QUIC_PN_SPACE_INITIAL,
                                                           fake_time), 1))
                    goto err;

                break;

            case TX_ACK_TIME_OP_EXPECT:
                for (i = 0; i < num_pkts; ++i) {
                    if (!TEST_int_eq(h.pkts[i].acked,
                                     (s->expect[i] & 1) != 0 ? 1 : 0))
                        goto err;
                    if (!TEST_int_eq(h.pkts[i].lost,
                                     (s->expect[i] & 2) != 0 ? 1 : 0))
                        goto err;
                    if (!TEST_int_eq(h.pkts[i].discarded,
                                     (s->expect[i] & 4) != 0 ? 1 : 0))
                        goto err;
                }

                break;
        }

    testresult = 1;
err:
    helper_destroy(&h);
    return testresult;
}

/*
 * RX ACK Test
 * ******************************************************************
 */
enum {
    RX_OPK_END,
    RX_OPK_PKT,              /* RX packet */
    RX_OPK_CHECK_UNPROC,     /* check PNs unprocessable */
    RX_OPK_CHECK_PROC,       /* check PNs processable */
    RX_OPK_CHECK_STATE,      /* check is_desired/deadline */
    RX_OPK_CHECK_ACKS,       /* check ACK ranges */
    RX_OPK_TX,               /* TX packet */
    RX_OPK_RX_ACK,           /* RX ACK frame */
    RX_OPK_SKIP_IF_PN_SPACE  /* skip for a given PN space */
};

struct rx_test_op {
    int                         kind;
    uint64_t                    time_advance;

    QUIC_PN                     pn;     /* PKT, CHECK_(UN)PROC, TX, RX_ACK */
    size_t                      num_pn; /* PKT, CHECK_(UN)PROC, TX, RX_ACK */

    char                        expect_desired;     /* CHECK_STATE */
    char                        expect_deadline;    /* CHECK_STATE */

    const OSSL_QUIC_ACK_RANGE  *ack_ranges;         /* CHECK_ACKS */
    size_t                      num_ack_ranges;     /* CHECK_ACKS */

    QUIC_PN                     largest_acked;      /* TX */
};

#define RX_OP_PKT(advance, pn, num_pn)                              \
    {                                                               \
      RX_OPK_PKT, (advance) * OSSL_TIME_MS, (pn), (num_pn),         \
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_CHECK_UNPROC(advance, pn, num_pn)                     \
    {                                                               \
      RX_OPK_CHECK_UNPROC, (advance) * OSSL_TIME_MS, (pn), (num_pn),\
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_CHECK_PROC(advance, pn, num_pn)                       \
    {                                                               \
      RX_OPK_CHECK_PROC, (advance) * OSSL_TIME_MS, (pn), (num_pn),  \
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_CHECK_STATE(advance, expect_desired, expect_deadline) \
    {                                                               \
      RX_OPK_CHECK_STATE, (advance) * OSSL_TIME_MS, 0, 0,           \
      (expect_desired), (expect_deadline), NULL, 0, 0               \
    },

#define RX_OP_CHECK_ACKS(advance, ack_ranges)                       \
    {                                                               \
      RX_OPK_CHECK_ACKS, (advance) * OSSL_TIME_MS, 0, 0,            \
      0, 0, (ack_ranges), OSSL_NELEM(ack_ranges), 0                 \
    },

#define RX_OP_CHECK_NO_ACKS(advance)                                \
    {                                                               \
      RX_OPK_CHECK_ACKS, (advance) * OSSL_TIME_MS, 0, 0,            \
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_TX(advance, pn, largest_acked)                        \
    {                                                               \
      RX_OPK_TX, (advance) * OSSL_TIME_MS, (pn), 1,                 \
      0, 0, NULL, 0, (largest_acked)                                \
    },

#define RX_OP_RX_ACK(advance, pn, num_pn)                           \
    {                                                               \
      RX_OPK_RX_ACK, (advance) * OSSL_TIME_MS, (pn), (num_pn),      \
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_SKIP_IF_PN_SPACE(pn_space)                            \
    {                                                               \
      RX_OPK_SKIP_IF_PN_SPACE, 0, (pn_space), 0,                    \
      0, 0, NULL, 0, 0                                              \
    },

#define RX_OP_END                                                   \
    { RX_OPK_END }

/* RX 1. Simple Test with ACK Desired (Packet Threshold, Exactly) */
static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_1a[] = {
    { 0, 1 }
};

static const struct rx_test_op rx_script_1[] = {
    RX_OP_CHECK_STATE   (0, 0, 0)   /* no threshold yet */
    RX_OP_CHECK_PROC    (0, 0, 3)

    RX_OP_PKT           (0, 0, 2)   /* two packets, threshold */
    RX_OP_CHECK_UNPROC  (0, 0, 2)
    RX_OP_CHECK_PROC    (0, 2, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* threshold met, immediate */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_1a)

    /* At this point we would generate e.g. a packet with an ACK. */
    RX_OP_TX            (0, 0, 1)   /* ACKs both */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_1a) /* not provably ACKed yet */
    RX_OP_RX_ACK        (0, 0, 1)   /* TX'd packet is ACK'd */

    RX_OP_CHECK_NO_ACKS (0)         /* nothing more to ACK */
    RX_OP_CHECK_UNPROC  (0, 0, 2)   /* still unprocessable */
    RX_OP_CHECK_PROC    (0, 2, 1)   /* still processable */

    RX_OP_END
};

/* RX 2. Simple Test with ACK Not Yet Desired (Packet Threshold) (1-RTT) */
static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_2a[] = {
    { 0, 0 }
};

static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_2b[] = {
    { 0, 2 }
};

static const struct rx_test_op rx_script_2[] = {
    /*
     * We skip this for INITIAL/HANDSHAKE and use a separate version
     * (rx_script_4) for those spaces as those spaces should not delay ACK
     * generation, so a different RX_OP_CHECK_STATE test is needed.
     */
    RX_OP_SKIP_IF_PN_SPACE(QUIC_PN_SPACE_INITIAL)
    RX_OP_SKIP_IF_PN_SPACE(QUIC_PN_SPACE_HANDSHAKE)

    RX_OP_CHECK_STATE   (0, 0, 0)   /* no threshold yet */
    RX_OP_CHECK_PROC    (0, 0, 3)

    /* First packet always generates an ACK so get it out of the way. */
    RX_OP_PKT           (0, 0, 1)
    RX_OP_CHECK_UNPROC  (0, 0, 1)
    RX_OP_CHECK_PROC    (0, 1, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* first packet always causes ACK */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_2a) /* clears packet counter */
    RX_OP_CHECK_STATE   (0, 0, 0)   /* desired state should have been cleared */

    /* Second packet should not cause ACK-desired state */
    RX_OP_PKT           (0, 1, 1)   /* just one packet, threshold is 2 */
    RX_OP_CHECK_UNPROC  (0, 0, 2)
    RX_OP_CHECK_PROC    (0, 2, 1)
    RX_OP_CHECK_STATE   (0, 0, 1)   /* threshold not yet met, so deadline */
    /* Don't check ACKs here, as it would reset our threshold counter. */

    /* Now receive a second packet, triggering the threshold */
    RX_OP_PKT           (0, 2, 1)   /* second packet meets threshold */
    RX_OP_CHECK_UNPROC  (0, 0, 3)
    RX_OP_CHECK_PROC    (0, 3, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* desired immediately */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_2b)

    /* At this point we would generate e.g. a packet with an ACK. */
    RX_OP_TX            (0, 0, 2)   /* ACKs all */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_2b) /* not provably ACKed yet */
    RX_OP_RX_ACK        (0, 0, 1)   /* TX'd packet is ACK'd */

    RX_OP_CHECK_NO_ACKS (0)         /* nothing more to ACK */
    RX_OP_CHECK_UNPROC  (0, 0, 3)   /* still unprocessable */
    RX_OP_CHECK_PROC    (0, 3, 1)   /* still processable */

    RX_OP_END
};

/* RX 3. Simple Test with ACK Desired (Packet Threshold, Multiple Watermarks) */
static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_3a[] = {
    { 0, 0 }
};

static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_3b[] = {
    { 0, 10 }
};

static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_3c[] = {
    { 6, 10 }
};

static const struct rx_test_op rx_script_3[] = {
    RX_OP_CHECK_STATE   (0, 0, 0)   /* no threshold yet */
    RX_OP_CHECK_PROC    (0, 0, 11)

    /* First packet always generates an ACK so get it out of the way. */
    RX_OP_PKT           (0, 0, 1)
    RX_OP_CHECK_UNPROC  (0, 0, 1)
    RX_OP_CHECK_PROC    (0, 1, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* first packet always causes ACK */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_3a) /* clears packet counter */
    RX_OP_CHECK_STATE   (0, 0, 0)   /* desired state should have been cleared */

    /* Generate ten packets, exceeding the threshold. */
    RX_OP_PKT           (0, 1, 10)  /* ten packets, threshold is 2 */
    RX_OP_CHECK_UNPROC  (0, 0, 11)
    RX_OP_CHECK_PROC    (0, 11, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* threshold met, immediate */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_3b)

    /*
     * Test TX'ing a packet which doesn't ACK anything.
     */
    RX_OP_TX            (0, 0, QUIC_PN_INVALID)
    RX_OP_RX_ACK        (0, 0, 1)

    /*
     * At this point we would generate a packet with an ACK immediately.
     * TX a packet which when ACKed makes [0,5] provably ACKed.
     */
    RX_OP_TX            (0, 1, 5)
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_3b) /* not provably ACKed yet */
    RX_OP_RX_ACK        (0, 1, 1)

    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_3c) /* provably ACKed now gone */
    RX_OP_CHECK_UNPROC  (0, 0, 11) /* still unprocessable */
    RX_OP_CHECK_PROC    (0, 11, 1) /* still processable */

    /*
     * Now TX another packet which provably ACKs the rest when ACKed.
     */
    RX_OP_TX            (0, 2, 10)
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_3c) /* not provably ACKed yet */
    RX_OP_RX_ACK        (0, 2, 1)

    RX_OP_CHECK_NO_ACKS (0)         /* provably ACKed now gone */
    RX_OP_CHECK_UNPROC  (0, 0, 11)  /* still unprocessable */
    RX_OP_CHECK_PROC    (0, 11, 1)  /* still processable */

    RX_OP_END
};

/*
 * RX 4. Simple Test with ACK Not Yet Desired (Packet Threshold)
 * (Initial/Handshake)
 */
static const OSSL_QUIC_ACK_RANGE rx_ack_ranges_4a[] = {
    { 0, 1 }
};

static const struct rx_test_op rx_script_4[] = {
    /* The application PN space is tested in rx_script_2. */
    RX_OP_SKIP_IF_PN_SPACE(QUIC_PN_SPACE_APP)

    RX_OP_CHECK_STATE   (0, 0, 0)   /* no threshold yet */
    RX_OP_CHECK_PROC    (0, 0, 3)

    /* First packet always generates an ACK so get it out of the way. */
    RX_OP_PKT           (0, 0, 1)
    RX_OP_CHECK_UNPROC  (0, 0, 1)
    RX_OP_CHECK_PROC    (0, 1, 1)
    RX_OP_CHECK_STATE   (0, 1, 0)   /* first packet always causes ACK */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_2a) /* clears packet counter */
    RX_OP_CHECK_STATE   (0, 0, 0)   /* desired state should have been cleared */

    /*
     * Second packet should cause ACK-desired state because we are
     * INITIAL/HANDSHAKE (RFC 9000 s. 13.2.1)
     */
    RX_OP_PKT           (0, 1, 1)   /* just one packet, threshold is 2 */
    RX_OP_CHECK_UNPROC  (0, 0, 2)
    RX_OP_CHECK_PROC    (0, 2, 1)
    RX_OP_CHECK_STATE   (0, 1, 1)
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_4a)
    RX_OP_CHECK_STATE   (0, 0, 0)   /* desired state should have been cleared */

    /* At this point we would generate e.g. a packet with an ACK. */
    RX_OP_TX            (0, 0, 1)   /* ACKs all */
    RX_OP_CHECK_ACKS    (0, rx_ack_ranges_4a) /* not provably ACKed yet */
    RX_OP_RX_ACK        (0, 0, 1)   /* TX'd packet is ACK'd */

    RX_OP_CHECK_NO_ACKS (0)         /* nothing more to ACK */
    RX_OP_CHECK_UNPROC  (0, 0, 2)   /* still unprocessable */
    RX_OP_CHECK_PROC    (0, 2, 1)   /* still processable */

    RX_OP_END
};

static const struct rx_test_op *const rx_test_scripts[] = {
    rx_script_1,
    rx_script_2,
    rx_script_3,
    rx_script_4
};

static void on_ack_deadline_callback(OSSL_TIME deadline,
                                     int pkt_space, void *arg)
{
    ((OSSL_TIME *)arg)[pkt_space] = deadline;
}

static int test_rx_ack_actual(int tidx, int space)
{
    int testresult = 0;
    struct helper h;
    const struct rx_test_op *script = rx_test_scripts[tidx], *s;
    size_t i, num_tx = 0, txi = 0;
    const OSSL_QUIC_FRAME_ACK *ack;
    OSSL_QUIC_FRAME_ACK rx_ack = {0};
    OSSL_QUIC_ACK_RANGE rx_ack_range = {0};
    struct pkt_info *pkts = NULL;
    OSSL_ACKM_TX_PKT *txs = NULL, *tx;
    OSSL_TIME ack_deadline[QUIC_PN_SPACE_NUM];
    size_t opn = 0;

    for (i = 0; i < QUIC_PN_SPACE_NUM; ++i)
        ack_deadline[i] = ossl_time_infinite();

    /* Initialise ACK manager. */
    if (!TEST_int_eq(helper_init(&h, 0), 1))
        goto err;

    /* Arm callback for testing. */
    ossl_ackm_set_ack_deadline_callback(h.ackm, on_ack_deadline_callback,
                                        ack_deadline);

    /*
     * Determine how many packets we are TXing, and therefore how many packet
     * structures we need.
     */
    for (s = script; s->kind != RX_OPK_END; ++s)
        if (s->kind == RX_OPK_TX)
            num_tx += s->num_pn;

    /* Allocate packet information structures. */
    txs = OPENSSL_zalloc(sizeof(*txs) * num_tx);
    if (!TEST_ptr(txs))
        goto err;

    pkts = OPENSSL_zalloc(sizeof(*pkts) * num_tx);
    if (!TEST_ptr(pkts))
        goto err;

    /* Run script. */
    for (s = script; s->kind != RX_OPK_END; ++s, ++opn) {
        fake_time = ossl_time_add(fake_time,
                                  ossl_ticks2time(s->time_advance));
        switch (s->kind) {
        case RX_OPK_PKT:
            for (i = 0; i < s->num_pn; ++i) {
                OSSL_ACKM_RX_PKT pkt = {0};

                pkt.pkt_num             = s->pn + i;
                pkt.time                = fake_time;
                pkt.pkt_space           = space;
                pkt.is_ack_eliciting    = 1;

                /* The packet should be processable before we feed it. */
                if (!TEST_int_eq(ossl_ackm_is_rx_pn_processable(h.ackm,
                                                                pkt.pkt_num,
                                                                pkt.pkt_space), 1))
                    goto err;

                if (!TEST_int_eq(ossl_ackm_on_rx_packet(h.ackm, &pkt), 1))
                    goto err;
            }

            break;

        case RX_OPK_CHECK_UNPROC:
        case RX_OPK_CHECK_PROC:
            for (i = 0; i < s->num_pn; ++i)
                if (!TEST_int_eq(ossl_ackm_is_rx_pn_processable(h.ackm,
                                                                s->pn + i, space),
                                 (s->kind == RX_OPK_CHECK_PROC)))
                    goto err;

            break;

        case RX_OPK_CHECK_STATE:
            if (!TEST_int_eq(ossl_ackm_is_ack_desired(h.ackm, space),
                             s->expect_desired))
                goto err;

            if (!TEST_int_eq(!ossl_time_is_infinite(ossl_ackm_get_ack_deadline(h.ackm, space))
                             && !ossl_time_is_zero(ossl_ackm_get_ack_deadline(h.ackm, space)),
                             s->expect_deadline))
                goto err;

            for (i = 0; i < QUIC_PN_SPACE_NUM; ++i) {
                if (i != (size_t)space
                        && !TEST_true(ossl_time_is_infinite(ossl_ackm_get_ack_deadline(h.ackm, i))))
                    goto err;

                if (!TEST_int_eq(ossl_time_compare(ossl_ackm_get_ack_deadline(h.ackm, i),
                                                   ack_deadline[i]), 0))
                    goto err;
            }

            break;

        case RX_OPK_CHECK_ACKS:
            ack = ossl_ackm_get_ack_frame(h.ackm, space);

            /* Should always be able to get an ACK frame. */
            if (!TEST_ptr(ack))
                goto err;

            if (!TEST_size_t_eq(ack->num_ack_ranges, s->num_ack_ranges))
                goto err;

            for (i = 0; i < ack->num_ack_ranges; ++i) {
                if (!TEST_uint64_t_eq(ack->ack_ranges[i].start,
                                      s->ack_ranges[i].start))
                    goto err;
                if (!TEST_uint64_t_eq(ack->ack_ranges[i].end,
                                      s->ack_ranges[i].end))
                    goto err;
            }

            break;

        case RX_OPK_TX:
            pkts[txi].pkt = tx = &txs[txi];

            tx->pkt_num             = s->pn;
            tx->pkt_space           = space;
            tx->num_bytes           = 123;
            tx->largest_acked       = s->largest_acked;
            tx->is_inflight         = 1;
            tx->is_ack_eliciting    = 1;
            tx->on_lost             = on_lost;
            tx->on_acked            = on_acked;
            tx->on_discarded        = on_discarded;
            tx->cb_arg              = &pkts[txi];
            tx->time                = fake_time;

            if (!TEST_int_eq(ossl_ackm_on_tx_packet(h.ackm, tx), 1))
                goto err;

            ++txi;
            break;

        case RX_OPK_RX_ACK:
            rx_ack.ack_ranges       = &rx_ack_range;
            rx_ack.num_ack_ranges   = 1;

            rx_ack_range.start      = s->pn;
            rx_ack_range.end        = s->pn + s->num_pn - 1;

            if (!TEST_int_eq(ossl_ackm_on_rx_ack_frame(h.ackm, &rx_ack,
                                                       space, fake_time), 1))
                goto err;

            break;

        case RX_OPK_SKIP_IF_PN_SPACE:
            if (space == (int)s->pn) {
                testresult = 1;
                goto err;
            }

            break;

        default:
            goto err;
        }
    }

    testresult = 1;
err:
    if (!testresult)
        TEST_error("error in ACKM RX script %d, op %zu", tidx + 1, opn + 1);

    helper_destroy(&h);
    OPENSSL_free(pkts);
    OPENSSL_free(txs);
    return testresult;
}

/*
 * Driver
 * ******************************************************************
 */
static int test_tx_ack_case(int idx)
{
    int tidx, space;

    tidx = idx % OSSL_NELEM(tx_ack_cases);
    idx /= OSSL_NELEM(tx_ack_cases);

    space = idx % QUIC_PN_SPACE_NUM;
    idx /= QUIC_PN_SPACE_NUM;

    return test_tx_ack_case_actual(tidx, space, idx);
}

static int test_rx_ack(int idx)
{
    int tidx;

    tidx = idx % OSSL_NELEM(rx_test_scripts);
    idx /= OSSL_NELEM(rx_test_scripts);

    return test_rx_ack_actual(tidx, idx);
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_tx_ack_case,
                  OSSL_NELEM(tx_ack_cases) * MODE_NUM * QUIC_PN_SPACE_NUM);
    ADD_ALL_TESTS(test_tx_ack_time_script, OSSL_NELEM(tx_ack_time_scripts));
    ADD_ALL_TESTS(test_rx_ack, OSSL_NELEM(rx_test_scripts) * QUIC_PN_SPACE_NUM);
    return 1;
}
