/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_CFQ_H
# define OSSL_QUIC_CFQ_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/quic_predef.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Control Frame Queue Item
 * =============================
 *
 * The CFQ item structure has a public and a private part. This structure
 * documents the public part.
 */
typedef struct quic_cfq_item_st QUIC_CFQ_ITEM;

struct quic_cfq_item_st {
    /*
     * These fields are not used by the CFQ, but are a convenience to assist the
     * TXPIM in keeping a list of GCR control frames which were sent in a
     * packet. They may be used for any purpose.
     */
    QUIC_CFQ_ITEM  *pkt_prev, *pkt_next;

    /* All other fields are private; use ossl_quic_cfq_item_* accessors. */
};

#  define QUIC_CFQ_STATE_NEW      0
#  define QUIC_CFQ_STATE_TX       1

/* If set, do not retransmit on loss */
#define QUIC_CFQ_ITEM_FLAG_UNRELIABLE   (1U << 0)

/* Returns the frame type of a CFQ item. */
uint64_t ossl_quic_cfq_item_get_frame_type(const QUIC_CFQ_ITEM *item);

/* Returns a pointer to the encoded buffer of a CFQ item. */
const unsigned char *ossl_quic_cfq_item_get_encoded(const QUIC_CFQ_ITEM *item);

/* Returns the length of the encoded buffer in bytes. */
size_t ossl_quic_cfq_item_get_encoded_len(const QUIC_CFQ_ITEM *item);

/* Returns the CFQ item state, a QUIC_CFQ_STATE_* value. */
int ossl_quic_cfq_item_get_state(const QUIC_CFQ_ITEM *item);

/* Returns the PN space for the CFQ item. */
uint32_t ossl_quic_cfq_item_get_pn_space(const QUIC_CFQ_ITEM *item);

/* Returns 1 if this is an unreliable frame. */
int ossl_quic_cfq_item_is_unreliable(const QUIC_CFQ_ITEM *item);

/*
 * QUIC Control Frame Queue
 * ========================
 */

QUIC_CFQ *ossl_quic_cfq_new(void);
void ossl_quic_cfq_free(QUIC_CFQ *cfq);

/*
 * Input Side
 * ----------
 */

/*
 * Enqueue a frame to the CFQ.
 *
 * encoded points to the opaque encoded frame.
 *
 * free_cb is called by the CFQ when the buffer is no longer needed;
 * free_cb_arg is an opaque value passed to free_cb.
 *
 * priority determines the relative ordering of control frames in a packet.
 * Lower numerical values for priority mean that a frame should come earlier in
 * a packet. pn_space is a QUIC_PN_SPACE_* value.
 *
 * On success, returns a QUIC_CFQ_ITEM pointer which acts as a handle to
 * the queued frame. On failure, returns NULL.
 *
 * The frame is initially in the TX state, so there is no need to call
 * ossl_quic_cfq_mark_tx() immediately after calling this function.
 *
 * The frame type is duplicated as the frame_type argument here, even though it
 * is also encoded into the buffer. This allows the caller to determine the
 * frame type if desired without having to decode the frame.
 *
 * flags is zero or more QUIC_CFQ_ITEM_FLAG values.
 */
typedef void (cfq_free_cb)(unsigned char *buf, size_t buf_len, void *arg);

QUIC_CFQ_ITEM *ossl_quic_cfq_add_frame(QUIC_CFQ            *cfq,
                                       uint32_t             priority,
                                       uint32_t             pn_space,
                                       uint64_t             frame_type,
                                       uint32_t             flags,
                                       const unsigned char *encoded,
                                       size_t               encoded_len,
                                       cfq_free_cb         *free_cb,
                                       void                *free_cb_arg);

/*
 * Effects an immediate transition of the given CFQ item to the TX state.
 */
void ossl_quic_cfq_mark_tx(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item);

/*
 * Effects an immediate transition of the given CFQ item to the NEW state,
 * allowing the frame to be retransmitted. If priority is not UINT32_MAX,
 * the priority is changed to the given value.
 */
void ossl_quic_cfq_mark_lost(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item,
                             uint32_t priority);

/*
 * Releases a CFQ item. The item may be in either state (NEW or TX) prior to the
 * call. The QUIC_CFQ_ITEM pointer must not be used following this call.
 */
void ossl_quic_cfq_release(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item);

/*
 * Output Side
 * -----------
 */

/*
 * Gets the highest priority CFQ item in the given PN space awaiting
 * transmission. If there are none, returns NULL.
 */
QUIC_CFQ_ITEM *ossl_quic_cfq_get_priority_head(const QUIC_CFQ *cfq,
                                               uint32_t pn_space);

/*
 * Given a CFQ item, gets the next CFQ item awaiting transmission in priority
 * order in the given PN space. In other words, given the return value of
 * ossl_quic_cfq_get_priority_head(), returns the next-lower priority item.
 * Returns NULL if the given item is the last item in priority order.
 */
QUIC_CFQ_ITEM *ossl_quic_cfq_item_get_priority_next(const QUIC_CFQ_ITEM *item,
                                                    uint32_t pn_space);

# endif

#endif
