/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "quictestlib.h"
#include "../testutil.h"

static long pkt_split_dgram_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret;
    BIO *next = BIO_next(bio);

    if (next == NULL)
        return 0;

    switch (cmd) {
    case BIO_CTRL_DUP:
        ret = 0L;
        break;
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static int pkt_split_dgram_sendmmsg(BIO *bio, BIO_MSG *msg, size_t stride,
                                    size_t num_msg, uint64_t flags,
                                    size_t *msgs_processed)
{
    BIO *next = BIO_next(bio);

    if (next == NULL)
        return 0;

    /*
     * We only introduce noise when receiving messages. We just pass this on
     * to the underlying BIO.
     */
    return BIO_sendmmsg(next, msg, stride, num_msg, flags, msgs_processed);
}

static int pkt_split_dgram_recvmmsg(BIO *bio, BIO_MSG *msg, size_t stride,
                                    size_t num_msg, uint64_t flags,
                                    size_t *msgs_processed)
{
    BIO *next = BIO_next(bio);
    size_t i, j, data_len = 0, msg_cnt = 0;
    BIO_MSG *thismsg;
    QTEST_DATA *bdata = BIO_get_data(bio);

    if (!TEST_ptr(next) || !TEST_ptr(bdata))
        return 0;

    /*
     * For simplicity we assume that all elements in the msg array have the
     * same data_len. They are not required to by the API, but it would be quite
     * strange for that not to be the case - and our code that calls
     * BIO_recvmmsg does do this (which is all that is important for this test
     * code). We test the invariant here.
     */
    for (i = 0; i < num_msg; i++) {
        if (i == 0)
            data_len = msg[i].data_len;
        else if (!TEST_size_t_eq(msg[i].data_len, data_len))
            return 0;
    }

    if (!BIO_recvmmsg(next, msg, stride, num_msg, flags, msgs_processed))
        return 0;

    msg_cnt = *msgs_processed;
    if (msg_cnt == num_msg)
        return 1; /* We've used all our slots and can't split any more */
    assert(msg_cnt < num_msg);

    for (i = 0, thismsg = msg; i < msg_cnt; i++, thismsg++) {
        QUIC_PKT_HDR hdr;
        PACKET pkt;
        size_t remain;

        if (!PACKET_buf_init(&pkt, thismsg->data, thismsg->data_len))
            return 0;

        /* Decode the packet header */
        if (ossl_quic_wire_decode_pkt_hdr(&pkt, bdata->short_conn_id_len,
                                          0, 0, &hdr, NULL, NULL) != 1)
            return 0;
        remain = PACKET_remaining(&pkt);
        if (remain > 0) {
            for (j = msg_cnt; j > i; j--) {
                if (!bio_msg_copy(&msg[j], &msg[j - 1]))
                    return 0;
            }
            thismsg->data_len -= remain;
            msg[i + 1].data_len = remain;
            memmove(msg[i + 1].data,
                    (unsigned char *)msg[i + 1].data + thismsg->data_len,
                    remain);
            msg_cnt++;
        }
    }

    *msgs_processed = msg_cnt;
    return 1;
}

/* Choose a sufficiently large type likely to be unused for this custom BIO */
#define BIO_TYPE_PKT_SPLIT_DGRAM_FILTER  (0x81 | BIO_TYPE_FILTER)

static BIO_METHOD *method_pkt_split_dgram = NULL;

/* Note: Not thread safe! */
const BIO_METHOD *bio_f_pkt_split_dgram_filter(void)
{
    if (method_pkt_split_dgram == NULL) {
        method_pkt_split_dgram = BIO_meth_new(BIO_TYPE_PKT_SPLIT_DGRAM_FILTER,
                                              "Packet splitting datagram filter");
        if (method_pkt_split_dgram == NULL
            || !BIO_meth_set_ctrl(method_pkt_split_dgram, pkt_split_dgram_ctrl)
            || !BIO_meth_set_sendmmsg(method_pkt_split_dgram,
                                      pkt_split_dgram_sendmmsg)
            || !BIO_meth_set_recvmmsg(method_pkt_split_dgram,
                                      pkt_split_dgram_recvmmsg))
            return NULL;
    }
    return method_pkt_split_dgram;
}

void bio_f_pkt_split_dgram_filter_free(void)
{
    BIO_meth_free(method_pkt_split_dgram);
}
