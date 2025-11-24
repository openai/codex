/*
 * Copyright 2015-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include "internal/packet.h"
#if !defined OPENSSL_NO_QUIC && !defined FIPS_MODULE
# include "internal/packet_quic.h"
#endif
#include <openssl/err.h>

#define DEFAULT_BUF_SIZE    256

int WPACKET_allocate_bytes(WPACKET *pkt, size_t len, unsigned char **allocbytes)
{
    if (!WPACKET_reserve_bytes(pkt, len, allocbytes))
        return 0;

    pkt->written += len;
    pkt->curr += len;
    return 1;
}

int WPACKET_sub_allocate_bytes__(WPACKET *pkt, size_t len,
                                 unsigned char **allocbytes, size_t lenbytes)
{
    if (!WPACKET_start_sub_packet_len__(pkt, lenbytes)
            || !WPACKET_allocate_bytes(pkt, len, allocbytes)
            || !WPACKET_close(pkt))
        return 0;

    return 1;
}

#define GETBUF(p)   (((p)->staticbuf != NULL) \
                     ? (p)->staticbuf \
                     : ((p)->buf != NULL \
                        ? (unsigned char *)(p)->buf->data \
                        : NULL))

int WPACKET_reserve_bytes(WPACKET *pkt, size_t len, unsigned char **allocbytes)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(pkt->subs != NULL && len != 0))
        return 0;

    if (pkt->maxsize - pkt->written < len)
        return 0;

    if (pkt->buf != NULL && (pkt->buf->length - pkt->written < len)) {
        size_t newlen;
        size_t reflen;

        reflen = (len > pkt->buf->length) ? len : pkt->buf->length;

        if (reflen > SIZE_MAX / 2) {
            newlen = SIZE_MAX;
        } else {
            newlen = reflen * 2;
            if (newlen < DEFAULT_BUF_SIZE)
                newlen = DEFAULT_BUF_SIZE;
        }
        if (BUF_MEM_grow(pkt->buf, newlen) == 0)
            return 0;
    }
    if (allocbytes != NULL) {
        *allocbytes = WPACKET_get_curr(pkt);
        if (pkt->endfirst && *allocbytes != NULL)
            *allocbytes -= len;
    }

    return 1;
}

int WPACKET_sub_reserve_bytes__(WPACKET *pkt, size_t len,
                                unsigned char **allocbytes, size_t lenbytes)
{
    if (pkt->endfirst && lenbytes > 0)
        return 0;

    if (!WPACKET_reserve_bytes(pkt, lenbytes + len, allocbytes))
        return 0;

    if (*allocbytes != NULL)
        *allocbytes += lenbytes;

    return 1;
}

static size_t maxmaxsize(size_t lenbytes)
{
    if (lenbytes >= sizeof(size_t) || lenbytes == 0)
        return SIZE_MAX;

    return ((size_t)1 << (lenbytes * 8)) - 1 + lenbytes;
}

static int wpacket_intern_init_len(WPACKET *pkt, size_t lenbytes)
{
    unsigned char *lenchars;

    pkt->curr = 0;
    pkt->written = 0;

    if ((pkt->subs = OPENSSL_zalloc(sizeof(*pkt->subs))) == NULL)
        return 0;

    if (lenbytes == 0)
        return 1;

    pkt->subs->pwritten = lenbytes;
    pkt->subs->lenbytes = lenbytes;

    if (!WPACKET_allocate_bytes(pkt, lenbytes, &lenchars)) {
        OPENSSL_free(pkt->subs);
        pkt->subs = NULL;
        return 0;
    }
    pkt->subs->packet_len = 0;

    return 1;
}

int WPACKET_init_static_len(WPACKET *pkt, unsigned char *buf, size_t len,
                            size_t lenbytes)
{
    size_t max = maxmaxsize(lenbytes);

    /* Internal API, so should not fail */
    if (!ossl_assert(buf != NULL && len > 0))
        return 0;

    pkt->staticbuf = buf;
    pkt->buf = NULL;
    pkt->maxsize = (max < len) ? max : len;
    pkt->endfirst = 0;

    return wpacket_intern_init_len(pkt, lenbytes);
}

int WPACKET_init_der(WPACKET *pkt, unsigned char *buf, size_t len)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(buf != NULL && len > 0))
        return 0;

    pkt->staticbuf = buf;
    pkt->buf = NULL;
    pkt->maxsize = len;
    pkt->endfirst = 1;

    return wpacket_intern_init_len(pkt, 0);
}

int WPACKET_init_len(WPACKET *pkt, BUF_MEM *buf, size_t lenbytes)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(buf != NULL))
        return 0;

    pkt->staticbuf = NULL;
    pkt->buf = buf;
    pkt->maxsize = maxmaxsize(lenbytes);
    pkt->endfirst = 0;

    return wpacket_intern_init_len(pkt, lenbytes);
}

int WPACKET_init(WPACKET *pkt, BUF_MEM *buf)
{
    return WPACKET_init_len(pkt, buf, 0);
}

int WPACKET_init_null(WPACKET *pkt, size_t lenbytes)
{
    pkt->staticbuf = NULL;
    pkt->buf = NULL;
    pkt->maxsize = maxmaxsize(lenbytes);
    pkt->endfirst = 0;

    return wpacket_intern_init_len(pkt, 0);
}

int WPACKET_init_null_der(WPACKET *pkt)
{
    pkt->staticbuf = NULL;
    pkt->buf = NULL;
    pkt->maxsize = SIZE_MAX;
    pkt->endfirst = 1;

    return wpacket_intern_init_len(pkt, 0);
}

int WPACKET_set_flags(WPACKET *pkt, unsigned int flags)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(pkt->subs != NULL))
        return 0;

    pkt->subs->flags = flags;

    return 1;
}

/* Store the |value| of length |len| at location |data| */
static int put_value(unsigned char *data, uint64_t value, size_t len)
{
    if (data == NULL)
        return 1;

    for (data += len - 1; len > 0; len--) {
        *data = (unsigned char)(value & 0xff);
        data--;
        value >>= 8;
    }

    /* Check whether we could fit the value in the assigned number of bytes */
    if (value > 0)
        return 0;

    return 1;
}

#if !defined OPENSSL_NO_QUIC && !defined FIPS_MODULE
static int put_quic_value(unsigned char *data, size_t value, size_t len)
{
    if (data == NULL)
        return 1;

    /* Value too large for field. */
    if (ossl_quic_vlint_encode_len(value) > len)
        return 0;

    ossl_quic_vlint_encode_n(data, value, len);
    return 1;
}
#endif

/*
 * Internal helper function used by WPACKET_close(), WPACKET_finish() and
 * WPACKET_fill_lengths() to close a sub-packet and write out its length if
 * necessary. If |doclose| is 0 then it goes through the motions of closing
 * (i.e. it fills in all the lengths), but doesn't actually close anything.
 */
static int wpacket_intern_close(WPACKET *pkt, WPACKET_SUB *sub, int doclose)
{
    size_t packlen = pkt->written - sub->pwritten;

    if (packlen == 0
            && (sub->flags & WPACKET_FLAGS_NON_ZERO_LENGTH) != 0)
        return 0;

    if (packlen == 0
            && sub->flags & WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH) {
        /* We can't handle this case. Return an error */
        if (!doclose)
            return 0;

        /* Deallocate any bytes allocated for the length of the WPACKET */
        if ((pkt->curr - sub->lenbytes) == sub->packet_len) {
            pkt->written -= sub->lenbytes;
            pkt->curr -= sub->lenbytes;
        }

        /* Don't write out the packet length */
        sub->packet_len = 0;
        sub->lenbytes = 0;
    }

    /* Write out the WPACKET length if needed */
    if (sub->lenbytes > 0) {
        unsigned char *buf = GETBUF(pkt);

        if (buf != NULL) {
#if !defined OPENSSL_NO_QUIC && !defined FIPS_MODULE
            if ((sub->flags & WPACKET_FLAGS_QUIC_VLINT) == 0) {
                if (!put_value(&buf[sub->packet_len], packlen, sub->lenbytes))
                    return 0;
            } else {
                if (!put_quic_value(&buf[sub->packet_len], packlen, sub->lenbytes))
                    return 0;
            }
#else
            if (!put_value(&buf[sub->packet_len], packlen, sub->lenbytes))
                return 0;
#endif
        }
    } else if (pkt->endfirst && sub->parent != NULL
               && (packlen != 0
                   || (sub->flags
                       & WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH) == 0)) {
        size_t tmplen = packlen;
        size_t numlenbytes = 1;

        while ((tmplen = tmplen >> 8) > 0)
            numlenbytes++;
        if (!WPACKET_put_bytes__(pkt, packlen, numlenbytes))
            return 0;
        if (packlen > 0x7f) {
            numlenbytes |= 0x80;
            if (!WPACKET_put_bytes_u8(pkt, numlenbytes))
                return 0;
        }
    }

    if (doclose) {
        pkt->subs = sub->parent;
        OPENSSL_free(sub);
    }

    return 1;
}

int WPACKET_fill_lengths(WPACKET *pkt)
{
    WPACKET_SUB *sub;

    if (!ossl_assert(pkt->subs != NULL))
        return 0;

    for (sub = pkt->subs; sub != NULL; sub = sub->parent) {
        if (!wpacket_intern_close(pkt, sub, 0))
            return 0;
    }

    return 1;
}

int WPACKET_close(WPACKET *pkt)
{
    /*
     * Internal API, so should not fail - but we do negative testing of this
     * so no assert (otherwise the tests fail)
     */
    if (pkt->subs == NULL || pkt->subs->parent == NULL)
        return 0;

    return wpacket_intern_close(pkt, pkt->subs, 1);
}

int WPACKET_finish(WPACKET *pkt)
{
    int ret;

    /*
     * Internal API, so should not fail - but we do negative testing of this
     * so no assert (otherwise the tests fail)
     */
    if (pkt->subs == NULL || pkt->subs->parent != NULL)
        return 0;

    ret = wpacket_intern_close(pkt, pkt->subs, 1);
    if (ret) {
        OPENSSL_free(pkt->subs);
        pkt->subs = NULL;
    }

    return ret;
}

int WPACKET_start_sub_packet_len__(WPACKET *pkt, size_t lenbytes)
{
    WPACKET_SUB *sub;
    unsigned char *lenchars;

    /* Internal API, so should not fail */
    if (!ossl_assert(pkt->subs != NULL))
        return 0;

    /* We don't support lenbytes greater than 0 when doing endfirst writing */
    if (lenbytes > 0 && pkt->endfirst)
        return 0;

    if ((sub = OPENSSL_zalloc(sizeof(*sub))) == NULL)
        return 0;

    sub->parent = pkt->subs;
    pkt->subs = sub;
    sub->pwritten = pkt->written + lenbytes;
    sub->lenbytes = lenbytes;

    if (lenbytes == 0) {
        sub->packet_len = 0;
        return 1;
    }

    sub->packet_len = pkt->written;

    if (!WPACKET_allocate_bytes(pkt, lenbytes, &lenchars))
        return 0;

    return 1;
}

int WPACKET_start_sub_packet(WPACKET *pkt)
{
    return WPACKET_start_sub_packet_len__(pkt, 0);
}

int WPACKET_put_bytes__(WPACKET *pkt, uint64_t val, size_t size)
{
    unsigned char *data;

    /* Internal API, so should not fail */
    if (!ossl_assert(size <= sizeof(uint64_t))
            || !WPACKET_allocate_bytes(pkt, size, &data)
            || !put_value(data, val, size))
        return 0;

    return 1;
}

int WPACKET_set_max_size(WPACKET *pkt, size_t maxsize)
{
    WPACKET_SUB *sub;
    size_t lenbytes;

    /* Internal API, so should not fail */
    if (!ossl_assert(pkt->subs != NULL))
        return 0;

    /* Find the WPACKET_SUB for the top level */
    for (sub = pkt->subs; sub->parent != NULL; sub = sub->parent)
        continue;

    lenbytes = sub->lenbytes;
    if (lenbytes == 0)
        lenbytes = sizeof(pkt->maxsize);

    if (maxmaxsize(lenbytes) < maxsize || maxsize < pkt->written)
        return 0;

    pkt->maxsize = maxsize;

    return 1;
}

int WPACKET_memset(WPACKET *pkt, int ch, size_t len)
{
    unsigned char *dest;

    if (len == 0)
        return 1;

    if (!WPACKET_allocate_bytes(pkt, len, &dest))
        return 0;

    if (dest != NULL)
        memset(dest, ch, len);

    return 1;
}

int WPACKET_memcpy(WPACKET *pkt, const void *src, size_t len)
{
    unsigned char *dest;

    if (len == 0)
        return 1;

    if (!WPACKET_allocate_bytes(pkt, len, &dest))
        return 0;

    if (dest != NULL)
        memcpy(dest, src, len);

    return 1;
}

int WPACKET_sub_memcpy__(WPACKET *pkt, const void *src, size_t len,
                         size_t lenbytes)
{
    if (!WPACKET_start_sub_packet_len__(pkt, lenbytes)
            || !WPACKET_memcpy(pkt, src, len)
            || !WPACKET_close(pkt))
        return 0;

    return 1;
}

int WPACKET_get_total_written(WPACKET *pkt, size_t *written)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(written != NULL))
        return 0;

    *written = pkt->written;

    return 1;
}

int WPACKET_get_length(WPACKET *pkt, size_t *len)
{
    /* Internal API, so should not fail */
    if (!ossl_assert(pkt->subs != NULL && len != NULL))
        return 0;

    *len = pkt->written - pkt->subs->pwritten;

    return 1;
}

unsigned char *WPACKET_get_curr(WPACKET *pkt)
{
    unsigned char *buf = GETBUF(pkt);

    if (buf == NULL)
        return NULL;

    if (pkt->endfirst)
        return buf + pkt->maxsize - pkt->curr;

    return buf + pkt->curr;
}

int WPACKET_is_null_buf(WPACKET *pkt)
{
    return pkt->buf == NULL && pkt->staticbuf == NULL;
}

void WPACKET_cleanup(WPACKET *pkt)
{
    WPACKET_SUB *sub, *parent;

    for (sub = pkt->subs; sub != NULL; sub = parent) {
        parent = sub->parent;
        OPENSSL_free(sub);
    }
    pkt->subs = NULL;
}

#if !defined OPENSSL_NO_QUIC && !defined FIPS_MODULE

int WPACKET_start_quic_sub_packet_bound(WPACKET *pkt, size_t max_len)
{
    size_t enclen = ossl_quic_vlint_encode_len(max_len);

    if (enclen == 0)
        return 0;

    if (WPACKET_start_sub_packet_len__(pkt, enclen) == 0)
        return 0;

    pkt->subs->flags |= WPACKET_FLAGS_QUIC_VLINT;
    return 1;
}

int WPACKET_start_quic_sub_packet(WPACKET *pkt)
{
    /*
     * Assume no (sub)packet will exceed 4GiB, thus the 8-byte encoding need not
     * be used.
     */
    return WPACKET_start_quic_sub_packet_bound(pkt, OSSL_QUIC_VLINT_4B_MIN);
}

int WPACKET_quic_sub_allocate_bytes(WPACKET *pkt, size_t len, unsigned char **allocbytes)
{
    if (!WPACKET_start_quic_sub_packet_bound(pkt, len)
            || !WPACKET_allocate_bytes(pkt, len, allocbytes)
            || !WPACKET_close(pkt))
        return 0;

    return 1;
}

/*
 * Write a QUIC variable-length integer to the packet.
 */
int WPACKET_quic_write_vlint(WPACKET *pkt, uint64_t v)
{
    unsigned char *b = NULL;
    size_t enclen = ossl_quic_vlint_encode_len(v);

    if (enclen == 0)
        return 0;

    if (WPACKET_allocate_bytes(pkt, enclen, &b) == 0)
        return 0;

    ossl_quic_vlint_encode(b, v);
    return 1;
}

#endif
