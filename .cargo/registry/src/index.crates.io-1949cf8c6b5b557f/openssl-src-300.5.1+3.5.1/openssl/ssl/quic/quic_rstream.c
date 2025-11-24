/*
* Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/
#include <openssl/err.h>
#include "internal/common.h"
#include "internal/time.h"
#include "internal/quic_stream.h"
#include "internal/quic_sf_list.h"
#include "internal/ring_buf.h"

struct quic_rstream_st {
    SFRAME_LIST fl;
    QUIC_RXFC *rxfc;
    OSSL_STATM *statm;
    UINT_RANGE head_range;
    struct ring_buf rbuf;
};

QUIC_RSTREAM *ossl_quic_rstream_new(QUIC_RXFC *rxfc,
                                    OSSL_STATM *statm, size_t rbuf_size)
{
    QUIC_RSTREAM *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ring_buf_init(&ret->rbuf);
    if (!ring_buf_resize(&ret->rbuf, rbuf_size, 0)) {
        OPENSSL_free(ret);
        return NULL;
    }

    ossl_sframe_list_init(&ret->fl);
    ret->rxfc = rxfc;
    ret->statm = statm;
    return ret;
}

void ossl_quic_rstream_free(QUIC_RSTREAM *qrs)
{
    int cleanse;

    if (qrs == NULL)
        return;

    cleanse = qrs->fl.cleanse;
    ossl_sframe_list_destroy(&qrs->fl);
    ring_buf_destroy(&qrs->rbuf, cleanse);
    OPENSSL_free(qrs);
}

int ossl_quic_rstream_queue_data(QUIC_RSTREAM *qrs, OSSL_QRX_PKT *pkt,
                                 uint64_t offset,
                                 const unsigned char *data, uint64_t data_len,
                                 int fin)
{
    UINT_RANGE range;

    if ((data == NULL && data_len != 0) || (data_len == 0 && fin == 0)) {
        /* empty frame allowed only at the end of the stream */
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    range.start = offset;
    range.end = offset + data_len;

    return ossl_sframe_list_insert(&qrs->fl, &range, pkt, data, fin);
}

static int read_internal(QUIC_RSTREAM *qrs, unsigned char *buf, size_t size,
                         size_t *readbytes, int *fin, int drop)
{
    void *iter = NULL;
    UINT_RANGE range;
    const unsigned char *data;
    uint64_t offset = 0;
    size_t readbytes_ = 0;
    int fin_ = 0, ret = 1;

    while (ossl_sframe_list_peek(&qrs->fl, &iter, &range, &data, &fin_)) {
        size_t l = (size_t)(range.end - range.start);

        if (l > size) {
            l = size;
            fin_ = 0;
        }
        offset = range.start + l;
        if (l == 0)
            break;

        if (data == NULL) {
            size_t max_len;

            data = ring_buf_get_ptr(&qrs->rbuf, range.start, &max_len);
            if (!ossl_assert(data != NULL))
                return 0;
            if (max_len < l) {
                memcpy(buf, data, max_len);
                size -= max_len;
                buf += max_len;
                readbytes_ += max_len;
                l -= max_len;
                data = ring_buf_get_ptr(&qrs->rbuf, range.start + max_len,
                                        &max_len);
                if (!ossl_assert(data != NULL) || !ossl_assert(max_len > l))
                    return 0;
            }
        }

        memcpy(buf, data, l);
        size -= l;
        buf += l;
        readbytes_ += l;
        if (size == 0)
            break;
    }

    if (drop && offset != 0) {
        ret = ossl_sframe_list_drop_frames(&qrs->fl, offset);
        ring_buf_cpop_range(&qrs->rbuf, 0, offset - 1, qrs->fl.cleanse);
    }

    if (ret) {
        *readbytes = readbytes_;
        *fin = fin_;
    }

    return ret;
}

static OSSL_TIME get_rtt(QUIC_RSTREAM *qrs)
{
    OSSL_TIME rtt;

    if (qrs->statm != NULL) {
        OSSL_RTT_INFO rtt_info;

        ossl_statm_get_rtt_info(qrs->statm, &rtt_info);
        rtt = rtt_info.smoothed_rtt;
    } else {
        rtt = ossl_time_zero();
    }
    return rtt;
}

int ossl_quic_rstream_read(QUIC_RSTREAM *qrs, unsigned char *buf, size_t size,
                           size_t *readbytes, int *fin)
{
    OSSL_TIME rtt = get_rtt(qrs);

    if (!read_internal(qrs, buf, size, readbytes, fin, 1))
        return 0;

    if (qrs->rxfc != NULL
        && !ossl_quic_rxfc_on_retire(qrs->rxfc, *readbytes, rtt))
        return 0;

    return 1;
}

int ossl_quic_rstream_peek(QUIC_RSTREAM *qrs, unsigned char *buf, size_t size,
                           size_t *readbytes, int *fin)
{
    return read_internal(qrs, buf, size, readbytes, fin, 0);
}

int ossl_quic_rstream_available(QUIC_RSTREAM *qrs, size_t *avail, int *fin)
{
    void *iter = NULL;
    UINT_RANGE range;
    const unsigned char *data;
    uint64_t avail_ = 0;

    while (ossl_sframe_list_peek(&qrs->fl, &iter, &range, &data, fin))
        avail_ += range.end - range.start;

#if SIZE_MAX < UINT64_MAX
    *avail = avail_ > SIZE_MAX ? SIZE_MAX : (size_t)avail_;
#else
    *avail = (size_t)avail_;
#endif
    return 1;
}

int ossl_quic_rstream_get_record(QUIC_RSTREAM *qrs,
                                 const unsigned char **record, size_t *rec_len,
                                 int *fin)
{
    const unsigned char *record_ = NULL;
    size_t rec_len_, max_len;

    if (!ossl_sframe_list_lock_head(&qrs->fl, &qrs->head_range, &record_, fin)) {
        /* No head frame to lock and return */
        *record = NULL;
        *rec_len = 0;
        return 1;
    }

    /* if final empty frame, we drop it immediately */
    if (qrs->head_range.end == qrs->head_range.start) {
        if (!ossl_assert(*fin))
            return 0;
        if (!ossl_sframe_list_drop_frames(&qrs->fl, qrs->head_range.end))
            return 0;
    }

    rec_len_ = (size_t)(qrs->head_range.end - qrs->head_range.start);

    if (record_ == NULL && rec_len_ != 0) {
        record_ = ring_buf_get_ptr(&qrs->rbuf, qrs->head_range.start,
                                   &max_len);
        if (!ossl_assert(record_ != NULL))
            return 0;
        if (max_len < rec_len_) {
            rec_len_ = max_len;
            qrs->head_range.end = qrs->head_range.start + max_len;
        }
    }

    *rec_len = rec_len_;
    *record = record_;
    return 1;
}


int ossl_quic_rstream_release_record(QUIC_RSTREAM *qrs, size_t read_len)
{
    uint64_t offset;

    if (!ossl_sframe_list_is_head_locked(&qrs->fl))
        return 0;

    if (read_len > qrs->head_range.end - qrs->head_range.start) {
        if (read_len != SIZE_MAX)
            return 0;
        offset = qrs->head_range.end;
    } else {
        offset = qrs->head_range.start + read_len;
    }

    if (!ossl_sframe_list_drop_frames(&qrs->fl, offset))
        return 0;

    if (offset > 0)
        ring_buf_cpop_range(&qrs->rbuf, 0, offset - 1, qrs->fl.cleanse);

    if (qrs->rxfc != NULL) {
        OSSL_TIME rtt = get_rtt(qrs);

        if (!ossl_quic_rxfc_on_retire(qrs->rxfc, offset, rtt))
            return 0;
    }

    return 1;
}

static int write_at_ring_buf_cb(uint64_t logical_offset,
                                const unsigned char *buf,
                                size_t buf_len,
                                void *cb_arg)
{
    struct ring_buf *rbuf = cb_arg;

    return ring_buf_write_at(rbuf, logical_offset, buf, buf_len);
}

int ossl_quic_rstream_move_to_rbuf(QUIC_RSTREAM *qrs)
{
    if (ring_buf_avail(&qrs->rbuf) == 0)
        return 0;
    return ossl_sframe_list_move_data(&qrs->fl,
                                      write_at_ring_buf_cb, &qrs->rbuf);
}

int ossl_quic_rstream_resize_rbuf(QUIC_RSTREAM *qrs, size_t rbuf_size)
{
    if (ossl_sframe_list_is_head_locked(&qrs->fl))
        return 0;

    if (!ring_buf_resize(&qrs->rbuf, rbuf_size, qrs->fl.cleanse))
        return 0;

    return 1;
}

void ossl_quic_rstream_set_cleanse(QUIC_RSTREAM *qrs, int cleanse)
{
    qrs->fl.cleanse = cleanse;
}
