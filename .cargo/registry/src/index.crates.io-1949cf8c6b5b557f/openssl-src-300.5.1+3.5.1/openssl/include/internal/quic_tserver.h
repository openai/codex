/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_TSERVER_H
# define OSSL_QUIC_TSERVER_H

# include <openssl/ssl.h>
# include <openssl/bio.h>
# include "internal/quic_stream.h"
# include "internal/quic_channel.h"
# include "internal/statem.h"
# include "internal/time.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Test Server Module
 * =======================
 *
 * This implements a QUIC test server. Since full QUIC server support is not yet
 * implemented this server is limited in features and scope. It exists to
 * provide a target for our QUIC client to talk to for testing purposes.
 *
 * A given QUIC test server instance supports only one client at a time.
 *
 * Note that this test server is not suitable for production use because it does
 * not implement address verification, anti-amplification or retry logic.
 */
typedef struct quic_tserver_st QUIC_TSERVER;

typedef struct quic_tserver_args_st {
    OSSL_LIB_CTX *libctx;
    const char *propq;
    SSL_CTX *ctx;
    BIO *net_rbio, *net_wbio;
    OSSL_TIME (*now_cb)(void *arg);
    void *now_cb_arg;
    const unsigned char *alpn;
    size_t alpnlen;
} QUIC_TSERVER_ARGS;

QUIC_TSERVER *ossl_quic_tserver_new(const QUIC_TSERVER_ARGS *args,
                                    const char *certfile, const char *keyfile);

void ossl_quic_tserver_free(QUIC_TSERVER *srv);

/* Set mutator callbacks for test framework support */
int ossl_quic_tserver_set_plain_packet_mutator(QUIC_TSERVER *srv,
                                               ossl_mutate_packet_cb mutatecb,
                                               ossl_finish_mutate_cb finishmutatecb,
                                               void *mutatearg);

int ossl_quic_tserver_set_handshake_mutator(QUIC_TSERVER *srv,
                                            ossl_statem_mutate_handshake_cb mutate_handshake_cb,
                                            ossl_statem_finish_mutate_handshake_cb finish_mutate_handshake_cb,
                                            void *mutatearg);

/* Advances the state machine. */
int ossl_quic_tserver_tick(QUIC_TSERVER *srv);

/* Returns 1 if we have a (non-terminated) client. */
int ossl_quic_tserver_is_connected(QUIC_TSERVER *srv);

/*
 * Returns 1 if we have finished the TLS handshake
 */
int ossl_quic_tserver_is_handshake_confirmed(const QUIC_TSERVER *srv);

/* Returns 1 if the server is in any terminating or terminated state */
int ossl_quic_tserver_is_term_any(const QUIC_TSERVER *srv);

const QUIC_TERMINATE_CAUSE *
ossl_quic_tserver_get_terminate_cause(const QUIC_TSERVER *srv);

/* Returns 1 if the server is in a terminated state */
int ossl_quic_tserver_is_terminated(const QUIC_TSERVER *srv);

/* Get out short header conn id length */
size_t ossl_quic_tserver_get_short_header_conn_id_len(const QUIC_TSERVER *srv);

/*
 * Attempts to read from stream 0. Writes the number of bytes read to
 * *bytes_read and returns 1 on success. If no bytes are available, 0 is written
 * to *bytes_read and 1 is returned (this is considered a success case).
 *
 * Returns 0 if connection is not currently active. If the receive part of
 * the stream has reached the end of stream condition, returns 0; call
 * ossl_quic_tserver_has_read_ended() to identify this condition.
 */
int ossl_quic_tserver_read(QUIC_TSERVER *srv,
                           uint64_t stream_id,
                           unsigned char *buf,
                           size_t buf_len,
                           size_t *bytes_read);

/*
 * Returns 1 if the read part of the stream has ended normally.
 */
int ossl_quic_tserver_has_read_ended(QUIC_TSERVER *srv, uint64_t stream_id);

/*
 * Attempts to write to the given stream. Writes the number of bytes consumed to
 * *bytes_written and returns 1 on success. If there is no space currently
 * available to write any bytes, 0 is written to *consumed and 1 is returned
 * (this is considered a success case).
 *
 * Note that unlike libssl public APIs, this API always works in a 'partial
 * write' mode.
 *
 * Returns 0 if connection is not currently active.
 */
int ossl_quic_tserver_write(QUIC_TSERVER *srv,
                            uint64_t stream_id,
                            const unsigned char *buf,
                            size_t buf_len,
                            size_t *bytes_written);

/*
 * Signals normal end of the stream.
 */
int ossl_quic_tserver_conclude(QUIC_TSERVER *srv, uint64_t stream_id);

/*
 * Create a server-initiated stream. The stream ID of the newly
 * created stream is written to *stream_id.
 */
int ossl_quic_tserver_stream_new(QUIC_TSERVER *srv,
                                 int is_uni,
                                 uint64_t *stream_id);

BIO *ossl_quic_tserver_get0_rbio(QUIC_TSERVER *srv);

SSL_CTX *ossl_quic_tserver_get0_ssl_ctx(QUIC_TSERVER *srv);

/*
 * Returns 1 if the peer has sent a STOP_SENDING frame for a stream.
 * app_error_code is written if this returns 1.
 */
int ossl_quic_tserver_stream_has_peer_stop_sending(QUIC_TSERVER *srv,
                                                   uint64_t stream_id,
                                                   uint64_t *app_error_code);

/*
 * Returns 1 if the peer has sent a RESET_STREAM frame for a stream.
 * app_error_code is written if this returns 1.
 */
int ossl_quic_tserver_stream_has_peer_reset_stream(QUIC_TSERVER *srv,
                                                   uint64_t stream_id,
                                                   uint64_t *app_error_code);

/*
 * Replaces existing local connection ID in the underlying QUIC_CHANNEL.
 */
int ossl_quic_tserver_set_new_local_cid(QUIC_TSERVER *srv,
                                        const QUIC_CONN_ID *conn_id);

/*
 * Returns the stream ID of the next incoming stream, or UINT64_MAX if there
 * currently is none.
 */
uint64_t ossl_quic_tserver_pop_incoming_stream(QUIC_TSERVER *srv);

/*
 * Returns 1 if all data sent on the given stream_id has been acked by the peer.
 */
int ossl_quic_tserver_is_stream_totally_acked(QUIC_TSERVER *srv,
                                              uint64_t stream_id);

/* Returns 1 if we are currently interested in reading data from the network */
int ossl_quic_tserver_get_net_read_desired(QUIC_TSERVER *srv);

/* Returns 1 if we are currently interested in writing data to the network */
int ossl_quic_tserver_get_net_write_desired(QUIC_TSERVER *srv);

/* Returns the next event deadline */
OSSL_TIME ossl_quic_tserver_get_deadline(QUIC_TSERVER *srv);

/*
 * Shutdown the QUIC connection. Returns 1 if the connection is terminated and
 * 0 otherwise.
 */
int ossl_quic_tserver_shutdown(QUIC_TSERVER *srv, uint64_t app_error_code);

/* Force generation of an ACK-eliciting packet. */
int ossl_quic_tserver_ping(QUIC_TSERVER *srv);

/* Set tracing callback on channel. */
void ossl_quic_tserver_set_msg_callback(QUIC_TSERVER *srv,
                                        void (*f)(int write_p, int version,
                                                  int content_type,
                                                  const void *buf, size_t len,
                                                  SSL *ssl, void *arg),
                                        void *arg);

/*
 * This is similar to ossl_quic_conn_get_channel; it should be used for test
 * instrumentation only and not to bypass QUIC_TSERVER for 'normal' operations.
 */
QUIC_CHANNEL *ossl_quic_tserver_get_channel(QUIC_TSERVER *srv);

/* Send a TLS new session ticket */
int ossl_quic_tserver_new_ticket(QUIC_TSERVER *srv);

/*
 * Set the max_early_data value to be sent in NewSessionTickets. Only the
 * values 0 and 0xffffffff are valid for use in QUIC.
 */
int ossl_quic_tserver_set_max_early_data(QUIC_TSERVER *srv,
                                         uint32_t max_early_data);

/* Set the find session callback for getting a server PSK */
void ossl_quic_tserver_set_psk_find_session_cb(QUIC_TSERVER *srv,
                                               SSL_psk_find_session_cb_func cb);

# endif

#endif
