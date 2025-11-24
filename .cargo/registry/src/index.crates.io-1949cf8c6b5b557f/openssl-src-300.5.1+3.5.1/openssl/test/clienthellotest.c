/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
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

#define CLIENT_VERSION_LEN      2

#define TOTAL_NUM_TESTS                         3

/*
 * Test that explicitly setting ticket data results in it appearing in the
 * ClientHello for a negotiated SSL/TLS version
 */
#define TEST_SET_SESSION_TICK_DATA_VER_NEG      0
/* Enable padding and make sure ClientHello is long enough to require it */
#define TEST_ADD_PADDING                        1
/* Enable padding and make sure ClientHello is short enough to not need it */
#define TEST_PADDING_NOT_NEEDED                 2

#define F5_WORKAROUND_MIN_MSG_LEN   0x7f
#define F5_WORKAROUND_MAX_MSG_LEN   0x200

/* Dummy ALPN protocols used to pad out the size of the ClientHello */
/* ASCII 'O' = 79 = 0x4F = EBCDIC '|'*/
#ifdef CHARSET_EBCDIC
static const char alpn_prots[] =
    "|1234567890123456789012345678901234567890123456789012345678901234567890123456789"
    "|1234567890123456789012345678901234567890123456789012345678901234567890123456789";
#else
static const char alpn_prots[] =
    "O1234567890123456789012345678901234567890123456789012345678901234567890123456789"
    "O1234567890123456789012345678901234567890123456789012345678901234567890123456789";
#endif

static int test_client_hello(int currtest)
{
    SSL_CTX *ctx;
    SSL *con = NULL;
    BIO *rbio;
    BIO *wbio;
    long len;
    unsigned char *data;
    PACKET pkt, pkt2, pkt3;
    char *dummytick = "Hello World!";
    unsigned int type = 0;
    int testresult = 0;
    size_t msglen;
    BIO *sessbio = NULL;
    SSL_SESSION *sess = NULL;

    memset(&pkt, 0, sizeof(pkt));
    memset(&pkt2, 0, sizeof(pkt2));
    memset(&pkt3, 0, sizeof(pkt3));

    /*
     * For each test set up an SSL_CTX and SSL and see what ClientHello gets
     * produced when we try to connect
     */
    ctx = SSL_CTX_new(TLS_method());
    if (!TEST_ptr(ctx))
        goto end;
    if (!TEST_true(SSL_CTX_set_max_proto_version(ctx, 0)))
        goto end;

    switch (currtest) {
    case TEST_SET_SESSION_TICK_DATA_VER_NEG:
#if !defined(OPENSSL_NO_TLS1_3) && defined(OPENSSL_NO_TLS1_2)
        /* TLSv1.3 is enabled and TLSv1.2 is disabled so can't do this test */
        SSL_CTX_free(ctx);
        return 1;
#else
        /* Testing for session tickets <= TLS1.2; not relevant for 1.3 */
        if (!TEST_true(SSL_CTX_set_max_proto_version(ctx, TLS1_2_VERSION)))
            goto end;
#endif
        break;

    case TEST_ADD_PADDING:
    case TEST_PADDING_NOT_NEEDED:
        SSL_CTX_set_options(ctx, SSL_OP_TLSEXT_PADDING);
        /* Make sure we get a consistent size across TLS versions */
        SSL_CTX_clear_options(ctx, SSL_OP_ENABLE_MIDDLEBOX_COMPAT);
        /* Avoid large keyshares */
        if (!TEST_true(SSL_CTX_set1_groups_list(ctx,
                       "?X25519:?secp256r1:?ffdhe2048:?ffdhe3072")))
            goto end;
        /*
         * Add some dummy ALPN protocols so that the ClientHello is at least
         * F5_WORKAROUND_MIN_MSG_LEN bytes long - meaning padding will be
         * needed.
         */
        if (currtest == TEST_ADD_PADDING) {
             if (!TEST_false(SSL_CTX_set_alpn_protos(ctx,
                                    (unsigned char *)alpn_prots,
                                    sizeof(alpn_prots) - 1)))
                goto end;
        /*
         * Otherwise we need to make sure we have a small enough message to
         * not need padding.
         */
        } else if (!TEST_true(SSL_CTX_set_cipher_list(ctx,
                              "AES128-SHA"))
                   || !TEST_true(SSL_CTX_set_ciphersuites(ctx,
                                 "TLS_AES_128_GCM_SHA256"))) {
            goto end;
        }
        break;

    default:
        goto end;
    }

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

    if (currtest == TEST_SET_SESSION_TICK_DATA_VER_NEG) {
        if (!TEST_true(SSL_set_session_ticket_ext(con, dummytick,
                                                  strlen(dummytick))))
            goto end;
    }

    if (!TEST_int_le(SSL_connect(con), 0)) {
        /* This shouldn't succeed because we don't have a server! */
        goto end;
    }

    if (!TEST_long_ge(len = BIO_get_mem_data(wbio, (char **)&data), 0)
            || !TEST_true(PACKET_buf_init(&pkt, data, len))
               /* Skip the record header */
            || !PACKET_forward(&pkt, SSL3_RT_HEADER_LENGTH))
        goto end;

    msglen = PACKET_remaining(&pkt);

    /* Skip the handshake message header */
    if (!TEST_true(PACKET_forward(&pkt, SSL3_HM_HEADER_LENGTH))
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

    /* Loop through all extensions */
    while (PACKET_remaining(&pkt2)) {

        if (!TEST_true(PACKET_get_net_2(&pkt2, &type))
                || !TEST_true(PACKET_get_length_prefixed_2(&pkt2, &pkt3)))
            goto end;

        if (type == TLSEXT_TYPE_session_ticket) {
            if (currtest == TEST_SET_SESSION_TICK_DATA_VER_NEG) {
                if (TEST_true(PACKET_equal(&pkt3, dummytick,
                                           strlen(dummytick)))) {
                    /* Ticket data is as we expected */
                    testresult = 1;
                }
                goto end;
            }
        }
        if (type == TLSEXT_TYPE_padding) {
            if (!TEST_false(currtest == TEST_PADDING_NOT_NEEDED))
                goto end;
            else if (TEST_true(currtest == TEST_ADD_PADDING))
                testresult = TEST_true(msglen == F5_WORKAROUND_MAX_MSG_LEN);
        }
    }

    if (currtest == TEST_PADDING_NOT_NEEDED)
        testresult = 1;

end:
    SSL_free(con);
    SSL_CTX_free(ctx);
    SSL_SESSION_free(sess);
    BIO_free(sessbio);

    return testresult;
}

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    ADD_ALL_TESTS(test_client_hello, TOTAL_NUM_TESTS);
    return 1;
}
