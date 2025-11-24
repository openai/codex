/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_rcidm.h"
#include "internal/priority_queue.h"
#include "internal/list.h"
#include "internal/common.h"

/*
 * QUIC Remote Connection ID Manager
 * =================================
 *
 * We can receive an arbitrary number of RCIDs via NCID frames. Periodically, we
 * may desire (for example for anti-connection fingerprinting reasons, etc.)
 * to switch to a new RCID according to some arbitrary policy such as the number
 * of packets we have sent.
 *
 * When we do this we should move to the next RCID in the sequence of received
 * RCIDs ordered by sequence number. For example, if a peer sends us three NCID
 * frames with sequence numbers 10, 11, 12, we should seek to consume these
 * RCIDs in order.
 *
 * However, due to the possibility of packet reordering in the network, NCID
 * frames might be received out of order. Thus if a peer sends us NCID frames
 * with sequence numbers 12, 10, 11, we should still consume the RCID with
 * sequence number 10 before consuming the RCIDs with sequence numbers 11 or 12.
 *
 * We use a priority queue for this purpose.
 */
static void rcidm_update(QUIC_RCIDM *rcidm);
static void rcidm_set_preferred_rcid(QUIC_RCIDM *rcidm,
                                     const QUIC_CONN_ID *rcid);

#define PACKETS_PER_RCID        10000

#define INITIAL_SEQ_NUM         0
#define PREF_ADDR_SEQ_NUM       1

/*
 * RCID
 * ====
 *
 * The RCID structure is used to track RCIDs which have sequence numbers (i.e.,
 * INITIAL, PREF_ADDR and NCID type RCIDs). The RCIDs without sequence numbers
 * (Initial ODCIDs and Retry ODCIDs), hereafter referred to as unnumbered RCIDs,
 * can logically be viewed as their own type of RCID but are tracked separately
 * as singletons without needing a discrete structure.
 *
 * At any given time an RCID object is in one of these states:
 *
 *
 *      (start)
 *         |
 *       [add]
 *         |
 *    _____v_____                 ___________                 ____________
 *   |           |               |           |               |            |
 *   |  PENDING  | --[select]--> |  CURRENT  | --[retire]--> |  RETIRING  |
 *   |___________|               |___________|               |____________|
 *                                                                  |
 *                                                                [pop]
 *                                                                  |
 *                                                                  v
 *                                                                (fin)
 *
 *   The transition through the states is monotonic and irreversible.
 *   The RCID object is freed when it is popped.
 *
 *   PENDING
 *     Invariants:
 *       rcid->state == RCID_STATE_PENDING;
 *       rcid->pq_idx != SIZE_MAX (debug assert only);
 *       the RCID is not the current RCID, rcidm->cur_rcid != rcid;
 *       the RCID is in the priority queue;
 *       the RCID is not in the retiring_list.
 *
 *   CURRENT
 *     Invariants:
 *       rcid->state == RCID_STATE_CUR;
 *       rcid->pq_idx == SIZE_MAX (debug assert only);
 *       the RCID is the current RCID, rcidm->cur_rcid == rcid;
 *       the RCID is not in the priority queue;
 *       the RCID is not in the retiring_list.
 *
 *   RETIRING
 *     Invariants:
 *       rcid->state == RCID_STATE_RETIRING;
 *       rcid->pq_idx == SIZE_MAX (debug assert only);
 *       the RCID is not the current RCID, rcidm->cur_rcid != rcid;
 *       the RCID is not in the priority queue;
 *       the RCID is in the retiring_list.
 *
 *   Invariant: At most one RCID object is in the CURRENT state at any one time.
 *
 *      (If no RCID object is in the CURRENT state, this means either
 *       an unnumbered RCID is being used as the preferred RCID
 *       or we currently have no preferred RCID.)
 *
 *   All of the above states can be considered substates of the 'ACTIVE' state
 *   for an RCID as specified in RFC 9000. A CID only ceases to be active
 *   when we send a RETIRE_CONN_ID frame, which is the responsibility of the
 *   user of the RCIDM and happens after the above state machine is terminated.
 */
enum {
    RCID_STATE_PENDING,
    RCID_STATE_CUR,
    RCID_STATE_RETIRING
};

enum {
    RCID_TYPE_INITIAL,      /* CID is from an peer INITIAL packet     (seq 0) */
    RCID_TYPE_PREF_ADDR,    /* CID is from a preferred_address TPARAM (seq 1) */
    RCID_TYPE_NCID          /* CID is from a NCID frame */
    /*
     * INITIAL_ODCID and RETRY_ODCID also conceptually exist but are tracked
     * separately.
     */
};

typedef struct rcid_st {
    OSSL_LIST_MEMBER(retiring, struct rcid_st); /* valid iff RETIRING */

    QUIC_CONN_ID    cid;        /* The actual CID string for this RCID */
    uint64_t        seq_num;
    size_t          pq_idx;     /* Index of entry into priority queue */
    unsigned int    state  : 2; /* RCID_STATE_* */
    unsigned int    type   : 2; /* RCID_TYPE_* */
} RCID;

DEFINE_PRIORITY_QUEUE_OF(RCID);
DEFINE_LIST_OF(retiring, RCID);

/*
 * RCID Manager
 * ============
 *
 * The following "business logic" invariants also apply to the RCIDM
 * as a whole:
 *
 *   Invariant: An RCID of INITIAL   type has a sequence number of 0.
 *   Invariant: An RCID of PREF_ADDR type has a sequence number of 1.
 *
 *   Invariant: There is never more than one Initial ODCID
 *              added throughout the lifetime of an RCIDM.
 *   Invariant: There is never more than one Retry ODCID
 *              added throughout the lifetime of an RCIDM.
 *   Invariant: There is never more than one INITIAL RCID created
 *              throughout the lifetime of an RCIDM.
 *   Invariant: There is never more than one PREF_ADDR RCID created
 *              throughout the lifetime of an RCIDM.
 *   Invariant: No INITIAL or PREF_ADDR RCID may be added after
 *              the handshake is completed.
 *
 */
struct quic_rcidm_st {
    /*
     * The current RCID we prefer to use (value undefined if
     * !have_preferred_rcid).
     *
     * This is preferentially set to a numbered RCID (represented by an RCID
     * object) if we have one (in which case preferred_rcid == cur_rcid->cid);
     * otherwise it is set to one of the unnumbered RCIDs (the Initial ODCID or
     * Retry ODCID) if available (and cur_rcid == NULL).
     */
    QUIC_CONN_ID                preferred_rcid;

    /*
     * These are initialized if the corresponding added_ flags are set.
     */
    QUIC_CONN_ID                initial_odcid, retry_odcid;

    /*
     * Total number of packets sent since we last made a packet count-based RCID
     * update decision.
     */
    uint64_t                    packets_sent;

    /* Number of post-handshake RCID changes we have performed. */
    uint64_t                    num_changes;

    /*
     * The Retire Prior To watermark value; max(retire_prior_to) of all received
     * NCID frames.
     */
    uint64_t                    retire_prior_to;

    /* (SORT BY seq_num ASC) -> (RCID *) */
    PRIORITY_QUEUE_OF(RCID)     *rcids;

    /*
     * Current RCID object we are using. This may differ from the first item in
     * the priority queue if we received NCID frames out of order. For example
     * if we get seq 5, switch to it immediately, then get seq 4, we want to
     * keep using seq 5 until we decide to roll again rather than immediately
     * switch to seq 4. Never points to an object on the retiring_list.
     */
    RCID                        *cur_rcid;

    /*
     * When a RCID becomes pending-retirement, it is moved to the retiring_list,
     * then freed when it is popped from the retired queue. We use a list for
     * this rather than a priority queue as the order in which items are freed
     * does not matter. We always append to the tail of the list in order to
     * maintain the guarantee that the head (if present) only changes when a
     * caller calls pop().
     */
    OSSL_LIST(retiring)         retiring_list;

    /* Number of entries on the retiring_list. */
    size_t                      num_retiring;

    /* preferred_rcid has been changed? */
    unsigned int    preferred_rcid_changed          : 1;

    /* Do we have any RCID we can use currently? */
    unsigned int    have_preferred_rcid             : 1;

    /* QUIC handshake has been completed? */
    unsigned int    handshake_complete              : 1;

    /* odcid was set (not necessarily still valid as a RCID)? */
    unsigned int    added_initial_odcid             : 1;
    /* retry_odcid was set (not necessarily still valid as a RCID?) */
    unsigned int    added_retry_odcid               : 1;
    /* An initial RCID was added as an RCID structure? */
    unsigned int    added_initial_rcid              : 1;
    /* Has a RCID roll been manually requested? */
    unsigned int    roll_requested                  : 1;
};

/*
 * Caller must periodically pop retired RCIDs and handle them. If the caller
 * fails to do so, fail safely rather than start exhibiting integer rollover.
 * Limit the total number of numbered RCIDs to an implausibly large but safe
 * value.
 */
#define MAX_NUMBERED_RCIDS      (SIZE_MAX / 2)

static void rcidm_transition_rcid(QUIC_RCIDM *rcidm, RCID *rcid,
                                  unsigned int state);

/* Check invariants of an RCID */
static void rcidm_check_rcid(QUIC_RCIDM *rcidm, RCID *rcid)
{
    assert(rcid->state == RCID_STATE_PENDING
           || rcid->state == RCID_STATE_CUR
           || rcid->state == RCID_STATE_RETIRING);
    assert((rcid->state == RCID_STATE_PENDING)
           == (rcid->pq_idx != SIZE_MAX));
    assert((rcid->state == RCID_STATE_CUR)
           == (rcidm->cur_rcid == rcid));
    assert((ossl_list_retiring_next(rcid) != NULL
            || ossl_list_retiring_prev(rcid) != NULL
            || ossl_list_retiring_head(&rcidm->retiring_list) == rcid)
           == (rcid->state == RCID_STATE_RETIRING));
    assert(rcid->type != RCID_TYPE_INITIAL || rcid->seq_num == 0);
    assert(rcid->type != RCID_TYPE_PREF_ADDR || rcid->seq_num == 1);
    assert(rcid->seq_num <= OSSL_QUIC_VLINT_MAX);
    assert(rcid->cid.id_len > 0 && rcid->cid.id_len <= QUIC_MAX_CONN_ID_LEN);
    assert(rcid->seq_num >= rcidm->retire_prior_to
            || rcid->state == RCID_STATE_RETIRING);
    assert(rcidm->num_changes == 0 || rcidm->handshake_complete);
    assert(rcid->state != RCID_STATE_RETIRING || rcidm->num_retiring > 0);
}

static int rcid_cmp(const RCID *a, const RCID *b)
{
    if (a->seq_num < b->seq_num)
        return -1;
    if (a->seq_num > b->seq_num)
        return 1;
    return 0;
}

QUIC_RCIDM *ossl_quic_rcidm_new(const QUIC_CONN_ID *initial_odcid)
{
    QUIC_RCIDM *rcidm;

    if ((rcidm = OPENSSL_zalloc(sizeof(*rcidm))) == NULL)
        return NULL;

    if ((rcidm->rcids = ossl_pqueue_RCID_new(rcid_cmp)) == NULL) {
        OPENSSL_free(rcidm);
        return NULL;
    }

    if (initial_odcid != NULL) {
        rcidm->initial_odcid        = *initial_odcid;
        rcidm->added_initial_odcid  = 1;
    }

    rcidm_update(rcidm);
    return rcidm;
}

void ossl_quic_rcidm_free(QUIC_RCIDM *rcidm)
{
    RCID *rcid, *rnext;

    if (rcidm == NULL)
        return;

    OPENSSL_free(rcidm->cur_rcid);
    while ((rcid = ossl_pqueue_RCID_pop(rcidm->rcids)) != NULL)
        OPENSSL_free(rcid);

    OSSL_LIST_FOREACH_DELSAFE(rcid, rnext, retiring, &rcidm->retiring_list)
        OPENSSL_free(rcid);

    ossl_pqueue_RCID_free(rcidm->rcids);
    OPENSSL_free(rcidm);
}

static void rcidm_set_preferred_rcid(QUIC_RCIDM *rcidm,
                                     const QUIC_CONN_ID *rcid)
{
    if (rcid == NULL) {
        rcidm->preferred_rcid_changed   = 1;
        rcidm->have_preferred_rcid      = 0;
        return;
    }

    if (ossl_quic_conn_id_eq(&rcidm->preferred_rcid, rcid))
        return;

    rcidm->preferred_rcid           = *rcid;
    rcidm->preferred_rcid_changed   = 1;
    rcidm->have_preferred_rcid      = 1;
}

/*
 * RCID Lifecycle Management
 * =========================
 */
static RCID *rcidm_create_rcid(QUIC_RCIDM *rcidm, uint64_t seq_num,
                               const QUIC_CONN_ID *cid,
                               unsigned int type)
{
    RCID *rcid;

    if (cid->id_len < 1 || cid->id_len > QUIC_MAX_CONN_ID_LEN
        || seq_num > OSSL_QUIC_VLINT_MAX
        || ossl_pqueue_RCID_num(rcidm->rcids) + rcidm->num_retiring
            > MAX_NUMBERED_RCIDS)
        return NULL;

    if ((rcid = OPENSSL_zalloc(sizeof(*rcid))) == NULL)
        return NULL;

    rcid->seq_num           = seq_num;
    rcid->cid               = *cid;
    rcid->type              = type;

    if (rcid->seq_num >= rcidm->retire_prior_to) {
        rcid->state = RCID_STATE_PENDING;

        if (!ossl_pqueue_RCID_push(rcidm->rcids, rcid, &rcid->pq_idx)) {
            OPENSSL_free(rcid);
            return NULL;
        }
    } else {
        /* RCID is immediately retired upon creation. */
        rcid->state     = RCID_STATE_RETIRING;
        rcid->pq_idx    = SIZE_MAX;
        ossl_list_retiring_insert_tail(&rcidm->retiring_list, rcid);
        ++rcidm->num_retiring;
    }

    rcidm_check_rcid(rcidm, rcid);
    return rcid;
}

static void rcidm_transition_rcid(QUIC_RCIDM *rcidm, RCID *rcid,
                                  unsigned int state)
{
    unsigned int old_state = rcid->state;

    assert(state >= old_state && state <= RCID_STATE_RETIRING);
    rcidm_check_rcid(rcidm, rcid);
    if (state == old_state)
        return;

    if (rcidm->cur_rcid != NULL && state == RCID_STATE_CUR) {
        rcidm_transition_rcid(rcidm, rcidm->cur_rcid, RCID_STATE_RETIRING);
        assert(rcidm->cur_rcid == NULL);
    }

    if (old_state == RCID_STATE_PENDING) {
        ossl_pqueue_RCID_remove(rcidm->rcids, rcid->pq_idx);
        rcid->pq_idx = SIZE_MAX;
    }

    rcid->state = state;

    if (state == RCID_STATE_CUR) {
        rcidm->cur_rcid = rcid;
    } else if (state == RCID_STATE_RETIRING) {
        if (old_state == RCID_STATE_CUR)
            rcidm->cur_rcid = NULL;

        ossl_list_retiring_insert_tail(&rcidm->retiring_list, rcid);
        ++rcidm->num_retiring;
    }

    rcidm_check_rcid(rcidm, rcid);
}

static void rcidm_free_rcid(QUIC_RCIDM *rcidm, RCID *rcid)
{
    if (rcid == NULL)
        return;

    rcidm_check_rcid(rcidm, rcid);

    switch (rcid->state) {
    case RCID_STATE_PENDING:
        ossl_pqueue_RCID_remove(rcidm->rcids, rcid->pq_idx);
        break;
    case RCID_STATE_CUR:
        rcidm->cur_rcid = NULL;
        break;
    case RCID_STATE_RETIRING:
        ossl_list_retiring_remove(&rcidm->retiring_list, rcid);
        --rcidm->num_retiring;
        break;
    default:
        assert(0);
        break;
    }

    OPENSSL_free(rcid);
}

static void rcidm_handle_retire_prior_to(QUIC_RCIDM *rcidm,
                                         uint64_t retire_prior_to)
{
    RCID *rcid;

    if (retire_prior_to <= rcidm->retire_prior_to)
        return;

    /*
     * Retire the current RCID (if any) if it is affected.
     */
    if (rcidm->cur_rcid != NULL && rcidm->cur_rcid->seq_num < retire_prior_to)
        rcidm_transition_rcid(rcidm, rcidm->cur_rcid, RCID_STATE_RETIRING);

    /*
     * Any other RCIDs needing retirement will be at the start of the priority
     * queue, so just stop once we see a higher sequence number exceeding the
     * threshold.
     */
    while ((rcid = ossl_pqueue_RCID_peek(rcidm->rcids)) != NULL
           && rcid->seq_num < retire_prior_to)
        rcidm_transition_rcid(rcidm, rcid, RCID_STATE_RETIRING);

    rcidm->retire_prior_to = retire_prior_to;
}

/*
 * Decision Logic
 * ==============
 */

static void rcidm_roll(QUIC_RCIDM *rcidm)
{
    RCID *rcid;

    if ((rcid = ossl_pqueue_RCID_peek(rcidm->rcids)) == NULL)
        return;

    rcidm_transition_rcid(rcidm, rcid, RCID_STATE_CUR);

    ++rcidm->num_changes;
    rcidm->roll_requested = 0;

    if (rcidm->packets_sent >= PACKETS_PER_RCID)
        rcidm->packets_sent %= PACKETS_PER_RCID;
    else
        rcidm->packets_sent = 0;
}

static void rcidm_update(QUIC_RCIDM *rcidm)
{
    RCID *rcid;

    /*
     * If we have no current numbered RCID but have one or more pending, use it.
     */
    if (rcidm->cur_rcid == NULL
        && (rcid = ossl_pqueue_RCID_peek(rcidm->rcids)) != NULL) {
        rcidm_transition_rcid(rcidm, rcid, RCID_STATE_CUR);
        assert(rcidm->cur_rcid != NULL);
    }

    /* Prefer use of any current numbered RCID we have, if possible. */
    if (rcidm->cur_rcid != NULL) {
        rcidm_check_rcid(rcidm, rcidm->cur_rcid);
        rcidm_set_preferred_rcid(rcidm, &rcidm->cur_rcid->cid);
        return;
    }

    /*
     * If there are no RCIDs from NCID frames we can use, go through the various
     * kinds of bootstrapping RCIDs we can use in order of priority.
     */
    if (rcidm->added_retry_odcid && !rcidm->handshake_complete) {
        rcidm_set_preferred_rcid(rcidm, &rcidm->retry_odcid);
        return;
    }

    if (rcidm->added_initial_odcid && !rcidm->handshake_complete) {
        rcidm_set_preferred_rcid(rcidm, &rcidm->initial_odcid);
        return;
    }

    /* We don't know of any usable RCIDs */
    rcidm_set_preferred_rcid(rcidm, NULL);
}

static int rcidm_should_roll(QUIC_RCIDM *rcidm)
{
    /*
     * Always switch as soon as possible if handshake completes;
     * and every n packets after handshake completes or the last roll; and
     * whenever manually requested.
     */
    return rcidm->handshake_complete
        && (rcidm->num_changes == 0
            || rcidm->packets_sent >= PACKETS_PER_RCID
            || rcidm->roll_requested);
}

static void rcidm_tick(QUIC_RCIDM *rcidm)
{
    if (rcidm_should_roll(rcidm))
        rcidm_roll(rcidm);

    rcidm_update(rcidm);
}

/*
 * Events
 * ======
 */
void ossl_quic_rcidm_on_handshake_complete(QUIC_RCIDM *rcidm)
{
    if (rcidm->handshake_complete)
        return;

    rcidm->handshake_complete = 1;
    rcidm_tick(rcidm);
}

void ossl_quic_rcidm_on_packet_sent(QUIC_RCIDM *rcidm, uint64_t num_packets)
{
    if (num_packets == 0)
        return;

    rcidm->packets_sent += num_packets;
    rcidm_tick(rcidm);
}

void ossl_quic_rcidm_request_roll(QUIC_RCIDM *rcidm)
{
    rcidm->roll_requested = 1;
    rcidm_tick(rcidm);
}

/*
 * Mutation Operations
 * ===================
 */
int ossl_quic_rcidm_add_from_initial(QUIC_RCIDM *rcidm,
                                     const QUIC_CONN_ID *rcid)
{
    RCID *rcid_obj;

    if (rcidm->added_initial_rcid || rcidm->handshake_complete)
        return 0;

    rcid_obj = rcidm_create_rcid(rcidm, INITIAL_SEQ_NUM,
                                 rcid, RCID_TYPE_INITIAL);
    if (rcid_obj == NULL)
        return 0;

    rcidm->added_initial_rcid = 1;
    rcidm_tick(rcidm);
    return 1;
}

int ossl_quic_rcidm_add_from_server_retry(QUIC_RCIDM *rcidm,
                                          const QUIC_CONN_ID *retry_odcid)
{
    if (rcidm->added_retry_odcid || rcidm->handshake_complete)
        return 0;

    rcidm->retry_odcid          = *retry_odcid;
    rcidm->added_retry_odcid    = 1;
    rcidm_tick(rcidm);
    return 1;
}

int ossl_quic_rcidm_add_from_ncid(QUIC_RCIDM *rcidm,
                                  const OSSL_QUIC_FRAME_NEW_CONN_ID *ncid)
{
    RCID *rcid;

    rcid = rcidm_create_rcid(rcidm, ncid->seq_num, &ncid->conn_id, RCID_TYPE_NCID);
    if (rcid == NULL)
        return 0;

    rcidm_handle_retire_prior_to(rcidm, ncid->retire_prior_to);
    rcidm_tick(rcidm);
    return 1;
}

/*
 * Queries
 * =======
 */

static int rcidm_get_retire(QUIC_RCIDM *rcidm, uint64_t *seq_num, int peek)
{
    RCID *rcid = ossl_list_retiring_head(&rcidm->retiring_list);

    if (rcid == NULL)
        return 0;

    if (seq_num != NULL)
        *seq_num = rcid->seq_num;

    if (!peek)
        rcidm_free_rcid(rcidm, rcid);

    return 1;
}

int ossl_quic_rcidm_pop_retire_seq_num(QUIC_RCIDM *rcidm,
                                       uint64_t *seq_num)
{
    return rcidm_get_retire(rcidm, seq_num, /*peek=*/0);
}

int ossl_quic_rcidm_peek_retire_seq_num(QUIC_RCIDM *rcidm,
                                        uint64_t *seq_num)
{
    return rcidm_get_retire(rcidm, seq_num, /*peek=*/1);
}

int ossl_quic_rcidm_get_preferred_tx_dcid(QUIC_RCIDM *rcidm,
                                          QUIC_CONN_ID *tx_dcid)
{
    if (!rcidm->have_preferred_rcid)
        return 0;

    *tx_dcid = rcidm->preferred_rcid;
    return 1;
}

int ossl_quic_rcidm_get_preferred_tx_dcid_changed(QUIC_RCIDM *rcidm,
                                                  int clear)
{
    int r = rcidm->preferred_rcid_changed;

    if (clear)
        rcidm->preferred_rcid_changed = 0;

    return r;
}

size_t ossl_quic_rcidm_get_num_active(const QUIC_RCIDM *rcidm)
{
    return ossl_pqueue_RCID_num(rcidm->rcids)
        + (rcidm->cur_rcid != NULL ? 1 : 0)
        + ossl_quic_rcidm_get_num_retiring(rcidm);
}

size_t ossl_quic_rcidm_get_num_retiring(const QUIC_RCIDM *rcidm)
{
    return rcidm->num_retiring;
}
