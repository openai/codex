/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_PORT_LOCAL_H
# define OSSL_QUIC_PORT_LOCAL_H

# include "internal/quic_port.h"
# include "internal/quic_reactor.h"
# include "internal/list.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Port Structure
 * ===================
 *
 * QUIC port internals. It is intended that only the QUIC_PORT and QUIC_CHANNEL
 * implementation be allowed to access this structure directly.
 *
 * Other components should not include this header.
 */
DECLARE_LIST_OF(ch, QUIC_CHANNEL);
DECLARE_LIST_OF(incoming_ch, QUIC_CHANNEL);

/* A port is always in one of the following states: */
enum {
    /* Initial and steady state. */
    QUIC_PORT_STATE_RUNNING,

    /*
     * Terminal state indicating port is no longer functioning. There are no
     * transitions out of this state. May be triggered by e.g. a permanent
     * network BIO error.
     */
    QUIC_PORT_STATE_FAILED
};

struct quic_port_st {
    /* The engine which this port is a child of. */
    QUIC_ENGINE                     *engine;

    /*
     * QUIC_ENGINE keeps the ports which belong to it on a list for bookkeeping
     * purposes.
     */
    OSSL_LIST_MEMBER(port, QUIC_PORT);

    SSL * (*get_conn_user_ssl)(QUIC_CHANNEL *ch, void *arg);
    void *user_ssl_arg;

    /* Used to create handshake layer objects inside newly created channels. */
    SSL_CTX                         *channel_ctx;

    /* Network-side read and write BIOs. */
    BIO                             *net_rbio, *net_wbio;

    /* RX demuxer. We register incoming DCIDs with this. */
    QUIC_DEMUX                      *demux;

    /* List of all child channels. */
    OSSL_LIST(ch)                   channel_list;

    /*
     * Queue of unaccepted incoming channels. Each such channel is also on
     * channel_list.
     */
    OSSL_LIST(incoming_ch)          incoming_channel_list;

    /* Special TSERVER channel. To be removed in the future. */
    QUIC_CHANNEL                    *tserver_ch;

    /* LCIDM used for incoming packet routing by DCID. */
    QUIC_LCIDM                      *lcidm;

    /* SRTM used for incoming packet routing by SRT. */
    QUIC_SRTM                       *srtm;

    /* Port-level permanent errors (causing failure state) are stored here. */
    ERR_STATE                       *err_state;

    /* DCID length used for incoming short header packets. */
    unsigned char                   rx_short_dcid_len;
    /* For clients, CID length used for outgoing Initial packets. */
    unsigned char                   tx_init_dcid_len;

    /* Port state (QUIC_PORT_STATE_*). */
    unsigned int                    state                           : 1;

    /* Is this port created to support multiple connections? */
    unsigned int                    is_multi_conn                   : 1;

    /* Is this port doing server address validation */
    unsigned int                    validate_addr                   : 1;

    /* Has this port sent any packet of any kind yet? */
    unsigned int                    have_sent_any_pkt               : 1;

    /* Does this port allow incoming connections? */
    unsigned int                    allow_incoming                  : 1;

    /* Are we on the QUIC_ENGINE linked list of ports? */
    unsigned int                    on_engine_list                  : 1;

    /* Are we using addressed mode (BIO_sendmmsg with non-NULL peer)? */
    unsigned int                    addressed_mode_w                : 1;
    unsigned int                    addressed_mode_r                : 1;

    /* Has the BIO been changed since we last updated reactor pollability? */
    unsigned int                    bio_changed                     : 1;

    /* AES-256 GCM context for token encryption */
    EVP_CIPHER_CTX *token_ctx;
};

# endif

#endif
