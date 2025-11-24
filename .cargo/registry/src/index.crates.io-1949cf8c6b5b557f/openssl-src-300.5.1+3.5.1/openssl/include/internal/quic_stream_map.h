/*
* Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_STREAM_MAP_H
# define OSSL_INTERNAL_QUIC_STREAM_MAP_H
# pragma once

# include "internal/e_os.h"
# include "internal/time.h"
# include "internal/common.h"
# include "internal/quic_types.h"
# include "internal/quic_predef.h"
# include "internal/quic_stream.h"
# include "internal/quic_fc.h"
# include <openssl/lhash.h>

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Stream
 * ===========
 *
 * Logical QUIC stream composing all relevant send and receive components.
 */

typedef struct quic_stream_list_node_st QUIC_STREAM_LIST_NODE;

struct quic_stream_list_node_st {
    QUIC_STREAM_LIST_NODE *prev, *next;
};

/*
 * QUIC Send Stream States
 * -----------------------
 *
 * These correspond to the states defined in RFC 9000 s. 3.1, with the
 * exception of the NONE state which represents the absence of a send stream
 * part.
 *
 * Invariants in each state are noted in comments below. In particular, once all
 * data has been acknowledged received, or we have reset the stream, we don't
 * need to keep the QUIC_SSTREAM and data buffers around. Of course, we also
 * don't have a QUIC_SSTREAM on a receive-only stream.
 */
#define QUIC_SSTREAM_STATE_NONE         0   /* --- sstream == NULL  */
#define QUIC_SSTREAM_STATE_READY        1   /* \                    */
#define QUIC_SSTREAM_STATE_SEND         2   /* |-- sstream != NULL  */
#define QUIC_SSTREAM_STATE_DATA_SENT    3   /* /                    */
#define QUIC_SSTREAM_STATE_DATA_RECVD   4   /* \                    */
#define QUIC_SSTREAM_STATE_RESET_SENT   5   /* |-- sstream == NULL  */
#define QUIC_SSTREAM_STATE_RESET_RECVD  6   /* /                    */

/*
 * QUIC Receive Stream States
 * --------------------------
 *
 * These correspond to the states defined in RFC 9000 s. 3.2, with the exception
 * of the NONE state which represents the absence of a receive stream part.
 *
 * Invariants in each state are noted in comments below. In particular, once all
 * data has been read by the application, we don't need to keep the QUIC_RSTREAM
 * and data buffers around. If the receive part is instead reset before it is
 * finished, we also don't need to keep the QUIC_RSTREAM around. Finally, we
 * don't need a QUIC_RSTREAM on a send-only stream.
 */
#define QUIC_RSTREAM_STATE_NONE         0   /* --- rstream == NULL  */
#define QUIC_RSTREAM_STATE_RECV         1   /* \                    */
#define QUIC_RSTREAM_STATE_SIZE_KNOWN   2   /* |-- rstream != NULL  */
#define QUIC_RSTREAM_STATE_DATA_RECVD   3   /* /                    */
#define QUIC_RSTREAM_STATE_DATA_READ    4   /* \                    */
#define QUIC_RSTREAM_STATE_RESET_RECVD  5   /* |-- rstream == NULL  */
#define QUIC_RSTREAM_STATE_RESET_READ   6   /* /                    */

struct quic_stream_st {
    QUIC_STREAM_LIST_NODE active_node; /* for use by QUIC_STREAM_MAP */
    QUIC_STREAM_LIST_NODE accept_node; /* accept queue of remotely-created streams */
    QUIC_STREAM_LIST_NODE ready_for_gc_node; /* queue of streams now ready for GC */

    /* Temporary link used by TXP. */
    QUIC_STREAM    *txp_next;

    /*
     * QUIC Stream ID. Do not assume that this encodes a type as this is a
     * version-specific property and may change between QUIC versions; instead,
     * use the type field.
     */
    uint64_t        id;

    /*
     * Application Error Code (AEC) used for STOP_SENDING frame.
     * This is only valid if stop_sending is 1.
     */
    uint64_t        stop_sending_aec;

    /*
     * Application Error Code (AEC) used for RESET_STREAM frame.
     * This is only valid if reset_stream is 1.
     */
    uint64_t        reset_stream_aec;

    /*
     * Application Error Code (AEC) for incoming STOP_SENDING frame.
     * This is only valid if peer_stop_sending is 1.
     */
    uint64_t        peer_stop_sending_aec;

    /*
     * Application Error Code (AEC) for incoming RESET_STREAM frame.
     * This is only valid if peer_reset_stream is 1.
     */
    uint64_t        peer_reset_stream_aec;

    /* Temporary value used by TXP. */
    uint64_t        txp_txfc_new_credit_consumed;

    /*
     * The final size of the send stream. Although this information can be
     * discerned from a QUIC_SSTREAM, it is stored separately as we need to keep
     * track of this even if we have thrown away the QUIC_SSTREAM. Use
     * ossl_quic_stream_send_get_final_size to determine if this contain a
     * valid value or if there is no final size yet for a sending part.
     *
     * For the receive part, the final size is tracked by the stream-level RXFC;
     * use ossl_quic_stream_recv_get_final_size or
     * ossl_quic_rxfc_get_final_size.
     */
    uint64_t        send_final_size;

    /*
     * Send stream part and receive stream part buffer management objects.
     *
     * DO NOT test these pointers (sstream, rstream) for NULL. Determine the
     * state of the send or receive stream part first using the appropriate
     * function; then the invariant of that state guarantees that sstream or
     * rstream either is or is not NULL respectively, therefore there is no
     * valid use case for testing these pointers for NULL. In particular, a
     * stream with a send part can still have sstream as NULL, and a stream with
     * a receive part can still have rstream as NULL. QUIC_SSTREAM and
     * QUIC_RSTREAM are stream buffer resource management objects which exist
     * only when they need to for buffer management purposes. The existence or
     * non-existence of a QUIC_SSTREAM or QUIC_RSTREAM object does not
     * correspond with whether a stream's respective send or receive part
     * logically exists or not.
     */
    QUIC_SSTREAM    *sstream;   /* NULL if RX-only */
    QUIC_RSTREAM    *rstream;   /* NULL if TX only */

    /* Stream-level flow control managers. */
    QUIC_TXFC       txfc;       /* NULL if RX-only */
    QUIC_RXFC       rxfc;       /* NULL if TX-only */

    unsigned int    type : 8; /* QUIC_STREAM_INITIATOR_*, QUIC_STREAM_DIR_* */

    unsigned int    send_state : 8; /* QUIC_SSTREAM_STATE_* */
    unsigned int    recv_state : 8; /* QUIC_RSTREAM_STATE_* */

    /* 1 iff this QUIC_STREAM is on the active queue (invariant). */
    unsigned int    active : 1;

    /*
     * This is a copy of the QUIC connection as_server value, indicating
     * whether we are locally operating as a server or not. Having this
     * significantly simplifies stream type determination relative to our
     * perspective. It never changes after a QUIC_STREAM is created and is the
     * same for all QUIC_STREAMS under a QUIC_STREAM_MAP.
     */
    unsigned int    as_server : 1;

    /*
     * Has STOP_SENDING been requested (by us)? Note that this is not the same
     * as want_stop_sending below, as a STOP_SENDING frame may already have been
     * sent and fully acknowledged.
     */
    unsigned int    stop_sending            : 1;

    /*
     * Has RESET_STREAM been requested (by us)? Works identically to
     * STOP_SENDING for transmission purposes.
     */
    /* Has our peer sent a STOP_SENDING frame? */
    unsigned int    peer_stop_sending       : 1;

    /* Temporary flags used by TXP. */
    unsigned int    txp_sent_fc             : 1;
    unsigned int    txp_sent_stop_sending   : 1;
    unsigned int    txp_sent_reset_stream   : 1;
    unsigned int    txp_drained             : 1;
    unsigned int    txp_blocked             : 1;

    /* Frame regeneration flags. */
    unsigned int    want_max_stream_data    : 1; /* used for regen only */
    unsigned int    want_stop_sending       : 1; /* used for gen or regen */
    unsigned int    want_reset_stream       : 1; /* used for gen or regen */

    /* Flags set when frames *we* sent were acknowledged. */
    unsigned int    acked_stop_sending      : 1;

    /*
     * The stream's XSO has been deleted. Pending GC.
     *
     * Here is how stream deletion works:
     *
     *   - A QUIC_STREAM cannot be deleted until it is neither in the accept
     *     queue nor has an associated XSO. This condition occurs when and only
     *     when deleted is true.
     *
     *   - Once this is the case (i.e., no user-facing API object exposing the
     *     stream), we can delete the stream once we determine that all of our
     *     protocol obligations requiring us to keep the QUIC_STREAM around have
     *     been met.
     *
     *     The following frames relate to the streams layer for a specific
     *     stream:
     *
     *          STREAM
     *
     *              RX Obligations:
     *                  Ignore for a deleted stream.
     *
     *                  (This is different from our obligation for a
     *                  locally-initiated stream ID we have not created yet,
     *                  which we must treat as a protocol error. This can be
     *                  distinguished via a simple monotonic counter.)
     *
     *              TX Obligations:
     *                  None, once we've decided to (someday) delete the stream.
     *
     *          STOP_SENDING
     *
     *              We cannot delete the stream until we have finished informing
     *              the peer that we are not going to be listening to it
     *              anymore.
     *
     *              RX Obligations:
     *                  When we delete a stream we must have already had a FIN
     *                  or RESET_STREAM we transmitted acknowledged by the peer.
     *                  Thus we can ignore STOP_SENDING frames for deleted
     *                  streams (if they occur, they are probably just
     *                  retransmissions).
     *
     *              TX Obligations:
     *                  _Acknowledged_ receipt of a STOP_SENDING frame by the
     *                  peer (unless the peer's send part has already FIN'd).
     *
     *          RESET_STREAM
     *
     *              We cannot delete the stream until we have finished informing
     *              the peer that we are not going to be transmitting on it
     *              anymore.
     *
     *              RX Obligations:
     *                  This indicates the peer is not going to send any more
     *                  data on the stream. We don't need to care about this
     *                  since once a stream is marked for deletion we don't care
     *                  about any data it does send. We can ignore this for
     *                  deleted streams. The important criterion is that the
     *                  peer has been successfully delivered our STOP_SENDING
     *                  frame.
     *
     *              TX Obligations:
     *                  _Acknowledged_ receipt of a RESET_STREAM frame or FIN by
     *                  the peer.
     *
     *          MAX_STREAM_DATA
     *
     *              RX Obligations:
     *                 Ignore. Since we are not going to be sending any more
     *                 data on a stream once it has been marked for deletion,
     *                 we don't need to care about flow control information.
     *
     *              TX Obligations:
     *                  None.
     *
     *     In other words, our protocol obligation is simply:
     *
     *       - either:
     *         - the peer has acknowledged receipt of a STOP_SENDING frame sent
     *            by us; -or-
     *         - we have received a FIN and all preceding segments from the peer
     *
     *            [NOTE: The actual criterion required here is simply 'we have
     *            received a FIN from the peer'. However, due to reordering and
     *            retransmissions we might subsequently receive non-FIN segments
     *            out of order. The FIN means we know the peer will stop
     *            transmitting on the stream at *some* point, but by sending
     *            STOP_SENDING we can avoid these needless retransmissions we
     *            will just ignore anyway. In actuality we could just handle all
     *            cases by sending a STOP_SENDING. The strategy we choose is to
     *            only avoid sending a STOP_SENDING and rely on a received FIN
     *            when we have received all preceding data, as this makes it
     *            reasonably certain no benefit would be gained by sending
     *            STOP_SENDING.]
     *
     *            TODO(QUIC FUTURE): Implement the latter case (currently we
                                     just always do STOP_SENDING).
     *
     *         and;
     *
     *       - we have drained our send stream (for a finished send stream)
     *         and got acknowledgement all parts of it including the FIN, or
     *         sent a RESET_STREAM frame and got acknowledgement of that frame.
     *
     *      Once these conditions are met, we can GC the QUIC_STREAM.
     *
     */
    unsigned int    deleted                 : 1;
    /* Set to 1 once the above conditions are actually met. */
    unsigned int    ready_for_gc            : 1;
    /* Set to 1 if this is currently counted in the shutdown flush stream count. */
    unsigned int    shutdown_flush          : 1;
};

#define QUIC_STREAM_INITIATOR_CLIENT        0
#define QUIC_STREAM_INITIATOR_SERVER        1
#define QUIC_STREAM_INITIATOR_MASK          1

#define QUIC_STREAM_DIR_BIDI                0
#define QUIC_STREAM_DIR_UNI                 2
#define QUIC_STREAM_DIR_MASK                2

void ossl_quic_stream_check(const QUIC_STREAM *s);

/*
 * Returns 1 if the QUIC_STREAM was initiated by the endpoint with the server
 * role.
 */
static ossl_inline ossl_unused int ossl_quic_stream_is_server_init(const QUIC_STREAM *s)
{
    return (s->type & QUIC_STREAM_INITIATOR_MASK) == QUIC_STREAM_INITIATOR_SERVER;
}

/*
 * Returns 1 if the QUIC_STREAM is bidirectional and 0 if it is unidirectional.
 */
static ossl_inline ossl_unused int ossl_quic_stream_is_bidi(const QUIC_STREAM *s)
{
    return (s->type & QUIC_STREAM_DIR_MASK) == QUIC_STREAM_DIR_BIDI;
}

/* Returns 1 if the QUIC_STREAM was locally initiated. */
static ossl_inline ossl_unused int ossl_quic_stream_is_local_init(const QUIC_STREAM *s)
{
    return ossl_quic_stream_is_server_init(s) == s->as_server;
}

/*
 * Returns 1 if the QUIC_STREAM has a sending part, based on its stream type.
 *
 * Do NOT use (s->sstream != NULL) to test this; use this function. Note that
 * even if this function returns 1, s->sstream might be NULL if the QUIC_SSTREAM
 * has been deemed no longer needed, for example due to a RESET_STREAM.
 */
static ossl_inline ossl_unused int ossl_quic_stream_has_send(const QUIC_STREAM *s)
{
    return s->send_state != QUIC_SSTREAM_STATE_NONE;
}

/*
 * Returns 1 if the QUIC_STREAM has a receiving part, based on its stream type.
 *
 * Do NOT use (s->rstream != NULL) to test this; use this function. Note that
 * even if this function returns 1, s->rstream might be NULL if the QUIC_RSTREAM
 * has been deemed no longer needed, for example if the receive stream is
 * completely finished with.
 */
static ossl_inline ossl_unused int ossl_quic_stream_has_recv(const QUIC_STREAM *s)
{
    return s->recv_state != QUIC_RSTREAM_STATE_NONE;
}

/*
 * Returns 1 if the QUIC_STREAM has a QUIC_SSTREAM send buffer associated with
 * it. If this returns 1, s->sstream is guaranteed to be non-NULL. The converse
 * is not necessarily true; erasure of a send stream buffer which is no longer
 * required is an optimisation which the QSM may, but is not obliged, to
 * perform.
 *
 * This call should be used where it is desired to do something with the send
 * stream buffer but there is no more specific send state restriction which is
 * applicable.
 *
 * Note: This does NOT indicate whether it is suitable to allow an application
 * to append to the buffer. DATA_SENT indicates all data (including FIN) has
 * been *sent*; the absence of DATA_SENT does not mean a FIN has not been queued
 * (meaning no more application data can be appended). This is enforced by
 * QUIC_SSTREAM.
 */
static ossl_inline ossl_unused int ossl_quic_stream_has_send_buffer(const QUIC_STREAM *s)
{
    switch (s->send_state) {
    case QUIC_SSTREAM_STATE_READY:
    case QUIC_SSTREAM_STATE_SEND:
    case QUIC_SSTREAM_STATE_DATA_SENT:
        return 1;
    default:
        return 0;
    }
}

/*
 * Returns 1 if the QUIC_STREAM has a sending part which is in one of the reset
 * states.
 */
static ossl_inline ossl_unused int ossl_quic_stream_send_is_reset(const QUIC_STREAM *s)
{
    return s->send_state == QUIC_SSTREAM_STATE_RESET_SENT
        || s->send_state == QUIC_SSTREAM_STATE_RESET_RECVD;
}

/*
 * Returns 1 if the QUIC_STREAM has a QUIC_RSTREAM receive buffer associated
 * with it. If this returns 1, s->rstream is guaranteed to be non-NULL. The
 * converse is not necessarily true; erasure of a receive stream buffer which is
 * no longer required is an optimisation which the QSM may, but is not obliged,
 * to perform.
 *
 * This call should be used where it is desired to do something with the receive
 * stream buffer but there is no more specific receive state restriction which is
 * applicable.
 */
static ossl_inline ossl_unused int ossl_quic_stream_has_recv_buffer(const QUIC_STREAM *s)
{
    switch (s->recv_state) {
    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
    case QUIC_RSTREAM_STATE_DATA_RECVD:
        return 1;
    default:
        return 0;
    }
}

/*
 * Returns 1 if the QUIC_STREAM has a receiving part which is in one of the
 * reset states.
 */
static ossl_inline ossl_unused int ossl_quic_stream_recv_is_reset(const QUIC_STREAM *s)
{
    return s->recv_state == QUIC_RSTREAM_STATE_RESET_RECVD
        || s->recv_state == QUIC_RSTREAM_STATE_RESET_READ;
}

/*
 * Returns 1 if the stream has a send part and that part has a final size.
 *
 * If final_size is non-NULL, *final_size is the final size (on success) or an
 * undefined value otherwise.
 */
static ossl_inline ossl_unused int ossl_quic_stream_send_get_final_size(const QUIC_STREAM *s,
                                                                        uint64_t *final_size)
{
    switch (s->send_state) {
    default:
    case QUIC_SSTREAM_STATE_NONE:
        return 0;
    case QUIC_SSTREAM_STATE_SEND:
        /*
         * SEND may or may not have had a FIN - even if we have a FIN we do not
         * move to DATA_SENT until we have actually sent all the data. So
         * ask the QUIC_SSTREAM.
         */
        return ossl_quic_sstream_get_final_size(s->sstream, final_size);
    case QUIC_SSTREAM_STATE_DATA_SENT:
    case QUIC_SSTREAM_STATE_DATA_RECVD:
    case QUIC_SSTREAM_STATE_RESET_SENT:
    case QUIC_SSTREAM_STATE_RESET_RECVD:
        if (final_size != NULL)
            *final_size = s->send_final_size;
        return 1;
    }
}

/*
 * Returns 1 if the stream has a receive part and that part has a final size.
 *
 * If final_size is non-NULL, *final_size is the final size (on success) or an
 * undefined value otherwise.
 */
static ossl_inline ossl_unused int ossl_quic_stream_recv_get_final_size(const QUIC_STREAM *s,
                                                                        uint64_t *final_size)
{
    switch (s->recv_state) {
    default:
        assert(0);
    case QUIC_RSTREAM_STATE_NONE:
    case QUIC_RSTREAM_STATE_RECV:
        return 0;

    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
    case QUIC_RSTREAM_STATE_DATA_RECVD:
    case QUIC_RSTREAM_STATE_DATA_READ:
    case QUIC_RSTREAM_STATE_RESET_RECVD:
    case QUIC_RSTREAM_STATE_RESET_READ:
        if (!ossl_assert(ossl_quic_rxfc_get_final_size(&s->rxfc, final_size)))
            return 0;

        return 1;
    }
}

/*
 * Determines the number of bytes available still to be read, and (if
 * include_fin is 1) whether a FIN or reset has yet to be read.
 */
static ossl_inline ossl_unused int ossl_quic_stream_recv_pending(const QUIC_STREAM *s,
                                                                 int include_fin)
{
    size_t avail;
    int fin = 0;

    switch (s->recv_state) {
    default:
        assert(0);
    case QUIC_RSTREAM_STATE_NONE:
        return 0;

    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
    case QUIC_RSTREAM_STATE_DATA_RECVD:
        if (!ossl_quic_rstream_available(s->rstream, &avail, &fin))
            avail = 0;

        if (avail == 0 && include_fin && fin)
            avail = 1;

        return avail;

    case QUIC_RSTREAM_STATE_RESET_RECVD:
        return include_fin;

    case QUIC_RSTREAM_STATE_DATA_READ:
    case QUIC_RSTREAM_STATE_RESET_READ:
        return 0;
    }
}

/*
 * QUIC Stream Map
 * ===============
 *
 * The QUIC stream map:
 *
 *   - maps stream IDs to QUIC_STREAM objects;
 *   - tracks which streams are 'active' (currently have data for transmission);
 *   - allows iteration over the active streams only.
 *
 */
struct quic_stream_map_st {
    LHASH_OF(QUIC_STREAM)   *map;
    QUIC_STREAM_LIST_NODE   active_list;
    QUIC_STREAM_LIST_NODE   accept_list;
    QUIC_STREAM_LIST_NODE   ready_for_gc_list;
    size_t                  rr_stepping, rr_counter;
    size_t                  num_accept_bidi, num_accept_uni, num_shutdown_flush;
    QUIC_STREAM             *rr_cur;
    uint64_t                (*get_stream_limit_cb)(int uni, void *arg);
    void                    *get_stream_limit_cb_arg;
    QUIC_RXFC               *max_streams_bidi_rxfc;
    QUIC_RXFC               *max_streams_uni_rxfc;
    int                     is_server;
};

/*
 * get_stream_limit is a callback which is called to retrieve the current stream
 * limit for streams created by us. This mechanism is not used for
 * peer-initiated streams. If a stream's stream ID is x, a stream is allowed if
 * (x >> 2) < returned limit value; i.e., the returned value is exclusive.
 *
 * If uni is 1, get the limit for locally-initiated unidirectional streams, else
 * get the limit for locally-initiated bidirectional streams.
 *
 * If the callback is NULL, stream limiting is not applied.
 * Stream limiting is used to determine if frames can currently be produced for
 * a stream.
 */
int ossl_quic_stream_map_init(QUIC_STREAM_MAP *qsm,
                              uint64_t (*get_stream_limit_cb)(int uni, void *arg),
                              void *get_stream_limit_cb_arg,
                              QUIC_RXFC *max_streams_bidi_rxfc,
                              QUIC_RXFC *max_streams_uni_rxfc,
                              int is_server);

/*
 * Any streams still in the map will be released as though
 * ossl_quic_stream_map_release was called on them.
 */
void ossl_quic_stream_map_cleanup(QUIC_STREAM_MAP *qsm);

/*
 * Allocate a new stream. type is a combination of one QUIC_STREAM_INITIATOR_*
 * value and one QUIC_STREAM_DIR_* value. Note that clients can e.g. allocate
 * server-initiated streams as they will need to allocate a QUIC_STREAM
 * structure to track any stream created by the server, etc.
 *
 * stream_id must be a valid value. Returns NULL if a stream already exists
 * with the given ID.
 */
QUIC_STREAM *ossl_quic_stream_map_alloc(QUIC_STREAM_MAP *qsm,
                                        uint64_t stream_id,
                                        int type);

/*
 * Releases a stream object. Note that this must only be done once the teardown
 * process is entirely complete and the object will never be referenced again.
 */
void ossl_quic_stream_map_release(QUIC_STREAM_MAP *qsm, QUIC_STREAM *stream);

/*
 * Calls visit_cb() for each stream in the map. visit_cb_arg is an opaque
 * argument which is passed through.
 */
void ossl_quic_stream_map_visit(QUIC_STREAM_MAP *qsm,
                                void (*visit_cb)(QUIC_STREAM *stream, void *arg),
                                void *visit_cb_arg);

/*
 * Retrieves a stream by stream ID. Returns NULL if it does not exist.
 */
QUIC_STREAM *ossl_quic_stream_map_get_by_id(QUIC_STREAM_MAP *qsm,
                                            uint64_t stream_id);

/*
 * Marks the given stream as active or inactive based on its state. Idempotent.
 *
 * When a stream is marked active, it becomes available in the iteration list,
 * and when a stream is marked inactive, it no longer appears in the iteration
 * list.
 *
 * Calling this function invalidates any iterator currently pointing at the
 * given stream object, but iterators not currently pointing at the given stream
 * object are not invalidated.
 */
void ossl_quic_stream_map_update_state(QUIC_STREAM_MAP *qsm, QUIC_STREAM *s);

/*
 * Sets the RR stepping value, n. The RR rotation will be advanced every n
 * packets. The default value is 1.
 */
void ossl_quic_stream_map_set_rr_stepping(QUIC_STREAM_MAP *qsm, size_t stepping);

/*
 * Returns 1 if the stream ordinal given is allowed by the current stream count
 * flow control limit, assuming a locally initiated stream of a type described
 * by is_uni.
 *
 * Note that stream_ordinal is a stream ordinal, not a stream ID.
 */
int ossl_quic_stream_map_is_local_allowed_by_stream_limit(QUIC_STREAM_MAP *qsm,
                                                          uint64_t stream_ordinal,
                                                          int is_uni);

/*
 * Stream Send Part
 * ================
 */

/*
 * Ensures that the sending part has transitioned out of the READY state (i.e.,
 * to SEND, or a subsequent state). This function is named as it is because,
 * while on paper the distinction between READY and SEND is whether we have
 * started transmitting application data, in practice the meaningful distinction
 * between the two states is whether we have allocated a stream ID to the stream
 * or not. QUIC permits us to defer stream ID allocation until first STREAM (or
 * STREAM_DATA_BLOCKED) frame transmission for locally-initiated streams.
 *
 * Our implementation does not currently do this and we allocate stream IDs up
 * front, however we may revisit this in the future. Calling this represents a
 * demand for a stream ID by the caller and ensures one has been allocated to
 * the stream, and causes us to transition to SEND if we are still in the READY
 * state.
 *
 * Returns 0 if there is no send part (caller error) and 1 otherwise.
 */
int ossl_quic_stream_map_ensure_send_part_id(QUIC_STREAM_MAP *qsm,
                                             QUIC_STREAM *qs);

/*
 * Transitions from SEND to the DATA_SENT state. Note that this is NOT the same
 * as the point in time at which the final size of the stream becomes known
 * (i.e., the time at which ossl_quic_sstream_fin()) is called as it occurs when
 * we have SENT all data on a given stream send part, not merely buffered it.
 * Note that this transition is NOT reversed in the event of some of that data
 * being lost.
 *
 * Returns 1 if the state transition was successfully taken. Returns 0 if there
 * is no send part (caller error) or if the state transition cannot be taken
 * because the send part is not in the SEND state.
 */
int ossl_quic_stream_map_notify_all_data_sent(QUIC_STREAM_MAP *qsm,
                                              QUIC_STREAM *qs);

/*
 * Transitions from the DATA_SENT to DATA_RECVD state; should be called
 * when all transmitted stream data is ACKed by the peer.
 *
 * Returns 1 if the state transition was successfully taken. Returns 0 if there
 * is no send part (caller error) or the state transition cannot be taken
 * because the send part is not in the DATA_SENT state. Because
 * ossl_quic_stream_map_notify_all_data_sent() should always be called prior to
 * this function, the send state must already be in DATA_SENT in order for this
 * function to succeed.
 */
int ossl_quic_stream_map_notify_totally_acked(QUIC_STREAM_MAP *qsm,
                                              QUIC_STREAM *qs);

/*
 * Resets the sending part of a stream. This is a transition from the READY,
 * SEND or DATA_SENT send stream states to the RESET_SENT state.
 *
 * This function returns 1 if the transition is taken (i.e., if the send stream
 * part was in one of the states above), or if it is already in the RESET_SENT
 * state (idempotent operation), or if it has reached the RESET_RECVD state.
 *
 * It returns 0 if in the DATA_RECVD state, as a send stream cannot be reset
 * in this state. It also returns 0 if there is no send part (caller error).
 */
int ossl_quic_stream_map_reset_stream_send_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t aec);

/*
 * Transitions from the RESET_SENT to the RESET_RECVD state. This should be
 * called when a sent RESET_STREAM frame has been acknowledged by the peer.
 *
 * This function returns 1 if the transition is taken (i.e., if the send stream
 * part was in one of the states above) or if it is already in the RESET_RECVD
 * state (idempotent operation).
 *
 * It returns 0 if not in the RESET_SENT or RESET_RECVD states, as this function
 * should only be called after we have already sent a RESET_STREAM frame and
 * entered the RESET_SENT state. It also returns 0 if there is no send part
 * (caller error).
 */
int ossl_quic_stream_map_notify_reset_stream_acked(QUIC_STREAM_MAP *qsm,
                                                   QUIC_STREAM *qs);


/*
 * Stream Receive Part
 * ===================
 */

/*
 * Transitions from the RECV receive stream state to the SIZE_KNOWN state. This
 * should be called once a STREAM frame is received for the stream with the FIN
 * bit set. final_size should be the final size of the stream in bytes.
 *
 * Returns 1 if the transition was taken.
 */
int ossl_quic_stream_map_notify_size_known_recv_part(QUIC_STREAM_MAP *qsm,
                                                     QUIC_STREAM *qs,
                                                     uint64_t final_size);

/*
 * Transitions from the SIZE_KNOWN receive stream state to the DATA_RECVD state.
 * This should be called once all data for a receive stream is received.
 *
 * Returns 1 if the transition was taken.
 */
int ossl_quic_stream_map_notify_totally_received(QUIC_STREAM_MAP *qsm,
                                                 QUIC_STREAM *qs);

/*
 * Transitions from the DATA_RECVD receive stream state to the DATA_READ state.
 * This should be called once all data for a receive stream is read by the
 * application.
 *
 * Returns 1 if the transition was taken.
 */
int ossl_quic_stream_map_notify_totally_read(QUIC_STREAM_MAP *qsm,
                                             QUIC_STREAM *qs);

/*
 * Transitions from the RECV, SIZE_KNOWN or DATA_RECVD receive stream state to
 * the RESET_RECVD state. This should be called on RESET_STREAM.
 *
 * Returns 1 if the transition was taken.
 */
int ossl_quic_stream_map_notify_reset_recv_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t app_error_code,
                                                uint64_t final_size);

/*
 * Transitions from the RESET_RECVD receive stream state to the RESET_READ
 * receive stream state. This should be called when the application is notified
 * of a stream reset.
 */
int ossl_quic_stream_map_notify_app_read_reset_recv_part(QUIC_STREAM_MAP *qsm,
                                                         QUIC_STREAM *qs);

/*
 * Marks the receiving part of a stream for STOP_SENDING. This is orthogonal to
 * receive stream state as it does not affect it directly.
 *
 * Returns 1 if the receiving part of a stream was not already marked for
 * STOP_SENDING.
 * Returns 0 otherwise, which need not be considered an error.
 */
int ossl_quic_stream_map_stop_sending_recv_part(QUIC_STREAM_MAP *qsm,
                                                QUIC_STREAM *qs,
                                                uint64_t aec);

/*
 * Marks the stream as wanting a STOP_SENDING frame transmitted. It is not valid
 * to call this if ossl_quic_stream_map_stop_sending_recv_part() has not been
 * called. For TXP use.
 */
int ossl_quic_stream_map_schedule_stop_sending(QUIC_STREAM_MAP *qsm,
                                               QUIC_STREAM *qs);


/*
 * Accept Queue Management
 * =======================
 */

/*
 * Adds a stream to the accept queue.
 */
void ossl_quic_stream_map_push_accept_queue(QUIC_STREAM_MAP *qsm,
                                            QUIC_STREAM *s);

/*
 * Returns the next item to be popped from the accept queue, or NULL if it is
 * empty.
 */
QUIC_STREAM *ossl_quic_stream_map_peek_accept_queue(QUIC_STREAM_MAP *qsm);

/*
 * Removes a stream from the accept queue. rtt is the estimated connection RTT.
 * The stream is retired for the purposes of MAX_STREAMS RXFC.
 *
 * Precondition: s is in the accept queue.
 */
void ossl_quic_stream_map_remove_from_accept_queue(QUIC_STREAM_MAP *qsm,
                                                   QUIC_STREAM *s,
                                                   OSSL_TIME rtt);

/* Returns the length of the accept queue for the given stream type. */
size_t ossl_quic_stream_map_get_accept_queue_len(QUIC_STREAM_MAP *qsm, int is_uni);

/* Returns the total length of the accept queues for all stream types. */
size_t ossl_quic_stream_map_get_total_accept_queue_len(QUIC_STREAM_MAP *qsm);

/*
 * Shutdown Flush and GC
 * =====================
 */

/*
 * Delete streams ready for GC. Pointers to those QUIC_STREAM objects become
 * invalid.
 */
void ossl_quic_stream_map_gc(QUIC_STREAM_MAP *qsm);

/*
 * Begins shutdown stream flush triage. Analyses all streams, including deleted
 * but not yet GC'd streams, to determine if we should wait for that stream to
 * be fully flushed before shutdown. After calling this, call
 * ossl_quic_stream_map_is_shutdown_flush_finished() to determine if all
 * shutdown flush eligible streams have been flushed.
 */
void ossl_quic_stream_map_begin_shutdown_flush(QUIC_STREAM_MAP *qsm);

/*
 * Returns 1 if all shutdown flush eligible streams have finished flushing,
 * or if ossl_quic_stream_map_begin_shutdown_flush() has not been called.
 */
int ossl_quic_stream_map_is_shutdown_flush_finished(QUIC_STREAM_MAP *qsm);

/*
 * QUIC Stream Iterator
 * ====================
 *
 * Allows the current set of active streams to be walked using a RR-based
 * algorithm. Each time ossl_quic_stream_iter_init is called, the RR algorithm
 * is stepped. The RR algorithm rotates the iteration order such that the next
 * active stream is returned first after n calls to ossl_quic_stream_iter_init,
 * where n is the stepping value configured via
 * ossl_quic_stream_map_set_rr_stepping.
 *
 * Suppose there are three active streams and the configured stepping is n:
 *
 *   Iteration 0n:  [Stream 1] [Stream 2] [Stream 3]
 *   Iteration 1n:  [Stream 2] [Stream 3] [Stream 1]
 *   Iteration 2n:  [Stream 3] [Stream 1] [Stream 2]
 *
 */
typedef struct quic_stream_iter_st {
    QUIC_STREAM_MAP     *qsm;
    QUIC_STREAM         *first_stream, *stream;
} QUIC_STREAM_ITER;

/*
 * Initialise an iterator, advancing the RR algorithm as necessary (if
 * advance_rr is 1). After calling this, it->stream will be the first stream in
 * the iteration sequence, or NULL if there are no active streams.
 */
void ossl_quic_stream_iter_init(QUIC_STREAM_ITER *it, QUIC_STREAM_MAP *qsm,
                                int advance_rr);

/*
 * Advances to next stream in iteration sequence. You do not need to call this
 * immediately after calling ossl_quic_stream_iter_init(). If the end of the
 * list is reached, it->stream will be NULL after calling this.
 */
void ossl_quic_stream_iter_next(QUIC_STREAM_ITER *it);

# endif

#endif
