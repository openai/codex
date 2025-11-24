/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet_quic.h"
#include "internal/nelem.h"
#include "internal/quic_wire.h"
#include "internal/quic_record_rx.h"
#include "internal/quic_ackm.h"
#include "internal/quic_rx_depack.h"
#include "internal/quic_error.h"
#include "internal/quic_fc.h"
#include "internal/quic_channel.h"
#include "internal/sockets.h"

#include "quic_local.h"
#include "quic_channel_local.h"
#include "../ssl_local.h"

/*
 * Helper functions to process different frame types.
 *
 * Typically, those that are ACK eliciting will take an OSSL_ACKM_RX_PKT
 * pointer argument, the few that aren't ACK eliciting will not.  This makes
 * them a verifiable pattern against tables where this is specified.
 */
static int depack_do_implicit_stream_create(QUIC_CHANNEL *ch,
                                            uint64_t stream_id,
                                            uint64_t frame_type,
                                            QUIC_STREAM **result);

static int depack_do_frame_padding(PACKET *pkt)
{
    /* We ignore this frame */
    ossl_quic_wire_decode_padding(pkt);
    return 1;
}

static int depack_do_frame_ping(PACKET *pkt, QUIC_CHANNEL *ch,
                                uint32_t enc_level,
                                OSSL_ACKM_RX_PKT *ackm_data)
{
    /* We ignore this frame, apart from eliciting an ACK */
    if (!ossl_quic_wire_decode_frame_ping(pkt)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_PING,
                                               "decode error");
        return 0;
    }

    ossl_quic_tx_packetiser_schedule_ack_eliciting(ch->txp, enc_level);
    return 1;
}

static int depack_do_frame_ack(PACKET *pkt, QUIC_CHANNEL *ch,
                               int packet_space, OSSL_TIME received,
                               uint64_t frame_type,
                               OSSL_QRX_PKT *qpacket)
{
    OSSL_QUIC_FRAME_ACK ack;
    OSSL_QUIC_ACK_RANGE *p;
    uint64_t total_ranges = 0;
    uint32_t ack_delay_exp = ch->rx_ack_delay_exp;

    if (!ossl_quic_wire_peek_frame_ack_num_ranges(pkt, &total_ranges)
        /* In case sizeof(uint64_t) > sizeof(size_t) */
        || total_ranges > SIZE_MAX / sizeof(OSSL_QUIC_ACK_RANGE))
        goto malformed;

    if (ch->num_ack_range_scratch < (size_t)total_ranges) {
        if ((p = OPENSSL_realloc(ch->ack_range_scratch,
                                 sizeof(OSSL_QUIC_ACK_RANGE)
                                 * (size_t)total_ranges)) == NULL)
            goto malformed;

        ch->ack_range_scratch       = p;
        ch->num_ack_range_scratch   = (size_t)total_ranges;
    }

    ack.ack_ranges = ch->ack_range_scratch;
    ack.num_ack_ranges = (size_t)total_ranges;

    if (!ossl_quic_wire_decode_frame_ack(pkt, ack_delay_exp, &ack, NULL))
        goto malformed;

    if (qpacket->hdr->type == QUIC_PKT_TYPE_1RTT
        && (qpacket->key_epoch < ossl_qrx_get_key_epoch(ch->qrx)
            || ch->rxku_expected)
        && ack.ack_ranges[0].end >= ch->txku_pn) {
        /*
         * RFC 9001 s. 6.2: An endpoint that receives an acknowledgment that is
         * carried in a packet protected with old keys where any acknowledged
         * packet was protected with newer keys MAY treat that as a connection
         * error of type KEY_UPDATE_ERROR.
         *
         * Two cases to handle here:
         *
         *   - We did spontaneous TXKU, the peer has responded in kind and we
         *     have detected RXKU; !ch->rxku_expected, but then it sent a packet
         *     with old keys acknowledging a packet in the new key epoch.
         *
         *     This also covers the case where we got RXKU and triggered
         *     solicited TXKU, and then for some reason the peer sent an ACK of
         *     a PN in our new TX key epoch with old keys.
         *
         *   - We did spontaneous TXKU; ch->txku_pn is the starting PN of our
         *     new TX key epoch; the peer has not initiated a solicited TXKU in
         *     response (so we have not detected RXKU); in this case the RX key
         *     epoch has not incremented and ch->rxku_expected is still 1.
         */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_KEY_UPDATE_ERROR,
                                               frame_type,
                                               "acked packet which initiated a "
                                               "key update without a "
                                               "corresponding key update");
        return 0;
    }

    if (!ossl_ackm_on_rx_ack_frame(ch->ackm, &ack,
                                   packet_space, received))
        goto malformed;

    ++ch->diag_num_rx_ack;
    return 1;

malformed:
    ossl_quic_channel_raise_protocol_error(ch,
                                           OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                           frame_type,
                                           "decode error");
    return 0;
}

static int depack_do_frame_reset_stream(PACKET *pkt,
                                        QUIC_CHANNEL *ch,
                                        OSSL_ACKM_RX_PKT *ackm_data)
{
    OSSL_QUIC_FRAME_RESET_STREAM frame_data;
    QUIC_STREAM *stream = NULL;
    uint64_t fce;

    if (!ossl_quic_wire_decode_frame_reset_stream(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                               "decode error");
        return 0;
    }

    if (!depack_do_implicit_stream_create(ch, frame_data.stream_id,
                                          OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                          &stream))
        return 0; /* error already raised for us */

    if (stream == NULL)
        return 1; /* old deleted stream, not a protocol violation, ignore */

    if (!ossl_quic_stream_has_recv(stream)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                               "RESET_STREAM frame for "
                                               "TX only stream");
        return 0;
    }

    /*
     * The final size field of the RESET_STREAM frame must be used to determine
     * how much flow control credit the aborted stream was considered to have
     * consumed.
     *
     * We also need to ensure that if we already have a final size for the
     * stream, the RESET_STREAM frame's Final Size field matches this; we SHOULD
     * terminate the connection otherwise (RFC 9000 s. 4.5). The RXFC takes care
     * of this for us.
     */
    if (!ossl_quic_rxfc_on_rx_stream_frame(&stream->rxfc,
                                           frame_data.final_size, /*is_fin=*/1)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                               "internal error (flow control)");
        return 0;
    }

    /* Has a flow control error occurred? */
    fce = ossl_quic_rxfc_get_error(&stream->rxfc, 0);
    if (fce != OSSL_QUIC_ERR_NO_ERROR) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               fce,
                                               OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                               "flow control violation");
        return 0;
    }

    /*
     * Depending on the receive part state this is handled either as a reset
     * transition or a no-op (e.g. if a reset has already been received before,
     * or the application already retired a FIN). Best effort - there are no
     * protocol error conditions we need to check for here.
     */
    ossl_quic_stream_map_notify_reset_recv_part(&ch->qsm, stream,
                                                frame_data.app_error_code,
                                                frame_data.final_size);

    ossl_quic_stream_map_update_state(&ch->qsm, stream);
    return 1;
}

static int depack_do_frame_stop_sending(PACKET *pkt,
                                        QUIC_CHANNEL *ch,
                                        OSSL_ACKM_RX_PKT *ackm_data)
{
    OSSL_QUIC_FRAME_STOP_SENDING frame_data;
    QUIC_STREAM *stream = NULL;

    if (!ossl_quic_wire_decode_frame_stop_sending(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                                               "decode error");
        return 0;
    }

    if (!depack_do_implicit_stream_create(ch, frame_data.stream_id,
                                          OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                                          &stream))
        return 0; /* error already raised for us */

    if (stream == NULL)
        return 1; /* old deleted stream, not a protocol violation, ignore */

    if (!ossl_quic_stream_has_send(stream)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                                               "STOP_SENDING frame for "
                                               "RX only stream");
        return 0;
    }

    stream->peer_stop_sending       = 1;
    stream->peer_stop_sending_aec   = frame_data.app_error_code;

    /*
     * RFC 9000 s. 3.5: Receiving a STOP_SENDING frame means we must respond in
     * turn with a RESET_STREAM frame for the same part of the stream. The other
     * part is unaffected.
     */
    ossl_quic_stream_map_reset_stream_send_part(&ch->qsm, stream,
                                                frame_data.app_error_code);
    return 1;
}

static int depack_do_frame_crypto(PACKET *pkt, QUIC_CHANNEL *ch,
                                  OSSL_QRX_PKT *parent_pkt,
                                  OSSL_ACKM_RX_PKT *ackm_data,
                                  uint64_t *datalen)
{
    OSSL_QUIC_FRAME_CRYPTO f;
    QUIC_RSTREAM *rstream;
    QUIC_RXFC *rxfc;

    *datalen = 0;

    if (!ossl_quic_wire_decode_frame_crypto(pkt, 0, &f)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                               "decode error");
        return 0;
    }

    if (f.len == 0)
        return 1; /* nothing to do */

    rstream = ch->crypto_recv[ackm_data->pkt_space];
    if (!ossl_assert(rstream != NULL))
        /*
         * This should not happen; we should only have a NULL stream here if
         * the EL has been discarded, and if the EL has been discarded we
         * shouldn't be here.
         */
        return 0;

    rxfc = &ch->crypto_rxfc[ackm_data->pkt_space];

    if (!ossl_quic_rxfc_on_rx_stream_frame(rxfc, f.offset + f.len,
                                           /*is_fin=*/0)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                               "internal error (crypto RXFC)");
        return 0;
    }

    if (ossl_quic_rxfc_get_error(rxfc, 0) != OSSL_QUIC_ERR_NO_ERROR) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_CRYPTO_BUFFER_EXCEEDED,
                                               OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                               "exceeded maximum crypto buffer");
        return 0;
    }

    if (!ossl_quic_rstream_queue_data(rstream, parent_pkt,
                                      f.offset, f.data, f.len, 0)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                               "internal error (rstream queue)");
        return 0;
    }

    ch->did_crypto_frame = 1;
    *datalen = f.len;

    return 1;
}

static int depack_do_frame_new_token(PACKET *pkt, QUIC_CHANNEL *ch,
                                     OSSL_ACKM_RX_PKT *ackm_data)
{
    const uint8_t *token;
    size_t token_len;

    if (!ossl_quic_wire_decode_frame_new_token(pkt, &token, &token_len)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_NEW_TOKEN,
                                               "decode error");
        return 0;
    }

    if (token_len == 0) {
        /*
         * RFC 9000 s. 19.7: "A client MUST treat receipt of a NEW_TOKEN frame
         * with an empty Token field as a connection error of type
         * FRAME_ENCODING_ERROR."
         */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_NEW_TOKEN,
                                               "zero-length NEW_TOKEN");
        return 0;
    }

    /* store the new token in our token cache */
    if (!ossl_quic_set_peer_token(ossl_quic_port_get_channel_ctx(ch->port),
                                  &ch->cur_peer_addr, token, token_len))
        return 0;

    return 1;
}

/*
 * Returns 1 if no protocol violation has occurred. In this case *result will be
 * non-NULL unless this is an old deleted stream and we should ignore the frame
 * causing this function to be called. Returns 0 on protocol violation.
 */
static int depack_do_implicit_stream_create(QUIC_CHANNEL *ch,
                                            uint64_t stream_id,
                                            uint64_t frame_type,
                                            QUIC_STREAM **result)
{
    QUIC_STREAM *stream;
    uint64_t peer_role, stream_ordinal;
    uint64_t *p_next_ordinal_local, *p_next_ordinal_remote;
    QUIC_RXFC *max_streams_fc;
    int is_uni, is_remote_init;

    stream = ossl_quic_stream_map_get_by_id(&ch->qsm, stream_id);
    if (stream != NULL) {
        *result = stream;
        return 1;
    }

    /*
     * If we do not yet have a stream with the given ID, there are three
     * possibilities:
     *
     *   (a) The stream ID is for a remotely-created stream and the peer
     *       is creating a stream.
     *
     *   (b) The stream ID is for a locally-created stream which has
     *       previously been deleted.
     *
     *   (c) The stream ID is for a locally-created stream which does
     *       not exist yet. This is a protocol violation and we must
     *       terminate the connection in this case.
     *
     * We distinguish between (b) and (c) using the stream ID allocator
     * variable. Since stream ordinals are allocated monotonically, we
     * simply determine if the stream ordinal is in the future.
     */
    peer_role = ch->is_server
        ? QUIC_STREAM_INITIATOR_CLIENT
        : QUIC_STREAM_INITIATOR_SERVER;

    is_remote_init = ((stream_id & QUIC_STREAM_INITIATOR_MASK) == peer_role);
    is_uni = ((stream_id & QUIC_STREAM_DIR_MASK) == QUIC_STREAM_DIR_UNI);

    stream_ordinal = stream_id >> 2;

    if (is_remote_init) {
        /*
         * Peer-created stream which does not yet exist. Create it. QUIC stream
         * ordinals within a given stream type MUST be used in sequence and
         * receiving a STREAM frame for ordinal n must implicitly create streams
         * with ordinals [0, n) within that stream type even if no explicit
         * STREAM frames are received for those ordinals.
         */
        p_next_ordinal_remote = is_uni
            ? &ch->next_remote_stream_ordinal_uni
            : &ch->next_remote_stream_ordinal_bidi;

        /* Check this isn't violating stream count flow control. */
        max_streams_fc = is_uni
            ? &ch->max_streams_uni_rxfc
            : &ch->max_streams_bidi_rxfc;

        if (!ossl_quic_rxfc_on_rx_stream_frame(max_streams_fc,
                                               stream_ordinal + 1,
                                               /*is_fin=*/0)) {
            ossl_quic_channel_raise_protocol_error(ch,
                                                   OSSL_QUIC_ERR_INTERNAL_ERROR,
                                                   frame_type,
                                                   "internal error (stream count RXFC)");
            return 0;
        }

        if (ossl_quic_rxfc_get_error(max_streams_fc, 0) != OSSL_QUIC_ERR_NO_ERROR) {
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,
                                                   frame_type,
                                                   "exceeded maximum allowed streams");
            return 0;
        }

        /*
         * Create the named stream and any streams coming before it yet to be
         * created.
         */
        while (*p_next_ordinal_remote <= stream_ordinal) {
            uint64_t cur_stream_id = (*p_next_ordinal_remote << 2) |
                (stream_id
                 & (QUIC_STREAM_DIR_MASK | QUIC_STREAM_INITIATOR_MASK));

            stream = ossl_quic_channel_new_stream_remote(ch, cur_stream_id);
            if (stream == NULL) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_INTERNAL_ERROR,
                                                       frame_type,
                                                       "internal error (stream allocation)");
                return 0;
            }

            ++*p_next_ordinal_remote;
        }

        *result = stream;
    } else {
        /* Locally-created stream which does not yet exist. */
        p_next_ordinal_local = is_uni
            ? &ch->next_local_stream_ordinal_uni
            : &ch->next_local_stream_ordinal_bidi;

        if (stream_ordinal >= *p_next_ordinal_local) {
            /*
             * We never created this stream yet, this is a protocol
             * violation.
             */
            ossl_quic_channel_raise_protocol_error(ch,
                                                   OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                                   frame_type,
                                                   "STREAM frame for nonexistent "
                                                   "stream");
            return 0;
        }

        /*
         * Otherwise this is for an old locally-initiated stream which we
         * have subsequently deleted. Ignore the data; it may simply be a
         * retransmission. We already take care of notifying the peer of the
         * termination of the stream during the stream deletion lifecycle.
         */
        *result = NULL;
    }

    return 1;
}

static int depack_do_frame_stream(PACKET *pkt, QUIC_CHANNEL *ch,
                                  OSSL_QRX_PKT *parent_pkt,
                                  OSSL_ACKM_RX_PKT *ackm_data,
                                  uint64_t frame_type,
                                  uint64_t *datalen)
{
    OSSL_QUIC_FRAME_STREAM frame_data;
    QUIC_STREAM *stream;
    uint64_t fce;
    size_t rs_avail;
    int rs_fin = 0;

    *datalen = 0;

    if (!ossl_quic_wire_decode_frame_stream(pkt, 0, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "decode error");
        return 0;
    }

    if (!depack_do_implicit_stream_create(ch, frame_data.stream_id,
                                          frame_type, &stream))
        return 0; /* protocol error raised by above call */

    if (stream == NULL)
        /*
         * Data for old stream which is not a protocol violation but should be
         * ignored, so stop here.
         */
        return 1;

    if (!ossl_quic_stream_has_recv(stream)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                               frame_type,
                                               "STREAM frame for TX only "
                                               "stream");
        return 0;
    }

    /* Notify stream flow controller. */
    if (!ossl_quic_rxfc_on_rx_stream_frame(&stream->rxfc,
                                           frame_data.offset + frame_data.len,
                                           frame_data.is_fin)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               frame_type,
                                               "internal error (flow control)");
        return 0;
    }

    /* Has a flow control error occurred? */
    fce = ossl_quic_rxfc_get_error(&stream->rxfc, 0);
    if (fce != OSSL_QUIC_ERR_NO_ERROR) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               fce,
                                               frame_type,
                                               "flow control violation");
        return 0;
    }

    switch (stream->recv_state) {
    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
        /*
         * It only makes sense to process incoming STREAM frames in these
         * states.
         */
        break;

    case QUIC_RSTREAM_STATE_DATA_RECVD:
    case QUIC_RSTREAM_STATE_DATA_READ:
    case QUIC_RSTREAM_STATE_RESET_RECVD:
    case QUIC_RSTREAM_STATE_RESET_READ:
    default:
        /*
         * We have no use for STREAM frames once the receive part reaches any of
         * these states, so just ignore.
         */
        return 1;
    }

    /* If we are in RECV, auto-transition to SIZE_KNOWN on FIN. */
    if (frame_data.is_fin
        && !ossl_quic_stream_recv_get_final_size(stream, NULL)) {

        /* State was already checked above, so can't fail. */
        ossl_quic_stream_map_notify_size_known_recv_part(&ch->qsm, stream,
                                                         frame_data.offset
                                                         + frame_data.len);
    }

    /*
     * If we requested STOP_SENDING do not bother buffering the data. Note that
     * this must happen after RXFC checks above as even if we sent STOP_SENDING
     * we must still enforce correct flow control (RFC 9000 s. 3.5).
     */
    if (stream->stop_sending)
        return 1; /* not an error - packet reordering, etc. */

    /*
     * The receive stream buffer may or may not choose to consume the data
     * without copying by reffing the OSSL_QRX_PKT. In this case
     * ossl_qrx_pkt_release() will be eventually called when the data is no
     * longer needed.
     *
     * It is OK for the peer to send us a zero-length non-FIN STREAM frame,
     * which is a no-op, aside from the fact that it ensures the stream exists.
     * In this case we have nothing to report to the receive buffer.
     */
    if ((frame_data.len > 0 || frame_data.is_fin)
        && !ossl_quic_rstream_queue_data(stream->rstream, parent_pkt,
                                      frame_data.offset,
                                      frame_data.data,
                                      frame_data.len,
                                      frame_data.is_fin)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               frame_type,
                                               "internal error (rstream queue)");
        return 0;
    }

    /*
     * rs_fin will be 1 only if we can read all data up to and including the FIN
     * without any gaps before it; this implies we have received all data. Avoid
     * calling ossl_quic_rstream_available() where it is not necessary as it is
     * more expensive.
     */
    if (stream->recv_state == QUIC_RSTREAM_STATE_SIZE_KNOWN
        && !ossl_quic_rstream_available(stream->rstream, &rs_avail, &rs_fin)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               frame_type,
                                               "internal error (rstream available)");
        return 0;
    }

    if (rs_fin)
        ossl_quic_stream_map_notify_totally_received(&ch->qsm, stream);

    *datalen = frame_data.len;

    return 1;
}

static void update_streams(QUIC_STREAM *s, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    ossl_quic_stream_map_update_state(&ch->qsm, s);
}

static void update_streams_bidi(QUIC_STREAM *s, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    if (!ossl_quic_stream_is_bidi(s))
        return;

    ossl_quic_stream_map_update_state(&ch->qsm, s);
}

static void update_streams_uni(QUIC_STREAM *s, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    if (ossl_quic_stream_is_bidi(s))
        return;

    ossl_quic_stream_map_update_state(&ch->qsm, s);
}

static int depack_do_frame_max_data(PACKET *pkt, QUIC_CHANNEL *ch,
                                    OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_max_data(pkt, &max_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_MAX_DATA,
                                               "decode error");
        return 0;
    }

    ossl_quic_txfc_bump_cwm(&ch->conn_txfc, max_data);
    ossl_quic_stream_map_visit(&ch->qsm, update_streams, ch);
    return 1;
}

static int depack_do_frame_max_stream_data(PACKET *pkt,
                                           QUIC_CHANNEL *ch,
                                           OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t stream_id = 0;
    uint64_t max_stream_data = 0;
    QUIC_STREAM *stream;

    if (!ossl_quic_wire_decode_frame_max_stream_data(pkt, &stream_id,
                                                     &max_stream_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA,
                                               "decode error");
        return 0;
    }

    if (!depack_do_implicit_stream_create(ch, stream_id,
                                          OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA,
                                          &stream))
        return 0; /* error already raised for us */

    if (stream == NULL)
        return 1; /* old deleted stream, not a protocol violation, ignore */

    if (!ossl_quic_stream_has_send(stream)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA,
                                               "MAX_STREAM_DATA for TX only "
                                               "stream");
        return 0;
    }

    ossl_quic_txfc_bump_cwm(&stream->txfc, max_stream_data);
    ossl_quic_stream_map_update_state(&ch->qsm, stream);
    return 1;
}

static int depack_do_frame_max_streams(PACKET *pkt,
                                       QUIC_CHANNEL *ch,
                                       OSSL_ACKM_RX_PKT *ackm_data,
                                       uint64_t frame_type)
{
    uint64_t max_streams = 0;

    if (!ossl_quic_wire_decode_frame_max_streams(pkt, &max_streams)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "decode error");
        return 0;
    }

    if (max_streams > (((uint64_t)1) << 60)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "invalid max streams value");
        return 0;
    }

    switch (frame_type) {
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
        if (max_streams > ch->max_local_streams_bidi)
            ch->max_local_streams_bidi = max_streams;

        /* Some streams may now be able to send. */
        ossl_quic_stream_map_visit(&ch->qsm, update_streams_bidi, ch);
        break;
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
        if (max_streams > ch->max_local_streams_uni)
            ch->max_local_streams_uni = max_streams;

        /* Some streams may now be able to send. */
        ossl_quic_stream_map_visit(&ch->qsm, update_streams_uni, ch);
        break;
    default:
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "decode error");
        return 0;
    }

    return 1;
}

static int depack_do_frame_data_blocked(PACKET *pkt,
                                        QUIC_CHANNEL *ch,
                                        OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_data_blocked(pkt, &max_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED,
                                               "decode error");
        return 0;
    }

    /* No-op - informative/debugging frame. */
    return 1;
}

static int depack_do_frame_stream_data_blocked(PACKET *pkt,
                                               QUIC_CHANNEL *ch,
                                               OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t stream_id = 0;
    uint64_t max_data = 0;
    QUIC_STREAM *stream;

    if (!ossl_quic_wire_decode_frame_stream_data_blocked(pkt, &stream_id,
                                                         &max_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED,
                                               "decode error");
        return 0;
    }

    /*
     * This is an informative/debugging frame, so we don't have to do anything,
     * but it does trigger stream creation.
     */
    if (!depack_do_implicit_stream_create(ch, stream_id,
                                          OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED,
                                          &stream))
        return 0; /* error already raised for us */

    if (stream == NULL)
        return 1; /* old deleted stream, not a protocol violation, ignore */

    if (!ossl_quic_stream_has_recv(stream)) {
        /*
         * RFC 9000 s. 19.14: "An endpoint that receives a STREAM_DATA_BLOCKED
         * frame for a send-only stream MUST terminate the connection with error
         * STREAM_STATE_ERROR."
         */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_STATE_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED,
                                               "STREAM_DATA_BLOCKED frame for "
                                               "TX only stream");
        return 0;
    }

    /* No-op - informative/debugging frame. */
    return 1;
}

static int depack_do_frame_streams_blocked(PACKET *pkt,
                                           QUIC_CHANNEL *ch,
                                           OSSL_ACKM_RX_PKT *ackm_data,
                                           uint64_t frame_type)
{
    uint64_t max_data = 0;

    if (!ossl_quic_wire_decode_frame_streams_blocked(pkt, &max_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "decode error");
        return 0;
    }

    if (max_data > (((uint64_t)1) << 60)) {
        /*
         * RFC 9000 s. 19.14: "This value cannot exceed 2**60, as it is not
         * possible to encode stream IDs larger than 2**62 - 1. Receipt of a
         * frame that encodes a larger stream ID MUST be treated as a connection
         * error of type STREAM_LIMIT_ERROR or FRAME_ENCODING_ERROR."
         */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,
                                               frame_type,
                                               "invalid stream count limit");
        return 0;
    }

    /* No-op - informative/debugging frame. */
    return 1;
}

static int depack_do_frame_new_conn_id(PACKET *pkt,
                                       QUIC_CHANNEL *ch,
                                       OSSL_ACKM_RX_PKT *ackm_data)
{
    OSSL_QUIC_FRAME_NEW_CONN_ID frame_data;

    if (!ossl_quic_wire_decode_frame_new_conn_id(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                                               "decode error");
        return 0;
    }

    ossl_quic_channel_on_new_conn_id(ch, &frame_data);

    return 1;
}

static int depack_do_frame_retire_conn_id(PACKET *pkt,
                                          QUIC_CHANNEL *ch,
                                          OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t seq_num;

    if (!ossl_quic_wire_decode_frame_retire_conn_id(pkt, &seq_num)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID,
                                               "decode error");
        return 0;
    }

    /*
     * RFC 9000 s. 19.16: "An endpoint cannot send this frame if it was provided
     * with a zero-length connection ID by its peer. An endpoint that provides a
     * zero-length connection ID MUST treat receipt of a RETIRE_CONNECTION_ID
     * frame as a connection error of type PROTOCOL_VIOLATION."
     *
     * Since we always use a zero-length SCID as a client, there is no case
     * where it is valid for a server to send this. Our server support is
     * currently non-conformant and for internal testing use; simply handle it
     * as a no-op in this case.
     *
     * TODO(QUIC FUTURE): Revise and implement correctly for server support.
     */
    if (!ch->is_server) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID,
                                               "conn has zero-length CID");
        return 0;
    }

    return 1;
}

static void free_path_response(unsigned char *buf, size_t buf_len, void *arg)
{
    OPENSSL_free(buf);
}

static int depack_do_frame_path_challenge(PACKET *pkt,
                                          QUIC_CHANNEL *ch,
                                          OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t frame_data = 0;
    unsigned char *encoded = NULL;
    size_t encoded_len;
    WPACKET wpkt;

    if (!ossl_quic_wire_decode_frame_path_challenge(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE,
                                               "decode error");
        return 0;
    }

    /*
     * RFC 9000 s. 8.2.2: On receiving a PATH_CHALLENGE frame, an endpoint MUST
     * respond by echoing the data contained in the PATH_CHALLENGE frame in a
     * PATH_RESPONSE frame.
     *
     * TODO(QUIC FUTURE): We should try to avoid allocation here in the future.
     */
    encoded_len = sizeof(uint64_t) + 1;
    if ((encoded = OPENSSL_malloc(encoded_len)) == NULL)
        goto err;

    if (!WPACKET_init_static_len(&wpkt, encoded, encoded_len, 0))
        goto err;

    if (!ossl_quic_wire_encode_frame_path_response(&wpkt, frame_data)) {
        WPACKET_cleanup(&wpkt);
        goto err;
    }

    WPACKET_finish(&wpkt);

    if (!ossl_quic_cfq_add_frame(ch->cfq, 0, QUIC_PN_SPACE_APP,
                                 OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE,
                                 QUIC_CFQ_ITEM_FLAG_UNRELIABLE,
                                 encoded, encoded_len,
                                 free_path_response, NULL))
        goto err;

    return 1;

err:
    OPENSSL_free(encoded);
    ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR,
                                           OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE,
                                           "internal error");
    return 0;
}

static int depack_do_frame_path_response(PACKET *pkt,
                                         QUIC_CHANNEL *ch,
                                         OSSL_ACKM_RX_PKT *ackm_data)
{
    uint64_t frame_data = 0;

    if (!ossl_quic_wire_decode_frame_path_response(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE,
                                               "decode error");
        return 0;
    }

    /* TODO(QUIC MULTIPATH): ADD CODE to send |frame_data| to the ch manager */

    return 1;
}

static int depack_do_frame_conn_close(PACKET *pkt, QUIC_CHANNEL *ch,
                                      uint64_t frame_type)
{
    OSSL_QUIC_FRAME_CONN_CLOSE frame_data;

    if (!ossl_quic_wire_decode_frame_conn_close(pkt, &frame_data)) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                               frame_type,
                                               "decode error");
        return 0;
    }

    ossl_quic_channel_on_remote_conn_close(ch, &frame_data);
    return 1;
}

static int depack_do_frame_handshake_done(PACKET *pkt,
                                          QUIC_CHANNEL *ch,
                                          OSSL_ACKM_RX_PKT *ackm_data)
{
    if (!ossl_quic_wire_decode_frame_handshake_done(pkt)) {
        /* This can fail only with an internal error. */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_INTERNAL_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE,
                                               "internal error (decode frame handshake done)");
        return 0;
    }

    ossl_quic_channel_on_handshake_confirmed(ch);
    return 1;
}

/* Main frame processor */

static int depack_process_frames(QUIC_CHANNEL *ch, PACKET *pkt,
                                 OSSL_QRX_PKT *parent_pkt, uint32_t enc_level,
                                 OSSL_TIME received, OSSL_ACKM_RX_PKT *ackm_data)
{
    uint32_t pkt_type = parent_pkt->hdr->type;
    uint32_t packet_space = ossl_quic_enc_level_to_pn_space(enc_level);

    if (PACKET_remaining(pkt) == 0) {
        /*
         * RFC 9000 s. 12.4: An endpoint MUST treat receipt of a packet
         * containing no frames as a connection error of type
         * PROTOCOL_VIOLATION.
         */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               0,
                                               "empty packet payload");
        return 0;
    }

    while (PACKET_remaining(pkt) > 0) {
        int was_minimal;
        uint64_t frame_type;
        const unsigned char *sof = NULL;
        uint64_t datalen = 0;

        if (ch->msg_callback != NULL)
            sof = PACKET_data(pkt);

        if (!ossl_quic_wire_peek_frame_header(pkt, &frame_type, &was_minimal)) {
            ossl_quic_channel_raise_protocol_error(ch,
                                                   OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                   0,
                                                   "malformed frame header");
            return 0;
        }

        if (!was_minimal) {
            ossl_quic_channel_raise_protocol_error(ch,
                                                   OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                   frame_type,
                                                   "non-minimal frame type encoding");
            return 0;
        }

        /*
         * There are only a few frame types which are not ACK-eliciting. Handle
         * these centrally to make error handling cases more resilient, as we
         * should tell the ACKM about an ACK-eliciting frame even if it was not
         * successfully handled.
         */
        switch (frame_type) {
        case OSSL_QUIC_FRAME_TYPE_PADDING:
        case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
        case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
        case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
        case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
            break;
        default:
            ackm_data->is_ack_eliciting = 1;
            break;
        }

        switch (frame_type) {
        case OSSL_QUIC_FRAME_TYPE_PING:
            /* Allowed in all packet types */
            if (!depack_do_frame_ping(pkt, ch, enc_level, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_PADDING:
            /* Allowed in all packet types */
            if (!depack_do_frame_padding(pkt))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
        case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
            /* ACK frames are valid everywhere except in 0RTT packets */
            if (pkt_type == QUIC_PKT_TYPE_0RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "ACK not valid in 0-RTT");
                return 0;
            }
            if (!depack_do_frame_ack(pkt, ch, packet_space, received,
                                     frame_type, parent_pkt))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
            /* RESET_STREAM frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "RESET_STREAM not valid in "
                                                       "INITIAL/HANDSHAKE");
                return 0;
            }
            if (!depack_do_frame_reset_stream(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
            /* STOP_SENDING frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "STOP_SENDING not valid in "
                                                       "INITIAL/HANDSHAKE");
                return 0;
            }
            if (!depack_do_frame_stop_sending(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_CRYPTO:
            /* CRYPTO frames are valid everywhere except in 0RTT packets */
            if (pkt_type == QUIC_PKT_TYPE_0RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "CRYPTO frame not valid in 0-RTT");
                return 0;
            }
            if (!depack_do_frame_crypto(pkt, ch, parent_pkt, ackm_data, &datalen))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
            /* NEW_TOKEN frames are valid in 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "NEW_TOKEN valid only in 1-RTT");
                return 0;
            }

            /*
             * RFC 9000 s. 19.7: "A server MUST treat receipt of a NEW_TOKEN
             * frame as a connection error of type PROTOCOL_VIOLATION."
             */
            if (ch->is_server) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "NEW_TOKEN can only be sent by a server");
                return 0;
            }

            if (!depack_do_frame_new_token(pkt, ch, ackm_data))
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
            /* STREAM frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "STREAM valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_stream(pkt, ch, parent_pkt, ackm_data,
                                        frame_type, &datalen))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
            /* MAX_DATA frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "MAX_DATA valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_max_data(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA:
            /* MAX_STREAM_DATA frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "MAX_STREAM_DATA valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_max_stream_data(pkt, ch, ackm_data))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
        case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
            /* MAX_STREAMS frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "MAX_STREAMS valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_max_streams(pkt, ch, ackm_data,
                                             frame_type))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED:
            /* DATA_BLOCKED frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "DATA_BLOCKED valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_data_blocked(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED:
            /* STREAM_DATA_BLOCKED frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "STREAM_DATA_BLOCKED valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_stream_data_blocked(pkt, ch, ackm_data))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI:
        case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI:
            /* STREAMS_BLOCKED frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "STREAMS valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_streams_blocked(pkt, ch, ackm_data,
                                                 frame_type))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
            /* NEW_CONN_ID frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "NEW_CONN_ID valid only in 0/1-RTT");
            }
            if (!depack_do_frame_new_conn_id(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
            /* RETIRE_CONN_ID frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "RETIRE_CONN_ID valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_retire_conn_id(pkt, ch, ackm_data))
                return 0;
            break;
        case OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE:
            /* PATH_CHALLENGE frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "PATH_CHALLENGE valid only in 0/1-RTT");
                return 0;
            }
            if (!depack_do_frame_path_challenge(pkt, ch, ackm_data))
                return 0;

            break;
        case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
            /* PATH_RESPONSE frames are valid in 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "PATH_CHALLENGE valid only in 1-RTT");
                return 0;
            }
            if (!depack_do_frame_path_response(pkt, ch, ackm_data))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
            /* CONN_CLOSE_APP frames are valid in 0RTT and 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_0RTT
                && pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "CONN_CLOSE (APP) valid only in 0/1-RTT");
                return 0;
            }
            /* FALLTHRU */
        case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
            /* CONN_CLOSE_TRANSPORT frames are valid in all packets */
            if (!depack_do_frame_conn_close(pkt, ch, frame_type))
                return 0;
            break;

        case OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE:
            /* HANDSHAKE_DONE frames are valid in 1RTT packets */
            if (pkt_type != QUIC_PKT_TYPE_1RTT) {
                ossl_quic_channel_raise_protocol_error(ch,
                                                       OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                       frame_type,
                                                       "HANDSHAKE_DONE valid only in 1-RTT");
                return 0;
            }
            if (!depack_do_frame_handshake_done(pkt, ch, ackm_data))
                return 0;
            break;

        default:
            /* Unknown frame type */
            ossl_quic_channel_raise_protocol_error(ch,
                                                   OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,
                                                   frame_type,
                                                   "Unknown frame type received");
            return 0;
        }

        if (ch->msg_callback != NULL) {
            int ctype = SSL3_RT_QUIC_FRAME_FULL;

            size_t framelen = PACKET_data(pkt) - sof;

            if (frame_type == OSSL_QUIC_FRAME_TYPE_PADDING) {
                ctype = SSL3_RT_QUIC_FRAME_PADDING;
            } else if (OSSL_QUIC_FRAME_TYPE_IS_STREAM(frame_type)
                    || frame_type == OSSL_QUIC_FRAME_TYPE_CRYPTO) {
                ctype = SSL3_RT_QUIC_FRAME_HEADER;
                framelen -= (size_t)datalen;
            }

            ch->msg_callback(0, OSSL_QUIC1_VERSION, ctype, sof, framelen,
                             ch->msg_callback_ssl, ch->msg_callback_arg);
        }
    }

    return 1;
}

QUIC_NEEDS_LOCK
int ossl_quic_handle_frames(QUIC_CHANNEL *ch, OSSL_QRX_PKT *qpacket)
{
    PACKET pkt;
    OSSL_ACKM_RX_PKT ackm_data;
    uint32_t enc_level;
    size_t dgram_len = qpacket->datagram_len;

    /*
     * ok has three states:
     * -1 error with ackm_data uninitialized
     *  0 error with ackm_data initialized
     *  1 success (ackm_data initialized)
     */
    int ok = -1;                  /* Assume the worst */

    if (ch == NULL)
        goto end;

    ch->did_crypto_frame = 0;

    /* Initialize |ackm_data| (and reinitialize |ok|)*/
    memset(&ackm_data, 0, sizeof(ackm_data));
    /*
     * ASSUMPTION: All packets that aren't special case have a
     * packet number.
     */
    ackm_data.pkt_num = qpacket->pn;
    ackm_data.time = qpacket->time;
    enc_level = ossl_quic_pkt_type_to_enc_level(qpacket->hdr->type);
    if (enc_level >= QUIC_ENC_LEVEL_NUM)
        /*
         * Retry and Version Negotiation packets should not be passed to this
         * function.
         */
        goto end;

    ok = 0; /* Still assume the worst */
    ackm_data.pkt_space = ossl_quic_enc_level_to_pn_space(enc_level);

    /*
     * RFC 9000 s. 8.1
     * We can consider the connection to be validated, if we receive a packet
     * from the client protected via handshake keys, meaning that the
     * amplification limit no longer applies (i.e. we can set it as validated.
     * Otherwise, add the size of this packet to the unvalidated credit for
     * the connection.
     */
    if (enc_level == QUIC_ENC_LEVEL_HANDSHAKE)
        ossl_quic_tx_packetiser_set_validated(ch->txp);
    else
        ossl_quic_tx_packetiser_add_unvalidated_credit(ch->txp, dgram_len);

    /* Now that special cases are out of the way, parse frames */
    if (!PACKET_buf_init(&pkt, qpacket->hdr->data, qpacket->hdr->len)
        || !depack_process_frames(ch, &pkt, qpacket,
                                  enc_level,
                                  qpacket->time,
                                  &ackm_data))
        goto end;

    ok = 1;
 end:
    /*
     * ASSUMPTION: If this function is called at all, |qpacket| is
     * a legitimate packet, even if its contents aren't.
     * Therefore, we call ossl_ackm_on_rx_packet() unconditionally, as long as
     * |ackm_data| has at least been initialized.
     */
    if (ok >= 0)
        ossl_ackm_on_rx_packet(ch->ackm, &ackm_data);

    return ok > 0;
}
