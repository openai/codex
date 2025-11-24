/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "internal/packet.h"
#include "internal/quic_stream.h"
#include "testutil.h"

static int compare_iov(const unsigned char *ref, size_t ref_len,
                       const OSSL_QTX_IOVEC *iov, size_t iov_len)
{
    size_t i, total_len = 0;
    const unsigned char *cur = ref;

    for (i = 0; i < iov_len; ++i)
        total_len += iov[i].buf_len;

    if (ref_len != total_len)
        return 0;

    for (i = 0; i < iov_len; ++i) {
        if (memcmp(cur, iov[i].buf, iov[i].buf_len))
            return 0;

        cur += iov[i].buf_len;
    }

    return 1;
}

static const unsigned char data_1[] = {
    0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5a, 0x5b, 0x5c, 0x5d, 0x5e, 0x5f
};

static int test_sstream_simple(void)
{
    int testresult = 0;
    QUIC_SSTREAM *sstream = NULL;
    OSSL_QUIC_FRAME_STREAM hdr;
    OSSL_QTX_IOVEC iov[2];
    size_t num_iov = 0, wr = 0, i, init_size = 8192;

    if (!TEST_ptr(sstream = ossl_quic_sstream_new(init_size)))
        goto err;

    /* A stream with nothing yet appended is totally acked */
    if (!TEST_true(ossl_quic_sstream_is_totally_acked(sstream)))
        goto err;

    /* Should not have any data yet */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_false(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                       &num_iov)))
        goto err;

    /* Append data */
    if (!TEST_true(ossl_quic_sstream_append(sstream, data_1, sizeof(data_1),
                                            &wr))
        || !TEST_size_t_eq(wr, sizeof(data_1)))
        goto err;

    /* No longer totally acked */
    if (!TEST_false(ossl_quic_sstream_is_totally_acked(sstream)))
        goto err;

    /* Read data */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                      &num_iov))
        || !TEST_size_t_gt(num_iov, 0)
        || !TEST_uint64_t_eq(hdr.offset, 0)
        || !TEST_uint64_t_eq(hdr.len, sizeof(data_1))
        || !TEST_false(hdr.is_fin))
        goto err;

    if (!TEST_true(compare_iov(data_1, sizeof(data_1), iov, num_iov)))
        goto err;

    /* Mark data as half transmitted */
    if (!TEST_true(ossl_quic_sstream_mark_transmitted(sstream, 0, 7)))
        goto err;

    /* Read data */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                      &num_iov))
        || !TEST_size_t_gt(num_iov, 0)
        || !TEST_uint64_t_eq(hdr.offset, 8)
        || !TEST_uint64_t_eq(hdr.len, sizeof(data_1) - 8)
        || !TEST_false(hdr.is_fin))
        goto err;

    if (!TEST_true(compare_iov(data_1 + 8, sizeof(data_1) - 8, iov, num_iov)))
        goto err;

    if (!TEST_true(ossl_quic_sstream_mark_transmitted(sstream, 8, 15)))
        goto err;

    /* Read more data; should not be any more */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_false(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                       &num_iov)))
        goto err;

    /* Now we have lost bytes 4-6 */
    if (!TEST_true(ossl_quic_sstream_mark_lost(sstream, 4, 6)))
        goto err;

    /* Should be able to read them */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                      &num_iov))
        || !TEST_size_t_gt(num_iov, 0)
        || !TEST_uint64_t_eq(hdr.offset, 4)
        || !TEST_uint64_t_eq(hdr.len, 3)
        || !TEST_false(hdr.is_fin))
        goto err;

    if (!TEST_true(compare_iov(data_1 + 4, 3, iov, num_iov)))
        goto err;

    /* Retransmit */
    if (!TEST_true(ossl_quic_sstream_mark_transmitted(sstream, 4, 6)))
        goto err;

    /* Read more data; should not be any more */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_false(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                       &num_iov)))
        goto err;

    if (!TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(sstream), 16))
        goto err;

    /* Data has been acknowledged, space should be not be freed yet */
    if (!TEST_true(ossl_quic_sstream_mark_acked(sstream, 1, 7))
        || !TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(sstream), 16))
        goto err;

    /* Now data should be freed */
    if (!TEST_true(ossl_quic_sstream_mark_acked(sstream, 0, 0))
        || !TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(sstream), 8))
        goto err;

    if (!TEST_true(ossl_quic_sstream_mark_acked(sstream, 0, 15))
        || !TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(sstream), 0))
        goto err;

    /* Now FIN */
    ossl_quic_sstream_fin(sstream);

    /* Get FIN frame */
    for (i = 0; i < 2; ++i) {
        num_iov = OSSL_NELEM(iov);
        if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                          &num_iov))
            || !TEST_uint64_t_eq(hdr.offset, 16)
            || !TEST_uint64_t_eq(hdr.len, 0)
            || !TEST_true(hdr.is_fin)
            || !TEST_size_t_eq(num_iov, 0))
            goto err;
    }

    if (!TEST_true(ossl_quic_sstream_mark_transmitted_fin(sstream, 16)))
        goto err;

    /* Read more data; FIN should not be returned any more */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_false(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                       &num_iov)))
        goto err;

    /* Lose FIN frame */
    if (!TEST_true(ossl_quic_sstream_mark_lost_fin(sstream)))
        goto err;

    /* Get FIN frame */
    for (i = 0; i < 2; ++i) {
        num_iov = OSSL_NELEM(iov);
        if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                          &num_iov))
            || !TEST_uint64_t_eq(hdr.offset, 16)
            || !TEST_uint64_t_eq(hdr.len, 0)
            || !TEST_true(hdr.is_fin)
            || !TEST_size_t_eq(num_iov, 0))
            goto err;
    }

    if (!TEST_true(ossl_quic_sstream_mark_transmitted_fin(sstream, 16)))
        goto err;

    /* Read more data; FIN should not be returned any more */
    num_iov = OSSL_NELEM(iov);
    if (!TEST_false(ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov,
                                                       &num_iov)))
        goto err;

    /* Acknowledge fin. */
    if (!TEST_true(ossl_quic_sstream_mark_acked_fin(sstream)))
        goto err;

    if (!TEST_true(ossl_quic_sstream_is_totally_acked(sstream)))
        goto err;

    testresult = 1;
 err:
    ossl_quic_sstream_free(sstream);
    return testresult;
}

static int test_sstream_bulk(int idx)
{
    int testresult = 0;
    QUIC_SSTREAM *sstream = NULL;
    OSSL_QUIC_FRAME_STREAM hdr;
    OSSL_QTX_IOVEC iov[2];
    size_t i, j, num_iov = 0, init_size = 8192, l;
    size_t consumed = 0, total_written = 0, rd, cur_rd, expected = 0, start_at;
    unsigned char *src_buf = NULL, *dst_buf = NULL;
    unsigned char *ref_src_buf = NULL, *ref_dst_buf = NULL;
    unsigned char *ref_dst_cur, *ref_src_cur, *dst_cur;

    if (!TEST_ptr(sstream = ossl_quic_sstream_new(init_size)))
        goto err;

    if (!TEST_size_t_eq(ossl_quic_sstream_get_buffer_size(sstream), init_size))
        goto err;

    if (!TEST_ptr(src_buf = OPENSSL_zalloc(init_size)))
        goto err;

    if (!TEST_ptr(dst_buf = OPENSSL_malloc(init_size)))
        goto err;

    if (!TEST_ptr(ref_src_buf = OPENSSL_malloc(init_size)))
        goto err;

    if (!TEST_ptr(ref_dst_buf = OPENSSL_malloc(init_size)))
        goto err;

    /*
     * Append a preliminary buffer to allow later code to exercise wraparound.
     */
    if (!TEST_true(ossl_quic_sstream_append(sstream, src_buf, init_size / 2,
                                            &consumed))
        || !TEST_size_t_eq(consumed, init_size / 2)
        || !TEST_true(ossl_quic_sstream_mark_transmitted(sstream, 0,
                                                         init_size / 2 - 1))
        || !TEST_true(ossl_quic_sstream_mark_acked(sstream, 0,
                                                   init_size / 2 - 1)))
        goto err;

    start_at = init_size / 2;

    /* Generate a random buffer. */
    for (i = 0; i < init_size; ++i)
        src_buf[i] = (unsigned char)(test_random() & 0xFF);

    /* Append bytes into the buffer in chunks of random length. */
    ref_src_cur = ref_src_buf;
    do {
        l = (test_random() % init_size) + 1;
        if (!TEST_true(ossl_quic_sstream_append(sstream, src_buf, l, &consumed)))
            goto err;

        memcpy(ref_src_cur, src_buf, consumed);
        ref_src_cur     += consumed;
        total_written   += consumed;
    } while (consumed > 0);

    if (!TEST_size_t_eq(ossl_quic_sstream_get_buffer_used(sstream), init_size)
        || !TEST_size_t_eq(ossl_quic_sstream_get_buffer_avail(sstream), 0))
        goto err;

    /*
     * Randomly select bytes out of the buffer by marking them as transmitted.
     * Record the remaining bytes, which should be the sequence of bytes
     * returned.
     */
    ref_src_cur = ref_src_buf;
    ref_dst_cur = ref_dst_buf;
    for (i = 0; i < total_written; ++i) {
        if ((test_random() & 1) != 0) {
            *ref_dst_cur++ = *ref_src_cur;
            ++expected;
        } else if (!TEST_true(ossl_quic_sstream_mark_transmitted(sstream,
                                                                 start_at + i,
                                                                 start_at + i)))
            goto err;

        ++ref_src_cur;
    }

    /* Exercise resize. */
    if (!TEST_true(ossl_quic_sstream_set_buffer_size(sstream, init_size * 2))
        || !TEST_true(ossl_quic_sstream_set_buffer_size(sstream, init_size)))
        goto err;

    /* Readout and verification. */
    dst_cur = dst_buf;
    for (i = 0, rd = 0; rd < expected; ++i) {
        num_iov = OSSL_NELEM(iov);
        if (!TEST_true(ossl_quic_sstream_get_stream_frame(sstream, i, &hdr, iov,
                                                          &num_iov)))
            goto err;

        cur_rd = 0;
        for (j = 0; j < num_iov; ++j) {
            if (!TEST_size_t_le(iov[j].buf_len + rd, expected))
                goto err;

            memcpy(dst_cur, iov[j].buf, iov[j].buf_len);
            dst_cur += iov[j].buf_len;
            cur_rd  += iov[j].buf_len;
        }

        if (!TEST_uint64_t_eq(cur_rd, hdr.len))
            goto err;

        rd += cur_rd;
    }

    if (!TEST_mem_eq(dst_buf, rd, ref_dst_buf, expected))
        goto err;

    testresult = 1;
 err:
    OPENSSL_free(src_buf);
    OPENSSL_free(dst_buf);
    OPENSSL_free(ref_src_buf);
    OPENSSL_free(ref_dst_buf);
    ossl_quic_sstream_free(sstream);
    return testresult;
}

static int test_single_copy_read(QUIC_RSTREAM *qrs,
                                 unsigned char *buf, size_t size,
                                 size_t *readbytes, int *fin)
{
    const unsigned char *record;
    size_t rec_len;

    *readbytes = 0;

    for (;;) {
        if (!ossl_quic_rstream_get_record(qrs, &record, &rec_len, fin))
            return 0;
        if (rec_len == 0)
            break;
        if (rec_len > size) {
            rec_len = size;
            *fin = 0;
        }
        memcpy(buf, record, rec_len);
        size -= rec_len;
        *readbytes += rec_len;
        buf += rec_len;

        if (!ossl_quic_rstream_release_record(qrs, rec_len))
            return 0;
        if (*fin || size == 0)
            break;
    }

    return 1;
}

static const unsigned char simple_data[] =
    "Hello world! And thank you for all the fish!";

static int test_rstream_simple(int idx)
{
    QUIC_RSTREAM *rstream = NULL;
    int ret = 0;
    unsigned char buf[sizeof(simple_data)];
    size_t readbytes = 0, avail = 0;
    int fin = 0;
    int use_rbuf = idx > 1;
    int use_sc = idx % 2;
    int (* read_fn)(QUIC_RSTREAM *, unsigned char *, size_t, size_t *,
                    int *) = use_sc ? test_single_copy_read
                                    : ossl_quic_rstream_read;

    if (!TEST_ptr(rstream = ossl_quic_rstream_new(NULL, NULL, 0)))
        goto err;

    if (!TEST_true(ossl_quic_rstream_queue_data(rstream, NULL, 5,
                                                simple_data + 5, 10, 0))
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   sizeof(simple_data) - 1,
                                                   simple_data + sizeof(simple_data) - 1,
                                                   1, 1))
        || !TEST_true(ossl_quic_rstream_peek(rstream, buf, sizeof(buf),
                                             &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 0)
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   sizeof(simple_data) - 10,
                                                   simple_data + sizeof(simple_data) - 10,
                                                   10, 1))
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL, 0,
                                                   simple_data, 1, 0))
        || !TEST_true(ossl_quic_rstream_peek(rstream, buf, sizeof(buf),
                                             &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 1)
        || !TEST_mem_eq(buf, 1, simple_data, 1)
        || (use_rbuf && !TEST_false(ossl_quic_rstream_move_to_rbuf(rstream)))
        || (use_rbuf
            && !TEST_true(ossl_quic_rstream_resize_rbuf(rstream,
                                                        sizeof(simple_data))))
        || (use_rbuf && !TEST_true(ossl_quic_rstream_move_to_rbuf(rstream)))
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   0, simple_data,
                                                   10, 0))
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   sizeof(simple_data),
                                                   NULL,
                                                   0, 1))
        || !TEST_true(ossl_quic_rstream_peek(rstream, buf, sizeof(buf),
                                             &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 15)
        || !TEST_mem_eq(buf, 15, simple_data, 15)
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   15,
                                                   simple_data + 15,
                                                   sizeof(simple_data) - 15, 1))
        || !TEST_true(ossl_quic_rstream_available(rstream, &avail, &fin))
        || !TEST_true(fin)
        || !TEST_size_t_eq(avail, sizeof(simple_data))
        || !TEST_true(read_fn(rstream, buf, 2, &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 2)
        || !TEST_mem_eq(buf, 2, simple_data, 2)
        || !TEST_true(read_fn(rstream, buf + 2, 12, &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 12)
        || !TEST_mem_eq(buf + 2, 12, simple_data + 2, 12)
        || !TEST_true(ossl_quic_rstream_queue_data(rstream, NULL,
                                                   sizeof(simple_data),
                                                   NULL,
                                                   0, 1))
        || (use_rbuf
            && !TEST_true(ossl_quic_rstream_resize_rbuf(rstream,
                                                        2 * sizeof(simple_data))))
        || (use_rbuf && !TEST_true(ossl_quic_rstream_move_to_rbuf(rstream)))
        || !TEST_true(read_fn(rstream, buf + 14, 5, &readbytes, &fin))
        || !TEST_false(fin)
        || !TEST_size_t_eq(readbytes, 5)
        || !TEST_mem_eq(buf, 14 + 5, simple_data, 14 + 5)
        || !TEST_true(read_fn(rstream, buf + 14 + 5, sizeof(buf) - 14 - 5,
                              &readbytes, &fin))
        || !TEST_true(fin)
        || !TEST_size_t_eq(readbytes, sizeof(buf) - 14 - 5)
        || !TEST_mem_eq(buf, sizeof(buf), simple_data, sizeof(simple_data))
        || (use_rbuf && !TEST_true(ossl_quic_rstream_move_to_rbuf(rstream)))
        || !TEST_true(read_fn(rstream, buf, sizeof(buf), &readbytes, &fin))
        || !TEST_true(fin)
        || !TEST_size_t_eq(readbytes, 0))
        goto err;

    ret = 1;

 err:
    ossl_quic_rstream_free(rstream);
    return ret;
}

static int test_rstream_random(int idx)
{
    unsigned char *bulk_data = NULL;
    unsigned char *read_buf = NULL;
    QUIC_RSTREAM *rstream = NULL;
    size_t i, read_off, queued_min, queued_max;
    const size_t data_size = 10000;
    int r, s, fin = 0, fin_set = 0;
    int ret = 0;
    size_t readbytes = 0;

    if (!TEST_ptr(bulk_data = OPENSSL_malloc(data_size))
        || !TEST_ptr(read_buf = OPENSSL_malloc(data_size))
        || !TEST_ptr(rstream = ossl_quic_rstream_new(NULL, NULL, 0)))
        goto err;

    if (idx % 3 == 0)
        ossl_quic_rstream_set_cleanse(rstream, 1);

    for (i = 0; i < data_size; ++i)
        bulk_data[i] = (unsigned char)(test_random() & 0xFF);

    read_off = queued_min = queued_max = 0;
    for (r = 0; r < 100; ++r) {
        for (s = 0; s < 10; ++s) {
            size_t off = (r * 10 + s) * 10, size = 10;

            if (test_random() % 10 == 0)
                /* drop packet */
                continue;

            if (off <= queued_min && off + size > queued_min)
                queued_min = off + size;

            if (!TEST_true(ossl_quic_rstream_queue_data(rstream, NULL, off,
                                                        bulk_data + off,
                                                        size, 0)))
                goto err;
            if (queued_max < off + size)
                queued_max = off + size;

            if (test_random() % 5 != 0)
               continue;

            /* random overlapping retransmit */
            off = read_off + test_random() % 50;
            if (off > 50)
                off -= 50;
            size = test_random() % 100 + 1;
            if (off + size > data_size)
                off = data_size - size;
            if (off <= queued_min && off + size > queued_min)
                queued_min = off + size;

            if (!TEST_true(ossl_quic_rstream_queue_data(rstream, NULL, off,
                                                        bulk_data + off,
                                                        size, 0)))
                goto err;
            if (queued_max < off + size)
                queued_max = off + size;
        }
        if (idx % 2 == 0) {
            if (!TEST_true(test_single_copy_read(rstream, read_buf, data_size,
                                                 &readbytes, &fin)))
                goto err;
        } else if (!TEST_true(ossl_quic_rstream_read(rstream, read_buf,
                                                     data_size,
                                                     &readbytes, &fin))) {
            goto err;
        }
        if (!TEST_size_t_ge(readbytes, queued_min - read_off)
            || !TEST_size_t_le(readbytes + read_off, data_size)
            || (idx % 3 != 0
                && !TEST_mem_eq(read_buf, readbytes, bulk_data + read_off,
                                readbytes)))
            goto err;
        read_off += readbytes;
        queued_min = read_off;
        if (test_random() % 50 == 0)
            if (!TEST_true(ossl_quic_rstream_resize_rbuf(rstream,
                                                         queued_max - read_off + 1))
                || !TEST_true(ossl_quic_rstream_move_to_rbuf(rstream)))
                goto err;
        if (!fin_set && queued_max >= data_size - test_random() % 200) {
            fin_set = 1;
            /* Queue empty fin frame */
            if (!TEST_true(ossl_quic_rstream_queue_data(rstream, NULL, data_size,
                                                        NULL, 0, 1)))
                goto err;
        }
    }

    TEST_info("Total read bytes: %zu Fin rcvd: %d", read_off, fin);

    if (idx % 3 == 0)
        for (i = 0; i < read_off; i++)
            if (!TEST_uchar_eq(bulk_data[i], 0))
                goto err;

    if (read_off == data_size && fin_set && !fin) {
        /* We might still receive the final empty frame */
        if (idx % 2 == 0) {
            if (!TEST_true(test_single_copy_read(rstream, read_buf, data_size,
                                                 &readbytes, &fin)))
                goto err;
        } else if (!TEST_true(ossl_quic_rstream_read(rstream, read_buf,
                                                     data_size,
                                                     &readbytes, &fin))) {
            goto err;
        }
        if (!TEST_size_t_eq(readbytes, 0) || !TEST_true(fin))
            goto err;
    }

    ret = 1;

 err:
    ossl_quic_rstream_free(rstream);
    OPENSSL_free(bulk_data);
    OPENSSL_free(read_buf);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_sstream_simple);
    ADD_ALL_TESTS(test_sstream_bulk, 100);
    ADD_ALL_TESTS(test_rstream_simple, 4);
    ADD_ALL_TESTS(test_rstream_random, 100);
    return 1;
}
