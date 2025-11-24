/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_TEST_SSL_TEST_CTX_H
#define OSSL_TEST_SSL_TEST_CTX_H

#include <openssl/conf.h>
#include <openssl/ssl.h>

typedef enum {
    SSL_TEST_SUCCESS = 0,  /* Default */
    SSL_TEST_SERVER_FAIL,
    SSL_TEST_CLIENT_FAIL,
    SSL_TEST_INTERNAL_ERROR,
    /* Couldn't test resumption/renegotiation: original handshake failed. */
    SSL_TEST_FIRST_HANDSHAKE_FAILED
} ssl_test_result_t;

typedef enum {
    SSL_TEST_VERIFY_NONE = 0, /* Default */
    SSL_TEST_VERIFY_ACCEPT_ALL,
    SSL_TEST_VERIFY_RETRY_ONCE,
    SSL_TEST_VERIFY_REJECT_ALL
} ssl_verify_callback_t;

typedef enum {
    SSL_TEST_SERVERNAME_NONE = 0, /* Default */
    SSL_TEST_SERVERNAME_SERVER1,
    SSL_TEST_SERVERNAME_SERVER2,
    SSL_TEST_SERVERNAME_INVALID
} ssl_servername_t;

typedef enum {
    SSL_TEST_SERVERNAME_CB_NONE = 0,  /* Default */
    SSL_TEST_SERVERNAME_IGNORE_MISMATCH,
    SSL_TEST_SERVERNAME_REJECT_MISMATCH,
    SSL_TEST_SERVERNAME_CLIENT_HELLO_IGNORE_MISMATCH,
    SSL_TEST_SERVERNAME_CLIENT_HELLO_REJECT_MISMATCH,
    SSL_TEST_SERVERNAME_CLIENT_HELLO_NO_V12
} ssl_servername_callback_t;

typedef enum {
    SSL_TEST_SESSION_TICKET_IGNORE = 0, /* Default */
    SSL_TEST_SESSION_TICKET_YES,
    SSL_TEST_SESSION_TICKET_NO,
    SSL_TEST_SESSION_TICKET_BROKEN /* Special test */
} ssl_session_ticket_t;

typedef enum {
    SSL_TEST_COMPRESSION_NO = 0, /* Default */
    SSL_TEST_COMPRESSION_YES
} ssl_compression_t;

typedef enum {
    SSL_TEST_SESSION_ID_IGNORE = 0, /* Default */
    SSL_TEST_SESSION_ID_YES,
    SSL_TEST_SESSION_ID_NO
} ssl_session_id_t;

typedef enum {
    SSL_TEST_METHOD_TLS = 0, /* Default */
    SSL_TEST_METHOD_DTLS,
    SSL_TEST_METHOD_QUIC
} ssl_test_method_t;

typedef enum {
    SSL_TEST_HANDSHAKE_SIMPLE = 0, /* Default */
    SSL_TEST_HANDSHAKE_RESUME,
    SSL_TEST_HANDSHAKE_RENEG_SERVER,
    SSL_TEST_HANDSHAKE_RENEG_CLIENT,
    SSL_TEST_HANDSHAKE_KEY_UPDATE_SERVER,
    SSL_TEST_HANDSHAKE_KEY_UPDATE_CLIENT,
    SSL_TEST_HANDSHAKE_POST_HANDSHAKE_AUTH
} ssl_handshake_mode_t;

typedef enum {
    SSL_TEST_CT_VALIDATION_NONE = 0, /* Default */
    SSL_TEST_CT_VALIDATION_PERMISSIVE,
    SSL_TEST_CT_VALIDATION_STRICT
} ssl_ct_validation_t;

typedef enum {
    SSL_TEST_CERT_STATUS_NONE = 0, /* Default */
    SSL_TEST_CERT_STATUS_GOOD_RESPONSE,
    SSL_TEST_CERT_STATUS_BAD_RESPONSE
} ssl_cert_status_t;

/*
 * Server/client settings that aren't supported by the SSL CONF library,
 * such as callbacks.
 */
typedef struct {
    /* One of a number of predefined custom callbacks. */
    ssl_verify_callback_t verify_callback;
    /* One of a number of predefined server names use by the client */
    ssl_servername_t servername;
    /* Maximum Fragment Length extension mode */
    int max_fragment_len_mode;
    /* Supported NPN and ALPN protocols. A comma-separated list. */
    char *npn_protocols;
    char *alpn_protocols;
    ssl_ct_validation_t ct_validation;
    /* Ciphersuites to set on a renegotiation */
    char *reneg_ciphers;
    char *srp_user;
    char *srp_password;
    /* PHA enabled */
    int enable_pha;
    /* Do not send extms on renegotiation */
    int no_extms_on_reneg;
} SSL_TEST_CLIENT_CONF;

typedef struct {
    /* SNI callback (server-side). */
    ssl_servername_callback_t servername_callback;
    /* Supported NPN and ALPN protocols. A comma-separated list. */
    char *npn_protocols;
    char *alpn_protocols;
    /* Whether to set a broken session ticket callback. */
    int broken_session_ticket;
    /* Should we send a CertStatus message? */
    ssl_cert_status_t cert_status;
    /* An SRP user known to the server. */
    char *srp_user;
    char *srp_password;
    /* Forced PHA */
    int force_pha;
    char *session_ticket_app_data;
} SSL_TEST_SERVER_CONF;

typedef struct {
    SSL_TEST_CLIENT_CONF client;
    SSL_TEST_SERVER_CONF server;
    SSL_TEST_SERVER_CONF server2;
} SSL_TEST_EXTRA_CONF;

typedef struct {
    /*
     * Global test configuration. Does not change between handshakes.
     */
    /* Whether the server/client CTX should use DTLS or TLS. */
    ssl_test_method_t method;
    /* Whether to test a resumed/renegotiated handshake. */
    ssl_handshake_mode_t handshake_mode;
    /*
     * How much application data to exchange (default is 256 bytes).
     * Both peers will send |app_data_size| bytes interleaved.
     */
    int app_data_size;
    /* Maximum send fragment size. */
    int max_fragment_size;
    /* KeyUpdate type */
    int key_update_type;

    /*
     * Extra server/client configurations. Per-handshake.
     */
    /* First handshake. */
    SSL_TEST_EXTRA_CONF extra;
    /* Resumed handshake. */
    SSL_TEST_EXTRA_CONF resume_extra;

    /*
     * Test expectations. These apply to the LAST handshake.
     */
    /* Defaults to SUCCESS. */
    ssl_test_result_t expected_result;
    /* Alerts. 0 if no expectation. */
    /* See ssl.h for alert codes. */
    /* Alert sent by the client / received by the server. */
    int expected_client_alert;
    /* Alert sent by the server / received by the client. */
    int expected_server_alert;
    /* Negotiated protocol version. 0 if no expectation. */
    /* See ssl.h for protocol versions. */
    int expected_protocol;
    /*
     * The expected SNI context to use.
     * We test server-side that the server switched to the expected context.
     * Set by the callback upon success, so if the callback wasn't called or
     * terminated with an alert, the servername will match with
     * SSL_TEST_SERVERNAME_NONE.
     * Note: in the event that the servername was accepted, the client should
     * also receive an empty SNI extension back but we have no way of probing
     * client-side via the API that this was the case.
     */
    ssl_servername_t expected_servername;
    ssl_session_ticket_t session_ticket_expected;
    int compression_expected;
    /* The expected NPN/ALPN protocol to negotiate. */
    char *expected_npn_protocol;
    char *expected_alpn_protocol;
    /* Whether the second handshake is resumed or a full handshake (boolean). */
    int resumption_expected;
    /* Expected temporary key type */
    int expected_tmp_key_type;
    /* Expected server certificate key type */
    int expected_server_cert_type;
    /* Expected server signing hash */
    int expected_server_sign_hash;
    /* Expected server signature type */
    int expected_server_sign_type;
    /* Expected server CA names */
    STACK_OF(X509_NAME) *expected_server_ca_names;
    /* Expected client certificate key type */
    int expected_client_cert_type;
    /* Expected client signing hash */
    int expected_client_sign_hash;
    /* Expected client signature type */
    int expected_client_sign_type;
    /* Expected CA names for client auth */
    STACK_OF(X509_NAME) *expected_client_ca_names;
    /* Whether to use SCTP for the transport */
    int use_sctp;
    /* Whether to pre-compress server certificates */
    int compress_certificates;
    /* Enable SSL_MODE_DTLS_SCTP_LABEL_LENGTH_BUG on client side */
    int enable_client_sctp_label_bug;
    /* Enable SSL_MODE_DTLS_SCTP_LABEL_LENGTH_BUG on server side */
    int enable_server_sctp_label_bug;
    /* Whether to expect a session id from the server */
    ssl_session_id_t session_id_expected;
    char *expected_cipher;
    /* Expected Session Ticket Application Data */
    char *expected_session_ticket_app_data;

    OSSL_LIB_CTX *libctx;

    /* FIPS version string to check for compatibility */
    char *fips_version;
} SSL_TEST_CTX;

const char *ssl_test_result_name(ssl_test_result_t result);
const char *ssl_alert_name(int alert);
const char *ssl_protocol_name(int protocol);
const char *ssl_verify_callback_name(ssl_verify_callback_t verify_callback);
const char *ssl_servername_name(ssl_servername_t server);
const char *ssl_servername_callback_name(ssl_servername_callback_t
                                         servername_callback);
const char *ssl_session_ticket_name(ssl_session_ticket_t server);
const char *ssl_session_id_name(ssl_session_id_t server);
const char *ssl_test_method_name(ssl_test_method_t method);
const char *ssl_handshake_mode_name(ssl_handshake_mode_t mode);
const char *ssl_ct_validation_name(ssl_ct_validation_t mode);
const char *ssl_certstatus_name(ssl_cert_status_t cert_status);
const char *ssl_max_fragment_len_name(int MFL_mode);

/*
 * Load the test case context from |conf|.
 * See test/README.ssltest.md for details on the conf file format.
 */
SSL_TEST_CTX *SSL_TEST_CTX_create(const CONF *conf, const char *test_section,
                                  OSSL_LIB_CTX *libctx);

SSL_TEST_CTX *SSL_TEST_CTX_new(OSSL_LIB_CTX *libctx);

void SSL_TEST_CTX_free(SSL_TEST_CTX *ctx);

#endif  /* OSSL_TEST_SSL_TEST_CTX_H */
