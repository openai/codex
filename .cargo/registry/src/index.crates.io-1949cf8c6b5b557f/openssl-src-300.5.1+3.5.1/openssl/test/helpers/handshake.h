/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_TEST_HANDSHAKE_HELPER_H
#define OSSL_TEST_HANDSHAKE_HELPER_H

#include "ssl_test_ctx.h"

typedef struct ctx_data_st {
    unsigned char *npn_protocols;
    size_t npn_protocols_len;
    unsigned char *alpn_protocols;
    size_t alpn_protocols_len;
    char *srp_user;
    char *srp_password;
    char *session_ticket_app_data;
} CTX_DATA;

typedef struct handshake_result {
    ssl_test_result_t result;
    /* These alerts are in the 2-byte format returned by the info_callback. */
    /* (Latest) alert sent by the client; 0 if no alert. */
    int client_alert_sent;
    /* Number of fatal or close_notify alerts sent. */
    int client_num_fatal_alerts_sent;
    /* (Latest) alert received by the server; 0 if no alert. */
    int client_alert_received;
    /* (Latest) alert sent by the server; 0 if no alert. */
    int server_alert_sent;
    /* Number of fatal or close_notify alerts sent. */
    int server_num_fatal_alerts_sent;
    /* (Latest) alert received by the client; 0 if no alert. */
    int server_alert_received;
    /* Negotiated protocol. On success, these should always match. */
    int server_protocol;
    int client_protocol;
    /* Server connection */
    ssl_servername_t servername;
    /* Session ticket status */
    ssl_session_ticket_t session_ticket;
    int compression;
    /* Was this called on the second context? */
    int session_ticket_do_not_call;
    char *client_npn_negotiated;
    char *server_npn_negotiated;
    char *client_alpn_negotiated;
    char *server_alpn_negotiated;
    /* Was the handshake resumed? */
    int client_resumed;
    int server_resumed;
    /* Temporary key type */
    int tmp_key_type;
    /* server certificate key type */
    int server_cert_type;
    /* server signing hash */
    int server_sign_hash;
    /* server signature type */
    int server_sign_type;
    /* server CA names */
    STACK_OF(X509_NAME) *server_ca_names;
    /* client certificate key type */
    int client_cert_type;
    /* client signing hash */
    int client_sign_hash;
    /* client signature type */
    int client_sign_type;
    /* Client CA names */
    STACK_OF(X509_NAME) *client_ca_names;
    /* Session id status */
    ssl_session_id_t session_id;
    char *cipher;
    /* session ticket application data */
    char *result_session_ticket_app_data;
} HANDSHAKE_RESULT;

HANDSHAKE_RESULT *HANDSHAKE_RESULT_new(void);
void HANDSHAKE_RESULT_free(HANDSHAKE_RESULT *result);

/* Do a handshake and report some information about the result. */
HANDSHAKE_RESULT *do_handshake(SSL_CTX *server_ctx, SSL_CTX *server2_ctx,
                               SSL_CTX *client_ctx, SSL_CTX *resume_server_ctx,
                               SSL_CTX *resume_client_ctx,
                               const SSL_TEST_CTX *test_ctx);

int configure_handshake_ctx_for_srp(SSL_CTX *server_ctx, SSL_CTX *server2_ctx,
                                    SSL_CTX *client_ctx,
                                    const SSL_TEST_EXTRA_CONF *extra,
                                    CTX_DATA *server_ctx_data,
                                    CTX_DATA *server2_ctx_data,
                                    CTX_DATA *client_ctx_data);

#endif  /* OSSL_TEST_HANDSHAKE_HELPER_H */
