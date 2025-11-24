/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_TXP_H
# define OSSL_QUIC_TXP_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/quic_predef.h"
# include "internal/quic_record_tx.h"
# include "internal/quic_cfq.h"
# include "internal/quic_txpim.h"
# include "internal/quic_stream.h"
# include "internal/quic_stream_map.h"
# include "internal/quic_fc.h"
# include "internal/bio_addr.h"
# include "internal/time.h"
# include "internal/qlog.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC TX Packetiser
 * ==================
 */
typedef struct ossl_quic_tx_packetiser_args_st {
    /* Configuration Settings */
    QUIC_CONN_ID    cur_scid;   /* Current Source Connection ID we use. */
    QUIC_CONN_ID    cur_dcid;   /* Current Destination Connection ID we use. */
    BIO_ADDR        peer;       /* Current destination L4 address we use. */
    uint32_t        ack_delay_exponent; /* ACK delay exponent used when encoding. */

    /* Injected Dependencies */
    OSSL_QTX        *qtx;       /* QUIC Record Layer TX we are using */
    QUIC_TXPIM      *txpim;     /* QUIC TX'd Packet Information Manager */
    QUIC_CFQ        *cfq;       /* QUIC Control Frame Queue */
    OSSL_ACKM       *ackm;      /* QUIC Acknowledgement Manager */
    QUIC_STREAM_MAP *qsm;       /* QUIC Streams Map */
    QUIC_TXFC       *conn_txfc; /* QUIC Connection-Level TX Flow Controller */
    QUIC_RXFC       *conn_rxfc; /* QUIC Connection-Level RX Flow Controller */
    QUIC_RXFC       *max_streams_bidi_rxfc; /* QUIC RXFC for MAX_STREAMS generation */
    QUIC_RXFC       *max_streams_uni_rxfc;
    const OSSL_CC_METHOD *cc_method; /* QUIC Congestion Controller */
    OSSL_CC_DATA    *cc_data;   /* QUIC Congestion Controller Instance */
    OSSL_TIME       (*now)(void *arg);  /* Callback to get current time. */
    void            *now_arg;
    QLOG            *(*get_qlog_cb)(void *arg); /* Optional QLOG retrieval func */
    void            *get_qlog_cb_arg;
    uint32_t        protocol_version; /* The protocol version to try negotiating */

    /*
     * Injected dependencies - crypto streams.
     *
     * Note: There is no crypto stream for the 0-RTT EL.
     *       crypto[QUIC_PN_SPACE_APP] is the 1-RTT crypto stream.
     */
    QUIC_SSTREAM    *crypto[QUIC_PN_SPACE_NUM];

 } OSSL_QUIC_TX_PACKETISER_ARGS;

OSSL_QUIC_TX_PACKETISER *ossl_quic_tx_packetiser_new(const OSSL_QUIC_TX_PACKETISER_ARGS *args);

void ossl_quic_tx_packetiser_set_validated(OSSL_QUIC_TX_PACKETISER *txp);
void ossl_quic_tx_packetiser_add_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                    size_t credit);
void ossl_quic_tx_packetiser_consume_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                        size_t credit);
int ossl_quic_tx_packetiser_check_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                     size_t req_credit);

typedef void (ossl_quic_initial_token_free_fn)(const unsigned char *buf,
                                               size_t buf_len, void *arg);

void ossl_quic_tx_packetiser_free(OSSL_QUIC_TX_PACKETISER *txp);

/*
 * When in the closing state we need to maintain a count of received bytes
 * so that we can limit the number of close connection frames we send.
 * Refer RFC 9000 s. 10.2.1 Closing Connection State.
 */
void ossl_quic_tx_packetiser_record_received_closing_bytes(
        OSSL_QUIC_TX_PACKETISER *txp, size_t n);

/*
 * Generates a datagram by polling the various ELs to determine if they want to
 * generate any frames, and generating a datagram which coalesces packets for
 * any ELs which do.
 *
 * Returns 0 on failure (e.g. allocation error or other errors), 1 otherwise.
 *
 * *status is filled with status information about the generated packet.
 * It is always filled even in case of failure. In particular, packets can be
 * sent even if failure is later returned.
 * See QUIC_TXP_STATUS for details.
 */
typedef struct quic_txp_status_st {
    int sent_ack_eliciting; /* Was an ACK-eliciting packet sent? */
    int sent_handshake; /* Was a Handshake packet sent? */
    size_t sent_pkt; /* Number of packets sent (0 if nothing was sent) */
} QUIC_TXP_STATUS;

int ossl_quic_tx_packetiser_generate(OSSL_QUIC_TX_PACKETISER *txp,
                                     QUIC_TXP_STATUS *status);

/*
 * Returns a deadline after which a call to ossl_quic_tx_packetiser_generate()
 * might succeed even if it did not previously. This may return
 * ossl_time_infinite() if there is no such deadline currently applicable. It
 * returns ossl_time_zero() if there is (potentially) more data to be generated
 * immediately. The value returned is liable to change after any call to
 * ossl_quic_tx_packetiser_generate() (or after ACKM or CC state changes). Note
 * that ossl_quic_tx_packetiser_generate() can also start to succeed for other
 * non-chronological reasons, such as changes to send stream buffers, etc.
 */
OSSL_TIME ossl_quic_tx_packetiser_get_deadline(OSSL_QUIC_TX_PACKETISER *txp);

/*
 * Set the token used in Initial packets. The callback is called when the buffer
 * is no longer needed; for example, when the TXP is freed or when this function
 * is called again with a new buffer. Fails returning 0 if the token is too big
 * to ever be reasonably encapsulated in an outgoing packet based on our current
 * understanding of our PMTU.
 */
int ossl_quic_tx_packetiser_set_initial_token(OSSL_QUIC_TX_PACKETISER *txp,
                                              const unsigned char *token,
                                              size_t token_len,
                                              ossl_quic_initial_token_free_fn *free_cb,
                                              void *free_cb_arg);

/*
 * Set the protocol version used when generating packets.  Currently should
 * only ever be set to QUIC_VERSION_1
 */
int ossl_quic_tx_packetiser_set_protocol_version(OSSL_QUIC_TX_PACKETISER *txp,
                                                 uint32_t protocol_version);

/* Change the DCID the TXP uses to send outgoing packets. */
int ossl_quic_tx_packetiser_set_cur_dcid(OSSL_QUIC_TX_PACKETISER *txp,
                                         const QUIC_CONN_ID *dcid);

/* Change the SCID the TXP uses to send outgoing (long) packets. */
int ossl_quic_tx_packetiser_set_cur_scid(OSSL_QUIC_TX_PACKETISER *txp,
                                         const QUIC_CONN_ID *scid);

/*
 * Change the destination L4 address the TXP uses to send datagrams. Specify
 * NULL (or AF_UNSPEC) to disable use of addressed mode.
 */
int ossl_quic_tx_packetiser_set_peer(OSSL_QUIC_TX_PACKETISER *txp,
                                     const BIO_ADDR *peer);

/*
 * Change the QLOG instance retrieval function in use after instantiation.
 */
void ossl_quic_tx_packetiser_set_qlog_cb(OSSL_QUIC_TX_PACKETISER *txp,
                                         QLOG *(*get_qlog_cb)(void *arg),
                                         void *get_qlog_cb_arg);

/*
 * Inform the TX packetiser that an EL has been discarded. Idempotent.
 *
 * This does not inform the QTX as well; the caller must also inform the QTX.
 *
 * The TXP will no longer reference the crypto[enc_level] QUIC_SSTREAM which was
 * provided in the TXP arguments. However, it is the callers responsibility to
 * free that QUIC_SSTREAM if desired.
 */
int ossl_quic_tx_packetiser_discard_enc_level(OSSL_QUIC_TX_PACKETISER *txp,
                                              uint32_t enc_level);

/*
 * Informs the TX packetiser that the handshake is complete. The TX packetiser
 * will not send 1-RTT application data until the handshake is complete,
 * as the authenticity of the peer is not confirmed until the handshake
 * complete event occurs.
 */
void ossl_quic_tx_packetiser_notify_handshake_complete(OSSL_QUIC_TX_PACKETISER *txp);

/* Asks the TXP to generate a HANDSHAKE_DONE frame in the next 1-RTT packet. */
void ossl_quic_tx_packetiser_schedule_handshake_done(OSSL_QUIC_TX_PACKETISER *txp);

/* Asks the TXP to ensure the next packet in the given PN space is ACK-eliciting. */
void ossl_quic_tx_packetiser_schedule_ack_eliciting(OSSL_QUIC_TX_PACKETISER *txp,
                                                    uint32_t pn_space);

/*
 * Asks the TXP to ensure an ACK is put in the next packet in the given PN
 * space.
 */
void ossl_quic_tx_packetiser_schedule_ack(OSSL_QUIC_TX_PACKETISER *txp,
                                          uint32_t pn_space);

/*
 * Schedules a connection close. *f and f->reason are copied. This operation is
 * irreversible and causes all further packets generated by the TXP to contain a
 * CONNECTION_CLOSE frame. This function fails if it has already been called
 * successfully; the information in *f cannot be changed after the first
 * successful call to this function.
 */
int ossl_quic_tx_packetiser_schedule_conn_close(OSSL_QUIC_TX_PACKETISER *txp,
                                                const OSSL_QUIC_FRAME_CONN_CLOSE *f);

/* Setters for the msg_callback and msg_callback_arg */
void ossl_quic_tx_packetiser_set_msg_callback(OSSL_QUIC_TX_PACKETISER *txp,
                                              ossl_msg_cb msg_callback,
                                              SSL *msg_callback_ssl);
void ossl_quic_tx_packetiser_set_msg_callback_arg(OSSL_QUIC_TX_PACKETISER *txp,
                                                  void *msg_callback_arg);

/*
 * Determines the next PN which will be used for a given PN space.
 */
QUIC_PN ossl_quic_tx_packetiser_get_next_pn(OSSL_QUIC_TX_PACKETISER *txp,
                                            uint32_t pn_space);

/*
 * Sets a callback which is called whenever TXP sends an ACK frame. The callee
 * must not modify the ACK frame data. Can be used to snoop on PNs being ACKed.
 */
void ossl_quic_tx_packetiser_set_ack_tx_cb(OSSL_QUIC_TX_PACKETISER *txp,
                                           void (*cb)(const OSSL_QUIC_FRAME_ACK *ack,
                                                      uint32_t pn_space,
                                                      void *arg),
                                           void *cb_arg);

# endif

#endif
