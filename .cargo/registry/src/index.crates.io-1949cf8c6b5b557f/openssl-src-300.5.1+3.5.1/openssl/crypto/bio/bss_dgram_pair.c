/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <errno.h>
#include "bio_local.h"
#include "internal/cryptlib.h"
#include "internal/safe_math.h"

#if !defined(OPENSSL_NO_DGRAM) && !defined(OPENSSL_NO_SOCK)

OSSL_SAFE_MATH_UNSIGNED(size_t, size_t)

/* ===========================================================================
 * Byte-wise ring buffer which supports pushing and popping blocks of multiple
 * bytes at a time.
 */
struct ring_buf {
    unsigned char *start; /* start of buffer */
    size_t len; /* size of buffer allocation in bytes */
    size_t count; /* number of bytes currently pushed */
    /*
     * These index into start. Where idx[0] == idx[1], the buffer is full
     * (if count is nonzero) and empty otherwise.
     */
    size_t idx[2]; /* 0: head, 1: tail */
};

static int ring_buf_init(struct ring_buf *r, size_t nbytes)
{
    r->start = OPENSSL_malloc(nbytes);
    if (r->start == NULL)
        return 0;

    r->len = nbytes;
    r->idx[0] = r->idx[1] = r->count = 0;
    return 1;
}

static void ring_buf_destroy(struct ring_buf *r)
{
    OPENSSL_free(r->start);
    r->start    = NULL;
    r->len      = 0;
    r->count    = 0;
}

/*
 * Get a pointer to the next place to write data to be pushed to the ring buffer
 * (idx=0), or the next data to be popped from the ring buffer (idx=1). The
 * pointer is written to *buf and the maximum number of bytes which can be
 * read/written are written to *len. After writing data to the buffer, call
 * ring_buf_push/pop() with the number of bytes actually read/written, which
 * must not exceed the returned length.
 */
static void ring_buf_head_tail(struct ring_buf *r, int idx, uint8_t **buf, size_t *len)
{
    size_t max_len = r->len - r->idx[idx];

    if (idx == 0 && max_len > r->len - r->count)
        max_len = r->len - r->count;
    if (idx == 1 && max_len > r->count)
        max_len = r->count;

    *buf = (uint8_t *)r->start + r->idx[idx];
    *len = max_len;
}

#define ring_buf_head(r, buf, len) ring_buf_head_tail((r), 0, (buf), (len))
#define ring_buf_tail(r, buf, len) ring_buf_head_tail((r), 1, (buf), (len))

/*
 * Commit bytes to the ring buffer previously filled after a call to
 * ring_buf_head().
 */
static void ring_buf_push_pop(struct ring_buf *r, int idx, size_t num_bytes)
{
    size_t new_idx;

    /* A single push/pop op cannot wrap around, though it can reach the end.
     * If the caller adheres to the convention of using the length returned
     * by ring_buf_head/tail(), this cannot happen.
     */
    if (!ossl_assert(num_bytes <= r->len - r->idx[idx]))
        return;

    /*
     * Must not overfill the buffer, or pop more than is in the buffer either.
     */
    if (!ossl_assert(idx != 0 ? num_bytes <= r->count
                              : num_bytes + r->count <= r->len))
        return;

    /* Update the index. */
    new_idx = r->idx[idx] + num_bytes;
    if (new_idx == r->len)
        new_idx = 0;

    r->idx[idx] = new_idx;
    if (idx != 0)
        r->count -= num_bytes;
    else
        r->count += num_bytes;
}

#define ring_buf_push(r, num_bytes) ring_buf_push_pop((r), 0, (num_bytes))
#define ring_buf_pop(r, num_bytes) ring_buf_push_pop((r), 1, (num_bytes))

static void ring_buf_clear(struct ring_buf *r)
{
    r->idx[0] = r->idx[1] = r->count = 0;
}

static int ring_buf_resize(struct ring_buf *r, size_t nbytes)
{
    unsigned char *new_start;

    if (r->start == NULL)
        return ring_buf_init(r, nbytes);

    if (nbytes == r->len)
        return 1;

    if (r->count > 0 && nbytes < r->len)
        /* fail shrinking the ring buffer when there is any data in it */
        return 0;

    new_start = OPENSSL_realloc(r->start, nbytes);
    if (new_start == NULL)
        return 0;

    /* Moving tail if it is after (or equal to) head */
    if (r->count > 0) {
        if (r->idx[0] <= r->idx[1]) {
            size_t offset = nbytes - r->len;

            memmove(new_start + r->idx[1] + offset, new_start + r->idx[1],
                    r->len - r->idx[1]);
            r->idx[1] += offset;
        }
    } else {
        /* just reset the head/tail because it might be pointing outside */
        r->idx[0] = r->idx[1] = 0;
    }

    r->start = new_start;
    r->len = nbytes;

    return 1;
}

/* ===========================================================================
 * BIO_s_dgram_pair is documented in BIO_s_dgram_pair(3).
 *
 * INTERNAL DATA STRUCTURE
 *
 * This is managed internally by using a bytewise ring buffer which supports
 * pushing and popping spans of multiple bytes at once. The ring buffer stores
 * internal packets which look like this:
 *
 *   struct dgram_hdr hdr;
 *   uint8_t data[];
 *
 * The header contains the length of the data and metadata such as
 * source/destination addresses.
 *
 * The datagram pair BIO is designed to support both traditional
 * BIO_read/BIO_write (likely to be used by applications) as well as
 * BIO_recvmmsg/BIO_sendmmsg.
 */
struct bio_dgram_pair_st;
static int dgram_pair_write(BIO *bio, const char *buf, int sz_);
static int dgram_pair_read(BIO *bio, char *buf, int sz_);
static int dgram_mem_read(BIO *bio, char *buf, int sz_);
static long dgram_pair_ctrl(BIO *bio, int cmd, long num, void *ptr);
static long dgram_mem_ctrl(BIO *bio, int cmd, long num, void *ptr);
static int dgram_pair_init(BIO *bio);
static int dgram_mem_init(BIO *bio);
static int dgram_pair_free(BIO *bio);
static int dgram_pair_sendmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                               size_t num_msg, uint64_t flags,
                               size_t *num_processed);
static int dgram_pair_recvmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                               size_t num_msg, uint64_t flags,
                               size_t *num_processed);

static int dgram_pair_ctrl_destroy_bio_pair(BIO *bio1);
static size_t dgram_pair_read_inner(struct bio_dgram_pair_st *b, uint8_t *buf,
                                    size_t sz);

#define BIO_MSG_N(array, n) (*(BIO_MSG *)((char *)(array) + (n)*stride))

static const BIO_METHOD dgram_pair_method = {
    BIO_TYPE_DGRAM_PAIR,
    "BIO dgram pair",
    bwrite_conv,
    dgram_pair_write,
    bread_conv,
    dgram_pair_read,
    NULL, /* dgram_pair_puts */
    NULL, /* dgram_pair_gets */
    dgram_pair_ctrl,
    dgram_pair_init,
    dgram_pair_free,
    NULL, /* dgram_pair_callback_ctrl */
    dgram_pair_sendmmsg,
    dgram_pair_recvmmsg,
};

static const BIO_METHOD dgram_mem_method = {
    BIO_TYPE_DGRAM_MEM,
    "BIO dgram mem",
    bwrite_conv,
    dgram_pair_write,
    bread_conv,
    dgram_mem_read,
    NULL, /* dgram_pair_puts */
    NULL, /* dgram_pair_gets */
    dgram_mem_ctrl,
    dgram_mem_init,
    dgram_pair_free,
    NULL, /* dgram_pair_callback_ctrl */
    dgram_pair_sendmmsg,
    dgram_pair_recvmmsg,
};

const BIO_METHOD *BIO_s_dgram_pair(void)
{
    return &dgram_pair_method;
}

const BIO_METHOD *BIO_s_dgram_mem(void)
{
    return &dgram_mem_method;
}

struct dgram_hdr {
    size_t len; /* payload length in bytes, not including this struct */
    BIO_ADDR src_addr, dst_addr; /* family == 0: not present */
};

struct bio_dgram_pair_st {
    /* The other half of the BIO pair. NULL for dgram_mem. */
    BIO *peer;
    /* Writes are directed to our own ringbuf and reads to our peer. */
    struct ring_buf rbuf;
    /* Requested size of rbuf buffer in bytes once we initialize. */
    size_t req_buf_len;
    /* Largest possible datagram size */
    size_t mtu;
    /* Capability flags. */
    uint32_t cap;
    /* The local address to use (if set) */
    BIO_ADDR *local_addr;
    /*
     * This lock protects updates to our rbuf. Since writes are directed to our
     * own rbuf, this means we use this lock for writes and our peer's lock for
     * reads.
     */
    CRYPTO_RWLOCK *lock;
    unsigned int no_trunc          : 1; /* Reads fail if they would truncate */
    unsigned int local_addr_enable : 1; /* Can use BIO_MSG->local? */
    unsigned int role              : 1; /* Determines lock order */
    unsigned int grows_on_write    : 1; /* Set for BIO_s_dgram_mem only */
};

#define MIN_BUF_LEN (1024)

#define is_dgram_pair(b) (b->peer != NULL)

static int dgram_pair_init(BIO *bio)
{
    struct bio_dgram_pair_st *b = OPENSSL_zalloc(sizeof(*b));

    if (b == NULL)
        return 0;

    b->mtu         = 1472;    /* conservative default MTU */
    /* default buffer size */
    b->req_buf_len = 9 * (sizeof(struct dgram_hdr) + b->mtu);

    b->lock = CRYPTO_THREAD_lock_new();
    if (b->lock == NULL) {
        OPENSSL_free(b);
        return 0;
    }

    bio->ptr = b;
    return 1;
}

static int dgram_mem_init(BIO *bio)
{
    struct bio_dgram_pair_st *b;

    if (!dgram_pair_init(bio))
        return 0;

    b = bio->ptr;

    if (ring_buf_init(&b->rbuf, b->req_buf_len) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_BIO_LIB);
        return 0;
    }

    b->grows_on_write = 1;

    bio->init = 1;
    return 1;
}

static int dgram_pair_free(BIO *bio)
{
    struct bio_dgram_pair_st *b;

    if (bio == NULL)
        return 0;

    b = bio->ptr;
    if (!ossl_assert(b != NULL))
        return 0;

    /* We are being freed. Disconnect any peer and destroy buffers. */
    dgram_pair_ctrl_destroy_bio_pair(bio);

    CRYPTO_THREAD_lock_free(b->lock);
    OPENSSL_free(b);
    return 1;
}

/* BIO_make_bio_pair (BIO_C_MAKE_BIO_PAIR) */
static int dgram_pair_ctrl_make_bio_pair(BIO *bio1, BIO *bio2)
{
    struct bio_dgram_pair_st *b1, *b2;

    /* peer must be non-NULL. */
    if (bio1 == NULL || bio2 == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return 0;
    }

    /* Ensure the BIO we have been passed is actually a dgram pair BIO. */
    if (bio1->method != &dgram_pair_method || bio2->method != &dgram_pair_method) {
        ERR_raise_data(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT,
                       "both BIOs must be BIO_dgram_pair");
        return 0;
    }

    b1 = bio1->ptr;
    b2 = bio2->ptr;

    if (!ossl_assert(b1 != NULL && b2 != NULL)) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return 0;
    }

    /*
     * This ctrl cannot be used to associate a BIO pair half which is already
     * associated.
     */
    if (b1->peer != NULL || b2->peer != NULL) {
        ERR_raise_data(ERR_LIB_BIO, BIO_R_IN_USE,
                       "cannot associate a BIO_dgram_pair which is already in use");
        return 0;
    }

    if (!ossl_assert(b1->req_buf_len >= MIN_BUF_LEN
                        && b2->req_buf_len >= MIN_BUF_LEN)) {
        ERR_raise(ERR_LIB_BIO, BIO_R_UNINITIALIZED);
        return 0;
    }

    if (b1->rbuf.len != b1->req_buf_len)
        if (ring_buf_init(&b1->rbuf, b1->req_buf_len) == 0) {
            ERR_raise(ERR_LIB_BIO, ERR_R_BIO_LIB);
            return 0;
        }

    if (b2->rbuf.len != b2->req_buf_len)
        if (ring_buf_init(&b2->rbuf, b2->req_buf_len) == 0) {
            ERR_raise(ERR_LIB_BIO, ERR_R_BIO_LIB);
            ring_buf_destroy(&b1->rbuf);
            return 0;
        }

    b1->peer    = bio2;
    b2->peer    = bio1;
    b1->role    = 0;
    b2->role    = 1;
    bio1->init  = 1;
    bio2->init  = 1;
    return 1;
}

/* BIO_destroy_bio_pair (BIO_C_DESTROY_BIO_PAIR) */
static int dgram_pair_ctrl_destroy_bio_pair(BIO *bio1)
{
    BIO *bio2;
    struct bio_dgram_pair_st *b1 = bio1->ptr, *b2;

    ring_buf_destroy(&b1->rbuf);
    bio1->init = 0;

    BIO_ADDR_free(b1->local_addr);

    /* Early return if we don't have a peer. */
    if (b1->peer == NULL)
        return 1;

    bio2 = b1->peer;
    b2 = bio2->ptr;

    /* Invariant. */
    if (!ossl_assert(b2->peer == bio1))
        return 0;

    /* Free buffers. */
    ring_buf_destroy(&b2->rbuf);

    bio2->init = 0;
    b1->peer = NULL;
    b2->peer = NULL;
    return 1;
}

/* BIO_eof (BIO_CTRL_EOF) */
static int dgram_pair_ctrl_eof(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr, *peerb;

    if (!ossl_assert(b != NULL))
        return -1;

    /* If we aren't initialized, we can never read anything */
    if (!bio->init)
        return 1;
    if (!is_dgram_pair(b))
        return 0;


    peerb = b->peer->ptr;
    if (!ossl_assert(peerb != NULL))
        return -1;

    /*
     * Since we are emulating datagram semantics, never indicate EOF so long as
     * we have a peer.
     */
    return 0;
}

/* BIO_set_write_buf_size (BIO_C_SET_WRITE_BUF_SIZE) */
static int dgram_pair_ctrl_set_write_buf_size(BIO *bio, size_t len)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    /* Changing buffer sizes is not permitted while a peer is connected. */
    if (b->peer != NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_IN_USE);
        return 0;
    }

    /* Enforce minimum size. */
    if (len < MIN_BUF_LEN)
        len = MIN_BUF_LEN;

    if (b->rbuf.start != NULL) {
        if (!ring_buf_resize(&b->rbuf, len))
            return 0;
    }

    b->req_buf_len = len;
    b->grows_on_write = 0;
    return 1;
}

/* BIO_reset (BIO_CTRL_RESET) */
static int dgram_pair_ctrl_reset(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    ring_buf_clear(&b->rbuf);
    return 1;
}

/* BIO_pending (BIO_CTRL_PENDING) (Threadsafe) */
static size_t dgram_pair_ctrl_pending(BIO *bio)
{
    size_t saved_idx, saved_count;
    struct bio_dgram_pair_st *b = bio->ptr, *readb;
    struct dgram_hdr hdr;
    size_t l;

    /* Safe to check; init may not change during this call */
    if (!bio->init)
        return 0;
    if (is_dgram_pair(b))
        readb = b->peer->ptr;
    else
        readb = b;

    if (CRYPTO_THREAD_write_lock(readb->lock) == 0)
        return 0;

    saved_idx   = readb->rbuf.idx[1];
    saved_count = readb->rbuf.count;

    l = dgram_pair_read_inner(readb, (uint8_t *)&hdr, sizeof(hdr));

    readb->rbuf.idx[1] = saved_idx;
    readb->rbuf.count  = saved_count;

    CRYPTO_THREAD_unlock(readb->lock);

    if (!ossl_assert(l == 0 || l == sizeof(hdr)))
        return 0;

    return l > 0 ? hdr.len : 0;
}

/* BIO_get_write_guarantee (BIO_C_GET_WRITE_GUARANTEE) (Threadsafe) */
static size_t dgram_pair_ctrl_get_write_guarantee(BIO *bio)
{
    size_t l;
    struct bio_dgram_pair_st *b = bio->ptr;

    if (CRYPTO_THREAD_read_lock(b->lock) == 0)
        return 0;

    l = b->rbuf.len - b->rbuf.count;
    if (l >= sizeof(struct dgram_hdr))
        l -= sizeof(struct dgram_hdr);

    /*
     * If the amount of buffer space would not be enough to accommodate the
     * worst-case size of a datagram, report no space available.
     */
    if (l < b->mtu)
        l = 0;

    CRYPTO_THREAD_unlock(b->lock);
    return l;
}

/* BIO_dgram_get_local_addr_cap (BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP) */
static int dgram_pair_ctrl_get_local_addr_cap(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr, *readb;

    if (!bio->init)
        return 0;

    if (is_dgram_pair(b))
        readb = b->peer->ptr;
    else
        readb = b;

    return (~readb->cap & (BIO_DGRAM_CAP_HANDLES_SRC_ADDR
                           | BIO_DGRAM_CAP_PROVIDES_DST_ADDR)) == 0;
}

/* BIO_dgram_get_effective_caps (BIO_CTRL_DGRAM_GET_EFFECTIVE_CAPS) */
static int dgram_pair_ctrl_get_effective_caps(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr, *peerb;

    if (b->peer == NULL)
        return 0;

    peerb = b->peer->ptr;

    return peerb->cap;
}

/* BIO_dgram_get_caps (BIO_CTRL_DGRAM_GET_CAPS) */
static uint32_t dgram_pair_ctrl_get_caps(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    return b->cap;
}

/* BIO_dgram_set_caps (BIO_CTRL_DGRAM_SET_CAPS) */
static int dgram_pair_ctrl_set_caps(BIO *bio, uint32_t caps)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    b->cap = caps;
    return 1;
}

/* BIO_dgram_get_local_addr_enable (BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE) */
static int dgram_pair_ctrl_get_local_addr_enable(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    return b->local_addr_enable;
}

/* BIO_dgram_set_local_addr_enable (BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE) */
static int dgram_pair_ctrl_set_local_addr_enable(BIO *bio, int enable)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    if (dgram_pair_ctrl_get_local_addr_cap(bio) == 0)
        return 0;

    b->local_addr_enable = (enable != 0 ? 1 : 0);
    return 1;
}

/* BIO_dgram_get_mtu (BIO_CTRL_DGRAM_GET_MTU) */
static int dgram_pair_ctrl_get_mtu(BIO *bio)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    return b->mtu;
}

/* BIO_dgram_set_mtu (BIO_CTRL_DGRAM_SET_MTU) */
static int dgram_pair_ctrl_set_mtu(BIO *bio, size_t mtu)
{
    struct bio_dgram_pair_st *b = bio->ptr, *peerb;

    b->mtu = mtu;

    if (b->peer != NULL) {
        peerb = b->peer->ptr;
        peerb->mtu = mtu;
    }

    return 1;
}

/* BIO_dgram_set0_local_addr (BIO_CTRL_DGRAM_SET0_LOCAL_ADDR) */
static int dgram_pair_ctrl_set0_local_addr(BIO *bio, BIO_ADDR *addr)
{
    struct bio_dgram_pair_st *b = bio->ptr;

    BIO_ADDR_free(b->local_addr);
    b->local_addr = addr;
    return 1;
}

/* Partially threadsafe (some commands) */
static long dgram_mem_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret = 1;
    struct bio_dgram_pair_st *b = bio->ptr;

    if (!ossl_assert(b != NULL))
        return 0;

    switch (cmd) {
    /*
     * BIO_set_write_buf_size: Set the size of the ring buffer used for storing
     * datagrams. No more writes can be performed once the buffer is filled up,
     * until reads are performed. This cannot be used after a peer is connected.
     */
    case BIO_C_SET_WRITE_BUF_SIZE: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_set_write_buf_size(bio, (size_t)num);
        break;

    /*
     * BIO_get_write_buf_size: Get ring buffer size.
     */
    case BIO_C_GET_WRITE_BUF_SIZE: /* Non-threadsafe */
        ret = (long)b->req_buf_len;
        break;

    /*
     * BIO_reset: Clear all data which was written to this side of the pair.
     */
    case BIO_CTRL_RESET: /* Non-threadsafe */
        dgram_pair_ctrl_reset(bio);
        break;

    /*
     * BIO_get_write_guarantee: Any BIO_write providing a buffer less than or
     * equal to this value is guaranteed to succeed.
     */
    case BIO_C_GET_WRITE_GUARANTEE: /* Threadsafe */
        ret = (long)dgram_pair_ctrl_get_write_guarantee(bio);
        break;

    /* BIO_pending: Bytes available to read. */
    case BIO_CTRL_PENDING: /* Threadsafe */
        ret = (long)dgram_pair_ctrl_pending(bio);
        break;

    /* BIO_flush: No-op. */
    case BIO_CTRL_FLUSH: /* Threadsafe */
        break;

    /* BIO_dgram_get_no_trunc */
    case BIO_CTRL_DGRAM_GET_NO_TRUNC: /* Non-threadsafe */
        ret = (long)b->no_trunc;
        break;

    /* BIO_dgram_set_no_trunc */
    case BIO_CTRL_DGRAM_SET_NO_TRUNC: /* Non-threadsafe */
        b->no_trunc = (num > 0);
        break;

    /* BIO_dgram_get_local_addr_enable */
    case BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE: /* Non-threadsafe */
        *(int *)ptr = (int)dgram_pair_ctrl_get_local_addr_enable(bio);
        break;

    /* BIO_dgram_set_local_addr_enable */
    case BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_set_local_addr_enable(bio, num);
        break;

    /* BIO_dgram_get_local_addr_cap: Can local addresses be supported? */
    case BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_get_local_addr_cap(bio);
        break;

    /* BIO_dgram_get_effective_caps */
    case BIO_CTRL_DGRAM_GET_EFFECTIVE_CAPS: /* Non-threadsafe */
    /* BIO_dgram_get_caps */
    case BIO_CTRL_DGRAM_GET_CAPS: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_get_caps(bio);
        break;

    /* BIO_dgram_set_caps */
    case BIO_CTRL_DGRAM_SET_CAPS: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_set_caps(bio, (uint32_t)num);
        break;

    /* BIO_dgram_get_mtu */
    case BIO_CTRL_DGRAM_GET_MTU: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_get_mtu(bio);
        break;

    /* BIO_dgram_set_mtu */
    case BIO_CTRL_DGRAM_SET_MTU: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_set_mtu(bio, (uint32_t)num);
        break;

    case BIO_CTRL_DGRAM_SET0_LOCAL_ADDR:
        ret = (long)dgram_pair_ctrl_set0_local_addr(bio, (BIO_ADDR *)ptr);
        break;

    /*
     * BIO_eof: Returns whether this half of the BIO pair is empty of data to
     * read.
     */
    case BIO_CTRL_EOF: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_eof(bio);
        break;

    default:
        ret = 0;
        break;
    }

    return ret;
}

static long dgram_pair_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret = 1;

    switch (cmd) {
    /*
     * BIO_make_bio_pair: this is usually used by BIO_new_dgram_pair, though it
     * may be used manually after manually creating each half of a BIO pair
     * using BIO_new. This only needs to be called on one of the BIOs.
     */
    case BIO_C_MAKE_BIO_PAIR: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_make_bio_pair(bio, (BIO *)ptr);
        break;

    /*
     * BIO_destroy_bio_pair: Manually disconnect two halves of a BIO pair so
     * that they are no longer peers.
     */
    case BIO_C_DESTROY_BIO_PAIR: /* Non-threadsafe */
        dgram_pair_ctrl_destroy_bio_pair(bio);
        break;

    /* BIO_dgram_get_effective_caps */
    case BIO_CTRL_DGRAM_GET_EFFECTIVE_CAPS: /* Non-threadsafe */
        ret = (long)dgram_pair_ctrl_get_effective_caps(bio);
        break;

    default:
        ret = dgram_mem_ctrl(bio, cmd, num, ptr);
        break;
    }

    return ret;
}

int BIO_new_bio_dgram_pair(BIO **pbio1, size_t writebuf1,
                           BIO **pbio2, size_t writebuf2)
{
    int ret = 0;
    long r;
    BIO *bio1 = NULL, *bio2 = NULL;

    bio1 = BIO_new(BIO_s_dgram_pair());
    if (bio1 == NULL)
        goto err;

    bio2 = BIO_new(BIO_s_dgram_pair());
    if (bio2 == NULL)
        goto err;

    if (writebuf1 > 0) {
        r = BIO_set_write_buf_size(bio1, writebuf1);
        if (r == 0)
            goto err;
    }

    if (writebuf2 > 0) {
        r = BIO_set_write_buf_size(bio2, writebuf2);
        if (r == 0)
            goto err;
    }

    r = BIO_make_bio_pair(bio1, bio2);
    if (r == 0)
        goto err;

    ret = 1;
err:
    if (ret == 0) {
        BIO_free(bio1);
        bio1 = NULL;
        BIO_free(bio2);
        bio2 = NULL;
    }

    *pbio1 = bio1;
    *pbio2 = bio2;
    return ret;
}

/* Must hold peer write lock */
static size_t dgram_pair_read_inner(struct bio_dgram_pair_st *b, uint8_t *buf, size_t sz)
{
    size_t total_read = 0;

    /*
     * We repeat pops from the ring buffer for as long as we have more
     * application *buffer to fill until we fail. We may not be able to pop
     * enough data to fill the buffer in one operation if the ring buffer wraps
     * around, but there may still be more data available.
     */
    while (sz > 0) {
        uint8_t *src_buf = NULL;
        size_t src_len = 0;

        /*
         * There are two BIO instances, each with a ringbuf. We read from the
         * peer ringbuf and write to our own ringbuf.
         */
        ring_buf_tail(&b->rbuf, &src_buf, &src_len);
        if (src_len == 0)
            break;

        if (src_len > sz)
            src_len = sz;

        if (buf != NULL)
            memcpy(buf, src_buf, src_len);

        ring_buf_pop(&b->rbuf, src_len);

        if (buf != NULL)
            buf += src_len;
        total_read  += src_len;
        sz          -= src_len;
    }

    return total_read;
}

/*
 * Must hold peer write lock. Returns number of bytes processed or negated BIO
 * response code.
 */
static ossl_ssize_t dgram_pair_read_actual(BIO *bio, char *buf, size_t sz,
                                           BIO_ADDR *local, BIO_ADDR *peer,
                                           int is_multi)
{
    size_t l, trunc = 0, saved_idx, saved_count;
    struct bio_dgram_pair_st *b = bio->ptr, *readb;
    struct dgram_hdr hdr;

    if (!is_multi)
        BIO_clear_retry_flags(bio);

    if (!bio->init)
        return -BIO_R_UNINITIALIZED;

    if (!ossl_assert(b != NULL))
        return -BIO_R_TRANSFER_ERROR;

    if (is_dgram_pair(b))
        readb = b->peer->ptr;
    else
        readb = b;
    if (!ossl_assert(readb != NULL && readb->rbuf.start != NULL))
        return -BIO_R_TRANSFER_ERROR;

    if (sz > 0 && buf == NULL)
        return -BIO_R_INVALID_ARGUMENT;

    /* If the caller wants to know the local address, it must be enabled */
    if (local != NULL && b->local_addr_enable == 0)
        return -BIO_R_LOCAL_ADDR_NOT_AVAILABLE;

    /* Read the header. */
    saved_idx   = readb->rbuf.idx[1];
    saved_count = readb->rbuf.count;
    l = dgram_pair_read_inner(readb, (uint8_t *)&hdr, sizeof(hdr));
    if (l == 0) {
        /* Buffer was empty. */
        if (!is_multi)
            BIO_set_retry_read(bio);
        return -BIO_R_NON_FATAL;
    }

    if (!ossl_assert(l == sizeof(hdr)))
        /*
         * This should not be possible as headers (and their following payloads)
         * should always be written atomically.
         */
        return -BIO_R_BROKEN_PIPE;

    if (sz > hdr.len) {
        sz = hdr.len;
    } else if (sz < hdr.len) {
        /* Truncation is occurring. */
        trunc = hdr.len - sz;
        if (b->no_trunc) {
            /* Restore original state. */
            readb->rbuf.idx[1] = saved_idx;
            readb->rbuf.count  = saved_count;
            return -BIO_R_NON_FATAL;
        }
    }

    l = dgram_pair_read_inner(readb, (uint8_t *)buf, sz);
    if (!ossl_assert(l == sz))
        /* We were somehow not able to read the entire datagram. */
        return -BIO_R_TRANSFER_ERROR;

    /*
     * If the datagram was truncated due to an inadequate buffer, discard the
     * remainder.
     */
    if (trunc > 0 && !ossl_assert(dgram_pair_read_inner(readb, NULL, trunc) == trunc))
        /* We were somehow not able to read/skip the entire datagram. */
        return -BIO_R_TRANSFER_ERROR;

    if (local != NULL)
        *local = hdr.dst_addr;
    if (peer != NULL)
        *peer  = hdr.src_addr;

    return (ossl_ssize_t)l;
}

/* Threadsafe */
static int dgram_pair_lock_both_write(struct bio_dgram_pair_st *a,
                                      struct bio_dgram_pair_st *b)
{
    struct bio_dgram_pair_st *x, *y;

    x = (a->role == 1) ? a : b;
    y = (a->role == 1) ? b : a;

    if (!ossl_assert(a->role != b->role))
        return 0;

    if (!ossl_assert(a != b && x != y))
        return 0;

    if (CRYPTO_THREAD_write_lock(x->lock) == 0)
        return 0;

    if (CRYPTO_THREAD_write_lock(y->lock) == 0) {
        CRYPTO_THREAD_unlock(x->lock);
        return 0;
    }

    return 1;
}

static void dgram_pair_unlock_both(struct bio_dgram_pair_st *a,
                                   struct bio_dgram_pair_st *b)
{
    CRYPTO_THREAD_unlock(a->lock);
    CRYPTO_THREAD_unlock(b->lock);
}

/* Threadsafe */
static int dgram_pair_read(BIO *bio, char *buf, int sz_)
{
    int ret;
    ossl_ssize_t l;
    struct bio_dgram_pair_st *b = bio->ptr, *peerb;

    if (sz_ < 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }

    if (b->peer == NULL) {
        ERR_raise(ERR_LIB_BIO, BIO_R_BROKEN_PIPE);
        return -1;
    }

    peerb = b->peer->ptr;

    /*
     * For BIO_read we have to acquire both locks because we touch the retry
     * flags on the local bio. (This is avoided in the recvmmsg case as it does
     * not touch the retry flags.)
     */
    if (dgram_pair_lock_both_write(peerb, b) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        return -1;
    }

    l = dgram_pair_read_actual(bio, buf, (size_t)sz_, NULL, NULL, 0);
    if (l < 0) {
        if (l != -BIO_R_NON_FATAL)
            ERR_raise(ERR_LIB_BIO, -l);
        ret = -1;
    } else {
        ret = (int)l;
    }

    dgram_pair_unlock_both(peerb, b);
    return ret;
}

/* Threadsafe */
static int dgram_pair_recvmmsg(BIO *bio, BIO_MSG *msg,
                               size_t stride, size_t num_msg,
                               uint64_t flags,
                               size_t *num_processed)
{
    int ret;
    ossl_ssize_t l;
    BIO_MSG *m;
    size_t i;
    struct bio_dgram_pair_st *b = bio->ptr, *readb;

    if (num_msg == 0) {
        *num_processed = 0;
        return 1;
    }

    if (!bio->init) {
        ERR_raise(ERR_LIB_BIO, BIO_R_BROKEN_PIPE);
        *num_processed = 0;
        return 0;
    }

    if (is_dgram_pair(b))
        readb = b->peer->ptr;
    else
        readb = b;

    if (CRYPTO_THREAD_write_lock(readb->lock) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        *num_processed = 0;
        return 0;
    }

    for (i = 0; i < num_msg; ++i) {
        m = &BIO_MSG_N(msg, i);
        l = dgram_pair_read_actual(bio, m->data, m->data_len,
                                   m->local, m->peer, 1);
        if (l < 0) {
            *num_processed = i;
            if (i > 0) {
                ret = 1;
            } else {
                ERR_raise(ERR_LIB_BIO, -l);
                ret = 0;
            }
            goto out;
        }

        m->data_len = l;
        m->flags    = 0;
    }

    *num_processed = i;
    ret = 1;
out:
    CRYPTO_THREAD_unlock(readb->lock);
    return ret;
}

/* Threadsafe */
static int dgram_mem_read(BIO *bio, char *buf, int sz_)
{
    int ret;
    ossl_ssize_t l;
    struct bio_dgram_pair_st *b = bio->ptr;

    if (sz_ < 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }

    if (CRYPTO_THREAD_write_lock(b->lock) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        return -1;
    }

    l = dgram_pair_read_actual(bio, buf, (size_t)sz_, NULL, NULL, 0);
    if (l < 0) {
        if (l != -BIO_R_NON_FATAL)
            ERR_raise(ERR_LIB_BIO, -l);
        ret = -1;
    } else {
        ret = (int)l;
    }

    CRYPTO_THREAD_unlock(b->lock);
    return ret;
}

/*
 * Calculate the array growth based on the target size.
 *
 * The growth factor is a rational number and is defined by a numerator
 * and a denominator.  According to Andrew Koenig in his paper "Why Are
 * Vectors Efficient?" from JOOP 11(5) 1998, this factor should be less
 * than the golden ratio (1.618...).
 *
 * We use an expansion factor of 8 / 5 = 1.6
 */
static const size_t max_rbuf_size = SIZE_MAX / 2; /* unlimited in practice */
static ossl_inline size_t compute_rbuf_growth(size_t target, size_t current)
{
    int err = 0;

    while (current < target) {
        if (current >= max_rbuf_size)
            return 0;

        current = safe_muldiv_size_t(current, 8, 5, &err);
        if (err)
            return 0;
        if (current >= max_rbuf_size)
            current = max_rbuf_size;
    }
    return current;
}

/* Must hold local write lock */
static size_t dgram_pair_write_inner(struct bio_dgram_pair_st *b,
                                     const uint8_t *buf, size_t sz)
{
    size_t total_written = 0;

    /*
     * We repeat pushes to the ring buffer for as long as we have data until we
     * fail. We may not be able to push in one operation if the ring buffer
     * wraps around, but there may still be more room for data.
     */
    while (sz > 0) {
        size_t dst_len;
        uint8_t *dst_buf;

        /*
         * There are two BIO instances, each with a ringbuf. We write to our own
         * ringbuf and read from the peer ringbuf.
         */
        ring_buf_head(&b->rbuf, &dst_buf, &dst_len);
        if (dst_len == 0) {
            size_t new_len;

            if (!b->grows_on_write) /* resize only if size not set explicitly */
                break;
            /* increase the size */
            new_len = compute_rbuf_growth(b->req_buf_len + sz, b->req_buf_len);
            if (new_len == 0 || !ring_buf_resize(&b->rbuf, new_len))
                break;
            b->req_buf_len = new_len;
        }

        if (dst_len > sz)
            dst_len = sz;

        memcpy(dst_buf, buf, dst_len);
        ring_buf_push(&b->rbuf, dst_len);

        buf             += dst_len;
        sz              -= dst_len;
        total_written   += dst_len;
    }

    return total_written;
}

/*
 * Must hold local write lock. Returns number of bytes processed or negated BIO
 * response code.
 */
static ossl_ssize_t dgram_pair_write_actual(BIO *bio, const char *buf, size_t sz,
                                            const BIO_ADDR *local, const BIO_ADDR *peer,
                                            int is_multi)
{
    static const BIO_ADDR zero_addr;
    size_t saved_idx, saved_count;
    struct bio_dgram_pair_st *b = bio->ptr, *readb;
    struct dgram_hdr hdr = {0};

    if (!is_multi)
        BIO_clear_retry_flags(bio);

    if (!bio->init)
        return -BIO_R_UNINITIALIZED;

    if (!ossl_assert(b != NULL && b->rbuf.start != NULL))
        return -BIO_R_TRANSFER_ERROR;

    if (sz > 0 && buf == NULL)
        return -BIO_R_INVALID_ARGUMENT;

    if (local != NULL && b->local_addr_enable == 0)
        return -BIO_R_LOCAL_ADDR_NOT_AVAILABLE;

    if (is_dgram_pair(b))
        readb = b->peer->ptr;
    else
        readb = b;
    if (peer != NULL && (readb->cap & BIO_DGRAM_CAP_HANDLES_DST_ADDR) == 0)
        return -BIO_R_PEER_ADDR_NOT_AVAILABLE;

    hdr.len = sz;
    hdr.dst_addr = (peer != NULL ? *peer : zero_addr);
    if (local == NULL)
        local = b->local_addr;
    hdr.src_addr = (local != NULL ? *local : zero_addr);

    saved_idx   = b->rbuf.idx[0];
    saved_count = b->rbuf.count;
    if (dgram_pair_write_inner(b, (const uint8_t *)&hdr, sizeof(hdr)) != sizeof(hdr)
            || dgram_pair_write_inner(b, (const uint8_t *)buf, sz) != sz) {
        /*
         * We were not able to push the header and the entirety of the payload
         * onto the ring buffer, so abort and roll back the ring buffer state.
         */
        b->rbuf.idx[0] = saved_idx;
        b->rbuf.count  = saved_count;
        if (!is_multi)
            BIO_set_retry_write(bio);
        return -BIO_R_NON_FATAL;
    }

    return sz;
}

/* Threadsafe */
static int dgram_pair_write(BIO *bio, const char *buf, int sz_)
{
    int ret;
    ossl_ssize_t l;
    struct bio_dgram_pair_st *b = bio->ptr;

    if (sz_ < 0) {
        ERR_raise(ERR_LIB_BIO, BIO_R_INVALID_ARGUMENT);
        return -1;
    }

    if (CRYPTO_THREAD_write_lock(b->lock) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        return -1;
    }

    l = dgram_pair_write_actual(bio, buf, (size_t)sz_, NULL, NULL, 0);
    if (l < 0) {
        ERR_raise(ERR_LIB_BIO, -l);
        ret = -1;
    } else {
        ret = (int)l;
    }

    CRYPTO_THREAD_unlock(b->lock);
    return ret;
}

/* Threadsafe */
static int dgram_pair_sendmmsg(BIO *bio, BIO_MSG *msg,
                               size_t stride, size_t num_msg,
                               uint64_t flags, size_t *num_processed)
{
    ossl_ssize_t ret, l;
    BIO_MSG *m;
    size_t i;
    struct bio_dgram_pair_st *b = bio->ptr;

    if (num_msg == 0) {
        *num_processed = 0;
        return 1;
    }

    if (CRYPTO_THREAD_write_lock(b->lock) == 0) {
        ERR_raise(ERR_LIB_BIO, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        *num_processed = 0;
        return 0;
    }

    for (i = 0; i < num_msg; ++i) {
        m = &BIO_MSG_N(msg, i);
        l = dgram_pair_write_actual(bio, m->data, m->data_len,
                                    m->local, m->peer, 1);
        if (l < 0) {
            *num_processed = i;
            if (i > 0) {
                ret = 1;
            } else {
                ERR_raise(ERR_LIB_BIO, -l);
                ret = 0;
            }
            goto out;
        }

        m->flags = 0;
    }

    *num_processed = i;
    ret = 1;
out:
    CRYPTO_THREAD_unlock(b->lock);
    return ret;
}

#endif
