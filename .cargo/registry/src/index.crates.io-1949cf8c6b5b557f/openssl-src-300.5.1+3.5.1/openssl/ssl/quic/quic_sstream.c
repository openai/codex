/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_stream.h"
#include "internal/uint_set.h"
#include "internal/common.h"
#include "internal/ring_buf.h"

/*
 * ==================================================================
 * QUIC Send Stream
 */
struct quic_sstream_st {
    struct ring_buf ring_buf;

    /*
     * Any logical byte in the stream is in one of these states:
     *
     *   - NEW: The byte has not yet been transmitted, or has been lost and is
     *     in need of retransmission.
     *
     *   - IN_FLIGHT: The byte has been transmitted but is awaiting
     *     acknowledgement. We continue to store the data in case we return
     *     to the NEW state.
     *
     *   - ACKED: The byte has been acknowledged and we can cease storing it.
     *     We do not necessarily cull it immediately, so there may be a delay
     *     between reaching the ACKED state and the buffer space actually being
     *     recycled.
     *
     * A logical byte in the stream is
     *
     *   - in the NEW state if it is in new_set;
     *   - is in the ACKED state if it is in acked_set
     *       (and may or may not have been culled);
     *   - is in the IN_FLIGHT state otherwise.
     *
     * Invariant: No logical byte is ever in both new_set and acked_set.
     */
    UINT_SET        new_set, acked_set;

    /*
     * The current size of the stream is ring_buf.head_offset. If
     * have_final_size is true, this is also the final size of the stream.
     */
    unsigned int    have_final_size     : 1;
    unsigned int    sent_final_size     : 1;
    unsigned int    acked_final_size    : 1;
    unsigned int    cleanse             : 1;
};

static void qss_cull(QUIC_SSTREAM *qss);

QUIC_SSTREAM *ossl_quic_sstream_new(size_t init_buf_size)
{
    QUIC_SSTREAM *qss;

    qss = OPENSSL_zalloc(sizeof(QUIC_SSTREAM));
    if (qss == NULL)
        return NULL;

    ring_buf_init(&qss->ring_buf);
    if (!ring_buf_resize(&qss->ring_buf, init_buf_size, 0)) {
        ring_buf_destroy(&qss->ring_buf, 0);
        OPENSSL_free(qss);
        return NULL;
    }

    ossl_uint_set_init(&qss->new_set);
    ossl_uint_set_init(&qss->acked_set);
    return qss;
}

void ossl_quic_sstream_free(QUIC_SSTREAM *qss)
{
    if (qss == NULL)
        return;

    ossl_uint_set_destroy(&qss->new_set);
    ossl_uint_set_destroy(&qss->acked_set);
    ring_buf_destroy(&qss->ring_buf, qss->cleanse);
    OPENSSL_free(qss);
}

int ossl_quic_sstream_get_stream_frame(QUIC_SSTREAM *qss,
                                       size_t skip,
                                       OSSL_QUIC_FRAME_STREAM *hdr,
                                       OSSL_QTX_IOVEC *iov,
                                       size_t *num_iov)
{
    size_t num_iov_ = 0, src_len = 0, total_len = 0, i;
    uint64_t max_len;
    const unsigned char *src = NULL;
    UINT_SET_ITEM *range = ossl_list_uint_set_head(&qss->new_set);

    if (*num_iov < 2)
        return 0;

    for (i = 0; i < skip && range != NULL; ++i)
        range = ossl_list_uint_set_next(range);

    if (range == NULL) {
        if (i < skip)
            /* Don't return FIN for infinitely increasing skip */
            return 0;

        /* No new bytes to send, but we might have a FIN */
        if (!qss->have_final_size || qss->sent_final_size)
            return 0;

        hdr->offset = qss->ring_buf.head_offset;
        hdr->len    = 0;
        hdr->is_fin = 1;
        *num_iov    = 0;
        return 1;
    }

    /*
     * We can only send a contiguous range of logical bytes in a single
     * stream frame, so limit ourselves to the range of the first set entry.
     *
     * Set entries never have 'adjacent' entries so we don't have to worry
     * about them here.
     */
    max_len = range->range.end - range->range.start + 1;

    for (i = 0;; ++i) {
        if (total_len >= max_len)
            break;

        if (!ring_buf_get_buf_at(&qss->ring_buf,
                                 range->range.start + total_len,
                                 &src, &src_len))
            return 0;

        if (src_len == 0)
            break;

        assert(i < 2);

        if (total_len + src_len > max_len)
            src_len = (size_t)(max_len - total_len);

        iov[num_iov_].buf       = src;
        iov[num_iov_].buf_len   = src_len;

        total_len += src_len;
        ++num_iov_;
    }

    hdr->offset = range->range.start;
    hdr->len    = total_len;
    hdr->is_fin = qss->have_final_size
        && hdr->offset + hdr->len == qss->ring_buf.head_offset;

    *num_iov    = num_iov_;
    return 1;
}

int ossl_quic_sstream_has_pending(QUIC_SSTREAM *qss)
{
    OSSL_QUIC_FRAME_STREAM shdr;
    OSSL_QTX_IOVEC iov[2];
    size_t num_iov = OSSL_NELEM(iov);

    return ossl_quic_sstream_get_stream_frame(qss, 0, &shdr, iov, &num_iov);
}

uint64_t ossl_quic_sstream_get_cur_size(QUIC_SSTREAM *qss)
{
    return qss->ring_buf.head_offset;
}

int ossl_quic_sstream_mark_transmitted(QUIC_SSTREAM *qss,
                                       uint64_t start,
                                       uint64_t end)
{
    UINT_RANGE r;

    r.start = start;
    r.end   = end;

    if (!ossl_uint_set_remove(&qss->new_set, &r))
        return 0;

    return 1;
}

int ossl_quic_sstream_mark_transmitted_fin(QUIC_SSTREAM *qss,
                                           uint64_t final_size)
{
    /*
     * We do not really need final_size since we already know the size of the
     * stream, but this serves as a sanity check.
     */
    if (!qss->have_final_size || final_size != qss->ring_buf.head_offset)
        return 0;

    qss->sent_final_size = 1;
    return 1;
}

int ossl_quic_sstream_mark_lost(QUIC_SSTREAM *qss,
                                uint64_t start,
                                uint64_t end)
{
    UINT_RANGE r;
    r.start = start;
    r.end   = end;

    /*
     * We lost a range of stream data bytes, so reinsert them into the new set,
     * so that they are returned once more by ossl_quic_sstream_get_stream_frame.
     */
    if (!ossl_uint_set_insert(&qss->new_set, &r))
        return 0;

    return 1;
}

int ossl_quic_sstream_mark_lost_fin(QUIC_SSTREAM *qss)
{
    if (qss->acked_final_size)
        /* Does not make sense to lose a FIN after it has been ACKed */
        return 0;

    /* FIN was lost, so we need to transmit it again. */
    qss->sent_final_size = 0;
    return 1;
}

int ossl_quic_sstream_mark_acked(QUIC_SSTREAM *qss,
                                 uint64_t start,
                                 uint64_t end)
{
    UINT_RANGE r;
    r.start = start;
    r.end   = end;

    if (!ossl_uint_set_insert(&qss->acked_set, &r))
        return 0;

    qss_cull(qss);
    return 1;
}

int ossl_quic_sstream_mark_acked_fin(QUIC_SSTREAM *qss)
{
    if (!qss->have_final_size)
        /* Cannot ack final size before we have a final size */
        return 0;

    qss->acked_final_size = 1;
    return 1;
}

void ossl_quic_sstream_fin(QUIC_SSTREAM *qss)
{
    if (qss->have_final_size)
        return;

    qss->have_final_size = 1;
}

int ossl_quic_sstream_get_final_size(QUIC_SSTREAM *qss, uint64_t *final_size)
{
    if (!qss->have_final_size)
        return 0;

    if (final_size != NULL)
        *final_size = qss->ring_buf.head_offset;

    return 1;
}

int ossl_quic_sstream_append(QUIC_SSTREAM *qss,
                             const unsigned char *buf,
                             size_t buf_len,
                             size_t *consumed)
{
    size_t l, consumed_ = 0;
    UINT_RANGE r;
    struct ring_buf old_ring_buf = qss->ring_buf;

    if (qss->have_final_size) {
        *consumed = 0;
        return 0;
    }

    /*
     * Note: It is assumed that ossl_quic_sstream_append will be called during a
     * call to e.g. SSL_write and this function is therefore designed to support
     * such semantics. In particular, the buffer pointed to by buf is only
     * assumed to be valid for the duration of this call, therefore we must copy
     * the data here. We will later copy-and-encrypt the data during packet
     * encryption, so this is a two-copy design. Supporting a one-copy design in
     * the future will require applications to use a different kind of API.
     * Supporting such changes in future will require corresponding enhancements
     * to this code.
     */
    while (buf_len > 0) {
        l = ring_buf_push(&qss->ring_buf, buf, buf_len);
        if (l == 0)
            break;

        buf         += l;
        buf_len     -= l;
        consumed_   += l;
    }

    if (consumed_ > 0) {
        r.start = old_ring_buf.head_offset;
        r.end   = r.start + consumed_ - 1;
        assert(r.end + 1 == qss->ring_buf.head_offset);
        if (!ossl_uint_set_insert(&qss->new_set, &r)) {
            qss->ring_buf = old_ring_buf;
            *consumed = 0;
            return 0;
        }
    }

    *consumed = consumed_;
    return 1;
}

static void qss_cull(QUIC_SSTREAM *qss)
{
    UINT_SET_ITEM *h = ossl_list_uint_set_head(&qss->acked_set);

    /*
     * Potentially cull data from our ring buffer. This can happen once data has
     * been ACKed and we know we are never going to have to transmit it again.
     *
     * Since we use a ring buffer design for simplicity, we cannot cull byte n +
     * k (for k > 0) from the ring buffer until byte n has also been culled.
     * This means if parts of the stream get acknowledged out of order we might
     * keep around some data we technically don't need to for a while. The
     * impact of this is likely to be small and limited to quite a short
     * duration, and doesn't justify the use of a more complex design.
     */

    /*
     * We only need to check the first range entry in the integer set because we
     * can only cull contiguous areas at the start of the ring buffer anyway.
     */
    if (h != NULL)
        ring_buf_cpop_range(&qss->ring_buf, h->range.start, h->range.end,
                            qss->cleanse);
}

int ossl_quic_sstream_set_buffer_size(QUIC_SSTREAM *qss, size_t num_bytes)
{
    return ring_buf_resize(&qss->ring_buf, num_bytes, qss->cleanse);
}

size_t ossl_quic_sstream_get_buffer_size(QUIC_SSTREAM *qss)
{
    return qss->ring_buf.alloc;
}

size_t ossl_quic_sstream_get_buffer_used(QUIC_SSTREAM *qss)
{
    return ring_buf_used(&qss->ring_buf);
}

size_t ossl_quic_sstream_get_buffer_avail(QUIC_SSTREAM *qss)
{
    return ring_buf_avail(&qss->ring_buf);
}

int ossl_quic_sstream_is_totally_acked(QUIC_SSTREAM *qss)
{
    UINT_RANGE r;
    uint64_t cur_size;

    if (qss->have_final_size && !qss->acked_final_size)
        return 0;

    if (ossl_quic_sstream_get_cur_size(qss) == 0)
        return 1;

    if (ossl_list_uint_set_num(&qss->acked_set) != 1)
        return 0;

    r = ossl_list_uint_set_head(&qss->acked_set)->range;
    cur_size = qss->ring_buf.head_offset;

    /*
     * The invariants of UINT_SET guarantee a single list element if we have a
     * single contiguous range, which is what we should have if everything has
     * been acked.
     */
    assert(r.end + 1 <= cur_size);
    return r.start == 0 && r.end + 1 == cur_size;
}

void ossl_quic_sstream_adjust_iov(size_t len,
                                  OSSL_QTX_IOVEC *iov,
                                  size_t num_iov)
{
    size_t running = 0, i, iovlen;

    for (i = 0, running = 0; i < num_iov; ++i) {
        iovlen = iov[i].buf_len;

        if (running >= len)
            iov[i].buf_len = 0;
        else if (running + iovlen > len)
            iov[i].buf_len = len - running;

        running += iovlen;
    }
}

void ossl_quic_sstream_set_cleanse(QUIC_SSTREAM *qss, int cleanse)
{
    qss->cleanse = cleanse;
}
