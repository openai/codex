/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_fifd.h"
#include "internal/quic_wire.h"
#include "internal/qlog_event_helpers.h"

DEFINE_LIST_OF(tx_history, OSSL_ACKM_TX_PKT);

int ossl_quic_fifd_init(QUIC_FIFD *fifd,
                        QUIC_CFQ *cfq,
                        OSSL_ACKM *ackm,
                        QUIC_TXPIM *txpim,
                        /* stream_id is UINT64_MAX for the crypto stream */
                        QUIC_SSTREAM *(*get_sstream_by_id)(uint64_t stream_id,
                                                           uint32_t pn_space,
                                                           void *arg),
                        void *get_sstream_by_id_arg,
                        /* stream_id is UINT64_MAX if not applicable */
                        void (*regen_frame)(uint64_t frame_type,
                                            uint64_t stream_id,
                                            QUIC_TXPIM_PKT *pkt,
                                            void *arg),
                        void *regen_frame_arg,
                        void (*confirm_frame)(uint64_t frame_type,
                                              uint64_t stream_id,
                                              QUIC_TXPIM_PKT *pkt,
                                              void *arg),
                        void *confirm_frame_arg,
                        void (*sstream_updated)(uint64_t stream_id,
                                                void *arg),
                        void *sstream_updated_arg,
                        QLOG *(*get_qlog_cb)(void *arg),
                        void *get_qlog_cb_arg)
{
    if (cfq == NULL || ackm == NULL || txpim == NULL
        || get_sstream_by_id == NULL || regen_frame == NULL)
        return 0;

    fifd->cfq                   = cfq;
    fifd->ackm                  = ackm;
    fifd->txpim                 = txpim;
    fifd->get_sstream_by_id     = get_sstream_by_id;
    fifd->get_sstream_by_id_arg = get_sstream_by_id_arg;
    fifd->regen_frame           = regen_frame;
    fifd->regen_frame_arg       = regen_frame_arg;
    fifd->confirm_frame         = confirm_frame;
    fifd->confirm_frame_arg     = confirm_frame_arg;
    fifd->sstream_updated       = sstream_updated;
    fifd->sstream_updated_arg   = sstream_updated_arg;
    fifd->get_qlog_cb           = get_qlog_cb;
    fifd->get_qlog_cb_arg       = get_qlog_cb_arg;
    return 1;
}

void ossl_quic_fifd_cleanup(QUIC_FIFD *fifd)
{
    /* No-op. */
}

static void on_acked(void *arg)
{
    QUIC_TXPIM_PKT *pkt = arg;
    QUIC_FIFD *fifd = pkt->fifd;
    const QUIC_TXPIM_CHUNK *chunks = ossl_quic_txpim_pkt_get_chunks(pkt);
    size_t i, num_chunks = ossl_quic_txpim_pkt_get_num_chunks(pkt);
    QUIC_SSTREAM *sstream;
    QUIC_CFQ_ITEM *cfq_item, *cfq_item_next;

    /* STREAM and CRYPTO stream chunks, FINs and stream FC frames */
    for (i = 0; i < num_chunks; ++i) {
        sstream = fifd->get_sstream_by_id(chunks[i].stream_id,
                                          pkt->ackm_pkt.pkt_space,
                                          fifd->get_sstream_by_id_arg);
        if (sstream == NULL)
            continue;

        if (chunks[i].end >= chunks[i].start)
            /* coverity[check_return]: Best effort - we cannot fail here. */
            ossl_quic_sstream_mark_acked(sstream,
                                         chunks[i].start, chunks[i].end);

        if (chunks[i].has_fin && chunks[i].stream_id != UINT64_MAX)
            ossl_quic_sstream_mark_acked_fin(sstream);

        if (chunks[i].has_stop_sending && chunks[i].stream_id != UINT64_MAX)
            fifd->confirm_frame(OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                                chunks[i].stream_id, pkt,
                                fifd->confirm_frame_arg);

        if (chunks[i].has_reset_stream && chunks[i].stream_id != UINT64_MAX)
            fifd->confirm_frame(OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                                chunks[i].stream_id, pkt,
                                fifd->confirm_frame_arg);

        if (ossl_quic_sstream_is_totally_acked(sstream))
            fifd->sstream_updated(chunks[i].stream_id, fifd->sstream_updated_arg);
    }

    /* GCR */
    for (cfq_item = pkt->retx_head; cfq_item != NULL; cfq_item = cfq_item_next) {
        cfq_item_next = cfq_item->pkt_next;
        ossl_quic_cfq_release(fifd->cfq, cfq_item);
    }

    ossl_quic_txpim_pkt_release(fifd->txpim, pkt);
}

static QLOG *fifd_get_qlog(QUIC_FIFD *fifd)
{
    if (fifd->get_qlog_cb == NULL)
        return NULL;

    return fifd->get_qlog_cb(fifd->get_qlog_cb_arg);
}

static void on_lost(void *arg)
{
    QUIC_TXPIM_PKT *pkt = arg;
    QUIC_FIFD *fifd = pkt->fifd;
    const QUIC_TXPIM_CHUNK *chunks = ossl_quic_txpim_pkt_get_chunks(pkt);
    size_t i, num_chunks = ossl_quic_txpim_pkt_get_num_chunks(pkt);
    QUIC_SSTREAM *sstream;
    QUIC_CFQ_ITEM *cfq_item, *cfq_item_next;
    int sstream_updated;

    ossl_qlog_event_recovery_packet_lost(fifd_get_qlog(fifd), pkt);

    /* STREAM and CRYPTO stream chunks, FIN and stream FC frames */
    for (i = 0; i < num_chunks; ++i) {
        sstream = fifd->get_sstream_by_id(chunks[i].stream_id,
                                          pkt->ackm_pkt.pkt_space,
                                          fifd->get_sstream_by_id_arg);
        if (sstream == NULL)
            continue;

        sstream_updated = 0;

        if (chunks[i].end >= chunks[i].start) {
            /*
             * Note: If the stream is being reset, we do not need to retransmit
             * old data as this is pointless. In this case this will be handled
             * by (sstream == NULL) above as the QSM will free the QUIC_SSTREAM
             * and our call to get_sstream_by_id above will return NULL.
             */
            ossl_quic_sstream_mark_lost(sstream,
                                        chunks[i].start, chunks[i].end);
            sstream_updated = 1;
        }

        if (chunks[i].has_fin && chunks[i].stream_id != UINT64_MAX) {
            ossl_quic_sstream_mark_lost_fin(sstream);
            sstream_updated = 1;
        }

        if (chunks[i].has_stop_sending && chunks[i].stream_id != UINT64_MAX)
            fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                              chunks[i].stream_id, pkt,
                              fifd->regen_frame_arg);

        if (chunks[i].has_reset_stream && chunks[i].stream_id != UINT64_MAX)
            fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                              chunks[i].stream_id, pkt,
                              fifd->regen_frame_arg);

        /*
         * Inform caller that stream needs an FC frame.
         *
         * Note: We could track whether an FC frame was sent originally for the
         * stream to determine if it really needs to be regenerated or not.
         * However, if loss has occurred, it's probably better to ensure the
         * peer has up-to-date flow control data for the stream. Given that
         * these frames are extremely small, we may as well always send it when
         * handling loss.
         */
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA,
                          chunks[i].stream_id,
                          pkt,
                          fifd->regen_frame_arg);

        if (sstream_updated && chunks[i].stream_id != UINT64_MAX)
            fifd->sstream_updated(chunks[i].stream_id,
                                  fifd->sstream_updated_arg);
    }

    /* GCR */
    for (cfq_item = pkt->retx_head; cfq_item != NULL; cfq_item = cfq_item_next) {
        cfq_item_next = cfq_item->pkt_next;
        ossl_quic_cfq_mark_lost(fifd->cfq, cfq_item, UINT32_MAX);
    }

    /* Regenerate flag frames */
    if (pkt->had_handshake_done_frame)
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE,
                          UINT64_MAX, pkt,
                          fifd->regen_frame_arg);

    if (pkt->had_max_data_frame)
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_MAX_DATA,
                          UINT64_MAX, pkt,
                          fifd->regen_frame_arg);

    if (pkt->had_max_streams_bidi_frame)
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI,
                          UINT64_MAX, pkt,
                          fifd->regen_frame_arg);

    if (pkt->had_max_streams_uni_frame)
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI,
                          UINT64_MAX, pkt,
                          fifd->regen_frame_arg);

    if (pkt->had_ack_frame)
        /*
         * We always use the ACK_WITH_ECN frame type to represent the ACK frame
         * type in our callback; we assume it is the caller's job to decide
         * whether it wants to send ECN data or not.
         */
        fifd->regen_frame(OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN,
                          UINT64_MAX, pkt,
                          fifd->regen_frame_arg);

    ossl_quic_txpim_pkt_release(fifd->txpim, pkt);
}

static void on_discarded(void *arg)
{
    QUIC_TXPIM_PKT *pkt = arg;
    QUIC_FIFD *fifd = pkt->fifd;
    QUIC_CFQ_ITEM *cfq_item, *cfq_item_next;

    /*
     * Don't need to do anything to SSTREAMs for STREAM and CRYPTO streams, as
     * we assume caller will clean them up.
     */

    /* GCR */
    for (cfq_item = pkt->retx_head; cfq_item != NULL; cfq_item = cfq_item_next) {
        cfq_item_next = cfq_item->pkt_next;
        ossl_quic_cfq_release(fifd->cfq, cfq_item);
    }

    ossl_quic_txpim_pkt_release(fifd->txpim, pkt);
}

int ossl_quic_fifd_pkt_commit(QUIC_FIFD *fifd, QUIC_TXPIM_PKT *pkt)
{
    QUIC_CFQ_ITEM *cfq_item;
    const QUIC_TXPIM_CHUNK *chunks;
    size_t i, num_chunks;
    QUIC_SSTREAM *sstream;

    pkt->fifd                   = fifd;

    pkt->ackm_pkt.on_lost       = on_lost;
    pkt->ackm_pkt.on_acked      = on_acked;
    pkt->ackm_pkt.on_discarded  = on_discarded;
    pkt->ackm_pkt.cb_arg        = pkt;

    ossl_list_tx_history_init_elem(&pkt->ackm_pkt);
    pkt->ackm_pkt.anext = pkt->ackm_pkt.lnext = NULL;

    /*
     * Mark the CFQ items which have been added to this packet as having been
     * transmitted.
     */
    for (cfq_item = pkt->retx_head;
         cfq_item != NULL;
         cfq_item = cfq_item->pkt_next)
        ossl_quic_cfq_mark_tx(fifd->cfq, cfq_item);

    /*
     * Mark the send stream chunks which have been added to the packet as having
     * been transmitted.
     */
    chunks = ossl_quic_txpim_pkt_get_chunks(pkt);
    num_chunks = ossl_quic_txpim_pkt_get_num_chunks(pkt);
    for (i = 0; i < num_chunks; ++i) {
        sstream = fifd->get_sstream_by_id(chunks[i].stream_id,
                                          pkt->ackm_pkt.pkt_space,
                                          fifd->get_sstream_by_id_arg);
        if (sstream == NULL)
            continue;

        if (chunks[i].end >= chunks[i].start
            && !ossl_quic_sstream_mark_transmitted(sstream,
                                                   chunks[i].start,
                                                   chunks[i].end))
            return 0;

        if (chunks[i].has_fin
            && !ossl_quic_sstream_mark_transmitted_fin(sstream,
                                                       chunks[i].end + 1))
                return 0;
    }

    /* Inform the ACKM. */
    return ossl_ackm_on_tx_packet(fifd->ackm, &pkt->ackm_pkt);
}

void ossl_quic_fifd_set_qlog_cb(QUIC_FIFD *fifd, QLOG *(*get_qlog_cb)(void *arg),
                                void *get_qlog_cb_arg)
{
    fifd->get_qlog_cb       = get_qlog_cb;
    fifd->get_qlog_cb_arg   = get_qlog_cb_arg;
}
