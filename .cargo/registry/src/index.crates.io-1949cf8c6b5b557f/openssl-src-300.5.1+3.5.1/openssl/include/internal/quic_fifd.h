/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_FIFD_H
# define OSSL_QUIC_FIFD_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/quic_cfq.h"
# include "internal/quic_ackm.h"
# include "internal/quic_txpim.h"
# include "internal/quic_stream.h"
# include "internal/qlog.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Frame-in-Flight Dispatcher (FIFD)
 * ======================================
 */
struct quic_fifd_st {
    /* Internal data; use the ossl_quic_fifd functions. */
    QUIC_CFQ       *cfq;
    OSSL_ACKM      *ackm;
    QUIC_TXPIM     *txpim;
    QUIC_SSTREAM *(*get_sstream_by_id)(uint64_t stream_id,
                                       uint32_t pn_space,
                                       void *arg);
    void           *get_sstream_by_id_arg;
    void          (*regen_frame)(uint64_t frame_type,
                                 uint64_t stream_id,
                                 QUIC_TXPIM_PKT *pkt,
                                 void *arg);
    void           *regen_frame_arg;
    void          (*confirm_frame)(uint64_t frame_type,
                                   uint64_t stream_id,
                                   QUIC_TXPIM_PKT *pkt,
                                   void *arg);
    void           *confirm_frame_arg;
    void          (*sstream_updated)(uint64_t stream_id,
                                   void *arg);
    void           *sstream_updated_arg;
    QLOG         *(*get_qlog_cb)(void *arg);
    void           *get_qlog_cb_arg;
};

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
                        void *get_qlog_cb_arg);

void ossl_quic_fifd_cleanup(QUIC_FIFD *fifd); /* (no-op) */

int ossl_quic_fifd_pkt_commit(QUIC_FIFD *fifd, QUIC_TXPIM_PKT *pkt);

void ossl_quic_fifd_set_qlog_cb(QUIC_FIFD *fifd, QLOG *(*get_qlog_cb)(void *arg),
                                void *arg);

# endif

#endif
