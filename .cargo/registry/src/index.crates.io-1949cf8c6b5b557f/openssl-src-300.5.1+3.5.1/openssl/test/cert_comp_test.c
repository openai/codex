/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * We need access to the deprecated low level HMAC APIs for legacy purposes
 * when the deprecated calls are not hidden
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
# define OPENSSL_SUPPRESS_DEPRECATED
#endif

#include <openssl/ssl.h>
#include "internal/nelem.h"
#include "internal/ssl_unwrap.h"
#include "helpers/ssltestlib.h"
#include "testutil.h"
#include "../ssl/ssl_local.h"

#undef OSSL_NO_USABLE_TLS1_3
#if defined(OPENSSL_NO_TLS1_3) \
    || (defined(OPENSSL_NO_EC) && defined(OPENSSL_NO_DH))
/*
 * If we don't have ec or dh then there are no built-in groups that are usable
 * with TLSv1.3
 */
# define OSSL_NO_USABLE_TLS1_3
#endif

#if !defined(OSSL_NO_USEABLE_TLS1_3)

static char *certsdir = NULL;
static char *cert = NULL;
static char *privkey = NULL;

static int client_cert_cb(SSL *ssl, X509 **x509, EVP_PKEY **pkey)
{
    X509 *xcert;
    EVP_PKEY *privpkey;
    BIO *in = NULL;
    BIO *priv_in = NULL;

    /* Check that SSL_get0_peer_certificate() returns something sensible */
    if (!TEST_ptr(SSL_get0_peer_certificate(ssl)))
        return 0;

    in = BIO_new_file(cert, "r");
    if (!TEST_ptr(in))
        return 0;

    if (!TEST_ptr(xcert = X509_new_ex(NULL, NULL))
            || !TEST_ptr(PEM_read_bio_X509(in, &xcert, NULL, NULL))
            || !TEST_ptr(priv_in = BIO_new_file(privkey, "r"))
            || !TEST_ptr(privpkey = PEM_read_bio_PrivateKey_ex(priv_in, NULL,
                                                               NULL, NULL,
                                                               NULL, NULL)))
        goto err;

    *x509 = xcert;
    *pkey = privpkey;

    BIO_free(in);
    BIO_free(priv_in);
    return 1;
err:
    X509_free(xcert);
    BIO_free(in);
    BIO_free(priv_in);
    return 0;
}

static int verify_cb(int preverify_ok, X509_STORE_CTX *x509_ctx)
{
    return 1;
}

/*
 * Test 0 = app pre-compresses certificate in SSL
 * Test 1 = app pre-compresses certificate in SSL_CTX
 * Test 2 = app pre-compresses certificate in SSL_CTX, client authentication
 * Test 3 = app pre-compresses certificate in SSL_CTX, but it's unused due to prefs
 */
/* Compression helper */
static int ssl_comp_cert(SSL *ssl, int alg)
{
    unsigned char *comp_data = NULL;
    size_t comp_len = 0;
    size_t orig_len = 0;
    int retval = 0;

    if (!TEST_size_t_gt(comp_len = SSL_get1_compressed_cert(ssl, alg, &comp_data, &orig_len), 0))
        goto err;

    if (!TEST_true(SSL_set1_compressed_cert(ssl, alg, comp_data, comp_len, orig_len)))
        goto err;
    retval = alg;

 err:
    OPENSSL_free(comp_data);
    return retval;
}

static void cert_comp_info_cb(const SSL *s, int where, int ret)
{
    int *seen = (int*)SSL_get_app_data(s);

    if (SSL_is_server(s)) {
        /* TLS_ST_SR_COMP_CERT */
        if (!strcmp(SSL_state_string(s), "TRCCC") && seen != NULL)
            *seen = 1;
    } else {
        /* TLS_ST_CR_COMP_CERT */
        if (!strcmp(SSL_state_string(s), "TRSCC") && seen != NULL)
            *seen = 1;
    }
}

static int test_ssl_cert_comp(int test)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    int expected_client = TLSEXT_comp_cert_none;
    int expected_server = TLSEXT_comp_cert_none;
    int client_seen = 0;
    int server_seen = 0;
    /* reverse default order */
    int server_pref[] = { TLSEXT_comp_cert_zstd, TLSEXT_comp_cert_zlib, TLSEXT_comp_cert_brotli };
    /* default order */
    int client_pref[] = { TLSEXT_comp_cert_brotli, TLSEXT_comp_cert_zlib, TLSEXT_comp_cert_zstd };

    /* one of these *must* be defined! */
#ifndef OPENSSL_NO_BROTLI
    expected_server = TLSEXT_comp_cert_brotli;
    expected_client = TLSEXT_comp_cert_brotli;
#endif
#ifndef OPENSSL_NO_ZLIB
    expected_server = TLSEXT_comp_cert_zlib;
    if (expected_client == TLSEXT_comp_cert_none)
        expected_client = TLSEXT_comp_cert_zlib;
#endif
#ifndef OPENSSL_NO_ZSTD
    expected_server = TLSEXT_comp_cert_zstd;
    if (expected_client == TLSEXT_comp_cert_none)
        expected_client = TLSEXT_comp_cert_zstd;
#endif
    /*
     * If there's only one comp algorithm, pref won't do much
     * Coverity can get confused in this case, and consider test == 3
     * to be DEADCODE
     */
    if (test == 3 && expected_client == expected_server) {
        TEST_info("Only one compression algorithm configured");
        return 1;
    }

    if (!TEST_true(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;
    if (test == 3) {
        /* coverity[deadcode] */
        server_pref[0] = expected_server;
        server_pref[1] = expected_client;
        if (!TEST_true(SSL_CTX_set1_cert_comp_preference(sctx, server_pref, 2)))
            goto end;
        client_pref[0] = expected_client;
        if (!TEST_true(SSL_CTX_set1_cert_comp_preference(cctx, client_pref, 1)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_set1_cert_comp_preference(sctx, server_pref, OSSL_NELEM(server_pref))))
            goto end;
        if (!TEST_true(SSL_CTX_set1_cert_comp_preference(cctx, client_pref, OSSL_NELEM(client_pref))))
            goto end;
    }
    if (test == 2) {
        /* Use callbacks from test_client_cert_cb() */
        SSL_CTX_set_client_cert_cb(cctx, client_cert_cb);
        SSL_CTX_set_verify(sctx, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, verify_cb);
    }

    if (test == 1 || test== 2 || test == 3) {
        if (!TEST_true(SSL_CTX_compress_certs(sctx, expected_server)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_app_data(clientssl, &client_seen)))
        goto end;
    if (!TEST_true(SSL_set_app_data(serverssl, &server_seen)))
        goto end;
    SSL_set_info_callback(clientssl, cert_comp_info_cb);
    SSL_set_info_callback(serverssl, cert_comp_info_cb);

    if (test == 0) {
        if (!TEST_int_eq(ssl_comp_cert(serverssl, expected_server), expected_server))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;
    if (test == 3) {
        /* coverity[deadcode] */
        SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(serverssl);

        /* expect that the pre-compressed cert won't be used */
        if (!TEST_int_eq(sc->cert->key->cert_comp_used, 0))
            goto end;

        if (!TEST_false(*(int*)SSL_get_app_data(clientssl)))
            goto end;
    } else {
        SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(serverssl);

        if (!TEST_int_gt(sc->cert->key->cert_comp_used, 0))
            goto end;

        if (!TEST_true(*(int*)SSL_get_app_data(clientssl)))
            goto end;
    }

    if (test == 2) {
        /* Only for client auth */
        if (!TEST_true(*(int*)SSL_get_app_data(serverssl)))
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
#endif

OPT_TEST_DECLARE_USAGE("certdir\n")

int setup_tests(void)
{
#if !defined(OSSL_NO_USEABLE_TLS1_3)
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(certsdir = test_get_argument(0)))
        return 0;

    cert = test_mk_file_path(certsdir, "servercert.pem");
    if (cert == NULL)
        goto err;

    privkey = test_mk_file_path(certsdir, "serverkey.pem");
    if (privkey == NULL)
        goto err;

    ADD_ALL_TESTS(test_ssl_cert_comp, 4);
    return 1;

 err:
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    return 0;
#else
    return 1;
#endif
}

void cleanup_tests(void)
{
#if !defined(OSSL_NO_USEABLE_TLS1_3)
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
#endif
}
