/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_TLS_H
# define OSSL_QUIC_TLS_H

# include <openssl/ssl.h>

typedef struct quic_tls_st QUIC_TLS;

typedef struct quic_tls_args_st {
    /*
     * The "inner" SSL object for the QUIC Connection. Contains an
     * SSL_CONNECTION
     */
    SSL *s;

    /*
     * Called to send data on the crypto stream. We use a callback rather than
     * passing the crypto stream QUIC_SSTREAM directly because this lets the CSM
     * dynamically select the correct outgoing crypto stream based on the
     * current EL.
     */
    int (*crypto_send_cb)(const unsigned char *buf, size_t buf_len,
                          size_t *consumed, void *arg);
    void *crypto_send_cb_arg;

    /*
     * Call to receive crypto stream data. A pointer to the underlying buffer
     * is provided, and subsequently released to avoid unnecessary copying of
     * data.
     */
    int (*crypto_recv_rcd_cb)(const unsigned char **buf, size_t *bytes_read,
                              void *arg);
    void *crypto_recv_rcd_cb_arg;
    int (*crypto_release_rcd_cb)(size_t bytes_read, void *arg);
    void *crypto_release_rcd_cb_arg;

    /*
     * Called when a traffic secret is available for a given TLS protection
     * level.
     */
    int (*yield_secret_cb)(uint32_t prot_level, int direction /* 0=RX, 1=TX */,
                           uint32_t suite_id, EVP_MD *md,
                           const unsigned char *secret, size_t secret_len,
                           void *arg);
    void *yield_secret_cb_arg;

    /*
     * Called when we receive transport parameters from the peer.
     *
     * Note: These parameters are not authenticated until the handshake is
     * marked as completed.
     */
    int (*got_transport_params_cb)(const unsigned char *params,
                                   size_t params_len,
                                   void *arg);
    void *got_transport_params_cb_arg;

    /*
     * Called when the handshake has been completed as far as the handshake
     * protocol is concerned, meaning that the connection has been
     * authenticated.
     */
    int (*handshake_complete_cb)(void *arg);
    void *handshake_complete_cb_arg;

    /*
     * Called when something has gone wrong with the connection as far as the
     * handshake layer is concerned, meaning that it should be immediately torn
     * down. Note that this may happen at any time, including after a connection
     * has been fully established.
     */
    int (*alert_cb)(void *arg, unsigned char alert_code);
    void *alert_cb_arg;

    /* Set to 1 if we are running in the server role. */
    int is_server;

    /* Set to 1 if this is an internal use of the QUIC TLS */
    int ossl_quic;
} QUIC_TLS_ARGS;

QUIC_TLS *ossl_quic_tls_new(const QUIC_TLS_ARGS *args);

void ossl_quic_tls_free(QUIC_TLS *qtls);

int ossl_quic_tls_configure(QUIC_TLS *qtls);

/* Advance the state machine */
int ossl_quic_tls_tick(QUIC_TLS *qtls);

void ossl_quic_tls_clear(QUIC_TLS *qtls);

int ossl_quic_tls_set_transport_params(QUIC_TLS *qtls,
                                       const unsigned char *transport_params,
                                       size_t transport_params_len);

int ossl_quic_tls_get_error(QUIC_TLS *qtls,
                            uint64_t *error_code,
                            const char **error_msg,
                            ERR_STATE **error_state);

int ossl_quic_tls_is_cert_request(QUIC_TLS *qtls);
int ossl_quic_tls_has_bad_max_early_data(QUIC_TLS *qtls);

int ossl_quic_tls_set_early_data_enabled(QUIC_TLS *qtls, int enabled);
#endif
