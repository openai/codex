/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_ackm.h"
#include "internal/uint_set.h"
#include "internal/common.h"
#include <assert.h>

DEFINE_LIST_OF(tx_history, OSSL_ACKM_TX_PKT);

/*
 * TX Packet History
 * *****************
 *
 * The TX Packet History object tracks information about packets which have been
 * sent for which we later expect to receive an ACK. It is essentially a simple
 * database keeping a list of packet information structures in packet number
 * order which can also be looked up directly by packet number.
 *
 * We currently only allow packets to be appended to the list (i.e. the packet
 * numbers of the packets appended to the list must monotonically increase), as
 * we should not currently need more general functionality such as a sorted list
 * insert.
 */
struct tx_pkt_history_st {
    /* A linked list of all our packets. */
    OSSL_LIST(tx_history) packets;

    /*
     * Mapping from packet numbers (uint64_t) to (OSSL_ACKM_TX_PKT *)
     *
     * Invariant: A packet is in this map if and only if it is in the linked
     *            list.
     */
    LHASH_OF(OSSL_ACKM_TX_PKT) *map;

    /*
     * The lowest packet number which may currently be added to the history list
     * (inclusive). We do not allow packet numbers to be added to the history
     * list non-monotonically, so packet numbers must be greater than or equal
     * to this value.
     */
    uint64_t watermark;

    /*
     * Packet number of the highest packet info structure we have yet appended
     * to the list. This is usually one less than watermark, except when we have
     * not added any packet yet.
     */
    uint64_t highest_sent;
};

DEFINE_LHASH_OF_EX(OSSL_ACKM_TX_PKT);

static unsigned long tx_pkt_info_hash(const OSSL_ACKM_TX_PKT *pkt)
{
    /* Using low bits of the packet number as the hash should be enough */
    return (unsigned long)pkt->pkt_num;
}

static int tx_pkt_info_compare(const OSSL_ACKM_TX_PKT *a,
                               const OSSL_ACKM_TX_PKT *b)
{
    if (a->pkt_num < b->pkt_num)
        return -1;
    if (a->pkt_num > b->pkt_num)
        return 1;
    return 0;
}

static int
tx_pkt_history_init(struct tx_pkt_history_st *h)
{
    ossl_list_tx_history_init(&h->packets);
    h->watermark    = 0;
    h->highest_sent = 0;

    h->map = lh_OSSL_ACKM_TX_PKT_new(tx_pkt_info_hash, tx_pkt_info_compare);
    if (h->map == NULL)
        return 0;

    return 1;
}

static void
tx_pkt_history_destroy(struct tx_pkt_history_st *h)
{
    lh_OSSL_ACKM_TX_PKT_free(h->map);
    h->map = NULL;
    ossl_list_tx_history_init(&h->packets);
}

static int
tx_pkt_history_add_actual(struct tx_pkt_history_st *h,
                          OSSL_ACKM_TX_PKT *pkt)
{
    OSSL_ACKM_TX_PKT *existing;

    /*
     * There should not be any existing packet with this number
     * in our mapping.
     */
    existing = lh_OSSL_ACKM_TX_PKT_retrieve(h->map, pkt);
    if (!ossl_assert(existing == NULL))
        return 0;

    /* Should not already be in a list. */
    if (!ossl_assert(ossl_list_tx_history_next(pkt) == NULL
            && ossl_list_tx_history_prev(pkt) == NULL))
        return 0;

    lh_OSSL_ACKM_TX_PKT_insert(h->map, pkt);

    ossl_list_tx_history_insert_tail(&h->packets, pkt);
    return 1;
}

/* Adds a packet information structure to the history list. */
static int
tx_pkt_history_add(struct tx_pkt_history_st *h,
                   OSSL_ACKM_TX_PKT *pkt)
{
    if (!ossl_assert(pkt->pkt_num >= h->watermark))
        return 0;

    if (tx_pkt_history_add_actual(h, pkt) < 1)
        return 0;

    h->watermark    = pkt->pkt_num + 1;
    h->highest_sent = pkt->pkt_num;
    return 1;
}

/* Retrieve a packet information structure by packet number. */
static OSSL_ACKM_TX_PKT *
tx_pkt_history_by_pkt_num(struct tx_pkt_history_st *h, uint64_t pkt_num)
{
    OSSL_ACKM_TX_PKT key;

    key.pkt_num = pkt_num;

    return lh_OSSL_ACKM_TX_PKT_retrieve(h->map, &key);
}

/* Remove a packet information structure from the history log. */
static int
tx_pkt_history_remove(struct tx_pkt_history_st *h, uint64_t pkt_num)
{
    OSSL_ACKM_TX_PKT key, *pkt;
    key.pkt_num = pkt_num;

    pkt = tx_pkt_history_by_pkt_num(h, pkt_num);
    if (pkt == NULL)
        return 0;

    ossl_list_tx_history_remove(&h->packets, pkt);
    lh_OSSL_ACKM_TX_PKT_delete(h->map, &key);
    return 1;
}

/*
 * RX Packet Number Tracking
 * *************************
 *
 * **Background.** The RX side of the ACK manager must track packets we have
 * received for which we have to generate ACK frames. Broadly, this means we
 * store a set of packet numbers which we have received but which we do not know
 * for a fact that the transmitter knows we have received.
 *
 * This must handle various situations:
 *
 *   1. We receive a packet but have not sent an ACK yet, so the transmitter
 *      does not know whether we have received it or not yet.
 *
 *   2. We receive a packet and send an ACK which is lost. We do not
 *      immediately know that the ACK was lost and the transmitter does not know
 *      that we have received the packet.
 *
 *   3. We receive a packet and send an ACK which is received by the
 *      transmitter. The transmitter does not immediately respond with an ACK,
 *      or responds with an ACK which is lost. The transmitter knows that we
 *      have received the packet, but we do not know for sure that it knows,
 *      because the ACK we sent could have been lost.
 *
 *   4. We receive a packet and send an ACK which is received by the
 *      transmitter. The transmitter subsequently sends us an ACK which confirms
 *      its receipt of the ACK we sent, and we successfully receive that ACK, so
 *      we know that the transmitter knows, that we received the original
 *      packet.
 *
 * Only when we reach case (4) are we relieved of any need to track a given
 * packet number we have received, because only in this case do we know for sure
 * that the peer knows we have received the packet. Having reached case (4) we
 * will never again need to generate an ACK containing the PN in question, but
 * until we reach that point, we must keep track of the PN as not having been
 * provably ACKed, as we may have to keep generating ACKs for the given PN not
 * just until the transmitter receives one, but until we know that it has
 * received one. This will be referred to herein as "provably ACKed".
 *
 * **Duplicate handling.** The above discusses the case where we have received a
 * packet with a given PN but are at best unsure whether the sender knows we
 * have received it or not. However, we must also handle the case where we have
 * yet to receive a packet with a given PN in the first place. The reason for
 * this is because of the requirement expressed by RFC 9000 s. 12.3:
 *
 *   "A receiver MUST discard a newly unprotected packet unless it is certain
 *    that it has not processed another packet with the same packet number from
 *    the same packet number space."
 *
 * We must ensure we never process a duplicate PN. As such, each possible PN we
 * can receive must exist in one of the following logical states:
 *
 *   - We have never processed this PN before
 *     (so if we receive such a PN, it can be processed)
 *
 *   - We have processed this PN but it has not yet been provably ACKed
 *     (and should therefore be in any future ACK frame generated;
 *      if we receive such a PN again, it must be ignored)
 *
 *   - We have processed this PN and it has been provably ACKed
 *     (if we receive such a PN again, it must be ignored)
 *
 * However, if we were to track this state for every PN ever used in the history
 * of a connection, the amount of state required would increase unboundedly as
 * the connection goes on (for example, we would have to store a set of every PN
 * ever received.)
 *
 * RFC 9000 s. 12.3 continues:
 *
 *   "Endpoints that track all individual packets for the purposes of detecting
 *    duplicates are at risk of accumulating excessive state. The data required
 *    for detecting duplicates can be limited by maintaining a minimum packet
 *    number below which all packets are immediately dropped."
 *
 * Moreover, RFC 9000 s. 13.2.3 states that:
 *
 *   "A receiver MUST retain an ACK Range unless it can ensure that it will not
 *    subsequently accept packets with numbers in that range. Maintaining a
 *    minimum packet number that increases as ranges are discarded is one way to
 *    achieve this with minimal state."
 *
 * This touches on a subtlety of the original requirement quoted above: the
 * receiver MUST discard a packet unless it is certain that it has not processed
 * another packet with the same PN. However, this does not forbid the receiver
 * from also discarding some PNs even though it has not yet processed them. In
 * other words, implementations must be conservative and err in the direction of
 * assuming a packet is a duplicate, but it is acceptable for this to come at
 * the cost of falsely identifying some packets as duplicates.
 *
 * This allows us to bound the amount of state we must keep, and we adopt the
 * suggested strategy quoted above to do so. We define a watermark PN below
 * which all PNs are in the same state. This watermark is only ever increased.
 * Thus the PNs the state for which needs to be explicitly tracked is limited to
 * only a small number of recent PNs, and all older PNs have an assumed state.
 *
 * Any given PN thus falls into one of the following states:
 *
 *   - (A) The PN is above the watermark but we have not yet received it.
 *
 *         If we receive such a PN, we should process it and record the PN as
 *         received.
 *
 *   - (B) The PN is above the watermark and we have received it.
 *
 *         The PN should be included in any future ACK frame we generate.
 *         If we receive such a PN again, we should ignore it.
 *
 *   - (C) The PN is below the watermark.
 *
 *         We do not know whether a packet with the given PN was received or
 *         not. To be safe, if we receive such a packet, it is not processed.
 *
 * Note that state (C) corresponds to both "we have processed this PN and it has
 * been provably ACKed" logical state and a subset of the PNs in the "we have
 * never processed this PN before" logical state (namely all PNs which were lost
 * and never received, but which are not recent enough to be above the
 * watermark). The reason we can merge these states and avoid tracking states
 * for the PNs in this state is because the provably ACKed and never-received
 * states are functionally identical in terms of how we need to handle them: we
 * don't need to do anything for PNs in either of these states, so we don't have
 * to care about PNs in this state nor do we have to care about distinguishing
 * the two states for a given PN.
 *
 * Note that under this scheme provably ACKed PNs are by definition always below
 * the watermark; therefore, it follows that when a PN becomes provably ACKed,
 * the watermark must be immediately increased to exceed it (otherwise we would
 * keep reporting it in future ACK frames).
 *
 * This is in line with RFC 9000 s. 13.2.4's suggested strategy on when
 * to advance the watermark:
 *
 *   "When a packet containing an ACK frame is sent, the Largest Acknowledged
 *    field in that frame can be saved. When a packet containing an ACK frame is
 *    acknowledged, the receiver can stop acknowledging packets less than or
 *    equal to the Largest Acknowledged field in the sent ACK frame."
 *
 * This is where our scheme's false positives arise. When a packet containing an
 * ACK frame is itself ACK'd, PNs referenced in that ACK frame become provably
 * acked, and the watermark is bumped accordingly. However, the Largest
 * Acknowledged field does not imply that all lower PNs have been received,
 * because there may be gaps expressed in the ranges of PNs expressed by that
 * and previous ACK frames. Thus, some unreceived PNs may be moved below the
 * watermark, and we may subsequently reject those PNs as possibly being
 * duplicates even though we have not actually received those PNs. Since we bump
 * the watermark when a PN becomes provably ACKed, it follows that an unreceived
 * PN falls below the watermark (and thus becomes a false positive for the
 * purposes of duplicate detection) when a higher-numbered PN becomes provably
 * ACKed.
 *
 * Thus, when PN n becomes provably acked, any unreceived PNs in the range [0,
 * n) will no longer be processed. Although datagrams may be reordered in the
 * network, a PN we receive can only become provably ACKed after our own
 * subsequently generated ACK frame is sent in a future TX packet, and then we
 * receive another RX PN acknowledging that TX packet. This means that a given RX
 * PN can only become provably ACKed at least 1 RTT after it is received; it is
 * unlikely that any reordered datagrams will still be "in the network" (and not
 * lost) by this time. If this does occur for whatever reason and a late PN is
 * received, the packet will be discarded unprocessed and the PN is simply
 * handled as though lost (a "written off" PN).
 *
 * **Data structure.** Our state for the RX handling side of the ACK manager, as
 * discussed above, mainly comprises:
 *
 *   a) a logical set of PNs, and
 *   b) a monotonically increasing PN counter (the watermark).
 *
 * For (a), we define a data structure which stores a logical set of PNs, which
 * we use to keep track of which PNs we have received but which have not yet
 * been provably ACKed, and thus will later need to generate an ACK frame for.
 *
 * The correspondence with the logical states discussed above is as follows. A
 * PN is in state (C) if it is below the watermark; otherwise it is in state (B)
 * if it is in the logical set of PNs, and in state (A) otherwise.
 *
 * Note that PNs are only removed from the PN set (when they become provably
 * ACKed or written off) by virtue of advancement of the watermark. Removing PNs
 * from the PN set any other way would be ambiguous as it would be
 * indistinguishable from a PN we have not yet received and risk us processing a
 * duplicate packet. In other words, for a given PN:
 *
 *   - State (A) can transition to state (B) or (C)
 *   - State (B) can transition to state (C) only
 *   - State (C) is the terminal state
 *
 * We can query the logical set data structure for PNs which have been received
 * but which have not been provably ACKed when we want to generate ACK frames.
 * Since ACK frames can be lost and/or we might not know that the peer has
 * successfully received them, we might generate multiple ACK frames covering a
 * given PN until that PN becomes provably ACKed and we finally remove it from
 * our set (by bumping the watermark) as no longer being our concern.
 *
 * The data structure used is the UINT_SET structure defined in uint_set.h,
 * which is used as a PN set. We use the following operations of the structure:
 *
 *   Insert Range: Used when we receive a new PN.
 *
 *   Remove Range: Used when bumping the watermark.
 *
 *   Query:        Used to determine if a PN is in the set.
 *
 * **Possible duplicates.** A PN is considered a possible duplicate when either:
 *
 *  a) its PN is already in the PN set (i.e. has already been received), or
 *  b) its PN is below the watermark (i.e. was provably ACKed or written off).
 *
 * A packet with a given PN is considered 'processable' when that PN is not
 * considered a possible duplicate (see ossl_ackm_is_rx_pn_processable).
 *
 * **TX/RX interaction.** The watermark is bumped whenever an RX packet becomes
 * provably ACKed. This occurs when an ACK frame is received by the TX side of
 * the ACK manager; thus, there is necessary interaction between the TX and RX
 * sides of the ACK manager.
 *
 * This is implemented as follows. When a packet is queued as sent in the TX
 * side of the ACK manager, it may optionally have a Largest Acked value set on
 * it. The user of the ACK manager should do this if the packet being
 * transmitted contains an ACK frame, by setting the field to the Largest Acked
 * field of that frame. Otherwise, this field should be set to QUIC_PN_INVALID.
 * When a TX packet is eventually acknowledged which has this field set, it is
 * used to update the state of the RX side of the ACK manager by bumping the
 * watermark accordingly.
 */
struct rx_pkt_history_st {
    UINT_SET set;

    /*
     * Invariant: PNs below this are not in the set.
     * Invariant: This is monotonic and only ever increases.
     */
    QUIC_PN watermark;
};

static int rx_pkt_history_bump_watermark(struct rx_pkt_history_st *h,
                                         QUIC_PN watermark);

static void rx_pkt_history_init(struct rx_pkt_history_st *h)
{
    ossl_uint_set_init(&h->set);
    h->watermark = 0;
}

static void rx_pkt_history_destroy(struct rx_pkt_history_st *h)
{
    ossl_uint_set_destroy(&h->set);
}

/*
 * Limit the number of ACK ranges we store to prevent resource consumption DoS
 * attacks.
 */
#define MAX_RX_ACK_RANGES   32

static void rx_pkt_history_trim_range_count(struct rx_pkt_history_st *h)
{
    QUIC_PN highest = QUIC_PN_INVALID;

    while (ossl_list_uint_set_num(&h->set) > MAX_RX_ACK_RANGES) {
        UINT_RANGE r = ossl_list_uint_set_head(&h->set)->range;

        highest = (highest == QUIC_PN_INVALID)
            ? r.end : ossl_quic_pn_max(highest, r.end);

        ossl_uint_set_remove(&h->set, &r);
    }

    /*
     * Bump watermark to cover all PNs we removed to avoid accidental
     * reprocessing of packets.
     */
    if (highest != QUIC_PN_INVALID)
        rx_pkt_history_bump_watermark(h, highest + 1);
}

static int rx_pkt_history_add_pn(struct rx_pkt_history_st *h,
                                 QUIC_PN pn)
{
    UINT_RANGE r;

    r.start = pn;
    r.end   = pn;

    if (pn < h->watermark)
        return 1; /* consider this a success case */

    if (ossl_uint_set_insert(&h->set, &r) != 1)
        return 0;

    rx_pkt_history_trim_range_count(h);
    return 1;
}

static int rx_pkt_history_bump_watermark(struct rx_pkt_history_st *h,
                                         QUIC_PN watermark)
{
    UINT_RANGE r;

    if (watermark <= h->watermark)
        return 1;

    /* Remove existing PNs below the watermark. */
    r.start = 0;
    r.end   = watermark - 1;
    if (ossl_uint_set_remove(&h->set, &r) != 1)
        return 0;

    h->watermark = watermark;
    return 1;
}

/*
 * ACK Manager Implementation
 * **************************
 * Implementation of the ACK manager proper.
 */

/* Constants used by the ACK manager; see RFC 9002. */
#define K_GRANULARITY           (1 * OSSL_TIME_MS)
#define K_PKT_THRESHOLD         3
#define K_TIME_THRESHOLD_NUM    9
#define K_TIME_THRESHOLD_DEN    8

/* The maximum number of times we allow PTO to be doubled. */
#define MAX_PTO_COUNT          16

/* Default maximum amount of time to leave an ACK-eliciting packet un-ACK'd. */
#define DEFAULT_TX_MAX_ACK_DELAY       ossl_ms2time(QUIC_DEFAULT_MAX_ACK_DELAY)

struct ossl_ackm_st {
    /* Our list of transmitted packets. Corresponds to RFC 9002 sent_packets. */
    struct tx_pkt_history_st tx_history[QUIC_PN_SPACE_NUM];

    /* Our list of received PNs which are not yet provably acked. */
    struct rx_pkt_history_st rx_history[QUIC_PN_SPACE_NUM];

    /* Polymorphic dependencies that we consume. */
    OSSL_TIME             (*now)(void *arg);
    void                   *now_arg;
    OSSL_STATM             *statm;
    const OSSL_CC_METHOD   *cc_method;
    OSSL_CC_DATA           *cc_data;

    /* RFC 9002 variables. */
    uint32_t        pto_count;
    QUIC_PN         largest_acked_pkt[QUIC_PN_SPACE_NUM];
    OSSL_TIME       time_of_last_ack_eliciting_pkt[QUIC_PN_SPACE_NUM];
    OSSL_TIME       loss_time[QUIC_PN_SPACE_NUM];
    OSSL_TIME       loss_detection_deadline;

    /* Lowest PN which is still not known to be ACKed. */
    QUIC_PN         lowest_unacked_pkt[QUIC_PN_SPACE_NUM];

    /* Time at which we got our first RTT sample, or 0. */
    OSSL_TIME       first_rtt_sample;

    /*
     * A packet's num_bytes are added to this if it is inflight,
     * and removed again once ack'd/lost/discarded.
     */
    uint64_t        bytes_in_flight;

    /*
     * A packet's num_bytes are added to this if it is both inflight and
     * ack-eliciting, and removed again once ack'd/lost/discarded.
     */
    uint64_t        ack_eliciting_bytes_in_flight[QUIC_PN_SPACE_NUM];

    /* Count of ECN-CE events. */
    uint64_t        peer_ecnce[QUIC_PN_SPACE_NUM];

    /* Set to 1 when the handshake is confirmed. */
    char            handshake_confirmed;

    /* Set to 1 when the peer has completed address validation. */
    char            peer_completed_addr_validation;

    /* Set to 1 when a PN space has been discarded. */
    char            discarded[QUIC_PN_SPACE_NUM];

    /* Set to 1 when we think an ACK frame should be generated. */
    char            rx_ack_desired[QUIC_PN_SPACE_NUM];

    /* Set to 1 if an ACK frame has ever been generated. */
    char            rx_ack_generated[QUIC_PN_SPACE_NUM];

    /* Probe request counts for reporting to the user. */
    OSSL_ACKM_PROBE_INFO    pending_probe;

    /* Generated ACK frames for each PN space. */
    OSSL_QUIC_FRAME_ACK     ack[QUIC_PN_SPACE_NUM];
    OSSL_QUIC_ACK_RANGE     ack_ranges[QUIC_PN_SPACE_NUM][MAX_RX_ACK_RANGES];

    /* Other RX state. */
    /* Largest PN we have RX'd. */
    QUIC_PN         rx_largest_pn[QUIC_PN_SPACE_NUM];

    /* Time at which the PN in rx_largest_pn was RX'd. */
    OSSL_TIME       rx_largest_time[QUIC_PN_SPACE_NUM];

    /*
     * ECN event counters. Each time we receive a packet with a given ECN label,
     * the corresponding ECN counter here is incremented.
     */
    uint64_t        rx_ect0[QUIC_PN_SPACE_NUM];
    uint64_t        rx_ect1[QUIC_PN_SPACE_NUM];
    uint64_t        rx_ecnce[QUIC_PN_SPACE_NUM];

    /*
     * Number of ACK-eliciting packets since last ACK. We use this to defer
     * emitting ACK frames until a threshold number of ACK-eliciting packets
     * have been received.
     */
    uint32_t        rx_ack_eliciting_pkts_since_last_ack[QUIC_PN_SPACE_NUM];

    /*
     * The ACK frame coalescing deadline at which we should flush any unsent ACK
     * frames.
     */
    OSSL_TIME       rx_ack_flush_deadline[QUIC_PN_SPACE_NUM];

    /*
     * The RX maximum ACK delay (the maximum amount of time our peer might
     * wait to send us an ACK after receiving an ACK-eliciting packet).
     */
    OSSL_TIME       rx_max_ack_delay;

    /*
     * The TX maximum ACK delay (the maximum amount of time we allow ourselves
     * to wait before generating an ACK after receiving an ACK-eliciting
     * packet).
     */
    OSSL_TIME       tx_max_ack_delay;

    /* Callbacks for deadline updates. */
    void (*loss_detection_deadline_cb)(OSSL_TIME deadline, void *arg);
    void *loss_detection_deadline_cb_arg;

    void (*ack_deadline_cb)(OSSL_TIME deadline, int pkt_space, void *arg);
    void *ack_deadline_cb_arg;
};

static ossl_inline uint32_t min_u32(uint32_t x, uint32_t y)
{
    return x < y ? x : y;
}

/*
 * Get TX history for a given packet number space. Must not have been
 * discarded.
 */
static struct tx_pkt_history_st *get_tx_history(OSSL_ACKM *ackm, int pkt_space)
{
    assert(!ackm->discarded[pkt_space]);

    return &ackm->tx_history[pkt_space];
}

/*
 * Get RX history for a given packet number space. Must not have been
 * discarded.
 */
static struct rx_pkt_history_st *get_rx_history(OSSL_ACKM *ackm, int pkt_space)
{
    assert(!ackm->discarded[pkt_space]);

    return &ackm->rx_history[pkt_space];
}

/* Does the newly-acknowledged list contain any ack-eliciting packet? */
static int ack_includes_ack_eliciting(OSSL_ACKM_TX_PKT *pkt)
{
    for (; pkt != NULL; pkt = pkt->anext)
        if (pkt->is_ack_eliciting)
            return 1;

    return 0;
}

/* Return number of ACK-eliciting bytes in flight across all PN spaces. */
static uint64_t ackm_ack_eliciting_bytes_in_flight(OSSL_ACKM *ackm)
{
    int i;
    uint64_t total = 0;

    for (i = 0; i < QUIC_PN_SPACE_NUM; ++i)
        total += ackm->ack_eliciting_bytes_in_flight[i];

    return total;
}

/* Return 1 if the range contains the given PN. */
static int range_contains(const OSSL_QUIC_ACK_RANGE *range, QUIC_PN pn)
{
    return pn >= range->start && pn <= range->end;
}

/*
 * Given a logical representation of an ACK frame 'ack', create a singly-linked
 * list of the newly ACK'd frames; that is, of frames which are matched by the
 * list of PN ranges contained in the ACK frame. The packet structures in the
 * list returned are removed from the TX history list. Returns a pointer to the
 * list head (or NULL) if empty.
 */
static OSSL_ACKM_TX_PKT *ackm_detect_and_remove_newly_acked_pkts(OSSL_ACKM *ackm,
                                                                 const OSSL_QUIC_FRAME_ACK *ack,
                                                                 int pkt_space)
{
    OSSL_ACKM_TX_PKT *acked_pkts = NULL, **fixup = &acked_pkts, *pkt, *pprev;
    struct tx_pkt_history_st *h;
    size_t ridx = 0;

    assert(ack->num_ack_ranges > 0);

    /*
     * Our history list is a list of packets sorted in ascending order
     * by packet number.
     *
     * ack->ack_ranges is a list of packet number ranges in descending order.
     *
     * Walk through our history list from the end in order to efficiently detect
     * membership in the specified ack ranges. As an optimization, we use our
     * hashtable to try and skip to the first matching packet. This may fail if
     * the ACK ranges given include nonexistent packets.
     */
    h = get_tx_history(ackm, pkt_space);

    pkt = tx_pkt_history_by_pkt_num(h, ack->ack_ranges[0].end);
    if (pkt == NULL)
        pkt = ossl_list_tx_history_tail(&h->packets);

    for (; pkt != NULL; pkt = pprev) {
        /*
         * Save prev value as it will be zeroed if we remove the packet from the
         * history list below.
         */
        pprev = ossl_list_tx_history_prev(pkt);

        for (;; ++ridx) {
            if (ridx >= ack->num_ack_ranges) {
                /*
                 * We have exhausted all ranges so stop here, even if there are
                 * more packets to look at.
                 */
                goto stop;
            }

            if (range_contains(&ack->ack_ranges[ridx], pkt->pkt_num)) {
                /* We have matched this range. */
                tx_pkt_history_remove(h, pkt->pkt_num);

                *fixup = pkt;
                fixup = &pkt->anext;
                *fixup = NULL;
                break;
            } else if (pkt->pkt_num > ack->ack_ranges[ridx].end) {
                /*
                 * We have not reached this range yet in our list, so do not
                 * advance ridx.
                 */
                break;
            } else {
                /*
                 * We have moved beyond this range, so advance to the next range
                 * and try matching again.
                 */
                assert(pkt->pkt_num < ack->ack_ranges[ridx].start);
                continue;
            }
        }
    }
stop:

    return acked_pkts;
}

/*
 * Create a singly-linked list of newly detected-lost packets in the given
 * packet number space. Returns the head of the list or NULL if no packets were
 * detected lost. The packets in the list are removed from the TX history list.
 */
static OSSL_ACKM_TX_PKT *ackm_detect_and_remove_lost_pkts(OSSL_ACKM *ackm,
                                                          int pkt_space)
{
    OSSL_ACKM_TX_PKT *lost_pkts = NULL, **fixup = &lost_pkts, *pkt, *pnext;
    OSSL_TIME loss_delay, lost_send_time, now;
    OSSL_RTT_INFO rtt;
    struct tx_pkt_history_st *h;

    assert(ackm->largest_acked_pkt[pkt_space] != QUIC_PN_INVALID);

    ossl_statm_get_rtt_info(ackm->statm, &rtt);

    ackm->loss_time[pkt_space] = ossl_time_zero();

    loss_delay = ossl_time_multiply(ossl_time_max(rtt.latest_rtt,
                                                  rtt.smoothed_rtt),
                                    K_TIME_THRESHOLD_NUM);
    loss_delay = ossl_time_divide(loss_delay, K_TIME_THRESHOLD_DEN);

    /* Minimum time of K_GRANULARITY before packets are deemed lost. */
    loss_delay = ossl_time_max(loss_delay, ossl_ticks2time(K_GRANULARITY));

    /* Packets sent before this time are deemed lost. */
    now = ackm->now(ackm->now_arg);
    lost_send_time = ossl_time_subtract(now, loss_delay);

    h   = get_tx_history(ackm, pkt_space);
    pkt = ossl_list_tx_history_head(&h->packets);

    for (; pkt != NULL; pkt = pnext) {
        assert(pkt_space == pkt->pkt_space);

        /*
         * Save prev value as it will be zeroed if we remove the packet from the
         * history list below.
         */
        pnext = ossl_list_tx_history_next(pkt);

        if (pkt->pkt_num > ackm->largest_acked_pkt[pkt_space])
            continue;

        /*
         * Mark packet as lost, or set time when it should be marked.
         */
        if (ossl_time_compare(pkt->time, lost_send_time) <= 0
                || ackm->largest_acked_pkt[pkt_space]
                >= pkt->pkt_num + K_PKT_THRESHOLD) {
            tx_pkt_history_remove(h, pkt->pkt_num);

            *fixup = pkt;
            fixup = &pkt->lnext;
            *fixup = NULL;
        } else {
            if (ossl_time_is_zero(ackm->loss_time[pkt_space]))
                ackm->loss_time[pkt_space] =
                    ossl_time_add(pkt->time, loss_delay);
            else
                ackm->loss_time[pkt_space] =
                    ossl_time_min(ackm->loss_time[pkt_space],
                                  ossl_time_add(pkt->time, loss_delay));
        }
    }

    return lost_pkts;
}

static OSSL_TIME ackm_get_loss_time_and_space(OSSL_ACKM *ackm, int *pspace)
{
    OSSL_TIME time = ackm->loss_time[QUIC_PN_SPACE_INITIAL];
    int i, space = QUIC_PN_SPACE_INITIAL;

    for (i = space + 1; i < QUIC_PN_SPACE_NUM; ++i)
        if (ossl_time_is_zero(time)
            || ossl_time_compare(ackm->loss_time[i], time) == -1) {
            time    = ackm->loss_time[i];
            space   = i;
        }

    *pspace = space;
    return time;
}

static OSSL_TIME ackm_get_pto_time_and_space(OSSL_ACKM *ackm, int *space)
{
    OSSL_RTT_INFO rtt;
    OSSL_TIME duration;
    OSSL_TIME pto_timeout = ossl_time_infinite(), t;
    int pto_space = QUIC_PN_SPACE_INITIAL, i;

    ossl_statm_get_rtt_info(ackm->statm, &rtt);

    duration
        = ossl_time_add(rtt.smoothed_rtt,
                        ossl_time_max(ossl_time_multiply(rtt.rtt_variance, 4),
                                      ossl_ticks2time(K_GRANULARITY)));

    duration
        = ossl_time_multiply(duration,
                             (uint64_t)1 << min_u32(ackm->pto_count,
                                                    MAX_PTO_COUNT));

    /* Anti-deadlock PTO starts from the current time. */
    if (ackm_ack_eliciting_bytes_in_flight(ackm) == 0) {
        assert(!ackm->peer_completed_addr_validation);

        *space = ackm->discarded[QUIC_PN_SPACE_INITIAL]
                    ? QUIC_PN_SPACE_HANDSHAKE
                    : QUIC_PN_SPACE_INITIAL;
        return ossl_time_add(ackm->now(ackm->now_arg), duration);
    }

    for (i = QUIC_PN_SPACE_INITIAL; i < QUIC_PN_SPACE_NUM; ++i) {
        if (ackm->ack_eliciting_bytes_in_flight[i] == 0)
            continue;

        if (i == QUIC_PN_SPACE_APP) {
            /* Skip application data until handshake confirmed. */
            if (!ackm->handshake_confirmed)
                break;

            /* Include max_ack_delay and backoff for app data. */
            if (!ossl_time_is_infinite(ackm->rx_max_ack_delay)) {
                uint64_t factor
                    = (uint64_t)1 << min_u32(ackm->pto_count, MAX_PTO_COUNT);

                duration
                    = ossl_time_add(duration,
                                    ossl_time_multiply(ackm->rx_max_ack_delay,
                                                       factor));
            }
        }

        t = ossl_time_add(ackm->time_of_last_ack_eliciting_pkt[i], duration);
        if (ossl_time_compare(t, pto_timeout) < 0) {
            pto_timeout = t;
            pto_space   = i;
        }
    }

    *space = pto_space;
    return pto_timeout;
}

static void ackm_set_loss_detection_timer_actual(OSSL_ACKM *ackm,
                                                 OSSL_TIME deadline)
{
    ackm->loss_detection_deadline = deadline;

    if (ackm->loss_detection_deadline_cb != NULL)
        ackm->loss_detection_deadline_cb(deadline,
                                         ackm->loss_detection_deadline_cb_arg);
}

static int ackm_set_loss_detection_timer(OSSL_ACKM *ackm)
{
    int space;
    OSSL_TIME earliest_loss_time, timeout;

    earliest_loss_time = ackm_get_loss_time_and_space(ackm, &space);
    if (!ossl_time_is_zero(earliest_loss_time)) {
        /* Time threshold loss detection. */
        ackm_set_loss_detection_timer_actual(ackm, earliest_loss_time);
        return 1;
    }

    if (ackm_ack_eliciting_bytes_in_flight(ackm) == 0
            && ackm->peer_completed_addr_validation) {
        /*
         * Nothing to detect lost, so no timer is set. However, the client
         * needs to arm the timer if the server might be blocked by the
         * anti-amplification limit.
         */
        ackm_set_loss_detection_timer_actual(ackm, ossl_time_zero());
        return 1;
    }

    timeout = ackm_get_pto_time_and_space(ackm, &space);
    ackm_set_loss_detection_timer_actual(ackm, timeout);
    return 1;
}

static int ackm_in_persistent_congestion(OSSL_ACKM *ackm,
                                         const OSSL_ACKM_TX_PKT *lpkt)
{
    /* TODO(QUIC FUTURE): Persistent congestion not currently implemented. */
    return 0;
}

static void ackm_on_pkts_lost(OSSL_ACKM *ackm, int pkt_space,
                              const OSSL_ACKM_TX_PKT *lpkt, int pseudo)
{
    const OSSL_ACKM_TX_PKT *p, *pnext;
    OSSL_RTT_INFO rtt;
    QUIC_PN largest_pn_lost = 0;
    OSSL_CC_LOSS_INFO loss_info = {0};
    uint32_t flags = 0;

    for (p = lpkt; p != NULL; p = pnext) {
        pnext = p->lnext;

        if (p->is_inflight) {
            ackm->bytes_in_flight -= p->num_bytes;
            if (p->is_ack_eliciting)
                ackm->ack_eliciting_bytes_in_flight[p->pkt_space]
                    -= p->num_bytes;

            if (p->pkt_num > largest_pn_lost)
                largest_pn_lost = p->pkt_num;

            if (!pseudo) {
                /*
                 * If this is pseudo-loss (e.g. during connection retry) we do not
                 * inform the CC as it is not a real loss and not reflective of
                 * network conditions.
                 */
                loss_info.tx_time = p->time;
                loss_info.tx_size = p->num_bytes;

                ackm->cc_method->on_data_lost(ackm->cc_data, &loss_info);
            }
        }

        p->on_lost(p->cb_arg);
    }

    /*
     * Persistent congestion can only be considered if we have gotten at least
     * one RTT sample.
     */
    ossl_statm_get_rtt_info(ackm->statm, &rtt);
    if (!ossl_time_is_zero(ackm->first_rtt_sample)
        && ackm_in_persistent_congestion(ackm, lpkt))
        flags |= OSSL_CC_LOST_FLAG_PERSISTENT_CONGESTION;

    ackm->cc_method->on_data_lost_finished(ackm->cc_data, flags);
}

static void ackm_on_pkts_acked(OSSL_ACKM *ackm, const OSSL_ACKM_TX_PKT *apkt)
{
    const OSSL_ACKM_TX_PKT *anext;
    QUIC_PN last_pn_acked = 0;
    OSSL_CC_ACK_INFO ainfo = {0};

    for (; apkt != NULL; apkt = anext) {
        if (apkt->is_inflight) {
            ackm->bytes_in_flight -= apkt->num_bytes;
            if (apkt->is_ack_eliciting)
                ackm->ack_eliciting_bytes_in_flight[apkt->pkt_space]
                    -= apkt->num_bytes;

            if (apkt->pkt_num > last_pn_acked)
                last_pn_acked = apkt->pkt_num;

            if (apkt->largest_acked != QUIC_PN_INVALID)
                /*
                 * This can fail, but it is monotonic; worst case we try again
                 * next time.
                 */
                rx_pkt_history_bump_watermark(get_rx_history(ackm,
                                                             apkt->pkt_space),
                                              apkt->largest_acked + 1);
        }

        ainfo.tx_time = apkt->time;
        ainfo.tx_size = apkt->num_bytes;

        anext = apkt->anext;
        apkt->on_acked(apkt->cb_arg); /* may free apkt */

        if (apkt->is_inflight)
            ackm->cc_method->on_data_acked(ackm->cc_data, &ainfo);
    }
}

OSSL_ACKM *ossl_ackm_new(OSSL_TIME (*now)(void *arg),
                         void *now_arg,
                         OSSL_STATM *statm,
                         const OSSL_CC_METHOD *cc_method,
                         OSSL_CC_DATA *cc_data)
{
    OSSL_ACKM *ackm;
    int i;

    ackm = OPENSSL_zalloc(sizeof(OSSL_ACKM));
    if (ackm == NULL)
        return NULL;

    for (i = 0; i < (int)OSSL_NELEM(ackm->tx_history); ++i) {
        ackm->largest_acked_pkt[i] = QUIC_PN_INVALID;
        ackm->rx_ack_flush_deadline[i] = ossl_time_infinite();
        if (tx_pkt_history_init(&ackm->tx_history[i]) < 1)
            goto err;
    }

    for (i = 0; i < (int)OSSL_NELEM(ackm->rx_history); ++i)
        rx_pkt_history_init(&ackm->rx_history[i]);

    ackm->now       = now;
    ackm->now_arg   = now_arg;
    ackm->statm     = statm;
    ackm->cc_method = cc_method;
    ackm->cc_data   = cc_data;

    ackm->rx_max_ack_delay = ossl_ms2time(QUIC_DEFAULT_MAX_ACK_DELAY);
    ackm->tx_max_ack_delay = DEFAULT_TX_MAX_ACK_DELAY;

    return ackm;

err:
    while (--i >= 0)
        tx_pkt_history_destroy(&ackm->tx_history[i]);

    OPENSSL_free(ackm);
    return NULL;
}

void ossl_ackm_free(OSSL_ACKM *ackm)
{
    size_t i;

    if (ackm == NULL)
        return;

    for (i = 0; i < OSSL_NELEM(ackm->tx_history); ++i)
        if (!ackm->discarded[i]) {
            tx_pkt_history_destroy(&ackm->tx_history[i]);
            rx_pkt_history_destroy(&ackm->rx_history[i]);
        }

    OPENSSL_free(ackm);
}

int ossl_ackm_on_tx_packet(OSSL_ACKM *ackm, OSSL_ACKM_TX_PKT *pkt)
{
    struct tx_pkt_history_st *h = get_tx_history(ackm, pkt->pkt_space);

    /* Time must be set and not move backwards. */
    if (ossl_time_is_zero(pkt->time)
        || ossl_time_compare(ackm->time_of_last_ack_eliciting_pkt[pkt->pkt_space],
                             pkt->time) > 0)
        return 0;

    /* Must have non-zero number of bytes. */
    if (pkt->num_bytes == 0)
        return 0;

    /* Does not make any sense for a non-in-flight packet to be ACK-eliciting. */
    if (!pkt->is_inflight && pkt->is_ack_eliciting)
        return 0;

    if (tx_pkt_history_add(h, pkt) == 0)
        return 0;

    if (pkt->is_inflight) {
        if (pkt->is_ack_eliciting) {
            ackm->time_of_last_ack_eliciting_pkt[pkt->pkt_space] = pkt->time;
            ackm->ack_eliciting_bytes_in_flight[pkt->pkt_space]
                += pkt->num_bytes;
        }

        ackm->bytes_in_flight += pkt->num_bytes;
        ackm_set_loss_detection_timer(ackm);

        ackm->cc_method->on_data_sent(ackm->cc_data, pkt->num_bytes);
    }

    return 1;
}

int ossl_ackm_on_rx_datagram(OSSL_ACKM *ackm, size_t num_bytes)
{
    /* No-op on the client. */
    return 1;
}

static void ackm_process_ecn(OSSL_ACKM *ackm, const OSSL_QUIC_FRAME_ACK *ack,
                             int pkt_space)
{
    struct tx_pkt_history_st *h;
    OSSL_ACKM_TX_PKT *pkt;
    OSSL_CC_ECN_INFO ecn_info = {0};

    /*
     * If the ECN-CE counter reported by the peer has increased, this could
     * be a new congestion event.
     */
    if (ack->ecnce > ackm->peer_ecnce[pkt_space]) {
        ackm->peer_ecnce[pkt_space] = ack->ecnce;

        h = get_tx_history(ackm, pkt_space);
        pkt = tx_pkt_history_by_pkt_num(h, ack->ack_ranges[0].end);
        if (pkt == NULL)
            return;

        ecn_info.largest_acked_time = pkt->time;
        ackm->cc_method->on_ecn(ackm->cc_data, &ecn_info);
    }
}

int ossl_ackm_on_rx_ack_frame(OSSL_ACKM *ackm, const OSSL_QUIC_FRAME_ACK *ack,
                              int pkt_space, OSSL_TIME rx_time)
{
    OSSL_ACKM_TX_PKT *na_pkts, *lost_pkts;
    int must_set_timer = 0;

    if (ackm->largest_acked_pkt[pkt_space] == QUIC_PN_INVALID)
        ackm->largest_acked_pkt[pkt_space] = ack->ack_ranges[0].end;
    else
        ackm->largest_acked_pkt[pkt_space]
            = ossl_quic_pn_max(ackm->largest_acked_pkt[pkt_space],
                               ack->ack_ranges[0].end);

    /*
     * If we get an ACK in the handshake space, address validation is completed.
     * Make sure we update the timer, even if no packets were ACK'd.
     */
    if (!ackm->peer_completed_addr_validation
            && pkt_space == QUIC_PN_SPACE_HANDSHAKE) {
        ackm->peer_completed_addr_validation = 1;
        must_set_timer = 1;
    }

    /*
     * Find packets that are newly acknowledged and remove them from the list.
     */
    na_pkts = ackm_detect_and_remove_newly_acked_pkts(ackm, ack, pkt_space);
    if (na_pkts == NULL) {
        if (must_set_timer)
            ackm_set_loss_detection_timer(ackm);

        return 1;
    }

    /*
     * Update the RTT if the largest acknowledged is newly acked and at least
     * one ACK-eliciting packet was newly acked.
     *
     * First packet in the list is always the one with the largest PN.
     */
    if (na_pkts->pkt_num == ack->ack_ranges[0].end &&
        ack_includes_ack_eliciting(na_pkts)) {
        OSSL_TIME now = ackm->now(ackm->now_arg), ack_delay;
        if (ossl_time_is_zero(ackm->first_rtt_sample))
            ackm->first_rtt_sample = now;

        /* Enforce maximum ACK delay. */
        ack_delay = ack->delay_time;
        if (ackm->handshake_confirmed)
            ack_delay = ossl_time_min(ack_delay, ackm->rx_max_ack_delay);

        ossl_statm_update_rtt(ackm->statm, ack_delay,
                              ossl_time_subtract(now, na_pkts->time));
    }

    /*
     * Process ECN information if present.
     *
     * We deliberately do most ECN processing in the ACKM rather than the
     * congestion controller to avoid having to give the congestion controller
     * access to ACKM internal state.
     */
    if (ack->ecn_present)
        ackm_process_ecn(ackm, ack, pkt_space);

    /* Handle inferred loss. */
    lost_pkts = ackm_detect_and_remove_lost_pkts(ackm, pkt_space);
    if (lost_pkts != NULL)
        ackm_on_pkts_lost(ackm, pkt_space, lost_pkts, /*pseudo=*/0);

    ackm_on_pkts_acked(ackm, na_pkts);

    /*
     * Reset pto_count unless the client is unsure if the server validated the
     * client's address.
     */
    if (ackm->peer_completed_addr_validation)
        ackm->pto_count = 0;

    ackm_set_loss_detection_timer(ackm);
    return 1;
}

int ossl_ackm_on_pkt_space_discarded(OSSL_ACKM *ackm, int pkt_space)
{
    OSSL_ACKM_TX_PKT *pkt, *pnext;
    uint64_t num_bytes_invalidated = 0;

    if (ackm->discarded[pkt_space])
        return 0;

    if (pkt_space == QUIC_PN_SPACE_HANDSHAKE)
        ackm->peer_completed_addr_validation = 1;

    for (pkt = ossl_list_tx_history_head(&get_tx_history(ackm, pkt_space)->packets);
            pkt != NULL; pkt = pnext) {
        pnext = ossl_list_tx_history_next(pkt);
        if (pkt->is_inflight) {
            ackm->bytes_in_flight -= pkt->num_bytes;
            num_bytes_invalidated += pkt->num_bytes;
        }

        pkt->on_discarded(pkt->cb_arg); /* may free pkt */
    }

    tx_pkt_history_destroy(&ackm->tx_history[pkt_space]);
    rx_pkt_history_destroy(&ackm->rx_history[pkt_space]);

    if (num_bytes_invalidated > 0)
        ackm->cc_method->on_data_invalidated(ackm->cc_data,
                                             num_bytes_invalidated);

    ackm->time_of_last_ack_eliciting_pkt[pkt_space] = ossl_time_zero();
    ackm->loss_time[pkt_space] = ossl_time_zero();
    ackm->pto_count = 0;
    ackm->discarded[pkt_space] = 1;
    ackm->ack_eliciting_bytes_in_flight[pkt_space] = 0;
    ackm_set_loss_detection_timer(ackm);
    return 1;
}

int ossl_ackm_on_handshake_confirmed(OSSL_ACKM *ackm)
{
    ackm->handshake_confirmed               = 1;
    ackm->peer_completed_addr_validation    = 1;
    ackm_set_loss_detection_timer(ackm);
    return 1;
}

static void ackm_queue_probe_anti_deadlock_handshake(OSSL_ACKM *ackm)
{
    ++ackm->pending_probe.anti_deadlock_handshake;
}

static void ackm_queue_probe_anti_deadlock_initial(OSSL_ACKM *ackm)
{
    ++ackm->pending_probe.anti_deadlock_initial;
}

static void ackm_queue_probe(OSSL_ACKM *ackm, int pkt_space)
{
    /*
     * TODO(QUIC FUTURE): We are allowed to send either one or two probe
     * packets here.
     * Determine a strategy for when we should send two probe packets.
     */
    ++ackm->pending_probe.pto[pkt_space];
}

int ossl_ackm_on_timeout(OSSL_ACKM *ackm)
{
    int pkt_space;
    OSSL_TIME earliest_loss_time;
    OSSL_ACKM_TX_PKT *lost_pkts;

    earliest_loss_time = ackm_get_loss_time_and_space(ackm, &pkt_space);
    if (!ossl_time_is_zero(earliest_loss_time)) {
        /* Time threshold loss detection. */
        lost_pkts = ackm_detect_and_remove_lost_pkts(ackm, pkt_space);
        if (lost_pkts != NULL)
            ackm_on_pkts_lost(ackm, pkt_space, lost_pkts, /*pseudo=*/0);
        ackm_set_loss_detection_timer(ackm);
        return 1;
    }

    if (ackm_ack_eliciting_bytes_in_flight(ackm) == 0) {
        assert(!ackm->peer_completed_addr_validation);
        /*
         * Client sends an anti-deadlock packet: Initial is padded to earn more
         * anti-amplification credit. A handshake packet proves address
         * ownership.
         */
        if (ackm->discarded[QUIC_PN_SPACE_INITIAL])
            ackm_queue_probe_anti_deadlock_handshake(ackm);
        else
            ackm_queue_probe_anti_deadlock_initial(ackm);
    } else {
        /*
         * PTO. The user of the ACKM should send new data if available, else
         * retransmit old data, or if neither is available, send a single PING
         * frame.
         */
        ackm_get_pto_time_and_space(ackm, &pkt_space);
        ackm_queue_probe(ackm, pkt_space);
    }

    ++ackm->pto_count;
    ackm_set_loss_detection_timer(ackm);
    return 1;
}

OSSL_TIME ossl_ackm_get_loss_detection_deadline(OSSL_ACKM *ackm)
{
    return ackm->loss_detection_deadline;
}

OSSL_ACKM_PROBE_INFO *ossl_ackm_get0_probe_request(OSSL_ACKM *ackm)
{
    return &ackm->pending_probe;
}

int ossl_ackm_get_largest_unacked(OSSL_ACKM *ackm, int pkt_space, QUIC_PN *pn)
{
    struct tx_pkt_history_st *h;
    OSSL_ACKM_TX_PKT *p;

    h = get_tx_history(ackm, pkt_space);
    p = ossl_list_tx_history_tail(&h->packets);
    if (p != NULL) {
        *pn = p->pkt_num;
        return 1;
    }

    return 0;
}

/* Number of ACK-eliciting packets RX'd before we always emit an ACK. */
#define PKTS_BEFORE_ACK     2

/*
 * Return 1 if emission of an ACK frame is currently desired.
 *
 * This occurs when one or more of the following conditions occurs:
 *
 *   - We have flagged that we want to send an ACK frame
 *     (for example, due to the packet threshold count being exceeded), or
 *
 *   - We have exceeded the ACK flush deadline, meaning that
 *     we have received at least one ACK-eliciting packet, but held off on
 *     sending an ACK frame immediately in the hope that more ACK-eliciting
 *     packets might come in, but not enough did and we are now requesting
 *     transmission of an ACK frame anyway.
 *
 */
int ossl_ackm_is_ack_desired(OSSL_ACKM *ackm, int pkt_space)
{
    return ackm->rx_ack_desired[pkt_space]
        || (!ossl_time_is_infinite(ackm->rx_ack_flush_deadline[pkt_space])
            && ossl_time_compare(ackm->now(ackm->now_arg),
                                 ackm->rx_ack_flush_deadline[pkt_space]) >= 0);
}

/*
 * Returns 1 if an ACK frame matches a given packet number.
 */
static int ack_contains(const OSSL_QUIC_FRAME_ACK *ack, QUIC_PN pkt_num)
{
    size_t i;

    for (i = 0; i < ack->num_ack_ranges; ++i)
        if (range_contains(&ack->ack_ranges[i], pkt_num))
            return 1;

    return 0;
}

/*
 * Returns 1 iff a PN (which we have just received) was previously reported as
 * implied missing (by us, in an ACK frame we previously generated).
 */
static int ackm_is_missing(OSSL_ACKM *ackm, int pkt_space, QUIC_PN pkt_num)
{
    /*
     * A PN is implied missing if it is not greater than the highest PN in our
     * generated ACK frame, but is not matched by the frame.
     */
    return ackm->ack[pkt_space].num_ack_ranges > 0
        && pkt_num <= ackm->ack[pkt_space].ack_ranges[0].end
        && !ack_contains(&ackm->ack[pkt_space], pkt_num);
}

/*
 * Returns 1 iff our RX of a PN newly establishes the implication of missing
 * packets.
 */
static int ackm_has_newly_missing(OSSL_ACKM *ackm, int pkt_space)
{
    struct rx_pkt_history_st *h;

    h = get_rx_history(ackm, pkt_space);

    if (ossl_list_uint_set_is_empty(&h->set))
        return 0;

    /*
     * The second condition here establishes that the highest PN range in our RX
     * history comprises only a single PN. If there is more than one, then this
     * function will have returned 1 during a previous call to
     * ossl_ackm_on_rx_packet assuming the third condition below was met. Thus
     * we only return 1 when the missing PN condition is newly established.
     *
     * The third condition here establishes that the highest PN range in our RX
     * history is beyond (and does not border) the highest PN we have yet
     * reported in any ACK frame. Thus there is a gap of at least one PN between
     * the PNs we have ACK'd previously and the PN we have just received.
     */
    return ackm->ack[pkt_space].num_ack_ranges > 0
        && ossl_list_uint_set_tail(&h->set)->range.start
           == ossl_list_uint_set_tail(&h->set)->range.end
        && ossl_list_uint_set_tail(&h->set)->range.start
            > ackm->ack[pkt_space].ack_ranges[0].end + 1;
}

static void ackm_set_flush_deadline(OSSL_ACKM *ackm, int pkt_space,
                                    OSSL_TIME deadline)
{
    ackm->rx_ack_flush_deadline[pkt_space] = deadline;

    if (ackm->ack_deadline_cb != NULL)
        ackm->ack_deadline_cb(ossl_ackm_get_ack_deadline(ackm, pkt_space),
                              pkt_space, ackm->ack_deadline_cb_arg);
}

/* Explicitly flags that we want to generate an ACK frame. */
static void ackm_queue_ack(OSSL_ACKM *ackm, int pkt_space)
{
    ackm->rx_ack_desired[pkt_space] = 1;

    /* Cancel deadline. */
    ackm_set_flush_deadline(ackm, pkt_space, ossl_time_infinite());
}

static void ackm_on_rx_ack_eliciting(OSSL_ACKM *ackm,
                                     OSSL_TIME rx_time, int pkt_space,
                                     int was_missing)
{
    OSSL_TIME tx_max_ack_delay;

    if (ackm->rx_ack_desired[pkt_space])
        /* ACK generation already requested so nothing to do. */
        return;

    ++ackm->rx_ack_eliciting_pkts_since_last_ack[pkt_space];

    if (!ackm->rx_ack_generated[pkt_space]
            || was_missing
            || ackm->rx_ack_eliciting_pkts_since_last_ack[pkt_space]
                >= PKTS_BEFORE_ACK
            || ackm_has_newly_missing(ackm, pkt_space)) {
        /*
         * Either:
         *
         *   - We have never yet generated an ACK frame, meaning that this
         *     is the first ever packet received, which we should always
         *     acknowledge immediately, or
         *
         *   - We previously reported the PN that we have just received as
         *     missing in a previous ACK frame (meaning that we should report
         *     the fact that we now have it to the peer immediately), or
         *
         *   - We have exceeded the ACK-eliciting packet threshold count
         *     for the purposes of ACK coalescing, so request transmission
         *     of an ACK frame, or
         *
         *   - The PN we just received and added to our PN RX history
         *     newly implies one or more missing PNs, in which case we should
         *     inform the peer by sending an ACK frame immediately.
         *
         * We do not test the ACK flush deadline here because it is tested
         * separately in ossl_ackm_is_ack_desired.
         */
        ackm_queue_ack(ackm, pkt_space);
        return;
    }

    /*
     * Not emitting an ACK yet.
     *
     * Update the ACK flush deadline.
     *
     * RFC 9000 s. 13.2.1: "An endpoint MUST acknowledge all ack-eliciting
     * Initial and Handshake packets immediately"; don't delay ACK generation if
     * we are using the Initial or Handshake PN spaces.
     */
    tx_max_ack_delay = ackm->tx_max_ack_delay;
    if (pkt_space == QUIC_PN_SPACE_INITIAL
        || pkt_space == QUIC_PN_SPACE_HANDSHAKE)
        tx_max_ack_delay = ossl_time_zero();

    if (ossl_time_is_infinite(ackm->rx_ack_flush_deadline[pkt_space]))
        ackm_set_flush_deadline(ackm, pkt_space,
                                ossl_time_add(rx_time, tx_max_ack_delay));
    else
        ackm_set_flush_deadline(ackm, pkt_space,
                                ossl_time_min(ackm->rx_ack_flush_deadline[pkt_space],
                                              ossl_time_add(rx_time,
                                                            tx_max_ack_delay)));
}

int ossl_ackm_on_rx_packet(OSSL_ACKM *ackm, const OSSL_ACKM_RX_PKT *pkt)
{
    struct rx_pkt_history_st *h = get_rx_history(ackm, pkt->pkt_space);
    int was_missing;

    if (ossl_ackm_is_rx_pn_processable(ackm, pkt->pkt_num, pkt->pkt_space) != 1)
        /* PN has already been processed or written off, no-op. */
        return 1;

    /*
     * Record the largest PN we have RX'd and the time we received it.
     * We use this to calculate the ACK delay field of ACK frames.
     */
    if (pkt->pkt_num > ackm->rx_largest_pn[pkt->pkt_space]) {
        ackm->rx_largest_pn[pkt->pkt_space]   = pkt->pkt_num;
        ackm->rx_largest_time[pkt->pkt_space] = pkt->time;
    }

    /*
     * If the PN we just received was previously implied missing by virtue of
     * being omitted from a previous ACK frame generated, we skip any packet
     * count thresholds or coalescing delays and emit a new ACK frame
     * immediately.
     */
    was_missing = ackm_is_missing(ackm, pkt->pkt_space, pkt->pkt_num);

    /*
     * Add the packet number to our history list of PNs we have not yet provably
     * acked.
     */
    if (rx_pkt_history_add_pn(h, pkt->pkt_num) != 1)
        return 0;

    /*
     * Receiving this packet may or may not cause us to emit an ACK frame.
     * We may not emit an ACK frame yet if we have not yet received a threshold
     * number of packets.
     */
    if (pkt->is_ack_eliciting)
        ackm_on_rx_ack_eliciting(ackm, pkt->time, pkt->pkt_space, was_missing);

    /* Update the ECN counters according to which ECN signal we got, if any. */
    switch (pkt->ecn) {
    case OSSL_ACKM_ECN_ECT0:
        ++ackm->rx_ect0[pkt->pkt_space];
        break;
    case OSSL_ACKM_ECN_ECT1:
        ++ackm->rx_ect1[pkt->pkt_space];
        break;
    case OSSL_ACKM_ECN_ECNCE:
        ++ackm->rx_ecnce[pkt->pkt_space];
        break;
    default:
        break;
    }

    return 1;
}

static void ackm_fill_rx_ack_ranges(OSSL_ACKM *ackm, int pkt_space,
                                    OSSL_QUIC_FRAME_ACK *ack)
{
    struct rx_pkt_history_st *h = get_rx_history(ackm, pkt_space);
    UINT_SET_ITEM *x;
    size_t i = 0;

    /*
     * Copy out ranges from the PN set, starting at the end, until we reach our
     * maximum number of ranges.
     */
    for (x = ossl_list_uint_set_tail(&h->set);
         x != NULL && i < OSSL_NELEM(ackm->ack_ranges);
         x = ossl_list_uint_set_prev(x), ++i) {
        ackm->ack_ranges[pkt_space][i].start = x->range.start;
        ackm->ack_ranges[pkt_space][i].end   = x->range.end;
    }

    ack->ack_ranges     = ackm->ack_ranges[pkt_space];
    ack->num_ack_ranges = i;
}

const OSSL_QUIC_FRAME_ACK *ossl_ackm_get_ack_frame(OSSL_ACKM *ackm,
                                                   int pkt_space)
{
    OSSL_QUIC_FRAME_ACK *ack = &ackm->ack[pkt_space];
    OSSL_TIME now = ackm->now(ackm->now_arg);

    ackm_fill_rx_ack_ranges(ackm, pkt_space, ack);

    if (!ossl_time_is_zero(ackm->rx_largest_time[pkt_space])
            && ossl_time_compare(now, ackm->rx_largest_time[pkt_space]) > 0
            && pkt_space == QUIC_PN_SPACE_APP)
        ack->delay_time =
            ossl_time_subtract(now, ackm->rx_largest_time[pkt_space]);
    else
        ack->delay_time = ossl_time_zero();

    ack->ect0              = ackm->rx_ect0[pkt_space];
    ack->ect1              = ackm->rx_ect1[pkt_space];
    ack->ecnce             = ackm->rx_ecnce[pkt_space];
    ack->ecn_present       = 1;

    ackm->rx_ack_eliciting_pkts_since_last_ack[pkt_space] = 0;

    ackm->rx_ack_generated[pkt_space]       = 1;
    ackm->rx_ack_desired[pkt_space]         = 0;
    ackm_set_flush_deadline(ackm, pkt_space, ossl_time_infinite());
    return ack;
}


OSSL_TIME ossl_ackm_get_ack_deadline(OSSL_ACKM *ackm, int pkt_space)
{
    if (ackm->rx_ack_desired[pkt_space])
        /* Already desired, deadline is now. */
        return ossl_time_zero();

    return ackm->rx_ack_flush_deadline[pkt_space];
}

int ossl_ackm_is_rx_pn_processable(OSSL_ACKM *ackm, QUIC_PN pn, int pkt_space)
{
    struct rx_pkt_history_st *h = get_rx_history(ackm, pkt_space);

    return pn >= h->watermark && ossl_uint_set_query(&h->set, pn) == 0;
}

void ossl_ackm_set_loss_detection_deadline_callback(OSSL_ACKM *ackm,
                                                    void (*fn)(OSSL_TIME deadline,
                                                               void *arg),
                                                    void *arg)
{
    ackm->loss_detection_deadline_cb      = fn;
    ackm->loss_detection_deadline_cb_arg  = arg;
}

void ossl_ackm_set_ack_deadline_callback(OSSL_ACKM *ackm,
                                         void (*fn)(OSSL_TIME deadline,
                                                    int pkt_space,
                                                    void *arg),
                                         void *arg)
{
    ackm->ack_deadline_cb     = fn;
    ackm->ack_deadline_cb_arg = arg;
}

int ossl_ackm_mark_packet_pseudo_lost(OSSL_ACKM *ackm,
                                      int pkt_space, QUIC_PN pn)
{
    struct tx_pkt_history_st *h = get_tx_history(ackm, pkt_space);
    OSSL_ACKM_TX_PKT *pkt;

    pkt = tx_pkt_history_by_pkt_num(h, pn);
    if (pkt == NULL)
        return 0;

    tx_pkt_history_remove(h, pkt->pkt_num);
    pkt->lnext = NULL;
    ackm_on_pkts_lost(ackm, pkt_space, pkt, /*pseudo=*/1);
    return 1;
}

OSSL_TIME ossl_ackm_get_pto_duration(OSSL_ACKM *ackm)
{
    OSSL_TIME duration;
    OSSL_RTT_INFO rtt;

    ossl_statm_get_rtt_info(ackm->statm, &rtt);

    duration = ossl_time_add(rtt.smoothed_rtt,
                             ossl_time_max(ossl_time_multiply(rtt.rtt_variance, 4),
                                           ossl_ticks2time(K_GRANULARITY)));
    if (!ossl_time_is_infinite(ackm->rx_max_ack_delay))
        duration = ossl_time_add(duration, ackm->rx_max_ack_delay);

    return duration;
}

QUIC_PN ossl_ackm_get_largest_acked(OSSL_ACKM *ackm, int pkt_space)
{
    return ackm->largest_acked_pkt[pkt_space];
}

void ossl_ackm_set_rx_max_ack_delay(OSSL_ACKM *ackm, OSSL_TIME rx_max_ack_delay)
{
    ackm->rx_max_ack_delay = rx_max_ack_delay;
}

void ossl_ackm_set_tx_max_ack_delay(OSSL_ACKM *ackm, OSSL_TIME tx_max_ack_delay)
{
    ackm->tx_max_ack_delay = tx_max_ack_delay;
}
