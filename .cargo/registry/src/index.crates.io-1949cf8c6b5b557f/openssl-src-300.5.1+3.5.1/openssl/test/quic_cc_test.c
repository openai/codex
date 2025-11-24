/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* For generating debug statistics during congestion controller development. */
/*#define GENERATE_LOG*/

#include "testutil.h"
#include <openssl/ssl.h>
#include "internal/quic_cc.h"
#include "internal/priority_queue.h"

/*
 * Time Simulation
 * ===============
 */
static OSSL_TIME fake_time = {0};

#define TIME_BASE (ossl_ticks2time(5 * OSSL_TIME_SECOND))

static OSSL_TIME fake_now(void *arg)
{
    return fake_time;
}

static void step_time(uint32_t ms)
{
    fake_time = ossl_time_add(fake_time, ossl_ms2time(ms));
}

/*
 * Network Simulation
 * ==================
 *
 * This is a simple 'network simulator' which emulates a network with a certain
 * bandwidth and latency. Sending a packet into the network causes it to consume
 * some capacity of the network until the packet exits the network. Note that
 * the capacity is not known to the congestion controller as the entire point of
 * a congestion controller is to correctly estimate this capacity and this is
 * what we are testing. The network simulator does take care of informing the
 * congestion controller of ack/loss events automatically but the caller is
 * responsible for querying the congestion controller and choosing the size of
 * simulated transmitted packets.
 */
typedef struct net_pkt_st {
    /*
     * The time at which the packet was sent.
     */
    OSSL_TIME   tx_time;

    /*
     * The time at which the simulated packet arrives at the RX side (success)
     * or is dropped (!success).
     */
    OSSL_TIME   arrive_time;

    /*
     * The time at which the transmitting side makes a determination of
     * acknowledgement (if success) or loss (if !success).
     */
    OSSL_TIME   determination_time;

    /*
     * Current earliest time there is something to be done for this packet.
     * min(arrive_time, determination_time).
     */
    OSSL_TIME   next_time;

    /* 1 if the packet will be successfully delivered, 0 if it is to be lost. */
    int         success;

    /* 1 if we have already processed packet arrival. */
    int         arrived;

    /* Size of simulated packet in bytes. */
    size_t      size;

    /* pqueue internal index. */
    size_t      idx;
} NET_PKT;

DEFINE_PRIORITY_QUEUE_OF(NET_PKT);

static int net_pkt_cmp(const NET_PKT *a, const NET_PKT *b)
{
    return ossl_time_compare(a->next_time, b->next_time);
}

struct net_sim {
    const OSSL_CC_METHOD *ccm;
    OSSL_CC_DATA         *cc;

    uint64_t capacity; /* bytes/s */
    uint64_t latency;  /* ms */

    uint64_t spare_capacity;
    PRIORITY_QUEUE_OF(NET_PKT) *pkts;

    uint64_t total_acked, total_lost; /* bytes */
};

static int net_sim_init(struct net_sim *s,
                        const OSSL_CC_METHOD *ccm, OSSL_CC_DATA *cc,
                        uint64_t capacity, uint64_t latency)
{
    s->ccm              = ccm;
    s->cc               = cc;

    s->capacity         = capacity;
    s->latency          = latency;

    s->spare_capacity   = capacity;

    s->total_acked      = 0;
    s->total_lost       = 0;

    if (!TEST_ptr(s->pkts = ossl_pqueue_NET_PKT_new(net_pkt_cmp)))
        return 0;

    return 1;
}

static void do_free(NET_PKT *pkt)
{
    OPENSSL_free(pkt);
}

static void net_sim_cleanup(struct net_sim *s)
{
    ossl_pqueue_NET_PKT_pop_free(s->pkts, do_free);
}

static int net_sim_process(struct net_sim *s, size_t skip_forward);

static int net_sim_send(struct net_sim *s, size_t sz)
{
    NET_PKT *pkt = OPENSSL_zalloc(sizeof(*pkt));
    int success;

    if (!TEST_ptr(pkt))
        return 0;

    /*
     * Ensure we have processed any events which have come due as these might
     * increase our spare capacity.
     */
    if (!TEST_true(net_sim_process(s, 0)))
        goto err;

    /* Do we have room for the packet in the network? */
    success = (sz <= s->spare_capacity);

    pkt->tx_time = fake_time;
    pkt->success = success;
    if (success) {
        /* This packet will arrive successfully after |latency| time. */
        pkt->arrive_time        = ossl_time_add(pkt->tx_time,
                                                ossl_ms2time(s->latency));
        /* Assume all received packets are acknowledged immediately. */
        pkt->determination_time = ossl_time_add(pkt->arrive_time,
                                                ossl_ms2time(s->latency));
        pkt->next_time          = pkt->arrive_time;
        s->spare_capacity      -= sz;
    } else {
        /*
         * In our network model, assume all packets are dropped due to a
         * bottleneck at the peer's NIC RX queue; thus dropping occurs after
         * |latency|.
         */
        pkt->arrive_time        = ossl_time_add(pkt->tx_time,
                                                ossl_ms2time(s->latency));
        /*
         * It will take longer to detect loss than to detect acknowledgement.
         */
        pkt->determination_time = ossl_time_add(pkt->tx_time,
                                                ossl_ms2time(3 * s->latency));
        pkt->next_time          = pkt->determination_time;
    }

    pkt->size = sz;

    if (!TEST_true(s->ccm->on_data_sent(s->cc, sz)))
        goto err;

    if (!TEST_true(ossl_pqueue_NET_PKT_push(s->pkts, pkt, &pkt->idx)))
        goto err;

    return 1;

err:
    OPENSSL_free(pkt);
    return 0;
}

static int net_sim_process_one(struct net_sim *s, int skip_forward)
{
    NET_PKT *pkt = ossl_pqueue_NET_PKT_peek(s->pkts);

    if (pkt == NULL)
        return 3;

    /* Jump forward to the next significant point in time. */
    if (skip_forward && ossl_time_compare(pkt->next_time, fake_time) > 0)
        fake_time = pkt->next_time;

    if (pkt->success && !pkt->arrived
        && ossl_time_compare(fake_time, pkt->arrive_time) >= 0) {
        /* Packet arrives */
        s->spare_capacity += pkt->size;
        pkt->arrived = 1;

        ossl_pqueue_NET_PKT_pop(s->pkts);
        pkt->next_time = pkt->determination_time;
        if (!ossl_pqueue_NET_PKT_push(s->pkts, pkt, &pkt->idx))
            return 0;

        return 1;
    }

    if (ossl_time_compare(fake_time, pkt->determination_time) < 0)
        return 2;

    if (!TEST_true(!pkt->success || pkt->arrived))
        return 0;

    if (!pkt->success) {
        OSSL_CC_LOSS_INFO loss_info = {0};

        loss_info.tx_time = pkt->tx_time;
        loss_info.tx_size = pkt->size;

        if (!TEST_true(s->ccm->on_data_lost(s->cc, &loss_info)))
            return 0;

        if (!TEST_true(s->ccm->on_data_lost_finished(s->cc, 0)))
            return 0;

        s->total_lost += pkt->size;
        ossl_pqueue_NET_PKT_pop(s->pkts);
        OPENSSL_free(pkt);
    } else {
        OSSL_CC_ACK_INFO ack_info = {0};

        ack_info.tx_time = pkt->tx_time;
        ack_info.tx_size = pkt->size;

        if (!TEST_true(s->ccm->on_data_acked(s->cc, &ack_info)))
            return 0;

        s->total_acked += pkt->size;
        ossl_pqueue_NET_PKT_pop(s->pkts);
        OPENSSL_free(pkt);
    }

    return 1;
}

static int net_sim_process(struct net_sim *s, size_t skip_forward)
{
    int rc;

    while ((rc = net_sim_process_one(s, skip_forward > 0 ? 1 : 0)) == 1)
        if (skip_forward > 0)
            --skip_forward;

    return rc;
}

/*
 * State Dumping Utilities
 * =======================
 *
 * Utilities for outputting CC state information.
 */
#ifdef GENERATE_LOG
static FILE *logfile;
#endif

static int dump_state(const OSSL_CC_METHOD *ccm, OSSL_CC_DATA *cc,
                                  struct net_sim *s)
{
#ifdef GENERATE_LOG
    uint64_t cwnd_size, cur_bytes, state;

    if (logfile == NULL)
        return 1;

    if (!TEST_true(ccm->get_option_uint(cc, OSSL_CC_OPTION_CUR_CWND_SIZE,
                                        &cwnd_size)))
        return 0;

    if (!TEST_true(ccm->get_option_uint(cc, OSSL_CC_OPTION_CUR_BYTES_IN_FLIGHT,
                                        &cur_bytes)))
        return 0;

    if (!TEST_true(ccm->get_option_uint(cc, OSSL_CC_OPTION_CUR_STATE,
                                        &state)))
        return 0;

    fprintf(logfile, "%10lu,%10lu,%10lu,%10lu,%10lu,%10lu,%10lu,%10lu,\"%c\"\n",
            ossl_time2ms(fake_time),
            ccm->get_tx_allowance(cc),
            cwnd_size,
            cur_bytes,
            s->total_acked,
            s->total_lost,
            s->capacity,
            s->spare_capacity,
            (char)state);
#endif

    return 1;
}

/*
 * Simulation Test
 * ===============
 *
 * Simulator-based unit test in which we simulate a network with a certain
 * capacity. The average estimated channel capacity should not be too far from
 * the actual channel capacity.
 */
static int test_simulate(void)
{
    int testresult = 0;
    int rc;
    int have_sim = 0;
    const OSSL_CC_METHOD *ccm = &ossl_cc_newreno_method;
    OSSL_CC_DATA *cc = NULL;
    size_t mdpl = 1472;
    uint64_t total_sent = 0, total_to_send, allowance;
    uint64_t actual_capacity = 16000; /* B/s - 128kb/s */
    uint64_t cwnd_sample_sum = 0, cwnd_sample_count = 0;
    uint64_t diag_cur_bytes_in_flight = UINT64_MAX;
    uint64_t diag_cur_cwnd_size = UINT64_MAX;
    struct net_sim sim;
    OSSL_PARAM params[3], *p = params;

    fake_time = TIME_BASE;

    if (!TEST_ptr(cc = ccm->new(fake_now, NULL)))
        goto err;

    if (!TEST_true(net_sim_init(&sim, ccm, cc, actual_capacity, 100)))
        goto err;

    have_sim = 1;

    *p++ = OSSL_PARAM_construct_size_t(OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN,
                                       &mdpl);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_true(ccm->set_input_params(cc, params)))
        goto err;

    p = params;
    *p++ = OSSL_PARAM_construct_uint64(OSSL_CC_OPTION_CUR_BYTES_IN_FLIGHT,
                                       &diag_cur_bytes_in_flight);
    *p++ = OSSL_PARAM_construct_uint64(OSSL_CC_OPTION_CUR_CWND_SIZE,
                                       &diag_cur_cwnd_size);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_true(ccm->bind_diagnostics(cc, params)))
        goto err;

    ccm->reset(cc);

    if (!TEST_uint64_t_ge(allowance = ccm->get_tx_allowance(cc), mdpl))
        goto err;

    /*
     * Start generating traffic. Stop when we've sent 30 MiB.
     */
    total_to_send = 30 * 1024 * 1024;

    while (total_sent < total_to_send) {
        /*
         * Assume we are bottlenecked by the network (which is the interesting
         * case for testing a congestion controller) and always fill our entire
         * TX allowance as and when it becomes available.
         */
        for (;;) {
            uint64_t sz;

            dump_state(ccm, cc, &sim);

            allowance = ccm->get_tx_allowance(cc);
            sz = allowance > mdpl ? mdpl : allowance;
            if (sz > SIZE_MAX)
                sz = SIZE_MAX;

            /*
             * QUIC minimum packet sizes, etc. mean that in practice we will not
             * consume the allowance exactly, so only send above a certain size.
             */
            if (sz < 30)
                break;

            step_time(7);

            if (!TEST_true(net_sim_send(&sim, (size_t)sz)))
                goto err;

            total_sent += sz;
        }

        /* Skip to next event. */
        rc = net_sim_process(&sim, 1);
        if (!TEST_int_gt(rc, 0))
            goto err;

        /*
         * If we are out of any events to handle at all we definitely should
         * have at least one MDPL's worth of allowance as nothing is in flight.
         */
        if (rc == 3) {
            if (!TEST_uint64_t_eq(diag_cur_bytes_in_flight, 0))
                goto err;

            if (!TEST_uint64_t_ge(ccm->get_tx_allowance(cc), mdpl))
                goto err;
        }

        /* Update our average of the estimated channel capacity. */
        {
            uint64_t v = 1;

            if (!TEST_uint64_t_ne(diag_cur_bytes_in_flight, UINT64_MAX)
                || !TEST_uint64_t_ne(diag_cur_cwnd_size, UINT64_MAX))
                goto err;

            cwnd_sample_sum += v;
            ++cwnd_sample_count;
        }
    }

    /*
     * Ensure estimated channel capacity is not too far off from actual channel
     * capacity.
     */
    {
        uint64_t estimated_capacity = cwnd_sample_sum / cwnd_sample_count;

        double error = ((double)estimated_capacity / (double)actual_capacity) - 1.0;

        TEST_info("est = %6llu kB/s, act=%6llu kB/s (error=%.02f%%)\n",
                  (unsigned long long)estimated_capacity,
                  (unsigned long long)actual_capacity,
                  error * 100.0);

        /* Max 5% error */
        if (!TEST_double_le(error, 0.05))
            goto err;
    }

    testresult = 1;
err:
    if (have_sim)
        net_sim_cleanup(&sim);

    if (cc != NULL)
        ccm->free(cc);

#ifdef GENERATE_LOG
    if (logfile != NULL)
        fflush(logfile);
#endif

    return testresult;
}

/*
 * Sanity Test
 * ===========
 *
 * Basic test of the congestion control APIs.
 */
static int test_sanity(void)
{
    int testresult = 0;
    OSSL_CC_DATA *cc = NULL;
    const OSSL_CC_METHOD *ccm = &ossl_cc_newreno_method;
    OSSL_CC_LOSS_INFO loss_info = {0};
    OSSL_CC_ACK_INFO ack_info = {0};
    uint64_t allowance, allowance2;
    OSSL_PARAM params[3], *p = params;
    size_t mdpl = 1472, diag_mdpl = SIZE_MAX;
    uint64_t diag_cur_bytes_in_flight = UINT64_MAX;

    fake_time = TIME_BASE;

    if (!TEST_ptr(cc = ccm->new(fake_now, NULL)))
        goto err;

    /* Test configuration of options. */
    *p++ = OSSL_PARAM_construct_size_t(OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN,
                                       &mdpl);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_true(ccm->set_input_params(cc, params)))
        goto err;

    ccm->reset(cc);

    p = params;
    *p++ = OSSL_PARAM_construct_size_t(OSSL_CC_OPTION_MAX_DGRAM_PAYLOAD_LEN,
                                       &diag_mdpl);
    *p++ = OSSL_PARAM_construct_uint64(OSSL_CC_OPTION_CUR_BYTES_IN_FLIGHT,
                                       &diag_cur_bytes_in_flight);
    *p++ = OSSL_PARAM_construct_end();

    if (!TEST_true(ccm->bind_diagnostics(cc, params))
        || !TEST_size_t_eq(diag_mdpl, 1472))
        goto err;

    if (!TEST_uint64_t_ge(allowance = ccm->get_tx_allowance(cc), 1472))
        goto err;

    /* There is TX allowance so wakeup should be immediate */
    if (!TEST_true(ossl_time_is_zero(ccm->get_wakeup_deadline(cc))))
        goto err;

    /* No bytes should currently be in flight. */
    if (!TEST_uint64_t_eq(diag_cur_bytes_in_flight, 0))
        goto err;

    /* Tell the CC we have sent some data. */
    if (!TEST_true(ccm->on_data_sent(cc, 1200)))
        goto err;

    /* Allowance should have decreased. */
    if (!TEST_uint64_t_eq(ccm->get_tx_allowance(cc), allowance - 1200))
        goto err;

    /* Acknowledge the data. */
    ack_info.tx_time = fake_time;
    ack_info.tx_size = 1200;
    step_time(100);
    if (!TEST_true(ccm->on_data_acked(cc, &ack_info)))
        goto err;

    /* Allowance should have returned. */
    if (!TEST_uint64_t_ge(allowance2 = ccm->get_tx_allowance(cc), allowance))
        goto err;

    /* Test invalidation. */
    if (!TEST_true(ccm->on_data_sent(cc, 1200)))
        goto err;

    /* Allowance should have decreased. */
    if (!TEST_uint64_t_eq(ccm->get_tx_allowance(cc), allowance - 1200))
        goto err;

    if (!TEST_true(ccm->on_data_invalidated(cc, 1200)))
        goto err;

    /* Allowance should have returned. */
    if (!TEST_uint64_t_eq(ccm->get_tx_allowance(cc), allowance2))
        goto err;

    /* Test loss. */
    if (!TEST_uint64_t_ge(allowance = ccm->get_tx_allowance(cc), 1200 + 1300))
        goto err;

    if (!TEST_true(ccm->on_data_sent(cc, 1200)))
        goto err;

    if (!TEST_true(ccm->on_data_sent(cc, 1300)))
        goto err;

    if (!TEST_uint64_t_eq(allowance2 = ccm->get_tx_allowance(cc),
                          allowance - 1200 - 1300))
        goto err;

    loss_info.tx_time = fake_time;
    loss_info.tx_size = 1200;
    step_time(100);

    if (!TEST_true(ccm->on_data_lost(cc, &loss_info)))
        goto err;

    loss_info.tx_size = 1300;
    if (!TEST_true(ccm->on_data_lost(cc, &loss_info)))
        goto err;

    if (!TEST_true(ccm->on_data_lost_finished(cc, 0)))
        goto err;

    /* Allowance should have changed due to the lost calls */
    if (!TEST_uint64_t_ne(ccm->get_tx_allowance(cc), allowance2))
        goto err;

    /* But it should not be as high as the original value */
    if (!TEST_uint64_t_lt(ccm->get_tx_allowance(cc), allowance))
        goto err;

    testresult = 1;

err:
    if (cc != NULL)
        ccm->free(cc);

    return testresult;
}

int setup_tests(void)
{

#ifdef GENERATE_LOG
    logfile = fopen("quic_cc_stats.csv", "w");
    fprintf(logfile,
        "\"Time\","
        "\"TX Allowance\","
        "\"CWND Size\","
        "\"Bytes in Flight\","
        "\"Total Acked\",\"Total Lost\","
        "\"Capacity\",\"Spare Capacity\","
        "\"State\"\n");
#endif

    ADD_TEST(test_simulate);
    ADD_TEST(test_sanity);
    return 1;
}
