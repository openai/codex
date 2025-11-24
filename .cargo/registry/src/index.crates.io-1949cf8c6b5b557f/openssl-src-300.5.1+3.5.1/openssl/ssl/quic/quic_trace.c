/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/bio.h>
#include "../ssl_local.h"
#include "internal/quic_trace.h"
#include "internal/quic_ssl.h"
#include "internal/quic_channel.h"
#include "internal/quic_wire_pkt.h"
#include "internal/quic_wire.h"
#include "internal/ssl_unwrap.h"

static const char *packet_type(int type)
{
    switch (type) {
    case QUIC_PKT_TYPE_INITIAL:
        return "Initial";

    case QUIC_PKT_TYPE_0RTT:
        return "0RTT";

    case QUIC_PKT_TYPE_HANDSHAKE:
        return "Handshake";

    case QUIC_PKT_TYPE_RETRY:
        return "Retry";

    case QUIC_PKT_TYPE_1RTT:
        return "1RTT";

    case QUIC_PKT_TYPE_VERSION_NEG:
        return "VersionNeg";

    default:
        return "Unknown";
    }
}

/* Print a non-NUL terminated string to BIO */
static void put_str(BIO *bio, char *str, size_t slen)
{
    size_t i;

    for (i = 0; i < slen; i++)
        BIO_printf(bio, "%c", str[i]);
}

static void put_data(BIO *bio, const uint8_t *data, size_t datalen)
{
    size_t i;

    for (i = 0; i < datalen; i++)
        BIO_printf(bio, "%02x", data[i]);
}

static void put_conn_id(BIO *bio, QUIC_CONN_ID *id)
{
    if (id->id_len == 0) {
        BIO_puts(bio, "<zero length id>");
        return;
    }

    BIO_puts(bio, "0x");
    put_data(bio, id->id, id->id_len);
}

static void put_token(BIO *bio, const uint8_t *token, size_t token_len)
{
    if (token_len == 0)
        BIO_puts(bio, "<zero length token>");
    else
        put_data(bio, token, token_len);
}

static int frame_ack(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_ACK ack;
    OSSL_QUIC_ACK_RANGE *ack_ranges = NULL;
    uint64_t total_ranges = 0;
    uint64_t i;
    int ret = 0;

    if (!ossl_quic_wire_peek_frame_ack_num_ranges(pkt, &total_ranges)
        /* In case sizeof(uint64_t) > sizeof(size_t) */
        || total_ranges > SIZE_MAX / sizeof(ack_ranges[0])
        || (ack_ranges = OPENSSL_zalloc(sizeof(ack_ranges[0])
                                        * (size_t)total_ranges)) == NULL)
        return ret;

    ack.ack_ranges = ack_ranges;
    ack.num_ack_ranges = (size_t)total_ranges;

    /* Ack delay exponent is 0, so we can get the raw delay time below */
    if (!ossl_quic_wire_decode_frame_ack(pkt, 0, &ack, NULL))
        goto end;

    BIO_printf(bio, "    Largest acked: %llu\n",
               (unsigned long long)ack.ack_ranges[0].end);
    BIO_printf(bio, "    Ack delay (raw) %llu\n",
               (unsigned long long)ossl_time2ticks(ack.delay_time));
    BIO_printf(bio, "    Ack range count: %llu\n",
               (unsigned long long)total_ranges - 1);
    BIO_printf(bio, "    First ack range: %llu\n",
               (unsigned long long)(ack.ack_ranges[0].end
                                    - ack.ack_ranges[0].start));
    for (i = 1; i < total_ranges; i++) {
        BIO_printf(bio, "    Gap: %llu\n",
                   (unsigned long long)(ack.ack_ranges[i - 1].start
                                        - ack.ack_ranges[i].end - 2));
        BIO_printf(bio, "    Ack range len: %llu\n",
                   (unsigned long long)(ack.ack_ranges[i].end
                                        - ack.ack_ranges[i].start));
    }

    ret = 1;
end:
    OPENSSL_free(ack_ranges);
    return ret;
}

static int frame_reset_stream(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_RESET_STREAM frame_data;

    if (!ossl_quic_wire_decode_frame_reset_stream(pkt, &frame_data))
        return 0;

    BIO_printf(bio, "    Stream id: %llu\n",
               (unsigned long long)frame_data.stream_id);
    BIO_printf(bio, "    App Protocol Error Code: %llu\n",
               (unsigned long long)frame_data.app_error_code);
    BIO_printf(bio, "    Final size: %llu\n",
               (unsigned long long)frame_data.final_size);

    return 1;
}

static int frame_stop_sending(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_STOP_SENDING frame_data;

    if (!ossl_quic_wire_decode_frame_stop_sending(pkt, &frame_data))
        return 0;

    BIO_printf(bio, "    Stream id: %llu\n",
               (unsigned long long)frame_data.stream_id);
    BIO_printf(bio, "    App Protocol Error Code: %llu\n",
               (unsigned long long)frame_data.app_error_code);

    return 1;
}

static int frame_crypto(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_CRYPTO frame_data;

    if (!ossl_quic_wire_decode_frame_crypto(pkt, 1, &frame_data))
        return 0;

    BIO_printf(bio, "    Offset: %llu\n", (unsigned long long)frame_data.offset);
    BIO_printf(bio, "    Len: %llu\n", (unsigned long long)frame_data.len);

    return 1;
}

static int frame_new_token(BIO *bio, PACKET *pkt)
{
    const uint8_t *token;
    size_t token_len;

    if (!ossl_quic_wire_decode_frame_new_token(pkt, &token, &token_len))
        return 0;

    BIO_puts(bio, "    Token: ");
    put_token(bio, token, token_len);
    BIO_puts(bio, "\n");

    return 1;
}

static int frame_stream(BIO *bio, PACKET *pkt, uint64_t frame_type)
{

    OSSL_QUIC_FRAME_STREAM frame_data;

    BIO_puts(bio, "Stream");
    switch(frame_type) {
    case OSSL_QUIC_FRAME_TYPE_STREAM:
        BIO_puts(bio, "\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_FIN:
        BIO_puts(bio, " (Fin)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN:
        BIO_puts(bio, " (Len)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN_FIN:
        BIO_puts(bio, " (Len, Fin)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF:
        BIO_puts(bio, " (Off)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_FIN:
        BIO_puts(bio, " (Off, Fin)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN:
        BIO_puts(bio, " (Off, Len)\n");
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN_FIN:
        BIO_puts(bio, " (Off, Len, Fin)\n");
        break;

    default:
        return 0;
    }

    if (!ossl_quic_wire_decode_frame_stream(pkt, 1, &frame_data))
        return 0;

    BIO_printf(bio, "    Stream id: %llu\n",
               (unsigned long long)frame_data.stream_id);
    BIO_printf(bio, "    Offset: %llu\n",
               (unsigned long long)frame_data.offset);
    /*
     * It would be nice to find a way of passing the implicit length through
     * to the msg_callback. But this is not currently possible.
     */
    if (frame_data.has_explicit_len)
        BIO_printf(bio, "    Len: %llu\n", (unsigned long long)frame_data.len);
    else
        BIO_puts(bio, "    Len: <implicit length>\n");

    return 1;
}

static int frame_max_data(BIO *bio, PACKET *pkt)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_max_data(pkt, &max_data))
        return 0;

    BIO_printf(bio, "    Max Data: %llu\n", (unsigned long long)max_data);

    return 1;
}

static int frame_max_stream_data(BIO *bio, PACKET *pkt)
{
    uint64_t stream_id = 0;
    uint64_t max_stream_data = 0;

    if (!ossl_quic_wire_decode_frame_max_stream_data(pkt, &stream_id,
                                                     &max_stream_data))
        return 0;

    BIO_printf(bio, "    Max Stream Data: %llu\n",
               (unsigned long long)max_stream_data);

    return 1;
}

static int frame_max_streams(BIO *bio, PACKET *pkt)
{
    uint64_t max_streams = 0;

    if (!ossl_quic_wire_decode_frame_max_streams(pkt, &max_streams))
        return 0;

    BIO_printf(bio, "    Max Streams: %llu\n", (unsigned long long)max_streams);

    return 1;
}

static int frame_data_blocked(BIO *bio, PACKET *pkt)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_data_blocked(pkt, &max_data))
        return 0;

    BIO_printf(bio, "    Max Data: %llu\n", (unsigned long long)max_data);

    return 1;
}

static int frame_stream_data_blocked(BIO *bio, PACKET *pkt)
{
    uint64_t stream_id = 0;
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_stream_data_blocked(pkt, &stream_id,
                                                         &max_data))
        return 0;

    BIO_printf(bio, "    Stream id: %llu\n", (unsigned long long)stream_id);
    BIO_printf(bio, "    Max Data: %llu\n", (unsigned long long)max_data);

    return 1;
}

static int frame_streams_blocked(BIO *bio, PACKET *pkt)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_streams_blocked(pkt, &max_data))
        return 0;

    BIO_printf(bio, "    Max Data: %llu\n", (unsigned long long)max_data);

    return 1;
}

static int frame_new_conn_id(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_NEW_CONN_ID frame_data;

    if (!ossl_quic_wire_decode_frame_new_conn_id(pkt, &frame_data))
        return 0;

    BIO_printf(bio, "    Sequence Number: %llu\n",
               (unsigned long long)frame_data.seq_num);
    BIO_printf(bio, "    Retire prior to: %llu\n",
               (unsigned long long)frame_data.retire_prior_to);
    BIO_puts(bio, "    Connection id: ");
    put_conn_id(bio, &frame_data.conn_id);
    BIO_puts(bio, "\n    Stateless Reset Token: ");
    put_data(bio, frame_data.stateless_reset.token,
             sizeof(frame_data.stateless_reset.token));
    BIO_puts(bio, "\n");

    return 1;
}

static int frame_retire_conn_id(BIO *bio, PACKET *pkt)
{
    uint64_t seq_num;

    if (!ossl_quic_wire_decode_frame_retire_conn_id(pkt, &seq_num))
        return 0;

    BIO_printf(bio, "    Sequence Number: %llu\n", (unsigned long long)seq_num);

    return 1;
}

static int frame_path_challenge(BIO *bio, PACKET *pkt)
{
    uint64_t data = 0;

    if (!ossl_quic_wire_decode_frame_path_challenge(pkt, &data))
        return 0;

    BIO_printf(bio, "    Data: %016llx\n", (unsigned long long)data);

    return 1;
}

static int frame_path_response(BIO *bio, PACKET *pkt)
{
    uint64_t data = 0;

    if (!ossl_quic_wire_decode_frame_path_response(pkt, &data))
        return 0;

    BIO_printf(bio, "    Data: %016llx\n", (unsigned long long)data);

    return 1;
}

static int frame_conn_closed(BIO *bio, PACKET *pkt)
{
    OSSL_QUIC_FRAME_CONN_CLOSE frame_data;

    if (!ossl_quic_wire_decode_frame_conn_close(pkt, &frame_data))
        return 0;

    BIO_printf(bio, "    Error Code: %llu\n",
               (unsigned long long)frame_data.error_code);
    BIO_puts(bio, "    Reason: ");
    put_str(bio, frame_data.reason, frame_data.reason_len);
    BIO_puts(bio, "\n");

    return 1;
}

static int trace_frame_data(BIO *bio, PACKET *pkt)
{
    uint64_t frame_type;

    if (!ossl_quic_wire_peek_frame_header(pkt, &frame_type, NULL))
        return 0;

    switch (frame_type) {
    case OSSL_QUIC_FRAME_TYPE_PING:
        BIO_puts(bio, "Ping\n");
        if (!ossl_quic_wire_decode_frame_ping(pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_PADDING:
        BIO_puts(bio, "Padding\n");
        ossl_quic_wire_decode_padding(pkt);
        break;

    case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
    case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
        BIO_puts(bio, "Ack ");
        if (frame_type == OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN)
            BIO_puts(bio, " (with ECN)\n");
        else
            BIO_puts(bio, " (without ECN)\n");
        if (!frame_ack(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
        BIO_puts(bio, "Reset stream\n");
        if (!frame_reset_stream(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
        BIO_puts(bio, "Stop sending\n");
        if (!frame_stop_sending(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_CRYPTO:
        BIO_puts(bio, "Crypto\n");
        if (!frame_crypto(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
        BIO_puts(bio, "New token\n");
        if (!frame_new_token(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM:
    case OSSL_QUIC_FRAME_TYPE_STREAM_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN_FIN:
        /* frame_stream() prints the frame type string */
        if (!frame_stream(bio, pkt, frame_type))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
        BIO_puts(bio, "Max data\n");
        if (!frame_max_data(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA:
        BIO_puts(bio, "Max stream data\n");
        if (!frame_max_stream_data(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
        BIO_puts(bio, "Max streams ");
        if (frame_type == OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI)
            BIO_puts(bio, " (Bidi)\n");
        else
            BIO_puts(bio, " (Uni)\n");
        if (!frame_max_streams(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED:
        BIO_puts(bio, "Data blocked\n");
        if (!frame_data_blocked(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED:
        BIO_puts(bio, "Stream data blocked\n");
        if (!frame_stream_data_blocked(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI:
    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI:
        BIO_puts(bio, "Streams blocked");
        if (frame_type == OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI)
            BIO_puts(bio, " (Bidi)\n");
        else
            BIO_puts(bio, " (Uni)\n");
        if (!frame_streams_blocked(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
        BIO_puts(bio, "New conn id\n");
        if (!frame_new_conn_id(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
        BIO_puts(bio, "Retire conn id\n");
        if (!frame_retire_conn_id(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE:
        BIO_puts(bio, "Path challenge\n");
        if (!frame_path_challenge(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
        BIO_puts(bio, "Path response\n");
        if (!frame_path_response(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
        BIO_puts(bio, "Connection close");
        if (frame_type == OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP)
            BIO_puts(bio, " (app)\n");
        else
            BIO_puts(bio, " (transport)\n");
        if (!frame_conn_closed(bio, pkt))
            return 0;
        break;

    case OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE:
        BIO_puts(bio, "Handshake done\n");
        if (!ossl_quic_wire_decode_frame_handshake_done(pkt))
            return 0;
        break;

    default:
        return 0;
    }

    if (PACKET_remaining(pkt) != 0)
        BIO_puts(bio, "    <unexpected trailing frame data skipped>\n");

    return 1;
}

int ossl_quic_trace(int write_p, int version, int content_type,
                    const void *buf, size_t msglen, SSL *ssl, void *arg)
{
    BIO *bio = arg;
    PACKET pkt;
    size_t id_len = 0;
    QUIC_CHANNEL *ch;

    switch (content_type) {
    case SSL3_RT_QUIC_DATAGRAM:
        BIO_puts(bio, write_p ? "Sent" : "Received");
        /*
         * Unfortunately there is no way of receiving auxiliary information
         * about the datagram through the msg_callback API such as the peer
         * address
         */
        BIO_printf(bio, " Datagram\n  Length: %zu\n", msglen);
        break;

    case SSL3_RT_QUIC_PACKET:
        {
            QUIC_PKT_HDR hdr;
            size_t i;

            if (!PACKET_buf_init(&pkt, buf, msglen))
                return 0;
            /* Decode the packet header */
            ch = ossl_quic_conn_get_channel(ssl);
            id_len = ossl_quic_channel_get_short_header_conn_id_len(ch);
            if (ossl_quic_wire_decode_pkt_hdr(&pkt, id_len, 0, 1, &hdr, NULL,
                                              NULL) != 1)
                return 0;

            BIO_puts(bio, write_p ? "Sent" : "Received");
            BIO_puts(bio, " Packet\n");
            BIO_printf(bio, "  Packet Type: %s\n", packet_type(hdr.type));
            if (hdr.type != QUIC_PKT_TYPE_1RTT)
                BIO_printf(bio, "  Version: 0x%08lx\n",
                           (unsigned long)hdr.version);
            BIO_puts(bio, "  Destination Conn Id: ");
            put_conn_id(bio, &hdr.dst_conn_id);
            BIO_puts(bio, "\n");
            if (hdr.type != QUIC_PKT_TYPE_1RTT) {
                BIO_puts(bio, "  Source Conn Id: ");
                put_conn_id(bio, &hdr.src_conn_id);
                BIO_puts(bio, "\n");
            }
            BIO_printf(bio, "  Payload length: %zu\n", hdr.len);
            if (hdr.type == QUIC_PKT_TYPE_INITIAL) {
                BIO_puts(bio, "  Token: ");
                put_token(bio, hdr.token, hdr.token_len);
                BIO_puts(bio, "\n");
            }
            if (hdr.type != QUIC_PKT_TYPE_VERSION_NEG
                    && hdr.type != QUIC_PKT_TYPE_RETRY) {
                BIO_puts(bio, "  Packet Number: 0x");
                /* Will always be at least 1 byte */
                for (i = 0; i < hdr.pn_len; i++)
                    BIO_printf(bio, "%02x", hdr.pn[i]);
                BIO_puts(bio, "\n");
            }
            break;
        }

    case SSL3_RT_QUIC_FRAME_PADDING:
    case SSL3_RT_QUIC_FRAME_FULL:
    case SSL3_RT_QUIC_FRAME_HEADER:
        {
            BIO_puts(bio, write_p ? "Sent" : "Received");
            BIO_puts(bio, " Frame: ");

            if (!PACKET_buf_init(&pkt, buf, msglen))
                return 0;
            if (!trace_frame_data(bio, &pkt)) {
                BIO_puts(bio, "  <error processing frame data>\n");
                return 0;
            }
        }
        break;

    default:
        /* Unrecognised content_type. We defer to SSL_trace */
        return 0;
    }

    return 1;
}
