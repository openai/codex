/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_SF_LIST_H
# define OSSL_QUIC_SF_LIST_H

#include "internal/common.h"
#include "internal/uint_set.h"
#include "internal/quic_record_rx.h"

/*
 * Stream frame list
 * =================
 *
 * This data structure supports similar operations as uint64 set but
 * it has slightly different invariants and also carries data associated with
 * the ranges in the list.
 *
 * Operations:
 *   Insert frame (optimized insertion at the beginning and at the end).
 *   Iterated peek into the frame(s) from the beginning.
 *   Dropping frames from the beginning up to an offset (exclusive).
 *
 * Invariant: The frames in the list are sorted by the start and end bounds.
 * Invariant: There are no fully overlapping frames or frames that would
 *            be fully encompassed by another frame in the list.
 * Invariant: No frame has start > end.
 * Invariant: The range start is inclusive the end is exclusive to be
 *            able to mark an empty frame.
 * Invariant: The offset never points further than into the first frame.
 */
# ifndef OPENSSL_NO_QUIC

typedef struct stream_frame_st STREAM_FRAME;

typedef struct sframe_list_st {
    STREAM_FRAME  *head, *tail;
    /* Is the tail frame final. */
    unsigned int fin;
    /* Number of stream frames in the list. */
    size_t num_frames;
    /* Offset of data not yet dropped */
    uint64_t offset;
    /* Is head locked ? */
    int head_locked;
    /* Cleanse data on release? */
    int cleanse;
} SFRAME_LIST;

/*
 * Initializes the stream frame list fl.
 */
void ossl_sframe_list_init(SFRAME_LIST *fl);

/*
 * Destroys the stream frame list fl releasing any data
 * still present inside it.
 */
void ossl_sframe_list_destroy(SFRAME_LIST *fl);

/*
 * Insert a stream frame data into the list.
 * The data covers an offset range (range.start is inclusive,
 * range.end is exclusive).
 * fin should be set if this is the final frame of the stream.
 * Returns an error if a frame cannot be inserted - due to
 * STREAM_FRAME allocation error, or in case of erroneous
 * fin flag (this is an ossl_assert() check so a caller must
 * check it on its own too).
 */
int ossl_sframe_list_insert(SFRAME_LIST *fl, UINT_RANGE *range,
                            OSSL_QRX_PKT *pkt,
                            const unsigned char *data, int fin);

/*
 * Iterator to peek at the contiguous frames at the beginning
 * of the frame list fl.
 * The *data covers an offset range (range.start is inclusive,
 * range.end is exclusive).
 * *fin is set if this is the final frame of the stream.
 * Opaque iterator *iter can be used to peek at the subsequent
 * frame if there is any without any gap before it.
 * Returns 1 on success.
 * Returns 0 if there is no further contiguous frame. In that
 * case *fin is set, if the end of the stream is reached.
 */
int ossl_sframe_list_peek(const SFRAME_LIST *fl, void **iter,
                          UINT_RANGE *range, const unsigned char **data,
                          int *fin);

/*
 * Drop all frames up to the offset limit.
 * Also unlocks the head frame if locked.
 * Returns 1 on success.
 * Returns 0 when trying to drop frames at offsets that were not
 * received yet. (ossl_assert() is used to check, so this is an invalid call.)
 */
int ossl_sframe_list_drop_frames(SFRAME_LIST *fl, uint64_t limit);

/*
 * Locks and returns the head frame of fl if it is readable - read offset is
 * at the beginning or middle of the frame.
 * range is set to encompass the not yet read part of the head frame,
 * data pointer is set to appropriate offset within the frame if the read
 * offset points in the middle of the frame,
 * fin is set to 1 if the head frame is also the tail frame.
 * Returns 1 on success, 0 if there is no readable data or the head
 * frame is already locked.
 */
int ossl_sframe_list_lock_head(SFRAME_LIST *fl, UINT_RANGE *range,
                               const unsigned char **data,
                               int *fin);

/*
 * Just returns whether the head frame is locked by previous
 * ossl_sframe_list_lock_head() call.
 */
int ossl_sframe_list_is_head_locked(SFRAME_LIST *fl);

/*
 * Callback function type to write stream frame data to some
 * side storage before the packet containing the frame data
 * is released.
 * It should return 1 on success or 0 if there is not enough
 * space available in the side storage.
 */
typedef int (sframe_list_write_at_cb)(uint64_t logical_offset,
                                      const unsigned char *buf,
                                      size_t buf_len,
                                      void *cb_arg);

/*
 * Move the frame data in all the stream frames in the list fl
 * from the packets to the side storage using the write_at_cb
 * callback.
 * Returns 1 if all the calls to the callback return 1.
 * If the callback returns 0, the function stops processing further
 * frames and returns 0.
 */
int ossl_sframe_list_move_data(SFRAME_LIST *fl,
                               sframe_list_write_at_cb *write_at_cb,
                               void *cb_arg);
# endif

#endif
