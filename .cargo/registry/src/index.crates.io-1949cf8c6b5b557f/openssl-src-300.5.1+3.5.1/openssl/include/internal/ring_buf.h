/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_RING_BUF_H
# define OSSL_INTERNAL_RING_BUF_H
# pragma once

# include <openssl/e_os2.h>              /* For 'ossl_inline' */
# include "internal/safe_math.h"

/*
 * ==================================================================
 * Byte-wise ring buffer which supports pushing and popping blocks of multiple
 * bytes at a time. The logical offset of each byte for the purposes of a QUIC
 * stream is tracked. Bytes can be popped from the ring buffer in two stages;
 * first they are popped, and then they are culled. Bytes which have been popped
 * but not yet culled will not be overwritten, and can be restored.
 */
struct ring_buf {
    void       *start;
    size_t      alloc;        /* size of buffer allocation in bytes */

    /*
     * Logical offset of the head (where we append to). This is the current size
     * of the QUIC stream. This increases monotonically.
     */
    uint64_t    head_offset;

    /*
     * Logical offset of the cull tail. Data is no longer needed and is
     * deallocated as the cull tail advances, which occurs as data is
     * acknowledged. This increases monotonically.
     */
    uint64_t    ctail_offset;
};

OSSL_SAFE_MATH_UNSIGNED(u64, uint64_t)

#define MAX_OFFSET   (((uint64_t)1) << 62) /* QUIC-imposed limit */

static ossl_inline int ring_buf_init(struct ring_buf *r)
{
    r->start = NULL;
    r->alloc = 0;
    r->head_offset = r->ctail_offset = 0;
    return 1;
}

static ossl_inline void ring_buf_destroy(struct ring_buf *r, int cleanse)
{
    if (cleanse)
        OPENSSL_clear_free(r->start, r->alloc);
    else
        OPENSSL_free(r->start);
    r->start = NULL;
    r->alloc = 0;
}

static ossl_inline size_t ring_buf_used(struct ring_buf *r)
{
    return (size_t)(r->head_offset - r->ctail_offset);
}

static ossl_inline size_t ring_buf_avail(struct ring_buf *r)
{
    return r->alloc - ring_buf_used(r);
}

static ossl_inline int ring_buf_write_at(struct ring_buf *r,
                                         uint64_t logical_offset,
                                         const unsigned char *buf,
                                         size_t buf_len)
{
    size_t avail, idx, l;
    unsigned char *start = r->start;
    int i, err = 0;

    avail = ring_buf_avail(r);
    if (logical_offset < r->ctail_offset
        || safe_add_u64(logical_offset, buf_len, &err)
           > safe_add_u64(r->head_offset, avail, &err)
        || safe_add_u64(r->head_offset, buf_len, &err)
           > MAX_OFFSET
        || err)
        return 0;

    for (i = 0; buf_len > 0 && i < 2; ++i) {
        idx = logical_offset % r->alloc;
        l = r->alloc - idx;
        if (buf_len < l)
            l = buf_len;

        memcpy(start + idx, buf, l);
        if (r->head_offset < logical_offset + l)
            r->head_offset = logical_offset + l;

        logical_offset += l;
        buf += l;
        buf_len -= l;
    }

    assert(buf_len == 0);

    return 1;
}

static ossl_inline size_t ring_buf_push(struct ring_buf *r,
                                        const unsigned char *buf,
                                        size_t buf_len)
{
    size_t pushed = 0, avail, idx, l;
    unsigned char *start = r->start;

    for (;;) {
        avail = ring_buf_avail(r);
        if (buf_len > avail)
            buf_len = avail;

        if (buf_len > MAX_OFFSET - r->head_offset)
            buf_len = (size_t)(MAX_OFFSET - r->head_offset);

        if (buf_len == 0)
            break;

        idx = r->head_offset % r->alloc;
        l = r->alloc - idx;
        if (buf_len < l)
            l = buf_len;

        memcpy(start + idx, buf, l);
        r->head_offset  += l;
        buf             += l;
        buf_len         -= l;
        pushed          += l;
    }

    return pushed;
}

static ossl_inline const unsigned char *ring_buf_get_ptr(const struct ring_buf *r,
                                                         uint64_t logical_offset,
                                                         size_t *max_len)
{
    unsigned char *start = r->start;
    size_t idx;

    if (logical_offset >= r->head_offset || logical_offset < r->ctail_offset)
        return NULL;
    idx = logical_offset % r->alloc;
    *max_len = r->alloc - idx;
    return start + idx;
}

/*
 * Retrieves data out of the read side of the ring buffer starting at the given
 * logical offset. *buf is set to point to a contiguous span of bytes and
 * *buf_len is set to the number of contiguous bytes. After this function
 * returns, there may or may not be more bytes available at the logical offset
 * of (logical_offset + *buf_len) by calling this function again. If the logical
 * offset is out of the range retained by the ring buffer, returns 0, else
 * returns 1. A logical offset at the end of the range retained by the ring
 * buffer is not considered an error and is returned with a *buf_len of 0.
 *
 * The ring buffer state is not changed.
 */
static ossl_inline int ring_buf_get_buf_at(const struct ring_buf *r,
                                           uint64_t logical_offset,
                                           const unsigned char **buf,
                                           size_t *buf_len)
{
    const unsigned char *start = r->start;
    size_t idx, l;

    if (logical_offset > r->head_offset || logical_offset < r->ctail_offset)
        return 0;

    if (r->alloc == 0) {
        *buf        = NULL;
        *buf_len    = 0;
        return 1;
    }

    idx = logical_offset % r->alloc;
    l   = (size_t)(r->head_offset - logical_offset);
    if (l > r->alloc - idx)
        l = r->alloc - idx;

    *buf        = start + idx;
    *buf_len    = l;
    return 1;
}

static ossl_inline void ring_buf_cpop_range(struct ring_buf *r,
                                            uint64_t start, uint64_t end,
                                            int cleanse)
{
    assert(end >= start);

    if (start > r->ctail_offset || end >= MAX_OFFSET)
        return;

    if (cleanse && r->alloc > 0 && end > r->ctail_offset) {
        size_t idx = r->ctail_offset % r->alloc;
        uint64_t cleanse_end = end + 1;
        size_t l;

        if (cleanse_end > r->head_offset)
            cleanse_end = r->head_offset;
        l = (size_t)(cleanse_end - r->ctail_offset);
        if (l > r->alloc - idx) {
            OPENSSL_cleanse((unsigned char *)r->start + idx, r->alloc - idx);
            l -= r->alloc - idx;
            idx = 0;
        }
        if (l > 0)
            OPENSSL_cleanse((unsigned char *)r->start + idx, l);
    }

    r->ctail_offset = end + 1;
    /* Allow culling unpushed data */
    if (r->head_offset < r->ctail_offset)
        r->head_offset = r->ctail_offset;
}

static ossl_inline int ring_buf_resize(struct ring_buf *r, size_t num_bytes,
                                       int cleanse)
{
    struct ring_buf rnew = {0};
    const unsigned char *src = NULL;
    size_t src_len = 0, copied = 0;

    if (num_bytes == r->alloc)
        return 1;

    if (num_bytes < ring_buf_used(r))
        return 0;

    rnew.start = OPENSSL_malloc(num_bytes);
    if (rnew.start == NULL)
        return 0;

    rnew.alloc          = num_bytes;
    rnew.head_offset    = r->head_offset - ring_buf_used(r);
    rnew.ctail_offset   = rnew.head_offset;

    for (;;) {
        if (!ring_buf_get_buf_at(r, r->ctail_offset + copied, &src, &src_len)) {
            OPENSSL_free(rnew.start);
            return 0;
        }

        if (src_len == 0)
            break;

        if (ring_buf_push(&rnew, src, src_len) != src_len) {
            OPENSSL_free(rnew.start);
            return 0;
        }

        copied += src_len;
    }

    assert(rnew.head_offset == r->head_offset);
    rnew.ctail_offset = r->ctail_offset;

    ring_buf_destroy(r, cleanse);
    memcpy(r, &rnew, sizeof(*r));
    return 1;
}

#endif                          /* OSSL_INTERNAL_RING_BUF_H */
