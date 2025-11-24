/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/quic.h>
#include <openssl/rand.h>

#include "helpers/ssltestlib.h"
#include "helpers/quictestlib.h"
#include "testutil.h"
#include "testutil/output.h"
#include "../ssl/ssl_local.h"
#include "internal/quic_error.h"

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *defctxnull = NULL;
static char *certsdir = NULL;
static char *cert = NULL;
static char *ccert = NULL;
static char *cauthca = NULL;
static char *privkey = NULL;
static char *cprivkey = NULL;
static char *datadir = NULL;

static int is_fips = 0;

/* The ssltrace test assumes some options are switched on/off */
#if !defined(OPENSSL_NO_SSL_TRACE) \
    && defined(OPENSSL_NO_BROTLI) && defined(OPENSSL_NO_ZSTD) \
    && !defined(OPENSSL_NO_ECX) && !defined(OPENSSL_NO_DH) \
    && !defined(OPENSSL_NO_ML_DSA) && !defined(OPENSSL_NO_ML_KEM)
# define DO_SSL_TRACE_TEST
#endif

/*
 * Test that we read what we've written.
 * Test 0: Non-blocking
 * Test 1: Blocking
 * Test 2: Blocking, introduce socket error, test error handling.
 */
static int test_quic_write_read(int idx)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL_CTX *sctx = NULL;
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int j, k, ret = 0;
    unsigned char buf[20], scratch[64];
    static char *msg = "A test message";
    size_t msglen = strlen(msg);
    size_t numbytes = 0;
    int ssock = 0, csock = 0;
    uint64_t sid = UINT64_MAX;
    SSL_SESSION *sess = NULL;

    if (idx >= 1 && !qtest_supports_blocking())
        return TEST_skip("Blocking tests not supported in this build");

    for (k = 0; k < 2; k++) {
        if (!TEST_ptr(cctx)
                || !TEST_true(qtest_create_quic_objects(libctx, cctx, sctx,
                                                        cert, privkey,
                                                        idx >= 1
                                                            ? QTEST_FLAG_BLOCK
                                                            : 0,
                                                        &qtserv, &clientquic,
                                                        NULL, NULL))
                || !TEST_true(SSL_set_tlsext_host_name(clientquic, "localhost")))
            goto end;

        if (sess != NULL && !TEST_true(SSL_set_session(clientquic, sess)))
            goto end;

        if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
            goto end;

        if (idx >= 1) {
            if (!TEST_true(BIO_get_fd(ossl_quic_tserver_get0_rbio(qtserv),
                                      &ssock)))
                goto end;
            if (!TEST_int_gt(csock = SSL_get_rfd(clientquic), 0))
                goto end;
        }

        sid = 0; /* client-initiated bidirectional stream */

        for (j = 0; j < 2; j++) {
            /* Check that sending and receiving app data is ok */
            if (!TEST_true(SSL_write_ex(clientquic, msg, msglen, &numbytes))
                || !TEST_size_t_eq(numbytes, msglen))
                goto end;
            if (idx >= 1) {
                do {
                    if (!TEST_true(wait_until_sock_readable(ssock)))
                        goto end;

                    ossl_quic_tserver_tick(qtserv);

                    if (!TEST_true(ossl_quic_tserver_read(qtserv, sid, buf,
                                                          sizeof(buf),
                                                          &numbytes)))
                        goto end;
                } while (numbytes == 0);

                if (!TEST_mem_eq(buf, numbytes, msg, msglen))
                    goto end;
            }

            if (idx >= 2 && j > 0)
                /* Introduce permanent socket error */
                BIO_closesocket(csock);

            ossl_quic_tserver_tick(qtserv);
            if (!TEST_true(ossl_quic_tserver_write(qtserv, sid,
                                                   (unsigned char *)msg,
                                                   msglen, &numbytes)))
                goto end;
            ossl_quic_tserver_tick(qtserv);
            SSL_handle_events(clientquic);

            if (idx >= 2 && j > 0) {
                if (!TEST_false(SSL_read_ex(clientquic, buf, 1, &numbytes))
                        || !TEST_int_eq(SSL_get_error(clientquic, 0),
                                        SSL_ERROR_SYSCALL)
                        || !TEST_false(SSL_write_ex(clientquic, msg, msglen,
                                                    &numbytes))
                        || !TEST_int_eq(SSL_get_error(clientquic, 0),
                                        SSL_ERROR_SYSCALL))
                    goto end;
                break;
            }

            /*
            * In blocking mode the SSL_read_ex call will block until the socket
            * is readable and has our data. In non-blocking mode we're doing
            * everything in memory, so it should be immediately available
            */
            if (!TEST_true(SSL_read_ex(clientquic, buf, 1, &numbytes))
                    || !TEST_size_t_eq(numbytes, 1)
                    || !TEST_true(SSL_has_pending(clientquic))
                    || !TEST_int_eq(SSL_pending(clientquic), msglen - 1)
                    || !TEST_true(SSL_read_ex(clientquic, buf + 1,
                                              sizeof(buf) - 1, &numbytes))
                    || !TEST_mem_eq(buf, numbytes + 1, msg, msglen))
                goto end;
        }

        /* Test that exporters work. */
        if (!TEST_true(SSL_export_keying_material(clientquic, scratch,
                        sizeof(scratch), "test", 4, (unsigned char *)"ctx", 3,
                        1)))
            goto end;

        if (sess == NULL) {
            /* We didn't supply a session so we're not expecting resumption */
            if (!TEST_false(SSL_session_reused(clientquic)))
                goto end;
            /* We should have a session ticket by now */
            sess = SSL_get1_session(clientquic);
            if (!TEST_ptr(sess))
                goto end;
        } else {
            /* We supplied a session so we should have resumed */
            if (!TEST_true(SSL_session_reused(clientquic)))
                goto end;
        }

        if (!TEST_true(qtest_shutdown(qtserv, clientquic)))
            goto end;

        if (sctx == NULL) {
            sctx = ossl_quic_tserver_get0_ssl_ctx(qtserv);
            if (!TEST_true(SSL_CTX_up_ref(sctx))) {
                sctx = NULL;
                goto end;
            }
        }
        ossl_quic_tserver_free(qtserv);
        qtserv = NULL;
        SSL_free(clientquic);
        clientquic = NULL;

        if (idx >= 2)
            break;
    }

    ret = 1;

 end:
    SSL_SESSION_free(sess);
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    SSL_CTX_free(sctx);

    return ret;
}

/*
 * Test that sending FIN with no data to a client blocking in SSL_read_ex() will
 * wake up the client.
 */
static int test_fin_only_blocking(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL_CTX *sctx = NULL;
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    const char *msg = "Hello World";
    uint64_t sid;
    size_t numbytes;
    unsigned char buf[32];
    int ret = 0;
    OSSL_TIME timer, timediff;

    if (!qtest_supports_blocking())
        return TEST_skip("Blocking tests not supported in this build");

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, sctx,
                                                    cert, privkey,
                                                    QTEST_FLAG_BLOCK,
                                                    &qtserv, &clientquic,
                                                    NULL, NULL))
            || !TEST_true(SSL_set_tlsext_host_name(clientquic, "localhost")))
        goto end;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto end;

    if (!TEST_true(ossl_quic_tserver_stream_new(qtserv, 0, &sid))
            || !TEST_true(ossl_quic_tserver_write(qtserv, sid,
                                                  (unsigned char *)msg,
                                                  strlen(msg), &numbytes))
            || !TEST_size_t_eq(strlen(msg), numbytes))
        goto end;

    ossl_quic_tserver_tick(qtserv);

    if (!TEST_true(SSL_read_ex(clientquic, buf, sizeof(buf), &numbytes))
            || !TEST_mem_eq(msg, strlen(msg), buf, numbytes))


        goto end;

    if (!TEST_true(ossl_quic_tserver_conclude(qtserv, sid)))
        goto end;

    timer = ossl_time_now();
    if (!TEST_false(SSL_read_ex(clientquic, buf, sizeof(buf), &numbytes)))
        goto end;
    timediff = ossl_time_subtract(ossl_time_now(), timer);

    if (!TEST_int_eq(SSL_get_error(clientquic, 0), SSL_ERROR_ZERO_RETURN)
               /*
                * We expect the SSL_read_ex to not have blocked so this should
                * be very fast. 40ms should be plenty.
                */
            || !TEST_uint64_t_le(ossl_time2ms(timediff), 40))
        goto end;

    if (!TEST_true(qtest_shutdown(qtserv, clientquic)))
        goto end;

    ret = 1;

 end:
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    SSL_CTX_free(sctx);

    return ret;
}

/* Test that a vanilla QUIC SSL object has the expected ciphersuites available */
static int test_ciphersuites(void)
{
    SSL_CTX *ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *ssl = NULL;
    int testresult = 0;
    const STACK_OF(SSL_CIPHER) *ciphers = NULL;
    const SSL_CIPHER *cipher;
    /* We expect this exact list of ciphersuites by default */
    int cipherids[] = {
        TLS1_3_CK_AES_256_GCM_SHA384,
#if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
        TLS1_3_CK_CHACHA20_POLY1305_SHA256,
#endif
        TLS1_3_CK_AES_128_GCM_SHA256
    };
    size_t i, j;

    if (!TEST_ptr(ctx))
        return 0;

    /*
     * Attempting to set TLSv1.2 ciphersuites should succeed, even though they
     * aren't used in QUIC.
     */
    if (!TEST_true(SSL_CTX_set_cipher_list(ctx, "DEFAULT")))
        goto err;

    ssl = SSL_new(ctx);
    if (!TEST_ptr(ssl))
        goto err;

    if (!TEST_true(SSL_set_cipher_list(ssl, "DEFAULT")))
        goto err;

    ciphers = SSL_get_ciphers(ssl);

    for (i = 0, j = 0; i < OSSL_NELEM(cipherids); i++) {
        if (cipherids[i] == TLS1_3_CK_CHACHA20_POLY1305_SHA256 && is_fips)
            continue;
        cipher = sk_SSL_CIPHER_value(ciphers, j++);
        if (!TEST_ptr(cipher))
            goto err;
        if (!TEST_uint_eq(SSL_CIPHER_get_id(cipher), cipherids[i]))
            goto err;
    }

    /* We should have checked all the ciphers in the stack */
    if (!TEST_int_eq(sk_SSL_CIPHER_num(ciphers), j))
        goto err;

    testresult = 1;
 err:
    SSL_free(ssl);
    SSL_CTX_free(ctx);

    return testresult;
}

static int test_cipher_find(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    struct {
        const unsigned char *cipherbytes;
        int ok;
    } testciphers[]  = {
        { TLS13_AES_128_GCM_SHA256_BYTES, 1 },
        { TLS13_AES_256_GCM_SHA384_BYTES, 1 },
        { TLS13_CHACHA20_POLY1305_SHA256_BYTES, 1 },
        { TLS13_AES_128_CCM_SHA256_BYTES, 0 },
        { TLS13_AES_128_CCM_8_SHA256_BYTES, 0 },
#if !defined(OPENSSL_NO_INTEGRITY_ONLY_CIPHERS)
        { TLS13_SHA256_SHA256_BYTES, 0 },
        { TLS13_SHA384_SHA384_BYTES, 0 }
#endif
    };
    size_t i;
    int testresult = 0;

    if (!TEST_ptr(cctx))
        goto err;

    clientquic = SSL_new(cctx);
    if (!TEST_ptr(clientquic))
        goto err;

    for (i = 0; i < OSSL_NELEM(testciphers); i++)
        if (testciphers[i].ok) {
            if (!TEST_ptr(SSL_CIPHER_find(clientquic,
                                          testciphers[i].cipherbytes)))
                goto err;
        } else {
            if (!TEST_ptr_null(SSL_CIPHER_find(clientquic,
                                               testciphers[i].cipherbytes)))
                goto err;
        }

    testresult = 1;
 err:
    SSL_free(clientquic);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test that SSL_version, SSL_get_version, SSL_is_quic, SSL_is_tls and
 * SSL_is_dtls return the expected results for a QUIC connection. Compare with
 * test_version() in sslapitest.c which does the same thing for TLS/DTLS
 * connections.
 */
static int test_version(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, 0, &qtserv,
                                                    &clientquic, NULL, NULL))
            || !TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    if (!TEST_int_eq(SSL_version(clientquic), OSSL_QUIC1_VERSION)
            || !TEST_str_eq(SSL_get_version(clientquic), "QUICv1"))
        goto err;

    if (!TEST_true(SSL_is_quic(clientquic))
            || !TEST_false(SSL_is_tls(clientquic))
            || !TEST_false(SSL_is_dtls(clientquic)))
        goto err;


    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);

    return testresult;
}

#if defined(DO_SSL_TRACE_TEST)
static void strip_line_ends(char *str)
{
    size_t i;

    for (i = strlen(str);
         i > 0 && (str[i - 1] == '\n' || str[i - 1] == '\r');
         i--);

    str[i] = '\0';
}

static int compare_with_file(BIO *membio)
{
    BIO *file = NULL, *newfile = NULL;
    char buf1[8192], buf2[8192];
    char *reffile;
    int ret = 0;
    size_t i;

#ifdef OPENSSL_NO_ZLIB
    reffile = test_mk_file_path(datadir, "ssltraceref.txt");
#else
    reffile = test_mk_file_path(datadir, "ssltraceref-zlib.txt");
#endif
    if (!TEST_ptr(reffile))
        goto err;

    file = BIO_new_file(reffile, "rb");
    if (!TEST_ptr(file))
        goto err;

    newfile = BIO_new_file("ssltraceref-new.txt", "wb");
    if (!TEST_ptr(newfile))
        goto err;

    while (BIO_gets(membio, buf2, sizeof(buf2)) > 0)
        if (BIO_puts(newfile, buf2) <= 0) {
            TEST_error("Failed writing new file data");
            goto err;
        }

    if (!TEST_int_ge(BIO_seek(membio, 0), 0))
        goto err;

    while (BIO_gets(file, buf1, sizeof(buf1)) > 0) {
        size_t line_len;

        if (BIO_gets(membio, buf2, sizeof(buf2)) <= 0) {
            TEST_error("Failed reading mem data");
            goto err;
        }
        strip_line_ends(buf1);
        strip_line_ends(buf2);
        line_len = strlen(buf1);
        if (line_len > 0 && buf1[line_len - 1] == '?') {
            /* Wildcard at the EOL means ignore anything after it */
            if (strlen(buf2) > line_len)
                buf2[line_len] = '\0';
        }
        if (line_len != strlen(buf2)) {
            TEST_error("Actual and ref line data length mismatch");
            TEST_info("%s", buf1);
            TEST_info("%s", buf2);
           goto err;
        }
        for (i = 0; i < line_len; i++) {
            /* '?' is a wild card character in the reference text */
            if (buf1[i] == '?')
                buf2[i] = '?';
        }
        if (!TEST_str_eq(buf1, buf2))
            goto err;
    }
    if (!TEST_true(BIO_eof(file))
            || !TEST_true(BIO_eof(membio)))
        goto err;

    ret = 1;
 err:
    OPENSSL_free(reffile);
    BIO_free(file);
    BIO_free(newfile);
    return ret;
}

/*
 * Tests that the SSL_trace() msg_callback works as expected with a QUIC
 * connection. This also provides testing of the msg_callback at the same time.
 */
static int test_ssl_trace(void)
{
    SSL_CTX *cctx = NULL;
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    BIO *bio = NULL;

    if (!TEST_ptr(cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method()))
            || !TEST_ptr(bio = BIO_new(BIO_s_mem()))
            || !TEST_true(SSL_CTX_set_ciphersuites(cctx, "TLS_AES_128_GCM_SHA256"))
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey,
                                                    QTEST_FLAG_FAKE_TIME,
                                                    &qtserv,
                                                    &clientquic, NULL, NULL)))
        goto err;

    SSL_set_msg_callback(clientquic, SSL_trace);
    SSL_set_msg_callback_arg(clientquic, bio);

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    /* Skip the comparison of the trace when the fips provider is used. */
    if (is_fips) {
        /* Check whether there was something written. */
        if (!TEST_int_gt(BIO_pending(bio), 0))
            goto err;
    } else {
        if (!TEST_true(compare_with_file(bio)))
            goto err;
    }

    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    BIO_free(bio);

    return testresult;
}
#endif

#ifndef OPENSSL_NO_SSL_TRACE
enum {
    INITIAL = 0,
    GATHER_TOKEN = 1,
    CHECK_TOKEN = 2,
    SUCCESS = 3,
    FAILED = 4
};

static int find_new_token_data(BIO *membio)
{
    char buf[1024];
    int state = INITIAL;
    char *tmpstring;
    char *tokenval = NULL;
    /*
     * This is a state machine, in which we traverse the ssl trace
     * looking for a sequence of items
     * The states are:
     * +---Current State---|----------Action-------------|---Next State---+
     * |      INITIAL      | "Received Frame: New token" | GATHER_TOKEN   |
     * |                   | !"Received Frame: New token"| INITIAL        |
     * |-------------------|-----------------------------|----------------|
     * |    GATHER_TOKEN   | "Token: <TOKENVAL>"         | CHECK_TOKEN    |
     * |                   | !"Token: <TOKENVAL>"        | FAILED         |
     * |-------------------|-----------------------------|----------------|
     * |    CHECK_TOKEN    | "Token: <TOKENVAL>"         | SUCCESS        |
     * |                   | EOF                         | FAILED         |
     * +-------------------|-----------------------------|----------------|
     */

    while (state != SUCCESS
           && state != FAILED
           && BIO_gets(membio, buf, sizeof(buf)) > 0) {
        switch (state) {
        case INITIAL:
            if (strstr(buf, "Received Frame: New token"))
                state = GATHER_TOKEN;
            break;
        case GATHER_TOKEN:
            TEST_info("Found New Token Marker\n");
            tmpstring = strstr(buf, "Token: ");
            if (tmpstring == NULL) {
                TEST_info("Next line did not contain a new token\n");
                state = FAILED;
            } else {
                if (!TEST_ptr(tokenval = OPENSSL_strdup(tmpstring)))
                    return 0;
                state = CHECK_TOKEN;
                TEST_info("Recorded Token %s\n", tokenval);
            }
            break;
        case CHECK_TOKEN:
            tmpstring = strstr(buf, "Token: ");
            if (tmpstring != NULL
                && !strcmp(tmpstring, tokenval)) {
                state = SUCCESS;
                TEST_info("Matched next connection token %s\n", tmpstring);
            }
        default:
            break;
        }
    }

    OPENSSL_free(tokenval);
    return (state == SUCCESS);
}

static int test_new_token(void)
{
    SSL_CTX *cctx = NULL;
    SSL *clientquic = NULL;
    SSL *clientquic2 = NULL;
    QUIC_TSERVER *qtserv = NULL;
    QUIC_TSERVER *qtserv2 = NULL;
    int testresult = 0;
    BIO *bio = NULL;
    char msg[] = "The Quic Brown Fox";
    size_t written;

    if (!TEST_ptr(cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method()))
        || !TEST_ptr(bio = BIO_new(BIO_s_mem()))
        || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                privkey,
                                                QTEST_FLAG_FAKE_TIME,
                                                &qtserv,
                                                &clientquic, NULL, NULL)))

        goto err;

    SSL_set_msg_callback(clientquic, SSL_trace);
    SSL_set_msg_callback_arg(clientquic, bio);

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    /* Send data from the client */
    if (!SSL_write_ex(clientquic, msg, sizeof(msg), &written))
        goto err;

    if (written != sizeof(msg))
        goto err;

    /* Receive data at the server */
    ossl_quic_tserver_tick(qtserv);

    if (!TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                             privkey,
                                             QTEST_FLAG_FAKE_TIME,
                                             &qtserv2,
                                             &clientquic2, NULL, NULL)))
        goto err;

    SSL_set_msg_callback(clientquic2, SSL_trace);
    SSL_set_msg_callback_arg(clientquic2, bio);

    /* once we have our new token, create the subsequent connection */
    if (!TEST_true(qtest_create_quic_connection(qtserv2, clientquic2)))
        goto err;

    /* Skip the comparison of the trace when the fips provider is used. */
    if (!TEST_true(find_new_token_data(bio)))
        goto err;

    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    ossl_quic_tserver_free(qtserv2);
    SSL_free(clientquic);
    SSL_free(clientquic2);
    SSL_CTX_free(cctx);
    BIO_free(bio);

    return testresult;
}
#endif

static int ensure_valid_ciphers(const STACK_OF(SSL_CIPHER) *ciphers)
{
    size_t i;

    /* Ensure ciphersuite list is suitably subsetted. */
    for (i = 0; i < (size_t)sk_SSL_CIPHER_num(ciphers); ++i) {
        const SSL_CIPHER *cipher = sk_SSL_CIPHER_value(ciphers, i);
        switch (SSL_CIPHER_get_id(cipher)) {
            case TLS1_3_CK_AES_128_GCM_SHA256:
            case TLS1_3_CK_AES_256_GCM_SHA384:
            case TLS1_3_CK_CHACHA20_POLY1305_SHA256:
                break;
            default:
                TEST_error("forbidden cipher: %s", SSL_CIPHER_get_name(cipher));
                return 0;
        }
    }

    return 1;
}

/*
 * Test that handshake-layer APIs which shouldn't work don't work with QUIC.
 */
static int test_quic_forbidden_apis_ctx(void)
{
    int testresult = 0;
    SSL_CTX *ctx = NULL;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method())))
        goto err;

#ifndef OPENSSL_NO_SRTP
    /* This function returns 0 on success and 1 on error, and should fail. */
    if (!TEST_true(SSL_CTX_set_tlsext_use_srtp(ctx, "SRTP_AEAD_AES_128_GCM")))
        goto err;
#endif

    /*
     * List of ciphersuites we do and don't allow in QUIC.
     */
#define QUIC_CIPHERSUITES \
    "TLS_AES_128_GCM_SHA256:"           \
    "TLS_AES_256_GCM_SHA384:"           \
    "TLS_CHACHA20_POLY1305_SHA256"

#define NON_QUIC_CIPHERSUITES           \
    "TLS_AES_128_CCM_SHA256:"           \
    "TLS_AES_256_CCM_SHA384:"           \
    "TLS_AES_128_CCM_8_SHA256:"         \
    "TLS_SHA256_SHA256:"                \
    "TLS_SHA384_SHA384"

    /* Set TLSv1.3 ciphersuite list for the SSL_CTX. */
    if (!TEST_true(SSL_CTX_set_ciphersuites(ctx,
                                            QUIC_CIPHERSUITES ":"
                                            NON_QUIC_CIPHERSUITES)))
        goto err;

    /*
     * Forbidden ciphersuites should show up in SSL_CTX accessors, they are only
     * filtered in SSL_get1_supported_ciphers, so we don't check for
     * non-inclusion here.
     */

    testresult = 1;
err:
    SSL_CTX_free(ctx);
    return testresult;
}

static int test_quic_forbidden_apis(void)
{
    int testresult = 0;
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    STACK_OF(SSL_CIPHER) *ciphers = NULL;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method())))
        goto err;

    if (!TEST_ptr(ssl = SSL_new(ctx)))
        goto err;

#ifndef OPENSSL_NO_SRTP
    /* This function returns 0 on success and 1 on error, and should fail. */
    if (!TEST_true(SSL_set_tlsext_use_srtp(ssl, "SRTP_AEAD_AES_128_GCM")))
        goto err;
#endif

    /* Set TLSv1.3 ciphersuite list for the SSL_CTX. */
    if (!TEST_true(SSL_set_ciphersuites(ssl,
                                        QUIC_CIPHERSUITES ":"
                                        NON_QUIC_CIPHERSUITES)))
        goto err;

    /* Non-QUIC ciphersuites must not appear in supported ciphers list. */
    if (!TEST_ptr(ciphers = SSL_get1_supported_ciphers(ssl))
        || !TEST_true(ensure_valid_ciphers(ciphers)))
        goto err;

    testresult = 1;
err:
    sk_SSL_CIPHER_free(ciphers);
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return testresult;
}

static int test_quic_forbidden_options(void)
{
    int testresult = 0;
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    char buf[16];
    size_t len;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method())))
        goto err;

    /* QUIC options restrictions do not affect SSL_CTX */
    SSL_CTX_set_options(ctx, UINT64_MAX);

    if (!TEST_uint64_t_eq(SSL_CTX_get_options(ctx), UINT64_MAX))
        goto err;

    /* Set options on CTX which should not be inherited (tested below). */
    SSL_CTX_set_read_ahead(ctx, 1);
    SSL_CTX_set_max_early_data(ctx, 1);
    SSL_CTX_set_recv_max_early_data(ctx, 1);
    SSL_CTX_set_quiet_shutdown(ctx, 1);

    if (!TEST_ptr(ssl = SSL_new(ctx)))
        goto err;

    /* Only permitted options get transferred to SSL object */
    if (!TEST_uint64_t_eq(SSL_get_options(ssl), OSSL_QUIC_PERMITTED_OPTIONS))
        goto err;

    /* Try again using SSL_set_options */
    SSL_set_options(ssl, UINT64_MAX);

    if (!TEST_uint64_t_eq(SSL_get_options(ssl), OSSL_QUIC_PERMITTED_OPTIONS))
        goto err;

    /* Clear everything */
    SSL_clear_options(ssl, UINT64_MAX);

    if (!TEST_uint64_t_eq(SSL_get_options(ssl), 0))
        goto err;

    /* Readahead */
    if (!TEST_false(SSL_get_read_ahead(ssl)))
        goto err;

    SSL_set_read_ahead(ssl, 1);
    if (!TEST_false(SSL_get_read_ahead(ssl)))
        goto err;

    /* Block padding */
    if (!TEST_true(SSL_set_block_padding(ssl, 0))
        || !TEST_true(SSL_set_block_padding(ssl, 1))
        || !TEST_false(SSL_set_block_padding(ssl, 2)))
        goto err;

    /* Max fragment length */
    if (!TEST_true(SSL_set_tlsext_max_fragment_length(ssl, TLSEXT_max_fragment_length_DISABLED))
        || !TEST_false(SSL_set_tlsext_max_fragment_length(ssl, TLSEXT_max_fragment_length_512)))
        goto err;

    /* Max early data */
    if (!TEST_false(SSL_set_recv_max_early_data(ssl, 1))
        || !TEST_false(SSL_set_max_early_data(ssl, 1)))
        goto err;

    /* Read/Write */
    if (!TEST_false(SSL_read_early_data(ssl, buf, sizeof(buf), &len))
        || !TEST_false(SSL_write_early_data(ssl, buf, sizeof(buf), &len)))
        goto err;

    /* Buffer Management */
    if (!TEST_true(SSL_alloc_buffers(ssl))
        || !TEST_false(SSL_free_buffers(ssl)))
        goto err;

    /* Pipelining */
    if (!TEST_false(SSL_set_max_send_fragment(ssl, 2))
        || !TEST_false(SSL_set_split_send_fragment(ssl, 2))
        || !TEST_false(SSL_set_max_pipelines(ssl, 2)))
        goto err;

    /* HRR */
    if  (!TEST_false(SSL_stateless(ssl)))
        goto err;

    /* Quiet Shutdown */
    if (!TEST_false(SSL_get_quiet_shutdown(ssl)))
        goto err;

    /* No duplication */
    if (!TEST_ptr_null(SSL_dup(ssl)))
        goto err;

    /* No clear */
    if (!TEST_false(SSL_clear(ssl)))
        goto err;

    testresult = 1;
err:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return testresult;
}

static int test_quic_set_fd(int idx)
{
    int testresult = 0;
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    int fd = -1, resfd = -1;
    BIO *bio = NULL;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method())))
        goto err;

    if (!TEST_ptr(ssl = SSL_new(ctx)))
        goto err;

    if (!TEST_int_ge(fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0), 0))
        goto err;

    if (idx == 0) {
        if (!TEST_true(SSL_set_fd(ssl, fd)))
            goto err;
        if (!TEST_ptr(bio = SSL_get_rbio(ssl)))
            goto err;
        if (!TEST_ptr_eq(bio, SSL_get_wbio(ssl)))
            goto err;
    } else if (idx == 1) {
        if (!TEST_true(SSL_set_rfd(ssl, fd)))
            goto err;
        if (!TEST_ptr(bio = SSL_get_rbio(ssl)))
            goto err;
        if (!TEST_ptr_null(SSL_get_wbio(ssl)))
            goto err;
    } else {
        if (!TEST_true(SSL_set_wfd(ssl, fd)))
            goto err;
        if (!TEST_ptr(bio = SSL_get_wbio(ssl)))
            goto err;
        if (!TEST_ptr_null(SSL_get_rbio(ssl)))
            goto err;
    }

    if (!TEST_int_eq(BIO_method_type(bio), BIO_TYPE_DGRAM))
        goto err;

    if (!TEST_true(BIO_get_fd(bio, &resfd))
        || !TEST_int_eq(resfd, fd))
        goto err;

    testresult = 1;
err:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    if (fd >= 0)
        BIO_closesocket(fd);
    return testresult;
}

#define MAXLOOPS    1000

static int test_bio_ssl(void)
{
    /*
     * We just use OSSL_QUIC_client_method() rather than
     * OSSL_QUIC_client_thread_method(). We will never leave the connection idle
     * so we will always be implicitly handling time events anyway via other
     * IO calls.
     */
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL, *stream = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    BIO *cbio = NULL, *strbio = NULL, *thisbio;
    const char *msg = "Hello world";
    int abortctr = 0, err, clienterr = 0, servererr = 0, retc = 0, rets = 0;
    size_t written, readbytes, msglen;
    int sid = 0, i;
    unsigned char buf[80];

    if (!TEST_ptr(cctx))
        goto err;

    cbio = BIO_new_ssl(cctx, 1);
    if (!TEST_ptr(cbio))
        goto err;

    /*
     * We must configure the ALPN/peer address etc so we get the SSL object in
     * order to pass it to qtest_create_quic_objects for configuration.
     */
    if (!TEST_int_eq(BIO_get_ssl(cbio, &clientquic), 1))
        goto err;

    if (!TEST_true(qtest_create_quic_objects(libctx, NULL, NULL, cert, privkey,
                                             QTEST_FLAG_FAKE_TIME, &qtserv,
                                             &clientquic, NULL, NULL)))
        goto err;

    msglen = strlen(msg);

    do {
        err = BIO_FLAGS_WRITE;
        while (!clienterr && !retc && err == BIO_FLAGS_WRITE) {
            retc = BIO_write_ex(cbio, msg, msglen, &written);
            if (!retc) {
                if (BIO_should_retry(cbio))
                    err = BIO_retry_type(cbio);
                else
                    err = 0;
            }
        }

        if (!clienterr && retc <= 0 && err != BIO_FLAGS_READ) {
            TEST_info("BIO_write_ex() failed %d, %d", retc, err);
            TEST_openssl_errors();
            clienterr = 1;
        }

        if (!servererr && rets <= 0) {
            ossl_quic_tserver_tick(qtserv);
            qtest_add_time(100);
            servererr = ossl_quic_tserver_is_term_any(qtserv);
            if (!servererr)
                rets = ossl_quic_tserver_is_handshake_confirmed(qtserv);
        }

        if (clienterr && servererr)
            goto err;

        if (++abortctr == MAXLOOPS) {
            TEST_info("No progress made");
            goto err;
        }
    } while ((!retc && !clienterr) || (rets <= 0 && !servererr));

    /*
     * 2 loops: The first using the default stream, and the second using a new
     * client initiated bidi stream.
     */
    for (i = 0, thisbio = cbio; i < 2; i++) {
        if (!TEST_true(ossl_quic_tserver_read(qtserv, sid, buf, sizeof(buf),
                                              &readbytes))
                || !TEST_mem_eq(msg, msglen, buf, readbytes))
            goto err;

        if (!TEST_true(ossl_quic_tserver_write(qtserv, sid, (unsigned char *)msg,
                                               msglen, &written)))
            goto err;
        ossl_quic_tserver_tick(qtserv);

        if (!TEST_true(BIO_read_ex(thisbio, buf, sizeof(buf), &readbytes))
                || !TEST_mem_eq(msg, msglen, buf, readbytes))
            goto err;

        if (i == 1)
            break;

        if (!TEST_true(SSL_set_mode(clientquic, 0)))
            goto err;

        /*
         * Now create a new stream and repeat. The bottom two bits of the stream
         * id represents whether the stream is bidi and whether it is client
         * initiated or not. For client initiated bidi they are both 0. So the
         * first client initiated bidi stream is 0 and the next one is 4.
         */
        sid = 4;
        stream = SSL_new_stream(clientquic, 0);
        if (!TEST_ptr(stream))
            goto err;

        if (!TEST_true(SSL_set_mode(stream, 0)))
            goto err;

        thisbio = strbio = BIO_new(BIO_f_ssl());
        if (!TEST_ptr(strbio))
            goto err;

        if (!TEST_int_eq(BIO_set_ssl(thisbio, stream, BIO_CLOSE), 1))
            goto err;
        stream = NULL;

        if (!TEST_true(BIO_write_ex(thisbio, msg, msglen, &written)))
            goto err;

        ossl_quic_tserver_tick(qtserv);
    }

    testresult = 1;
 err:
    BIO_free_all(cbio);
    BIO_free_all(strbio);
    SSL_free(stream);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);

    return testresult;
}

#define BACK_PRESSURE_NUM_LOOPS 10000
/*
 * Test that sending data from the client to the server faster than the server
 * can process it eventually results in back pressure on the client.
 */
static int test_back_pressure(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    unsigned char *msg = NULL;
    const size_t msglen = 1024;
    unsigned char buf[64];
    size_t readbytes, written;
    int i;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, 0, &qtserv,
                                                    &clientquic, NULL, NULL))
            || !TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    msg = OPENSSL_malloc(msglen);
    if (!TEST_ptr(msg))
        goto err;
    if (!TEST_int_eq(RAND_bytes_ex(libctx, msg, msglen, 0), 1))
        goto err;

    /*
     * Limit to 10000 loops. If we've not seen any back pressure after that
     * we're going to run out of memory, so abort.
     */
    for (i = 0; i < BACK_PRESSURE_NUM_LOOPS; i++) {
        /* Send data from the client */
        if (!SSL_write_ex(clientquic, msg, msglen, &written)) {
            /* Check if we are seeing back pressure */
            if (SSL_get_error(clientquic, 0) == SSL_ERROR_WANT_WRITE)
                break;
            TEST_error("Unexpected client failure");
            goto err;
        }

        /* Receive data at the server */
        ossl_quic_tserver_tick(qtserv);
        if (!TEST_true(ossl_quic_tserver_read(qtserv, 0, buf, sizeof(buf),
                                              &readbytes)))
            goto err;
    }

    if (i == BACK_PRESSURE_NUM_LOOPS) {
        TEST_error("No back pressure seen");
        goto err;
    }

    testresult = 1;
 err:
    SSL_free(clientquic);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);
    OPENSSL_free(msg);

    return testresult;
}


static int dgram_ctr = 0;

static void dgram_cb(int write_p, int version, int content_type,
                     const void *buf, size_t msglen, SSL *ssl, void *arg)
{
    if (!write_p)
        return;

    if (content_type != SSL3_RT_QUIC_DATAGRAM)
        return;

    dgram_ctr++;
}

/* Test that we send multiple datagrams in one go when appropriate */
static int test_multiple_dgrams(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    unsigned char *buf;
    const size_t buflen = 1400;
    size_t written;

    buf = OPENSSL_zalloc(buflen);

    if (!TEST_ptr(cctx)
            || !TEST_ptr(buf)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, 0, &qtserv,
                                                    &clientquic, NULL, NULL))
            || !TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    dgram_ctr = 0;
    SSL_set_msg_callback(clientquic, dgram_cb);
    if (!TEST_true(SSL_write_ex(clientquic, buf, buflen, &written))
            || !TEST_size_t_eq(written, buflen)
               /* We wrote enough data for 2 datagrams */
            || !TEST_int_eq(dgram_ctr, 2))
        goto err;

    testresult = 1;
 err:
    OPENSSL_free(buf);
    SSL_free(clientquic);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);

    return testresult;
}

static int non_io_retry_cert_verify_cb(X509_STORE_CTX *ctx, void *arg)
{
    int idx = SSL_get_ex_data_X509_STORE_CTX_idx();
    SSL *ssl;
    const int *allow = (int *)arg;

    /* this should not happen but check anyway */
    if (idx < 0
        || (ssl = X509_STORE_CTX_get_ex_data(ctx, idx)) == NULL)
        return 0;

    /* If this is our first attempt then retry */
    if (*allow == 0)
        return SSL_set_retry_verify(ssl);

    /* Otherwise do nothing - verification succeeds. Continue as normal */
    return 1;
}

/* Test that we can handle a non-io related retry error
 * Test 0: Non-blocking
 * Test 1: Blocking
 */
static int test_non_io_retry(int idx)
{
    SSL_CTX *cctx;
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    int flags = 0, allow = 0;

    if (idx >= 1 && !qtest_supports_blocking())
        return TEST_skip("Blocking tests not supported in this build");

    cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    if (!TEST_ptr(cctx))
        goto err;

    SSL_CTX_set_cert_verify_callback(cctx, non_io_retry_cert_verify_cb, &allow);

    flags = (idx >= 1) ? QTEST_FLAG_BLOCK : 0;
    if (!TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert, privkey,
                                             flags, &qtserv, &clientquic, NULL,
                                             NULL))
            || !TEST_true(qtest_create_quic_connection_ex(qtserv, clientquic,
                            SSL_ERROR_WANT_RETRY_VERIFY))
            || !TEST_int_eq(SSL_want(clientquic), SSL_RETRY_VERIFY))
        goto err;

    allow = 1;
    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    testresult = 1;
 err:
    SSL_free(clientquic);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);

    return testresult;
}

static int use_session_cb_cnt = 0;
static int find_session_cb_cnt = 0;
static const char *pskid = "Identity";
static SSL_SESSION *serverpsk = NULL, *clientpsk = NULL;

static int use_session_cb(SSL *ssl, const EVP_MD *md, const unsigned char **id,
                          size_t *idlen, SSL_SESSION **sess)
{
    use_session_cb_cnt++;

    if (clientpsk == NULL || !SSL_SESSION_up_ref(clientpsk))
        return 0;

    *sess = clientpsk;
    *id = (const unsigned char *)pskid;
    *idlen = strlen(pskid);

    return 1;
}

static int find_session_cb(SSL *ssl, const unsigned char *identity,
                           size_t identity_len, SSL_SESSION **sess)
{
    find_session_cb_cnt++;

    if (serverpsk == NULL || !SSL_SESSION_up_ref(serverpsk))
        return 0;

    /* Identity should match that set by the client */
    if (strlen(pskid) != identity_len
            || strncmp(pskid, (const char *)identity, identity_len) != 0) {
        SSL_SESSION_free(serverpsk);
        return 0;
    }

    *sess = serverpsk;

    return 1;
}

static int test_quic_psk(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;

    if (!TEST_ptr(cctx)
               /* No cert or private key for the server, i.e. PSK only */
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, NULL,
                                                    NULL, 0, &qtserv,
                                                    &clientquic, NULL, NULL)))
        goto end;

    SSL_set_psk_use_session_callback(clientquic, use_session_cb);
    ossl_quic_tserver_set_psk_find_session_cb(qtserv, find_session_cb);
    use_session_cb_cnt = 0;
    find_session_cb_cnt = 0;

    clientpsk = serverpsk = create_a_psk(clientquic, SHA384_DIGEST_LENGTH);
    /* We already had one ref. Add another one */
    if (!TEST_ptr(clientpsk) || !TEST_true(SSL_SESSION_up_ref(clientpsk)))
        goto end;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic))
            || !TEST_int_eq(1, find_session_cb_cnt)
            || !TEST_int_eq(1, use_session_cb_cnt)
               /* Check that we actually used the PSK */
            || !TEST_true(SSL_session_reused(clientquic)))
        goto end;

    testresult = 1;

 end:
    SSL_free(clientquic);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;

    return testresult;
}

static int test_client_auth(int idx)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL_CTX *sctx = SSL_CTX_new_ex(libctx, NULL, TLS_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    unsigned char buf[20];
    static char *msg = "A test message";
    size_t msglen = strlen(msg);
    size_t numbytes = 0;

    if (!TEST_ptr(cctx) || !TEST_ptr(sctx))
        goto err;

    SSL_CTX_set_verify(sctx, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT
                             | SSL_VERIFY_CLIENT_ONCE, NULL);

    if (!TEST_true(SSL_CTX_load_verify_file(sctx, cauthca)))
        goto err;

    if (idx > 0
        && (!TEST_true(SSL_CTX_use_certificate_chain_file(cctx, ccert))
            || !TEST_true(SSL_CTX_use_PrivateKey_file(cctx, cprivkey,
                                                      SSL_FILETYPE_PEM))))
            goto err;

    if (!TEST_true(qtest_create_quic_objects(libctx, cctx, sctx, cert,
                                             privkey, 0, &qtserv,
                                             &clientquic, NULL, NULL)))
        goto err;

    if (idx > 1) {
        if (!TEST_true(ssl_ctx_add_large_cert_chain(libctx, cctx, ccert))
            || !TEST_true(ssl_ctx_add_large_cert_chain(libctx, sctx, cert)))
            goto err;
    }

    if (idx == 0) {
        if (!TEST_false(qtest_create_quic_connection(qtserv, clientquic)))
            goto err;

        /* negative test passed */
        testresult = 1;
        goto err;
    }

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    /* Check that sending and receiving app data is ok */
    if (!TEST_true(SSL_write_ex(clientquic, msg, msglen, &numbytes))
        || !TEST_size_t_eq(numbytes, msglen))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    if (!TEST_true(ossl_quic_tserver_write(qtserv, 0,
                                           (unsigned char *)msg,
                                           msglen, &numbytes)))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    SSL_handle_events(clientquic);

    if (!TEST_true(SSL_read_ex(clientquic, buf, sizeof(buf), &numbytes))
            || !TEST_size_t_eq(numbytes, msglen)
            || !TEST_mem_eq(buf, numbytes, msg, msglen))
        goto err;

    if (!TEST_true(qtest_shutdown(qtserv, clientquic)))
        goto err;

    testresult = 1;

 err:
    SSL_free(clientquic);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test that we correctly handle ALPN supplied by the application
 * Test 0: ALPN is provided
 * Test 1: No ALPN is provided
 */
static int test_alpn(int idx)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    int ret;

    /*
     * Ensure we only configure ciphersuites that are available with both the
     * default and fips providers to get the same output in both cases
     */
    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx, "TLS_AES_128_GCM_SHA256")))
        goto err;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey,
                                                    QTEST_FLAG_FAKE_TIME,
                                                    &qtserv,
                                                    &clientquic, NULL, NULL)))
        goto err;

    if (idx == 0) {
        /*
        * Clear the ALPN we set in qtest_create_quic_objects. We use TEST_false
        * because SSL_set_alpn_protos returns 0 for success.
        */
        if (!TEST_false(SSL_set_alpn_protos(clientquic, NULL, 0)))
            goto err;
    }

    ret = SSL_connect(clientquic);
    if (!TEST_int_le(ret, 0))
        goto err;
    if (idx == 0) {
        /* We expect an immediate error due to lack of ALPN */
        if (!TEST_int_eq(SSL_get_error(clientquic, ret), SSL_ERROR_SSL))
            goto err;
    } else {
        /* ALPN was provided so we expect the connection to succeed */
        if (!TEST_int_eq(SSL_get_error(clientquic, ret), SSL_ERROR_WANT_READ)
                || !TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
            goto err;
    }

    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test SSL_get_shutdown() behavior.
 */
static int test_get_shutdown(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey,
                                                    QTEST_FLAG_FAKE_TIME,
                                                    &qtserv, &clientquic,
                                                    NULL, NULL))
            || !TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    if (!TEST_int_eq(SSL_get_shutdown(clientquic), 0))
        goto err;

    if (!TEST_int_eq(SSL_shutdown(clientquic), 0))
        goto err;

    if (!TEST_int_eq(SSL_get_shutdown(clientquic), SSL_SENT_SHUTDOWN))
        goto err;

    do {
        ossl_quic_tserver_tick(qtserv);
        qtest_add_time(100);
    } while (SSL_shutdown(clientquic) == 0);

    if (!TEST_int_eq(SSL_get_shutdown(clientquic),
                     SSL_SENT_SHUTDOWN | SSL_RECEIVED_SHUTDOWN))
        goto err;

    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);

    return testresult;
}

#define MAX_LOOPS   2000

/*
 * Keep retrying SSL_read_ex until it succeeds or we give up. Accept a stream
 * if we don't already have one
 */
static int unreliable_client_read(SSL *clientquic, SSL **stream, void *buf,
                                  size_t buflen, size_t *readbytes,
                                  QUIC_TSERVER *qtserv)
{
    int abortctr;

    /* We just do this in a loop with a sleep for simplicity */
    for (abortctr = 0; abortctr < MAX_LOOPS; abortctr++) {
        if (*stream == NULL) {
            SSL_handle_events(clientquic);
            *stream = SSL_accept_stream(clientquic, 0);
        }

        if (*stream != NULL) {
            if (SSL_read_ex(*stream, buf, buflen, readbytes))
                return 1;
            if (!TEST_int_eq(SSL_get_error(*stream, 0), SSL_ERROR_WANT_READ))
                return 0;
        }
        ossl_quic_tserver_tick(qtserv);
        qtest_add_time(1);
        qtest_wait_for_timeout(clientquic, qtserv);
    }

    TEST_error("No progress made");
    return 0;
}

/* Keep retrying ossl_quic_tserver_read until it succeeds or we give up */
static int unreliable_server_read(QUIC_TSERVER *qtserv, uint64_t sid,
                                  void *buf, size_t buflen, size_t *readbytes,
                                  SSL *clientquic)
{
    int abortctr;

    /* We just do this in a loop with a sleep for simplicity */
    for (abortctr = 0; abortctr < MAX_LOOPS; abortctr++) {
        if (ossl_quic_tserver_read(qtserv, sid, buf, buflen, readbytes)
                && *readbytes > 1)
            return 1;
        ossl_quic_tserver_tick(qtserv);
        SSL_handle_events(clientquic);
        qtest_add_time(1);
        qtest_wait_for_timeout(clientquic, qtserv);
    }

    TEST_error("No progress made");
    return 0;
}

/*
 * Create a connection and send data using an unreliable transport. We introduce
 * random noise to drop, delay and duplicate datagrams.
 * Test 0: Introduce random noise to datagrams
 * Test 1: As with test 0 but also split datagrams containing multiple packets
 *         into individual datagrams so that individual packets can be affected
 *         by noise - not just a whole datagram.
 */
static int test_noisy_dgram(int idx)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL, *stream[2] = { NULL, NULL };
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    uint64_t sid = 0;
    char *msg = "Hello world!";
    size_t msglen = strlen(msg), written, readbytes, i, j;
    unsigned char buf[80];
    int flags = QTEST_FLAG_NOISE | QTEST_FLAG_FAKE_TIME;
    QTEST_FAULT *fault = NULL;

    if (idx == 1)
        flags |= QTEST_FLAG_PACKET_SPLIT;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, flags,
                                                    &qtserv,
                                                    &clientquic, &fault, NULL)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
            goto err;

    if (!TEST_true(SSL_set_incoming_stream_policy(clientquic,
                                                  SSL_INCOMING_STREAM_POLICY_ACCEPT,
                                                  0))
            || !TEST_true(SSL_set_default_stream_mode(clientquic,
                                                      SSL_DEFAULT_STREAM_MODE_NONE)))
        goto err;

    for (j = 0; j < 2; j++) {
        if (!TEST_true(ossl_quic_tserver_stream_new(qtserv, 0, &sid)))
            goto err;
        ossl_quic_tserver_tick(qtserv);
        qtest_add_time(1);

        /*
         * Send data from the server to the client. Some datagrams may get
         * lost, modified, dropped or re-ordered. We repeat 20 times to ensure
         * we are sending enough datagrams for problems to be noticed.
         */
        for (i = 0; i < 20; i++) {
            if (!TEST_true(ossl_quic_tserver_write(qtserv, sid,
                                                   (unsigned char *)msg, msglen,
                                                   &written))
                    || !TEST_size_t_eq(msglen, written))
                goto err;
            ossl_quic_tserver_tick(qtserv);
            qtest_add_time(1);

            /*
             * Since the underlying BIO is now noisy we may get failures that
             * need to be retried - so we use unreliable_client_read() to
             * handle that
             */
            if (!TEST_true(unreliable_client_read(clientquic, &stream[j], buf,
                                                  sizeof(buf), &readbytes,
                                                  qtserv))
                    || !TEST_mem_eq(msg, msglen, buf, readbytes))
                goto err;
        }

        /* Send data from the client to the server */
        for (i = 0; i < 20; i++) {
            if (!TEST_true(SSL_write_ex(stream[j], (unsigned char *)msg,
                                        msglen, &written))
                    || !TEST_size_t_eq(msglen, written))
                goto err;

            ossl_quic_tserver_tick(qtserv);
            qtest_add_time(1);

            /*
             * Since the underlying BIO is now noisy we may get failures that
             * need to be retried - so we use unreliable_server_read() to
             * handle that
             */
            if (!TEST_true(unreliable_server_read(qtserv, sid, buf, sizeof(buf),
                                                  &readbytes, clientquic))
                    || !TEST_mem_eq(msg, msglen, buf, readbytes))
                goto err;
        }
    }

    testresult = 1;
 err:
    ossl_quic_tserver_free(qtserv);
    SSL_free(stream[0]);
    SSL_free(stream[1]);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    qtest_fault_free(fault);

    return testresult;
}

/*
 * Create a connection and send some big data using a transport with limited bandwidth.
 */

#define TEST_TRANSFER_DATA_SIZE (2*1024*1024)    /* 2 MBytes */
#define TEST_SINGLE_WRITE_SIZE (16*1024)        /* 16 kBytes */
#define TEST_BW_LIMIT 1000                      /* 1000 Bytes/ms */
static int test_bw_limit(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    unsigned char *msg = NULL, *recvbuf = NULL;
    size_t sendlen = TEST_TRANSFER_DATA_SIZE;
    size_t recvlen = TEST_TRANSFER_DATA_SIZE;
    size_t written, readbytes;
    int flags = QTEST_FLAG_NOISE | QTEST_FLAG_FAKE_TIME;
    QTEST_FAULT *fault = NULL;
    uint64_t real_bw;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, flags,
                                                    &qtserv,
                                                    &clientquic, &fault, NULL)))
        goto err;

    if (!TEST_ptr(msg = OPENSSL_zalloc(TEST_SINGLE_WRITE_SIZE))
        || !TEST_ptr(recvbuf = OPENSSL_zalloc(TEST_SINGLE_WRITE_SIZE)))
        goto err;

    /* Set BW to 1000 Bytes/ms -> 1MByte/s both ways */
    if (!TEST_true(qtest_fault_set_bw_limit(fault, 1000, 1000, 0)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
            goto err;

    qtest_start_stopwatch();

    while (recvlen > 0) {
        qtest_add_time(1);

        if (sendlen > 0) {
            if (!SSL_write_ex(clientquic, msg,
                              sendlen > TEST_SINGLE_WRITE_SIZE ? TEST_SINGLE_WRITE_SIZE
                                                               : sendlen,
                              &written)) {
                TEST_info("Retrying to send: %llu", (unsigned long long) sendlen);
                if (!TEST_int_eq(SSL_get_error(clientquic, 0), SSL_ERROR_WANT_WRITE))
                    goto err;
            } else {
                sendlen -= written;
                TEST_info("Remaining to send: %llu", (unsigned long long) sendlen);
            }
        } else {
            SSL_handle_events(clientquic);
        }

        if (ossl_quic_tserver_read(qtserv, 0, recvbuf,
                                   recvlen > TEST_SINGLE_WRITE_SIZE ? TEST_SINGLE_WRITE_SIZE
                                                                    : recvlen,
                                   &readbytes)
            && readbytes > 1) {
            recvlen -= readbytes;
            TEST_info("Remaining to recv: %llu", (unsigned long long) recvlen);
        } else {
            TEST_info("No progress on recv: %llu", (unsigned long long) recvlen);
        }
        ossl_quic_tserver_tick(qtserv);
    }
    real_bw = TEST_TRANSFER_DATA_SIZE / qtest_get_stopwatch_time();

    TEST_info("BW limit: %d Bytes/ms Real bandwidth reached: %llu Bytes/ms",
              TEST_BW_LIMIT, (unsigned long long)real_bw);

    if (!TEST_uint64_t_lt(real_bw, TEST_BW_LIMIT))
        goto err;

    testresult = 1;
 err:
    OPENSSL_free(msg);
    OPENSSL_free(recvbuf);
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    qtest_fault_free(fault);

    return testresult;
}

enum {
    TPARAM_OP_DUP,
    TPARAM_OP_DROP,
    TPARAM_OP_INJECT,
    TPARAM_OP_INJECT_TWICE,
    TPARAM_OP_INJECT_RAW,
    TPARAM_OP_DROP_INJECT,
    TPARAM_OP_MUTATE
};

#define TPARAM_CHECK_DUP(name, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_DUP, (reason) },
#define TPARAM_CHECK_DROP(name, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_DROP, (reason) },
#define TPARAM_CHECK_INJECT(name, buf, buf_len, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_INJECT, (reason), \
      (buf), (buf_len) },
#define TPARAM_CHECK_INJECT_A(name, buf, reason) \
    TPARAM_CHECK_INJECT(name, buf, sizeof(buf), reason)
#define TPARAM_CHECK_DROP_INJECT(name, buf, buf_len, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_DROP_INJECT, (reason), \
      (buf), (buf_len) },
#define TPARAM_CHECK_DROP_INJECT_A(name, buf, reason) \
    TPARAM_CHECK_DROP_INJECT(name, buf, sizeof(buf), reason)
#define TPARAM_CHECK_INJECT_TWICE(name, buf, buf_len, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_INJECT_TWICE, (reason), \
      (buf), (buf_len) },
#define TPARAM_CHECK_INJECT_TWICE_A(name, buf, reason) \
    TPARAM_CHECK_INJECT_TWICE(name, buf, sizeof(buf), reason)
#define TPARAM_CHECK_INJECT_RAW(buf, buf_len, reason) \
    { 0, TPARAM_OP_INJECT_RAW, (reason), \
      (buf), (buf_len) },
#define TPARAM_CHECK_INJECT_RAW_A(buf, reason) \
    TPARAM_CHECK_INJECT_RAW(buf, sizeof(buf), reason)
#define TPARAM_CHECK_MUTATE(name, reason) \
    { QUIC_TPARAM_##name, TPARAM_OP_MUTATE, (reason) },
#define TPARAM_CHECK_INT(name, reason) \
    TPARAM_CHECK_DROP_INJECT(name, NULL, 0, reason) \
    TPARAM_CHECK_DROP_INJECT_A(name, bogus_int, reason) \
    TPARAM_CHECK_DROP_INJECT_A(name, int_with_trailer, reason)

struct tparam_test {
    uint64_t    id;
    int         op;
    const char  *expect_fail; /* substring to expect in reason */
    const void  *buf;
    size_t      buf_len;
};


static const unsigned char disable_active_migration_1[] = {
    0x00
};

static const unsigned char malformed_stateless_reset_token_1[] = {
    0x02, 0xff
};

static const unsigned char malformed_stateless_reset_token_2[] = {
    0x01
};

static const unsigned char malformed_stateless_reset_token_3[15] = { 0 };

static const unsigned char malformed_stateless_reset_token_4[17] = { 0 };

static const unsigned char malformed_preferred_addr_1[] = {
    0x0d, 0xff
};

static const unsigned char malformed_preferred_addr_2[42] = {
    0x0d, 0x28, /* too short */
};

static const unsigned char malformed_preferred_addr_3[64] = {
    0x0d, 0x3e, /* too long */
};

static const unsigned char malformed_preferred_addr_4[] = {
    /* TPARAM too short for CID length indicated */
    0x0d, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x01, 0x55,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
};

static const unsigned char malformed_unknown_1[] = {
    0xff
};

static const unsigned char malformed_unknown_2[] = {
    0x55, 0x55,
};

static const unsigned char malformed_unknown_3[] = {
    0x55, 0x55, 0x01,
};

static const unsigned char ack_delay_exp[] = {
    0x03
};

static const unsigned char stateless_reset_token[16] = { 0x42 };

static const unsigned char preferred_addr[] = {
    0x44, 0x44, 0x44, 0x44,
    0x55, 0x55,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x77, 0x77,
    0x02, 0xAA, 0xBB,
    0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
    0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
};

static const unsigned char long_cid[21] = { 0x42 };

static const unsigned char excess_ack_delay_exp[] = {
    0x15,
};

static const unsigned char excess_max_ack_delay[] = {
    0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00,
};

static const unsigned char excess_initial_max_streams[] = {
    0xD0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
};

static const unsigned char undersize_udp_payload_size[] = {
    0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xaf,
};

static const unsigned char undersize_active_conn_id_limit[] = {
    0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
};

static const unsigned char bogus_int[9] = { 0 };

static const unsigned char int_with_trailer[2] = { 0x01 };

#define QUIC_TPARAM_UNKNOWN_1   0xf1f1

static const struct tparam_test tparam_tests[] = {
    TPARAM_CHECK_DUP(ORIG_DCID,
                     "ORIG_DCID appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_SCID,
                     "INITIAL_SCID appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_DATA,
                     "INITIAL_MAX_DATA appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_STREAM_DATA_BIDI_LOCAL,
                     "INITIAL_MAX_STREAM_DATA_BIDI_LOCAL appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_STREAM_DATA_BIDI_REMOTE,
                     "INITIAL_MAX_STREAM_DATA_BIDI_REMOTE appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_STREAM_DATA_UNI,
                     "INITIAL_MAX_STREAM_DATA_UNI appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_STREAMS_BIDI,
                     "INITIAL_MAX_STREAMS_BIDI appears multiple times")
    TPARAM_CHECK_DUP(INITIAL_MAX_STREAMS_UNI,
                     "INITIAL_MAX_STREAMS_UNI appears multiple times")
    TPARAM_CHECK_DUP(MAX_IDLE_TIMEOUT,
                     "MAX_IDLE_TIMEOUT appears multiple times")
    TPARAM_CHECK_DUP(MAX_UDP_PAYLOAD_SIZE,
                     "MAX_UDP_PAYLOAD_SIZE appears multiple times")
    TPARAM_CHECK_DUP(ACTIVE_CONN_ID_LIMIT,
                     "ACTIVE_CONN_ID_LIMIT appears multiple times")
    TPARAM_CHECK_DUP(DISABLE_ACTIVE_MIGRATION,
                     "DISABLE_ACTIVE_MIGRATION appears multiple times")

    TPARAM_CHECK_DROP(INITIAL_SCID,
                      "INITIAL_SCID was not sent but is required")
    TPARAM_CHECK_DROP(ORIG_DCID,
                      "ORIG_DCID was not sent but is required")

    TPARAM_CHECK_DROP_INJECT_A(DISABLE_ACTIVE_MIGRATION, disable_active_migration_1,
                               "DISABLE_ACTIVE_MIGRATION is malformed")
    TPARAM_CHECK_INJECT(UNKNOWN_1, NULL, 0,
                        NULL)
    TPARAM_CHECK_INJECT_RAW_A(malformed_stateless_reset_token_1,
                              "STATELESS_RESET_TOKEN is malformed")
    TPARAM_CHECK_INJECT_A(STATELESS_RESET_TOKEN,
                          malformed_stateless_reset_token_2,
                          "STATELESS_RESET_TOKEN is malformed")
    TPARAM_CHECK_INJECT_A(STATELESS_RESET_TOKEN,
                          malformed_stateless_reset_token_3,
                          "STATELESS_RESET_TOKEN is malformed")
    TPARAM_CHECK_INJECT_A(STATELESS_RESET_TOKEN,
                          malformed_stateless_reset_token_4,
                          "STATELESS_RESET_TOKEN is malformed")
    TPARAM_CHECK_INJECT(STATELESS_RESET_TOKEN,
                        NULL, 0,
                        "STATELESS_RESET_TOKEN is malformed")
    TPARAM_CHECK_INJECT_RAW_A(malformed_preferred_addr_1,
                              "PREFERRED_ADDR is malformed")
    TPARAM_CHECK_INJECT_RAW_A(malformed_preferred_addr_2,
                              "PREFERRED_ADDR is malformed")
    TPARAM_CHECK_INJECT_RAW_A(malformed_preferred_addr_3,
                              "PREFERRED_ADDR is malformed")
    TPARAM_CHECK_INJECT_RAW_A(malformed_preferred_addr_4,
                              "PREFERRED_ADDR is malformed")
    TPARAM_CHECK_INJECT_RAW_A(malformed_unknown_1,
                              "bad transport parameter")
    TPARAM_CHECK_INJECT_RAW_A(malformed_unknown_2,
                              "bad transport parameter")
    TPARAM_CHECK_INJECT_RAW_A(malformed_unknown_3,
                              "bad transport parameter")

    TPARAM_CHECK_INJECT_A(ACK_DELAY_EXP, excess_ack_delay_exp,
                          "ACK_DELAY_EXP is malformed")
    TPARAM_CHECK_INJECT_A(MAX_ACK_DELAY, excess_max_ack_delay,
                          "MAX_ACK_DELAY is malformed")
    TPARAM_CHECK_DROP_INJECT_A(INITIAL_MAX_STREAMS_BIDI, excess_initial_max_streams,
                               "INITIAL_MAX_STREAMS_BIDI is malformed")
    TPARAM_CHECK_DROP_INJECT_A(INITIAL_MAX_STREAMS_UNI, excess_initial_max_streams,
                               "INITIAL_MAX_STREAMS_UNI is malformed")

    TPARAM_CHECK_DROP_INJECT_A(MAX_UDP_PAYLOAD_SIZE, undersize_udp_payload_size,
                               "MAX_UDP_PAYLOAD_SIZE is malformed")
    TPARAM_CHECK_DROP_INJECT_A(ACTIVE_CONN_ID_LIMIT, undersize_active_conn_id_limit,
                               "ACTIVE_CONN_ID_LIMIT is malformed")

    TPARAM_CHECK_INJECT_TWICE_A(ACK_DELAY_EXP, ack_delay_exp,
                                "ACK_DELAY_EXP appears multiple times")
    TPARAM_CHECK_INJECT_TWICE_A(MAX_ACK_DELAY, ack_delay_exp,
                                "MAX_ACK_DELAY appears multiple times")
    TPARAM_CHECK_INJECT_TWICE_A(STATELESS_RESET_TOKEN, stateless_reset_token,
                                "STATELESS_RESET_TOKEN appears multiple times")
    TPARAM_CHECK_INJECT_TWICE_A(PREFERRED_ADDR, preferred_addr,
                                "PREFERRED_ADDR appears multiple times")

    TPARAM_CHECK_MUTATE(ORIG_DCID,
                        "ORIG_DCID does not match expected value")
    TPARAM_CHECK_MUTATE(INITIAL_SCID,
                        "INITIAL_SCID does not match expected value")

    TPARAM_CHECK_DROP_INJECT_A(ORIG_DCID, long_cid,
                               "ORIG_DCID is malformed")
    TPARAM_CHECK_DROP_INJECT_A(INITIAL_SCID, long_cid,
                               "INITIAL_SCID is malformed")

    TPARAM_CHECK_INT(INITIAL_MAX_DATA,
                     "INITIAL_MAX_DATA is malformed")
    TPARAM_CHECK_INT(INITIAL_MAX_STREAM_DATA_BIDI_LOCAL,
                     "INITIAL_MAX_STREAM_DATA_BIDI_LOCAL is malformed")
    TPARAM_CHECK_INT(INITIAL_MAX_STREAM_DATA_BIDI_REMOTE,
                     "INITIAL_MAX_STREAM_DATA_BIDI_REMOTE is malformed")
    TPARAM_CHECK_INT(INITIAL_MAX_STREAM_DATA_UNI,
                     "INITIAL_MAX_STREAM_DATA_UNI is malformed")
    TPARAM_CHECK_INT(ACK_DELAY_EXP,
                     "ACK_DELAY_EXP is malformed")
    TPARAM_CHECK_INT(MAX_ACK_DELAY,
                     "MAX_ACK_DELAY is malformed")
    TPARAM_CHECK_INT(INITIAL_MAX_STREAMS_BIDI,
                     "INITIAL_MAX_STREAMS_BIDI is malformed")
    TPARAM_CHECK_INT(INITIAL_MAX_STREAMS_UNI,
                     "INITIAL_MAX_STREAMS_UNI is malformed")
    TPARAM_CHECK_INT(MAX_IDLE_TIMEOUT,
                     "MAX_IDLE_TIMEOUT is malformed")
    TPARAM_CHECK_INT(MAX_UDP_PAYLOAD_SIZE,
                     "MAX_UDP_PAYLOAD_SIZE is malformed")
    TPARAM_CHECK_INT(ACTIVE_CONN_ID_LIMIT,
                     "ACTIVE_CONN_ID_LIMIT is malformed")
};

struct tparam_ctx {
    const struct tparam_test *t;
};

static int tparam_handle(struct tparam_ctx *ctx,
                         uint64_t id, unsigned char *data,
                         size_t data_len,
                         WPACKET *wpkt)
{
    const struct tparam_test *t = ctx->t;

    switch (t->op) {
    case TPARAM_OP_DUP:
        if (!TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(wpkt, id,
                                                                  data, data_len)))
            return 0;

        /*
         * If this is the matching ID, write it again, duplicating the TPARAM.
         */
        if (id == t->id
            && !TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(wpkt, id,
                                                                     data, data_len)))
            return 0;

        return 1;

    case TPARAM_OP_DROP:
    case TPARAM_OP_DROP_INJECT:
        /* Pass through unless ID matches. */
        if (id != t->id
            && !TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(wpkt, id,
                                                                     data, data_len)))
            return 0;

        return 1;

    case TPARAM_OP_INJECT:
    case TPARAM_OP_INJECT_TWICE:
    case TPARAM_OP_INJECT_RAW:
        /* Always pass through. */
        if (!TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(wpkt, id,
                                                                  data, data_len)))
            return 0;

        return 1;

    case TPARAM_OP_MUTATE:
        if (id == t->id) {
            if (!TEST_size_t_gt(data_len, 0))
                return 0;

            data[0] ^= 1;
        }

        if (!TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(wpkt, id,
                                                                  data, data_len)))
            return 0;

        if (id == t->id)
            data[0] ^= 1;

        return 1;

    default:
        return 0;
    }
}

static int tparam_on_enc_ext(QTEST_FAULT *qtf, QTEST_ENCRYPTED_EXTENSIONS *ee,
                             size_t ee_len, void *arg)
{
    int rc = 0;
    struct tparam_ctx *ctx = arg;
    PACKET pkt = {0};
    WPACKET wpkt;
    int have_wpkt = 0;
    BUF_MEM *old_bufm = NULL, *new_bufm = NULL;
    unsigned char *tp_p;
    size_t tp_len, written, old_len, eb_len;
    uint64_t id;

    if (!TEST_ptr(old_bufm = BUF_MEM_new()))
        goto err;

    /*
     * Delete transport parameters TLS extension and capture the contents of the
     * extension which was removed.
     */
    if (!TEST_true(qtest_fault_delete_extension(qtf, TLSEXT_TYPE_quic_transport_parameters,
                                                ee->extensions, &ee->extensionslen,
                                                old_bufm)))
        goto err;

    if (!TEST_true(PACKET_buf_init(&pkt, (unsigned char *)old_bufm->data, old_bufm->length))
        || !TEST_ptr(new_bufm = BUF_MEM_new())
        || !TEST_true(WPACKET_init(&wpkt, new_bufm)))
        goto err;

    have_wpkt = 1;

    /*
     * Open transport parameters TLS extension:
     *
     *   u16  Extension ID (quic_transport_parameters)
     *   u16  Extension Data Length
     *   ...  Extension Data
     *
     */
    if (!TEST_true(WPACKET_put_bytes_u16(&wpkt,
                                         TLSEXT_TYPE_quic_transport_parameters))
        || !TEST_true(WPACKET_start_sub_packet_u16(&wpkt)))
        goto err;

    for (; PACKET_remaining(&pkt) > 0; ) {
        tp_p = (unsigned char *)ossl_quic_wire_decode_transport_param_bytes(&pkt,
                                                                            &id,
                                                                            &tp_len);
        if (!TEST_ptr(tp_p)) {
            TEST_mem_eq(PACKET_data(&pkt), PACKET_remaining(&pkt), NULL, 0);
            goto err;
        }

        if (!TEST_true(tparam_handle(ctx, id, tp_p, tp_len, &wpkt)))
            goto err;
    }

    if (ctx->t->op == TPARAM_OP_INJECT || ctx->t->op == TPARAM_OP_DROP_INJECT
        || ctx->t->op == TPARAM_OP_INJECT_TWICE) {
        if (!TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(&wpkt, ctx->t->id,
                                                                  ctx->t->buf,
                                                                  ctx->t->buf_len)))
            goto err;

        if (ctx->t->op == TPARAM_OP_INJECT_TWICE
            && !TEST_ptr(ossl_quic_wire_encode_transport_param_bytes(&wpkt, ctx->t->id,
                                                                     ctx->t->buf,
                                                                     ctx->t->buf_len)))
            goto err;
    } else if (ctx->t->op == TPARAM_OP_INJECT_RAW) {
        if (!TEST_true(WPACKET_memcpy(&wpkt, ctx->t->buf, ctx->t->buf_len)))
            goto err;
    }

    if (!TEST_true(WPACKET_close(&wpkt))) /* end extension data, set length */
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    WPACKET_finish(&wpkt);
    have_wpkt = 0;

    /*
     * Append the constructed extension blob to the extension block.
     */
    old_len = ee->extensionslen;

    if (!qtest_fault_resize_message(qtf, ee->extensionslen + written))
        goto err;

    memcpy(ee->extensions + old_len, new_bufm->data, written);

    /* Fixup the extension block header (u16 length of entire block). */
    eb_len = (((uint16_t)ee->extensions[0]) << 8) + (uint16_t)ee->extensions[1];
    eb_len += written;
    ee->extensions[0] = (unsigned char)((eb_len >> 8) & 0xFF);
    ee->extensions[1] = (unsigned char)( eb_len       & 0xFF);

    rc = 1;
err:
    if (have_wpkt)
        WPACKET_cleanup(&wpkt);
    BUF_MEM_free(old_bufm);
    BUF_MEM_free(new_bufm);
    return rc;
}

static int test_tparam(int idx)
{
    int testresult = 0;
    SSL_CTX *c_ctx = NULL;
    SSL *c_ssl = NULL;
    QUIC_TSERVER *s = NULL;
    QTEST_FAULT *qtf = NULL;
    struct tparam_ctx ctx = {0};

    ctx.t = &tparam_tests[idx];

    if (!TEST_ptr(c_ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method())))
        goto err;

    if (!TEST_true(qtest_create_quic_objects(libctx, c_ctx, NULL, cert,
                                             privkey, 0, &s,
                                             &c_ssl, &qtf, NULL)))
        goto err;

    if (!TEST_true(qtest_fault_set_hand_enc_ext_listener(qtf, tparam_on_enc_ext,
                                                         &ctx)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection_ex(s, c_ssl,
                                                   ctx.t->expect_fail != NULL)))
        goto err;

    if (ctx.t->expect_fail != NULL) {
        SSL_CONN_CLOSE_INFO info = {0};

        if (!TEST_true(SSL_get_conn_close_info(c_ssl, &info, sizeof(info))))
            goto err;

        if (!TEST_true((info.flags & SSL_CONN_CLOSE_FLAG_TRANSPORT) != 0)
            || !TEST_uint64_t_eq(info.error_code, OSSL_QUIC_ERR_TRANSPORT_PARAMETER_ERROR)
            || !TEST_ptr(strstr(info.reason, ctx.t->expect_fail))) {
            TEST_error("expected connection closure information mismatch"
                       " during TPARAM test: flags=%llu ec=%llu reason='%s'",
                       (unsigned long long)info.flags,
                       (unsigned long long)info.error_code,
                       info.reason);
            goto err;
        }
    }

    testresult = 1;
err:
    if (!testresult) {
        if (ctx.t->expect_fail != NULL)
            TEST_info("failed during test for id=%llu, op=%d, bl=%zu, "
                      "expected failure='%s'", (unsigned long long)ctx.t->id,
                      ctx.t->op, ctx.t->buf_len, ctx.t->expect_fail);
        else
            TEST_info("failed during test for id=%llu, op=%d, bl=%zu",
                      (unsigned long long)ctx.t->id, ctx.t->op, ctx.t->buf_len);
    }

    ossl_quic_tserver_free(s);
    SSL_free(c_ssl);
    SSL_CTX_free(c_ctx);
    qtest_fault_free(qtf);
    return testresult;
}

static int new_called = 0;
static SSL *cbssl = NULL;

static int new_session_cb(SSL *ssl, SSL_SESSION *sess)
{
    new_called++;
    /*
     * Remember the SSL ref we were called with. No need to up-ref this. It
     * should remain valid for the duration of the test.
     */
    cbssl = ssl;
    /*
     * sess has been up-refed for us, but we don't actually need it so free it
     * immediately.
     */
    SSL_SESSION_free(sess);
    return 1;
}

/* Test using a new_session_cb with a QUIC SSL object works as expected */
static int test_session_cb(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;

    if (!TEST_ptr(cctx))
        goto err;

    new_called = 0;
    cbssl = NULL;
    SSL_CTX_sess_set_new_cb(cctx, new_session_cb);
    SSL_CTX_set_session_cache_mode(cctx, SSL_SESS_CACHE_CLIENT);

    if (!TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                             privkey,
                                             QTEST_FLAG_FAKE_TIME,
                                             &qtserv, &clientquic,
                                             NULL, NULL)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    /* Process the pending NewSessionTickets */
    if (!TEST_true(SSL_handle_events(clientquic)))
        goto err;

    if (!TEST_int_eq(SSL_shutdown(clientquic), 0))
        goto err;

    /*
     * Check the callback was called twice (we expect 2 tickets), and with the
     * correct SSL reference
     */
    if (!TEST_int_eq(new_called, 2)
            || !TEST_ptr_eq(clientquic, cbssl))
        goto err;

    testresult = 1;
 err:
    cbssl = NULL;
    ossl_quic_tserver_free(qtserv);
    SSL_free(clientquic);
    SSL_CTX_free(cctx);

    return testresult;
}

static int test_domain_flags(void)
{
    int testresult = 0;
    SSL_CTX *ctx = NULL;
    SSL *domain = NULL, *listener = NULL, *other_conn = NULL;
    uint64_t domain_flags = 0;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method()))
        || !TEST_true(SSL_CTX_get_domain_flags(ctx, &domain_flags))
        || !TEST_uint64_t_ne(domain_flags, 0)
        || !TEST_uint64_t_ne(domain_flags & (SSL_DOMAIN_FLAG_SINGLE_THREAD
                                             | SSL_DOMAIN_FLAG_MULTI_THREAD), 0)
        || !TEST_uint64_t_ne(domain_flags & SSL_DOMAIN_FLAG_LEGACY_BLOCKING, 0)
        || !TEST_true(SSL_CTX_set_domain_flags(ctx, SSL_DOMAIN_FLAG_SINGLE_THREAD))
        || !TEST_true(SSL_CTX_get_domain_flags(ctx, &domain_flags))
        || !TEST_uint64_t_eq(domain_flags, SSL_DOMAIN_FLAG_SINGLE_THREAD)
        || !TEST_ptr(domain = SSL_new_domain(ctx, 0))
        || !TEST_true(SSL_get_domain_flags(domain, &domain_flags))
        || !TEST_uint64_t_eq(domain_flags, SSL_DOMAIN_FLAG_SINGLE_THREAD)
        || !TEST_true(other_conn = SSL_new(ctx))
        || !TEST_true(SSL_get_domain_flags(other_conn, &domain_flags))
        || !TEST_uint64_t_eq(domain_flags, SSL_DOMAIN_FLAG_SINGLE_THREAD)
        || !TEST_true(SSL_is_domain(domain))
        || !TEST_false(SSL_is_domain(other_conn))
        || !TEST_ptr_eq(SSL_get0_domain(domain), domain)
        || !TEST_ptr_null(SSL_get0_domain(other_conn))
        || !TEST_ptr(listener = SSL_new_listener_from(domain, 0))
        || !TEST_true(SSL_is_listener(listener))
        || !TEST_false(SSL_is_domain(listener))
        || !TEST_ptr_eq(SSL_get0_domain(listener), domain)
        || !TEST_ptr_eq(SSL_get0_listener(listener), listener))
        goto err;

    testresult = 1;
err:
    SSL_free(domain);
    SSL_free(listener);
    SSL_free(other_conn);
    SSL_CTX_free(ctx);
    return testresult;
}

/*
 * Test that calling SSL_handle_events() early behaves as expected
 */
static int test_early_ticks(void)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method());
    SSL *clientquic = NULL;
    QUIC_TSERVER *qtserv = NULL;
    int testresult = 0;
    struct timeval tv;
    int inf = 0;

    if (!TEST_ptr(cctx)
            || !TEST_true(qtest_create_quic_objects(libctx, cctx, NULL, cert,
                                                    privkey, QTEST_FLAG_FAKE_TIME,
                                                    &qtserv,
                                                    &clientquic, NULL, NULL)))
        goto err;

    if (!TEST_true(SSL_in_before(clientquic)))
        goto err;

    if (!TEST_true(SSL_handle_events(clientquic)))
        goto err;

    if (!TEST_true(SSL_get_event_timeout(clientquic, &tv, &inf))
            || !TEST_true(inf))
        goto err;

    if (!TEST_false(SSL_has_pending(clientquic))
            || !TEST_int_eq(SSL_pending(clientquic), 0))
        goto err;

    if (!TEST_true(SSL_in_before(clientquic)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, clientquic)))
        goto err;

    if (!TEST_false(SSL_in_before(clientquic)))
        goto err;

    testresult = 1;
 err:
    SSL_free(clientquic);
    SSL_CTX_free(cctx);
    ossl_quic_tserver_free(qtserv);
    return testresult;
}

static int select_alpn(SSL *ssl, const unsigned char **out,
                       unsigned char *out_len, const unsigned char *in,
                       unsigned int in_len, void *arg)
{
    static unsigned char alpn[] = { 8, 'o', 's', 's', 'l', 't', 'e', 's', 't' };

    if (SSL_select_next_proto((unsigned char **)out, out_len, alpn, sizeof(alpn),
                              in, in_len) == OPENSSL_NPN_NEGOTIATED)
        return SSL_TLSEXT_ERR_OK;
    return SSL_TLSEXT_ERR_ALERT_FATAL;
}

static SSL_CTX *create_client_ctx(void)
{
    SSL_CTX *ssl_ctx;

    if (!TEST_ptr(ssl_ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_client_method()))) {
        SSL_CTX_free(ssl_ctx);
        ssl_ctx = NULL;
    }

    return ssl_ctx;
}

static SSL_CTX *create_server_ctx(void)
{
    SSL_CTX *ssl_ctx;

    if (!TEST_ptr(ssl_ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_server_method()))
        || !TEST_true(SSL_CTX_use_certificate_file(ssl_ctx, cert, SSL_FILETYPE_PEM))
        || !TEST_true(SSL_CTX_use_PrivateKey_file(ssl_ctx, privkey, SSL_FILETYPE_PEM))) {
        SSL_CTX_free(ssl_ctx);
        ssl_ctx = NULL;
    } else {
        SSL_CTX_set_alpn_select_cb(ssl_ctx, select_alpn, NULL);
        SSL_CTX_set_verify(ssl_ctx, SSL_VERIFY_NONE, NULL);
    }

    return ssl_ctx;
}

static BIO_ADDR *create_addr(struct in_addr *ina, short int port)
{
    BIO_ADDR *addr = NULL;

    if (!TEST_ptr(addr = BIO_ADDR_new()))
        return NULL;

    if (!TEST_true(BIO_ADDR_rawmake(addr, AF_INET, ina, sizeof(struct in_addr),
                                    htons(port)))) {
        BIO_ADDR_free(addr);
        return NULL;
    }

    return addr;
}

static int bio_addr_bind(BIO *bio, BIO_ADDR *addr)
{
    int bio_caps = BIO_DGRAM_CAP_HANDLES_DST_ADDR | BIO_DGRAM_CAP_HANDLES_SRC_ADDR;

    if (!TEST_true(BIO_dgram_set_caps(bio, bio_caps)))
        return 0;

    if (!TEST_int_eq(BIO_dgram_set0_local_addr(bio, addr), 1))
        return 0;

    return 1;
}

static SSL *ql_create(SSL_CTX *ssl_ctx, BIO *bio)
{
    SSL *qserver;

    if (!TEST_ptr(qserver = SSL_new_listener(ssl_ctx, 0))) {
        BIO_free(bio);
        return NULL;
    }

    SSL_set_bio(qserver, bio, bio);

    if (!TEST_true(SSL_listen(qserver))) {
        SSL_free(qserver);
        return NULL;
    }

    return qserver;
}

static int qc_init(SSL *qconn, BIO_ADDR *dst_addr)
{
    static unsigned char alpn[] = { 8, 'o', 's', 's', 'l', 't', 'e', 's', 't' };

    if (!TEST_true(SSL_set1_initial_peer_addr(qconn, dst_addr)))
        return 0;

    if (!TEST_false(SSL_set_alpn_protos(qconn, alpn, sizeof(alpn))))
        return 0;

    return 1;
}

static int test_ssl_new_from_listener(void)
{
    SSL_CTX *lctx = NULL, *sctx = NULL;
    SSL *qlistener = NULL, *qserver = NULL, *qconn = 0;
    int testresult = 0;
    int chk;
    BIO *lbio = NULL, *sbio = NULL;
    BIO_ADDR *addr = NULL;
    struct in_addr ina;

    ina.s_addr = htonl(0x1f000001);
    if (!TEST_ptr(lctx = create_server_ctx())
        || !TEST_ptr(sctx = create_server_ctx())
        || !TEST_true(BIO_new_bio_dgram_pair(&lbio, 0, &sbio, 0)))
        goto err;

    if (!TEST_ptr(addr = create_addr(&ina, 8040)))
        goto err;

    if (!TEST_true(bio_addr_bind(lbio, addr)))
        goto err;
    addr = NULL;

    if (!TEST_ptr(addr = create_addr(&ina, 4080)))
        goto err;

    if (!TEST_true(bio_addr_bind(sbio, addr)))
        goto err;
    addr = NULL;

    qlistener = ql_create(lctx, lbio);
    lbio = NULL;
    if (!TEST_ptr(qlistener))
        goto err;

    qserver = ql_create(sctx, sbio);
    sbio = NULL;
    if (!TEST_ptr(qserver))
        goto err;

    if (!TEST_ptr(qconn = SSL_new_from_listener(qlistener, 0)))
        goto err;

    if (!TEST_ptr(addr = create_addr(&ina, 4080)))
        goto err;

    chk = qc_init(qconn, addr);
    if (!TEST_true(chk))
        goto err;

    while ((chk = SSL_do_handshake(qconn)) == -1) {
        SSL_handle_events(qserver);
        SSL_handle_events(qlistener);
    }

    if (!TEST_int_gt(chk, 0)) {
        TEST_info("SSL_do_handshake() failed\n");
        goto err;
    }

    testresult = 1;
 err:
    SSL_free(qconn);
    SSL_free(qlistener);
    SSL_free(qserver);
    BIO_free(lbio);
    BIO_free(sbio);
    SSL_CTX_free(sctx);
    SSL_CTX_free(lctx);
    BIO_ADDR_free(addr);

    return testresult;
}

static int test_server_method_with_ssl_new(void)
{
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    int ret = 0;
    unsigned long err;

    /* Create a new SSL_CTX using the QUIC server method */
    ctx = SSL_CTX_new_ex(libctx, NULL, OSSL_QUIC_server_method());
    if (!TEST_ptr(ctx))
        goto end;

    /* Try to create a new SSL object - this should fail */
    ssl = SSL_new(ctx);

    /* Check that SSL_new() returned NULL */
    if (!TEST_ptr_null(ssl))
        goto end;

    /* Check for the expected error */
    err = ERR_peek_error();
    if (!TEST_true(ERR_GET_LIB(err) == ERR_LIB_SSL &&
                   ERR_GET_REASON(err) == ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED))
        goto end;

    ret = 1;

end:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return ret;
}

static int create_quic_ssl_objects(SSL_CTX *sctx, SSL_CTX *cctx,
                                   SSL **lssl, SSL **cssl)
{
    BIO_ADDR *addr = NULL;
    struct in_addr ina;
    BIO *cbio = NULL, *sbio = NULL;
    int ret = 0;

    *cssl = *lssl = NULL;
    ina.s_addr = htonl(0x1f000001);

    if (!TEST_true(BIO_new_bio_dgram_pair(&cbio, 0, &sbio, 0)))
        goto err;

    if (!TEST_ptr(addr = create_addr(&ina, 8040)))
        goto err;

    if (!TEST_true(bio_addr_bind(sbio, addr)))
        goto err;
    addr = NULL;

    *lssl = ql_create(sctx, sbio);
    sbio = NULL;
    if (!TEST_ptr(*lssl))
        goto err;

    if (!TEST_ptr(*cssl = SSL_new(cctx)))
        goto err;

    if (!TEST_ptr(addr = create_addr(&ina, 8040)))
        goto err;
    if (!TEST_true(bio_addr_bind(cbio, addr)))
        goto err;

    if (!TEST_true(qc_init(*cssl, addr))) {
        addr = NULL;
        goto err;
    }
    addr = NULL;
    SSL_set_bio(*cssl, cbio, cbio);
    cbio = NULL;

    ret = 1;

 err:
    if (!ret) {
        SSL_free(*cssl);
        SSL_free(*lssl);
        *cssl = *lssl = NULL;
    }
    BIO_free(cbio);
    BIO_free(sbio);
    BIO_ADDR_free(addr);

    return ret;
}

static int test_ssl_accept_connection(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL, *qlistener = NULL;
    int testresult = 0;
    int ret, i;

    if (!TEST_ptr(sctx = create_server_ctx())
        || !TEST_ptr(cctx = create_client_ctx()))
        goto err;

    if (!create_quic_ssl_objects(sctx, cctx, &qlistener, &clientssl))
        goto err;

    /* Calling SSL_accept() on a listener is expected to fail */
    ret = SSL_accept(qlistener);
    if (!TEST_int_le(ret, 0)
        || !TEST_int_eq(SSL_get_error(qlistener, ret), SSL_ERROR_SSL))
        goto err;

    /* Send ClientHello and server retry */
    for (i = 0; i < 2; i++) {
        ret = SSL_connect(clientssl);
        if (!TEST_int_le(ret, 0)
            || !TEST_int_eq(SSL_get_error(clientssl, ret), SSL_ERROR_WANT_READ))
            goto err;
        SSL_handle_events(qlistener);
    }

    /* We expect a server SSL object which has not yet completed its handshake */
    serverssl = SSL_accept_connection(qlistener, 0);
    if (!TEST_ptr(serverssl) || !TEST_false(SSL_is_init_finished(serverssl)))
        goto err;

    /* Call SSL_accept() and SSL_connect() until we are connected */
    if (!TEST_true(create_bare_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE, 0, 0)))
        goto err;

    testresult = 1;

 err:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_free(qlistener);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static SSL *quic_verify_ssl = NULL;

static int quic_verify_cb(int ok, X509_STORE_CTX *ctx)
{
    SSL *cssl = (SSL *)X509_STORE_CTX_get_ex_data(ctx, SSL_get_ex_data_X509_STORE_CTX_idx());

    /* Confirm we got the SSL object we were expecting */
    return TEST_ptr_eq(cssl, quic_verify_ssl);
}

static int test_ssl_set_verify(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL, *qlistener = NULL;
    int testresult = 0;
    int ret, i;

    if (!TEST_ptr(sctx = create_server_ctx())
        || !TEST_ptr(cctx = create_client_ctx()))
        goto err;

    if (!create_quic_ssl_objects(sctx, cctx, &qlistener, &clientssl))
        goto err;

    quic_verify_ssl = clientssl;
    SSL_set_verify(clientssl, SSL_VERIFY_PEER, quic_verify_cb);

    /* Send ClientHello and server retry */
    for (i = 0; i < 2; i++) {
        ret = SSL_connect(clientssl);
        if (!TEST_int_le(ret, 0)
            || !TEST_int_eq(SSL_get_error(clientssl, ret), SSL_ERROR_WANT_READ))
            goto err;
        SSL_handle_events(qlistener);
    }

    /* We expect a server SSL object which has not yet completed its handshake */
    serverssl = SSL_accept_connection(qlistener, 0);

    /* Call SSL_accept() and SSL_connect() until we are connected */
    if (!TEST_true(create_bare_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE, 0, 0)))
        goto err;

    testresult = 1;

 err:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_free(qlistener);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/***********************************************************************************/
OPT_TEST_DECLARE_USAGE("provider config certsdir datadir\n")

int setup_tests(void)
{
    char *modulename;
    char *configfile;

    libctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(libctx))
        return 0;

    defctxnull = OSSL_PROVIDER_load(NULL, "null");

    /*
     * Verify that the default and fips providers in the default libctx are not
     * available
     */
    if (!TEST_false(OSSL_PROVIDER_available(NULL, "default"))
            || !TEST_false(OSSL_PROVIDER_available(NULL, "fips")))
        goto err;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        goto err;
    }

    if (!TEST_ptr(modulename = test_get_argument(0))
            || !TEST_ptr(configfile = test_get_argument(1))
            || !TEST_ptr(certsdir = test_get_argument(2))
            || !TEST_ptr(datadir = test_get_argument(3)))
        goto err;

    if (!TEST_true(OSSL_LIB_CTX_load_config(libctx, configfile)))
        goto err;

    /* Check we have the expected provider available */
    if (!TEST_true(OSSL_PROVIDER_available(libctx, modulename)))
        goto err;

    /* Check the default provider is not available */
    if (strcmp(modulename, "default") != 0
            && !TEST_false(OSSL_PROVIDER_available(libctx, "default")))
        goto err;

    if (strcmp(modulename, "fips") == 0)
        is_fips = 1;

    cert = test_mk_file_path(certsdir, "servercert.pem");
    if (cert == NULL)
        goto err;

    ccert = test_mk_file_path(certsdir, "ee-client-chain.pem");
    if (ccert == NULL)
        goto err;

    cauthca = test_mk_file_path(certsdir, "root-cert.pem");
    if (cauthca == NULL)
        goto err;

    privkey = test_mk_file_path(certsdir, "serverkey.pem");
    if (privkey == NULL)
        goto err;

    cprivkey = test_mk_file_path(certsdir, "ee-key.pem");
    if (privkey == NULL)
        goto err;

    ADD_ALL_TESTS(test_quic_write_read, 3);
    ADD_TEST(test_fin_only_blocking);
    ADD_TEST(test_ciphersuites);
    ADD_TEST(test_cipher_find);
    ADD_TEST(test_version);
#if defined(DO_SSL_TRACE_TEST)
    ADD_TEST(test_ssl_trace);
#endif
    ADD_TEST(test_quic_forbidden_apis_ctx);
    ADD_TEST(test_quic_forbidden_apis);
    ADD_TEST(test_quic_forbidden_options);
    ADD_ALL_TESTS(test_quic_set_fd, 3);
    ADD_TEST(test_bio_ssl);
    ADD_TEST(test_back_pressure);
    ADD_TEST(test_multiple_dgrams);
    ADD_ALL_TESTS(test_non_io_retry, 2);
    ADD_TEST(test_quic_psk);
    ADD_ALL_TESTS(test_client_auth, 3);
    ADD_ALL_TESTS(test_alpn, 2);
    ADD_ALL_TESTS(test_noisy_dgram, 2);
    ADD_TEST(test_bw_limit);
    ADD_TEST(test_get_shutdown);
    ADD_ALL_TESTS(test_tparam, OSSL_NELEM(tparam_tests));
    ADD_TEST(test_session_cb);
    ADD_TEST(test_domain_flags);
    ADD_TEST(test_early_ticks);
    ADD_TEST(test_ssl_new_from_listener);
#ifndef OPENSSL_NO_SSL_TRACE
    ADD_TEST(test_new_token);
#endif
    ADD_TEST(test_server_method_with_ssl_new);
    ADD_TEST(test_ssl_accept_connection);
    ADD_TEST(test_ssl_set_verify);
    return 1;
 err:
    cleanup_tests();
    return 0;
}

void cleanup_tests(void)
{
    bio_f_noisy_dgram_filter_free();
    bio_f_pkt_split_dgram_filter_free();
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    OPENSSL_free(ccert);
    OPENSSL_free(cauthca);
    OPENSSL_free(cprivkey);
    OSSL_PROVIDER_unload(defctxnull);
    OSSL_LIB_CTX_free(libctx);
}
