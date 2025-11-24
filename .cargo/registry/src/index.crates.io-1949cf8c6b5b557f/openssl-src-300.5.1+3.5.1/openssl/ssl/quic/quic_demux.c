/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_demux.h"
#include "internal/quic_wire_pkt.h"
#include "internal/common.h"
#include <openssl/lhash.h>
#include <openssl/err.h>

#define URXE_DEMUX_STATE_FREE       0 /* on urx_free list */
#define URXE_DEMUX_STATE_PENDING    1 /* on urx_pending list */
#define URXE_DEMUX_STATE_ISSUED     2 /* on neither list */

#define DEMUX_MAX_MSGS_PER_CALL    32

#define DEMUX_DEFAULT_MTU        1500

struct quic_demux_st {
    /* The underlying transport BIO with datagram semantics. */
    BIO                        *net_bio;

    /*
     * QUIC short packets do not contain the length of the connection ID field,
     * therefore it must be known contextually. The demuxer requires connection
     * IDs of the same length to be used for all incoming packets.
     */
    size_t                      short_conn_id_len;

    /*
     * Our current understanding of the upper bound on an incoming datagram size
     * in bytes.
     */
    size_t                      mtu;

    /* The datagram_id to use for the next datagram we receive. */
    uint64_t                    next_datagram_id;

    /* Time retrieval callback. */
    OSSL_TIME                 (*now)(void *arg);
    void                       *now_arg;

    /* The default packet handler, if any. */
    ossl_quic_demux_cb_fn      *default_cb;
    void                       *default_cb_arg;

    /*
     * List of URXEs which are not currently in use (i.e., not filled with
     * unconsumed data). These are moved to the pending list as they are filled.
     */
    QUIC_URXE_LIST              urx_free;

    /*
     * List of URXEs which are filled with received encrypted data. These are
     * removed from this list as we invoke the callbacks for each of them. They
     * are then not on any list managed by us; we forget about them until our
     * user calls ossl_quic_demux_release_urxe to return the URXE to us, at
     * which point we add it to the free list.
     */
    QUIC_URXE_LIST              urx_pending;

    /* Whether to use local address support. */
    char                        use_local_addr;
};

QUIC_DEMUX *ossl_quic_demux_new(BIO *net_bio,
                                size_t short_conn_id_len,
                                OSSL_TIME (*now)(void *arg),
                                void *now_arg)
{
    QUIC_DEMUX *demux;

    demux = OPENSSL_zalloc(sizeof(QUIC_DEMUX));
    if (demux == NULL)
        return NULL;

    demux->net_bio                  = net_bio;
    demux->short_conn_id_len        = short_conn_id_len;
    /* We update this if possible when we get a BIO. */
    demux->mtu                      = DEMUX_DEFAULT_MTU;
    demux->now                      = now;
    demux->now_arg                  = now_arg;

    if (net_bio != NULL
        && BIO_dgram_get_local_addr_cap(net_bio)
        && BIO_dgram_set_local_addr_enable(net_bio, 1))
        demux->use_local_addr = 1;

    return demux;
}

static void demux_free_urxl(QUIC_URXE_LIST *l)
{
    QUIC_URXE *e, *enext;

    for (e = ossl_list_urxe_head(l); e != NULL; e = enext) {
        enext = ossl_list_urxe_next(e);
        ossl_list_urxe_remove(l, e);
        OPENSSL_free(e);
    }
}

void ossl_quic_demux_free(QUIC_DEMUX *demux)
{
    if (demux == NULL)
        return;

    /* Free all URXEs we are holding. */
    demux_free_urxl(&demux->urx_free);
    demux_free_urxl(&demux->urx_pending);

    OPENSSL_free(demux);
}

void ossl_quic_demux_set_bio(QUIC_DEMUX *demux, BIO *net_bio)
{
    unsigned int mtu;

    demux->net_bio = net_bio;

    if (net_bio != NULL) {
        /*
         * Try to determine our MTU if possible. The BIO is not required to
         * support this, in which case we remain at the last known MTU, or our
         * initial default.
         */
        mtu = BIO_dgram_get_mtu(net_bio);
        if (mtu >= QUIC_MIN_INITIAL_DGRAM_LEN)
            ossl_quic_demux_set_mtu(demux, mtu); /* best effort */
    }
}

int ossl_quic_demux_set_mtu(QUIC_DEMUX *demux, unsigned int mtu)
{
    if (mtu < QUIC_MIN_INITIAL_DGRAM_LEN)
        return 0;

    demux->mtu = mtu;
    return 1;
}

void ossl_quic_demux_set_default_handler(QUIC_DEMUX *demux,
                                         ossl_quic_demux_cb_fn *cb,
                                         void *cb_arg)
{
    demux->default_cb       = cb;
    demux->default_cb_arg   = cb_arg;
}

static QUIC_URXE *demux_alloc_urxe(size_t alloc_len)
{
    QUIC_URXE *e;

    if (alloc_len >= SIZE_MAX - sizeof(QUIC_URXE))
        return NULL;

    e = OPENSSL_malloc(sizeof(QUIC_URXE) + alloc_len);
    if (e == NULL)
        return NULL;

    ossl_list_urxe_init_elem(e);
    e->alloc_len   = alloc_len;
    e->data_len    = 0;
    return e;
}

static QUIC_URXE *demux_resize_urxe(QUIC_DEMUX *demux, QUIC_URXE *e,
                                    size_t new_alloc_len)
{
    QUIC_URXE *e2, *prev;

    if (!ossl_assert(e->demux_state == URXE_DEMUX_STATE_FREE))
        /* Never attempt to resize a URXE which is not on the free list. */
        return NULL;

    prev = ossl_list_urxe_prev(e);
    ossl_list_urxe_remove(&demux->urx_free, e);

    e2 = OPENSSL_realloc(e, sizeof(QUIC_URXE) + new_alloc_len);
    if (e2 == NULL) {
        /* Failed to resize, abort. */
        if (prev == NULL)
            ossl_list_urxe_insert_head(&demux->urx_free, e);
        else
            ossl_list_urxe_insert_after(&demux->urx_free, prev, e);

        return NULL;
    }

    if (prev == NULL)
        ossl_list_urxe_insert_head(&demux->urx_free, e2);
    else
        ossl_list_urxe_insert_after(&demux->urx_free, prev, e2);

    e2->alloc_len = new_alloc_len;
    return e2;
}

static QUIC_URXE *demux_reserve_urxe(QUIC_DEMUX *demux, QUIC_URXE *e,
                                     size_t alloc_len)
{
    return e->alloc_len < alloc_len ? demux_resize_urxe(demux, e, alloc_len) : e;
}

static int demux_ensure_free_urxe(QUIC_DEMUX *demux, size_t min_num_free)
{
    QUIC_URXE *e;

    while (ossl_list_urxe_num(&demux->urx_free) < min_num_free) {
        e = demux_alloc_urxe(demux->mtu);
        if (e == NULL)
            return 0;

        ossl_list_urxe_insert_tail(&demux->urx_free, e);
        e->demux_state = URXE_DEMUX_STATE_FREE;
    }

    return 1;
}

/*
 * Receive datagrams from network, placing them into URXEs.
 *
 * Returns 1 on success or 0 on failure.
 *
 * Precondition: at least one URXE is free
 * Precondition: there are no pending URXEs
 */
static int demux_recv(QUIC_DEMUX *demux)
{
    BIO_MSG msg[DEMUX_MAX_MSGS_PER_CALL];
    size_t rd, i;
    QUIC_URXE *urxe = ossl_list_urxe_head(&demux->urx_free), *unext;
    OSSL_TIME now;

    /* This should never be called when we have any pending URXE. */
    assert(ossl_list_urxe_head(&demux->urx_pending) == NULL);
    assert(urxe->demux_state == URXE_DEMUX_STATE_FREE);

    if (demux->net_bio == NULL)
        /*
         * If no BIO is plugged in, treat this as no datagram being available.
         */
        return QUIC_DEMUX_PUMP_RES_TRANSIENT_FAIL;

    /*
     * Opportunistically receive as many messages as possible in a single
     * syscall, determined by how many free URXEs are available.
     */
    for (i = 0; i < (ossl_ssize_t)OSSL_NELEM(msg);
            ++i, urxe = ossl_list_urxe_next(urxe)) {
        if (urxe == NULL) {
            /* We need at least one URXE to receive into. */
            if (!ossl_assert(i > 0))
                return QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL;

            break;
        }

        /* Ensure the URXE is big enough. */
        urxe = demux_reserve_urxe(demux, urxe, demux->mtu);
        if (urxe == NULL)
            /* Allocation error, fail. */
            return QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL;

        /* Ensure we zero any fields added to BIO_MSG at a later date. */
        memset(&msg[i], 0, sizeof(BIO_MSG));
        msg[i].data     = ossl_quic_urxe_data(urxe);
        msg[i].data_len = urxe->alloc_len;
        msg[i].peer     = &urxe->peer;
        BIO_ADDR_clear(&urxe->peer);
        if (demux->use_local_addr)
            msg[i].local = &urxe->local;
        else
            BIO_ADDR_clear(&urxe->local);
    }

    ERR_set_mark();
    if (!BIO_recvmmsg(demux->net_bio, msg, sizeof(BIO_MSG), i, 0, &rd)) {
        if (BIO_err_is_non_fatal(ERR_peek_last_error())) {
            /* Transient error, clear the error and stop. */
            ERR_pop_to_mark();
            return QUIC_DEMUX_PUMP_RES_TRANSIENT_FAIL;
        } else {
            /* Non-transient error, do not clear the error. */
            ERR_clear_last_mark();
            return QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL;
        }
    }

    ERR_clear_last_mark();
    now = demux->now != NULL ? demux->now(demux->now_arg) : ossl_time_zero();

    urxe = ossl_list_urxe_head(&demux->urx_free);
    for (i = 0; i < rd; ++i, urxe = unext) {
        unext = ossl_list_urxe_next(urxe);
        /* Set URXE with actual length of received datagram. */
        urxe->data_len      = msg[i].data_len;
        /* Time we received datagram. */
        urxe->time          = now;
        urxe->datagram_id   = demux->next_datagram_id++;
        /* Move from free list to pending list. */
        ossl_list_urxe_remove(&demux->urx_free, urxe);
        ossl_list_urxe_insert_tail(&demux->urx_pending, urxe);
        urxe->demux_state = URXE_DEMUX_STATE_PENDING;
    }

    return QUIC_DEMUX_PUMP_RES_OK;
}

/* Extract destination connection ID from the first packet in a datagram. */
static int demux_identify_conn_id(QUIC_DEMUX *demux,
                                  QUIC_URXE *e,
                                  QUIC_CONN_ID *dst_conn_id)
{
    return ossl_quic_wire_get_pkt_hdr_dst_conn_id(ossl_quic_urxe_data(e),
                                                  e->data_len,
                                                  demux->short_conn_id_len,
                                                  dst_conn_id);
}

/*
 * Process a single pending URXE.
 * Returning 1 on success, 0 on failure.
 */
static int demux_process_pending_urxe(QUIC_DEMUX *demux, QUIC_URXE *e)
{
    QUIC_CONN_ID dst_conn_id;
    int dst_conn_id_ok = 0;

    /* The next URXE we process should be at the head of the pending list. */
    if (!ossl_assert(e == ossl_list_urxe_head(&demux->urx_pending)))
        return 0;

    assert(e->demux_state == URXE_DEMUX_STATE_PENDING);

    /* Determine the DCID of the first packet in the datagram. */
    dst_conn_id_ok = demux_identify_conn_id(demux, e, &dst_conn_id);

    ossl_list_urxe_remove(&demux->urx_pending, e);
    if (demux->default_cb != NULL) {
        /*
         * Pass to default handler for routing. The URXE now belongs to the
         * callback.
         */
        e->demux_state = URXE_DEMUX_STATE_ISSUED;
        demux->default_cb(e, demux->default_cb_arg,
                          dst_conn_id_ok ? &dst_conn_id : NULL);
    } else {
        /* Discard. */
        ossl_list_urxe_insert_tail(&demux->urx_free, e);
        e->demux_state = URXE_DEMUX_STATE_FREE;
    }

    return 1; /* keep processing pending URXEs */
}

/* Process pending URXEs to generate callbacks. */
static int demux_process_pending_urxl(QUIC_DEMUX *demux)
{
    QUIC_URXE *e;
    int ret;

    while ((e = ossl_list_urxe_head(&demux->urx_pending)) != NULL)
        if ((ret = demux_process_pending_urxe(demux, e)) <= 0)
            return ret;

    return 1;
}

/*
 * Drain the pending URXE list, processing any pending URXEs by making their
 * callbacks. If no URXEs are pending, a network read is attempted first.
 */
int ossl_quic_demux_pump(QUIC_DEMUX *demux)
{
    int ret;

    if (ossl_list_urxe_head(&demux->urx_pending) == NULL) {
        ret = demux_ensure_free_urxe(demux, DEMUX_MAX_MSGS_PER_CALL);
        if (ret != 1)
            return QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL;

        ret = demux_recv(demux);
        if (ret != QUIC_DEMUX_PUMP_RES_OK)
            return ret;

        /*
         * If demux_recv returned successfully, we should always have something.
         */
        assert(ossl_list_urxe_head(&demux->urx_pending) != NULL);
    }

    if ((ret = demux_process_pending_urxl(demux)) <= 0)
        return QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL;

    return QUIC_DEMUX_PUMP_RES_OK;
}

/* Artificially inject a packet into the demuxer for testing purposes. */
int ossl_quic_demux_inject(QUIC_DEMUX *demux,
                           const unsigned char *buf,
                           size_t buf_len,
                           const BIO_ADDR *peer,
                           const BIO_ADDR *local)
{
    int ret;
    QUIC_URXE *urxe;

    ret = demux_ensure_free_urxe(demux, 1);
    if (ret != 1)
        return 0;

    urxe = ossl_list_urxe_head(&demux->urx_free);

    assert(urxe->demux_state == URXE_DEMUX_STATE_FREE);

    urxe = demux_reserve_urxe(demux, urxe, buf_len);
    if (urxe == NULL)
        return 0;

    memcpy(ossl_quic_urxe_data(urxe), buf, buf_len);
    urxe->data_len = buf_len;

    if (peer != NULL)
        urxe->peer = *peer;
    else
        BIO_ADDR_clear(&urxe->peer);

    if (local != NULL)
        urxe->local = *local;
    else
        BIO_ADDR_clear(&urxe->local);

    urxe->time
        = demux->now != NULL ? demux->now(demux->now_arg) : ossl_time_zero();

    /* Move from free list to pending list. */
    ossl_list_urxe_remove(&demux->urx_free, urxe);
    urxe->datagram_id = demux->next_datagram_id++;
    ossl_list_urxe_insert_tail(&demux->urx_pending, urxe);
    urxe->demux_state = URXE_DEMUX_STATE_PENDING;

    return demux_process_pending_urxl(demux) > 0;
}

/* Called by our user to return a URXE to the free list. */
void ossl_quic_demux_release_urxe(QUIC_DEMUX *demux,
                                  QUIC_URXE *e)
{
    assert(ossl_list_urxe_prev(e) == NULL && ossl_list_urxe_next(e) == NULL);
    assert(e->demux_state == URXE_DEMUX_STATE_ISSUED);
    ossl_list_urxe_insert_tail(&demux->urx_free, e);
    e->demux_state = URXE_DEMUX_STATE_FREE;
}

void ossl_quic_demux_reinject_urxe(QUIC_DEMUX *demux,
                                   QUIC_URXE *e)
{
    assert(ossl_list_urxe_prev(e) == NULL && ossl_list_urxe_next(e) == NULL);
    assert(e->demux_state == URXE_DEMUX_STATE_ISSUED);
    ossl_list_urxe_insert_head(&demux->urx_pending, e);
    e->demux_state = URXE_DEMUX_STATE_PENDING;
}

int ossl_quic_demux_has_pending(const QUIC_DEMUX *demux)
{
    return ossl_list_urxe_head(&demux->urx_pending) != NULL;
}
