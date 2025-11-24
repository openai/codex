/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QLOG_EVENT_HELPERS_H
# define OSSL_QLOG_EVENT_HELPERS_H

# include <openssl/ssl.h>
# include "internal/qlog.h"
# include "internal/quic_types.h"
# include "internal/quic_channel.h"
# include "internal/quic_txpim.h"
# include "internal/quic_record_tx.h"
# include "internal/quic_wire_pkt.h"

/* connectivity:connection_started */
void ossl_qlog_event_connectivity_connection_started(QLOG *qlog,
                                                     const QUIC_CONN_ID *init_dcid);

/* connectivity:connection_state_updated */
void ossl_qlog_event_connectivity_connection_state_updated(QLOG *qlog,
                                                           uint32_t old_state,
                                                           uint32_t new_state,
                                                           int handshake_complete,
                                                           int handshake_confirmed);

/* connectivity:connection_closed */
void ossl_qlog_event_connectivity_connection_closed(QLOG *qlog,
                                                    const QUIC_TERMINATE_CAUSE *tcause);

/* recovery:packet_lost */
void ossl_qlog_event_recovery_packet_lost(QLOG *qlog,
                                          const QUIC_TXPIM_PKT *tpkt);

/* transport:packet_sent */
void ossl_qlog_event_transport_packet_sent(QLOG *qlog,
                                           const QUIC_PKT_HDR *hdr,
                                           QUIC_PN pn,
                                           const OSSL_QTX_IOVEC *iovec,
                                           size_t numn_iovec,
                                           uint64_t datagram_id);

/* transport:packet_received */
void ossl_qlog_event_transport_packet_received(QLOG *qlog,
                                               const QUIC_PKT_HDR *hdr,
                                               QUIC_PN pn,
                                               const OSSL_QTX_IOVEC *iovec,
                                               size_t numn_iovec,
                                               uint64_t datagram_id);

#endif
