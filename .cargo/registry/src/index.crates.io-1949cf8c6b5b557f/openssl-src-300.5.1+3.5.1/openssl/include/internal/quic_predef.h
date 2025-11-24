/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_PREDEF_H
# define OSSL_QUIC_PREDEF_H

# ifndef OPENSSL_NO_QUIC

typedef struct ssl_token_store_st SSL_TOKEN_STORE;
typedef struct quic_port_st QUIC_PORT;
typedef struct quic_channel_st QUIC_CHANNEL;
typedef struct quic_txpim_st QUIC_TXPIM;
typedef struct quic_fifd_st QUIC_FIFD;
typedef struct quic_cfq_st QUIC_CFQ;
typedef struct ossl_quic_tx_packetiser_st OSSL_QUIC_TX_PACKETISER;
typedef struct ossl_ackm_st OSSL_ACKM;
typedef struct quic_srt_elem_st QUIC_SRT_ELEM;
typedef struct ossl_cc_data_st OSSL_CC_DATA;
typedef struct ossl_cc_method_st OSSL_CC_METHOD;
typedef struct quic_stream_map_st QUIC_STREAM_MAP;
typedef struct quic_stream_st QUIC_STREAM;
typedef struct quic_sstream_st QUIC_SSTREAM;
typedef struct quic_rstream_st QUIC_RSTREAM;
typedef struct quic_reactor_st QUIC_REACTOR;
typedef struct quic_reactor_wait_ctx_st QUIC_REACTOR_WAIT_CTX;
typedef struct ossl_statm_st OSSL_STATM;
typedef struct quic_demux_st QUIC_DEMUX;
typedef struct ossl_qrx_st OSSL_QRX;
typedef struct ossl_qrx_pkt_st OSSL_QRX_PKT;
typedef struct ossl_qtx_pkt_st OSSL_QTX_PKT;
typedef struct quic_tick_result_st QUIC_TICK_RESULT;
typedef struct quic_srtm_st QUIC_SRTM;
typedef struct quic_lcidm_st QUIC_LCIDM;
typedef struct quic_urxe_st QUIC_URXE;
typedef struct quic_engine_st QUIC_ENGINE;
typedef struct quic_obj_st QUIC_OBJ;
typedef struct quic_conn_st QUIC_CONNECTION;
typedef struct quic_xso_st QUIC_XSO;
typedef struct quic_listener_st QUIC_LISTENER;
typedef struct quic_domain_st QUIC_DOMAIN;

# endif

#endif
