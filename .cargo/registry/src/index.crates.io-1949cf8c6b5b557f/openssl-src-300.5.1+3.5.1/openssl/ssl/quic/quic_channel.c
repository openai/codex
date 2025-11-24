/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rand.h>
#include <openssl/err.h>
#include "internal/ssl_unwrap.h"
#include "internal/quic_channel.h"
#include "internal/quic_error.h"
#include "internal/quic_rx_depack.h"
#include "internal/quic_lcidm.h"
#include "internal/quic_srtm.h"
#include "internal/qlog_event_helpers.h"
#include "internal/quic_txp.h"
#include "internal/quic_tls.h"
#include "internal/quic_ssl.h"
#include "../ssl_local.h"
#include "quic_channel_local.h"
#include "quic_port_local.h"
#include "quic_engine_local.h"

#define INIT_CRYPTO_RECV_BUF_LEN    16384
#define INIT_CRYPTO_SEND_BUF_LEN    16384
#define INIT_APP_BUF_LEN             8192

/*
 * Interval before we force a PING to ensure NATs don't timeout. This is based
 * on the lowest commonly seen value of 30 seconds as cited in RFC 9000 s.
 * 10.1.2.
 */
#define MAX_NAT_INTERVAL (ossl_ms2time(25000))

/*
 * Our maximum ACK delay on the TX side. This is up to us to choose. Note that
 * this could differ from QUIC_DEFAULT_MAX_DELAY in future as that is a protocol
 * value which determines the value of the maximum ACK delay if the
 * max_ack_delay transport parameter is not set.
 */
#define DEFAULT_MAX_ACK_DELAY   QUIC_DEFAULT_MAX_ACK_DELAY

DEFINE_LIST_OF_IMPL(ch, QUIC_CHANNEL);

static void ch_save_err_state(QUIC_CHANNEL *ch);
static int ch_rx(QUIC_CHANNEL *ch, int channel_only, int *notify_other_threads);
static int ch_tx(QUIC_CHANNEL *ch, int *notify_other_threads);
static int ch_tick_tls(QUIC_CHANNEL *ch, int channel_only, int *notify_other_threads);
static void ch_rx_handle_packet(QUIC_CHANNEL *ch, int channel_only);
static OSSL_TIME ch_determine_next_tick_deadline(QUIC_CHANNEL *ch);
static int ch_retry(QUIC_CHANNEL *ch,
                    const unsigned char *retry_token,
                    size_t retry_token_len,
                    const QUIC_CONN_ID *retry_scid,
                    int drop_later_pn);
static int ch_restart(QUIC_CHANNEL *ch);

static void ch_cleanup(QUIC_CHANNEL *ch);
static int ch_generate_transport_params(QUIC_CHANNEL *ch);
static int ch_on_transport_params(const unsigned char *params,
                                  size_t params_len,
                                  void *arg);
static int ch_on_handshake_alert(void *arg, unsigned char alert_code);
static int ch_on_handshake_complete(void *arg);
static int ch_on_handshake_yield_secret(uint32_t prot_level, int direction,
                                        uint32_t suite_id, EVP_MD *md,
                                        const unsigned char *secret,
                                        size_t secret_len,
                                        void *arg);
static int ch_on_crypto_recv_record(const unsigned char **buf,
                                    size_t *bytes_read, void *arg);
static int ch_on_crypto_release_record(size_t bytes_read, void *arg);
static int crypto_ensure_empty(QUIC_RSTREAM *rstream);
static int ch_on_crypto_send(const unsigned char *buf, size_t buf_len,
                             size_t *consumed, void *arg);
static OSSL_TIME get_time(void *arg);
static uint64_t get_stream_limit(int uni, void *arg);
static int rx_late_validate(QUIC_PN pn, int pn_space, void *arg);
static void rxku_detected(QUIC_PN pn, void *arg);
static int ch_retry(QUIC_CHANNEL *ch,
                    const unsigned char *retry_token,
                    size_t retry_token_len,
                    const QUIC_CONN_ID *retry_scid,
                    int drop_later_pn);
static void ch_update_idle(QUIC_CHANNEL *ch);
static int ch_discard_el(QUIC_CHANNEL *ch,
                         uint32_t enc_level);
static void ch_on_idle_timeout(QUIC_CHANNEL *ch);
static void ch_update_idle(QUIC_CHANNEL *ch);
static void ch_update_ping_deadline(QUIC_CHANNEL *ch);
static void ch_on_terminating_timeout(QUIC_CHANNEL *ch);
static void ch_start_terminating(QUIC_CHANNEL *ch,
                                 const QUIC_TERMINATE_CAUSE *tcause,
                                 int force_immediate);
static void ch_on_txp_ack_tx(const OSSL_QUIC_FRAME_ACK *ack, uint32_t pn_space,
                             void *arg);
static void ch_rx_handle_version_neg(QUIC_CHANNEL *ch, OSSL_QRX_PKT *pkt);
static void ch_raise_version_neg_failure(QUIC_CHANNEL *ch);
static void ch_record_state_transition(QUIC_CHANNEL *ch, uint32_t new_state);

DEFINE_LHASH_OF_EX(QUIC_SRT_ELEM);

QUIC_NEEDS_LOCK
static QLOG *ch_get_qlog(QUIC_CHANNEL *ch)
{
#ifndef OPENSSL_NO_QLOG
    QLOG_TRACE_INFO qti = {0};

    if (ch->qlog != NULL)
        return ch->qlog;

    if (!ch->use_qlog)
        return NULL;

    if (ch->is_server && ch->init_dcid.id_len == 0)
        return NULL;

    qti.odcid       = ch->init_dcid;
    qti.title       = ch->qlog_title;
    qti.description = NULL;
    qti.group_id    = NULL;
    qti.is_server   = ch->is_server;
    qti.now_cb      = get_time;
    qti.now_cb_arg  = ch;
    if ((ch->qlog = ossl_qlog_new_from_env(&qti)) == NULL) {
        ch->use_qlog = 0; /* don't try again */
        return NULL;
    }

    return ch->qlog;
#else
    return NULL;
#endif
}

QUIC_NEEDS_LOCK
static QLOG *ch_get_qlog_cb(void *arg)
{
    QUIC_CHANNEL *ch = arg;

    return ch_get_qlog(ch);
}

/*
 * QUIC Channel Initialization and Teardown
 * ========================================
 */
#define DEFAULT_INIT_CONN_RXFC_WND      (768 * 1024)
#define DEFAULT_CONN_RXFC_MAX_WND_MUL   20

#define DEFAULT_INIT_STREAM_RXFC_WND    (512 * 1024)
#define DEFAULT_STREAM_RXFC_MAX_WND_MUL 12

#define DEFAULT_INIT_CONN_MAX_STREAMS           100

static int ch_init(QUIC_CHANNEL *ch)
{
    OSSL_QUIC_TX_PACKETISER_ARGS txp_args = {0};
    OSSL_QTX_ARGS qtx_args = {0};
    OSSL_QRX_ARGS qrx_args = {0};
    QUIC_TLS_ARGS tls_args = {0};
    uint32_t pn_space;
    size_t rx_short_dcid_len;
    size_t tx_init_dcid_len;

    if (ch->port == NULL || ch->lcidm == NULL || ch->srtm == NULL)
        goto err;

    rx_short_dcid_len = ossl_quic_port_get_rx_short_dcid_len(ch->port);
    tx_init_dcid_len = ossl_quic_port_get_tx_init_dcid_len(ch->port);

    /* For clients, generate our initial DCID. */
    if (!ch->is_server
        && !ossl_quic_gen_rand_conn_id(ch->port->engine->libctx, tx_init_dcid_len,
                                       &ch->init_dcid))
        goto err;

    /* We plug in a network write BIO to the QTX later when we get one. */
    qtx_args.libctx             = ch->port->engine->libctx;
    qtx_args.get_qlog_cb        = ch_get_qlog_cb;
    qtx_args.get_qlog_cb_arg    = ch;
    qtx_args.mdpl               = QUIC_MIN_INITIAL_DGRAM_LEN;
    ch->rx_max_udp_payload_size = qtx_args.mdpl;

    ch->ping_deadline = ossl_time_infinite();

    ch->qtx = ossl_qtx_new(&qtx_args);
    if (ch->qtx == NULL)
        goto err;

    ch->txpim = ossl_quic_txpim_new();
    if (ch->txpim == NULL)
        goto err;

    ch->cfq = ossl_quic_cfq_new();
    if (ch->cfq == NULL)
        goto err;

    if (!ossl_quic_txfc_init(&ch->conn_txfc, NULL))
        goto err;

    /*
     * Note: The TP we transmit governs what the peer can transmit and thus
     * applies to the RXFC.
     */
    ch->tx_init_max_stream_data_bidi_local  = DEFAULT_INIT_STREAM_RXFC_WND;
    ch->tx_init_max_stream_data_bidi_remote = DEFAULT_INIT_STREAM_RXFC_WND;
    ch->tx_init_max_stream_data_uni         = DEFAULT_INIT_STREAM_RXFC_WND;

    if (!ossl_quic_rxfc_init(&ch->conn_rxfc, NULL,
                             DEFAULT_INIT_CONN_RXFC_WND,
                             DEFAULT_CONN_RXFC_MAX_WND_MUL *
                             DEFAULT_INIT_CONN_RXFC_WND,
                             get_time, ch))
        goto err;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space)
        if (!ossl_quic_rxfc_init_standalone(&ch->crypto_rxfc[pn_space],
                                            INIT_CRYPTO_RECV_BUF_LEN,
                                            get_time, ch))
            goto err;

    if (!ossl_quic_rxfc_init_standalone(&ch->max_streams_bidi_rxfc,
                                        DEFAULT_INIT_CONN_MAX_STREAMS,
                                        get_time, ch))
        goto err;

    if (!ossl_quic_rxfc_init_standalone(&ch->max_streams_uni_rxfc,
                                        DEFAULT_INIT_CONN_MAX_STREAMS,
                                        get_time, ch))
        goto err;

    if (!ossl_statm_init(&ch->statm))
        goto err;

    ch->have_statm = 1;
    ch->cc_method = &ossl_cc_newreno_method;
    if ((ch->cc_data = ch->cc_method->new(get_time, ch)) == NULL)
        goto err;

    if ((ch->ackm = ossl_ackm_new(get_time, ch, &ch->statm,
                                  ch->cc_method, ch->cc_data)) == NULL)
        goto err;

    if (!ossl_quic_stream_map_init(&ch->qsm, get_stream_limit, ch,
                                   &ch->max_streams_bidi_rxfc,
                                   &ch->max_streams_uni_rxfc,
                                   ch->is_server))
        goto err;

    ch->have_qsm = 1;

    if (!ch->is_server
        && !ossl_quic_lcidm_generate_initial(ch->lcidm, ch, &ch->init_scid))
        goto err;

    txp_args.cur_scid               = ch->init_scid;
    txp_args.cur_dcid               = ch->init_dcid;
    txp_args.ack_delay_exponent     = 3;
    txp_args.qtx                    = ch->qtx;
    txp_args.txpim                  = ch->txpim;
    txp_args.cfq                    = ch->cfq;
    txp_args.ackm                   = ch->ackm;
    txp_args.qsm                    = &ch->qsm;
    txp_args.conn_txfc              = &ch->conn_txfc;
    txp_args.conn_rxfc              = &ch->conn_rxfc;
    txp_args.max_streams_bidi_rxfc  = &ch->max_streams_bidi_rxfc;
    txp_args.max_streams_uni_rxfc   = &ch->max_streams_uni_rxfc;
    txp_args.cc_method              = ch->cc_method;
    txp_args.cc_data                = ch->cc_data;
    txp_args.now                    = get_time;
    txp_args.now_arg                = ch;
    txp_args.get_qlog_cb            = ch_get_qlog_cb;
    txp_args.get_qlog_cb_arg        = ch;
    txp_args.protocol_version       = QUIC_VERSION_1;

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space) {
        ch->crypto_send[pn_space] = ossl_quic_sstream_new(INIT_CRYPTO_SEND_BUF_LEN);
        if (ch->crypto_send[pn_space] == NULL)
            goto err;

        txp_args.crypto[pn_space] = ch->crypto_send[pn_space];
    }

    ch->txp = ossl_quic_tx_packetiser_new(&txp_args);
    if (ch->txp == NULL)
        goto err;

    /* clients have no amplification limit, so are considered always valid */
    if (!ch->is_server)
        ossl_quic_tx_packetiser_set_validated(ch->txp);

    ossl_quic_tx_packetiser_set_ack_tx_cb(ch->txp, ch_on_txp_ack_tx, ch);

    /*
     * qrx does not exist yet, then we must be dealing with client channel
     * (QUIC connection initiator).
     * If qrx exists already, then we are dealing with server channel which
     * qrx gets created by port_default_packet_handler() before
     * port_default_packet_handler() accepts connection and creates channel
     * for it.
     * The exception here is tserver which always creates channel,
     * before the first packet is ever seen.
     */
    if (ch->qrx == NULL && ch->is_tserver_ch == 0) {
        /* we are regular client, create channel */
        qrx_args.libctx             = ch->port->engine->libctx;
        qrx_args.demux              = ch->port->demux;
        qrx_args.short_conn_id_len  = rx_short_dcid_len;
        qrx_args.max_deferred       = 32;

        if ((ch->qrx = ossl_qrx_new(&qrx_args)) == NULL)
            goto err;
    }

    if (ch->qrx != NULL) {
        /*
         * callbacks for channels associated with tserver's port
         * are set up later when we call ossl_quic_channel_bind_qrx()
         * in port_default_packet_handler()
         */
        if (!ossl_qrx_set_late_validation_cb(ch->qrx,
                                             rx_late_validate,
                                             ch))
            goto err;

        if (!ossl_qrx_set_key_update_cb(ch->qrx,
                                        rxku_detected,
                                        ch))
            goto err;
    }


    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space) {
        ch->crypto_recv[pn_space] = ossl_quic_rstream_new(NULL, NULL, 0);
        if (ch->crypto_recv[pn_space] == NULL)
            goto err;
    }

    /* Plug in the TLS handshake layer. */
    tls_args.s                          = ch->tls;
    tls_args.crypto_send_cb             = ch_on_crypto_send;
    tls_args.crypto_send_cb_arg         = ch;
    tls_args.crypto_recv_rcd_cb         = ch_on_crypto_recv_record;
    tls_args.crypto_recv_rcd_cb_arg     = ch;
    tls_args.crypto_release_rcd_cb      = ch_on_crypto_release_record;
    tls_args.crypto_release_rcd_cb_arg  = ch;
    tls_args.yield_secret_cb            = ch_on_handshake_yield_secret;
    tls_args.yield_secret_cb_arg        = ch;
    tls_args.got_transport_params_cb    = ch_on_transport_params;
    tls_args.got_transport_params_cb_arg= ch;
    tls_args.handshake_complete_cb      = ch_on_handshake_complete;
    tls_args.handshake_complete_cb_arg  = ch;
    tls_args.alert_cb                   = ch_on_handshake_alert;
    tls_args.alert_cb_arg               = ch;
    tls_args.is_server                  = ch->is_server;
    tls_args.ossl_quic                  = 1;

    if ((ch->qtls = ossl_quic_tls_new(&tls_args)) == NULL)
        goto err;

    ch->tx_max_ack_delay        = DEFAULT_MAX_ACK_DELAY;
    ch->rx_max_ack_delay        = QUIC_DEFAULT_MAX_ACK_DELAY;
    ch->rx_ack_delay_exp        = QUIC_DEFAULT_ACK_DELAY_EXP;
    ch->rx_active_conn_id_limit = QUIC_MIN_ACTIVE_CONN_ID_LIMIT;
    ch->tx_enc_level            = QUIC_ENC_LEVEL_INITIAL;
    ch->rx_enc_level            = QUIC_ENC_LEVEL_INITIAL;
    ch->txku_threshold_override = UINT64_MAX;

    ch->max_idle_timeout_local_req  = QUIC_DEFAULT_IDLE_TIMEOUT;
    ch->max_idle_timeout_remote_req = 0;
    ch->max_idle_timeout            = ch->max_idle_timeout_local_req;

    ossl_ackm_set_tx_max_ack_delay(ch->ackm, ossl_ms2time(ch->tx_max_ack_delay));
    ossl_ackm_set_rx_max_ack_delay(ch->ackm, ossl_ms2time(ch->rx_max_ack_delay));

    ch_update_idle(ch);
    ossl_list_ch_insert_tail(&ch->port->channel_list, ch);
    ch->on_port_list = 1;
    return 1;

err:
    ch_cleanup(ch);
    return 0;
}

static void ch_cleanup(QUIC_CHANNEL *ch)
{
    uint32_t pn_space;

    if (ch->ackm != NULL)
        for (pn_space = QUIC_PN_SPACE_INITIAL;
             pn_space < QUIC_PN_SPACE_NUM;
             ++pn_space)
            ossl_ackm_on_pkt_space_discarded(ch->ackm, pn_space);

    ossl_quic_lcidm_cull(ch->lcidm, ch);
    ossl_quic_srtm_cull(ch->srtm, ch);
    ossl_quic_tx_packetiser_free(ch->txp);
    ossl_quic_txpim_free(ch->txpim);
    ossl_quic_cfq_free(ch->cfq);
    ossl_qtx_free(ch->qtx);
    if (ch->cc_data != NULL)
        ch->cc_method->free(ch->cc_data);
    if (ch->have_statm)
        ossl_statm_destroy(&ch->statm);
    ossl_ackm_free(ch->ackm);

    if (ch->have_qsm)
        ossl_quic_stream_map_cleanup(&ch->qsm);

    for (pn_space = QUIC_PN_SPACE_INITIAL; pn_space < QUIC_PN_SPACE_NUM; ++pn_space) {
        ossl_quic_sstream_free(ch->crypto_send[pn_space]);
        ossl_quic_rstream_free(ch->crypto_recv[pn_space]);
    }

    ossl_qrx_pkt_release(ch->qrx_pkt);
    ch->qrx_pkt = NULL;

    ossl_quic_tls_free(ch->qtls);
    ossl_qrx_free(ch->qrx);
    OPENSSL_free(ch->local_transport_params);
    OPENSSL_free((char *)ch->terminate_cause.reason);
    OSSL_ERR_STATE_free(ch->err_state);
    OPENSSL_free(ch->ack_range_scratch);
    OPENSSL_free(ch->pending_new_token);

    if (ch->on_port_list) {
        ossl_list_ch_remove(&ch->port->channel_list, ch);
        ch->on_port_list = 0;
    }

#ifndef OPENSSL_NO_QLOG
    if (ch->qlog != NULL)
        ossl_qlog_flush(ch->qlog); /* best effort */

    OPENSSL_free(ch->qlog_title);
    ossl_qlog_free(ch->qlog);
#endif
}

int ossl_quic_channel_init(QUIC_CHANNEL *ch)
{
    return ch_init(ch);
}

void ossl_quic_channel_bind_qrx(QUIC_CHANNEL *tserver_ch, OSSL_QRX *qrx)
{
    if (tserver_ch->qrx == NULL && tserver_ch->is_tserver_ch == 1) {
        tserver_ch->qrx = qrx;
        ossl_qrx_set_late_validation_cb(tserver_ch->qrx, rx_late_validate,
                                        tserver_ch);
        ossl_qrx_set_key_update_cb(tserver_ch->qrx, rxku_detected,
                                   tserver_ch);
    }
}

QUIC_CHANNEL *ossl_quic_channel_alloc(const QUIC_CHANNEL_ARGS *args)
{
    QUIC_CHANNEL *ch = NULL;

    if ((ch = OPENSSL_zalloc(sizeof(*ch))) == NULL)
        return NULL;

    ch->port           = args->port;
    ch->is_server      = args->is_server;
    ch->tls            = args->tls;
    ch->lcidm          = args->lcidm;
    ch->srtm           = args->srtm;
    ch->qrx            = args->qrx;
    ch->is_tserver_ch  = args->is_tserver_ch;
#ifndef OPENSSL_NO_QLOG
    ch->use_qlog    = args->use_qlog;

    if (ch->use_qlog && args->qlog_title != NULL) {
        if ((ch->qlog_title = OPENSSL_strdup(args->qlog_title)) == NULL) {
            OPENSSL_free(ch);
            return NULL;
        }
    }
#endif

    return ch;
}

void ossl_quic_channel_free(QUIC_CHANNEL *ch)
{
    if (ch == NULL)
        return;

    ch_cleanup(ch);
    OPENSSL_free(ch);
}

/* Set mutator callbacks for test framework support */
int ossl_quic_channel_set_mutator(QUIC_CHANNEL *ch,
                                  ossl_mutate_packet_cb mutatecb,
                                  ossl_finish_mutate_cb finishmutatecb,
                                  void *mutatearg)
{
    if (ch->qtx == NULL)
        return 0;

    ossl_qtx_set_mutator(ch->qtx, mutatecb, finishmutatecb, mutatearg);
    return 1;
}

int ossl_quic_channel_get_peer_addr(QUIC_CHANNEL *ch, BIO_ADDR *peer_addr)
{
    if (!ch->addressed_mode)
        return 0;

    return BIO_ADDR_copy(peer_addr, &ch->cur_peer_addr);
}

int ossl_quic_channel_set_peer_addr(QUIC_CHANNEL *ch, const BIO_ADDR *peer_addr)
{
    if (ch->state != QUIC_CHANNEL_STATE_IDLE)
        return 0;

    if (peer_addr == NULL || BIO_ADDR_family(peer_addr) == AF_UNSPEC) {
        BIO_ADDR_clear(&ch->cur_peer_addr);
        ch->addressed_mode = 0;
        return 1;
    }

    if (!BIO_ADDR_copy(&ch->cur_peer_addr, peer_addr)) {
        ch->addressed_mode = 0;
        return 0;
    }
    ch->addressed_mode = 1;

    return 1;
}

QUIC_REACTOR *ossl_quic_channel_get_reactor(QUIC_CHANNEL *ch)
{
    return ossl_quic_port_get0_reactor(ch->port);
}

QUIC_STREAM_MAP *ossl_quic_channel_get_qsm(QUIC_CHANNEL *ch)
{
    return &ch->qsm;
}

OSSL_STATM *ossl_quic_channel_get_statm(QUIC_CHANNEL *ch)
{
    return &ch->statm;
}

SSL *ossl_quic_channel_get0_tls(QUIC_CHANNEL *ch)
{
    return ch->tls;
}

static void free_buf_mem(unsigned char *buf, size_t buf_len, void *arg)
{
    BUF_MEM_free((BUF_MEM *)arg);
}

int ossl_quic_channel_schedule_new_token(QUIC_CHANNEL *ch,
                                         const unsigned char *token,
                                         size_t token_len)
{
    int rc = 0;
    QUIC_CFQ_ITEM *cfq_item;
    WPACKET wpkt;
    BUF_MEM *buf_mem = NULL;
    size_t l = 0;

    buf_mem = BUF_MEM_new();
    if (buf_mem == NULL)
        goto err;

    if (!WPACKET_init(&wpkt, buf_mem))
        goto err;

    if (!ossl_quic_wire_encode_frame_new_token(&wpkt, token,
                                               token_len)) {
        WPACKET_cleanup(&wpkt);
        goto err;
    }

    WPACKET_finish(&wpkt);

    if (!WPACKET_get_total_written(&wpkt, &l))
        goto err;

    cfq_item = ossl_quic_cfq_add_frame(ch->cfq, 1,
                                       QUIC_PN_SPACE_APP,
                                       OSSL_QUIC_FRAME_TYPE_NEW_TOKEN, 0,
                                       (unsigned char *)buf_mem->data, l,
                                       free_buf_mem,
                                       buf_mem);
    if (cfq_item == NULL)
        goto err;

    rc = 1;
err:
    if (!rc)
        BUF_MEM_free(buf_mem);
    return rc;
}

size_t ossl_quic_channel_get_short_header_conn_id_len(QUIC_CHANNEL *ch)
{
    return ossl_quic_port_get_rx_short_dcid_len(ch->port);
}

QUIC_STREAM *ossl_quic_channel_get_stream_by_id(QUIC_CHANNEL *ch,
                                                uint64_t stream_id)
{
    return ossl_quic_stream_map_get_by_id(&ch->qsm, stream_id);
}

int ossl_quic_channel_is_active(const QUIC_CHANNEL *ch)
{
    return ch != NULL && ch->state == QUIC_CHANNEL_STATE_ACTIVE;
}

int ossl_quic_channel_is_closing(const QUIC_CHANNEL *ch)
{
    return ch->state == QUIC_CHANNEL_STATE_TERMINATING_CLOSING;
}

static int ossl_quic_channel_is_draining(const QUIC_CHANNEL *ch)
{
    return ch->state == QUIC_CHANNEL_STATE_TERMINATING_DRAINING;
}

static int ossl_quic_channel_is_terminating(const QUIC_CHANNEL *ch)
{
    return ossl_quic_channel_is_closing(ch)
        || ossl_quic_channel_is_draining(ch);
}

int ossl_quic_channel_is_terminated(const QUIC_CHANNEL *ch)
{
    return ch->state == QUIC_CHANNEL_STATE_TERMINATED;
}

int ossl_quic_channel_is_term_any(const QUIC_CHANNEL *ch)
{
    return ossl_quic_channel_is_terminating(ch)
        || ossl_quic_channel_is_terminated(ch);
}

const QUIC_TERMINATE_CAUSE *
ossl_quic_channel_get_terminate_cause(const QUIC_CHANNEL *ch)
{
    return ossl_quic_channel_is_term_any(ch) ? &ch->terminate_cause : NULL;
}

int ossl_quic_channel_is_handshake_complete(const QUIC_CHANNEL *ch)
{
    return ch->handshake_complete;
}

int ossl_quic_channel_is_handshake_confirmed(const QUIC_CHANNEL *ch)
{
    return ch->handshake_confirmed;
}

QUIC_DEMUX *ossl_quic_channel_get0_demux(QUIC_CHANNEL *ch)
{
    return ch->port->demux;
}

QUIC_PORT *ossl_quic_channel_get0_port(QUIC_CHANNEL *ch)
{
    return ch->port;
}

QUIC_ENGINE *ossl_quic_channel_get0_engine(QUIC_CHANNEL *ch)
{
    return ossl_quic_port_get0_engine(ch->port);
}

CRYPTO_MUTEX *ossl_quic_channel_get_mutex(QUIC_CHANNEL *ch)
{
    return ossl_quic_port_get0_mutex(ch->port);
}

int ossl_quic_channel_has_pending(const QUIC_CHANNEL *ch)
{
    return ossl_quic_demux_has_pending(ch->port->demux)
        || ossl_qrx_processed_read_pending(ch->qrx);
}

/*
 * QUIC Channel: Callbacks from Miscellaneous Subsidiary Components
 * ================================================================
 */

/* Used by various components. */
static OSSL_TIME get_time(void *arg)
{
    QUIC_CHANNEL *ch = arg;

    return ossl_quic_port_get_time(ch->port);
}

/* Used by QSM. */
static uint64_t get_stream_limit(int uni, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    return uni ? ch->max_local_streams_uni : ch->max_local_streams_bidi;
}

/*
 * Called by QRX to determine if a packet is potentially invalid before trying
 * to decrypt it.
 */
static int rx_late_validate(QUIC_PN pn, int pn_space, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    /* Potential duplicates should not be processed. */
    if (!ossl_ackm_is_rx_pn_processable(ch->ackm, pn, pn_space))
        return 0;

    return 1;
}

/*
 * Triggers a TXKU (whether spontaneous or solicited). Does not check whether
 * spontaneous TXKU is currently allowed.
 */
QUIC_NEEDS_LOCK
static void ch_trigger_txku(QUIC_CHANNEL *ch)
{
    uint64_t next_pn
        = ossl_quic_tx_packetiser_get_next_pn(ch->txp, QUIC_PN_SPACE_APP);

    if (!ossl_quic_pn_valid(next_pn)
        || !ossl_qtx_trigger_key_update(ch->qtx)) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR, 0,
                                               "key update");
        return;
    }

    ch->txku_in_progress    = 1;
    ch->txku_pn             = next_pn;
    ch->rxku_expected       = ch->ku_locally_initiated;
}

QUIC_NEEDS_LOCK
static int txku_in_progress(QUIC_CHANNEL *ch)
{
    if (ch->txku_in_progress
        && ossl_ackm_get_largest_acked(ch->ackm, QUIC_PN_SPACE_APP) >= ch->txku_pn) {
        OSSL_TIME pto = ossl_ackm_get_pto_duration(ch->ackm);

        /*
         * RFC 9001 s. 6.5: Endpoints SHOULD wait three times the PTO before
         * initiating a key update after receiving an acknowledgment that
         * confirms that the previous key update was received.
         *
         * Note that by the above wording, this period starts from when we get
         * the ack for a TXKU-triggering packet, not when the TXKU is initiated.
         * So we defer TXKU cooldown deadline calculation to this point.
         */
        ch->txku_in_progress        = 0;
        ch->txku_cooldown_deadline  = ossl_time_add(get_time(ch),
                                                    ossl_time_multiply(pto, 3));
    }

    return ch->txku_in_progress;
}

QUIC_NEEDS_LOCK
static int txku_allowed(QUIC_CHANNEL *ch)
{
    return ch->tx_enc_level == QUIC_ENC_LEVEL_1RTT /* Sanity check. */
        /* Strict RFC 9001 criterion for TXKU. */
        && ch->handshake_confirmed
        && !txku_in_progress(ch);
}

QUIC_NEEDS_LOCK
static int txku_recommendable(QUIC_CHANNEL *ch)
{
    if (!txku_allowed(ch))
        return 0;

    return
        /* Recommended RFC 9001 criterion for TXKU. */
        ossl_time_compare(get_time(ch), ch->txku_cooldown_deadline) >= 0
        /* Some additional sensible criteria. */
        && !ch->rxku_in_progress
        && !ch->rxku_pending_confirm;
}

QUIC_NEEDS_LOCK
static int txku_desirable(QUIC_CHANNEL *ch)
{
    uint64_t cur_pkt_count, max_pkt_count, thresh_pkt_count;
    const uint32_t enc_level = QUIC_ENC_LEVEL_1RTT;

    /* Check AEAD limit to determine if we should perform a spontaneous TXKU. */
    cur_pkt_count = ossl_qtx_get_cur_epoch_pkt_count(ch->qtx, enc_level);
    max_pkt_count = ossl_qtx_get_max_epoch_pkt_count(ch->qtx, enc_level);

    thresh_pkt_count = max_pkt_count / 2;
    if (ch->txku_threshold_override != UINT64_MAX)
        thresh_pkt_count = ch->txku_threshold_override;

    return cur_pkt_count >= thresh_pkt_count;
}

QUIC_NEEDS_LOCK
static void ch_maybe_trigger_spontaneous_txku(QUIC_CHANNEL *ch)
{
    if (!txku_recommendable(ch) || !txku_desirable(ch))
        return;

    ch->ku_locally_initiated = 1;
    ch_trigger_txku(ch);
}

QUIC_NEEDS_LOCK
static int rxku_allowed(QUIC_CHANNEL *ch)
{
    /*
     * RFC 9001 s. 6.1: An endpoint MUST NOT initiate a key update prior to
     * having confirmed the handshake (Section 4.1.2).
     *
     * RFC 9001 s. 6.1: An endpoint MUST NOT initiate a subsequent key update
     * unless it has received an acknowledgment for a packet that was sent
     * protected with keys from the current key phase.
     *
     * RFC 9001 s. 6.2: If an endpoint detects a second update before it has
     * sent any packets with updated keys containing an acknowledgment for the
     * packet that initiated the key update, it indicates that its peer has
     * updated keys twice without awaiting confirmation. An endpoint MAY treat
     * such consecutive key updates as a connection error of type
     * KEY_UPDATE_ERROR.
     */
    return ch->handshake_confirmed && !ch->rxku_pending_confirm;
}

/*
 * Called when the QRX detects a new RX key update event.
 */
enum rxku_decision {
    DECISION_RXKU_ONLY,
    DECISION_PROTOCOL_VIOLATION,
    DECISION_SOLICITED_TXKU
};

/* Called when the QRX detects a key update has occurred. */
QUIC_NEEDS_LOCK
static void rxku_detected(QUIC_PN pn, void *arg)
{
    QUIC_CHANNEL *ch = arg;
    enum rxku_decision decision;
    OSSL_TIME pto;

    /*
     * Note: rxku_in_progress is always 0 here as an RXKU cannot be detected
     * when we are still in UPDATING or COOLDOWN (see quic_record_rx.h).
     */
    assert(!ch->rxku_in_progress);

    if (!rxku_allowed(ch))
        /* Is RXKU even allowed at this time? */
        decision = DECISION_PROTOCOL_VIOLATION;

    else if (ch->ku_locally_initiated)
        /*
         * If this key update was locally initiated (meaning that this detected
         * RXKU event is a result of our own spontaneous TXKU), we do not
         * trigger another TXKU; after all, to do so would result in an infinite
         * ping-pong of key updates. We still process it as an RXKU.
         */
        decision = DECISION_RXKU_ONLY;

    else
        /*
         * Otherwise, a peer triggering a KU means we have to trigger a KU also.
         */
        decision = DECISION_SOLICITED_TXKU;

    if (decision == DECISION_PROTOCOL_VIOLATION) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_KEY_UPDATE_ERROR,
                                               0, "RX key update again too soon");
        return;
    }

    pto = ossl_ackm_get_pto_duration(ch->ackm);

    ch->ku_locally_initiated        = 0;
    ch->rxku_in_progress            = 1;
    ch->rxku_pending_confirm        = 1;
    ch->rxku_trigger_pn             = pn;
    ch->rxku_update_end_deadline    = ossl_time_add(get_time(ch), pto);
    ch->rxku_expected               = 0;

    if (decision == DECISION_SOLICITED_TXKU)
        /* NOT gated by usual txku_allowed() */
        ch_trigger_txku(ch);

    /*
     * Ordinarily, we only generate ACK when some ACK-eliciting frame has been
     * received. In some cases, this may not occur for a long time, for example
     * if transmission of application data is going in only one direction and
     * nothing else is happening with the connection. However, since the peer
     * cannot initiate a subsequent (spontaneous) TXKU until its prior
     * (spontaneous or solicited) TXKU has completed - meaning that prior
     * TXKU's trigger packet (or subsequent packet) has been acknowledged, this
     * can lead to very long times before a TXKU is considered 'completed'.
     * Optimise this by forcing ACK generation after triggering TXKU.
     * (Basically, we consider a RXKU event something that is 'ACK-eliciting',
     * which it more or less should be; it is necessarily separate from ordinary
     * processing of ACK-eliciting frames as key update is not indicated via a
     * frame.)
     */
    ossl_quic_tx_packetiser_schedule_ack(ch->txp, QUIC_PN_SPACE_APP);
}

/* Called per tick to handle RXKU timer events. */
QUIC_NEEDS_LOCK
static void ch_rxku_tick(QUIC_CHANNEL *ch)
{
    if (!ch->rxku_in_progress
        || ossl_time_compare(get_time(ch), ch->rxku_update_end_deadline) < 0)
        return;

    ch->rxku_update_end_deadline    = ossl_time_infinite();
    ch->rxku_in_progress            = 0;

    if (!ossl_qrx_key_update_timeout(ch->qrx, /*normal=*/1))
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR, 0,
                                               "RXKU cooldown internal error");
}

QUIC_NEEDS_LOCK
static void ch_on_txp_ack_tx(const OSSL_QUIC_FRAME_ACK *ack, uint32_t pn_space,
                             void *arg)
{
    QUIC_CHANNEL *ch = arg;

    if (pn_space != QUIC_PN_SPACE_APP || !ch->rxku_pending_confirm
        || !ossl_quic_frame_ack_contains_pn(ack, ch->rxku_trigger_pn))
        return;

    /*
     * Defer clearing rxku_pending_confirm until TXP generate call returns
     * successfully.
     */
    ch->rxku_pending_confirm_done = 1;
}

/*
 * QUIC Channel: Handshake Layer Event Handling
 * ============================================
 */
static int ch_on_crypto_send(const unsigned char *buf, size_t buf_len,
                             size_t *consumed, void *arg)
{
    int ret;
    QUIC_CHANNEL *ch = arg;
    uint32_t enc_level = ch->tx_enc_level;
    uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    QUIC_SSTREAM *sstream = ch->crypto_send[pn_space];

    if (!ossl_assert(sstream != NULL))
        return 0;

    ret = ossl_quic_sstream_append(sstream, buf, buf_len, consumed);
    return ret;
}

static int crypto_ensure_empty(QUIC_RSTREAM *rstream)
{
    size_t avail = 0;
    int is_fin = 0;

    if (rstream == NULL)
        return 1;

    if (!ossl_quic_rstream_available(rstream, &avail, &is_fin))
        return 0;

    return avail == 0;
}

static int ch_on_crypto_recv_record(const unsigned char **buf,
                                    size_t *bytes_read, void *arg)
{
    QUIC_CHANNEL *ch = arg;
    QUIC_RSTREAM *rstream;
    int is_fin = 0; /* crypto stream is never finished, so we don't use this */
    uint32_t i;

    /*
     * After we move to a later EL we must not allow our peer to send any new
     * bytes in the crypto stream on a previous EL. Retransmissions of old bytes
     * are allowed.
     *
     * In practice we will only move to a new EL when we have consumed all bytes
     * which should be sent on the crypto stream at a previous EL. For example,
     * the Handshake EL should not be provisioned until we have completely
     * consumed a TLS 1.3 ServerHello. Thus when we provision an EL the output
     * of ossl_quic_rstream_available() should be 0 for all lower ELs. Thus if a
     * given EL is available we simply ensure we have not received any further
     * bytes at a lower EL.
     */
    for (i = QUIC_ENC_LEVEL_INITIAL; i < ch->rx_enc_level; ++i)
        if (i != QUIC_ENC_LEVEL_0RTT &&
            !crypto_ensure_empty(ch->crypto_recv[ossl_quic_enc_level_to_pn_space(i)])) {
            /* Protocol violation (RFC 9001 s. 4.1.3) */
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                   OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                                   "crypto stream data in wrong EL");
            return 0;
        }

    rstream = ch->crypto_recv[ossl_quic_enc_level_to_pn_space(ch->rx_enc_level)];
    if (rstream == NULL)
        return 0;

    return ossl_quic_rstream_get_record(rstream, buf, bytes_read,
                                        &is_fin);
}

static int ch_on_crypto_release_record(size_t bytes_read, void *arg)
{
    QUIC_CHANNEL *ch = arg;
    QUIC_RSTREAM *rstream;
    OSSL_RTT_INFO rtt_info;
    uint32_t rx_pn_space = ossl_quic_enc_level_to_pn_space(ch->rx_enc_level);

    rstream = ch->crypto_recv[rx_pn_space];
    if (rstream == NULL)
        return 0;

    ossl_statm_get_rtt_info(ossl_quic_channel_get_statm(ch), &rtt_info);
    if (!ossl_quic_rxfc_on_retire(&ch->crypto_rxfc[rx_pn_space], bytes_read,
                                  rtt_info.smoothed_rtt))
        return 0;

    return ossl_quic_rstream_release_record(rstream, bytes_read);
}

static int ch_on_handshake_yield_secret(uint32_t prot_level, int direction,
                                        uint32_t suite_id, EVP_MD *md,
                                        const unsigned char *secret,
                                        size_t secret_len,
                                        void *arg)
{
    QUIC_CHANNEL *ch = arg;
    uint32_t i;
    uint32_t enc_level;

    /* Convert TLS protection level to QUIC encryption level */
    switch (prot_level) {
    case OSSL_RECORD_PROTECTION_LEVEL_EARLY:
        enc_level = QUIC_ENC_LEVEL_0RTT;
        break;

    case OSSL_RECORD_PROTECTION_LEVEL_HANDSHAKE:
        enc_level = QUIC_ENC_LEVEL_HANDSHAKE;
        break;

    case OSSL_RECORD_PROTECTION_LEVEL_APPLICATION:
        enc_level = QUIC_ENC_LEVEL_1RTT;
        break;

    default:
        return 0;
    }

    if (enc_level < QUIC_ENC_LEVEL_HANDSHAKE || enc_level >= QUIC_ENC_LEVEL_NUM)
        /* Invalid EL. */
        return 0;


    if (direction) {
        /* TX */
        if (enc_level <= ch->tx_enc_level)
            /*
             * Does not make sense for us to try and provision an EL we have already
             * attained.
             */
            return 0;

        if (!ossl_qtx_provide_secret(ch->qtx, enc_level,
                                     suite_id, md,
                                     secret, secret_len))
            return 0;

        ch->tx_enc_level = enc_level;
    } else {
        /* RX */
        if (enc_level <= ch->rx_enc_level)
            /*
             * Does not make sense for us to try and provision an EL we have already
             * attained.
             */
            return 0;

        /*
         * Ensure all crypto streams for previous ELs are now empty of available
         * data.
         */
        for (i = QUIC_ENC_LEVEL_INITIAL; i < enc_level; ++i)
            if (!crypto_ensure_empty(ch->crypto_recv[ossl_quic_enc_level_to_pn_space(i)])) {
                /* Protocol violation (RFC 9001 s. 4.1.3) */
                ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                    OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                                    "crypto stream data in wrong EL");
                return 0;
            }

        if (!ossl_qrx_provide_secret(ch->qrx, enc_level,
                                     suite_id, md,
                                     secret, secret_len))
            return 0;

        ch->have_new_rx_secret = 1;
        ch->rx_enc_level = enc_level;
    }

    return 1;
}

static int ch_on_handshake_complete(void *arg)
{
    QUIC_CHANNEL *ch = arg;

    if (!ossl_assert(!ch->handshake_complete))
        return 0; /* this should not happen twice */

    if (!ossl_assert(ch->tx_enc_level == QUIC_ENC_LEVEL_1RTT))
        return 0;

    /*
     * When handshake is complete, we no longer need to abide by the
     * 3x amplification limit, though we should be validated as soon
     * as we see a handshake key encrypted packet (see ossl_quic_handle_packet)
     */
    ossl_quic_tx_packetiser_set_validated(ch->txp);

    if (!ch->got_remote_transport_params) {
        /*
         * Was not a valid QUIC handshake if we did not get valid transport
         * params.
         */
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_CRYPTO_MISSING_EXT,
                                               OSSL_QUIC_FRAME_TYPE_CRYPTO,
                                               "no transport parameters received");
        return 0;
    }

    /* Don't need transport parameters anymore. */
    OPENSSL_free(ch->local_transport_params);
    ch->local_transport_params = NULL;

    /* Tell the QRX it can now process 1-RTT packets. */
    ossl_qrx_allow_1rtt_processing(ch->qrx);

    /* Tell TXP the handshake is complete. */
    ossl_quic_tx_packetiser_notify_handshake_complete(ch->txp);

    ch->handshake_complete = 1;

    if (ch->pending_new_token != NULL) {
        /*
         * Note this is a best effort operation here
         * If scheduling a new token fails, the worst outcome is that
         * a client, not having received it, will just have to go through
         * an extra roundtrip on a subsequent connection via the retry frame
         * path, at which point we get another opportunity to schedule another
         * new token.  As a result, we don't need to handle any errors here
         */
        ossl_quic_channel_schedule_new_token(ch,
                                             ch->pending_new_token,
                                             ch->pending_new_token_len);
        OPENSSL_free(ch->pending_new_token);
        ch->pending_new_token = NULL;
        ch->pending_new_token_len = 0;
    }

    if (ch->is_server) {
        /*
         * On the server, the handshake is confirmed as soon as it is complete.
         */
        ossl_quic_channel_on_handshake_confirmed(ch);

        ossl_quic_tx_packetiser_schedule_handshake_done(ch->txp);
    }

    ch_record_state_transition(ch, ch->state);
    return 1;
}

static int ch_on_handshake_alert(void *arg, unsigned char alert_code)
{
    QUIC_CHANNEL *ch = arg;

    /*
     * RFC 9001 s. 4.4: More specifically, servers MUST NOT send post-handshake
     * TLS CertificateRequest messages, and clients MUST treat receipt of such
     * messages as a connection error of type PROTOCOL_VIOLATION.
     */
    if (alert_code == SSL_AD_UNEXPECTED_MESSAGE
            && ch->handshake_complete
            && ossl_quic_tls_is_cert_request(ch->qtls))
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               0,
                                               "Post-handshake TLS "
                                               "CertificateRequest received");
    /*
     * RFC 9001 s. 4.6.1: Servers MUST NOT send the early_data extension with a
     * max_early_data_size field set to any value other than 0xffffffff. A
     * client MUST treat receipt of a NewSessionTicket that contains an
     * early_data extension with any other value as a connection error of type
     * PROTOCOL_VIOLATION.
     */
    else if (alert_code == SSL_AD_ILLEGAL_PARAMETER
             && ch->handshake_complete
             && ossl_quic_tls_has_bad_max_early_data(ch->qtls))
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               0,
                                               "Bad max_early_data received");
    else
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_CRYPTO_ERR_BEGIN
                                               + alert_code,
                                               0, "handshake alert");

    return 1;
}

/*
 * QUIC Channel: Transport Parameter Handling
 * ==========================================
 */

/*
 * Called by handshake layer when we receive QUIC Transport Parameters from the
 * peer. Note that these are not authenticated until the handshake is marked
 * as complete.
 */
#define TP_REASON_SERVER_ONLY(x) \
    x " may not be sent by a client"
#define TP_REASON_DUP(x) \
    x " appears multiple times"
#define TP_REASON_MALFORMED(x) \
    x " is malformed"
#define TP_REASON_EXPECTED_VALUE(x) \
    x " does not match expected value"
#define TP_REASON_NOT_RETRY(x) \
    x " sent when not performing a retry"
#define TP_REASON_REQUIRED(x) \
    x " was not sent but is required"
#define TP_REASON_INTERNAL_ERROR(x) \
    x " encountered internal error"

static void txfc_bump_cwm_bidi(QUIC_STREAM *s, void *arg)
{
    if (!ossl_quic_stream_is_bidi(s)
        || ossl_quic_stream_is_server_init(s))
        return;

    ossl_quic_txfc_bump_cwm(&s->txfc, *(uint64_t *)arg);
}

static void txfc_bump_cwm_uni(QUIC_STREAM *s, void *arg)
{
    if (ossl_quic_stream_is_bidi(s)
        || ossl_quic_stream_is_server_init(s))
        return;

    ossl_quic_txfc_bump_cwm(&s->txfc, *(uint64_t *)arg);
}

static void do_update(QUIC_STREAM *s, void *arg)
{
    QUIC_CHANNEL *ch = arg;

    ossl_quic_stream_map_update_state(&ch->qsm, s);
}

static uint64_t min_u64_ignore_0(uint64_t a, uint64_t b)
{
    if (a == 0)
        return b;
    if (b == 0)
        return a;

    return a < b ? a : b;
}

static int ch_on_transport_params(const unsigned char *params,
                                  size_t params_len,
                                  void *arg)
{
    QUIC_CHANNEL *ch = arg;
    PACKET pkt;
    uint64_t id, v;
    size_t len;
    const unsigned char *body;
    int got_orig_dcid = 0;
    int got_initial_scid = 0;
    int got_retry_scid = 0;
    int got_initial_max_data = 0;
    int got_initial_max_stream_data_bidi_local = 0;
    int got_initial_max_stream_data_bidi_remote = 0;
    int got_initial_max_stream_data_uni = 0;
    int got_initial_max_streams_bidi = 0;
    int got_initial_max_streams_uni = 0;
    int got_stateless_reset_token = 0;
    int got_preferred_addr = 0;
    int got_ack_delay_exp = 0;
    int got_max_ack_delay = 0;
    int got_max_udp_payload_size = 0;
    int got_max_idle_timeout = 0;
    int got_active_conn_id_limit = 0;
    int got_disable_active_migration = 0;
    QUIC_CONN_ID cid;
    const char *reason = "bad transport parameter";
    ossl_unused uint64_t rx_max_idle_timeout = 0;
    ossl_unused const void *stateless_reset_token_p = NULL;
    QUIC_PREFERRED_ADDR pfa;

    if (ch->got_remote_transport_params) {
        reason = "multiple transport parameter extensions";
        goto malformed;
    }

    if (!PACKET_buf_init(&pkt, params, params_len)) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR, 0,
                                               "internal error (packet buf init)");
        return 0;
    }

    while (PACKET_remaining(&pkt) > 0) {
        if (!ossl_quic_wire_peek_transport_param(&pkt, &id))
            goto malformed;

        switch (id) {
        case QUIC_TPARAM_ORIG_DCID:
            if (got_orig_dcid) {
                reason = TP_REASON_DUP("ORIG_DCID");
                goto malformed;
            }

            if (ch->is_server) {
                reason = TP_REASON_SERVER_ONLY("ORIG_DCID");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_cid(&pkt, NULL, &cid)) {
                reason = TP_REASON_MALFORMED("ORIG_DCID");
                goto malformed;
            }

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
            /* Must match our initial DCID. */
            if (!ossl_quic_conn_id_eq(&ch->init_dcid, &cid)) {
                reason = TP_REASON_EXPECTED_VALUE("ORIG_DCID");
                goto malformed;
            }
#endif

            got_orig_dcid = 1;
            break;

        case QUIC_TPARAM_RETRY_SCID:
            if (ch->is_server) {
                reason = TP_REASON_SERVER_ONLY("RETRY_SCID");
                goto malformed;
            }

            if (got_retry_scid) {
                reason = TP_REASON_DUP("RETRY_SCID");
                goto malformed;
            }

            if (!ch->doing_retry) {
                reason = TP_REASON_NOT_RETRY("RETRY_SCID");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_cid(&pkt, NULL, &cid)) {
                reason = TP_REASON_MALFORMED("RETRY_SCID");
                goto malformed;
            }

            /* Must match Retry packet SCID. */
            if (!ossl_quic_conn_id_eq(&ch->retry_scid, &cid)) {
                reason = TP_REASON_EXPECTED_VALUE("RETRY_SCID");
                goto malformed;
            }

            got_retry_scid = 1;
            break;

        case QUIC_TPARAM_INITIAL_SCID:
            if (got_initial_scid) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_SCID");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_cid(&pkt, NULL, &cid)) {
                reason = TP_REASON_MALFORMED("INITIAL_SCID");
                goto malformed;
            }

            if (!ossl_quic_conn_id_eq(&ch->init_scid, &cid)) {
                reason = TP_REASON_EXPECTED_VALUE("INITIAL_SCID");
                goto malformed;
            }

            got_initial_scid = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_DATA:
            if (got_initial_max_data) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_DATA");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_DATA");
                goto malformed;
            }

            ossl_quic_txfc_bump_cwm(&ch->conn_txfc, v);
            got_initial_max_data = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_LOCAL:
            if (got_initial_max_stream_data_bidi_local) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_STREAM_DATA_BIDI_LOCAL");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_STREAM_DATA_BIDI_LOCAL");
                goto malformed;
            }

            /*
             * This is correct; the BIDI_LOCAL TP governs streams created by
             * the endpoint which sends the TP, i.e., our peer.
             */
            ch->rx_init_max_stream_data_bidi_remote = v;
            got_initial_max_stream_data_bidi_local = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_REMOTE:
            if (got_initial_max_stream_data_bidi_remote) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_STREAM_DATA_BIDI_REMOTE");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_STREAM_DATA_BIDI_REMOTE");
                goto malformed;
            }

            /*
             * This is correct; the BIDI_REMOTE TP governs streams created
             * by the endpoint which receives the TP, i.e., us.
             */
            ch->rx_init_max_stream_data_bidi_local = v;

            /* Apply to all existing streams. */
            ossl_quic_stream_map_visit(&ch->qsm, txfc_bump_cwm_bidi, &v);
            got_initial_max_stream_data_bidi_remote = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_UNI:
            if (got_initial_max_stream_data_uni) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_STREAM_DATA_UNI");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_STREAM_DATA_UNI");
                goto malformed;
            }

            ch->rx_init_max_stream_data_uni = v;

            /* Apply to all existing streams. */
            ossl_quic_stream_map_visit(&ch->qsm, txfc_bump_cwm_uni, &v);
            got_initial_max_stream_data_uni = 1;
            break;

        case QUIC_TPARAM_ACK_DELAY_EXP:
            if (got_ack_delay_exp) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("ACK_DELAY_EXP");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v > QUIC_MAX_ACK_DELAY_EXP) {
                reason = TP_REASON_MALFORMED("ACK_DELAY_EXP");
                goto malformed;
            }

            ch->rx_ack_delay_exp = (unsigned char)v;
            got_ack_delay_exp = 1;
            break;

        case QUIC_TPARAM_MAX_ACK_DELAY:
            if (got_max_ack_delay) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("MAX_ACK_DELAY");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v >= (((uint64_t)1) << 14)) {
                reason = TP_REASON_MALFORMED("MAX_ACK_DELAY");
                goto malformed;
            }

            ch->rx_max_ack_delay = v;
            ossl_ackm_set_rx_max_ack_delay(ch->ackm,
                                           ossl_ms2time(ch->rx_max_ack_delay));

            got_max_ack_delay = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_STREAMS_BIDI:
            if (got_initial_max_streams_bidi) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_STREAMS_BIDI");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v > (((uint64_t)1) << 60)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_STREAMS_BIDI");
                goto malformed;
            }

            assert(ch->max_local_streams_bidi == 0);
            ch->max_local_streams_bidi = v;
            got_initial_max_streams_bidi = 1;
            break;

        case QUIC_TPARAM_INITIAL_MAX_STREAMS_UNI:
            if (got_initial_max_streams_uni) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("INITIAL_MAX_STREAMS_UNI");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v > (((uint64_t)1) << 60)) {
                reason = TP_REASON_MALFORMED("INITIAL_MAX_STREAMS_UNI");
                goto malformed;
            }

            assert(ch->max_local_streams_uni == 0);
            ch->max_local_streams_uni = v;
            got_initial_max_streams_uni = 1;
            break;

        case QUIC_TPARAM_MAX_IDLE_TIMEOUT:
            if (got_max_idle_timeout) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("MAX_IDLE_TIMEOUT");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)) {
                reason = TP_REASON_MALFORMED("MAX_IDLE_TIMEOUT");
                goto malformed;
            }

            ch->max_idle_timeout_remote_req = v;

            ch->max_idle_timeout = min_u64_ignore_0(ch->max_idle_timeout_local_req,
                                                    ch->max_idle_timeout_remote_req);


            ch_update_idle(ch);
            got_max_idle_timeout = 1;
            rx_max_idle_timeout = v;
            break;

        case QUIC_TPARAM_MAX_UDP_PAYLOAD_SIZE:
            if (got_max_udp_payload_size) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("MAX_UDP_PAYLOAD_SIZE");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v < QUIC_MIN_INITIAL_DGRAM_LEN) {
                reason = TP_REASON_MALFORMED("MAX_UDP_PAYLOAD_SIZE");
                goto malformed;
            }

            ch->rx_max_udp_payload_size = v;
            got_max_udp_payload_size    = 1;
            break;

        case QUIC_TPARAM_ACTIVE_CONN_ID_LIMIT:
            if (got_active_conn_id_limit) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("ACTIVE_CONN_ID_LIMIT");
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_int(&pkt, &id, &v)
                || v < QUIC_MIN_ACTIVE_CONN_ID_LIMIT) {
                reason = TP_REASON_MALFORMED("ACTIVE_CONN_ID_LIMIT");
                goto malformed;
            }

            ch->rx_active_conn_id_limit = v;
            got_active_conn_id_limit = 1;
            break;

        case QUIC_TPARAM_STATELESS_RESET_TOKEN:
            if (got_stateless_reset_token) {
                reason = TP_REASON_DUP("STATELESS_RESET_TOKEN");
                goto malformed;
            }

            /*
             * RFC 9000 s. 18.2: This transport parameter MUST NOT be sent
             * by a client but MAY be sent by a server.
             */
            if (ch->is_server) {
                reason = TP_REASON_SERVER_ONLY("STATELESS_RESET_TOKEN");
                goto malformed;
            }

            body = ossl_quic_wire_decode_transport_param_bytes(&pkt, &id, &len);
            if (body == NULL || len != QUIC_STATELESS_RESET_TOKEN_LEN) {
                reason = TP_REASON_MALFORMED("STATELESS_RESET_TOKEN");
                goto malformed;
            }
            if (!ossl_quic_srtm_add(ch->srtm, ch, ch->cur_remote_seq_num,
                                    (const QUIC_STATELESS_RESET_TOKEN *)body)) {
                reason = TP_REASON_INTERNAL_ERROR("STATELESS_RESET_TOKEN");
                goto malformed;
            }

            stateless_reset_token_p     = body;
            got_stateless_reset_token   = 1;
            break;

        case QUIC_TPARAM_PREFERRED_ADDR:
            /* TODO(QUIC FUTURE): Handle preferred address. */
            if (got_preferred_addr) {
                reason = TP_REASON_DUP("PREFERRED_ADDR");
                goto malformed;
            }

            /*
             * RFC 9000 s. 18.2: "A server that chooses a zero-length
             * connection ID MUST NOT provide a preferred address.
             * Similarly, a server MUST NOT include a zero-length connection
             * ID in this transport parameter. A client MUST treat a
             * violation of these requirements as a connection error of type
             * TRANSPORT_PARAMETER_ERROR."
             */
            if (ch->is_server) {
                reason = TP_REASON_SERVER_ONLY("PREFERRED_ADDR");
                goto malformed;
            }

            if (ch->cur_remote_dcid.id_len == 0) {
                reason = "PREFERRED_ADDR provided for zero-length CID";
                goto malformed;
            }

            if (!ossl_quic_wire_decode_transport_param_preferred_addr(&pkt, &pfa)) {
                reason = TP_REASON_MALFORMED("PREFERRED_ADDR");
                goto malformed;
            }

            if (pfa.cid.id_len == 0) {
                reason = "zero-length CID in PREFERRED_ADDR";
                goto malformed;
            }

            got_preferred_addr = 1;
            break;

        case QUIC_TPARAM_DISABLE_ACTIVE_MIGRATION:
            /* We do not currently handle migration, so nothing to do. */
            if (got_disable_active_migration) {
                /* must not appear more than once */
                reason = TP_REASON_DUP("DISABLE_ACTIVE_MIGRATION");
                goto malformed;
            }

            body = ossl_quic_wire_decode_transport_param_bytes(&pkt, &id, &len);
            if (body == NULL || len > 0) {
                reason = TP_REASON_MALFORMED("DISABLE_ACTIVE_MIGRATION");
                goto malformed;
            }

            got_disable_active_migration = 1;
            break;

        default:
            /*
             * Skip over and ignore.
             *
             * RFC 9000 s. 7.4: We SHOULD treat duplicated transport parameters
             * as a connection error, but we are not required to. Currently,
             * handle this programmatically by checking for duplicates in the
             * parameters that we recognise, as above, but don't bother
             * maintaining a list of duplicates for anything we don't recognise.
             */
            body = ossl_quic_wire_decode_transport_param_bytes(&pkt, &id,
                                                               &len);
            if (body == NULL)
                goto malformed;

            break;
        }
    }

    if (!got_initial_scid) {
        reason = TP_REASON_REQUIRED("INITIAL_SCID");
        goto malformed;
    }

    if (!ch->is_server) {
        if (!got_orig_dcid) {
            reason = TP_REASON_REQUIRED("ORIG_DCID");
            goto malformed;
        }

        if (ch->doing_retry && !got_retry_scid) {
            reason = TP_REASON_REQUIRED("RETRY_SCID");
            goto malformed;
        }
    }

    ch->got_remote_transport_params = 1;

#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(ch_get_qlog(ch), transport, parameters_set)
        QLOG_STR("owner", "remote");

        if (got_orig_dcid)
            QLOG_CID("original_destination_connection_id",
                     &ch->init_dcid);
        if (got_initial_scid)
            QLOG_CID("original_source_connection_id",
                     &ch->init_dcid);
        if (got_retry_scid)
            QLOG_CID("retry_source_connection_id",
                     &ch->retry_scid);
        if (got_initial_max_data)
            QLOG_U64("initial_max_data",
                     ossl_quic_txfc_get_cwm(&ch->conn_txfc));
        if (got_initial_max_stream_data_bidi_local)
            QLOG_U64("initial_max_stream_data_bidi_local",
                     ch->rx_init_max_stream_data_bidi_local);
        if (got_initial_max_stream_data_bidi_remote)
            QLOG_U64("initial_max_stream_data_bidi_remote",
                     ch->rx_init_max_stream_data_bidi_remote);
        if (got_initial_max_stream_data_uni)
            QLOG_U64("initial_max_stream_data_uni",
                     ch->rx_init_max_stream_data_uni);
        if (got_initial_max_streams_bidi)
            QLOG_U64("initial_max_streams_bidi",
                     ch->max_local_streams_bidi);
        if (got_initial_max_streams_uni)
            QLOG_U64("initial_max_streams_uni",
                     ch->max_local_streams_uni);
        if (got_ack_delay_exp)
            QLOG_U64("ack_delay_exponent", ch->rx_ack_delay_exp);
        if (got_max_ack_delay)
            QLOG_U64("max_ack_delay", ch->rx_max_ack_delay);
        if (got_max_udp_payload_size)
            QLOG_U64("max_udp_payload_size", ch->rx_max_udp_payload_size);
        if (got_max_idle_timeout)
            QLOG_U64("max_idle_timeout", rx_max_idle_timeout);
        if (got_active_conn_id_limit)
            QLOG_U64("active_connection_id_limit", ch->rx_active_conn_id_limit);
        if (got_stateless_reset_token)
            QLOG_BIN("stateless_reset_token", stateless_reset_token_p,
                     QUIC_STATELESS_RESET_TOKEN_LEN);
        if (got_preferred_addr) {
            QLOG_BEGIN("preferred_addr")
                QLOG_U64("port_v4", pfa.ipv4_port);
                QLOG_U64("port_v6", pfa.ipv6_port);
                QLOG_BIN("ip_v4", pfa.ipv4, sizeof(pfa.ipv4));
                QLOG_BIN("ip_v6", pfa.ipv6, sizeof(pfa.ipv6));
                QLOG_BIN("stateless_reset_token", pfa.stateless_reset.token,
                         sizeof(pfa.stateless_reset.token));
                QLOG_CID("connection_id", &pfa.cid);
            QLOG_END()
        }
        QLOG_BOOL("disable_active_migration", got_disable_active_migration);
    QLOG_EVENT_END()
#endif

    if (got_initial_max_data || got_initial_max_stream_data_bidi_remote
        || got_initial_max_streams_bidi || got_initial_max_streams_uni)
        /*
         * If FC credit was bumped, we may now be able to send. Update all
         * streams.
         */
        ossl_quic_stream_map_visit(&ch->qsm, do_update, ch);

    /* If we are a server, we now generate our own transport parameters. */
    if (ch->is_server && !ch_generate_transport_params(ch)) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR, 0,
                                               "internal error");
        return 0;
    }

    return 1;

malformed:
    ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_TRANSPORT_PARAMETER_ERROR,
                                           0, reason);
    return 0;
}

/*
 * Called when we want to generate transport parameters. This is called
 * immediately at instantiation time for a client and after we receive the
 * client's transport parameters for a server.
 */
static int ch_generate_transport_params(QUIC_CHANNEL *ch)
{
    int ok = 0;
    BUF_MEM *buf_mem = NULL;
    WPACKET wpkt;
    int wpkt_valid = 0;
    size_t buf_len = 0;
    QUIC_CONN_ID *id_to_use = NULL;

    /*
     * We need to select which connection id to encode in the
     * QUIC_TPARAM_ORIG_DCID transport parameter
     * If we have an odcid, then this connection was established
     * in response to a retry request, and we need to use the connection
     * id sent in the first initial packet.
     * If we don't have an odcid, then this connection was established
     * without a retry and the init_dcid is the connection we should use
     */
    if (ch->odcid.id_len == 0)
        id_to_use = &ch->init_dcid;
    else
        id_to_use = &ch->odcid;

    if (ch->local_transport_params != NULL || ch->got_local_transport_params)
        goto err;

    if ((buf_mem = BUF_MEM_new()) == NULL)
        goto err;

    if (!WPACKET_init(&wpkt, buf_mem))
        goto err;

    wpkt_valid = 1;

    if (ossl_quic_wire_encode_transport_param_bytes(&wpkt, QUIC_TPARAM_DISABLE_ACTIVE_MIGRATION,
                                                    NULL, 0) == NULL)
        goto err;

    if (ch->is_server) {
        if (!ossl_quic_wire_encode_transport_param_cid(&wpkt, QUIC_TPARAM_ORIG_DCID,
                                                       id_to_use))
            goto err;

        if (!ossl_quic_wire_encode_transport_param_cid(&wpkt, QUIC_TPARAM_INITIAL_SCID,
                                                       &ch->cur_local_cid))
            goto err;
        if (ch->odcid.id_len != 0)
            if (!ossl_quic_wire_encode_transport_param_cid(&wpkt,
                                                           QUIC_TPARAM_RETRY_SCID,
                                                           &ch->init_dcid))
                goto err;
    } else {
        if (!ossl_quic_wire_encode_transport_param_cid(&wpkt, QUIC_TPARAM_INITIAL_SCID,
                                                       &ch->init_scid))
            goto err;
    }

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_MAX_IDLE_TIMEOUT,
                                                   ch->max_idle_timeout_local_req))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_MAX_UDP_PAYLOAD_SIZE,
                                                   QUIC_MIN_INITIAL_DGRAM_LEN))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_ACTIVE_CONN_ID_LIMIT,
                                                   QUIC_MIN_ACTIVE_CONN_ID_LIMIT))
        goto err;

    if (ch->tx_max_ack_delay != QUIC_DEFAULT_MAX_ACK_DELAY
        && !ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_MAX_ACK_DELAY,
                                                      ch->tx_max_ack_delay))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_DATA,
                                                   ossl_quic_rxfc_get_cwm(&ch->conn_rxfc)))
        goto err;

    /* Send the default CWM for a new RXFC. */
    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_LOCAL,
                                                   ch->tx_init_max_stream_data_bidi_local))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_REMOTE,
                                                   ch->tx_init_max_stream_data_bidi_remote))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_UNI,
                                                   ch->tx_init_max_stream_data_uni))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_STREAMS_BIDI,
                                                   ossl_quic_rxfc_get_cwm(&ch->max_streams_bidi_rxfc)))
        goto err;

    if (!ossl_quic_wire_encode_transport_param_int(&wpkt, QUIC_TPARAM_INITIAL_MAX_STREAMS_UNI,
                                                   ossl_quic_rxfc_get_cwm(&ch->max_streams_uni_rxfc)))
        goto err;

    if (!WPACKET_finish(&wpkt))
        goto err;

    wpkt_valid = 0;

    if (!WPACKET_get_total_written(&wpkt, &buf_len))
        goto err;

    ch->local_transport_params = (unsigned char *)buf_mem->data;
    buf_mem->data = NULL;

    if (!ossl_quic_tls_set_transport_params(ch->qtls, ch->local_transport_params,
                                            buf_len))
        goto err;

#ifndef OPENSSL_NO_QLOG
    QLOG_EVENT_BEGIN(ch_get_qlog(ch), transport, parameters_set)
        QLOG_STR("owner", "local");
        QLOG_BOOL("disable_active_migration", 1);
        if (ch->is_server) {
            QLOG_CID("original_destination_connection_id", &ch->init_dcid);
            QLOG_CID("initial_source_connection_id", &ch->cur_local_cid);
        } else {
            QLOG_STR("initial_source_connection_id", "");
        }
        QLOG_U64("max_idle_timeout", ch->max_idle_timeout);
        QLOG_U64("max_udp_payload_size", QUIC_MIN_INITIAL_DGRAM_LEN);
        QLOG_U64("active_connection_id_limit", QUIC_MIN_ACTIVE_CONN_ID_LIMIT);
        QLOG_U64("max_ack_delay", ch->tx_max_ack_delay);
        QLOG_U64("initial_max_data", ossl_quic_rxfc_get_cwm(&ch->conn_rxfc));
        QLOG_U64("initial_max_stream_data_bidi_local",
                 ch->tx_init_max_stream_data_bidi_local);
        QLOG_U64("initial_max_stream_data_bidi_remote",
                 ch->tx_init_max_stream_data_bidi_remote);
        QLOG_U64("initial_max_stream_data_uni",
                 ch->tx_init_max_stream_data_uni);
        QLOG_U64("initial_max_streams_bidi",
                 ossl_quic_rxfc_get_cwm(&ch->max_streams_bidi_rxfc));
        QLOG_U64("initial_max_streams_uni",
                 ossl_quic_rxfc_get_cwm(&ch->max_streams_uni_rxfc));
    QLOG_EVENT_END()
#endif

    ch->got_local_transport_params = 1;

    ok = 1;
err:
    if (wpkt_valid)
        WPACKET_cleanup(&wpkt);
    BUF_MEM_free(buf_mem);
    return ok;
}

/*
 * QUIC Channel: Ticker-Mutator
 * ============================
 */

/*
 * The central ticker function called by the reactor. This does everything, or
 * at least everything network I/O related. Best effort - not allowed to fail
 * "loudly".
 */
void ossl_quic_channel_subtick(QUIC_CHANNEL *ch, QUIC_TICK_RESULT *res,
                               uint32_t flags)
{
    OSSL_TIME now, deadline;
    int channel_only = (flags & QUIC_REACTOR_TICK_FLAG_CHANNEL_ONLY) != 0;
    int notify_other_threads = 0;

    /*
     * When we tick the QUIC connection, we do everything we need to do
     * periodically. Network I/O handling will already have been performed
     * as necessary by the QUIC port. Thus, in order, we:
     *
     *   - handle any packets the DEMUX has queued up for us;
     *   - handle any timer events which are due to fire (ACKM, etc.);
     *   - generate any packets which need to be sent;
     *   - determine the time at which we should next be ticked.
     */

    /*
     * If the connection has not yet started, or we are in the TERMINATED state,
     * there is nothing to do.
     */
    if (ch->state == QUIC_CHANNEL_STATE_IDLE
            || ossl_quic_channel_is_terminated(ch)) {
        res->net_read_desired       = 0;
        res->net_write_desired      = 0;
        res->notify_other_threads   = 0;
        res->tick_deadline          = ossl_time_infinite();
        return;
    }

    /*
     * If we are in the TERMINATING state, check if the terminating timer has
     * expired.
     */
    if (ossl_quic_channel_is_terminating(ch)) {
        now = get_time(ch);

        if (ossl_time_compare(now, ch->terminate_deadline) >= 0) {
            ch_on_terminating_timeout(ch);
            res->net_read_desired       = 0;
            res->net_write_desired      = 0;
            res->notify_other_threads   = 1;
            res->tick_deadline          = ossl_time_infinite();
            return; /* abort normal processing, nothing to do */
        }
    }

    if (!ch->port->engine->inhibit_tick) {
        /* Handle RXKU timeouts. */
        ch_rxku_tick(ch);

        do {
            /* Process queued incoming packets. */
            ch->did_tls_tick        = 0;
            ch->have_new_rx_secret  = 0;
            ch_rx(ch, channel_only, &notify_other_threads);

            /*
             * Allow the handshake layer to check for any new incoming data and
             * generate new outgoing data.
             */
            if (!ch->did_tls_tick)
                ch_tick_tls(ch, channel_only, &notify_other_threads);

            /*
             * If the handshake layer gave us a new secret, we need to do RX
             * again because packets that were not previously processable and
             * were deferred might now be processable.
             *
             * TODO(QUIC FUTURE): Consider handling this in the yield_secret callback.
             */
        } while (ch->have_new_rx_secret);
    }

    /*
     * Handle any timer events which are due to fire; namely, the loss
     * detection deadline and the idle timeout.
     *
     * ACKM ACK generation deadline is polled by TXP, so we don't need to
     * handle it here.
     */
    now = get_time(ch);
    if (ossl_time_compare(now, ch->idle_deadline) >= 0) {
        /*
         * Idle timeout differs from normal protocol violation because we do
         * not send a CONN_CLOSE frame; go straight to TERMINATED.
         */
        if (!ch->port->engine->inhibit_tick)
            ch_on_idle_timeout(ch);

        res->net_read_desired       = 0;
        res->net_write_desired      = 0;
        res->notify_other_threads   = 1;
        res->tick_deadline          = ossl_time_infinite();
        return;
    }

    if (!ch->port->engine->inhibit_tick) {
        deadline = ossl_ackm_get_loss_detection_deadline(ch->ackm);
        if (!ossl_time_is_zero(deadline)
            && ossl_time_compare(now, deadline) >= 0)
            ossl_ackm_on_timeout(ch->ackm);

        /* If a ping is due, inform TXP. */
        if (ossl_time_compare(now, ch->ping_deadline) >= 0) {
            int pn_space = ossl_quic_enc_level_to_pn_space(ch->tx_enc_level);

            ossl_quic_tx_packetiser_schedule_ack_eliciting(ch->txp, pn_space);

            /*
             * If we have no CC budget at this time we cannot process the above
             * PING request immediately. In any case we have scheduled the
             * request so bump the ping deadline. If we don't do this we will
             * busy-loop endlessly as the above deadline comparison condition
             * will still be met.
             */
            ch_update_ping_deadline(ch);
        }

        /* Queue any data to be sent for transmission. */
        ch_tx(ch, &notify_other_threads);

        /* Do stream GC. */
        ossl_quic_stream_map_gc(&ch->qsm);
    }

    /* Determine the time at which we should next be ticked. */
    res->tick_deadline = ch_determine_next_tick_deadline(ch);

    /*
     * Always process network input unless we are now terminated. Although we
     * had not terminated at the beginning of this tick, network errors in
     * ch_tx() may have caused us to transition to the Terminated state.
     */
    res->net_read_desired = !ossl_quic_channel_is_terminated(ch);

    /* We want to write to the network if we have any data in our TX queue. */
    res->net_write_desired
        = (!ossl_quic_channel_is_terminated(ch)
           && ossl_qtx_get_queue_len_datagrams(ch->qtx) > 0);

    res->notify_other_threads = notify_other_threads;
}

static int ch_tick_tls(QUIC_CHANNEL *ch, int channel_only, int *notify_other_threads)
{
    uint64_t error_code;
    const char *error_msg;
    ERR_STATE *error_state = NULL;

    if (channel_only)
        return 1;

    ch->did_tls_tick = 1;
    ossl_quic_tls_tick(ch->qtls);

    if (ossl_quic_tls_get_error(ch->qtls, &error_code, &error_msg,
                                &error_state)) {
        ossl_quic_channel_raise_protocol_error_state(ch, error_code, 0,
                                                     error_msg, error_state);
        if (notify_other_threads != NULL)
            *notify_other_threads = 1;

        return 0;
    }

    return 1;
}

/* Check incoming forged packet limit and terminate connection if needed. */
static void ch_rx_check_forged_pkt_limit(QUIC_CHANNEL *ch)
{
    uint32_t enc_level;
    uint64_t limit = UINT64_MAX, l;

    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level)
    {
        /*
         * Different ELs can have different AEADs which can in turn impose
         * different limits, so use the lowest value of any currently valid EL.
         */
        if ((ch->el_discarded & (1U << enc_level)) != 0)
            continue;

        if (enc_level > ch->rx_enc_level)
            break;

        l = ossl_qrx_get_max_forged_pkt_count(ch->qrx, enc_level);
        if (l < limit)
            limit = l;
    }

    if (ossl_qrx_get_cur_forged_pkt_count(ch->qrx) < limit)
        return;

    ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_AEAD_LIMIT_REACHED, 0,
                                           "forgery limit");
}

/* Process queued incoming packets and handle frames, if any. */
static int ch_rx(QUIC_CHANNEL *ch, int channel_only, int *notify_other_threads)
{
    int handled_any = 0;
    const int closing = ossl_quic_channel_is_closing(ch);

    if (!ch->is_server && !ch->have_sent_any_pkt)
        /*
         * We have not sent anything yet, therefore there is no need to check
         * for incoming data.
         */
        return 1;

    for (;;) {
        assert(ch->qrx_pkt == NULL);

        if (!ossl_qrx_read_pkt(ch->qrx, &ch->qrx_pkt))
            break;

        /* Track the amount of data received while in the closing state */
        if (closing)
            ossl_quic_tx_packetiser_record_received_closing_bytes(
                    ch->txp, ch->qrx_pkt->hdr->len);

        if (!handled_any) {
            ch_update_idle(ch);
            ch_update_ping_deadline(ch);
        }

        ch_rx_handle_packet(ch, channel_only); /* best effort */

        /*
         * Regardless of the outcome of frame handling, unref the packet.
         * This will free the packet unless something added another
         * reference to it during frame processing.
         */
        ossl_qrx_pkt_release(ch->qrx_pkt);
        ch->qrx_pkt = NULL;

        ch->have_sent_ack_eliciting_since_rx = 0;
        handled_any = 1;
    }

    ch_rx_check_forged_pkt_limit(ch);

    if (handled_any && notify_other_threads != NULL)
        *notify_other_threads = 1;

    /*
     * When in TERMINATING - CLOSING, generate a CONN_CLOSE frame whenever we
     * process one or more incoming packets.
     */
    if (handled_any && closing)
        ch->conn_close_queued = 1;

    return 1;
}

static int bio_addr_eq(const BIO_ADDR *a, const BIO_ADDR *b)
{
    if (BIO_ADDR_family(a) != BIO_ADDR_family(b))
        return 0;

    switch (BIO_ADDR_family(a)) {
        case AF_INET:
            return !memcmp(&a->s_in.sin_addr,
                           &b->s_in.sin_addr,
                           sizeof(a->s_in.sin_addr))
                && a->s_in.sin_port == b->s_in.sin_port;
#if OPENSSL_USE_IPV6
        case AF_INET6:
            return !memcmp(&a->s_in6.sin6_addr,
                           &b->s_in6.sin6_addr,
                           sizeof(a->s_in6.sin6_addr))
                && a->s_in6.sin6_port == b->s_in6.sin6_port;
#endif
        default:
            return 0; /* not supported */
    }

    return 1;
}

/* Handles the packet currently in ch->qrx_pkt->hdr. */
static void ch_rx_handle_packet(QUIC_CHANNEL *ch, int channel_only)
{
    uint32_t enc_level;
    int old_have_processed_any_pkt = ch->have_processed_any_pkt;
    OSSL_QTX_IOVEC iovec;
    PACKET vpkt;
    unsigned long supported_ver;

    assert(ch->qrx_pkt != NULL);

    /*
     * RFC 9000 s. 10.2.1 Closing Connection State:
     *      An endpoint that is closing is not required to process any
     *      received frame.
     */
    if (!ossl_quic_channel_is_active(ch))
        return;

    if (ossl_quic_pkt_type_is_encrypted(ch->qrx_pkt->hdr->type)) {
        if (!ch->have_received_enc_pkt) {
            ch->cur_remote_dcid = ch->init_scid = ch->qrx_pkt->hdr->src_conn_id;
            ch->have_received_enc_pkt = 1;

            /*
             * We change to using the SCID in the first Initial packet as the
             * DCID.
             */
            ossl_quic_tx_packetiser_set_cur_dcid(ch->txp, &ch->init_scid);
        }

        enc_level = ossl_quic_pkt_type_to_enc_level(ch->qrx_pkt->hdr->type);
        if ((ch->el_discarded & (1U << enc_level)) != 0)
            /* Do not process packets from ELs we have already discarded. */
            return;
    }

    /*
     * RFC 9000 s. 9.6: "If a client receives packets from a new server address
     * when the client has not initiated a migration to that address, the client
     * SHOULD discard these packets."
     *
     * We need to be a bit careful here as due to the BIO abstraction layer an
     * application is liable to be weird and lie to us about peer addresses.
     * Only apply this check if we actually are using a real AF_INET or AF_INET6
     * address.
     */
    if (!ch->is_server
        && ch->qrx_pkt->peer != NULL
        && (
               BIO_ADDR_family(&ch->cur_peer_addr) == AF_INET
#if OPENSSL_USE_IPV6
            || BIO_ADDR_family(&ch->cur_peer_addr) == AF_INET6
#endif
        )
        && !bio_addr_eq(ch->qrx_pkt->peer, &ch->cur_peer_addr))
        return;

    if (!ch->is_server
        && ch->have_received_enc_pkt
        && ossl_quic_pkt_type_has_scid(ch->qrx_pkt->hdr->type)) {
        /*
         * RFC 9000 s. 7.2: "Once a client has received a valid Initial packet
         * from the server, it MUST discard any subsequent packet it receives on
         * that connection with a different SCID."
         */
        if (!ossl_quic_conn_id_eq(&ch->qrx_pkt->hdr->src_conn_id,
                                  &ch->init_scid))
            return;
    }

    if (ossl_quic_pkt_type_has_version(ch->qrx_pkt->hdr->type)
        && ch->qrx_pkt->hdr->version != QUIC_VERSION_1)
        /*
         * RFC 9000 s. 5.2.1: If a client receives a packet that uses a
         * different version than it initially selected, it MUST discard the
         * packet. We only ever use v1, so require it.
         */
        return;

    if (ch->qrx_pkt->hdr->type == QUIC_PKT_TYPE_VERSION_NEG) {

        /*
         * Sanity check.  Version negotiation packet MUST have a version
         * value of 0 according to the RFC.  We must discard such packets
         */
        if (ch->qrx_pkt->hdr->version != 0)
            return;

        /*
         * RFC 9000 s. 6.2: If a client receives a version negotiation
         * packet, we need to do the following:
         * a) If the negotiation packet lists the version we initially sent
         *    then we must abandon this connection attempt
         * b) We have to select a version from the list provided in the
         *    version negotiation packet, and retry the connection attempt
         *    in much the same way that ch_retry does, but we can reuse the
         *    connection id values
         */

        if (old_have_processed_any_pkt == 1) {
            /*
             * We've gotten previous packets, need to discard this.
             */
            return;
        }

        /*
         * Indicate that we have processed a packet, as any subsequently
         * received version negotiation packet must be discarded above
         */
        ch->have_processed_any_pkt = 1;

        /*
         * Following the header, version negotiation packets
         * contain an array of 32 bit integers representing
         * the supported versions that the server honors
         * this array, bounded by the hdr->len field
         * needs to be traversed so that we can find a matching
         * version
         */
        if (!PACKET_buf_init(&vpkt, ch->qrx_pkt->hdr->data,
                             ch->qrx_pkt->hdr->len))
            return;

        while (PACKET_remaining(&vpkt) > 0) {
            /*
             * We only support quic version 1 at the moment, so
             * look to see if thats offered
             */
            if (!PACKET_get_net_4(&vpkt, &supported_ver))
                return;

            supported_ver = ntohl(supported_ver);
            if (supported_ver == QUIC_VERSION_1) {
                /*
                 * If the server supports version 1, set it as
                 * the packetisers version
                 */
                ossl_quic_tx_packetiser_set_protocol_version(ch->txp, QUIC_VERSION_1);

                /*
                 * And then request a restart of the QUIC connection 
                 */
                if (!ch_restart(ch))
                    ossl_quic_channel_raise_protocol_error(ch,
                                                           OSSL_QUIC_ERR_INTERNAL_ERROR,
                                                           0, "handling ver negotiation packet");
                return;
            }
        }

        /*
         * If we get here, then the server doesn't support a version of the
         * protocol that we can handle, abandon the connection
         */
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_CONNECTION_REFUSED,
                                               0, "unsupported protocol version");
        return;
    }

    ch->have_processed_any_pkt = 1;

    /*
     * RFC 9000 s. 17.2: "An endpoint MUST treat receipt of a packet that has a
     * non-zero value for [the reserved bits] after removing both packet and
     * header protection as a connection error of type PROTOCOL_VIOLATION."
     */
    if (ossl_quic_pkt_type_is_encrypted(ch->qrx_pkt->hdr->type)
        && ch->qrx_pkt->hdr->reserved != 0) {
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               0, "packet header reserved bits");
        return;
    }

    iovec.buf       = ch->qrx_pkt->hdr->data;
    iovec.buf_len   = ch->qrx_pkt->hdr->len;
    ossl_qlog_event_transport_packet_received(ch_get_qlog(ch), ch->qrx_pkt->hdr,
                                              ch->qrx_pkt->pn, &iovec, 1,
                                              ch->qrx_pkt->datagram_id);

    /* Handle incoming packet. */
    switch (ch->qrx_pkt->hdr->type) {
    case QUIC_PKT_TYPE_RETRY:
        if (ch->doing_retry || ch->is_server)
            /*
             * It is not allowed to ask a client to do a retry more than
             * once. Clients may not send retries.
             */
            return;

        /*
         * RFC 9000 s 17.2.5.2: After the client has received and processed an
         * Initial or Retry packet from the server, it MUST discard any
         * subsequent Retry packets that it receives.
         */
        if (ch->have_received_enc_pkt)
            return;

        if (ch->qrx_pkt->hdr->len <= QUIC_RETRY_INTEGRITY_TAG_LEN)
            /* Packets with zero-length Retry Tokens are invalid. */
            return;

        /*
         * TODO(QUIC FUTURE): Theoretically this should probably be in the QRX.
         * However because validation is dependent on context (namely the
         * client's initial DCID) we can't do this cleanly. In the future we
         * should probably add a callback to the QRX to let it call us (via
         * the DEMUX) and ask us about the correct original DCID, rather
         * than allow the QRX to emit a potentially malformed packet to the
         * upper layers. However, special casing this will do for now.
         */
        if (!ossl_quic_validate_retry_integrity_tag(ch->port->engine->libctx,
                                                    ch->port->engine->propq,
                                                    ch->qrx_pkt->hdr,
                                                    &ch->init_dcid))
            /* Malformed retry packet, ignore. */
            return;

        if (!ch_retry(ch, ch->qrx_pkt->hdr->data,
                      ch->qrx_pkt->hdr->len - QUIC_RETRY_INTEGRITY_TAG_LEN,
                      &ch->qrx_pkt->hdr->src_conn_id, old_have_processed_any_pkt))
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR,
                                                   0, "handling retry packet");
        break;

    case QUIC_PKT_TYPE_0RTT:
        if (!ch->is_server)
            /* Clients should never receive 0-RTT packets. */
            return;

        /*
         * TODO(QUIC 0RTT): Implement 0-RTT on the server side. We currently
         * do not need to implement this as a client can only do 0-RTT if we
         * have given it permission to in a previous session.
         */
        break;

    case QUIC_PKT_TYPE_INITIAL:
    case QUIC_PKT_TYPE_HANDSHAKE:
    case QUIC_PKT_TYPE_1RTT:
        if (ch->is_server && ch->qrx_pkt->hdr->type == QUIC_PKT_TYPE_HANDSHAKE)
            /*
             * We automatically drop INITIAL EL keys when first successfully
             * decrypting a HANDSHAKE packet, as per the RFC.
             */
            ch_discard_el(ch, QUIC_ENC_LEVEL_INITIAL);

        if (ch->rxku_in_progress
            && ch->qrx_pkt->hdr->type == QUIC_PKT_TYPE_1RTT
            && ch->qrx_pkt->pn >= ch->rxku_trigger_pn
            && ch->qrx_pkt->key_epoch < ossl_qrx_get_key_epoch(ch->qrx)) {
            /*
             * RFC 9001 s. 6.4: Packets with higher packet numbers MUST be
             * protected with either the same or newer packet protection keys
             * than packets with lower packet numbers. An endpoint that
             * successfully removes protection with old keys when newer keys
             * were used for packets with lower packet numbers MUST treat this
             * as a connection error of type KEY_UPDATE_ERROR.
             */
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_KEY_UPDATE_ERROR,
                                                   0, "new packet with old keys");
            break;
        }

        if (!ch->is_server
            && ch->qrx_pkt->hdr->type == QUIC_PKT_TYPE_INITIAL
            && ch->qrx_pkt->hdr->token_len > 0) {
            /*
             * RFC 9000 s. 17.2.2: Clients that receive an Initial packet with a
             * non-zero Token Length field MUST either discard the packet or
             * generate a connection error of type PROTOCOL_VIOLATION.
             *
             * TODO(QUIC FUTURE): consider the implications of RFC 9000 s. 10.2.3
             * Immediate Close during the Handshake:
             *      However, at the cost of reducing feedback about
             *      errors for legitimate peers, some forms of denial of
             *      service can be made more difficult for an attacker
             *      if endpoints discard illegal packets rather than
             *      terminating a connection with CONNECTION_CLOSE. For
             *      this reason, endpoints MAY discard packets rather
             *      than immediately close if errors are detected in
             *      packets that lack authentication.
             * I.e. should we drop this packet instead of closing the connection?
             */
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                                   0, "client received initial token");
            break;
        }

        /* This packet contains frames, pass to the RXDP. */
        ossl_quic_handle_frames(ch, ch->qrx_pkt); /* best effort */

        if (ch->did_crypto_frame)
            ch_tick_tls(ch, channel_only, NULL);

        break;

    case QUIC_PKT_TYPE_VERSION_NEG:
        /*
         * "A client MUST discard any Version Negotiation packet if it has
         * received and successfully processed any other packet."
         */
        if (!old_have_processed_any_pkt)
            ch_rx_handle_version_neg(ch, ch->qrx_pkt);

        break;

    default:
        assert(0);
        break;
    }

}

static void ch_rx_handle_version_neg(QUIC_CHANNEL *ch, OSSL_QRX_PKT *pkt)
{
    /*
     * We do not support version negotiation at this time. As per RFC 9000 s.
     * 6.2., we MUST abandon the connection attempt if we receive a Version
     * Negotiation packet, unless we have already successfully processed another
     * incoming packet, or the packet lists the QUIC version we want to use.
     */
    PACKET vpkt;
    unsigned long v;

    if (!PACKET_buf_init(&vpkt, pkt->hdr->data, pkt->hdr->len))
        return;

    while (PACKET_remaining(&vpkt) > 0) {
        if (!PACKET_get_net_4(&vpkt, &v))
            break;

        if ((uint32_t)v == QUIC_VERSION_1)
            return;
    }

    /* No match, this is a failure case. */
    ch_raise_version_neg_failure(ch);
}

static void ch_raise_version_neg_failure(QUIC_CHANNEL *ch)
{
    QUIC_TERMINATE_CAUSE tcause = {0};

    tcause.error_code = OSSL_QUIC_ERR_CONNECTION_REFUSED;
    tcause.reason     = "version negotiation failure";
    tcause.reason_len = strlen(tcause.reason);

    /*
     * Skip TERMINATING state; this is not considered a protocol error and we do
     * not send CONNECTION_CLOSE.
     */
    ch_start_terminating(ch, &tcause, 1);
}

/* Try to generate packets and if possible, flush them to the network. */
static int ch_tx(QUIC_CHANNEL *ch, int *notify_other_threads)
{
    QUIC_TXP_STATUS status;
    int res;

    /*
     * RFC 9000 s. 10.2.2: Draining Connection State:
     *      While otherwise identical to the closing state, an endpoint
     *      in the draining state MUST NOT send any packets.
     * and:
     *      An endpoint MUST NOT send further packets.
     */
    if (ossl_quic_channel_is_draining(ch))
        return 0;

    if (ossl_quic_channel_is_closing(ch)) {
        /*
         * While closing, only send CONN_CLOSE if we've received more traffic
         * from the peer. Once we tell the TXP to generate CONN_CLOSE, all
         * future calls to it generate CONN_CLOSE frames, so otherwise we would
         * just constantly generate CONN_CLOSE frames.
         *
         * Confirming to RFC 9000 s. 10.2.1 Closing Connection State:
         *      An endpoint SHOULD limit the rate at which it generates
         *      packets in the closing state.
         */
        if (!ch->conn_close_queued)
            return 0;

        ch->conn_close_queued = 0;
    }

    /* Do TXKU if we need to. */
    ch_maybe_trigger_spontaneous_txku(ch);

    ch->rxku_pending_confirm_done = 0;

    /* Loop until we stop generating packets to send */
    do {
        /*
        * Send packet, if we need to. Best effort. The TXP consults the CC and
        * applies any limitations imposed by it, so we don't need to do it here.
        *
        * Best effort. In particular if TXP fails for some reason we should
        * still flush any queued packets which we already generated.
        */
        res = ossl_quic_tx_packetiser_generate(ch->txp, &status);
        if (status.sent_pkt > 0) {
            ch->have_sent_any_pkt = 1; /* Packet(s) were sent */
            ch->port->have_sent_any_pkt = 1;

            /*
            * RFC 9000 s. 10.1. 'An endpoint also restarts its idle timer when
            * sending an ack-eliciting packet if no other ack-eliciting packets
            * have been sent since last receiving and processing a packet.'
            */
            if (status.sent_ack_eliciting
                    && !ch->have_sent_ack_eliciting_since_rx) {
                ch_update_idle(ch);
                ch->have_sent_ack_eliciting_since_rx = 1;
            }

            if (!ch->is_server && status.sent_handshake)
                /*
                * RFC 9001 s. 4.9.1: A client MUST discard Initial keys when it
                * first sends a Handshake packet.
                */
                ch_discard_el(ch, QUIC_ENC_LEVEL_INITIAL);

            if (ch->rxku_pending_confirm_done)
                ch->rxku_pending_confirm = 0;

            ch_update_ping_deadline(ch);
        }

        if (!res) {
            /*
            * One case where TXP can fail is if we reach a TX PN of 2**62 - 1.
            * As per RFC 9000 s. 12.3, if this happens we MUST close the
            * connection without sending a CONNECTION_CLOSE frame. This is
            * actually handled as an emergent consequence of our design, as the
            * TX packetiser will never transmit another packet when the TX PN
            * reaches the limit.
            *
            * Calling the below function terminates the connection; its attempt
            * to schedule a CONNECTION_CLOSE frame will not actually cause a
            * packet to be transmitted for this reason.
            */
            ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INTERNAL_ERROR,
                                                   0,
                                                   "internal error (txp generate)");
            break;
        }
    } while (status.sent_pkt > 0);

    /* Flush packets to network. */
    switch (ossl_qtx_flush_net(ch->qtx)) {
    case QTX_FLUSH_NET_RES_OK:
    case QTX_FLUSH_NET_RES_TRANSIENT_FAIL:
        /* Best effort, done for now. */
        break;

    case QTX_FLUSH_NET_RES_PERMANENT_FAIL:
    default:
        /* Permanent underlying network BIO, start terminating. */
        ossl_quic_port_raise_net_error(ch->port, ch);
        break;
    }

    /*
     * If we have datagrams we have yet to successfully transmit, we need to
     * notify other threads so that they can switch to polling on POLLOUT as
     * well as POLLIN.
     */
    if (ossl_qtx_get_queue_len_datagrams(ch->qtx) > 0)
        *notify_other_threads = 1;

    return 1;
}

/* Determine next tick deadline. */
static OSSL_TIME ch_determine_next_tick_deadline(QUIC_CHANNEL *ch)
{
    OSSL_TIME deadline;
    int i;

    if (ossl_quic_channel_is_terminated(ch))
        return ossl_time_infinite();

    deadline = ossl_ackm_get_loss_detection_deadline(ch->ackm);
    if (ossl_time_is_zero(deadline))
        deadline = ossl_time_infinite();

    /*
     * Check the ack deadline for all enc_levels that are actually provisioned.
     * ACKs aren't restricted by CC.
     */
    for (i = 0; i < QUIC_ENC_LEVEL_NUM; i++) {
        if (ossl_qtx_is_enc_level_provisioned(ch->qtx, i)) {
            deadline = ossl_time_min(deadline,
                                     ossl_ackm_get_ack_deadline(ch->ackm,
                                                                ossl_quic_enc_level_to_pn_space(i)));
        }
    }

    /*
     * When do we need to send an ACK-eliciting packet to reset the idle
     * deadline timer for the peer?
     */
    if (!ossl_time_is_infinite(ch->ping_deadline))
        deadline = ossl_time_min(deadline, ch->ping_deadline);

    /* Apply TXP wakeup deadline. */
    deadline = ossl_time_min(deadline,
                             ossl_quic_tx_packetiser_get_deadline(ch->txp));

    /* Is the terminating timer armed? */
    if (ossl_quic_channel_is_terminating(ch))
        deadline = ossl_time_min(deadline,
                                 ch->terminate_deadline);
    else if (!ossl_time_is_infinite(ch->idle_deadline))
        deadline = ossl_time_min(deadline,
                                 ch->idle_deadline);

    /* When does the RXKU process complete? */
    if (ch->rxku_in_progress)
        deadline = ossl_time_min(deadline, ch->rxku_update_end_deadline);

    return deadline;
}

/*
 * QUIC Channel: Lifecycle Events
 * ==============================
 */

/*
 * Record a state transition. This is not necessarily a change to ch->state but
 * also includes the handshake becoming complete or confirmed, etc.
 */
static void ch_record_state_transition(QUIC_CHANNEL *ch, uint32_t new_state)
{
    uint32_t old_state = ch->state;

    ch->state = new_state;

    ossl_qlog_event_connectivity_connection_state_updated(ch_get_qlog(ch),
                                                          old_state,
                                                          new_state,
                                                          ch->handshake_complete,
                                                          ch->handshake_confirmed);
}

static void free_peer_token(const unsigned char *token,
                            size_t token_len, void *arg)
{
    ossl_quic_free_peer_token((QUIC_TOKEN *)arg);
}

int ossl_quic_channel_start(QUIC_CHANNEL *ch)
{
    QUIC_TOKEN *token;

    if (ch->is_server)
        /*
         * This is not used by the server. The server moves to active
         * automatically on receiving an incoming connection.
         */
        return 0;

    if (ch->state != QUIC_CHANNEL_STATE_IDLE)
        /* Calls to connect are idempotent */
        return 1;

    /* Inform QTX of peer address. */
    if (!ossl_quic_tx_packetiser_set_peer(ch->txp, &ch->cur_peer_addr))
        return 0;

    /*
     * Look to see if we have a token, and if so, set it on the packetiser
     */
    if (!ch->is_server
        && ossl_quic_get_peer_token(ch->port->channel_ctx,
                                    &ch->cur_peer_addr,
                                    &token)
        && !ossl_quic_tx_packetiser_set_initial_token(ch->txp, token->token,
                                                      token->token_len,
                                                      free_peer_token,
                                                      token))
        free_peer_token(NULL, 0, token);

    /* Plug in secrets for the Initial EL. */
    if (!ossl_quic_provide_initial_secret(ch->port->engine->libctx,
                                          ch->port->engine->propq,
                                          &ch->init_dcid,
                                          ch->is_server,
                                          ch->qrx, ch->qtx))
        return 0;

    /*
     * Determine the QUIC Transport Parameters and serialize the transport
     * parameters block. (For servers, we do this later as we must defer
     * generation until we have received the client's transport parameters.)
     */
    if (!ch->is_server && !ch->got_local_transport_params
        && !ch_generate_transport_params(ch))
        return 0;

    /* Change state. */
    ch_record_state_transition(ch, QUIC_CHANNEL_STATE_ACTIVE);
    ch->doing_proactive_ver_neg = 0; /* not currently supported */

    ossl_qlog_event_connectivity_connection_started(ch_get_qlog(ch),
                                                    &ch->init_dcid);

    /* Handshake layer: start (e.g. send CH). */
    if (!ch_tick_tls(ch, /*channel_only=*/0, NULL))
        return 0;

    ossl_quic_reactor_tick(ossl_quic_port_get0_reactor(ch->port), 0); /* best effort */
    return 1;
}

static void free_token(const unsigned char *token, size_t token_len, void *arg)
{
    OPENSSL_free((char *)token);
}

/* Start a locally initiated connection shutdown. */
void ossl_quic_channel_local_close(QUIC_CHANNEL *ch, uint64_t app_error_code,
                                   const char *app_reason)
{
    QUIC_TERMINATE_CAUSE tcause = {0};

    if (ossl_quic_channel_is_term_any(ch))
        return;

    tcause.app          = 1;
    tcause.error_code   = app_error_code;
    tcause.reason       = app_reason;
    tcause.reason_len   = app_reason != NULL ? strlen(app_reason) : 0;
    ch_start_terminating(ch, &tcause, 0);
}

/**
 * ch_restart - Restarts the QUIC channel by simulating loss of the initial
 * packet. This forces the packet to be regenerated with the updated protocol
 * version number.
 *
 * @ch: Pointer to the QUIC_CHANNEL structure.
 *
 * Returns 1 on success, 0 on failure.
 */
static int ch_restart(QUIC_CHANNEL *ch)
{
    /*
     * Just pretend we lost our initial packet, so it gets
     * regenerated, with our updated protocol version number
     */
   return ossl_ackm_mark_packet_pseudo_lost(ch->ackm, QUIC_PN_SPACE_INITIAL,
                                            /* PN= */ 0);
}

/* Called when a server asks us to do a retry. */
static int ch_retry(QUIC_CHANNEL *ch,
                    const unsigned char *retry_token,
                    size_t retry_token_len,
                    const QUIC_CONN_ID *retry_scid,
                    int drop_later_pn)
{
    void *buf;
    QUIC_PN pn = 0;

    /*
     * RFC 9000 s. 17.2.5.1: "A client MUST discard a Retry packet that contains
     * a SCID field that is identical to the DCID field of its initial packet."
     */
    if (ossl_quic_conn_id_eq(&ch->init_dcid, retry_scid))
        return 1;

    /* We change to using the SCID in the Retry packet as the DCID. */
    if (!ossl_quic_tx_packetiser_set_cur_dcid(ch->txp, retry_scid))
        return 0;

    /*
     * Now we retry. We will release the Retry packet immediately, so copy
     * the token.
     */
    if ((buf = OPENSSL_memdup(retry_token, retry_token_len)) == NULL)
        return 0;

    if (!ossl_quic_tx_packetiser_set_initial_token(ch->txp, buf,
                                                   retry_token_len,
                                                   free_token, NULL)) {
        /*
         * This may fail if the token we receive is too big for us to ever be
         * able to transmit in an outgoing Initial packet.
         */
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_INVALID_TOKEN, 0,
                                               "received oversize token");
        OPENSSL_free(buf);
        return 0;
    }

    ch->retry_scid  = *retry_scid;
    ch->doing_retry = 1;

    /*
     * If a retry isn't our first response, we need to drop packet number
     * one instead (i.e. the case where we did version negotiation first
     */
    if (drop_later_pn == 1)
        pn = 1;

    /*
     * We need to stimulate the Initial EL to generate the first CRYPTO frame
     * again. We can do this most cleanly by simply forcing the ACKM to consider
     * the first Initial packet as lost, which it effectively was as the server
     * hasn't processed it. This also maintains the desired behaviour with e.g.
     * PNs not resetting and so on.
     *
     * The PN we used initially is always zero, because QUIC does not allow
     * repeated retries.
     */
    if (!ossl_ackm_mark_packet_pseudo_lost(ch->ackm, QUIC_PN_SPACE_INITIAL,
                                           pn))
        return 0;

    /*
     * Plug in new secrets for the Initial EL. This is the only time we change
     * the secrets for an EL after we already provisioned it.
     */
    if (!ossl_quic_provide_initial_secret(ch->port->engine->libctx,
                                          ch->port->engine->propq,
                                          &ch->retry_scid,
                                          /*is_server=*/0,
                                          ch->qrx, ch->qtx))
        return 0;

    return 1;
}

/* Called when an EL is to be discarded. */
static int ch_discard_el(QUIC_CHANNEL *ch,
                         uint32_t enc_level)
{
    if (!ossl_assert(enc_level < QUIC_ENC_LEVEL_1RTT))
        return 0;

    if ((ch->el_discarded & (1U << enc_level)) != 0)
        /* Already done. */
        return 1;

    /* Best effort for all of these. */
    ossl_quic_tx_packetiser_discard_enc_level(ch->txp, enc_level);
    ossl_qrx_discard_enc_level(ch->qrx, enc_level);
    ossl_qtx_discard_enc_level(ch->qtx, enc_level);

    if (enc_level != QUIC_ENC_LEVEL_0RTT) {
        uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);

        ossl_ackm_on_pkt_space_discarded(ch->ackm, pn_space);

        /* We should still have crypto streams at this point. */
        if (!ossl_assert(ch->crypto_send[pn_space] != NULL)
            || !ossl_assert(ch->crypto_recv[pn_space] != NULL))
            return 0;

        /* Get rid of the crypto stream state for the EL. */
        ossl_quic_sstream_free(ch->crypto_send[pn_space]);
        ch->crypto_send[pn_space] = NULL;

        ossl_quic_rstream_free(ch->crypto_recv[pn_space]);
        ch->crypto_recv[pn_space] = NULL;
    }

    ch->el_discarded |= (1U << enc_level);
    return 1;
}

/* Intended to be called by the RXDP. */
int ossl_quic_channel_on_handshake_confirmed(QUIC_CHANNEL *ch)
{
    if (ch->handshake_confirmed)
        return 1;

    if (!ch->handshake_complete) {
        /*
         * Does not make sense for handshake to be confirmed before it is
         * completed.
         */
        ossl_quic_channel_raise_protocol_error(ch, OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE,
                                               "handshake cannot be confirmed "
                                               "before it is completed");
        return 0;
    }

    ch_discard_el(ch, QUIC_ENC_LEVEL_HANDSHAKE);
    ch->handshake_confirmed = 1;
    ch_record_state_transition(ch, ch->state);
    ossl_ackm_on_handshake_confirmed(ch->ackm);
    return 1;
}

/*
 * Master function used when we want to start tearing down a connection:
 *
 *   - If the connection is still IDLE we can go straight to TERMINATED;
 *
 *   - If we are already TERMINATED this is a no-op.
 *
 *   - If we are TERMINATING - CLOSING and we have now got a CONNECTION_CLOSE
 *     from the peer (tcause->remote == 1), we move to TERMINATING - DRAINING.
 *
 *   - If we are TERMINATING - DRAINING, we remain here until the terminating
 *     timer expires.
 *
 *   - Otherwise, we are in ACTIVE and move to TERMINATING - CLOSING.
 *     if we caused the termination (e.g. we have sent a CONNECTION_CLOSE). Note
 *     that we are considered to have caused a termination if we sent the first
 *     CONNECTION_CLOSE frame, even if it is caused by a peer protocol
 *     violation. If the peer sent the first CONNECTION_CLOSE frame, we move to
 *     TERMINATING - DRAINING.
 *
 * We record the termination cause structure passed on the first call only.
 * Any successive calls have their termination cause data discarded;
 * once we start sending a CONNECTION_CLOSE frame, we don't change the details
 * in it.
 *
 * This conforms to RFC 9000 s. 10.2.1: Closing Connection State:
 *      To minimize the state that an endpoint maintains for a closing
 *      connection, endpoints MAY send the exact same packet in response
 *      to any received packet.
 *
 * We don't drop any connection state (specifically packet protection keys)
 * even though we are permitted to.  This conforms to RFC 9000 s. 10.2.1:
 * Closing Connection State:
 *       An endpoint MAY retain packet protection keys for incoming
 *       packets to allow it to read and process a CONNECTION_CLOSE frame.
 *
 * Note that we do not conform to these two from the same section:
 *      An endpoint's selected connection ID and the QUIC version
 *      are sufficient information to identify packets for a closing
 *      connection; the endpoint MAY discard all other connection state.
 * and:
 *      An endpoint MAY drop packet protection keys when entering the
 *      closing state and send a packet containing a CONNECTION_CLOSE
 *      frame in response to any UDP datagram that is received.
 */
static void copy_tcause(QUIC_TERMINATE_CAUSE *dst,
                        const QUIC_TERMINATE_CAUSE *src)
{
    dst->error_code = src->error_code;
    dst->frame_type = src->frame_type;
    dst->app        = src->app;
    dst->remote     = src->remote;

    dst->reason     = NULL;
    dst->reason_len = 0;

    if (src->reason != NULL && src->reason_len > 0) {
        size_t l = src->reason_len;
        char *r;

        if (l >= SIZE_MAX)
            --l;

        /*
         * If this fails, dst->reason becomes NULL and we simply do not use a
         * reason. This ensures termination is infallible.
         */
        dst->reason = r = OPENSSL_memdup(src->reason, l + 1);
        if (r == NULL)
            return;

        r[l]  = '\0';
        dst->reason_len = l;
    }
}

static void ch_start_terminating(QUIC_CHANNEL *ch,
                                 const QUIC_TERMINATE_CAUSE *tcause,
                                 int force_immediate)
{
    /* No point sending anything if we haven't sent anything yet. */
    if (!ch->have_sent_any_pkt)
        force_immediate = 1;

    switch (ch->state) {
    default:
    case QUIC_CHANNEL_STATE_IDLE:
        copy_tcause(&ch->terminate_cause, tcause);
        ch_on_terminating_timeout(ch);
        break;

    case QUIC_CHANNEL_STATE_ACTIVE:
        copy_tcause(&ch->terminate_cause, tcause);

        ossl_qlog_event_connectivity_connection_closed(ch_get_qlog(ch), tcause);

        if (!force_immediate) {
            ch_record_state_transition(ch, tcause->remote
                                           ? QUIC_CHANNEL_STATE_TERMINATING_DRAINING
                                           : QUIC_CHANNEL_STATE_TERMINATING_CLOSING);
            /*
             * RFC 9000 s. 10.2 Immediate Close
             *  These states SHOULD persist for at least three times
             *  the current PTO interval as defined in [QUIC-RECOVERY].
             */
            ch->terminate_deadline
                = ossl_time_add(get_time(ch),
                                ossl_time_multiply(ossl_ackm_get_pto_duration(ch->ackm),
                                                   3));

            if (!tcause->remote) {
                OSSL_QUIC_FRAME_CONN_CLOSE f = {0};

                /* best effort */
                f.error_code = ch->terminate_cause.error_code;
                f.frame_type = ch->terminate_cause.frame_type;
                f.is_app     = ch->terminate_cause.app;
                f.reason     = (char *)ch->terminate_cause.reason;
                f.reason_len = ch->terminate_cause.reason_len;
                ossl_quic_tx_packetiser_schedule_conn_close(ch->txp, &f);
                /*
                 * RFC 9000 s. 10.2.2 Draining Connection State:
                 *  An endpoint that receives a CONNECTION_CLOSE frame MAY
                 *  send a single packet containing a CONNECTION_CLOSE
                 *  frame before entering the draining state, using a
                 *  NO_ERROR code if appropriate
                 */
                ch->conn_close_queued = 1;
            }
        } else {
            ch_on_terminating_timeout(ch);
        }
        break;

    case QUIC_CHANNEL_STATE_TERMINATING_CLOSING:
        if (force_immediate)
            ch_on_terminating_timeout(ch);
        else if (tcause->remote)
            /*
             * RFC 9000 s. 10.2.2 Draining Connection State:
             *  An endpoint MAY enter the draining state from the
             *  closing state if it receives a CONNECTION_CLOSE frame,
             *  which indicates that the peer is also closing or draining.
             */
            ch_record_state_transition(ch, QUIC_CHANNEL_STATE_TERMINATING_DRAINING);

        break;

    case QUIC_CHANNEL_STATE_TERMINATING_DRAINING:
        /*
         * Other than in the force-immediate case, we remain here until the
         * timeout expires.
         */
        if (force_immediate)
            ch_on_terminating_timeout(ch);

        break;

    case QUIC_CHANNEL_STATE_TERMINATED:
        /* No-op. */
        break;
    }
}

/* For RXDP use. */
void ossl_quic_channel_on_remote_conn_close(QUIC_CHANNEL *ch,
                                            OSSL_QUIC_FRAME_CONN_CLOSE *f)
{
    QUIC_TERMINATE_CAUSE tcause = {0};

    if (!ossl_quic_channel_is_active(ch))
        return;

    tcause.remote     = 1;
    tcause.app        = f->is_app;
    tcause.error_code = f->error_code;
    tcause.frame_type = f->frame_type;
    tcause.reason     = f->reason;
    tcause.reason_len = f->reason_len;
    ch_start_terminating(ch, &tcause, 0);
}

static void free_frame_data(unsigned char *buf, size_t buf_len, void *arg)
{
    OPENSSL_free(buf);
}

static int ch_enqueue_retire_conn_id(QUIC_CHANNEL *ch, uint64_t seq_num)
{
    BUF_MEM *buf_mem = NULL;
    WPACKET wpkt;
    size_t l;

    ossl_quic_srtm_remove(ch->srtm, ch, seq_num);

    if ((buf_mem = BUF_MEM_new()) == NULL)
        goto err;

    if (!WPACKET_init(&wpkt, buf_mem))
        goto err;

    if (!ossl_quic_wire_encode_frame_retire_conn_id(&wpkt, seq_num)) {
        WPACKET_cleanup(&wpkt);
        goto err;
    }

    WPACKET_finish(&wpkt);
    if (!WPACKET_get_total_written(&wpkt, &l))
        goto err;

    if (ossl_quic_cfq_add_frame(ch->cfq, 1, QUIC_PN_SPACE_APP,
                                OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID, 0,
                                (unsigned char *)buf_mem->data, l,
                                free_frame_data, NULL) == NULL)
        goto err;

    buf_mem->data = NULL;
    BUF_MEM_free(buf_mem);
    return 1;

err:
    ossl_quic_channel_raise_protocol_error(ch,
                                           OSSL_QUIC_ERR_INTERNAL_ERROR,
                                           OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                                           "internal error enqueueing retire conn id");
    BUF_MEM_free(buf_mem);
    return 0;
}

void ossl_quic_channel_on_new_conn_id(QUIC_CHANNEL *ch,
                                      OSSL_QUIC_FRAME_NEW_CONN_ID *f)
{
    uint64_t new_remote_seq_num = ch->cur_remote_seq_num;
    uint64_t new_retire_prior_to = ch->cur_retire_prior_to;

    if (!ossl_quic_channel_is_active(ch))
        return;

    /* We allow only two active connection ids; first check some constraints */
    if (ch->cur_remote_dcid.id_len == 0) {
        /* Changing from 0 length connection id is disallowed */
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_PROTOCOL_VIOLATION,
                                               OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                                               "zero length connection id in use");

        return;
    }

    if (f->seq_num > new_remote_seq_num)
        new_remote_seq_num = f->seq_num;
    if (f->retire_prior_to > new_retire_prior_to)
        new_retire_prior_to = f->retire_prior_to;

    /*
     * RFC 9000-5.1.1: An endpoint MUST NOT provide more connection IDs
     * than the peer's limit.
     *
     * After processing a NEW_CONNECTION_ID frame and adding and retiring
     * active connection IDs, if the number of active connection IDs exceeds
     * the value advertised in its active_connection_id_limit transport
     * parameter, an endpoint MUST close the connection with an error of
     * type CONNECTION_ID_LIMIT_ERROR.
     */
    if (new_remote_seq_num - new_retire_prior_to > 1) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_CONNECTION_ID_LIMIT_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                                               "active_connection_id limit violated");
        return;
    }

    /*
     * RFC 9000-5.1.1: An endpoint MAY send connection IDs that temporarily
     * exceed a peer's limit if the NEW_CONNECTION_ID frame also requires
     * the retirement of any excess, by including a sufficiently large
     * value in the Retire Prior To field.
     *
     * RFC 9000-5.1.2: An endpoint SHOULD allow for sending and tracking
     * a number of RETIRE_CONNECTION_ID frames of at least twice the value
     * of the active_connection_id_limit transport parameter.  An endpoint
     * MUST NOT forget a connection ID without retiring it, though it MAY
     * choose to treat having connection IDs in need of retirement that
     * exceed this limit as a connection error of type CONNECTION_ID_LIMIT_ERROR.
     *
     * We are a little bit more liberal than the minimum mandated.
     */
    if (new_retire_prior_to - ch->cur_retire_prior_to > 10) {
        ossl_quic_channel_raise_protocol_error(ch,
                                               OSSL_QUIC_ERR_CONNECTION_ID_LIMIT_ERROR,
                                               OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                                               "retiring connection id limit violated");

        return;
    }

    if (new_remote_seq_num > ch->cur_remote_seq_num) {
        /* Add new stateless reset token */
        if (!ossl_quic_srtm_add(ch->srtm, ch, new_remote_seq_num,
                                &f->stateless_reset)) {
            ossl_quic_channel_raise_protocol_error(
                    ch, OSSL_QUIC_ERR_CONNECTION_ID_LIMIT_ERROR,
                    OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID,
                    "unable to store stateless reset token");

            return;
        }
        ch->cur_remote_seq_num = new_remote_seq_num;
        ch->cur_remote_dcid = f->conn_id;
        ossl_quic_tx_packetiser_set_cur_dcid(ch->txp, &ch->cur_remote_dcid);
    }

    /*
     * RFC 9000-5.1.2: Upon receipt of an increased Retire Prior To
     * field, the peer MUST stop using the corresponding connection IDs
     * and retire them with RETIRE_CONNECTION_ID frames before adding the
     * newly provided connection ID to the set of active connection IDs.
     */

    /*
     * Note: RFC 9000 s. 19.15 says:
     *   "An endpoint that receives a NEW_CONNECTION_ID frame with a sequence
     *    number smaller than the Retire Prior To field of a previously received
     *    NEW_CONNECTION_ID frame MUST send a corresponding
     *    RETIRE_CONNECTION_ID frame that retires the newly received connection
     *    ID, unless it has already done so for that sequence number."
     *
     * Since we currently always queue RETIRE_CONN_ID frames based on the Retire
     * Prior To field of a NEW_CONNECTION_ID frame immediately upon receiving
     * that NEW_CONNECTION_ID frame, by definition this will always be met.
     * This may change in future when we change our CID handling.
     */
    while (new_retire_prior_to > ch->cur_retire_prior_to) {
        if (!ch_enqueue_retire_conn_id(ch, ch->cur_retire_prior_to))
            break;
        ++ch->cur_retire_prior_to;
    }
}

static void ch_save_err_state(QUIC_CHANNEL *ch)
{
    if (ch->err_state == NULL)
        ch->err_state = OSSL_ERR_STATE_new();

    if (ch->err_state == NULL)
        return;

    OSSL_ERR_STATE_save(ch->err_state);
}

void ossl_quic_channel_inject(QUIC_CHANNEL *ch, QUIC_URXE *e)
{
    ossl_qrx_inject_urxe(ch->qrx, e);
}

void ossl_quic_channel_inject_pkt(QUIC_CHANNEL *ch, OSSL_QRX_PKT *qpkt)
{
    ossl_qrx_inject_pkt(ch->qrx, qpkt);
}

void ossl_quic_channel_on_stateless_reset(QUIC_CHANNEL *ch)
{
    QUIC_TERMINATE_CAUSE tcause = {0};

    tcause.error_code   = OSSL_QUIC_ERR_NO_ERROR;
    tcause.remote       = 1;
    ch_start_terminating(ch, &tcause, 0);
}

void ossl_quic_channel_raise_net_error(QUIC_CHANNEL *ch)
{
    QUIC_TERMINATE_CAUSE tcause = {0};

    if (ch->net_error)
        return;

    ch->net_error = 1;

    tcause.error_code = OSSL_QUIC_ERR_INTERNAL_ERROR;
    tcause.reason     = "network BIO I/O error";
    tcause.reason_len = strlen(tcause.reason);

    /*
     * Skip Terminating state and go directly to Terminated, no point trying to
     * send CONNECTION_CLOSE if we cannot communicate.
     */
    ch_start_terminating(ch, &tcause, 1);
}

int ossl_quic_channel_net_error(QUIC_CHANNEL *ch)
{
    return ch->net_error;
}

void ossl_quic_channel_restore_err_state(QUIC_CHANNEL *ch)
{
    if (ch == NULL)
        return;

    if (!ossl_quic_port_is_running(ch->port))
        ossl_quic_port_restore_err_state(ch->port);
    else
        OSSL_ERR_STATE_restore(ch->err_state);
}

void ossl_quic_channel_raise_protocol_error_loc(QUIC_CHANNEL *ch,
                                                uint64_t error_code,
                                                uint64_t frame_type,
                                                const char *reason,
                                                ERR_STATE *err_state,
                                                const char *src_file,
                                                int src_line,
                                                const char *src_func)
{
    QUIC_TERMINATE_CAUSE tcause = {0};
    int err_reason = error_code == OSSL_QUIC_ERR_INTERNAL_ERROR
                     ? ERR_R_INTERNAL_ERROR : SSL_R_QUIC_PROTOCOL_ERROR;
    const char *err_str = ossl_quic_err_to_string(error_code);
    const char *err_str_pfx = " (", *err_str_sfx = ")";
    const char *ft_str = NULL;
    const char *ft_str_pfx = " (", *ft_str_sfx = ")";

    if (ch->protocol_error)
        /* Only the first call to this function matters. */
        return;

    if (err_str == NULL) {
        err_str     = "";
        err_str_pfx = "";
        err_str_sfx = "";
    }

    /*
     * If we were provided an underlying error state, restore it and then append
     * our ERR on top as a "cover letter" error.
     */
    if (err_state != NULL)
        OSSL_ERR_STATE_restore(err_state);

    if (frame_type != 0) {
        ft_str = ossl_quic_frame_type_to_string(frame_type);
        if (ft_str == NULL) {
            ft_str      = "";
            ft_str_pfx  = "";
            ft_str_sfx  = "";
        }

        ERR_raise_data(ERR_LIB_SSL, err_reason,
                       "QUIC error code: 0x%llx%s%s%s "
                       "(triggered by frame type: 0x%llx%s%s%s), reason: \"%s\"",
                       (unsigned long long) error_code,
                       err_str_pfx, err_str, err_str_sfx,
                       (unsigned long long) frame_type,
                       ft_str_pfx, ft_str, ft_str_sfx,
                       reason);
    } else {
        ERR_raise_data(ERR_LIB_SSL, err_reason,
                       "QUIC error code: 0x%llx%s%s%s, reason: \"%s\"",
                       (unsigned long long) error_code,
                       err_str_pfx, err_str, err_str_sfx,
                       reason);
    }

    if (src_file != NULL)
        ERR_set_debug(src_file, src_line, src_func);

    ch_save_err_state(ch);

    tcause.error_code = error_code;
    tcause.frame_type = frame_type;
    tcause.reason     = reason;
    tcause.reason_len = strlen(reason);

    ch->protocol_error = 1;
    ch_start_terminating(ch, &tcause, 0);
}

/*
 * Called once the terminating timer expires, meaning we move from TERMINATING
 * to TERMINATED.
 */
static void ch_on_terminating_timeout(QUIC_CHANNEL *ch)
{
    ch_record_state_transition(ch, QUIC_CHANNEL_STATE_TERMINATED);
}

/*
 * Determines the effective idle timeout duration. This is based on the idle
 * timeout values that we and our peer signalled in transport parameters
 * but have some limits applied.
 */
static OSSL_TIME ch_get_effective_idle_timeout_duration(QUIC_CHANNEL *ch)
{
    OSSL_TIME pto;

    if (ch->max_idle_timeout == 0)
        return ossl_time_infinite();

    /*
     * RFC 9000 s. 10.1: Idle Timeout
     *  To avoid excessively small idle timeout periods, endpoints
     *  MUST increase the idle timeout period to be at least three
     *  times the current Probe Timeout (PTO). This allows for
     *  multiple PTOs to expire, and therefore multiple probes to
     *  be sent and lost, prior to idle timeout.
     */
    pto = ossl_ackm_get_pto_duration(ch->ackm);
    return ossl_time_max(ossl_ms2time(ch->max_idle_timeout),
                                      ossl_time_multiply(pto, 3));
}

/*
 * Updates our idle deadline. Called when an event happens which should bump the
 * idle timeout.
 */
static void ch_update_idle(QUIC_CHANNEL *ch)
{
    ch->idle_deadline = ossl_time_add(get_time(ch),
                                      ch_get_effective_idle_timeout_duration(ch));
}

/*
 * Updates our ping deadline, which determines when we next generate a ping if
 * we don't have any other ACK-eliciting frames to send.
 */
static void ch_update_ping_deadline(QUIC_CHANNEL *ch)
{
    OSSL_TIME max_span, idle_duration;

    idle_duration = ch_get_effective_idle_timeout_duration(ch);
    if (ossl_time_is_infinite(idle_duration)) {
        ch->ping_deadline = ossl_time_infinite();
        return;
    }

    /*
     * Maximum amount of time without traffic before we send a PING to keep
     * the connection open. Usually we use max_idle_timeout/2, but ensure
     * the period never exceeds the assumed NAT interval to ensure NAT
     * devices don't have their state time out (RFC 9000 s. 10.1.2).
     */
    max_span = ossl_time_divide(idle_duration, 2);
    max_span = ossl_time_min(max_span, MAX_NAT_INTERVAL);
    ch->ping_deadline = ossl_time_add(get_time(ch), max_span);
}

/* Called when the idle timeout expires. */
static void ch_on_idle_timeout(QUIC_CHANNEL *ch)
{
    /*
     * Idle timeout does not have an error code associated with it because a
     * CONN_CLOSE is never sent for it. We shouldn't use this data once we reach
     * TERMINATED anyway.
     */
    ch->terminate_cause.app         = 0;
    ch->terminate_cause.error_code  = OSSL_QUIC_LOCAL_ERR_IDLE_TIMEOUT;
    ch->terminate_cause.frame_type  = 0;

    ch_record_state_transition(ch, QUIC_CHANNEL_STATE_TERMINATED);
}

/**
 * @brief Common handler for initializing a new QUIC connection.
 *
 * This function configures a QUIC channel (`QUIC_CHANNEL *ch`) for a new
 * connection by setting the peer address, connection IDs, and necessary
 * callbacks. It establishes initial secrets, sets up logging, and performs
 * required transitions for the channel state.
 *
 * @param ch       Pointer to the QUIC channel being initialized.
 * @param peer     Address of the peer to which the channel connects.
 * @param peer_scid Peer-specified source connection ID.
 * @param peer_dcid Peer-specified destination connection ID.
 * @param peer_odcid Peer-specified original destination connection ID
 *                   may be NULL if retry frame not sent to client
 * @return         1 on success, 0 on failure to set required elements.
 */
static int ch_on_new_conn_common(QUIC_CHANNEL *ch, const BIO_ADDR *peer,
                                 const QUIC_CONN_ID *peer_scid,
                                 const QUIC_CONN_ID *peer_dcid,
                                 const QUIC_CONN_ID *peer_odcid)
{
    /* Note our newly learnt peer address and CIDs. */
    if (!BIO_ADDR_copy(&ch->cur_peer_addr, peer))
        return 0;

    ch->init_dcid       = *peer_dcid;
    ch->cur_remote_dcid = *peer_scid;
    ch->odcid.id_len = 0;

    if (peer_odcid != NULL)
        ch->odcid = *peer_odcid;

    /* Inform QTX of peer address. */
    if (!ossl_quic_tx_packetiser_set_peer(ch->txp, &ch->cur_peer_addr))
        return 0;

    /* Inform TXP of desired CIDs. */
    if (!ossl_quic_tx_packetiser_set_cur_dcid(ch->txp, &ch->cur_remote_dcid))
        return 0;

    if (!ossl_quic_tx_packetiser_set_cur_scid(ch->txp, &ch->cur_local_cid))
        return 0;

    /* Setup QLOG, which did not happen earlier due to lacking an Initial ODCID. */
    ossl_qtx_set_qlog_cb(ch->qtx, ch_get_qlog_cb, ch);
    ossl_quic_tx_packetiser_set_qlog_cb(ch->txp, ch_get_qlog_cb, ch);

    /*
     * Plug in secrets for the Initial EL. secrets for QRX were created in
     * port_default_packet_handler() already.
     */
    if (!ossl_quic_provide_initial_secret(ch->port->engine->libctx,
                                          ch->port->engine->propq,
                                          &ch->init_dcid,
                                          /*is_server=*/1,
                                          NULL, ch->qtx))
        return 0;

    /* Register the peer ODCID in the LCIDM. */
    if (!ossl_quic_lcidm_enrol_odcid(ch->lcidm, ch, peer_odcid == NULL ?
                                     &ch->init_dcid :
                                     peer_odcid))
        return 0;

    /* Change state. */
    ch_record_state_transition(ch, QUIC_CHANNEL_STATE_ACTIVE);
    ch->doing_proactive_ver_neg = 0; /* not currently supported */
    return 1;
}

/* Called when we, as a server, get a new incoming connection. */
int ossl_quic_channel_on_new_conn(QUIC_CHANNEL *ch, const BIO_ADDR *peer,
                                  const QUIC_CONN_ID *peer_scid,
                                  const QUIC_CONN_ID *peer_dcid)
{
    if (!ossl_assert(ch->state == QUIC_CHANNEL_STATE_IDLE && ch->is_server))
        return 0;

    /* Generate an Initial LCID we will use for the connection. */
    if (!ossl_quic_lcidm_generate_initial(ch->lcidm, ch, &ch->cur_local_cid))
        return 0;

    return ch_on_new_conn_common(ch, peer, peer_scid, peer_dcid, NULL);
}

/**
 * Binds a QUIC channel to a specific peer's address and connection IDs.
 *
 * This function is used to establish a binding between a QUIC channel and a
 * peer's address and connection IDs. The binding is performed only if the
 * channel is idle and is on the server side. The peer's destination connection
 * ID (`peer_dcid`) is mandatory, and the channel's current local connection ID
 * is set to this value.
 *
 * @param ch          Pointer to the QUIC_CHANNEL structure representing the
 *                    channel to be bound.
 * @param peer        Pointer to a BIO_ADDR structure representing the peer's
 *                    address.
 * @param peer_scid   Pointer to the peer's source connection ID (QUIC_CONN_ID).
 * @param peer_dcid   Pointer to the peer's destination connection ID
 *                    (QUIC_CONN_ID). This must not be NULL.
 * @param peer_odcid  Pointer to the original destination connection ID
 *                    (QUIC_CONN_ID) chosen by the peer in its first initial
 *                    packet received without a token.
 *
 * @return 1 on success, or 0 on failure if the conditions for binding are not
 *         met (e.g., channel is not idle or not a server, or binding fails).
 */
int ossl_quic_bind_channel(QUIC_CHANNEL *ch, const BIO_ADDR *peer,
                           const QUIC_CONN_ID *peer_scid,
                           const QUIC_CONN_ID *peer_dcid,
                           const QUIC_CONN_ID *peer_odcid)
{
    if (peer_dcid == NULL)
        return 0;

    if (!ossl_assert(ch->state == QUIC_CHANNEL_STATE_IDLE && ch->is_server))
        return 0;

    ch->cur_local_cid = *peer_dcid;
    if (!ossl_quic_lcidm_bind_channel(ch->lcidm, ch, peer_dcid))
        return 0;

    /*
     * peer_odcid <=> is initial dst conn id chosen by peer in its
     * first initial packet we received without token.
     */
    return ch_on_new_conn_common(ch, peer, peer_scid, peer_dcid, peer_odcid);
}

SSL *ossl_quic_channel_get0_ssl(QUIC_CHANNEL *ch)
{
    return ch->tls;
}

static int ch_init_new_stream(QUIC_CHANNEL *ch, QUIC_STREAM *qs,
                              int can_send, int can_recv)
{
    uint64_t rxfc_wnd;
    int server_init = ossl_quic_stream_is_server_init(qs);
    int local_init = (ch->is_server == server_init);
    int is_uni = !ossl_quic_stream_is_bidi(qs);

    if (can_send)
        if ((qs->sstream = ossl_quic_sstream_new(INIT_APP_BUF_LEN)) == NULL)
            goto err;

    if (can_recv)
        if ((qs->rstream = ossl_quic_rstream_new(NULL, NULL, 0)) == NULL)
            goto err;

    /* TXFC */
    if (!ossl_quic_txfc_init(&qs->txfc, &ch->conn_txfc))
        goto err;

    if (ch->got_remote_transport_params) {
        /*
         * If we already got peer TPs we need to apply the initial CWM credit
         * now. If we didn't already get peer TPs this will be done
         * automatically for all extant streams when we do.
         */
        if (can_send) {
            uint64_t cwm;

            if (is_uni)
                cwm = ch->rx_init_max_stream_data_uni;
            else if (local_init)
                cwm = ch->rx_init_max_stream_data_bidi_local;
            else
                cwm = ch->rx_init_max_stream_data_bidi_remote;

            ossl_quic_txfc_bump_cwm(&qs->txfc, cwm);
        }
    }

    /* RXFC */
    if (!can_recv)
        rxfc_wnd = 0;
    else if (is_uni)
        rxfc_wnd = ch->tx_init_max_stream_data_uni;
    else if (local_init)
        rxfc_wnd = ch->tx_init_max_stream_data_bidi_local;
    else
        rxfc_wnd = ch->tx_init_max_stream_data_bidi_remote;

    if (!ossl_quic_rxfc_init(&qs->rxfc, &ch->conn_rxfc,
                             rxfc_wnd,
                             DEFAULT_STREAM_RXFC_MAX_WND_MUL * rxfc_wnd,
                             get_time, ch))
        goto err;

    return 1;

err:
    ossl_quic_sstream_free(qs->sstream);
    qs->sstream = NULL;
    ossl_quic_rstream_free(qs->rstream);
    qs->rstream = NULL;
    return 0;
}

static uint64_t *ch_get_local_stream_next_ordinal_ptr(QUIC_CHANNEL *ch,
                                                      int is_uni)
{
    return is_uni ? &ch->next_local_stream_ordinal_uni
                  : &ch->next_local_stream_ordinal_bidi;
}

static const uint64_t *ch_get_local_stream_max_ptr(const QUIC_CHANNEL *ch,
                                                   int is_uni)
{
    return is_uni ? &ch->max_local_streams_uni
                  : &ch->max_local_streams_bidi;
}

static const QUIC_RXFC *ch_get_remote_stream_count_rxfc(const QUIC_CHANNEL *ch,
                                                        int is_uni)
{
    return is_uni ? &ch->max_streams_uni_rxfc
                  : &ch->max_streams_bidi_rxfc;
}

int ossl_quic_channel_is_new_local_stream_admissible(QUIC_CHANNEL *ch,
                                                     int is_uni)
{
    const uint64_t *p_next_ordinal = ch_get_local_stream_next_ordinal_ptr(ch, is_uni);

    return ossl_quic_stream_map_is_local_allowed_by_stream_limit(&ch->qsm,
                                                                 *p_next_ordinal,
                                                                 is_uni);
}

uint64_t ossl_quic_channel_get_local_stream_count_avail(const QUIC_CHANNEL *ch,
                                                        int is_uni)
{
    const uint64_t *p_next_ordinal, *p_max;

    p_next_ordinal  = ch_get_local_stream_next_ordinal_ptr((QUIC_CHANNEL *)ch,
                                                           is_uni);
    p_max           = ch_get_local_stream_max_ptr(ch, is_uni);

    return *p_max - *p_next_ordinal;
}

uint64_t ossl_quic_channel_get_remote_stream_count_avail(const QUIC_CHANNEL *ch,
                                                         int is_uni)
{
    return ossl_quic_rxfc_get_credit(ch_get_remote_stream_count_rxfc(ch, is_uni));
}

QUIC_STREAM *ossl_quic_channel_new_stream_local(QUIC_CHANNEL *ch, int is_uni)
{
    QUIC_STREAM *qs;
    int type;
    uint64_t stream_id;
    uint64_t *p_next_ordinal;

    type = ch->is_server ? QUIC_STREAM_INITIATOR_SERVER
                         : QUIC_STREAM_INITIATOR_CLIENT;

    p_next_ordinal = ch_get_local_stream_next_ordinal_ptr(ch, is_uni);

    if (is_uni)
        type |= QUIC_STREAM_DIR_UNI;
    else
        type |= QUIC_STREAM_DIR_BIDI;

    if (*p_next_ordinal >= ((uint64_t)1) << 62)
        return NULL;

    stream_id = ((*p_next_ordinal) << 2) | type;

    if ((qs = ossl_quic_stream_map_alloc(&ch->qsm, stream_id, type)) == NULL)
        return NULL;

    /* Locally-initiated stream, so we always want a send buffer. */
    if (!ch_init_new_stream(ch, qs, /*can_send=*/1, /*can_recv=*/!is_uni))
        goto err;

    ++*p_next_ordinal;
    return qs;

err:
    ossl_quic_stream_map_release(&ch->qsm, qs);
    return NULL;
}

QUIC_STREAM *ossl_quic_channel_new_stream_remote(QUIC_CHANNEL *ch,
                                                 uint64_t stream_id)
{
    uint64_t peer_role;
    int is_uni;
    QUIC_STREAM *qs;

    peer_role = ch->is_server
        ? QUIC_STREAM_INITIATOR_CLIENT
        : QUIC_STREAM_INITIATOR_SERVER;

    if ((stream_id & QUIC_STREAM_INITIATOR_MASK) != peer_role)
        return NULL;

    is_uni = ((stream_id & QUIC_STREAM_DIR_MASK) == QUIC_STREAM_DIR_UNI);

    qs = ossl_quic_stream_map_alloc(&ch->qsm, stream_id,
                                    stream_id & (QUIC_STREAM_INITIATOR_MASK
                                                 | QUIC_STREAM_DIR_MASK));
    if (qs == NULL)
        return NULL;

    if (!ch_init_new_stream(ch, qs, /*can_send=*/!is_uni, /*can_recv=*/1))
        goto err;

    if (ch->incoming_stream_auto_reject)
        ossl_quic_channel_reject_stream(ch, qs);
    else
        ossl_quic_stream_map_push_accept_queue(&ch->qsm, qs);

    return qs;

err:
    ossl_quic_stream_map_release(&ch->qsm, qs);
    return NULL;
}

void ossl_quic_channel_set_incoming_stream_auto_reject(QUIC_CHANNEL *ch,
                                                       int enable,
                                                       uint64_t aec)
{
    ch->incoming_stream_auto_reject     = (enable != 0);
    ch->incoming_stream_auto_reject_aec = aec;
}

void ossl_quic_channel_reject_stream(QUIC_CHANNEL *ch, QUIC_STREAM *qs)
{
    ossl_quic_stream_map_stop_sending_recv_part(&ch->qsm, qs,
                                                ch->incoming_stream_auto_reject_aec);

    ossl_quic_stream_map_reset_stream_send_part(&ch->qsm, qs,
                                                ch->incoming_stream_auto_reject_aec);
    qs->deleted = 1;

    ossl_quic_stream_map_update_state(&ch->qsm, qs);
}

/* Replace local connection ID in TXP and DEMUX for testing purposes. */
int ossl_quic_channel_replace_local_cid(QUIC_CHANNEL *ch,
                                        const QUIC_CONN_ID *conn_id)
{
    /* Remove the current LCID from the LCIDM. */
    if (!ossl_quic_lcidm_debug_remove(ch->lcidm, &ch->cur_local_cid))
        return 0;
    ch->cur_local_cid = *conn_id;
    /* Set in the TXP, used only for long header packets. */
    if (!ossl_quic_tx_packetiser_set_cur_scid(ch->txp, &ch->cur_local_cid))
        return 0;
    /* Add the new LCID to the LCIDM. */
    if (!ossl_quic_lcidm_debug_add(ch->lcidm, ch, &ch->cur_local_cid,
                                   100))
        return 0;
    return 1;
}

void ossl_quic_channel_set_msg_callback(QUIC_CHANNEL *ch,
                                        ossl_msg_cb msg_callback,
                                        SSL *msg_callback_ssl)
{
    ch->msg_callback = msg_callback;
    ch->msg_callback_ssl = msg_callback_ssl;
    ossl_qtx_set_msg_callback(ch->qtx, msg_callback, msg_callback_ssl);
    ossl_quic_tx_packetiser_set_msg_callback(ch->txp, msg_callback,
                                             msg_callback_ssl);
    /*
     * postpone msg callback setting for tserver until port calls
     * port_bind_channel().
     */
    if (ch->is_tserver_ch == 0)
        ossl_qrx_set_msg_callback(ch->qrx, msg_callback, msg_callback_ssl);
}

void ossl_quic_channel_set_msg_callback_arg(QUIC_CHANNEL *ch,
                                            void *msg_callback_arg)
{
    ch->msg_callback_arg = msg_callback_arg;
    ossl_qtx_set_msg_callback_arg(ch->qtx, msg_callback_arg);
    ossl_quic_tx_packetiser_set_msg_callback_arg(ch->txp, msg_callback_arg);

    /*
     * postpone msg callback setting for tserver until port calls
     * port_bind_channel().
     */
    if (ch->is_tserver_ch == 0)
        ossl_qrx_set_msg_callback_arg(ch->qrx, msg_callback_arg);
}

void ossl_quic_channel_set_txku_threshold_override(QUIC_CHANNEL *ch,
                                                   uint64_t tx_pkt_threshold)
{
    ch->txku_threshold_override = tx_pkt_threshold;
}

uint64_t ossl_quic_channel_get_tx_key_epoch(QUIC_CHANNEL *ch)
{
    return ossl_qtx_get_key_epoch(ch->qtx);
}

uint64_t ossl_quic_channel_get_rx_key_epoch(QUIC_CHANNEL *ch)
{
    return ossl_qrx_get_key_epoch(ch->qrx);
}

int ossl_quic_channel_trigger_txku(QUIC_CHANNEL *ch)
{
    if (!txku_allowed(ch))
        return 0;

    ch->ku_locally_initiated = 1;
    ch_trigger_txku(ch);
    return 1;
}

int ossl_quic_channel_ping(QUIC_CHANNEL *ch)
{
    int pn_space = ossl_quic_enc_level_to_pn_space(ch->tx_enc_level);

    ossl_quic_tx_packetiser_schedule_ack_eliciting(ch->txp, pn_space);

    return 1;
}

uint16_t ossl_quic_channel_get_diag_num_rx_ack(QUIC_CHANNEL *ch)
{
    return ch->diag_num_rx_ack;
}

void ossl_quic_channel_get_diag_local_cid(QUIC_CHANNEL *ch, QUIC_CONN_ID *cid)
{
    *cid = ch->cur_local_cid;
}

int ossl_quic_channel_have_generated_transport_params(const QUIC_CHANNEL *ch)
{
    return ch->got_local_transport_params;
}

void ossl_quic_channel_set_max_idle_timeout_request(QUIC_CHANNEL *ch, uint64_t ms)
{
    ch->max_idle_timeout_local_req = ms;
}
uint64_t ossl_quic_channel_get_max_idle_timeout_request(const QUIC_CHANNEL *ch)
{
    return ch->max_idle_timeout_local_req;
}

uint64_t ossl_quic_channel_get_max_idle_timeout_peer_request(const QUIC_CHANNEL *ch)
{
    return ch->max_idle_timeout_remote_req;
}

uint64_t ossl_quic_channel_get_max_idle_timeout_actual(const QUIC_CHANNEL *ch)
{
    return ch->max_idle_timeout;
}
