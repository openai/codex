/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/dtls1.h>
#include <openssl/ssl.h>
#include <openssl/err.h>

#include "helpers/ssltestlib.h"
#include "testutil.h"

/* for SSL_READ_ETM() */
#include "../ssl/ssl_local.h"
#include "internal/ssl_unwrap.h"

static int debug = 0;

static unsigned int clnt_psk_callback(SSL *ssl, const char *hint,
                                      char *ident, unsigned int max_ident_len,
                                      unsigned char *psk,
                                      unsigned int max_psk_len)
{
    BIO_snprintf(ident, max_ident_len, "psk");

    if (max_psk_len > 20)
        max_psk_len = 20;
    memset(psk, 0x5a, max_psk_len);

    return max_psk_len;
}

static unsigned int srvr_psk_callback(SSL *ssl, const char *identity,
                                      unsigned char *psk,
                                      unsigned int max_psk_len)
{
    if (max_psk_len > 20)
        max_psk_len = 20;
    memset(psk, 0x5a, max_psk_len);
    return max_psk_len;
}

static int mtu_test(SSL_CTX *ctx, const char *cs, int no_etm)
{
    SSL *srvr_ssl = NULL, *clnt_ssl = NULL;
    BIO *sc_bio = NULL;
    int i;
    size_t s;
    size_t mtus[30];
    unsigned char buf[600];
    int rv = 0;
    SSL_CONNECTION *clnt_sc;

    memset(buf, 0x5a, sizeof(buf));

    if (!TEST_true(create_ssl_objects(ctx, ctx, &srvr_ssl, &clnt_ssl,
                                      NULL, NULL)))
        goto end;

    if (no_etm)
        SSL_set_options(srvr_ssl, SSL_OP_NO_ENCRYPT_THEN_MAC);

    if (!TEST_true(SSL_set_cipher_list(srvr_ssl, cs))
            || !TEST_true(SSL_set_cipher_list(clnt_ssl, cs))
            || !TEST_ptr(sc_bio = SSL_get_rbio(srvr_ssl))
            || !TEST_true(create_ssl_connection(clnt_ssl, srvr_ssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (debug)
        TEST_info("Channel established");

    /* For record MTU values between 500 and 539, call DTLS_get_data_mtu()
     * to query the payload MTU which will fit. */
    for (i = 0; i < 30; i++) {
        SSL_set_mtu(clnt_ssl, 500 + i);
        mtus[i] = DTLS_get_data_mtu(clnt_ssl);
        if (debug)
            TEST_info("%s%s MTU for record mtu %d = %lu",
                      cs, no_etm ? "-noEtM" : "",
                      500 + i, (unsigned long)mtus[i]);
        if (!TEST_size_t_ne(mtus[i], 0)) {
            TEST_info("Cipher %s MTU %d", cs, 500 + i);
            goto end;
        }
    }

    /* Now get out of the way */
    SSL_set_mtu(clnt_ssl, 1000);

    /*
     * Now for all values in the range of payload MTUs, send a payload of
     * that size and see what actual record size we end up with.
     */
    for (s = mtus[0]; s <= mtus[29]; s++) {
        size_t reclen;

        if (!TEST_int_eq(SSL_write(clnt_ssl, buf, s), (int)s))
            goto end;
        reclen = BIO_read(sc_bio, buf, sizeof(buf));
        if (debug)
            TEST_info("record %zu for payload %zu", reclen, s);

        for (i = 0; i < 30; i++) {
            /* DTLS_get_data_mtu() with record MTU 500+i returned mtus[i] ... */

            if (!TEST_false(s <= mtus[i] && reclen > (size_t)(500 + i))) {
                /*
                 * We sent a packet smaller than or equal to mtus[j] and
                 * that made a record *larger* than the record MTU 500+j!
                 */
                TEST_error("%s: s=%lu, mtus[i]=%lu, reclen=%lu, i=%d",
                           cs, (unsigned long)s, (unsigned long)mtus[i],
                           (unsigned long)reclen, 500 + i);
                goto end;
            }
            if (!TEST_false(s > mtus[i] && reclen <= (size_t)(500 + i))) {
                /*
                 * We sent a *larger* packet than mtus[i] and that *still*
                 * fits within the record MTU 500+i, so DTLS_get_data_mtu()
                 * was overly pessimistic.
                 */
                TEST_error("%s: s=%lu, mtus[i]=%lu, reclen=%lu, i=%d",
                           cs, (unsigned long)s, (unsigned long)mtus[i],
                           (unsigned long)reclen, 500 + i);
                goto end;
            }
        }
    }
    if (!TEST_ptr(clnt_sc = SSL_CONNECTION_FROM_SSL_ONLY(clnt_ssl)))
        goto end;
    rv = 1;
    if (SSL_READ_ETM(clnt_sc))
        rv = 2;
 end:
    SSL_free(clnt_ssl);
    SSL_free(srvr_ssl);
    return rv;
}

static int run_mtu_tests(void)
{
    SSL_CTX *ctx = NULL;
    STACK_OF(SSL_CIPHER) *ciphers;
    int i, ret = 0;

    if (!TEST_ptr(ctx = SSL_CTX_new(DTLS_method())))
        goto end;

    SSL_CTX_set_psk_server_callback(ctx, srvr_psk_callback);
    SSL_CTX_set_psk_client_callback(ctx, clnt_psk_callback);
    SSL_CTX_set_security_level(ctx, 0);

    /*
     * We only care about iterating over each enc/mac; we don't want to
     * repeat the test for each auth/kx variant. So keep life simple and
     * only do (non-DH) PSK.
     */
    if (!TEST_true(SSL_CTX_set_cipher_list(ctx, "PSK")))
        goto end;

    ciphers = SSL_CTX_get_ciphers(ctx);
    for (i = 0; i < sk_SSL_CIPHER_num(ciphers); i++) {
        const SSL_CIPHER *cipher = sk_SSL_CIPHER_value(ciphers, i);
        const char *cipher_name = SSL_CIPHER_get_name(cipher);

        /* As noted above, only one test for each enc/mac variant. */
        if (!HAS_PREFIX(cipher_name, "PSK-"))
            continue;

        if (!TEST_int_gt(ret = mtu_test(ctx, cipher_name, 0), 0))
            break;
        TEST_info("%s OK", cipher_name);
        if (ret == 1)
            continue;

        /* mtu_test() returns 2 if it used Encrypt-then-MAC */
        if (!TEST_int_gt(ret = mtu_test(ctx, cipher_name, 1), 0))
            break;
        TEST_info("%s without EtM OK", cipher_name);
    }

 end:
    SSL_CTX_free(ctx);
    return ret;
}

static int test_server_mtu_larger_than_max_fragment_length(void)
{
    SSL_CTX *ctx = NULL;
    SSL *srvr_ssl = NULL, *clnt_ssl = NULL;
    int rv = 0;

    if (!TEST_ptr(ctx = SSL_CTX_new(DTLS_method())))
        goto end;

    SSL_CTX_set_psk_server_callback(ctx, srvr_psk_callback);
    SSL_CTX_set_psk_client_callback(ctx, clnt_psk_callback);

#ifndef OPENSSL_NO_DH
    if (!TEST_true(SSL_CTX_set_dh_auto(ctx, 1)))
        goto end;
#endif

    if (!TEST_true(create_ssl_objects(ctx, ctx, &srvr_ssl, &clnt_ssl,
                                      NULL, NULL)))
        goto end;

    SSL_set_options(srvr_ssl, SSL_OP_NO_QUERY_MTU);
    if (!TEST_true(DTLS_set_link_mtu(srvr_ssl, 1500)))
        goto end;

    SSL_set_tlsext_max_fragment_length(clnt_ssl,
                                       TLSEXT_max_fragment_length_512);

    if (!TEST_true(create_ssl_connection(srvr_ssl, clnt_ssl,
                                         SSL_ERROR_NONE)))
        goto end;

    rv = 1;

 end:
    SSL_free(clnt_ssl);
    SSL_free(srvr_ssl);
    SSL_CTX_free(ctx);
    return rv;
}

int setup_tests(void)
{
    ADD_TEST(run_mtu_tests);
    ADD_TEST(test_server_mtu_larger_than_max_fragment_length);
    return 1;
}

void cleanup_tests(void)
{
    bio_s_mempacket_test_free();
}
