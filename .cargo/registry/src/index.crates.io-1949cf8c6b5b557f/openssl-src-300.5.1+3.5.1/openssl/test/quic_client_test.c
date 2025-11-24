/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stdio.h>
#include <openssl/ssl.h>
#include <openssl/quic.h>
#include <openssl/bio.h>
#include "internal/common.h"
#include "internal/sockets.h"
#include "internal/time.h"
#include "testutil.h"

static const char msg1[] = "GET LICENSE.txt\r\n";
static char msg2[16000];

#define DST_PORT        4433
#define DST_ADDR        0x7f000001UL

static int is_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);

    return ec == SSL_ERROR_WANT_READ || ec == SSL_ERROR_WANT_WRITE;
}

static int test_quic_client_ex(int fd_arg)
{
    int testresult = 0, ret;
    int c_fd;
    BIO *c_net_bio = NULL, *c_net_bio_own = NULL;
    BIO_ADDR *s_addr_ = NULL;
    struct in_addr ina = {0};
    SSL_CTX *c_ctx = NULL;
    SSL *c_ssl = NULL;
    short port = DST_PORT;
    int c_connected = 0, c_write_done = 0, c_shutdown = 0;
    size_t l = 0, c_total_read = 0;
    OSSL_TIME start_time;
    unsigned char alpn[] = { 8, 'h', 't', 't', 'p', '/', '0', '.', '9' };


    if (fd_arg == INVALID_SOCKET) {
        /* Setup test client. */
        c_fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
        if (!TEST_int_ne(c_fd, INVALID_SOCKET))
            goto err;

        if (!TEST_true(BIO_socket_nbio(c_fd, 1)))
            goto err;

        if (!TEST_ptr(s_addr_ = BIO_ADDR_new()))
            goto err;

        ina.s_addr = htonl(DST_ADDR);
        if (!TEST_true(BIO_ADDR_rawmake(s_addr_, AF_INET, &ina, sizeof(ina),
                                        htons(port))))
            goto err;
    } else {
        c_fd = fd_arg;
    }

    if (!TEST_ptr(c_net_bio = c_net_bio_own = BIO_new_dgram(c_fd, 0)))
        goto err;

    /* connected socket does not need to set peer */
    if (s_addr_ != NULL && !BIO_dgram_set_peer(c_net_bio, s_addr_))
        goto err;

    if (!TEST_ptr(c_ctx = SSL_CTX_new(OSSL_QUIC_client_method())))
        goto err;

    if (!TEST_ptr(c_ssl = SSL_new(c_ctx)))
        goto err;

    /* 0 is a success for SSL_set_alpn_protos() */
    if (!TEST_false(SSL_set_alpn_protos(c_ssl, alpn, sizeof(alpn))))
        goto err;

    /* Takes ownership of our reference to the BIO. */
    SSL_set0_rbio(c_ssl, c_net_bio);

    /* Get another reference to be transferred in the SSL_set0_wbio call. */
    if (!TEST_true(BIO_up_ref(c_net_bio))) {
        c_net_bio_own = NULL; /* SSL_free will free the first reference. */
        goto err;
    }

    SSL_set0_wbio(c_ssl, c_net_bio);
    c_net_bio_own = NULL;

    if (!TEST_true(SSL_set_blocking_mode(c_ssl, 0)))
        goto err;

    start_time = ossl_time_now();

    for (;;) {
        if (ossl_time_compare(ossl_time_subtract(ossl_time_now(), start_time),
                              ossl_ms2time(10000)) >= 0) {
            TEST_error("timeout while attempting QUIC client test");
            goto err;
        }

        if (!c_connected) {
            ret = SSL_connect(c_ssl);
            if (!TEST_true(ret == 1 || is_want(c_ssl, ret)))
                goto err;

            if (ret == 1) {
                c_connected = 1;
                TEST_info("Connected!");
            }
        }

        if (c_connected && !c_write_done) {
            if (!TEST_int_eq(SSL_write(c_ssl, msg1, sizeof(msg1) - 1),
                             (int)sizeof(msg1) - 1))
                goto err;

            if (!TEST_true(SSL_stream_conclude(c_ssl, 0)))
                goto err;

            c_write_done = 1;
        }

        if (c_write_done && !c_shutdown && c_total_read < sizeof(msg2) - 1) {
            ret = SSL_read_ex(c_ssl, msg2 + c_total_read,
                              sizeof(msg2) - 1 - c_total_read, &l);
            if (ret != 1) {
                if (SSL_get_error(c_ssl, ret) == SSL_ERROR_ZERO_RETURN) {
                    c_shutdown = 1;
                    TEST_info("Message:\n%s\n", msg2);
                } else if (!TEST_true(is_want(c_ssl, ret))) {
                    goto err;
                }
            } else {
                c_total_read += l;

                if (!TEST_size_t_lt(c_total_read, sizeof(msg2) - 1))
                    goto err;
            }
        }

        if (c_shutdown) {
            ret = SSL_shutdown(c_ssl);
            if (ret == 1)
                break;
        }

        /*
         * This is inefficient because we spin until things work without
         * blocking but this is just a test.
         */
        OSSL_sleep(0);
        SSL_handle_events(c_ssl);
    }

    testresult = 1;
err:
    SSL_free(c_ssl);
    SSL_CTX_free(c_ctx);
    BIO_ADDR_free(s_addr_);
    BIO_free(c_net_bio_own);
    if (fd_arg == INVALID_SOCKET && c_fd != INVALID_SOCKET)
        BIO_closesocket(c_fd);
    return testresult;
}

static int test_quic_client(void)
{
    return (test_quic_client_ex(INVALID_SOCKET));
}

static int test_quic_client_connect_first(void)
{
    struct sockaddr_in sin = {0};
    int c_fd;
    int rv;

#ifdef SA_LEN
    sin.sin_len = sizeof(struct sockaddr_in);
#endif
    sin.sin_family = AF_INET;
    sin.sin_port = htons(DST_PORT);
    sin.sin_addr.s_addr = htonl(DST_ADDR);

    c_fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (!TEST_int_ne(c_fd, INVALID_SOCKET))
        goto err;

    if (!TEST_int_eq(connect(c_fd, (const struct sockaddr *)&sin, sizeof(sin)), 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(c_fd, 1)))
        goto err;

    rv = test_quic_client_ex(c_fd);

    close(c_fd);

    return (rv);

err:
    if (c_fd != INVALID_SOCKET)
        close(c_fd);
    return (0);
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    ADD_TEST(test_quic_client);
    ADD_TEST(test_quic_client_connect_first);

    return 1;
}
