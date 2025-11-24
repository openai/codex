/*
* Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#include "internal/quic_stream_map.h"
#include "internal/nelem.h"

/*
 * QUIC Stream Map
 * ===============
 */
DEFINE_LHASH_OF_EX(QUIC_STREAM);

static void shutdown_flush_done(QUIC_STREAM_MAP *qsm, QUIC_STREAM *qs);

/* Circular list management. */
static void list_insert_tail(QUIC_STREAM_LIST_NODE *l,
                             QUIC_STREAM_LIST_NODE *n)
{
    /* Must not be in list. */
    assert(n->prev == NULL && n->next == NULL
           && l->prev != NULL && l->next != NULL);

    n->prev = l->prev;
    n->prev->next = n;
    l->prev = n;
    n->next = l;
}

static void list_remove(QUIC_STREAM_LIST_NODE *l,
                        QUIC_STREAM_LIST_NODE *n)
{
    assert(n->prev != NULL && n->next != NULL
           && n->prev != n && n->next != n);

    n->prev->next = n->next;
    n->next->prev = n->prev;
    n->next = n->prev = NULL;
}

static QUIC_STREAM *list_next(QUIC_STREAM_LIST_NODE *l, QUIC_STREAM_LIST_NODE *n,
                              size_t off)
{
    assert(n->prev != NULL && n->next != NULL
           && (n == l || (n->prev != n && n->next != n))
           && l->prev != NULL && l->next != NULL);

    n = n->next;

    if (n == l)
        n = n->next;
    if (n == l)
        return NULL;

    assert(n != NULL);

    return (QUIC_STREAM *)(((char *)n) - off);
}

#define active_next(l, s)       list_next((l), &(s)->active_node, \
                                          offsetof(QUIC_STREAM, active_node))
#define accept_next(l, s)       list_next((l), &(s)->accept_node, \
                                          offsetof(QUIC_STREAM, accept_node))
#define ready_for_gc_next(l, s) list_next((l), &(s)->ready_for_gc_node, \
                                          offsetof(QUIC_STREAM, ready_for_gc_node))
#define accept_head(l)          list_next((l), (l), \
                                          offsetof(QUIC_STREAM, accept_node))
#define ready_for_gc_head(l)    list_next((l), (l), \
                                          offsetof(QUIC_STREAM, ready_for_gc_node))

static unsigned long hash_stream(const QUIC_STREAM *s)
{
    return (unsigned long)s->id;
}

static int cmp_stream(const QUIC_STREAM *a, const QUIC_STREAM *b)
{
    if (a->id < b->id)
        return -1;
    if (a->id > b->id)
        return 1;
    return 0;
}

int ossl_quic_stream_map_init(QUIC_STREAM_MAP *qsm,
                              uint64_t (*get_stream_limit_cb)(int uni, void *arg),
                              void *get_stream_limit_cb_arg,
                              QUIC_RXFC *max_streams_bidi_rxfc,
                              QUIC_RXFC *max_streams_uni_rxfc,
                              int is_server)
{
    qsm->map = lh_QUIC_STREAM_new(hash_stream, cmp_stream);
    qsm->active_list.prev = qsm->active_list.next = &qsm->active_list;
    qsm->accept_list.prev = qsm->accept_list.next = &qsm->accept_list;
    qsm->ready_for_gc_list.prev = qsm->ready_for_gc_list.next
        = &qsm->ready_for_gc_list;
    qsm->rr_stepping = 1;
    qsm->rr_counter  = 0;
    qsm->rr_cur      = NULL;

    qsm->num_accept_bidi    = 0;
    qsm->num_accept_uni     = 0;
    qsm->num_shutdown_flush = 0;

    qsm->get_stream_limit_cb        = get_stream_limit_cb;
    qsm->get_stream_limit_cb_arg    = get_stream_limit_cb_arg;
    qsm->max_streams_bidi_rxfc      = max_streams_bidi_rxfc;
    qsm->max_streams_uni_rxfc       = max_streams_uni_rxfc;
    qsm->is_server                  = is_server;
    return 1;
}

static void release_each(QUIC_STREAM *stream, void *arg)
{
    QUIC_STREAM_MAP *qsm = arg;

    ossl_quic_stream_map_release(qsm, stream);
}

void ossl_quic_stream_map_cleanup(QUIC_STREAM_MAP *qsm)
{
    ossl_quic_stream_map_visit(qsm, release_each, qsm);

    lh_QUIC_STREAM_free(qsm->map);
    qsm->map = NULL;
}

void ossl_quic_stream_map_visit(QUIC_STREAM_MAP *qsm,
                                void (*visit_cb)(QUIC_STREAM *stream, void *arg),
                                void *visit_cb_arg)
{
    lh_QUIC_STREAM_doall_arg(qsm->map, visit_cb, visit_cb_arg);
}

QUIC_STREAM *ossl_quic_stream_map_alloc(QUIC_STREAM_MAP *qsm,
                                        uint64_t stream_id,
                                        int type)
{
    QUIC_STREAM *s;
    QUIC_STREAM key;

    key.id = stream_id;

    s = lh_QUIC_STREAM_retrieve(qsm->map, &key);
    if (s != NULL)
        return NULL;

    s = OPENSSL_zalloc(sizeof(*s));
    if (s == NULL)
        return NULL;

    s->id           = stream_id;
    s->type         = type;
    s->as_server    = qsm->is_server;
    s->send_state   = (ossl_quic_stream_is_local_init(s)
                       || ossl_quic_stream_is_bidi(s))
        ? QUIC_SSTREAM_STATE_READY
        : QUIC_SSTREAM_STATE_NONE;
    s->recv_state   = (!ossl_quic_stream_is_local_init(s)
                       || ossl_quic_stream_is_bidi(s))
        ? QUIC_RSTREAM_STATE_RECV
        : QUIC_RSTREAM_STATE_NONE;

    s->send_final_size  = UINT64_MAX;

    lh_QUIC_STREAM_insert(qsm->map, s);
    return s;
}

void ossl_quic_stream_map_release(QUIC_STREAM_MAP *qsm, QUIC_STREAM *stream)
{
    if (stream == NULL)
        return;

    if (stream->active_node.next != NULL)
        list_remove(&qsm->active_list, &stream->active_node);
    if (stream->accept_node.next != NULL)
        list_remove(&qsm->accept_list, &stream->accept_node);
    if (stream->ready_for_gc_node.next != NULL)
        list_remove(&qsm->ready_for_gc_list, &stream->ready_for_gc_node);

    ossl_quic_sstream_free(stream->sstream);
    stream->sstream = NULL;

    ossl_quic_rstream_free(stream->rstream);
    stream->rstream = NULL;

    lh_QUIC_STREAM_delete(qsm->map, stream);
    OPENSSL_free(stream);
}

QUIC_STREAM *ossl_quic_stream_map_get_by_id(QUIC_STREAM_MAP *qsm,
                                            uint64_t stream_id)
{
    QUIC_STREAM key;

    key.id = stream_id;

    return lh_QUIC_STREAM_retrieve(qsm->map, &key);
}

static void stream_map_mark_active(QUIC_STREAM_MAP *qsm, QUIC_STREAM *s)
{
    if (s->active)
        return;

    list_insert_tail(&qsm->active_list, &s->active_node);

    if (qsm->rr_cur == NULL)
        qsm->rr_cur = s;

    s->active = 1;
}

static void stream_map_mark_inactive(QUIC_STREAM_MAP *qsm, QUIC_STREAM *s)
{
    if (!s->active)
        return;

    if (qsm->rr_cur == s)
        qsm->rr_cur = active_next(&qsm->active_list, s);
    if (qsm->rr_cur == s)
        qsm->rr_cur = NULL;

    list_remove(&qsm->active_list, &s->active_node);

    s->active = 0;
}

void ossl_quic_stream_map_set_rr_stepping(QUIC_STREAM_MAP *qsm, size_t stepping)
{
    qsm->rr_stepping = stepping;
    qsm->rr_counter  = 0;
}

static int stream_has_data_to_send(QUIC_STREAM *s)
{
    OSSL_QUIC_FRAME_STREAM shdr;
    OSSL_QTX_IOVEC iov[2];
    size_t num_iov;
    uint64_t fc_credit, fc_swm, fc_limit;

    switch (s->send_state) {
    case QUIC_SSTREAM_STATE_READY:
    case QUIC_SSTREAM_STATE_SEND:
    case QUIC_SSTREAM_STATE_DATA_SENT:
        /*
         * We can still have data to send in DATA_SENT due to retransmissions,
         * etc.
         */
        break;
    default:
        return 0; /* Nothing to send. */
    }

    /*
     * We cannot determine if we have data to send simply by checking if
     * ossl_quic_txfc_get_credit() is zero, because we may also have older
     * stream data we need to retransmit. The SSTREAM returns older data first,
     * so we do a simple comparison of the next chunk the SSTREAM wants to send
     * against the TXFC CWM.
     */
    num_iov = OSSL_NELEM(iov);
    if (!ossl_quic_sstream_get_stream_frame(s->sstream, 0, &shdr, iov,
                                            &num_iov))
        return 0;

    fc_credit = ossl_quic_txfc_get_credit(&s->txfc, 0);
    fc_swm    = ossl_quic_txfc_get_swm(&s->txfc);
    fc_limit  = fc_swm + fc_credit;

    return (shdr.is_fin && shdr.len == 0) || shdr.offset < fc_limit;
}

static ossl_unused int qsm_send_part_permits_gc(const QUIC_STREAM *qs)
{
    switch (qs->send_state) {
    case QUIC_SSTREAM_STATE_NONE:
    case QUIC_SSTREAM_STATE_DATA_RECVD:
    case QUIC_SSTREAM_STATE_RESET_RECVD:
        return 1;
    default:
        return 0;
    }
}

static int qsm_ready_for_gc(QUIC_STREAM_MAP *qsm, QUIC_STREAM *qs)
{
    int recv_stream_fully_drained = 0; /* TODO(QUIC FUTURE): Optimisation */

    /*
     * If sstream has no FIN, we auto-reset it at marked-for-deletion time, so
     * we don't need to worry about that here.
     */
    assert(!qs->deleted
           || !ossl_quic_stream_has_send(qs)
           || ossl_quic_stream_send_is_reset(qs)
           || ossl_quic_stream_send_get_final_size(qs, NULL));

    return
        qs->deleted
        && (!ossl_quic_stream_has_recv(qs)
            || recv_stream_fully_drained
            || qs->acked_stop_sending)
        && (!ossl_quic_stream_has_send(qs)
            || qs->send_state == QUIC_SSTREAM_STATE_DATA_RECVD
            || qs->send_state == QUIC_SSTREAM_STATE_RESET_RECVD);
}

int ossl_quic_stream_map_is_local_allowed_by_stream_limit(QUIC_STREAM_MAP *qsm,
                                                          uint64_t stream_ordinal,
                                                          int is_uni)
{
    uint64_t stream_limit;

    if (qsm->get_stream_limit_cb == NULL)
        return 1;

    stream_limit = qsm->get_stream_limit_cb(is_uni, qsm->get_stream_limit_cb_arg);
    return stream_ordinal < stream_limit;
}

void ossl_quic_stream_map_update_state(QUIC_STREAM_MAP *qsm, QUIC_STREAM *s)
{
    int should_be_active, allowed_by_stream_limit = 1;

    if (ossl_quic_stream_is_server_init(s) == qsm->is_server) {
        int is_uni = !ossl_quic_stream_is_bidi(s);
        uint64_t stream_ordinal = s->id >> 2;

        allowed_by_stream_limit
            = ossl_quic_stream_map_is_local_allowed_by_stream_limit(qsm,
                                                                    stream_ordinal,
                                                                    is_uni);
    }

    if (s->send_state == QUIC_SSTREAM_STATE_DATA_SENT
        && ossl_quic_sstream_is_totally_acked(s->sstream))
        ossl_quic_stream_map_notify_totally_acked(qsm, s);
    else if (s->shutdown_flush
             && s->send_state == QUIC_SSTREAM_STATE_SEND
             && ossl_quic_sstream_is_totally_acked(s->sstream))
        shutdown_flush_done(qsm, s);

    if (!s->ready_for_gc) {
        s->ready_for_gc = qsm_ready_for_gc(qsm, s);
        if (s->ready_for_gc)
            list_insert_tail(&qsm->ready_for_gc_list, &s->ready_for_gc_node);
    }

    should_be_active
        = allowed_by_stream_limit
        && !s->ready_for_gc
        && ((ossl_quic_stream_has_recv(s)
             && !ossl_quic_stream_recv_is_reset(s)
             && (s->recv_state == QUIC_RSTREAM_STATE_RECV
                 && (s->want_max_stream_data
                     || ossl_quic_rxfc_has_cwm_changed(&s->rxfc, 0))))
            || s->want_stop_sending
            || s->want_reset_stream
            || (!s->peer_stop_sending && stream_has_data_to_send(s)));

    if (should_be_active)
        stream_map_mark_active(qsm, s);
    else
        stream_map_mark_inactive(qsm, s);
}

/*
 * Stream Send Part State Management
 * =================================
 */

int ossl_quic_stream_map_ensure_send_part_id(QUIC_STREAM_MAP *qsm,
                                             QUIC_STREAM *qs)
{
    switch (qs->send_state) {
    case QUIC_SSTREAM_STATE_NONE:
        /* Stream without send part - caller error. */
        return 0;

    case QUIC_SSTREAM_STATE_READY:
        /*
         * We always allocate a stream ID upfront, so we don't need to do it
         * here.
         */
        qs->send_state = QUIC_SSTREAM_STATE_SEND;
        return 1;

    default:
        /* Nothing to do. */
        return 1;
    }
}

int ossl_quic_stream_map_notify_all_data_sent(QUIC_STREAM_MAP *qsm,
                                              QUIC_STREAM *qs)
{
    switch (qs->send_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_SSTREAM_STATE_NONE:
        /* Stream without send part - caller error. */
        return 0;

    case QUIC_SSTREAM_STATE_SEND:
        if (!ossl_quic_sstream_get_final_size(qs->sstream, &qs->send_final_size))
            return 0;

        qs->send_state = QUIC_SSTREAM_STATE_DATA_SENT;
        return 1;
    }
}

static void shutdown_flush_done(QUIC_STREAM_MAP *qsm, QUIC_STREAM *qs)
{
    if (!qs->shutdown_flush)
        return;

    assert(qsm->num_shutdown_flush > 0);
    qs->shutdown_flush = 0;
    --qsm->num_shutdown_flush;
}

int ossl_quic_stream_map_notify_totally_acked(QUIC_STREAM_MAP *qsm,
                                              QUIC_STREAM *qs)
{
    switch (qs->send_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_SSTREAM_STATE_NONE:
        /* Stream without send part - caller error. */
        return 0;

    case QUIC_SSTREAM_STATE_DATA_SENT:
        qs->send_state = QUIC_SSTREAM_STATE_DATA_RECVD;
        /* We no longer need a QUIC_SSTREAM in this state. */
        ossl_quic_sstream_free(qs->sstream);
        qs->sstream = NULL;

        shutdown_flush_done(qsm, qs);
        return 1;
    }
}

int ossl_quic_stream_map_reset_stream_send_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t aec)
{
    switch (qs->send_state) {
    default:
    case QUIC_SSTREAM_STATE_NONE:
        /*
         * RESET_STREAM pertains to sending part only, so we cannot reset a
         * receive-only stream.
         */
    case QUIC_SSTREAM_STATE_DATA_RECVD:
        /*
         * RFC 9000 s. 3.3: A sender MUST NOT [...] send RESET_STREAM from a
         * terminal state. If the stream has already finished normally and the
         * peer has acknowledged this, we cannot reset it.
         */
        return 0;

    case QUIC_SSTREAM_STATE_READY:
        if (!ossl_quic_stream_map_ensure_send_part_id(qsm, qs))
            return 0;

        /* FALLTHROUGH */
    case QUIC_SSTREAM_STATE_SEND:
        /*
         * If we already have a final size (e.g. because we are coming from
         * DATA_SENT), we have to be consistent with that, so don't change it.
         * If we don't already have a final size, determine a final size value.
         * This is the value which we will end up using for a RESET_STREAM frame
         * for flow control purposes. We could send the stream size (total
         * number of bytes appended to QUIC_SSTREAM by the application), but it
         * is in our interest to exclude any bytes we have not actually
         * transmitted yet, to avoid unnecessarily consuming flow control
         * credit. We can get this from the TXFC.
         */
        qs->send_final_size = ossl_quic_txfc_get_swm(&qs->txfc);

        /* FALLTHROUGH */
    case QUIC_SSTREAM_STATE_DATA_SENT:
        qs->reset_stream_aec    = aec;
        qs->want_reset_stream   = 1;
        qs->send_state          = QUIC_SSTREAM_STATE_RESET_SENT;

        ossl_quic_sstream_free(qs->sstream);
        qs->sstream = NULL;

        shutdown_flush_done(qsm, qs);
        ossl_quic_stream_map_update_state(qsm, qs);
        return 1;

    case QUIC_SSTREAM_STATE_RESET_SENT:
    case QUIC_SSTREAM_STATE_RESET_RECVD:
        /*
         * Idempotent - no-op. In any case, do not send RESET_STREAM again - as
         * mentioned, we must not send it from a terminal state.
         */
        return 1;
    }
}

int ossl_quic_stream_map_notify_reset_stream_acked(QUIC_STREAM_MAP *qsm,
                                                   QUIC_STREAM *qs)
{
    switch (qs->send_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_SSTREAM_STATE_NONE:
        /* Stream without send part - caller error. */
         return 0;

    case QUIC_SSTREAM_STATE_RESET_SENT:
        qs->send_state = QUIC_SSTREAM_STATE_RESET_RECVD;
        return 1;

    case QUIC_SSTREAM_STATE_RESET_RECVD:
        /* Already in the correct state. */
        return 1;
    }
}

/*
 * Stream Receive Part State Management
 * ====================================
 */

int ossl_quic_stream_map_notify_size_known_recv_part(QUIC_STREAM_MAP *qsm,
                                                     QUIC_STREAM *qs,
                                                     uint64_t final_size)
{
    switch (qs->recv_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_RSTREAM_STATE_NONE:
        /* Stream without receive part - caller error. */
        return 0;

    case QUIC_RSTREAM_STATE_RECV:
        qs->recv_state = QUIC_RSTREAM_STATE_SIZE_KNOWN;
        return 1;
    }
}

int ossl_quic_stream_map_notify_totally_received(QUIC_STREAM_MAP *qsm,
                                                 QUIC_STREAM *qs)
{
    switch (qs->recv_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_RSTREAM_STATE_NONE:
        /* Stream without receive part - caller error. */
        return 0;

    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
        qs->recv_state          = QUIC_RSTREAM_STATE_DATA_RECVD;
        qs->want_stop_sending   = 0;
        return 1;
    }
}

int ossl_quic_stream_map_notify_totally_read(QUIC_STREAM_MAP *qsm,
                                             QUIC_STREAM *qs)
{
    switch (qs->recv_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_RSTREAM_STATE_NONE:
        /* Stream without receive part - caller error. */
        return 0;

    case QUIC_RSTREAM_STATE_DATA_RECVD:
        qs->recv_state = QUIC_RSTREAM_STATE_DATA_READ;

        /* QUIC_RSTREAM is no longer needed */
        ossl_quic_rstream_free(qs->rstream);
        qs->rstream = NULL;
        return 1;
    }
}

int ossl_quic_stream_map_notify_reset_recv_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t app_error_code,
                                                uint64_t final_size)
{
    uint64_t prev_final_size;

    switch (qs->recv_state) {
    default:
    case QUIC_RSTREAM_STATE_NONE:
        /* Stream without receive part - caller error. */
        return 0;

    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
    case QUIC_RSTREAM_STATE_DATA_RECVD:
        if (ossl_quic_stream_recv_get_final_size(qs, &prev_final_size)
            && prev_final_size != final_size)
            /* Cannot change previous final size. */
            return 0;

        qs->recv_state              = QUIC_RSTREAM_STATE_RESET_RECVD;
        qs->peer_reset_stream_aec   = app_error_code;

        /* RFC 9000 s. 3.3: No point sending STOP_SENDING if already reset. */
        qs->want_stop_sending       = 0;

        /* QUIC_RSTREAM is no longer needed */
        ossl_quic_rstream_free(qs->rstream);
        qs->rstream = NULL;

        ossl_quic_stream_map_update_state(qsm, qs);
        return 1;

    case QUIC_RSTREAM_STATE_DATA_READ:
        /*
         * If we already retired the FIN to the application this is moot
         * - just ignore.
         */
    case QUIC_RSTREAM_STATE_RESET_RECVD:
    case QUIC_RSTREAM_STATE_RESET_READ:
        /* Could be a reordered/retransmitted frame - just ignore. */
        return 1;
    }
}

int ossl_quic_stream_map_notify_app_read_reset_recv_part(QUIC_STREAM_MAP *qsm,
                                                         QUIC_STREAM *qs)
{
    switch (qs->recv_state) {
    default:
        /* Wrong state - caller error. */
    case QUIC_RSTREAM_STATE_NONE:
        /* Stream without receive part - caller error. */
        return 0;

    case QUIC_RSTREAM_STATE_RESET_RECVD:
        qs->recv_state = QUIC_RSTREAM_STATE_RESET_READ;
        return 1;
    }
}

int ossl_quic_stream_map_stop_sending_recv_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t aec)
{
    if (qs->stop_sending)
        return 0;

    switch (qs->recv_state) {
    default:
    case QUIC_RSTREAM_STATE_NONE:
        /* Send-only stream, so this makes no sense. */
    case QUIC_RSTREAM_STATE_DATA_RECVD:
    case QUIC_RSTREAM_STATE_DATA_READ:
        /*
         * Not really any point in STOP_SENDING if we already received all data.
         */
    case QUIC_RSTREAM_STATE_RESET_RECVD:
    case QUIC_RSTREAM_STATE_RESET_READ:
        /*
         * RFC 9000 s. 3.5: "STOP_SENDING SHOULD only be sent for a stream that
         * has not been reset by the peer."
         *
         * No point in STOP_SENDING if the peer already reset their send part.
         */
        return 0;

    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
        /*
         * RFC 9000 s. 3.5: "If the stream is in the Recv or Size Known state,
         * the transport SHOULD signal this by sending a STOP_SENDING frame to
         * prompt closure of the stream in the opposite direction."
         *
         * Note that it does make sense to send STOP_SENDING for a receive part
         * of a stream which has a known size (because we have received a FIN)
         * but which still has other (previous) stream data yet to be received.
         */
        break;
    }

    qs->stop_sending        = 1;
    qs->stop_sending_aec    = aec;
    return ossl_quic_stream_map_schedule_stop_sending(qsm, qs);
}

/* Called to mark STOP_SENDING for generation, or regeneration after loss. */
int ossl_quic_stream_map_schedule_stop_sending(QUIC_STREAM_MAP *qsm, QUIC_STREAM *qs)
{
    if (!qs->stop_sending)
        return 0;

    /*
     * Ignore the call as a no-op if already scheduled, or in a state
     * where it makes no sense to send STOP_SENDING.
     */
    if (qs->want_stop_sending)
        return 1;

    switch (qs->recv_state) {
    default:
        return 1; /* ignore */
    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
        /*
         * RFC 9000 s. 3.5: "An endpoint is expected to send another
         * STOP_SENDING frame if a packet containing a previous STOP_SENDING is
         * lost. However, once either all stream data or a RESET_STREAM frame
         * has been received for the stream -- that is, the stream is in any
         * state other than "Recv" or "Size Known" -- sending a STOP_SENDING
         * frame is unnecessary."
         */
        break;
    }

    qs->want_stop_sending = 1;
    ossl_quic_stream_map_update_state(qsm, qs);
    return 1;
}

QUIC_STREAM *ossl_quic_stream_map_peek_accept_queue(QUIC_STREAM_MAP *qsm)
{
    return accept_head(&qsm->accept_list);
}

void ossl_quic_stream_map_push_accept_queue(QUIC_STREAM_MAP *qsm,
                                            QUIC_STREAM *s)
{
    list_insert_tail(&qsm->accept_list, &s->accept_node);
    if (ossl_quic_stream_is_bidi(s))
        ++qsm->num_accept_bidi;
    else
        ++qsm->num_accept_uni;
}

static QUIC_RXFC *qsm_get_max_streams_rxfc(QUIC_STREAM_MAP *qsm, QUIC_STREAM *s)
{
    return ossl_quic_stream_is_bidi(s)
        ? qsm->max_streams_bidi_rxfc
        : qsm->max_streams_uni_rxfc;
}

void ossl_quic_stream_map_remove_from_accept_queue(QUIC_STREAM_MAP *qsm,
                                                   QUIC_STREAM *s,
                                                   OSSL_TIME rtt)
{
    QUIC_RXFC *max_streams_rxfc;

    list_remove(&qsm->accept_list, &s->accept_node);
    if (ossl_quic_stream_is_bidi(s))
        --qsm->num_accept_bidi;
    else
        --qsm->num_accept_uni;

    if ((max_streams_rxfc = qsm_get_max_streams_rxfc(qsm, s)) != NULL)
        (void)ossl_quic_rxfc_on_retire(max_streams_rxfc, 1, rtt);
}

size_t ossl_quic_stream_map_get_accept_queue_len(QUIC_STREAM_MAP *qsm, int is_uni)
{
    return is_uni ? qsm->num_accept_uni : qsm->num_accept_bidi;
}

size_t ossl_quic_stream_map_get_total_accept_queue_len(QUIC_STREAM_MAP *qsm)
{
    return ossl_quic_stream_map_get_accept_queue_len(qsm, /*is_uni=*/0)
        + ossl_quic_stream_map_get_accept_queue_len(qsm, /*is_uni=*/1);
}

void ossl_quic_stream_map_gc(QUIC_STREAM_MAP *qsm)
{
    QUIC_STREAM *qs, *qs_head, *qsn = NULL;

    for (qs = qs_head = ready_for_gc_head(&qsm->ready_for_gc_list);
         qs != NULL && qs != qs_head;
         qs = qsn)
    {
         qsn = ready_for_gc_next(&qsm->ready_for_gc_list, qs);

         ossl_quic_stream_map_release(qsm, qs);
    }
}

static int eligible_for_shutdown_flush(QUIC_STREAM *qs)
{
    /*
     * We only care about servicing the send part of a stream (if any) during
     * shutdown flush. We make sure we flush a stream if it is either
     * non-terminated or was terminated normally such as via
     * SSL_stream_conclude. A stream which was terminated via a reset is not
     * flushed, and we will have thrown away the send buffer in that case
     * anyway.
     */
    switch (qs->send_state) {
    case QUIC_SSTREAM_STATE_SEND:
    case QUIC_SSTREAM_STATE_DATA_SENT:
        return !ossl_quic_sstream_is_totally_acked(qs->sstream);
    default:
        return 0;
    }
}

static void begin_shutdown_flush_each(QUIC_STREAM *qs, void *arg)
{
    QUIC_STREAM_MAP *qsm = arg;

    if (!eligible_for_shutdown_flush(qs) || qs->shutdown_flush)
        return;

    qs->shutdown_flush = 1;
    ++qsm->num_shutdown_flush;
}

void ossl_quic_stream_map_begin_shutdown_flush(QUIC_STREAM_MAP *qsm)
{
    qsm->num_shutdown_flush = 0;

    ossl_quic_stream_map_visit(qsm, begin_shutdown_flush_each, qsm);
}

int ossl_quic_stream_map_is_shutdown_flush_finished(QUIC_STREAM_MAP *qsm)
{
    return qsm->num_shutdown_flush == 0;
}

/*
 * QUIC Stream Iterator
 * ====================
 */
void ossl_quic_stream_iter_init(QUIC_STREAM_ITER *it, QUIC_STREAM_MAP *qsm,
                                int advance_rr)
{
    it->qsm    = qsm;
    it->stream = it->first_stream = qsm->rr_cur;
    if (advance_rr && it->stream != NULL
        && ++qsm->rr_counter >= qsm->rr_stepping) {
        qsm->rr_counter = 0;
        qsm->rr_cur     = active_next(&qsm->active_list, qsm->rr_cur);
    }
}

void ossl_quic_stream_iter_next(QUIC_STREAM_ITER *it)
{
    if (it->stream == NULL)
        return;

    it->stream = active_next(&it->qsm->active_list, it->stream);
    if (it->stream == it->first_stream)
        it->stream = NULL;
}
