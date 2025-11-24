/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet.h"
#include "internal/quic_wire.h"
#include "internal/quic_wire_pkt.h"
#include "testutil.h"

struct encode_test_case {
    int (*serializer)(WPACKET *pkt);
    const unsigned char *expect_buf;
    size_t expect_buf_len;
    /*
     * fail: -1 if not truncated (function should test for success), else number
     * of bytes to which the input has been truncated (function should test that
     * decoding fails)
     */
    int (*deserializer)(PACKET *pkt, ossl_ssize_t fail);
};

/* 1. PADDING */
static int encode_case_1_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_padding(pkt, 3), 1))
        return 0;

    return 1;
}

static int encode_case_1_dec(PACKET *pkt, ossl_ssize_t fail)
{
    if (fail >= 0)
        /* No failure modes for padding */
        return 1;

    if (!TEST_int_eq(ossl_quic_wire_decode_padding(pkt), 3))
        return 0;

    return 1;
}

static const unsigned char encode_case_1_expect[] = {
    0, 0, 0
};

/* 2. PING */
static int encode_case_2_enc(WPACKET *pkt)
{

    if (!TEST_int_eq(ossl_quic_wire_encode_frame_ping(pkt), 1))
        return 0;

    return 1;
}

static int encode_case_2_dec(PACKET *pkt, ossl_ssize_t fail)
{

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_ping(pkt), fail < 0))
        return 0;

    return 1;
}

static const unsigned char encode_case_2_expect[] = {
    0x01
};

/* 3. ACK */
static const OSSL_QUIC_ACK_RANGE encode_case_3_ranges[] = {
    { 20, 30 },
    {  0, 10 }
};

static const OSSL_QUIC_FRAME_ACK encode_case_3_f = {
    (OSSL_QUIC_ACK_RANGE *)encode_case_3_ranges,
    OSSL_NELEM(encode_case_3_ranges),
    { OSSL_TIME_MS },
    60, 70, 80, 1
};

static int encode_case_3_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_ack(pkt, 3, &encode_case_3_f), 1))
        return 0;

    return 1;
}

static int encode_case_3_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_ACK_RANGE ranges[4] = {0};
    OSSL_QUIC_FRAME_ACK f = {0};
    uint64_t total_ranges = 0, peek_total_ranges = 0;
    int ret;

    f.ack_ranges        = ranges;
    f.num_ack_ranges    = OSSL_NELEM(ranges);

    ret = ossl_quic_wire_peek_frame_ack_num_ranges(pkt, &peek_total_ranges);
    if (fail < 0 && !TEST_int_eq(ret, 1))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_ack(pkt, 3, &f, &total_ranges), fail < 0))
        return 0;

    if (ret == 1 && !TEST_uint64_t_eq(peek_total_ranges, 2))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(total_ranges, peek_total_ranges))
        return 0;

    if (!TEST_uint64_t_le(f.num_ack_ranges * sizeof(OSSL_QUIC_ACK_RANGE),
                          SIZE_MAX)
        || !TEST_uint64_t_le(encode_case_3_f.num_ack_ranges
                             * sizeof(OSSL_QUIC_ACK_RANGE),
                             SIZE_MAX))
        return 0;

    if (!TEST_mem_eq(f.ack_ranges,
                     (size_t)f.num_ack_ranges * sizeof(OSSL_QUIC_ACK_RANGE),
                     encode_case_3_f.ack_ranges,
                     (size_t)encode_case_3_f.num_ack_ranges * sizeof(OSSL_QUIC_ACK_RANGE)))
        return 0;

    if (!TEST_uint64_t_eq(ossl_time2ticks(f.delay_time),
                          ossl_time2ticks(encode_case_3_f.delay_time)))
        return 0;

    if (!TEST_true(f.ecn_present))
        return 0;

    if (!TEST_uint64_t_eq(f.ect0, encode_case_3_f.ect0))
        return 0;

    if (!TEST_uint64_t_eq(f.ect1, encode_case_3_f.ect1))
        return 0;

    if (!TEST_uint64_t_eq(f.ecnce, encode_case_3_f.ecnce))
        return 0;

    return 1;
}

static const unsigned char encode_case_3_expect[] = {
    0x03,                   /* Type */
    0x1E,                   /* Largest Acknowledged */
    0x40, 0x7d,             /* ACK Delay */
    1,                      /* ACK Range Count */
    10,                     /* First ACK Range */

    8,                      /* Gap */
    10,                     /* Length */

    0x3c,                   /* ECT0 Count */
    0x40, 0x46,             /* ECT1 Count */
    0x40, 0x50,             /* ECNCE Count */
};

/* 4. RESET_STREAM */
static const OSSL_QUIC_FRAME_RESET_STREAM encode_case_4_f = {
    0x1234, 0x9781, 0x11717
};

static int encode_case_4_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_reset_stream(pkt,
                                                              &encode_case_4_f), 1))
        return 0;

    return 1;
}

static int encode_case_4_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_RESET_STREAM f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_reset_stream(pkt, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_mem_eq(&f, sizeof(f), &encode_case_4_f, sizeof(encode_case_4_f)))
        return 0;

    return 1;
}

static const unsigned char encode_case_4_expect[] = {
    0x04,                   /* Type */
    0x52, 0x34,             /* Stream ID */
    0x80, 0x00, 0x97, 0x81, /* App Error Code */
    0x80, 0x01, 0x17, 0x17, /* Final Size */
};

/* 5. STOP_SENDING */
static const OSSL_QUIC_FRAME_STOP_SENDING encode_case_5_f = {
    0x1234, 0x9781
};

static int encode_case_5_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_stop_sending(pkt,
                                                              &encode_case_5_f), 1))
        return 0;

    return 1;
}

static int encode_case_5_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_STOP_SENDING f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_stop_sending(pkt, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_mem_eq(&f, sizeof(f), &encode_case_5_f, sizeof(encode_case_5_f)))
        return 0;

    return 1;
}

static const unsigned char encode_case_5_expect[] = {
    0x05,                   /* Type */
    0x52, 0x34,             /* Stream ID */
    0x80, 0x00, 0x97, 0x81  /* App Error Code */
};

/* 6. CRYPTO */
static const unsigned char encode_case_6_data[] = {
    93, 18, 17, 102, 33
};

static const OSSL_QUIC_FRAME_CRYPTO encode_case_6_f = {
    0x1234, sizeof(encode_case_6_data), encode_case_6_data
};

static int encode_case_6_enc(WPACKET *pkt)
{
    if (!TEST_ptr(ossl_quic_wire_encode_frame_crypto(pkt,
                                                     &encode_case_6_f)))
        return 0;

    return 1;
}

static int encode_case_6_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_CRYPTO f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_crypto(pkt, 0, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(f.offset, 0x1234))
        return 0;

    if (!TEST_uint64_t_le(f.len, SIZE_MAX))
        return 0;

    if (!TEST_mem_eq(f.data, (size_t)f.len,
                     encode_case_6_data, sizeof(encode_case_6_data)))
        return 0;

    return 1;
}

static const unsigned char encode_case_6_expect[] = {
    0x06,                   /* Type */
    0x52, 0x34,             /* Offset */
    0x05,                   /* Length */
    93, 18, 17, 102, 33     /* Data */
};

/* 7. NEW_TOKEN */
static const unsigned char encode_case_7_token[] = {
    0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71,
    0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
};

static int encode_case_7_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_new_token(pkt,
                                                        encode_case_7_token,
                                                        sizeof(encode_case_7_token)), 1))
        return 0;

    return 1;
}

static int encode_case_7_dec(PACKET *pkt, ossl_ssize_t fail)
{
    const unsigned char *token = NULL;
    size_t token_len = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_new_token(pkt,
                                                           &token,
                                                           &token_len), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_mem_eq(token, token_len,
                     encode_case_7_token, sizeof(encode_case_7_token)))
        return 0;

    return 1;
}

static const unsigned char encode_case_7_expect[] = {
    0x07,                   /* Type */
    0x10,                   /* Length */
    0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71, /* Token */
    0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
};

/* 8. STREAM (no length, no offset, no fin) */
static const unsigned char encode_case_8_data[] = {
    0xde, 0x06, 0xcb, 0x76, 0x5d
};
static const OSSL_QUIC_FRAME_STREAM encode_case_8_f = {
   0x1234, 0, 5, encode_case_8_data, 0, 0
};

static int encode_case_8_enc(WPACKET *pkt)
{
    if (!TEST_ptr(ossl_quic_wire_encode_frame_stream(pkt,
                                                     &encode_case_8_f)))
        return 0;

    return 1;
}

static int encode_case_8_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_STREAM f = {0};

    if (fail >= 3)
        /*
         * This case uses implicit length signalling so truncation will not
         * cause it to fail unless the header (which is 3 bytes) is truncated.
         */
        return 1;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_stream(pkt, 0, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_le(f.len, SIZE_MAX))
        return 0;

    if (!TEST_mem_eq(f.data, (size_t)f.len,
                     encode_case_8_data, sizeof(encode_case_8_data)))
        return 0;

    if (!TEST_uint64_t_eq(f.stream_id, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(f.offset, 0))
        return 0;

    if (!TEST_int_eq(f.has_explicit_len, 0))
        return 0;

    if (!TEST_int_eq(f.is_fin, 0))
        return 0;

    return 1;
}

static const unsigned char encode_case_8_expect[] = {
    0x08,                           /* Type (OFF=0, LEN=0, FIN=0) */
    0x52, 0x34,                     /* Stream ID */
    0xde, 0x06, 0xcb, 0x76, 0x5d    /* Data */
};

/* 9. STREAM (length, offset, fin) */
static const unsigned char encode_case_9_data[] = {
    0xde, 0x06, 0xcb, 0x76, 0x5d
};
static const OSSL_QUIC_FRAME_STREAM encode_case_9_f = {
   0x1234, 0x39, 5, encode_case_9_data, 1, 1
};

static int encode_case_9_enc(WPACKET *pkt)
{
    if (!TEST_ptr(ossl_quic_wire_encode_frame_stream(pkt,
                                                     &encode_case_9_f)))
        return 0;

    return 1;
}

static int encode_case_9_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_STREAM f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_stream(pkt, 0, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_le(f.len, SIZE_MAX))
        return 0;

    if (!TEST_mem_eq(f.data, (size_t)f.len,
                     encode_case_9_data, sizeof(encode_case_9_data)))
        return 0;

    if (!TEST_uint64_t_eq(f.stream_id, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(f.offset, 0x39))
        return 0;

    if (!TEST_int_eq(f.has_explicit_len, 1))
        return 0;

    if (!TEST_int_eq(f.is_fin, 1))
        return 0;

    return 1;
}

static const unsigned char encode_case_9_expect[] = {
    0x0f,                           /* Type (OFF=1, LEN=1, FIN=1) */
    0x52, 0x34,                     /* Stream ID */
    0x39,                           /* Offset */
    0x05,                           /* Length */
    0xde, 0x06, 0xcb, 0x76, 0x5d    /* Data */
};

/* 10. MAX_DATA */
static int encode_case_10_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_max_data(pkt, 0x1234), 1))
        return 0;

    return 1;
}

static int encode_case_10_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t max_data = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_max_data(pkt, &max_data), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(max_data, 0x1234))
        return 0;

    return 1;
}

static const unsigned char encode_case_10_expect[] = {
    0x10,                           /* Type */
    0x52, 0x34,                     /* Max Data */
};

/* 11. MAX_STREAM_DATA */
static int encode_case_11_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_max_stream_data(pkt,
                                                                 0x1234,
                                                                 0x9781), 1))
        return 0;

    return 1;
}

static int encode_case_11_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t stream_id = 0, max_data = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_max_stream_data(pkt,
                                                                 &stream_id,
                                                                 &max_data), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(stream_id, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(max_data, 0x9781))
        return 0;

    return 1;
}

static const unsigned char encode_case_11_expect[] = {
    0x11,                           /* Type */
    0x52, 0x34,                     /* Stream ID */
    0x80, 0x00, 0x97, 0x81,         /* Max Data */
};

/* 12. MAX_STREAMS */
static int encode_case_12_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_max_streams(pkt, 0, 0x1234), 1))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_encode_frame_max_streams(pkt, 1, 0x9781), 1))
        return 0;

    return 1;
}

static int encode_case_12_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t max_streams_1 = 0, max_streams_2 = 0,
            frame_type_1 = 0, frame_type_2 = 0;
    int is_minimal = 1, success_if;

    success_if = (fail < 0 || fail >= 1);
    if (!TEST_int_eq(ossl_quic_wire_peek_frame_header(pkt, &frame_type_1,
                                                      &is_minimal),
                     success_if))
        return 0;

    if (!TEST_true(!success_if || is_minimal))
        return 0;

    success_if = (fail < 0 || fail >= 3);
    if (!TEST_int_eq(ossl_quic_wire_decode_frame_max_streams(pkt,
                                                             &max_streams_1),
                     success_if))
        return 0;

    success_if = (fail < 0 || fail >= 4);
    if (!TEST_int_eq(ossl_quic_wire_peek_frame_header(pkt, &frame_type_2,
                                                      &is_minimal),
                     success_if))
        return 0;

    if (!TEST_true(!success_if || is_minimal))
        return 0;

    success_if = (fail < 0);
    if (!TEST_int_eq(ossl_quic_wire_decode_frame_max_streams(pkt,
                                                             &max_streams_2),
                     success_if))
        return 0;

    if ((fail < 0 || fail >= 3)
        && !TEST_uint64_t_eq(frame_type_1, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI))
        return 0;

    if ((fail < 0 || fail >= 3)
        && !TEST_uint64_t_eq(max_streams_1, 0x1234))
        return 0;

    if ((fail < 0 || fail >= 8)
        && !TEST_uint64_t_eq(frame_type_2, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI))
        return 0;

    if ((fail < 0 || fail >= 8)
        && !TEST_uint64_t_eq(max_streams_2, 0x9781))
        return 0;

    return 1;
}

static const unsigned char encode_case_12_expect[] = {
    0x12,                           /* Type (MAX_STREAMS Bidirectional) */
    0x52, 0x34,                     /* Max Streams */
    0x13,                           /* Type (MAX_STREAMS Unidirectional) */
    0x80, 0x00, 0x97, 0x81,         /* Max Streams */
};

/* 13. DATA_BLOCKED */
static int encode_case_13_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_data_blocked(pkt, 0x1234), 1))
        return 0;

    return 1;
}

static int encode_case_13_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t max_data = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_data_blocked(pkt,
                                                              &max_data), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(max_data, 0x1234))
        return 0;

    return 1;
}

static const unsigned char encode_case_13_expect[] = {
    0x14,                           /* Type */
    0x52, 0x34,                     /* Max Data */
};

/* 14. STREAM_DATA_BLOCKED */
static int encode_case_14_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_stream_data_blocked(pkt,
                                                                     0x1234,
                                                                     0x9781), 1))
        return 0;

    return 1;
}

static int encode_case_14_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t stream_id = 0, max_data = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_stream_data_blocked(pkt,
                                                                     &stream_id,
                                                                     &max_data), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(stream_id, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(max_data, 0x9781))
        return 0;

    return 1;
}

static const unsigned char encode_case_14_expect[] = {
    0x15,                           /* Type */
    0x52, 0x34,                     /* Stream ID */
    0x80, 0x00, 0x97, 0x81,         /* Max Data */
};

/* 15. STREAMS_BLOCKED */
static int encode_case_15_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_streams_blocked(pkt, 0, 0x1234), 1))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_encode_frame_streams_blocked(pkt, 1, 0x9781), 1))
        return 0;

    return 1;
}

static int encode_case_15_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t max_streams_1 = 0, max_streams_2 = 0,
            frame_type_1 = 0, frame_type_2 = 0;
    int is_minimal = 1, success_if;

    success_if = (fail < 0 || fail >= 1);
    if (!TEST_int_eq(ossl_quic_wire_peek_frame_header(pkt, &frame_type_1,
                                                      &is_minimal),
                     success_if))
        return 0;

    if (!TEST_true(!success_if || is_minimal))
        return 0;

    success_if = (fail < 0 || fail >= 3);
    if (!TEST_int_eq(ossl_quic_wire_decode_frame_streams_blocked(pkt,
                                                                 &max_streams_1),
                     success_if))
        return 0;

    success_if = (fail < 0 || fail >= 4);
    if (!TEST_int_eq(ossl_quic_wire_peek_frame_header(pkt, &frame_type_2,
                                                      &is_minimal),
                     success_if))
        return 0;

    if (!TEST_true(!success_if || is_minimal))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_streams_blocked(pkt,
                                                                 &max_streams_2),
                     fail < 0 || fail >= 8))
        return 0;

    if ((fail < 0 || fail >= 1)
        && !TEST_uint64_t_eq(frame_type_1, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI))
        return 0;

    if ((fail < 0 || fail >= 3)
        && !TEST_uint64_t_eq(max_streams_1, 0x1234))
        return 0;

    if ((fail < 0 || fail >= 4)
        && !TEST_uint64_t_eq(frame_type_2, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI))
        return 0;

    if ((fail < 0 || fail >= 8)
        && !TEST_uint64_t_eq(max_streams_2, 0x9781))
        return 0;

    return 1;
}

static const unsigned char encode_case_15_expect[] = {
    0x16,                           /* Type (STREAMS_BLOCKED Bidirectional) */
    0x52, 0x34,                     /* Max Streams */
    0x17,                           /* Type (STREAMS_BLOCKED Unidirectional) */
    0x80, 0x00, 0x97, 0x81,         /* Max Streams */
};

/* 16. NEW_CONNECTION_ID */
static const unsigned char encode_case_16_conn_id[] = {
    0x33, 0x44, 0x55, 0x66
};

static const OSSL_QUIC_FRAME_NEW_CONN_ID encode_case_16_f = {
    0x9781,
    0x1234,
    {
        0x4,
        {0x33, 0x44, 0x55, 0x66}
    },
    {
        {
            0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71,
            0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
        }
    }
};

static int encode_case_16_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_new_conn_id(pkt,
                                                             &encode_case_16_f), 1))
        return 0;

    return 1;
}

static int encode_case_16_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_NEW_CONN_ID f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_new_conn_id(pkt, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(f.seq_num, 0x9781))
        return 0;

    if (!TEST_uint64_t_eq(f.retire_prior_to, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(f.conn_id.id_len, sizeof(encode_case_16_conn_id)))
        return 0;

    if (!TEST_mem_eq(f.conn_id.id, f.conn_id.id_len,
                     encode_case_16_conn_id, sizeof(encode_case_16_conn_id)))
        return 0;

    if (!TEST_mem_eq(f.stateless_reset.token,
                     sizeof(f.stateless_reset.token),
                     encode_case_16_f.stateless_reset.token,
                     sizeof(encode_case_16_f.stateless_reset.token)))
        return 0;

    return 1;
}

static const unsigned char encode_case_16_expect[] = {
    0x18,                           /* Type */
    0x80, 0x00, 0x97, 0x81,         /* Sequence Number */
    0x52, 0x34,                     /* Retire Prior To */
    0x04,                           /* Connection ID Length */
    0x33, 0x44, 0x55, 0x66,         /* Connection ID */
    0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71, /* Stateless Reset Token */
    0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
};

/* 16b. NEW_CONNECTION_ID seq_num < retire_prior_to */
static const OSSL_QUIC_FRAME_NEW_CONN_ID encode_case_16b_f = {
    0x1234,
    0x9781,
    {
        0x4,
        {0x33, 0x44, 0x55, 0x66}
    },
    {
        {
            0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71,
            0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
        }
    }
};

static int encode_case_16b_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_new_conn_id(pkt,
                                                             &encode_case_16b_f), 1))
        return 0;

    return 1;
}

static int encode_case_16b_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_NEW_CONN_ID f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_new_conn_id(pkt, &f), 0))
        return 0;

    if (!TEST_true(PACKET_forward(pkt, PACKET_remaining(pkt))))
        return 0;

    return 1;
}

static const unsigned char encode_case_16b_expect[] = {
    0x18,                           /* Type */
    0x52, 0x34,                     /* Sequence Number */
    0x80, 0x00, 0x97, 0x81,         /* Retire Prior To */
    0x04,                           /* Connection ID Length */
    0x33, 0x44, 0x55, 0x66,         /* Connection ID */
    0xde, 0x06, 0xcb, 0x76, 0x5d, 0xb1, 0xa7, 0x71, /* Stateless Reset Token */
    0x78, 0x09, 0xbb, 0xe8, 0x50, 0x19, 0x12, 0x9a
};

/* 17. RETIRE_CONNECTION_ID */
static int encode_case_17_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_retire_conn_id(pkt, 0x1234), 1))
        return 0;

    return 1;
}

static int encode_case_17_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t seq_num = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_retire_conn_id(pkt, &seq_num), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(seq_num, 0x1234))
        return 0;

    return 1;
}

static const unsigned char encode_case_17_expect[] = {
    0x19,                           /* Type */
    0x52, 0x34,                     /* Seq Num */
};

/* 18. PATH_CHALLENGE */
static const uint64_t encode_case_18_data
    = (((uint64_t)0x5f4b12)<<40) | (uint64_t)0x731834UL;

static int encode_case_18_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_path_challenge(pkt,
                                                                encode_case_18_data), 1))
        return 0;

    return 1;
}

static int encode_case_18_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t challenge = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_path_challenge(pkt, &challenge), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(challenge, encode_case_18_data))
        return 0;

    return 1;
}

static const unsigned char encode_case_18_expect[] = {
    0x1A,                                           /* Type */
    0x5f, 0x4b, 0x12, 0x00, 0x00, 0x73, 0x18, 0x34, /* Data */
};

/* 19. PATH_RESPONSE */
static const uint64_t encode_case_19_data
    = (((uint64_t)0x5f4b12)<<40) | (uint64_t)0x731834UL;

static int encode_case_19_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_path_response(pkt,
                                                               encode_case_19_data), 1))
        return 0;

    return 1;
}

static int encode_case_19_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t challenge = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_path_response(pkt, &challenge), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_uint64_t_eq(challenge, encode_case_19_data))
        return 0;

    return 1;
}

static const unsigned char encode_case_19_expect[] = {
    0x1B,                                           /* Type */
    0x5f, 0x4b, 0x12, 0x00, 0x00, 0x73, 0x18, 0x34, /* Data */
};

/* 20. CONNECTION_CLOSE (transport) */
static const char encode_case_20_reason[] = {
    /* "reason for closure" */
    0x72, 0x65, 0x61, 0x73, 0x6f, 0x6e, 0x20, 0x66, 0x6f,
    0x72, 0x20, 0x63, 0x6c, 0x6f, 0x73, 0x75, 0x72, 0x65
};

static const OSSL_QUIC_FRAME_CONN_CLOSE encode_case_20_f = {
    0,
    0x1234,
    0x9781,
    (char *)encode_case_20_reason,
    sizeof(encode_case_20_reason)
};

static int encode_case_20_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_frame_conn_close(pkt,
                                                            &encode_case_20_f), 1))
        return 0;

    return 1;
}

static int encode_case_20_dec(PACKET *pkt, ossl_ssize_t fail)
{
    OSSL_QUIC_FRAME_CONN_CLOSE f = {0};

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_conn_close(pkt, &f), fail < 0))
        return 0;

    if (fail >= 0)
        return 1;

    if (!TEST_int_eq(f.is_app, 0))
        return 0;

    if (!TEST_uint64_t_eq(f.error_code, 0x1234))
        return 0;

    if (!TEST_uint64_t_eq(f.frame_type, 0x9781))
        return 0;

    if (!TEST_size_t_eq(f.reason_len, 18))
        return 0;

    if (!TEST_mem_eq(f.reason, f.reason_len,
                     encode_case_20_f.reason, encode_case_20_f.reason_len))
        return 0;

    return 1;
}

static const unsigned char encode_case_20_expect[] = {
    0x1C,                           /* Type */
    0x52, 0x34,                     /* Sequence Number */
    0x80, 0x00, 0x97, 0x81,         /* Frame Type */
    0x12,                           /* Reason Length */
    0x72, 0x65, 0x61, 0x73, 0x6f, 0x6e, 0x20, 0x66, 0x6f, /* Reason */
    0x72, 0x20, 0x63, 0x6c, 0x6f, 0x73, 0x75, 0x72, 0x65
};

/* 21. HANDSHAKE_DONE */
static int encode_case_21_enc(WPACKET *pkt)
{

    if (!TEST_int_eq(ossl_quic_wire_encode_frame_handshake_done(pkt), 1))
        return 0;

    return 1;
}

static int encode_case_21_dec(PACKET *pkt, ossl_ssize_t fail)
{

    if (!TEST_int_eq(ossl_quic_wire_decode_frame_handshake_done(pkt), fail < 0))
        return 0;

    return 1;
}

static const unsigned char encode_case_21_expect[] = {
    0x1E
};

/* 22. Buffer Transport Parameter */
static const unsigned char encode_case_22_data[] = {0x55,0x77,0x32,0x46,0x99};

static int encode_case_22_enc(WPACKET *pkt)
{
    unsigned char *p;

    if (!TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(pkt, 0x1234,
                                                              encode_case_22_data,
                                                              sizeof(encode_case_22_data))))
        return 0;

    if (!TEST_ptr(p = ossl_quic_wire_encode_transport_param_bytes(pkt, 0x9781,
                                                                  NULL, 2)))
        return 0;

    p[0] = 0x33;
    p[1] = 0x44;

    return 1;
}

static int encode_case_22_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t id = 0;
    size_t len = 0;
    const unsigned char *p;
    static const unsigned char data[] = {0x33, 0x44};

    if (!TEST_int_eq(ossl_quic_wire_peek_transport_param(pkt, &id),
                     fail < 0 || fail >= 2))
        return 0;

    if ((fail < 0 || fail >= 2)
        && !TEST_uint64_t_eq(id, 0x1234))
        return 0;

    id = 0;

    p = ossl_quic_wire_decode_transport_param_bytes(pkt, &id, &len);
    if (fail < 0 || fail >= 8) {
        if (!TEST_ptr(p))
            return 0;
    } else {
        if (!TEST_ptr_null(p))
            return 0;
    }

    if ((fail < 0 || fail >= 8)
        && !TEST_uint64_t_eq(id, 0x1234))
        return 0;

    if ((fail < 0 || fail >= 8)
        && !TEST_mem_eq(p, len, encode_case_22_data, sizeof(encode_case_22_data)))
        return 0;

    if ((fail < 0 || fail >= 8)
        && !TEST_int_eq(ossl_quic_wire_peek_transport_param(pkt, &id),
                        fail < 0 || fail >= 12))
        return 0;

    if ((fail < 0 || fail >= 12)
        && !TEST_uint64_t_eq(id, 0x9781))
        return 0;

    id = 0;

    p = ossl_quic_wire_decode_transport_param_bytes(pkt, &id, &len);
    if (fail < 0 || fail >= 15) {
        if (!TEST_ptr(p))
            return 0;
    } else {
        if (!TEST_ptr_null(p))
            return 0;
    }

    if ((fail < 0 || fail >= 15)
        && !TEST_uint64_t_eq(id, 0x9781))
        return 0;

    if ((fail < 0 || fail >= 15)
        && !TEST_mem_eq(p, len, data, sizeof(data)))
        return 0;

    return 1;
}

static const unsigned char encode_case_22_expect[] = {
    0x52, 0x34,                         /* ID */
    0x05,                               /* Length */
    0x55, 0x77, 0x32, 0x46, 0x99,       /* Data */

    0x80, 0x00, 0x97, 0x81,             /* ID */
    0x02,                               /* Length */
    0x33, 0x44                          /* Data */
};

/* 23. Integer Transport Parameter */
static int encode_case_23_enc(WPACKET *pkt)
{
    if (!TEST_int_eq(ossl_quic_wire_encode_transport_param_int(pkt, 0x1234, 0x9781), 1))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_encode_transport_param_int(pkt, 0x2233, 0x4545), 1))
        return 0;

    return 1;
}

static int encode_case_23_dec(PACKET *pkt, ossl_ssize_t fail)
{
    uint64_t id = 0, value = 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_transport_param_int(pkt,
                                                               &id, &value),
                     fail < 0 || fail >= 7))
        return 0;

    if ((fail < 0 || fail >= 7)
        && !TEST_uint64_t_eq(id, 0x1234))
        return 0;

    if ((fail < 0 || fail >= 7)
        && !TEST_uint64_t_eq(value, 0x9781))
        return 0;

    if (!TEST_int_eq(ossl_quic_wire_decode_transport_param_int(pkt,
                                                               &id, &value),
                     fail < 0 || fail >= 14))
        return 0;

    if ((fail < 0 || fail >= 14)
        && !TEST_uint64_t_eq(id, 0x2233))
        return 0;

    if ((fail < 0 || fail >= 14)
        && !TEST_uint64_t_eq(value, 0x4545))
        return 0;

    return 1;
}

static const unsigned char encode_case_23_expect[] = {
    0x52, 0x34,
    0x04,
    0x80, 0x00, 0x97, 0x81,

    0x62, 0x33,
    0x04,
    0x80, 0x00, 0x45, 0x45,
};

#define ENCODE_CASE(n)                          \
    {                                           \
      encode_case_##n##_enc,                    \
      encode_case_##n##_expect,                 \
      OSSL_NELEM(encode_case_##n##_expect),     \
      encode_case_##n##_dec                     \
    },

static const struct encode_test_case encode_cases[] = {
    ENCODE_CASE(1)
    ENCODE_CASE(2)
    ENCODE_CASE(3)
    ENCODE_CASE(4)
    ENCODE_CASE(5)
    ENCODE_CASE(6)
    ENCODE_CASE(7)
    ENCODE_CASE(8)
    ENCODE_CASE(9)
    ENCODE_CASE(10)
    ENCODE_CASE(11)
    ENCODE_CASE(12)
    ENCODE_CASE(13)
    ENCODE_CASE(14)
    ENCODE_CASE(15)
    ENCODE_CASE(16)
    ENCODE_CASE(16b)
    ENCODE_CASE(17)
    ENCODE_CASE(18)
    ENCODE_CASE(19)
    ENCODE_CASE(20)
    ENCODE_CASE(21)
    ENCODE_CASE(22)
    ENCODE_CASE(23)
};

static int test_wire_encode(int idx)
{
    int testresult = 0;
    WPACKET wpkt;
    PACKET pkt;
    BUF_MEM *buf = NULL;
    size_t written;
    const struct encode_test_case *c = &encode_cases[idx];
    int have_wpkt = 0;
    size_t i;

    if (!TEST_ptr(buf = BUF_MEM_new()))
        goto err;

    if (!TEST_int_eq(WPACKET_init(&wpkt, buf), 1))
        goto err;

    have_wpkt = 1;
    if (!TEST_int_eq(c->serializer(&wpkt), 1))
        goto err;

    if (!TEST_int_eq(WPACKET_get_total_written(&wpkt, &written), 1))
        goto err;

    if (!TEST_mem_eq(buf->data, written, c->expect_buf, c->expect_buf_len))
        goto err;

    if (!TEST_int_eq(PACKET_buf_init(&pkt, (unsigned char *)buf->data, written), 1))
        goto err;

    if (!TEST_int_eq(c->deserializer(&pkt, -1), 1))
        goto err;

    if (!TEST_false(PACKET_remaining(&pkt)))
        goto err;

    for (i = 0; i < c->expect_buf_len; ++i) {
        PACKET pkt2;

        /*
         * Check parsing truncated (i.e., malformed) input is handled correctly.
         * Generate all possible truncations of our reference encoding and
         * verify that they are handled correctly. The number of bytes of the
         * truncated encoding is passed as an argument to the deserializer to
         * help it determine whether decoding should fail or not.
         */
        if (!TEST_int_eq(PACKET_buf_init(&pkt2, (unsigned char *)c->expect_buf, i), 1))
            goto err;

        if (!TEST_int_eq(c->deserializer(&pkt2, i), 1))
            goto err;
    }

    testresult = 1;
err:
    if (have_wpkt)
        WPACKET_finish(&wpkt);
    BUF_MEM_free(buf);
    return testresult;
}

struct ack_test_case {
    const unsigned char    *input_buf;
    size_t                  input_buf_len;
    int                   (*deserializer)(PACKET *pkt);
    int                     expect_fail;
};

/* ACK Frame with Excessive First ACK Range Field */
static const unsigned char ack_case_1_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x00,           /* ACK Range Count */
    0x09,           /* First ACK Range */
};

/* ACK Frame with Valid ACK Range Field */
static const unsigned char ack_case_2_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x00,           /* ACK Range Count */
    0x08,           /* First ACK Range */
};

/* ACK Frame with Excessive ACK Range Gap */
static const unsigned char ack_case_3_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x01,           /* ACK Range Count */
    0x01,           /* First ACK Range */

    0x05,           /* Gap */
    0x01,           /* ACK Range Length */
};

/* ACK Frame with Valid ACK Range */
static const unsigned char ack_case_4_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x01,           /* ACK Range Count */
    0x01,           /* First ACK Range */

    0x04,           /* Gap */
    0x01,           /* ACK Range Length */
};

/* ACK Frame with Excessive ACK Range Length */
static const unsigned char ack_case_5_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x01,           /* ACK Range Count */
    0x01,           /* First ACK Range */

    0x04,           /* Gap */
    0x02,           /* ACK Range Length */
};

/* ACK Frame with Multiple ACK Ranges, Final Having Excessive Length */
static const unsigned char ack_case_6_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x02,           /* ACK Range Count */
    0x01,           /* First ACK Range */

    0x01,           /* Gap */
    0x02,           /* ACK Range Length */

    0x00,           /* Gap */
    0x01,           /* ACK Range Length */
};

/* ACK Frame with Multiple ACK Ranges, Valid */
static const unsigned char ack_case_7_input[] = {
    0x02,           /* ACK Without ECN */
    0x08,           /* Largest Acknowledged */
    0x01,           /* ACK Delay */
    0x02,           /* ACK Range Count */
    0x01,           /* First ACK Range */

    0x01,           /* Gap */
    0x02,           /* ACK Range Length */

    0x00,           /* Gap */
    0x00,           /* ACK Range Length */
};

static int ack_generic_decode(PACKET *pkt)
{
    OSSL_QUIC_ACK_RANGE ranges[8] = {0};
    OSSL_QUIC_FRAME_ACK f = {0};
    uint64_t total_ranges = 0, peek_total_ranges = 0;
    int r;
    size_t i;

    f.ack_ranges        = ranges;
    f.num_ack_ranges    = OSSL_NELEM(ranges);

    if (!TEST_int_eq(ossl_quic_wire_peek_frame_ack_num_ranges(pkt,
                                                              &peek_total_ranges), 1))
        return 0;

    r = ossl_quic_wire_decode_frame_ack(pkt, 3, &f, &total_ranges);
    if (r == 0)
        return 0;

    if (!TEST_uint64_t_eq(total_ranges, peek_total_ranges))
        return 0;

    for (i = 0; i < f.num_ack_ranges; ++i) {
        if (!TEST_uint64_t_le(f.ack_ranges[i].start, f.ack_ranges[i].end))
            return 0;
        if (!TEST_uint64_t_lt(f.ack_ranges[i].end, 1000))
            return 0;
    }

    return 1;
}

#define ACK_CASE(n, expect_fail, dec)   \
    {                                   \
        ack_case_##n##_input,           \
        sizeof(ack_case_##n##_input),   \
        (dec),                          \
        (expect_fail)                   \
    },

static const struct ack_test_case ack_cases[] = {
    ACK_CASE(1, 1, ack_generic_decode)
    ACK_CASE(2, 0, ack_generic_decode)
    ACK_CASE(3, 1, ack_generic_decode)
    ACK_CASE(4, 0, ack_generic_decode)
    ACK_CASE(5, 1, ack_generic_decode)
    ACK_CASE(6, 1, ack_generic_decode)
    ACK_CASE(7, 0, ack_generic_decode)
};

static int test_wire_ack(int idx)
{
    int testresult = 0, r;
    PACKET pkt;
    const struct ack_test_case *c = &ack_cases[idx];

    if (!TEST_int_eq(PACKET_buf_init(&pkt,
                                     (unsigned char *)c->input_buf,
                                     c->input_buf_len), 1))
        goto err;

    r = c->deserializer(&pkt);
    if (c->expect_fail) {
        if (!TEST_int_eq(r, 0))
            goto err;
    } else {
        if (!TEST_int_eq(r, 1))
            goto err;

        if (!TEST_false(PACKET_remaining(&pkt)))
            goto err;
    }

    testresult = 1;
err:
    return testresult;
}

/* Packet Header PN Encoding Tests */
struct pn_test {
    QUIC_PN         pn, tx_largest_acked, rx_largest_pn;
    char            expected_len;
    unsigned char   expected_bytes[4];
};

static const struct pn_test pn_tests[] = {
    /* RFC 9000 Section A.2 */
    { 0xac5c02, 0xabe8b3, 0xabe8b3, 2, {0x5c,0x02} },
    { 0xace8fe, 0xabe8b3, 0xabe8b3, 3, {0xac,0xe8,0xfe} },
    /* RFC 9000 Section A.3 */
    { 0xa82f9b32, 0xa82f30ea, 0xa82f30ea, 2, {0x9b,0x32} },
    /* Boundary Cases */
    { 1, 0, 0, 1, {0x01} },
    { 256, 255, 255, 1, {0x00} },
    { 257, 255, 255, 1, {0x01} },
    { 256, 128, 128, 1, {0x00} },
    { 256, 127, 127, 2, {0x01,0x00} },
    { 65536, 32768, 32768, 2, {0x00,0x00} },
    { 65537, 32769, 32769, 2, {0x00,0x01} },
    { 65536, 32767, 32767, 3, {0x01,0x00,0x00} },
    { 65537, 32768, 32768, 3, {0x01,0x00,0x01} },
    { 16777216, 8388608, 8388608, 3, {0x00,0x00,0x00} },
    { 16777217, 8388609, 8388609, 3, {0x00,0x00,0x01} },
    { 16777216, 8388607, 8388607, 4, {0x01,0x00,0x00,0x00} },
    { 16777217, 8388608, 8388608, 4, {0x01,0x00,0x00,0x01} },
    { 4294967296, 2147483648, 2147483648, 4, {0x00,0x00,0x00,0x00} },
    { 4294967297, 2147483648, 2147483648, 4, {0x00,0x00,0x00,0x01} },
};

static int test_wire_pkt_hdr_pn(int tidx)
{
    int testresult = 0;
    const struct pn_test *t = &pn_tests[tidx];
    unsigned char buf[4];
    int pn_len;
    QUIC_PN res_pn;

    pn_len = ossl_quic_wire_determine_pn_len(t->pn, t->tx_largest_acked);
    if (!TEST_int_eq(pn_len, (int)t->expected_len))
        goto err;

    if (!TEST_true(ossl_quic_wire_encode_pkt_hdr_pn(t->pn, buf, pn_len)))
        goto err;

    if (!TEST_mem_eq(t->expected_bytes, t->expected_len, buf, pn_len))
        goto err;

    if (!TEST_true(ossl_quic_wire_decode_pkt_hdr_pn(buf, pn_len,
                                                    t->rx_largest_pn, &res_pn)))
        goto err;

    if (!TEST_uint64_t_eq(res_pn, t->pn))
        goto err;

    testresult = 1;
err:
    return testresult;
}

/* RFC 9001 s. A.4 */
static const QUIC_CONN_ID retry_orig_dcid = {
    8, { 0x83, 0x94, 0xc8, 0xf0, 0x3e, 0x51, 0x57, 0x08 }
};

static const unsigned char retry_encoded[] = {
  0xff,                                                 /* Long Header, Retry */
  0x00, 0x00, 0x00, 0x01,                               /* Version 1 */
  0x00,                                                 /* DCID */
  0x08, 0xf0, 0x67, 0xa5, 0x50, 0x2a, 0x42, 0x62, 0xb5, /* SCID */

  /* Retry Token */
  0x74, 0x6f, 0x6b, 0x65, 0x6e,

  /* Retry Integrity Tag */
  0x04, 0xa2, 0x65, 0xba, 0x2e, 0xff, 0x4d, 0x82, 0x90, 0x58, 0xfb, 0x3f, 0x0f,
  0x24, 0x96, 0xba
};

static int test_wire_retry_integrity_tag(void)
{
    int testresult = 0;
    PACKET pkt = {0};
    QUIC_PKT_HDR hdr = {0};
    unsigned char got_tag[QUIC_RETRY_INTEGRITY_TAG_LEN] = {0};

    if (!TEST_true(PACKET_buf_init(&pkt, retry_encoded, sizeof(retry_encoded))))
        goto err;

    if (!TEST_true(ossl_quic_wire_decode_pkt_hdr(&pkt, 0, 0, 0, &hdr, NULL, NULL)))
        goto err;

    if (!TEST_int_eq(hdr.type, QUIC_PKT_TYPE_RETRY))
        goto err;

    if (!TEST_true(ossl_quic_calculate_retry_integrity_tag(NULL, NULL, &hdr,
                                                           &retry_orig_dcid,
                                                           got_tag)))
        goto err;

    if (!TEST_mem_eq(got_tag, sizeof(got_tag),
                     retry_encoded + sizeof(retry_encoded)
                        - QUIC_RETRY_INTEGRITY_TAG_LEN,
                     QUIC_RETRY_INTEGRITY_TAG_LEN))
        goto err;

    if (!TEST_true(ossl_quic_validate_retry_integrity_tag(NULL, NULL, &hdr,
                                                          &retry_orig_dcid)))
        goto err;

    testresult = 1;
err:
    return testresult;
}

/* is_minimal=0 test */
static const unsigned char non_minimal_1[] = {
    0x40, 0x00,
};

static const unsigned char non_minimal_2[] = {
    0x40, 0x3F,
};

static const unsigned char non_minimal_3[] = {
    0x80, 0x00, 0x00, 0x00,
};

static const unsigned char non_minimal_4[] = {
    0x80, 0x00, 0x3F, 0xFF,
};

static const unsigned char non_minimal_5[] = {
    0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
};

static const unsigned char non_minimal_6[] = {
    0xC0, 0x00, 0x00, 0x00, 0x3F, 0xFF, 0xFF, 0xFF
};

static const unsigned char *const non_minimal[] = {
    non_minimal_1,
    non_minimal_2,
    non_minimal_3,
    non_minimal_4,
    non_minimal_5,
    non_minimal_6,
};

static const size_t non_minimal_len[] = {
    OSSL_NELEM(non_minimal_1),
    OSSL_NELEM(non_minimal_2),
    OSSL_NELEM(non_minimal_3),
    OSSL_NELEM(non_minimal_4),
    OSSL_NELEM(non_minimal_5),
    OSSL_NELEM(non_minimal_6),
};

static int test_wire_minimal(int idx)
{
    int testresult = 0;
    int is_minimal;
    uint64_t frame_type;
    PACKET pkt;

    if (!TEST_true(PACKET_buf_init(&pkt, non_minimal[idx],
                                   non_minimal_len[idx])))
        goto err;

    if (!TEST_true(ossl_quic_wire_peek_frame_header(&pkt, &frame_type,
                                                    &is_minimal)))
        goto err;

    if (!TEST_false(is_minimal))
        goto err;

    testresult = 1;
err:
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_wire_encode,     OSSL_NELEM(encode_cases));
    ADD_ALL_TESTS(test_wire_ack,        OSSL_NELEM(ack_cases));
    ADD_ALL_TESTS(test_wire_pkt_hdr_pn, OSSL_NELEM(pn_tests));
    ADD_TEST(test_wire_retry_integrity_tag);
    ADD_ALL_TESTS(test_wire_minimal,    OSSL_NELEM(non_minimal_len));
    return 1;
}
