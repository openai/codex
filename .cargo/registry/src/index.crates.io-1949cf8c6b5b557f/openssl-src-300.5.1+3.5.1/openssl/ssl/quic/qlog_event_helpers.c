/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/qlog_event_helpers.h"
#include "internal/common.h"
#include "internal/packet.h"
#include "internal/quic_channel.h"
#include "internal/quic_error.h"

void ossl_qlog_event_connectivity_connection_started(QLOG *qlog,
                                                     const QUIC_CONN_ID *init_dcid)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(qlog, connectivity, connection_started)
        QLOG_STR("protocol", "quic");
        QLOG_CID("dst_cid", init_dcid);
    QLOG_EVENT_END()
#endif
}

#ifndef OPENSSL_NO_QLOG
static const char *map_state_to_qlog(uint32_t state,
                                     int handshake_complete,
                                     int handshake_confirmed)
{
    switch (state) {
    default:
    case QUIC_CHANNEL_STATE_IDLE:
        return NULL;

    case QUIC_CHANNEL_STATE_ACTIVE:
        if (handshake_confirmed)
            return "handshake_confirmed";
        else if (handshake_complete)
            return "handshake_complete";
        else
            return "attempted";

    case QUIC_CHANNEL_STATE_TERMINATING_CLOSING:
        return "closing";

    case QUIC_CHANNEL_STATE_TERMINATING_DRAINING:
        return "draining";

    case QUIC_CHANNEL_STATE_TERMINATED:
        return "closed";
    }
}
#endif

void ossl_qlog_event_connectivity_connection_state_updated(QLOG *qlog,
                                                           uint32_t old_state,
                                                           uint32_t new_state,
                                                           int handshake_complete,
                                                           int handshake_confirmed)
{
#ifndef OPENSSL_NO_QLOG
    const char *state_s;

    QLOG_EVENT_BEGIN(qlog, connectivity, connection_state_updated)
        state_s = map_state_to_qlog(new_state,
                                    handshake_complete,
                                    handshake_confirmed);

        if (state_s != NULL)
            QLOG_STR("state", state_s);
    QLOG_EVENT_END()
#endif
}

#ifndef OPENSSL_NO_QLOG
static const char *quic_err_to_qlog(uint64_t error_code)
{
    switch (error_code) {
    case OSSL_QUIC_ERR_INTERNAL_ERROR:
        return "internal_error";
    case OSSL_QUIC_ERR_CONNECTION_REFUSED:
        return "connection_refused";
    case OSSL_QUIC_ERR_FLOW_CONTROL_ERROR:
        return "flow_control_error";
    case OSSL_QUIC_ERR_STREAM_LIMIT_ERROR:
        return "stream_limit_error";
    case OSSL_QUIC_ERR_STREAM_STATE_ERROR:
        return "stream_state_error";
    case OSSL_QUIC_ERR_FINAL_SIZE_ERROR:
        return "final_size_error";
    case OSSL_QUIC_ERR_FRAME_ENCODING_ERROR:
        return "frame_encoding_error";
    case OSSL_QUIC_ERR_TRANSPORT_PARAMETER_ERROR:
        return "transport_parameter_error";
    case OSSL_QUIC_ERR_CONNECTION_ID_LIMIT_ERROR:
        return "connection_id_limit_error";
    case OSSL_QUIC_ERR_PROTOCOL_VIOLATION:
        return "protocol_violation";
    case OSSL_QUIC_ERR_INVALID_TOKEN:
        return "invalid_token";
    case OSSL_QUIC_ERR_APPLICATION_ERROR:
        return "application_error";
    case OSSL_QUIC_ERR_CRYPTO_BUFFER_EXCEEDED:
        return "crypto_buffer_exceeded";
    case OSSL_QUIC_ERR_KEY_UPDATE_ERROR:
        return "key_update_error";
    case OSSL_QUIC_ERR_AEAD_LIMIT_REACHED:
        return "aead_limit_reached";
    case OSSL_QUIC_ERR_NO_VIABLE_PATH:
        return "no_viable_path";
    default:
        return NULL;
    }
}
#endif

void ossl_qlog_event_connectivity_connection_closed(QLOG *qlog,
                                                    const QUIC_TERMINATE_CAUSE *tcause)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(qlog, connectivity, connection_closed)
        QLOG_STR("owner", tcause->remote ? "remote" : "local");
        if (tcause->app) {
            QLOG_U64("application_code", tcause->error_code);
        } else {
            const char *m = quic_err_to_qlog(tcause->error_code);
            char ce[32];

            if (tcause->error_code >= OSSL_QUIC_ERR_CRYPTO_ERR_BEGIN
                && tcause->error_code <= OSSL_QUIC_ERR_CRYPTO_ERR_END) {
                BIO_snprintf(ce, sizeof(ce), "crypto_error_0x%03llx",
                             (unsigned long long)tcause->error_code);
                m = ce;
            }
            /* TODO(QLOG FUTURE): Consider adding ERR information in the output. */

            if (m != NULL)
                QLOG_STR("connection_code", m);
            else
                QLOG_U64("connection_code", tcause->error_code);
        }

        QLOG_STR_LEN("reason", tcause->reason, tcause->reason_len);
    QLOG_EVENT_END()
#endif
}

#ifndef OPENSSL_NO_QLOG
static const char *quic_pkt_type_to_qlog(uint32_t pkt_type)
{
    switch (pkt_type) {
    case QUIC_PKT_TYPE_INITIAL:
        return "initial";
    case QUIC_PKT_TYPE_HANDSHAKE:
        return "handshake";
    case QUIC_PKT_TYPE_0RTT:
        return "0RTT";
    case QUIC_PKT_TYPE_1RTT:
        return "1RTT";
    case QUIC_PKT_TYPE_VERSION_NEG:
        return "version_negotiation";
    case QUIC_PKT_TYPE_RETRY:
        return "retry";
    default:
        return "unknown";
    }
}
#endif

void ossl_qlog_event_recovery_packet_lost(QLOG *qlog,
                                          const QUIC_TXPIM_PKT *tpkt)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(qlog, recovery, packet_lost)
        QLOG_BEGIN("header")
            QLOG_STR("packet_type", quic_pkt_type_to_qlog(tpkt->pkt_type));
            if (ossl_quic_pkt_type_has_pn(tpkt->pkt_type))
                QLOG_U64("packet_number", tpkt->ackm_pkt.pkt_num);
        QLOG_END()
    QLOG_EVENT_END()
#endif
}

#ifndef OPENSSL_NO_QLOG
# define MAX_ACK_RANGES 32

static void ignore_res(int x) {}

/*
 * For logging received packets, we need to parse all the frames in the packet
 * to log them. We should do this separately to the RXDP code because we want to
 * log the packet and its contents before we start to actually process it in
 * case it causes an error. We also in general don't want to do other
 * non-logging related work in the middle of an event logging transaction.
 * Reparsing packet data allows us to meet these needs while avoiding the need
 * to keep around bookkeeping data on what frames were in a packet, etc.
 *
 * For logging transmitted packets, we actually reuse the same code and reparse
 * the outgoing packet's payload. This again has the advantage that we only log
 * a packet when it is actually queued for transmission (and not if something
 * goes wrong before then) while avoiding the need to keep around bookkeeping
 * data on what frames it contained.
 */
static int log_frame_actual(QLOG *qlog_instance, PACKET *pkt,
                            size_t *need_skip)
{
    uint64_t frame_type;
    OSSL_QUIC_FRAME_ACK ack;
    OSSL_QUIC_ACK_RANGE ack_ranges[MAX_ACK_RANGES];
    uint64_t num_ranges, total_ranges;
    size_t i;
    PACKET orig_pkt = *pkt;

    if (!ossl_quic_wire_peek_frame_header(pkt, &frame_type, NULL)) {
        *need_skip = SIZE_MAX;
        return 0;
    }

    /*
     * If something goes wrong decoding a frame we cannot log it as that frame
     * as we need to know how to decode it in order to be able to do so, but in
     * that case we log it as an unknown frame to assist with diagnosis.
     */
    switch (frame_type) {
    case OSSL_QUIC_FRAME_TYPE_PADDING:
        QLOG_STR("frame_type", "padding");
        QLOG_U64("payload_length",
                 ossl_quic_wire_decode_padding(pkt));
        break;
    case OSSL_QUIC_FRAME_TYPE_PING:
        if (!ossl_quic_wire_decode_frame_ping(pkt))
            goto unknown;

        QLOG_STR("frame_type", "ping");
        break;
    case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
    case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
        if (!ossl_quic_wire_peek_frame_ack_num_ranges(pkt, &num_ranges))
            goto unknown;

        ack.ack_ranges      = ack_ranges;
        ack.num_ack_ranges  = OSSL_NELEM(ack_ranges);
        if (!ossl_quic_wire_decode_frame_ack(pkt, 3, &ack, &total_ranges))
            goto unknown;

        QLOG_STR("frame_type", "ack");
        QLOG_U64("ack_delay", ossl_time2ms(ack.delay_time));
        if (ack.ecn_present) {
            QLOG_U64("ect1", ack.ect0);
            QLOG_U64("ect0", ack.ect1);
            QLOG_U64("ce", ack.ecnce);
        }
        QLOG_BEGIN_ARRAY("acked_ranges");
        for (i = 0; i < ack.num_ack_ranges; ++i) {
            QLOG_BEGIN_ARRAY(NULL)
                QLOG_U64(NULL, ack.ack_ranges[i].start);
                if (ack.ack_ranges[i].end != ack.ack_ranges[i].start)
                    QLOG_U64(NULL, ack.ack_ranges[i].end);
            QLOG_END_ARRAY()
        }
        QLOG_END_ARRAY()
        break;
    case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
        {
            OSSL_QUIC_FRAME_RESET_STREAM f;

            if (!ossl_quic_wire_decode_frame_reset_stream(pkt, &f))
                goto unknown;

            QLOG_STR("frame_type", "reset_stream");
            QLOG_U64("stream_id", f.stream_id);
            QLOG_U64("error_code", f.app_error_code);
            QLOG_U64("final_size", f.final_size);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
        {
            OSSL_QUIC_FRAME_STOP_SENDING f;

            if (!ossl_quic_wire_decode_frame_stop_sending(pkt, &f))
                goto unknown;

            QLOG_STR("frame_type", "stop_sending");
            QLOG_U64("stream_id", f.stream_id);
            QLOG_U64("error_code", f.app_error_code);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_CRYPTO:
        {
            OSSL_QUIC_FRAME_CRYPTO f;

            if (!ossl_quic_wire_decode_frame_crypto(pkt, 1, &f))
                goto unknown;

            QLOG_STR("frame_type", "crypto");
            QLOG_U64("offset", f.offset);
            QLOG_U64("payload_length", f.len);
            *need_skip += (size_t)f.len;
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_STREAM:
    case OSSL_QUIC_FRAME_TYPE_STREAM_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_LEN_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_FIN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN:
    case OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN_FIN:
        {
            OSSL_QUIC_FRAME_STREAM f;

            if (!ossl_quic_wire_decode_frame_stream(pkt, 1, &f))
                goto unknown;

            QLOG_STR("frame_type", "stream");
            QLOG_U64("stream_id", f.stream_id);
            QLOG_U64("offset", f.offset);
            QLOG_U64("payload_length", f.len);
            QLOG_BOOL("explicit_length", f.has_explicit_len);
            if (f.is_fin)
                QLOG_BOOL("fin", 1);
            *need_skip = f.has_explicit_len
                ? *need_skip + (size_t)f.len : SIZE_MAX;
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
        {
            uint64_t x;

            if (!ossl_quic_wire_decode_frame_max_data(pkt, &x))
                goto unknown;

            QLOG_STR("frame_type", "max_data");
            QLOG_U64("maximum", x);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
        {
            uint64_t x;

            if (!ossl_quic_wire_decode_frame_max_streams(pkt, &x))
                goto unknown;

            QLOG_STR("frame_type", "max_streams");
            QLOG_STR("stream_type",
                     frame_type == OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI
                        ? "bidirectional" : "unidirectional");
            QLOG_U64("maximum", x);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA:
        {
            uint64_t stream_id, max_data;

            if (!ossl_quic_wire_decode_frame_max_stream_data(pkt, &stream_id,
                                                             &max_data))
                goto unknown;

            QLOG_STR("frame_type", "max_stream_data");
            QLOG_U64("stream_id", stream_id);
            QLOG_U64("maximum", max_data);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE:
        {
            uint64_t challenge;

            if (!ossl_quic_wire_decode_frame_path_challenge(pkt, &challenge))
                goto unknown;

            QLOG_STR("frame_type", "path_challenge");
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
        {
            uint64_t challenge;

            if (!ossl_quic_wire_decode_frame_path_response(pkt, &challenge))
                goto unknown;

            QLOG_STR("frame_type", "path_response");
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
        {
            OSSL_QUIC_FRAME_CONN_CLOSE f;

            if (!ossl_quic_wire_decode_frame_conn_close(pkt, &f))
                goto unknown;

            QLOG_STR("frame_type", "connection_close");
            QLOG_STR("error_space", f.is_app ? "application" : "transport");
            QLOG_U64("error_code_value", f.error_code);
            if (f.is_app)
                QLOG_U64("error_code", f.error_code);
            if (!f.is_app && f.frame_type != 0)
                QLOG_U64("trigger_frame_type", f.frame_type);
            QLOG_STR_LEN("reason", f.reason, f.reason_len);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE:
        {
            if (!ossl_quic_wire_decode_frame_handshake_done(pkt))
                goto unknown;

            QLOG_STR("frame_type", "handshake_done");
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
        {
            OSSL_QUIC_FRAME_NEW_CONN_ID f;

            if (!ossl_quic_wire_decode_frame_new_conn_id(pkt, &f))
                goto unknown;

            QLOG_STR("frame_type", "new_connection_id");
            QLOG_U64("sequence_number", f.seq_num);
            QLOG_U64("retire_prior_to", f.retire_prior_to);
            QLOG_CID("connection_id", &f.conn_id);
            QLOG_BIN("stateless_reset_token",
                     f.stateless_reset.token,
                     sizeof(f.stateless_reset.token));
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
        {
            uint64_t seq_num;

            if (!ossl_quic_wire_decode_frame_retire_conn_id(pkt, &seq_num))
                goto unknown;

            QLOG_STR("frame_type", "retire_connection_id");
            QLOG_U64("sequence_number", seq_num);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED:
        {
            uint64_t x;

            if (!ossl_quic_wire_decode_frame_data_blocked(pkt, &x))
                goto unknown;

            QLOG_STR("frame_type", "data_blocked");
            QLOG_U64("limit", x);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED:
        {
            uint64_t stream_id, x;

            if (!ossl_quic_wire_decode_frame_stream_data_blocked(pkt,
                                                                 &stream_id,
                                                                 &x))
                goto unknown;

            QLOG_STR("frame_type", "stream_data_blocked");
            QLOG_U64("stream_id", stream_id);
            QLOG_U64("limit", x);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI:
    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI:
        {
            uint64_t x;

            if (!ossl_quic_wire_decode_frame_streams_blocked(pkt, &x))
                goto unknown;

            QLOG_STR("frame_type", "streams_blocked");
            QLOG_STR("stream_type",
                     frame_type == OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI
                        ? "bidirectional" : "unidirectional");
            QLOG_U64("limit", x);
        }
        break;
    case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
        {
            const unsigned char *token;
            size_t token_len;

            if (!ossl_quic_wire_decode_frame_new_token(pkt, &token, &token_len))
                goto unknown;

            QLOG_STR("frame_type", "new_token");
            QLOG_BEGIN("token");
                QLOG_BEGIN("raw");
                    QLOG_BIN("data", token, token_len);
                QLOG_END();
            QLOG_END();
        }
        break;
    default:
unknown:
        QLOG_STR("frame_type", "unknown");
        QLOG_U64("frame_type_value", frame_type);

        /*
         * Can't continue scanning for frames in this case as the frame length
         * is unknown. We log the entire body of the rest of the packet payload
         * as the raw data of the frame.
         */
        QLOG_BEGIN("raw");
            QLOG_BIN("data", PACKET_data(&orig_pkt),
                     PACKET_remaining(&orig_pkt));
        QLOG_END();
        ignore_res(PACKET_forward(pkt, PACKET_remaining(pkt)));
        break;
    }

    return 1;
}

static void log_frame(QLOG *qlog_instance, PACKET *pkt,
                      size_t *need_skip)
{
    size_t rem_before, rem_after;

    rem_before = PACKET_remaining(pkt);

    if (!log_frame_actual(qlog_instance, pkt, need_skip))
        return;

    rem_after = PACKET_remaining(pkt);
    QLOG_U64("length", rem_before - rem_after);
}

static int log_frames(QLOG *qlog_instance,
                      const OSSL_QTX_IOVEC *iovec,
                      size_t num_iovec)
{
    size_t i;
    PACKET pkt;
    size_t need_skip = 0;

    for (i = 0; i < num_iovec; ++i) {
        if (!PACKET_buf_init(&pkt, iovec[i].buf, iovec[i].buf_len))
            return 0;

        while (PACKET_remaining(&pkt) > 0) {
            if (need_skip > 0) {
                size_t adv = need_skip;

                if (adv > PACKET_remaining(&pkt))
                    adv = PACKET_remaining(&pkt);

                if (!PACKET_forward(&pkt, adv))
                    return 0;

                need_skip -= adv;
                continue;
            }

            QLOG_BEGIN(NULL)
            {
                log_frame(qlog_instance, &pkt, &need_skip);
            }
            QLOG_END()
        }
    }

    return 1;
}

static void log_packet(QLOG *qlog_instance,
                       const QUIC_PKT_HDR *hdr,
                       QUIC_PN pn,
                       const OSSL_QTX_IOVEC *iovec,
                       size_t num_iovec,
                       uint64_t datagram_id)
{
    const char *type_s;

    QLOG_BEGIN("header")
        type_s = quic_pkt_type_to_qlog(hdr->type);
        if (type_s == NULL)
            type_s = "unknown";

        QLOG_STR("packet_type", type_s);
        if (ossl_quic_pkt_type_has_pn(hdr->type))
            QLOG_U64("packet_number", pn);

        QLOG_CID("dcid", &hdr->dst_conn_id);
        if (ossl_quic_pkt_type_has_scid(hdr->type))
            QLOG_CID("scid", &hdr->src_conn_id);

        if (hdr->token_len > 0) {
            QLOG_BEGIN("token")
                QLOG_BEGIN("raw")
                    QLOG_BIN("data", hdr->token, hdr->token_len);
                QLOG_END()
            QLOG_END()
        }
        /* TODO(QLOG FUTURE): flags, length */
    QLOG_END()
    QLOG_U64("datagram_id", datagram_id);

    if (ossl_quic_pkt_type_is_encrypted(hdr->type)) {
        QLOG_BEGIN_ARRAY("frames")
            log_frames(qlog_instance, iovec, num_iovec);
        QLOG_END_ARRAY()
    }
}

#endif

void ossl_qlog_event_transport_packet_sent(QLOG *qlog,
                                           const QUIC_PKT_HDR *hdr,
                                           QUIC_PN pn,
                                           const OSSL_QTX_IOVEC *iovec,
                                           size_t num_iovec,
                                           uint64_t datagram_id)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(qlog, transport, packet_sent)
        log_packet(qlog, hdr, pn, iovec, num_iovec, datagram_id);
    QLOG_EVENT_END()
#endif
}

void ossl_qlog_event_transport_packet_received(QLOG *qlog,
                                               const QUIC_PKT_HDR *hdr,
                                               QUIC_PN pn,
                                               const OSSL_QTX_IOVEC *iovec,
                                               size_t num_iovec,
                                               uint64_t datagram_id)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(qlog, transport, packet_received)
        log_packet(qlog, hdr, pn, iovec, num_iovec, datagram_id);
    QLOG_EVENT_END()
#endif
}
