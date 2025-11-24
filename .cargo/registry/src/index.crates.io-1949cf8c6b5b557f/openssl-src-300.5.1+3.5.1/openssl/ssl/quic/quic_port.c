/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_port.h"
#include "internal/quic_channel.h"
#include "internal/quic_lcidm.h"
#include "internal/quic_srtm.h"
#include "internal/quic_txp.h"
#include "internal/ssl_unwrap.h"
#include "quic_port_local.h"
#include "quic_channel_local.h"
#include "quic_engine_local.h"
#include "quic_local.h"
#include "../ssl_local.h"
#include <openssl/rand.h>

/*
 * QUIC Port Structure
 * ===================
 */
#define INIT_DCID_LEN                   8

static int port_init(QUIC_PORT *port);
static void port_cleanup(QUIC_PORT *port);
static OSSL_TIME get_time(void *arg);
static void port_default_packet_handler(QUIC_URXE *e, void *arg,
                                        const QUIC_CONN_ID *dcid);
static void port_rx_pre(QUIC_PORT *port);

/**
 * @struct validation_token
 * @brief Represents a validation token for secure connection handling.
 *
 * This struct is used to store information related to a validation token.
 *
 * @var validation_token::is_retry
 * True iff this validation token is for a token sent in a RETRY packet.
 * Otherwise, this token is from a NEW_TOKEN_packet. Iff this value is true,
 * then ODCID and RSCID are set.
 *
 * @var validation_token::timestamp
 * Time that the validation token was minted.
 *
 * @var validation_token::odcid
 * An original connection ID (`QUIC_CONN_ID`) used to identify the QUIC
 * connection. This ID helps associate the token with a specific connection.
 * This will only be valid for validation tokens from RETRY packets.
 *
 * @var validation_token::rscid
 * DCID that the client will use as the DCID of the subsequent initial packet
 * i.e the "new" DCID.
 * This will only be valid for validation tokens from RETRY packets.
 *
 * @var validation_token::remote_addr_len
 * Length of the following character array.
 *
 * @var validation_token::remote_addr
 * A character array holding the raw address of the client requesting the
 * connection.
 */
typedef struct validation_token {
    OSSL_TIME timestamp;
    QUIC_CONN_ID odcid;
    QUIC_CONN_ID rscid;
    size_t remote_addr_len;
    unsigned char *remote_addr;
    unsigned char is_retry;
} QUIC_VALIDATION_TOKEN;

/*
 * Maximum length of a marshalled validation token.
 *
 * - timestamp is 8 bytes
 * - odcid and rscid are maximally 42 bytes in total
 * - remote_addr_len is a size_t (8 bytes)
 * - remote_addr is in the worst case 110 bytes (in the case of using a
 *   maximally sized AF_UNIX socket)
 * - is_retry is a single byte
 */
#define MARSHALLED_TOKEN_MAX_LEN 169

/*
 * Maximum length of an encrypted marshalled validation token.
 *
 * This will include the size of the marshalled validation token plus a 16 byte
 * tag and a 12 byte IV, so in total 197 bytes.
 */
#define ENCRYPTED_TOKEN_MAX_LEN (MARSHALLED_TOKEN_MAX_LEN + 16 + 12)

DEFINE_LIST_OF_IMPL(ch, QUIC_CHANNEL);
DEFINE_LIST_OF_IMPL(incoming_ch, QUIC_CHANNEL);
DEFINE_LIST_OF_IMPL(port, QUIC_PORT);

QUIC_PORT *ossl_quic_port_new(const QUIC_PORT_ARGS *args)
{
    QUIC_PORT *port;

    if ((port = OPENSSL_zalloc(sizeof(QUIC_PORT))) == NULL)
        return NULL;

    port->engine        = args->engine;
    port->channel_ctx   = args->channel_ctx;
    port->is_multi_conn = args->is_multi_conn;
    port->validate_addr = args->do_addr_validation;
    port->get_conn_user_ssl = args->get_conn_user_ssl;
    port->user_ssl_arg = args->user_ssl_arg;

    if (!port_init(port)) {
        OPENSSL_free(port);
        return NULL;
    }

    return port;
}

void ossl_quic_port_free(QUIC_PORT *port)
{
    if (port == NULL)
        return;

    port_cleanup(port);
    OPENSSL_free(port);
}

static int port_init(QUIC_PORT *port)
{
    size_t rx_short_dcid_len = (port->is_multi_conn ? INIT_DCID_LEN : 0);
    int key_len;
    EVP_CIPHER *cipher = NULL;
    unsigned char *token_key = NULL;
    int ret = 0;

    if (port->engine == NULL || port->channel_ctx == NULL)
        goto err;

    if ((port->err_state = OSSL_ERR_STATE_new()) == NULL)
        goto err;

    if ((port->demux = ossl_quic_demux_new(/*BIO=*/NULL,
                                           /*Short CID Len=*/rx_short_dcid_len,
                                           get_time, port)) == NULL)
        goto err;

    ossl_quic_demux_set_default_handler(port->demux,
                                        port_default_packet_handler,
                                        port);

    if ((port->srtm = ossl_quic_srtm_new(port->engine->libctx,
                                         port->engine->propq)) == NULL)
        goto err;

    if ((port->lcidm = ossl_quic_lcidm_new(port->engine->libctx,
                                           rx_short_dcid_len)) == NULL)
        goto err;

    port->rx_short_dcid_len = (unsigned char)rx_short_dcid_len;
    port->tx_init_dcid_len  = INIT_DCID_LEN;
    port->state             = QUIC_PORT_STATE_RUNNING;

    ossl_list_port_insert_tail(&port->engine->port_list, port);
    port->on_engine_list    = 1;
    port->bio_changed       = 1;

    /* Generate random key for token encryption */
    if ((port->token_ctx = EVP_CIPHER_CTX_new()) == NULL
        || (cipher = EVP_CIPHER_fetch(port->engine->libctx,
                                      "AES-256-GCM", NULL)) == NULL
        || !EVP_EncryptInit_ex(port->token_ctx, cipher, NULL, NULL, NULL)
        || (key_len = EVP_CIPHER_CTX_get_key_length(port->token_ctx)) <= 0
        || (token_key = OPENSSL_malloc(key_len)) == NULL
        || !RAND_bytes_ex(port->engine->libctx, token_key, key_len, 0)
        || !EVP_EncryptInit_ex(port->token_ctx, NULL, NULL, token_key, NULL))
        goto err;

    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    OPENSSL_free(token_key);
    if (!ret)
        port_cleanup(port);
    return ret;
}

static void port_cleanup(QUIC_PORT *port)
{
    assert(ossl_list_ch_num(&port->channel_list) == 0);

    ossl_quic_demux_free(port->demux);
    port->demux = NULL;

    ossl_quic_srtm_free(port->srtm);
    port->srtm = NULL;

    ossl_quic_lcidm_free(port->lcidm);
    port->lcidm = NULL;

    OSSL_ERR_STATE_free(port->err_state);
    port->err_state = NULL;

    if (port->on_engine_list) {
        ossl_list_port_remove(&port->engine->port_list, port);
        port->on_engine_list = 0;
    }

    EVP_CIPHER_CTX_free(port->token_ctx);
    port->token_ctx = NULL;
}

static void port_transition_failed(QUIC_PORT *port)
{
    if (port->state == QUIC_PORT_STATE_FAILED)
        return;

    port->state = QUIC_PORT_STATE_FAILED;
}

int ossl_quic_port_is_running(const QUIC_PORT *port)
{
    return port->state == QUIC_PORT_STATE_RUNNING;
}

QUIC_ENGINE *ossl_quic_port_get0_engine(QUIC_PORT *port)
{
    return port->engine;
}

QUIC_REACTOR *ossl_quic_port_get0_reactor(QUIC_PORT *port)
{
    return ossl_quic_engine_get0_reactor(port->engine);
}

QUIC_DEMUX *ossl_quic_port_get0_demux(QUIC_PORT *port)
{
    return port->demux;
}

CRYPTO_MUTEX *ossl_quic_port_get0_mutex(QUIC_PORT *port)
{
    return ossl_quic_engine_get0_mutex(port->engine);
}

OSSL_TIME ossl_quic_port_get_time(QUIC_PORT *port)
{
    return ossl_quic_engine_get_time(port->engine);
}

static OSSL_TIME get_time(void *port)
{
    return ossl_quic_port_get_time((QUIC_PORT *)port);
}

int ossl_quic_port_get_rx_short_dcid_len(const QUIC_PORT *port)
{
    return port->rx_short_dcid_len;
}

int ossl_quic_port_get_tx_init_dcid_len(const QUIC_PORT *port)
{
    return port->tx_init_dcid_len;
}

size_t ossl_quic_port_get_num_incoming_channels(const QUIC_PORT *port)
{
    return ossl_list_incoming_ch_num(&port->incoming_channel_list);
}

/*
 * QUIC Port: Network BIO Configuration
 * ====================================
 */

/* Determines whether we can support a given poll descriptor. */
static int validate_poll_descriptor(const BIO_POLL_DESCRIPTOR *d)
{
    if (d->type == BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD && d->value.fd < 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    return 1;
}

BIO *ossl_quic_port_get_net_rbio(QUIC_PORT *port)
{
    return port->net_rbio;
}

BIO *ossl_quic_port_get_net_wbio(QUIC_PORT *port)
{
    return port->net_wbio;
}

static int port_update_poll_desc(QUIC_PORT *port, BIO *net_bio, int for_write)
{
    BIO_POLL_DESCRIPTOR d = {0};

    if (net_bio == NULL
        || (!for_write && !BIO_get_rpoll_descriptor(net_bio, &d))
        || (for_write && !BIO_get_wpoll_descriptor(net_bio, &d)))
        /* Non-pollable BIO */
        d.type = BIO_POLL_DESCRIPTOR_TYPE_NONE;

    if (!validate_poll_descriptor(&d))
        return 0;

    /*
     * TODO(QUIC MULTIPORT): We currently only support one port per
     * engine/domain. This is necessitated because QUIC_REACTOR only supports a
     * single pollable currently. In the future, once complete polling
     * infrastructure has been implemented, this limitation can be removed.
     *
     * For now, just update the descriptor on the engine's reactor as we are
     * guaranteed to be the only port under it.
     */
    if (for_write)
        ossl_quic_reactor_set_poll_w(&port->engine->rtor, &d);
    else
        ossl_quic_reactor_set_poll_r(&port->engine->rtor, &d);

    return 1;
}

int ossl_quic_port_update_poll_descriptors(QUIC_PORT *port, int force)
{
    int ok = 1;

    if (!force && !port->bio_changed)
        return 0;

    if (!port_update_poll_desc(port, port->net_rbio, /*for_write=*/0))
        ok = 0;

    if (!port_update_poll_desc(port, port->net_wbio, /*for_write=*/1))
        ok = 0;

    port->bio_changed = 0;
    return ok;
}

/*
 * We need to determine our addressing mode. There are basically two ways we can
 * use L4 addresses:
 *
 *   - Addressed mode, in which our BIO_sendmmsg calls have destination
 *     addresses attached to them which we expect the underlying network BIO to
 *     handle;
 *
 *   - Unaddressed mode, in which the BIO provided to us on the network side
 *     neither provides us with L4 addresses nor is capable of honouring ones we
 *     provide. We don't know where the QUIC traffic we send ends up exactly and
 *     trust the application to know what it is doing.
 *
 * Addressed mode is preferred because it enables support for connection
 * migration, multipath, etc. in the future. Addressed mode is automatically
 * enabled if we are using e.g. BIO_s_datagram, with or without BIO_s_connect.
 *
 * If we are passed a BIO_s_dgram_pair (or some custom BIO) we may have to use
 * unaddressed mode unless that BIO supports capability flags indicating it can
 * provide and honour L4 addresses.
 *
 * Our strategy for determining address mode is simple: we probe the underlying
 * network BIOs for their capabilities. If the network BIOs support what we
 * need, we use addressed mode. Otherwise, we use unaddressed mode.
 *
 * If addressed mode is chosen, we require an initial peer address to be set. If
 * this is not set, we fail. If unaddressed mode is used, we do not require
 * this, as such an address is superfluous, though it can be set if desired.
 */
static void port_update_addressing_mode(QUIC_PORT *port)
{
    long rcaps = 0, wcaps = 0;

    if (port->net_rbio != NULL)
        rcaps = BIO_dgram_get_effective_caps(port->net_rbio);

    if (port->net_wbio != NULL)
        wcaps = BIO_dgram_get_effective_caps(port->net_wbio);

    port->addressed_mode_r = ((rcaps & BIO_DGRAM_CAP_PROVIDES_SRC_ADDR) != 0);
    port->addressed_mode_w = ((wcaps & BIO_DGRAM_CAP_HANDLES_DST_ADDR) != 0);
    port->bio_changed = 1;
}

int ossl_quic_port_is_addressed_r(const QUIC_PORT *port)
{
    return port->addressed_mode_r;
}

int ossl_quic_port_is_addressed_w(const QUIC_PORT *port)
{
    return port->addressed_mode_w;
}

int ossl_quic_port_is_addressed(const QUIC_PORT *port)
{
    return ossl_quic_port_is_addressed_r(port) && ossl_quic_port_is_addressed_w(port);
}

/*
 * QUIC_PORT does not ref any BIO it is provided with, nor is any ref
 * transferred to it. The caller (e.g., QUIC_CONNECTION) is responsible for
 * ensuring the BIO lasts until the channel is freed or the BIO is switched out
 * for another BIO by a subsequent successful call to this function.
 */
int ossl_quic_port_set_net_rbio(QUIC_PORT *port, BIO *net_rbio)
{
    if (port->net_rbio == net_rbio)
        return 1;

    if (!port_update_poll_desc(port, net_rbio, /*for_write=*/0))
        return 0;

    ossl_quic_demux_set_bio(port->demux, net_rbio);
    port->net_rbio = net_rbio;
    port_update_addressing_mode(port);
    return 1;
}

int ossl_quic_port_set_net_wbio(QUIC_PORT *port, BIO *net_wbio)
{
    QUIC_CHANNEL *ch;

    if (port->net_wbio == net_wbio)
        return 1;

    if (!port_update_poll_desc(port, net_wbio, /*for_write=*/1))
        return 0;

    OSSL_LIST_FOREACH(ch, ch, &port->channel_list)
        ossl_qtx_set_bio(ch->qtx, net_wbio);

    port->net_wbio = net_wbio;
    port_update_addressing_mode(port);
    return 1;
}

SSL_CTX *ossl_quic_port_get_channel_ctx(QUIC_PORT *port)
{
    return port->channel_ctx;
}

/*
 * QUIC Port: Channel Lifecycle
 * ============================
 */

static SSL *port_new_handshake_layer(QUIC_PORT *port, QUIC_CHANNEL *ch)
{
    SSL *tls = NULL;
    SSL_CONNECTION *tls_conn = NULL;
    SSL *user_ssl = NULL;
    QUIC_CONNECTION *qc = NULL;
    QUIC_LISTENER *ql = NULL;

    /*
     * It only makes sense to call this function if we know how to associate
     * the handshake layer we are about to create with some user_ssl object.
     */
    if (!ossl_assert(port->get_conn_user_ssl != NULL))
        return NULL;
    user_ssl = port->get_conn_user_ssl(ch, port->user_ssl_arg);
    if (user_ssl == NULL)
        return NULL;
    qc = (QUIC_CONNECTION *)user_ssl;
    ql = (QUIC_LISTENER *)port->user_ssl_arg;

    /*
     * We expect the user_ssl to be newly created so it must not have an
     * existing qc->tls
     */
    if (!ossl_assert(qc->tls == NULL)) {
        SSL_free(user_ssl);
        return NULL;
    }

    tls = ossl_ssl_connection_new_int(port->channel_ctx, user_ssl, TLS_method());
    qc->tls = tls;
    if (tls == NULL || (tls_conn = SSL_CONNECTION_FROM_SSL(tls)) == NULL) {
        SSL_free(user_ssl);
        return NULL;
    }

    if (ql != NULL && ql->obj.ssl.ctx->new_pending_conn_cb != NULL)
        if (!ql->obj.ssl.ctx->new_pending_conn_cb(ql->obj.ssl.ctx, user_ssl,
                                                  ql->obj.ssl.ctx->new_pending_conn_arg)) {
            SSL_free(user_ssl);
            return NULL;
        }

    /* Override the user_ssl of the inner connection. */
    tls_conn->s3.flags      |= TLS1_FLAGS_QUIC | TLS1_FLAGS_QUIC_INTERNAL;

    /* Restrict options derived from the SSL_CTX. */
    tls_conn->options       &= OSSL_QUIC_PERMITTED_OPTIONS_CONN;
    tls_conn->pha_enabled   = 0;
    return tls;
}

static QUIC_CHANNEL *port_make_channel(QUIC_PORT *port, SSL *tls, OSSL_QRX *qrx,
                                       int is_server, int is_tserver)
{
    QUIC_CHANNEL_ARGS args = {0};
    QUIC_CHANNEL *ch;

    args.port          = port;
    args.is_server     = is_server;
    args.lcidm         = port->lcidm;
    args.srtm          = port->srtm;
    args.qrx           = qrx;
    args.is_tserver_ch = is_tserver;

    /*
     * Creating a a new channel is made a bit tricky here as there is a
     * bit of a circular dependency.  Initalizing a channel requires that
     * the ch->tls and optionally the qlog_title be configured prior to
     * initalization, but we need the channel at least partially configured
     * to create the new handshake layer, so we have to do this in a few steps.
     */

    /*
     * start by allocation and provisioning as much of the channel as we can
     */
    ch = ossl_quic_channel_alloc(&args);
    if (ch == NULL)
        return NULL;

    /*
     * Fixup the channel tls connection here before we init the channel
     */
    ch->tls = (tls != NULL) ? tls : port_new_handshake_layer(port, ch);

    if (ch->tls == NULL) {
        OPENSSL_free(ch);
        return NULL;
    }

#ifndef OPENSSL_NO_QLOG
    /*
     * If we're using qlog, make sure the tls get further configured properly
     */
    ch->use_qlog = 1;
    if (ch->tls->ctx->qlog_title != NULL) {
        if ((ch->qlog_title = OPENSSL_strdup(ch->tls->ctx->qlog_title)) == NULL) {
            OPENSSL_free(ch);
            return NULL;
        }
    }
#endif

    /*
     * And finally init the channel struct
     */
    if (!ossl_quic_channel_init(ch)) {
        OPENSSL_free(ch);
        return NULL;
    }

    ossl_qtx_set_bio(ch->qtx, port->net_wbio);
    return ch;
}

QUIC_CHANNEL *ossl_quic_port_create_outgoing(QUIC_PORT *port, SSL *tls)
{
    return port_make_channel(port, tls, NULL, /* is_server= */ 0,
                             /* is_tserver= */ 0);
}

QUIC_CHANNEL *ossl_quic_port_create_incoming(QUIC_PORT *port, SSL *tls)
{
    QUIC_CHANNEL *ch;

    assert(port->tserver_ch == NULL);

    /*
     * pass -1 for qrx to indicate port will create qrx
     * later in port_default_packet_handler() when calling port_bind_channel().
     */
    ch = port_make_channel(port, tls, NULL, /* is_server= */ 1,
                           /* is_tserver_ch */ 1);
    port->tserver_ch = ch;
    port->allow_incoming = 1;
    return ch;
}

QUIC_CHANNEL *ossl_quic_port_pop_incoming(QUIC_PORT *port)
{
    QUIC_CHANNEL *ch;

    ch = ossl_list_incoming_ch_head(&port->incoming_channel_list);
    if (ch == NULL)
        return NULL;

    ossl_list_incoming_ch_remove(&port->incoming_channel_list, ch);
    return ch;
}

int ossl_quic_port_have_incoming(QUIC_PORT *port)
{
    return ossl_list_incoming_ch_head(&port->incoming_channel_list) != NULL;
}

void ossl_quic_port_drop_incoming(QUIC_PORT *port)
{
    QUIC_CHANNEL *ch;
    SSL *tls;
    SSL *user_ssl;
    SSL_CONNECTION *sc;

    for (;;) {
        ch = ossl_quic_port_pop_incoming(port);
        if (ch == NULL)
            break;

        tls = ossl_quic_channel_get0_tls(ch);
        /*
         * The user ssl may or may not have been created via the
         * get_conn_user_ssl callback in the QUIC stack.  The
         * differentiation being if the user_ssl pointer and tls pointer
         * are different.  If they are, then the user_ssl needs freeing here
         * which sends us through ossl_quic_free, which then drops the actual
         * ch->tls ref and frees the channel
         */
        sc = SSL_CONNECTION_FROM_SSL(tls);
        if (sc == NULL)
            break;

        user_ssl = SSL_CONNECTION_GET_USER_SSL(sc);
        if (user_ssl == tls) {
            ossl_quic_channel_free(ch);
            SSL_free(tls);
        } else {
            SSL_free(user_ssl);
        }
    }
}

void ossl_quic_port_set_allow_incoming(QUIC_PORT *port, int allow_incoming)
{
    port->allow_incoming = allow_incoming;
}

/*
 * QUIC Port: Ticker-Mutator
 * =========================
 */

/*
 * Tick function for this port. This does everything related to network I/O for
 * this port's network BIOs, and services child channels.
 */
void ossl_quic_port_subtick(QUIC_PORT *port, QUIC_TICK_RESULT *res,
                            uint32_t flags)
{
    QUIC_CHANNEL *ch;

    res->net_read_desired       = ossl_quic_port_is_running(port);
    res->net_write_desired      = 0;
    res->notify_other_threads   = 0;
    res->tick_deadline          = ossl_time_infinite();

    if (!port->engine->inhibit_tick) {
        /* Handle any incoming data from network. */
        if (ossl_quic_port_is_running(port))
            port_rx_pre(port);

        /* Iterate through all channels and service them. */
        OSSL_LIST_FOREACH(ch, ch, &port->channel_list) {
            QUIC_TICK_RESULT subr = {0};

            ossl_quic_channel_subtick(ch, &subr, flags);
            ossl_quic_tick_result_merge_into(res, &subr);
        }
    }
}

/* Process incoming datagrams, if any. */
static void port_rx_pre(QUIC_PORT *port)
{
    int ret;

    /*
     * Originally, this check (don't RX before we have sent anything if we are
     * not a server, because there can't be anything) was just intended as a
     * minor optimisation. However, it is actually required on Windows, and
     * removing this check will cause Windows to break.
     *
     * The reason is that under Win32, recvfrom() does not work on a UDP socket
     * which has not had bind() called (???). However, calling sendto() will
     * automatically bind an unbound UDP socket. Therefore, if we call a Winsock
     * recv-type function before calling a Winsock send-type function, that call
     * will fail with WSAEINVAL, which we will regard as a permanent network
     * error.
     *
     * Therefore, this check is essential as we do not require our API users to
     * bind a socket first when using the API in client mode.
     */
    if (!port->allow_incoming && !port->have_sent_any_pkt)
        return;

    /*
     * Get DEMUX to BIO_recvmmsg from the network and queue incoming datagrams
     * to the appropriate QRX instances.
     */
    ret = ossl_quic_demux_pump(port->demux);
    if (ret == QUIC_DEMUX_PUMP_RES_PERMANENT_FAIL)
        /*
         * We don't care about transient failure, but permanent failure means we
         * should tear down the port. All connections skip straight to the
         * Terminated state as there is no point trying to send CONNECTION_CLOSE
         * frames if the network BIO is not operating correctly.
         */
        ossl_quic_port_raise_net_error(port, NULL);
}

/*
 * Handles an incoming connection request and potentially decides to make a
 * connection from it. If a new connection is made, the new channel is written
 * to *new_ch.
 */
static void port_bind_channel(QUIC_PORT *port, const BIO_ADDR *peer,
                              const QUIC_CONN_ID *scid, const QUIC_CONN_ID *dcid,
                              const QUIC_CONN_ID *odcid, OSSL_QRX *qrx,
                              QUIC_CHANNEL **new_ch)
{
    QUIC_CHANNEL *ch;

    /*
     * If we're running with a simulated tserver, it will already have
     * a dummy channel created, use that instead
     */
    if (port->tserver_ch != NULL) {
        ch = port->tserver_ch;
        port->tserver_ch = NULL;
        ossl_quic_channel_bind_qrx(ch, qrx);
        ossl_qrx_set_msg_callback(ch->qrx, ch->msg_callback,
                                  ch->msg_callback_ssl);
        ossl_qrx_set_msg_callback_arg(ch->qrx, ch->msg_callback_arg);
    } else {
        ch = port_make_channel(port, NULL, qrx, /* is_server= */ 1,
                               /* is_tserver */ 0);
    }

    if (ch == NULL)
        return;

    /*
     * If we didn't provide a qrx here that means we need to set our initial
     * secret here, since we just created a qrx
     * Normally its not needed, as the initial secret gets added when we send
     * our first server hello, but if we get a huge client hello, crossing
     * multiple datagrams, we don't have a chance to do that, and datagrams
     * after the first won't get decoded properly, for lack of secrets
     */
    if (qrx == NULL)
        if (!ossl_quic_provide_initial_secret(ch->port->engine->libctx,
                                              ch->port->engine->propq,
                                              dcid, /* is_server */ 1,
                                              ch->qrx, NULL))
            return;

    if (odcid->id_len != 0) {
        /*
         * If we have an odcid, then we went through server address validation
         * and as such, this channel need not conform to the 3x validation cap
         * See RFC 9000 s. 8.1
         */
        ossl_quic_tx_packetiser_set_validated(ch->txp);
        if (!ossl_quic_bind_channel(ch, peer, scid, dcid, odcid)) {
            ossl_quic_channel_free(ch);
            return;
        }
    } else {
        /*
         * No odcid means we didn't do server validation, so we need to
         * generate a cid via ossl_quic_channel_on_new_conn
         */
        if (!ossl_quic_channel_on_new_conn(ch, peer, scid, dcid)) {
            ossl_quic_channel_free(ch);
            return;
        }
    }

    ossl_list_incoming_ch_insert_tail(&port->incoming_channel_list, ch);
    *new_ch = ch;
}

static int port_try_handle_stateless_reset(QUIC_PORT *port, const QUIC_URXE *e)
{
    size_t i;
    const unsigned char *data = ossl_quic_urxe_data(e);
    void *opaque = NULL;

    /*
     * Perform some fast and cheap checks for a packet not being a stateless
     * reset token.  RFC 9000 s. 10.3 specifies this layout for stateless
     * reset packets:
     *
     *  Stateless Reset {
     *      Fixed Bits (2) = 1,
     *      Unpredictable Bits (38..),
     *      Stateless Reset Token (128),
     *  }
     *
     * It also specifies:
     *      However, endpoints MUST treat any packet ending in a valid
     *      stateless reset token as a Stateless Reset, as other QUIC
     *      versions might allow the use of a long header.
     *
     * We can rapidly check for the minimum length and that the first pair
     * of bits in the first byte are 01 or 11.
     *
     * The function returns 1 if it is a stateless reset packet, 0 if it isn't
     * and -1 if an error was encountered.
     */
    if (e->data_len < QUIC_STATELESS_RESET_TOKEN_LEN + 5
        || (0100 & *data) != 0100)
        return 0;

    for (i = 0;; ++i) {
        if (!ossl_quic_srtm_lookup(port->srtm,
                                   (QUIC_STATELESS_RESET_TOKEN *)(data + e->data_len
                                   - sizeof(QUIC_STATELESS_RESET_TOKEN)),
                                   i, &opaque, NULL))
            break;

        assert(opaque != NULL);
        ossl_quic_channel_on_stateless_reset((QUIC_CHANNEL *)opaque);
    }

    return i > 0;
}

static void cleanup_validation_token(QUIC_VALIDATION_TOKEN *token)
{
    OPENSSL_free(token->remote_addr);
}

/**
 * @brief Generates a validation token for a RETRY/NEW_TOKEN packet.
 *
 *
 * @param peer  Address of the client peer receiving the packet.
 * @param odcid DCID of the connection attempt.
 * @param rscid Retry source connection ID of the connection attempt.
 * @param token Address of token to fill data.
 *
 * @return 1 if validation token is filled successfully, 0 otherwise.
 */
static int generate_token(BIO_ADDR *peer, QUIC_CONN_ID odcid,
                          QUIC_CONN_ID rscid, QUIC_VALIDATION_TOKEN *token,
                          int is_retry)
{
    token->is_retry = is_retry;
    token->timestamp = ossl_time_now();
    token->remote_addr = NULL;
    token->odcid = odcid;
    token->rscid = rscid;

    if (!BIO_ADDR_rawaddress(peer, NULL, &token->remote_addr_len)
        || token->remote_addr_len == 0
        || (token->remote_addr = OPENSSL_malloc(token->remote_addr_len)) == NULL
        || !BIO_ADDR_rawaddress(peer, token->remote_addr,
                                &token->remote_addr_len)) {
        cleanup_validation_token(token);
        return 0;
    }

    return 1;
}

/**
 * @brief Marshals a validation token into a new buffer.
 *
 * |buffer| should already be allocated and at least MARSHALLED_TOKEN_MAX_LEN
 * bytes long. Stores the length of data stored in |buffer| in |buffer_len|.
 *
 * @param token      Validation token.
 * @param buffer     Address to store the marshalled token.
 * @param buffer_len Size of data stored in |buffer|.
 */
static int marshal_validation_token(QUIC_VALIDATION_TOKEN *token,
                                    unsigned char *buffer, size_t *buffer_len)
{
    WPACKET wpkt = {0};
    BUF_MEM *buf_mem = BUF_MEM_new();

    if (buffer == NULL || buf_mem == NULL
        || (token->is_retry != 0 && token->is_retry != 1)) {
        BUF_MEM_free(buf_mem);
        return 0;
    }

    if (!WPACKET_init(&wpkt, buf_mem)
        || !WPACKET_memset(&wpkt, token->is_retry, 1)
        || !WPACKET_memcpy(&wpkt, &token->timestamp,
                           sizeof(token->timestamp))
        || (token->is_retry
            && (!WPACKET_sub_memcpy_u8(&wpkt, &token->odcid.id,
                                       token->odcid.id_len)
                || !WPACKET_sub_memcpy_u8(&wpkt, &token->rscid.id,
                                          token->rscid.id_len)))
        || !WPACKET_sub_memcpy_u8(&wpkt, token->remote_addr, token->remote_addr_len)
        || !WPACKET_get_total_written(&wpkt, buffer_len)
        || *buffer_len > MARSHALLED_TOKEN_MAX_LEN
        || !WPACKET_finish(&wpkt)) {
        WPACKET_cleanup(&wpkt);
        BUF_MEM_free(buf_mem);
        return 0;
    }

    memcpy(buffer, buf_mem->data, *buffer_len);
    BUF_MEM_free(buf_mem);
    return 1;
}

/**
 * @brief Encrypts a validation token using AES-256-GCM
 *
 * @param port       The QUIC port containing the encryption key
 * @param plaintext  The data to encrypt
 * @param pt_len     Length of the plaintext
 * @param ciphertext Buffer to receive encrypted data. If NULL, ct_len will be
 *                   set to the required buffer size and function returns
 *                   immediately.
 * @param ct_len     Pointer to size_t that will receive the ciphertext length.
 *                   This also includes bytes for QUIC_RETRY_INTEGRITY_TAG_LEN.
 *
 * @return 1 on success, 0 on failure
 *
 * The ciphertext format is:
 * [EVP_GCM_IV_LEN bytes IV][encrypted data][EVP_GCM_TAG_LEN bytes tag]
 */
static int encrypt_validation_token(const QUIC_PORT *port,
                                    const unsigned char *plaintext,
                                    size_t pt_len,
                                    unsigned char *ciphertext,
                                    size_t *ct_len)
{
    int iv_len, len, ret = 0;
    size_t tag_len;
    unsigned char *iv = ciphertext, *data, *tag;

    if ((tag_len = EVP_CIPHER_CTX_get_tag_length(port->token_ctx)) == 0
        || (iv_len = EVP_CIPHER_CTX_get_iv_length(port->token_ctx)) <= 0)
        goto err;

    *ct_len = iv_len + pt_len + tag_len + QUIC_RETRY_INTEGRITY_TAG_LEN;
    if (ciphertext == NULL) {
        ret = 1;
        goto err;
    }

    data = ciphertext + iv_len;
    tag = data + pt_len;

    if (!RAND_bytes_ex(port->engine->libctx, ciphertext, iv_len, 0)
        || !EVP_EncryptInit_ex(port->token_ctx, NULL, NULL, NULL, iv)
        || !EVP_EncryptUpdate(port->token_ctx, data, &len, plaintext, pt_len)
        || !EVP_EncryptFinal_ex(port->token_ctx, data + pt_len, &len)
        || !EVP_CIPHER_CTX_ctrl(port->token_ctx, EVP_CTRL_GCM_GET_TAG, tag_len, tag))
        goto err;

    ret = 1;
err:
    return ret;
}

/**
 * @brief Decrypts a validation token using AES-256-GCM
 *
 * @param port       The QUIC port containing the decryption key
 * @param ciphertext The encrypted data (including IV and tag)
 * @param ct_len     Length of the ciphertext
 * @param plaintext  Buffer to receive decrypted data. If NULL, pt_len will be
 *                   set to the required buffer size.
 * @param pt_len     Pointer to size_t that will receive the plaintext length
 *
 * @return 1 on success, 0 on failure
 *
 * Expected ciphertext format:
 * [EVP_GCM_IV_LEN bytes IV][encrypted data][EVP_GCM_TAG_LEN bytes tag]
 */
static int decrypt_validation_token(const QUIC_PORT *port,
                                    const unsigned char *ciphertext,
                                    size_t ct_len,
                                    unsigned char *plaintext,
                                    size_t *pt_len)
{
    int iv_len, len = 0, ret = 0;
    size_t tag_len;
    const unsigned char *iv = ciphertext, *data, *tag;

    if ((tag_len = EVP_CIPHER_CTX_get_tag_length(port->token_ctx)) == 0
        || (iv_len = EVP_CIPHER_CTX_get_iv_length(port->token_ctx)) <= 0)
        goto err;

    /* Prevent decryption of a buffer that is not within reasonable bounds */
    if (ct_len < (iv_len + tag_len) || ct_len > ENCRYPTED_TOKEN_MAX_LEN)
        goto err;

    *pt_len = ct_len - iv_len - tag_len;
    if (plaintext == NULL) {
        ret = 1;
        goto err;
    }

    data = ciphertext + iv_len;
    tag = ciphertext + ct_len - tag_len;

    if (!EVP_DecryptInit_ex(port->token_ctx, NULL, NULL, NULL, iv)
        || !EVP_DecryptUpdate(port->token_ctx, plaintext, &len, data,
                              ct_len - iv_len - tag_len)
        || !EVP_CIPHER_CTX_ctrl(port->token_ctx, EVP_CTRL_GCM_SET_TAG, tag_len,
                                (void *)tag)
        || !EVP_DecryptFinal_ex(port->token_ctx, plaintext + len, &len))
        goto err;

    ret = 1;

err:
    return ret;
}

/**
 * @brief Parses contents of a buffer into a validation token.
 *
 * VALIDATION_TOKEN should already be initalized. Does some basic sanity checks.
 *
 * @param token   Validation token to fill data in.
 * @param buf     Buffer of previously marshaled validation token.
 * @param buf_len Length of |buf|.
 */
static int parse_validation_token(QUIC_VALIDATION_TOKEN *token,
                                  const unsigned char *buf, size_t buf_len)
{
    PACKET pkt, subpkt;

    if (buf == NULL || token == NULL)
        return 0;

    token->remote_addr = NULL;

    if (!PACKET_buf_init(&pkt, buf, buf_len)
        || !PACKET_copy_bytes(&pkt, &token->is_retry, sizeof(token->is_retry))
        || !(token->is_retry == 0 || token->is_retry == 1)
        || !PACKET_copy_bytes(&pkt, (unsigned char *)&token->timestamp,
                              sizeof(token->timestamp))
        || (token->is_retry
            && (!PACKET_get_length_prefixed_1(&pkt, &subpkt)
                || (token->odcid.id_len = (unsigned char)PACKET_remaining(&subpkt))
                    > QUIC_MAX_CONN_ID_LEN
                || !PACKET_copy_bytes(&subpkt,
                                      (unsigned char *)&token->odcid.id,
                                      token->odcid.id_len)
                || !PACKET_get_length_prefixed_1(&pkt, &subpkt)
                || (token->rscid.id_len = (unsigned char)PACKET_remaining(&subpkt))
                    > QUIC_MAX_CONN_ID_LEN
                || !PACKET_copy_bytes(&subpkt, (unsigned char *)&token->rscid.id,
                                      token->rscid.id_len)))
        || !PACKET_get_length_prefixed_1(&pkt, &subpkt)
        || (token->remote_addr_len = PACKET_remaining(&subpkt)) == 0
        || (token->remote_addr = OPENSSL_malloc(token->remote_addr_len)) == NULL
        || !PACKET_copy_bytes(&subpkt, token->remote_addr, token->remote_addr_len)
        || PACKET_remaining(&pkt) != 0) {
        cleanup_validation_token(token);
        return 0;
    }

    return 1;
}

/**
 * @brief Sends a QUIC Retry packet to a client.
 *
 * This function constructs and sends a Retry packet to the specified client
 * using the provided connection header information. The Retry packet
 * includes a generated validation token and a new connection ID, following
 * the QUIC protocol specifications for connection establishment.
 *
 * @param port        Pointer to the QUIC port from which to send the packet.
 * @param peer        Address of the client peer receiving the packet.
 * @param client_hdr  Header of the client's initial packet, containing
 *                    connection IDs and other relevant information.
 *
 * This function performs the following steps:
 * - Generates a validation token for the client.
 * - Sets the destination and source connection IDs.
 * - Calculates the integrity tag and sets the token length.
 * - Encodes and sends the packet via the BIO network interface.
 *
 * Error handling is included for failures in CID generation, encoding, and
 * network transmiss
 */
static void port_send_retry(QUIC_PORT *port,
                            BIO_ADDR *peer,
                            QUIC_PKT_HDR *client_hdr)
{
    BIO_MSG msg[1];
    /*
     * Buffer is used for both marshalling the token as well as for the RETRY
     * packet. The size of buffer should not be less than
     * MARSHALLED_TOKEN_MAX_LEN.
     */
    unsigned char buffer[512];
    unsigned char ct_buf[ENCRYPTED_TOKEN_MAX_LEN];
    WPACKET wpkt;
    size_t written, token_buf_len, ct_len;
    QUIC_PKT_HDR hdr = {0};
    QUIC_VALIDATION_TOKEN token = {0};
    int ok;

    if (!ossl_assert(sizeof(buffer) >= MARSHALLED_TOKEN_MAX_LEN))
        return;
    /*
     * 17.2.5.1 Sending a Retry packet
     *   dst ConnId is src ConnId we got from client
     *   src ConnId comes from local conn ID manager
     */
    memset(&hdr, 0, sizeof(QUIC_PKT_HDR));
    hdr.dst_conn_id = client_hdr->src_conn_id;
    /*
     * this is the random connection ID, we expect client is
     * going to send the ID with next INITIAL packet which
     * will also come with token we generate here.
     */
    ok = ossl_quic_lcidm_get_unused_cid(port->lcidm, &hdr.src_conn_id);
    if (ok == 0)
        goto err;

    memset(&token, 0, sizeof(QUIC_VALIDATION_TOKEN));

    /* Generate retry validation token */
    if (!generate_token(peer, client_hdr->dst_conn_id,
                        hdr.src_conn_id, &token, 1)
        || !marshal_validation_token(&token, buffer, &token_buf_len)
        || !encrypt_validation_token(port, buffer, token_buf_len, NULL,
                                     &ct_len)
        || ct_len > ENCRYPTED_TOKEN_MAX_LEN
        || !encrypt_validation_token(port, buffer, token_buf_len, ct_buf,
                                     &ct_len)
        || !ossl_assert(ct_len >= QUIC_RETRY_INTEGRITY_TAG_LEN))
        goto err;

    hdr.dst_conn_id = client_hdr->src_conn_id;
    hdr.type = QUIC_PKT_TYPE_RETRY;
    hdr.fixed = 1;
    hdr.version = 1;
    hdr.len = ct_len;
    hdr.data = ct_buf;
    ok = ossl_quic_calculate_retry_integrity_tag(port->engine->libctx,
                                                 port->engine->propq, &hdr,
                                                 &client_hdr->dst_conn_id,
                                                 ct_buf + ct_len
                                                 - QUIC_RETRY_INTEGRITY_TAG_LEN);
    if (ok == 0)
        goto err;

    hdr.token = hdr.data;
    hdr.token_len = hdr.len;

    msg[0].data = buffer;
    msg[0].peer = peer;
    msg[0].local = NULL;
    msg[0].flags = 0;

    ok = WPACKET_init_static_len(&wpkt, buffer, sizeof(buffer), 0);
    if (ok == 0)
        goto err;

    ok = ossl_quic_wire_encode_pkt_hdr(&wpkt, client_hdr->dst_conn_id.id_len,
                                       &hdr, NULL);
    if (ok == 0)
        goto err;

    ok = WPACKET_get_total_written(&wpkt, &msg[0].data_len);
    if (ok == 0)
        goto err;

    ok = WPACKET_finish(&wpkt);
    if (ok == 0)
        goto err;

    /*
     * TODO(QUIC FUTURE) need to retry this in the event it return EAGAIN
     * on a non-blocking BIO
     */
    if (!BIO_sendmmsg(port->net_wbio, msg, sizeof(BIO_MSG), 1, 0, &written))
        ERR_raise_data(ERR_LIB_SSL, SSL_R_QUIC_NETWORK_ERROR,
                       "port retry send failed due to network BIO I/O error");

err:
    cleanup_validation_token(&token);
}

/**
 * @brief Sends a QUIC Version Negotiation packet to the specified peer.
 *
 * This function constructs and sends a Version Negotiation packet using
 * the connection IDs from the client's initial packet header. The
 * Version Negotiation packet indicates support for QUIC version 1.
 *
 * @param port      Pointer to the QUIC_PORT structure representing the port
 *                  context used for network communication.
 * @param peer      Pointer to the BIO_ADDR structure specifying the address
 *                  of the peer to which the Version Negotiation packet
 *                  will be sent.
 * @param client_hdr Pointer to the QUIC_PKT_HDR structure containing the
 *                  client's packet header used to extract connection IDs.
 *
 * @note The function will raise an error if sending the message fails.
 */
static void port_send_version_negotiation(QUIC_PORT *port, BIO_ADDR *peer,
                                          QUIC_PKT_HDR *client_hdr)
{
    BIO_MSG msg[1];
    unsigned char buffer[1024];
    QUIC_PKT_HDR hdr;
    WPACKET wpkt;
    uint32_t supported_versions[1];
    size_t written;
    size_t i;

    memset(&hdr, 0, sizeof(QUIC_PKT_HDR));
    /*
     * Reverse the source and dst conn ids
     */
    hdr.dst_conn_id = client_hdr->src_conn_id;
    hdr.src_conn_id = client_hdr->dst_conn_id;

    /*
     * This is our list of supported protocol versions
     * Currently only QUIC_VERSION_1
     */
    supported_versions[0] = QUIC_VERSION_1;

    /*
     * Fill out the header fields
     * Note: Version negotiation packets, must, unlike
     * other packet types have a version of 0
     */
    hdr.type = QUIC_PKT_TYPE_VERSION_NEG;
    hdr.version = 0;
    hdr.token = 0;
    hdr.token_len = 0;
    hdr.len = sizeof(supported_versions);
    hdr.data = (unsigned char *)supported_versions;

    msg[0].data = buffer;
    msg[0].peer = peer;
    msg[0].local = NULL;
    msg[0].flags = 0;

    if (!WPACKET_init_static_len(&wpkt, buffer, sizeof(buffer), 0))
        return;

    if (!ossl_quic_wire_encode_pkt_hdr(&wpkt, client_hdr->dst_conn_id.id_len,
                                       &hdr, NULL))
        return;

    /*
     * Add the array of supported versions to the end of the packet
     */
    for (i = 0; i < OSSL_NELEM(supported_versions); i++) {
        if (!WPACKET_put_bytes_u32(&wpkt, htonl(supported_versions[i])))
            return;
    }

    if (!WPACKET_get_total_written(&wpkt, &msg[0].data_len))
        return;

    if (!WPACKET_finish(&wpkt))
        return;

    /*
     * Send it back to the client attempting to connect
     * TODO(QUIC FUTURE): Need to handle the EAGAIN case here, if the
     * BIO_sendmmsg call falls in a retryable manner
     */
    if (!BIO_sendmmsg(port->net_wbio, msg, sizeof(BIO_MSG), 1, 0, &written))
        ERR_raise_data(ERR_LIB_SSL, SSL_R_QUIC_NETWORK_ERROR,
                       "port version negotiation send failed");
}

/**
 * @brief defintions of token lifetimes
 *
 * RETRY tokens are only valid for 10 seconds
 * NEW_TOKEN tokens have a lifetime of 3600 sec (1 hour)
 */

#define RETRY_LIFETIME 10
#define NEW_TOKEN_LIFETIME 3600
/**
 * @brief Validates a received token in a QUIC packet header.
 *
 * This function checks the validity of a token contained in the provided
 * QUIC packet header (`QUIC_PKT_HDR *hdr`). The validation process involves
 * verifying that the token matches an expected format and value. If the
 * token is from a RETRY packet, the function extracts the original connection
 * ID (ODCID)/original source connection ID (SCID) and stores it in the provided
 * parameters. If the token is from a NEW_TOKEN packet, the values will be
 * derived instead.
 *
 * @param hdr   Pointer to the QUIC packet header containing the token.
 * @param port  Pointer to the QUIC port from which to send the packet.
 * @param peer  Address of the client peer receiving the packet.
 * @param odcid Pointer to the connection ID structure to store the ODCID if the
 *              token is valid.
 * @param scid  Pointer to the connection ID structure to store the SCID if the
 *              token is valid.
 *
 * @return      1 if the token is valid and ODCID/SCID are successfully set.
 *              0 otherwise.
 *
 * The function performs the following checks:
 * - Token length meets the required minimum.
 * - Buffer matches expected format.
 * - Peer address matches previous connection address.
 * - Token has not expired. Currently set to 10 seconds for tokens from RETRY
 *   packets and 60 minutes for tokens from NEW_TOKEN packets. This may be
 *   configurable in the future.
 */
static int port_validate_token(QUIC_PKT_HDR *hdr, QUIC_PORT *port,
                               BIO_ADDR *peer, QUIC_CONN_ID *odcid,
                               QUIC_CONN_ID *scid, uint8_t *gen_new_token)
{
    int ret = 0;
    QUIC_VALIDATION_TOKEN token = { 0 };
    uint64_t time_diff;
    size_t remote_addr_len, dec_token_len;
    unsigned char *remote_addr = NULL, dec_token[MARSHALLED_TOKEN_MAX_LEN];
    OSSL_TIME now = ossl_time_now();

    *gen_new_token = 0;

    if (!decrypt_validation_token(port, hdr->token, hdr->token_len, NULL,
                                  &dec_token_len)
        || dec_token_len > MARSHALLED_TOKEN_MAX_LEN
        || !decrypt_validation_token(port, hdr->token, hdr->token_len,
                                     dec_token, &dec_token_len)
        || !parse_validation_token(&token, dec_token, dec_token_len))
        goto err;

    /*
     * Validate token timestamp. Current time should not be before the token
     * timestamp.
     */
    if (ossl_time_compare(now, token.timestamp) < 0)
        goto err;
    time_diff = ossl_time2seconds(ossl_time_abs_difference(token.timestamp,
                                                           now));
    if ((token.is_retry && time_diff > RETRY_LIFETIME)
        || (!token.is_retry && time_diff > NEW_TOKEN_LIFETIME))
        goto err;

    /* Validate remote address */
    if (!BIO_ADDR_rawaddress(peer, NULL, &remote_addr_len)
        || remote_addr_len != token.remote_addr_len
        || (remote_addr = OPENSSL_malloc(remote_addr_len)) == NULL
        || !BIO_ADDR_rawaddress(peer, remote_addr, &remote_addr_len)
        || memcmp(remote_addr, token.remote_addr, remote_addr_len) != 0)
        goto err;

    /*
     * Set ODCID and SCID. If the token is from a RETRY packet, retrieve both
     * from the token. Otherwise, generate a new ODCID and use the header's
     * source connection ID for SCID.
     */
    if (token.is_retry) {
        /*
         * We're parsing a packet header before its gone through AEAD validation
         * here, so there is a chance we are dealing with corrupted data. Make
         * Sure the dcid encoded in the token matches the headers dcid to
         * mitigate that.
         * TODO(QUIC FUTURE): Consider handling AEAD validation at the port
         * level rather than the QRX/channel level to eliminate the need for
         * this.
         */
        if (token.rscid.id_len != hdr->dst_conn_id.id_len
            || memcmp(&token.rscid.id, &hdr->dst_conn_id.id,
                      token.rscid.id_len) != 0)
            goto err;
        *odcid = token.odcid;
        *scid = token.rscid;
    } else {
        if (!ossl_quic_lcidm_get_unused_cid(port->lcidm, odcid))
            goto err;
        *scid = hdr->src_conn_id;
    }

    /*
     * Determine if we need to send a NEW_TOKEN frame
     * If we validated a retry token, we should always
     * send a NEW_TOKEN frame to the client
     *
     * If however, we validated a NEW_TOKEN, which may be
     * reused multiple times, only send a NEW_TOKEN frame
     * if the existing received token has less than 10% of its lifetime
     * remaining.  This prevents us from constantly sending
     * NEW_TOKEN frames on every connection when not needed
     */
    if (token.is_retry) {
        *gen_new_token = 1;
    } else {
        if (time_diff > ((NEW_TOKEN_LIFETIME * 9) / 10))
            *gen_new_token = 1;
    }

    ret = 1;
err:
    cleanup_validation_token(&token);
    OPENSSL_free(remote_addr);
    return ret;
}

static void generate_new_token(QUIC_CHANNEL *ch, BIO_ADDR *peer)
{
    QUIC_CONN_ID rscid = { 0 };
    QUIC_VALIDATION_TOKEN token;
    unsigned char buffer[ENCRYPTED_TOKEN_MAX_LEN];
    unsigned char *ct_buf;
    size_t ct_len;
    size_t token_buf_len = 0;

    /* Clients never send a NEW_TOKEN */
    if (!ch->is_server)
        return;

    ct_buf = OPENSSL_zalloc(ENCRYPTED_TOKEN_MAX_LEN);
    if (ct_buf == NULL)
        return;

    /*
     * NEW_TOKEN tokens may be used for multiple subsequent connections
     * within their timeout period, so don't reserve an rscid here
     * like we do for retry tokens, instead, just fill it with random
     * data, as we won't use it anyway
     */
    rscid.id_len = 8;
    if (!RAND_bytes_ex(ch->port->engine->libctx, rscid.id, 8, 0)) {
        OPENSSL_free(ct_buf);
        return;
    }

    memset(&token, 0, sizeof(QUIC_VALIDATION_TOKEN));

    if (!generate_token(peer, ch->init_dcid, rscid, &token, 0)
        || !marshal_validation_token(&token, buffer, &token_buf_len)
        || !encrypt_validation_token(ch->port, buffer, token_buf_len, NULL,
                                     &ct_len)
        || ct_len > ENCRYPTED_TOKEN_MAX_LEN
        || !encrypt_validation_token(ch->port, buffer, token_buf_len, ct_buf,
                                     &ct_len)
        || !ossl_assert(ct_len >= QUIC_RETRY_INTEGRITY_TAG_LEN)) {
        OPENSSL_free(ct_buf);
        cleanup_validation_token(&token);
        return;
    }

    ch->pending_new_token = ct_buf;
    ch->pending_new_token_len = ct_len;

    cleanup_validation_token(&token);
}

/*
 * This is called by the demux when we get a packet not destined for any known
 * DCID.
 */
static void port_default_packet_handler(QUIC_URXE *e, void *arg,
                                        const QUIC_CONN_ID *dcid)
{
    QUIC_PORT *port = arg;
    PACKET pkt;
    QUIC_PKT_HDR hdr;
    QUIC_CHANNEL *ch = NULL, *new_ch = NULL;
    QUIC_CONN_ID odcid, scid;
    uint8_t gen_new_token = 0;
    OSSL_QRX *qrx = NULL;
    OSSL_QRX *qrx_src = NULL;
    OSSL_QRX_ARGS qrx_args = {0};
    uint64_t cause_flags = 0;
    OSSL_QRX_PKT *qrx_pkt = NULL;

    /* Don't handle anything if we are no longer running. */
    if (!ossl_quic_port_is_running(port))
        goto undesirable;

    if (port_try_handle_stateless_reset(port, e))
        goto undesirable;

    if (dcid != NULL
        && ossl_quic_lcidm_lookup(port->lcidm, dcid, NULL,
                                  (void **)&ch)) {
        assert(ch != NULL);
        ossl_quic_channel_inject(ch, e);
        return;
    }

    /*
     * If we have an incoming packet which doesn't match any existing connection
     * we assume this is an attempt to make a new connection.
     */
    if (!port->allow_incoming)
        goto undesirable;

    /*
     * We have got a packet for an unknown DCID. This might be an attempt to
     * open a new connection.
     */
    if (e->data_len < QUIC_MIN_INITIAL_DGRAM_LEN)
        goto undesirable;

    if (!PACKET_buf_init(&pkt, ossl_quic_urxe_data(e), e->data_len))
        goto undesirable;

    /*
     * We set short_conn_id_len to SIZE_MAX here which will cause the decode
     * operation to fail if we get a 1-RTT packet. This is fine since we only
     * care about Initial packets.
     */
    if (!ossl_quic_wire_decode_pkt_hdr(&pkt, SIZE_MAX, 1, 0, &hdr, NULL,
                                       &cause_flags)) {
        /*
         * If we fail due to a bad version, we know the packet up to the version
         * number was decoded, and we use it below to send a version
         * negotiation packet
         */
        if ((cause_flags & QUIC_PKT_HDR_DECODE_BAD_VERSION) == 0)
            goto undesirable;
    }

    switch (hdr.version) {
    case QUIC_VERSION_1:
        break;

    case QUIC_VERSION_NONE:
    default:

        /*
         * If we get here, then we have a bogus version, and might need
         * to send a version negotiation packet.  According to
         * RFC 9000 s. 6 and 14.1, we only do so however, if the UDP datagram
         * is a minimum of 1200 bytes in size
         */
        if (e->data_len < 1200)
            goto undesirable;

        /*
         * If we don't get a supported version, respond with a ver
         * negotiation packet, and discard
         * TODO(QUIC FUTURE): Rate limit the reception of these
         */
        port_send_version_negotiation(port, &e->peer, &hdr);
        goto undesirable;
    }

    /*
     * We only care about Initial packets which might be trying to establish a
     * connection.
     */
    if (hdr.type != QUIC_PKT_TYPE_INITIAL)
        goto undesirable;

    odcid.id_len = 0;

    /*
     * Create qrx now so we can check integrity of packet
     * which does not belong to any channel.
     */
    qrx_args.libctx             = port->engine->libctx;
    qrx_args.demux              = port->demux;
    qrx_args.short_conn_id_len  = dcid->id_len;
    qrx_args.max_deferred       = 32;
    qrx = ossl_qrx_new(&qrx_args);
    if (qrx == NULL)
        goto undesirable;

    /*
     * Derive secrets for qrx only.
     */
    if (!ossl_quic_provide_initial_secret(port->engine->libctx,
                                          port->engine->propq,
                                          &hdr.dst_conn_id,
                                          /* is_server */ 1,
                                          qrx, NULL))
        goto undesirable;

    if (ossl_qrx_validate_initial_packet(qrx, e, (const QUIC_CONN_ID *)dcid) == 0)
        goto undesirable;

    if (port->validate_addr == 0) {
        /*
         * Forget qrx, because it becomes (almost) useless here. We must let
         * channel to create a new QRX for connection ID server chooses. The
         * validation keys for new DCID will be derived by
         * ossl_quic_channel_on_new_conn() when we will be creating channel.
         * See RFC 9000 section 7.2 negotiating connection id to better
         * understand what's going on here.
         *
         * Did we say qrx is almost useless? Why? Because qrx remembers packets
         * we just validated. Those packets must be injected to channel we are
         * going to create. We use qrx_src alias so we can read packets from
         * qrx and inject them to channel.
         */
         qrx_src = qrx;
         qrx = NULL;
    }
    /*
     * TODO(QUIC FUTURE): there should be some logic similar to accounting half-open
     * states in TCP. If we reach certain threshold, then we want to
     * validate clients.
     */
    if (port->validate_addr == 1 && hdr.token == NULL) {
        port_send_retry(port, &e->peer, &hdr);
        goto undesirable;
    }

    /*
     * Note, even if we don't enforce the sending of retry frames for
     * server address validation, we may still get a token if we sent
     * a NEW_TOKEN frame during a prior connection, which we should still
     * validate here
     */
    if (hdr.token != NULL
        && port_validate_token(&hdr, port, &e->peer,
                               &odcid, &scid,
                               &gen_new_token) == 0) {
        /*
         * RFC 9000 s 8.1.3
         * When a server receives an Initial packet with an address
         * validation token, it MUST attempt to validate the token,
         * unless it has already completed address validation.
         * If the token is invalid, then the server SHOULD proceed as
         * if the client did not have a validated address,
         * including potentially sending a Retry packet
         * Note: If address validation is disabled, just act like
         * the request is valid
         */
        if (port->validate_addr == 1) {
            /*
             * Again: we should consider saving initial encryption level
             * secrets to token here to save some CPU cycles.
             */
            port_send_retry(port, &e->peer, &hdr);
            goto undesirable;
        }

        /*
         * client is under amplification limit, until it completes
         * handshake.
         *
         * forget qrx so channel can create a new one
         * with valid initial encryption level keys.
         */
        qrx_src = qrx;
        qrx = NULL;
    }

    port_bind_channel(port, &e->peer, &scid, &hdr.dst_conn_id,
                      &odcid, qrx, &new_ch);

    /*
     * if packet validates it gets moved to channel, we've just bound
     * to port.
     */
    if (new_ch == NULL)
        goto undesirable;

    /*
     * Generate a token for sending in a later NEW_TOKEN frame
     */
    if (gen_new_token == 1)
        generate_new_token(new_ch, &e->peer);

    if (qrx != NULL) {
        /*
         * The qrx belongs to channel now, so don't free it.
         */
        qrx = NULL;
    } else {
        /*
         * We still need to salvage packets from almost forgotten qrx
         * and pass them to channel.
         */
        while (ossl_qrx_read_pkt(qrx_src, &qrx_pkt) == 1)
            ossl_quic_channel_inject_pkt(new_ch, qrx_pkt);
    }

    /*
     * If function reaches this place, then packet got validated in
     * ossl_qrx_validate_initial_packet(). Keep in mind the function
     * ossl_qrx_validate_initial_packet() decrypts the packet to validate it.
     * If packet validation was successful (and it was because we are here),
     * then the function puts the packet to qrx->rx_pending. We must not call
     * ossl_qrx_inject_urxe() here now, because we don't want to insert
     * the packet to qrx->urx_pending which keeps packet waiting for decryption.
     *
     * We are going to call ossl_quic_demux_release_urxe() to dispose buffer
     * which still holds encrypted data.
     */

undesirable:
    ossl_qrx_free(qrx);
    ossl_qrx_free(qrx_src);
    ossl_quic_demux_release_urxe(port->demux, e);
}

void ossl_quic_port_raise_net_error(QUIC_PORT *port,
                                    QUIC_CHANNEL *triggering_ch)
{
    QUIC_CHANNEL *ch;

    if (!ossl_quic_port_is_running(port))
        return;

    /*
     * Immediately capture any triggering error on the error stack, with a
     * cover error.
     */
    ERR_raise_data(ERR_LIB_SSL, SSL_R_QUIC_NETWORK_ERROR,
                   "port failed due to network BIO I/O error");
    OSSL_ERR_STATE_save(port->err_state);

    port_transition_failed(port);

    /* Give the triggering channel (if any) the first notification. */
    if (triggering_ch != NULL)
        ossl_quic_channel_raise_net_error(triggering_ch);

    OSSL_LIST_FOREACH(ch, ch, &port->channel_list)
        if (ch != triggering_ch)
            ossl_quic_channel_raise_net_error(ch);
}

void ossl_quic_port_restore_err_state(const QUIC_PORT *port)
{
    ERR_clear_error();
    OSSL_ERR_STATE_restore(port->err_state);
}
