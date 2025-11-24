/*
 * Copyright 2017-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 BaishanCloud. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <time.h>

#include "internal/packet.h"

#include "testutil.h"
#include "internal/nelem.h"
#include "helpers/ssltestlib.h"

#define CLIENT_VERSION_LEN      2

static const char *host = "dummy-host";

static char *cert = NULL;
static char *privkey = NULL;

#if defined(OPENSSL_NO_TLS1_3) || \
    (defined(OPENSSL_NO_EC) && defined(OPENSSL_NO_DH))
static int maxversion = TLS1_2_VERSION;
#else
static int maxversion = 0;
#endif

static int get_sni_from_client_hello(BIO *bio, char **sni)
{
    long len;
    unsigned char *data;
    PACKET pkt, pkt2, pkt3, pkt4, pkt5;
    unsigned int servname_type = 0, type = 0;
    int ret = 0;

    memset(&pkt, 0, sizeof(pkt));
    memset(&pkt2, 0, sizeof(pkt2));
    memset(&pkt3, 0, sizeof(pkt3));
    memset(&pkt4, 0, sizeof(pkt4));
    memset(&pkt5, 0, sizeof(pkt5));

    if (!TEST_long_ge(len = BIO_get_mem_data(bio, (char **)&data), 0)
            || !TEST_true(PACKET_buf_init(&pkt, data, len))
               /* Skip the record header */
            || !PACKET_forward(&pkt, SSL3_RT_HEADER_LENGTH)
               /* Skip the handshake message header */
            || !TEST_true(PACKET_forward(&pkt, SSL3_HM_HEADER_LENGTH))
               /* Skip client version and random */
            || !TEST_true(PACKET_forward(&pkt, CLIENT_VERSION_LEN
                                               + SSL3_RANDOM_SIZE))
               /* Skip session id */
            || !TEST_true(PACKET_get_length_prefixed_1(&pkt, &pkt2))
               /* Skip ciphers */
            || !TEST_true(PACKET_get_length_prefixed_2(&pkt, &pkt2))
               /* Skip compression */
            || !TEST_true(PACKET_get_length_prefixed_1(&pkt, &pkt2))
               /* Extensions len */
            || !TEST_true(PACKET_as_length_prefixed_2(&pkt, &pkt2)))
        goto end;

    /* Loop through all extensions for SNI */
    while (PACKET_remaining(&pkt2)) {
        if (!TEST_true(PACKET_get_net_2(&pkt2, &type))
                || !TEST_true(PACKET_get_length_prefixed_2(&pkt2, &pkt3)))
            goto end;
        if (type == TLSEXT_TYPE_server_name) {
            if (!TEST_true(PACKET_get_length_prefixed_2(&pkt3, &pkt4))
                    || !TEST_uint_ne(PACKET_remaining(&pkt4), 0)
                    || !TEST_true(PACKET_get_1(&pkt4, &servname_type))
                    || !TEST_uint_eq(servname_type, TLSEXT_NAMETYPE_host_name)
                    || !TEST_true(PACKET_get_length_prefixed_2(&pkt4, &pkt5))
                    || !TEST_uint_le(PACKET_remaining(&pkt5), TLSEXT_MAXLEN_host_name)
                    || !TEST_false(PACKET_contains_zero_byte(&pkt5))
                    || !TEST_true(PACKET_strndup(&pkt5, sni)))
                goto end;
            ret = 1;
            goto end;
        }
    }
end:
    return ret;
}

static int client_setup_sni_before_state(void)
{
    SSL_CTX *ctx;
    SSL *con = NULL;
    BIO *rbio;
    BIO *wbio;
    char *hostname = NULL;
    int ret = 0;

    /* use TLS_method to blur 'side' */
    ctx = SSL_CTX_new(TLS_method());
    if (!TEST_ptr(ctx))
        goto end;

    if (maxversion > 0
            && !TEST_true(SSL_CTX_set_max_proto_version(ctx, maxversion)))
        goto end;

    con = SSL_new(ctx);
    if (!TEST_ptr(con))
        goto end;

    /* set SNI before 'client side' is set */
    SSL_set_tlsext_host_name(con, host);

    rbio = BIO_new(BIO_s_mem());
    wbio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(rbio)|| !TEST_ptr(wbio)) {
        BIO_free(rbio);
        BIO_free(wbio);
        goto end;
    }

    SSL_set_bio(con, rbio, wbio);

    if (!TEST_int_le(SSL_connect(con), 0))
        /* This shouldn't succeed because we don't have a server! */
        goto end;
    if (!TEST_true(get_sni_from_client_hello(wbio, &hostname)))
        /* no SNI in client hello */
        goto end;
    if (!TEST_str_eq(hostname, host))
        /* incorrect SNI value */
        goto end;
    ret = 1;
end:
    OPENSSL_free(hostname);
    SSL_free(con);
    SSL_CTX_free(ctx);
    return ret;
}

static int client_setup_sni_after_state(void)
{
    SSL_CTX *ctx;
    SSL *con = NULL;
    BIO *rbio;
    BIO *wbio;
    char *hostname = NULL;
    int ret = 0;

    /* use TLS_method to blur 'side' */
    ctx = SSL_CTX_new(TLS_method());
    if (!TEST_ptr(ctx))
        goto end;

    if (maxversion > 0
            && !TEST_true(SSL_CTX_set_max_proto_version(ctx, maxversion)))
        goto end;

    con = SSL_new(ctx);
    if (!TEST_ptr(con))
        goto end;

    rbio = BIO_new(BIO_s_mem());
    wbio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(rbio)|| !TEST_ptr(wbio)) {
        BIO_free(rbio);
        BIO_free(wbio);
        goto end;
    }

    SSL_set_bio(con, rbio, wbio);
    SSL_set_connect_state(con);

    /* set SNI after 'client side' is set */
    SSL_set_tlsext_host_name(con, host);

    if (!TEST_int_le(SSL_connect(con), 0))
        /* This shouldn't succeed because we don't have a server! */
        goto end;
    if (!TEST_true(get_sni_from_client_hello(wbio, &hostname)))
        /* no SNI in client hello */
        goto end;
    if (!TEST_str_eq(hostname, host))
        /* incorrect SNI value */
        goto end;
    ret = 1;
end:
    OPENSSL_free(hostname);
    SSL_free(con);
    SSL_CTX_free(ctx);
    return ret;
}

static int server_setup_sni(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    /* set SNI at server side */
    SSL_set_tlsext_host_name(serverssl, host);

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_ptr_null(SSL_get_servername(serverssl,
                                          TLSEXT_NAMETYPE_host_name))) {
        /* SNI should have been cleared during handshake */
        goto end;
    }

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

typedef int (*sni_test_fn)(void);

static sni_test_fn sni_test_fns[3] = {
    client_setup_sni_before_state,
    client_setup_sni_after_state,
    server_setup_sni
};

static int test_servername(int test)
{
    /*
     * For each test set up an SSL_CTX and SSL and see
     * what SNI behaves.
     */
    return sni_test_fns[test]();
}

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(cert = test_get_argument(0))
            || !TEST_ptr(privkey = test_get_argument(1)))
        return 0;

    ADD_ALL_TESTS(test_servername, OSSL_NELEM(sni_test_fns));
    return 1;
}
