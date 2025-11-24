/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_TXPIM_H
# define OSSL_QUIC_TXPIM_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/quic_predef.h"
# include "internal/quic_cfq.h"
# include "internal/quic_ackm.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Transmitted Packet Information Manager
 * ===========================================
 */

typedef struct quic_txpim_pkt_st {
    /* ACKM-specific data. Caller should fill this. */
    OSSL_ACKM_TX_PKT    ackm_pkt;

    /* Linked list of CFQ items in this packet. */
    QUIC_CFQ_ITEM      *retx_head;

    /* Reserved for FIFD use. */
    QUIC_FIFD          *fifd;

    /* QUIC_PKT_TYPE value. For diagnostic use only. */
    unsigned char       pkt_type;

    /* Regenerate-strategy frames. */
    unsigned int        had_handshake_done_frame    : 1;
    unsigned int        had_max_data_frame          : 1;
    unsigned int        had_max_streams_bidi_frame  : 1;
    unsigned int        had_max_streams_uni_frame   : 1;
    unsigned int        had_ack_frame               : 1;
    unsigned int        had_conn_close              : 1;

    /* Private data follows. */
} QUIC_TXPIM_PKT;

/* Represents a range of bytes in an application or CRYPTO stream. */
typedef struct quic_txpim_chunk_st {
    /* The stream ID, or UINT64_MAX for the CRYPTO stream. */
    uint64_t        stream_id;
    /*
     * The inclusive range of bytes in the stream. Exceptionally, if end <
     * start, designates a frame of zero length (used for FIN-only frames). In
     * this case end is the number of the final byte (i.e., one less than the
     * final size of the stream).
     */
    uint64_t        start, end;
    /*
     * Whether a FIN was sent for this stream in the packet. Not valid for
     * CRYPTO stream.
     */
    unsigned int    has_fin : 1;
    /*
     * If set, a STOP_SENDING frame was sent for this stream ID. (If no data was
     * sent for the stream, set end < start.)
     */
    unsigned int    has_stop_sending : 1;
    /*
     * If set, a RESET_STREAM frame was sent for this stream ID. (If no data was
     * sent for the stream, set end < start.)
     */
    unsigned int    has_reset_stream : 1;
} QUIC_TXPIM_CHUNK;

QUIC_TXPIM *ossl_quic_txpim_new(void);

/*
 * Frees the TXPIM. All QUIC_TXPIM_PKTs which have been handed out by the TXPIM
 * must be released via a call to ossl_quic_txpim_pkt_release() before calling
 * this function.
 */
void ossl_quic_txpim_free(QUIC_TXPIM *txpim);

/*
 * Allocates a new QUIC_TXPIM_PKT structure from the pool. Returns NULL on
 * failure. The returned structure is cleared of all data and is in a fresh
 * initial state.
 */
QUIC_TXPIM_PKT *ossl_quic_txpim_pkt_alloc(QUIC_TXPIM *txpim);

/*
 * Releases the TXPIM packet, returning it to the pool.
 */
void ossl_quic_txpim_pkt_release(QUIC_TXPIM *txpim, QUIC_TXPIM_PKT *fpkt);

/* Clears the chunk list of the packet, removing all entries. */
void ossl_quic_txpim_pkt_clear_chunks(QUIC_TXPIM_PKT *fpkt);

/* Appends a chunk to the packet. The structure is copied. */
int ossl_quic_txpim_pkt_append_chunk(QUIC_TXPIM_PKT *fpkt,
                                     const QUIC_TXPIM_CHUNK *chunk);

/* Adds a CFQ item to the packet by prepending it to the retx_head list. */
void ossl_quic_txpim_pkt_add_cfq_item(QUIC_TXPIM_PKT *fpkt,
                                      QUIC_CFQ_ITEM *item);

/*
 * Returns a pointer to an array of stream chunk information structures for the
 * given packet. The caller must call ossl_quic_txpim_pkt_get_num_chunks() to
 * determine the length of this array. The returned pointer is invalidated
 * if the chunk list is mutated, for example via a call to
 * ossl_quic_txpim_pkt_append_chunk() or ossl_quic_txpim_pkt_clear_chunks().
 *
 * The chunks are sorted by (stream_id, start) in ascending order.
 */
const QUIC_TXPIM_CHUNK *ossl_quic_txpim_pkt_get_chunks(const QUIC_TXPIM_PKT *fpkt);

/*
 * Returns the number of entries in the array returned by
 * ossl_quic_txpim_pkt_get_chunks().
 */
size_t ossl_quic_txpim_pkt_get_num_chunks(const QUIC_TXPIM_PKT *fpkt);

/*
 * Returns the number of QUIC_TXPIM_PKTs allocated by the given TXPIM that have
 * yet to be returned to the TXPIM.
 */
size_t ossl_quic_txpim_get_in_use(const QUIC_TXPIM *txpim);

# endif

#endif
