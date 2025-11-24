/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/ssl.h>

#include "helpers/ssltestlib.h"
#include "internal/dane.h"
#include "testutil.h"

#undef OSSL_NO_USABLE_TLS1_3
#if defined(OPENSSL_NO_TLS1_3) \
    || (defined(OPENSSL_NO_EC) && defined(OPENSSL_NO_DH))
/*
 * If we don't have ec or dh then there are no built-in groups that are usable
 * with TLSv1.3
 */
# define OSSL_NO_USABLE_TLS1_3
#endif

static char *certsdir = NULL;
static char *rootcert = NULL;
static char *cert = NULL;
static char *privkey = NULL;
static char *cert2 = NULL;
static char *privkey2 = NULL;
static char *cert448 = NULL;
static char *privkey448 = NULL;
static char *cert25519 = NULL;
static char *privkey25519 = NULL;
static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *defctxnull = NULL;

static const unsigned char cert_type_rpk[] = { TLSEXT_cert_type_rpk, TLSEXT_cert_type_x509 };
static const unsigned char SID_CTX[] = { 'r', 'p', 'k' };

static int rpk_verify_client_cb(int ok, X509_STORE_CTX *ctx)
{
    int err = X509_STORE_CTX_get_error(ctx);

    if (X509_STORE_CTX_get0_rpk(ctx) != NULL) {
        if (err != X509_V_OK) {
            TEST_info("rpk_verify_client_cb: ok=%d err=%d", ok, err);
            return 0;
        }
    }
    return 1;
}
static int rpk_verify_server_cb(int ok, X509_STORE_CTX *ctx)
{
    int err = X509_STORE_CTX_get_error(ctx);

    if (X509_STORE_CTX_get0_rpk(ctx) != NULL) {
        if (err != X509_V_OK) {
            TEST_info("rpk_verify_server_cb: ok=%d err=%d", ok, err);
            return 0;
        }
    }
    return 1;
}

/*
 * Test dimensions:
 *   (2) server_cert_type RPK off/on for server
 *   (2) client_cert_type RPK off/on for server
 *   (2) server_cert_type RPK off/on for client
 *   (2) client_cert_type RPK off/on for client
 *   (4) RSA vs ECDSA vs Ed25519 vs Ed448 certificates
 *   (2) TLSv1.2 vs TLSv1.3
 *
 * Tests:
 * idx = 0 - is the normal success case, certificate, single peer key
 * idx = 1 - only a private key
 * idx = 2 - add client authentication
 * idx = 3 - add second peer key (rootcert.pem)
 * idx = 4 - add second peer key (different, RSA or ECDSA)
 * idx = 5 - reverse peer keys (rootcert.pem, different order)
 * idx = 6 - reverse peer keys (RSA or ECDSA, different order)
 * idx = 7 - expects failure due to mismatched key (RSA or ECDSA)
 * idx = 8 - expects failure due to no configured key on client
 * idx = 9 - add client authentication (PHA)
 * idx = 10 - add client authentication (privake key only)
 * idx = 11 - simple resumption
 * idx = 12 - simple resumption, no ticket
 * idx = 13 - resumption with client authentication
 * idx = 14 - resumption with client authentication, no ticket
 * idx = 15 - like 0, but use non-default libctx
 * idx = 16 - like 7, but with SSL_VERIFY_PEER connection should fail
 * idx = 17 - like 8, but with SSL_VERIFY_PEER connection should fail
 *
 * 18 * 2 * 4 * 2 * 2 * 2 * 2 = 2304 tests
 */
static int test_rpk(int idx)
{
# define RPK_TESTS 18
# define RPK_DIMS (2 * 4 * 2 * 2 * 2 * 2)
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    EVP_PKEY *pkey = NULL, *other_pkey = NULL, *root_pkey = NULL;
    X509 *x509 = NULL, *other_x509 = NULL, *root_x509 = NULL;
    int testresult = 0, ret, expected = 1;
    int client_expected = X509_V_OK;
    int verify;
    int tls_version;
    char *cert_file = NULL;
    char *privkey_file = NULL;
    char *other_cert_file = NULL;
    SSL_SESSION *client_sess = NULL;
    SSL_SESSION *server_sess = NULL;
    int idx_server_server_rpk, idx_server_client_rpk;
    int idx_client_server_rpk, idx_client_client_rpk;
    int idx_cert, idx_prot;
    int client_auth = 0;
    int resumption = 0;
    int want_error = SSL_ERROR_NONE;
    long server_verify_result = 0;
    long client_verify_result = 0;
    OSSL_LIB_CTX *test_libctx = NULL;

    if (!TEST_int_le(idx, RPK_TESTS * RPK_DIMS))
        return 0;

    idx_server_server_rpk = idx / (RPK_TESTS * 2 * 4 * 2 * 2 * 2);
    idx %= RPK_TESTS * 2 * 4 * 2 * 2 * 2;
    idx_server_client_rpk = idx / (RPK_TESTS * 2 * 4 * 2 * 2);
    idx %= RPK_TESTS * 2 * 4 * 2 * 2;
    idx_client_server_rpk = idx / (RPK_TESTS * 2 * 4 * 2);
    idx %= RPK_TESTS * 2 * 4 * 2;
    idx_client_client_rpk = idx / (RPK_TESTS * 2 * 4);
    idx %= RPK_TESTS * 2 * 4;
    idx_cert = idx / (RPK_TESTS * 2);
    idx %= RPK_TESTS * 2;
    idx_prot = idx / RPK_TESTS;
    idx %= RPK_TESTS;

    /* Load "root" cert/pubkey */
    root_x509 = load_cert_pem(rootcert, NULL);
    if (!TEST_ptr(root_x509))
        goto end;
    root_pkey = X509_get0_pubkey(root_x509);
    if (!TEST_ptr(root_pkey))
        goto end;

    switch (idx_cert) {
        case 0:
            /* use RSA */
            cert_file = cert;
            privkey_file = privkey;
            other_cert_file = cert2;
            break;
#ifndef OPENSSL_NO_ECDSA
        case 1:
            /* use ECDSA */
            cert_file = cert2;
            privkey_file = privkey2;
            other_cert_file = cert;
            break;
# ifndef OPENSSL_NO_ECX
        case 2:
            /* use Ed448 */
            cert_file = cert448;
            privkey_file = privkey448;
            other_cert_file = cert;
            break;
        case 3:
            /* use Ed25519 */
            cert_file = cert25519;
            privkey_file = privkey25519;
            other_cert_file = cert;
            break;
# endif
#endif
        default:
            testresult = TEST_skip("EDCSA disabled");
            goto end;
    }
    /* Load primary cert */
    x509 = load_cert_pem(cert_file, NULL);
    if (!TEST_ptr(x509))
        goto end;
    pkey = X509_get0_pubkey(x509);
    /* load other cert */
    other_x509 = load_cert_pem(other_cert_file, NULL);
    if (!TEST_ptr(other_x509))
        goto end;
    other_pkey = X509_get0_pubkey(other_x509);
#ifdef OPENSSL_NO_ECDSA
    /* Can't get other_key if it's ECDSA */
    if (other_pkey == NULL && idx_cert == 0
        && (idx == 4 || idx == 6 || idx == 7 || idx == 16)) {
        testresult = TEST_skip("EDCSA disabled");
        goto end;
    }
#endif

    switch (idx_prot) {
    case 0:
#ifdef OSSL_NO_USABLE_TLS1_3
        testresult = TEST_skip("TLSv1.3 disabled");
        goto end;
#else
        tls_version = TLS1_3_VERSION;
        break;
#endif
    case 1:
#ifdef OPENSSL_NO_TLS1_2
        testresult = TEST_skip("TLSv1.2 disabled");
        goto end;
#else
        tls_version = TLS1_2_VERSION;
        break;
#endif
    default:
        goto end;
    }

    if (idx == 15) {
        test_libctx = libctx;
        defctxnull = OSSL_PROVIDER_load(NULL, "null");
        if (!TEST_ptr(defctxnull))
            goto end;
    }
    if (!TEST_true(create_ssl_ctx_pair(test_libctx,
                                       TLS_server_method(), TLS_client_method(),
                                       tls_version, tls_version,
                                       &sctx, &cctx, NULL, NULL)))
        goto end;

    if (idx_server_server_rpk)
        if (!TEST_true(SSL_CTX_set1_server_cert_type(sctx, cert_type_rpk, sizeof(cert_type_rpk))))
            goto end;
    if (idx_server_client_rpk)
        if (!TEST_true(SSL_CTX_set1_client_cert_type(sctx, cert_type_rpk, sizeof(cert_type_rpk))))
            goto end;
    if (idx_client_server_rpk)
        if (!TEST_true(SSL_CTX_set1_server_cert_type(cctx, cert_type_rpk, sizeof(cert_type_rpk))))
            goto end;
    if (idx_client_client_rpk)
        if (!TEST_true(SSL_CTX_set1_client_cert_type(cctx, cert_type_rpk, sizeof(cert_type_rpk))))
            goto end;
    if (!TEST_true(SSL_CTX_set_session_id_context(sctx, SID_CTX, sizeof(SID_CTX))))
        goto end;
    if (!TEST_true(SSL_CTX_set_session_id_context(cctx, SID_CTX, sizeof(SID_CTX))))
        goto end;

    if (!TEST_int_gt(SSL_CTX_dane_enable(sctx), 0))
        goto end;
    if (!TEST_int_gt(SSL_CTX_dane_enable(cctx), 0))
        goto end;

    /* NEW */
    SSL_CTX_set_verify(cctx, SSL_VERIFY_PEER, rpk_verify_client_cb);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_int_gt(SSL_dane_enable(serverssl, NULL), 0))
        goto end;
    if (!TEST_int_gt(SSL_dane_enable(clientssl, "example.com"), 0))
        goto end;

    /* Set private key and certificate */
    if (!TEST_int_eq(SSL_use_PrivateKey_file(serverssl, privkey_file, SSL_FILETYPE_PEM), 1))
        goto end;
    /* Only a private key */
    if (idx == 1) {
        if (idx_server_server_rpk == 0 || idx_client_server_rpk == 0) {
            expected = 0;
            want_error = SSL_ERROR_SSL;
        }
    } else {
        /* Add certificate */
        if (!TEST_int_eq(SSL_use_certificate_file(serverssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(serverssl), 1))
            goto end;
    }

    switch (idx) {
    default:
        if (!TEST_true(idx < RPK_TESTS))
            goto end;
        break;
    case 0:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        break;
    case 1:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        break;
    case 2:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(serverssl, pkey)))
            goto end;
        /* Use the same key for client auth */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
            goto end;
        SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
        client_auth = 1;
        break;
    case 3:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(clientssl, root_pkey)))
            goto end;
        break;
    case 4:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(clientssl, other_pkey)))
            goto end;
        break;
    case 5:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, root_pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        break;
    case 6:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, other_pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        break;
    case 7:
        if (idx_server_server_rpk == 1 && idx_client_server_rpk == 1)
            client_expected = -1;
        if (!TEST_true(SSL_add_expected_rpk(clientssl, other_pkey)))
            goto end;
        SSL_set_verify(clientssl, SSL_VERIFY_NONE, rpk_verify_client_cb);
        client_verify_result = X509_V_ERR_DANE_NO_MATCH;
        break;
    case 8:
        if (idx_server_server_rpk == 1 && idx_client_server_rpk == 1)
            client_expected = -1;
        /* no peer keys */
        SSL_set_verify(clientssl, SSL_VERIFY_NONE, rpk_verify_client_cb);
        client_verify_result = X509_V_ERR_RPK_UNTRUSTED;
        break;
    case 9:
        if (tls_version != TLS1_3_VERSION) {
            testresult = TEST_skip("PHA requires TLSv1.3");
            goto end;
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(serverssl, pkey)))
            goto end;
        /* Use the same key for client auth */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
            goto end;
        SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT | SSL_VERIFY_POST_HANDSHAKE, rpk_verify_server_cb);
        SSL_set_post_handshake_auth(clientssl, 1);
        client_auth = 1;
        break;
    case 10:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(serverssl, pkey)))
            goto end;
        /* Use the same key for client auth */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        /* Since there's no cert, this is expected to fail without RPK support */
        if (!idx_server_client_rpk || !idx_client_client_rpk) {
            expected = 0;
            want_error = SSL_ERROR_SSL;
            SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, NULL);
        } else {
            SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
        }
        client_auth = 1;
        break;
    case 11:
        if (!idx_server_server_rpk || !idx_client_server_rpk) {
            testresult = TEST_skip("Only testing resumption with server RPK");
            goto end;
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        resumption = 1;
        break;
    case 12:
        if (!idx_server_server_rpk || !idx_client_server_rpk) {
            testresult = TEST_skip("Only testing resumption with server RPK");
            goto end;
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        SSL_set_options(serverssl, SSL_OP_NO_TICKET);
        SSL_set_options(clientssl, SSL_OP_NO_TICKET);
        resumption = 1;
        break;
    case 13:
        if (!idx_server_server_rpk || !idx_client_server_rpk) {
            testresult = TEST_skip("Only testing resumption with server RPK");
            goto end;
        }
        if (!idx_server_client_rpk || !idx_client_client_rpk) {
            testresult = TEST_skip("Only testing client authentication resumption with client RPK");
            goto end;
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(serverssl, pkey)))
            goto end;
        /* Use the same key for client auth */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
            goto end;
        SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
        client_auth = 1;
        resumption = 1;
        break;
    case 14:
        if (!idx_server_server_rpk || !idx_client_server_rpk) {
            testresult = TEST_skip("Only testing resumption with server RPK");
            goto end;
        }
        if (!idx_server_client_rpk || !idx_client_client_rpk) {
            testresult = TEST_skip("Only testing client authentication resumption with client RPK");
            goto end;
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        if (!TEST_true(SSL_add_expected_rpk(serverssl, pkey)))
            goto end;
        /* Use the same key for client auth */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
            goto end;
        SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
        SSL_set_options(serverssl, SSL_OP_NO_TICKET);
        SSL_set_options(clientssl, SSL_OP_NO_TICKET);
        client_auth = 1;
        resumption = 1;
        break;
    case 15:
        if (!TEST_true(SSL_add_expected_rpk(clientssl, pkey)))
            goto end;
        break;
    case 16:
        if (idx_server_server_rpk == 1 && idx_client_server_rpk == 1) {
            /* wrong expected server key */
            expected = 0;
            want_error = SSL_ERROR_SSL;
            SSL_set_verify(serverssl, SSL_VERIFY_PEER, NULL);
        }
        if (!TEST_true(SSL_add_expected_rpk(clientssl, other_pkey)))
            goto end;
        break;
    case 17:
        if (idx_server_server_rpk == 1 && idx_client_server_rpk == 1) {
            /* no expected server keys */
            expected = 0;
            want_error = SSL_ERROR_SSL;
            SSL_set_verify(serverssl, SSL_VERIFY_PEER, NULL);
        }
        break;
    }

    ret = create_ssl_connection(serverssl, clientssl, want_error);
    if (!TEST_int_eq(expected, ret))
        goto end;

    if (expected <= 0) {
        testresult = 1;
        goto end;
    }

    /* Make sure client gets RPK or certificate as configured */
    if (idx_server_server_rpk && idx_client_server_rpk) {
        if (!TEST_long_eq(SSL_get_verify_result(clientssl), client_verify_result))
            goto end;
        if (!TEST_ptr(SSL_get0_peer_rpk(clientssl)))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(serverssl), TLSEXT_cert_type_rpk))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(clientssl), TLSEXT_cert_type_rpk))
            goto end;
    } else {
        if (!TEST_ptr(SSL_get0_peer_certificate(clientssl)))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(serverssl), TLSEXT_cert_type_x509))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(clientssl), TLSEXT_cert_type_x509))
            goto end;
    }

    if (idx == 9) {
        /* Make PHA happen... */
        if (!TEST_true(SSL_verify_client_post_handshake(serverssl)))
            goto end;
        if (!TEST_true(SSL_do_handshake(serverssl)))
            goto end;
        if (!TEST_int_le(SSL_read(clientssl, NULL, 0), 0))
            goto end;
        if (!TEST_int_le(SSL_read(serverssl, NULL, 0), 0))
            goto end;
    }

    /* Make sure server gets an RPK or certificate as configured */
    if (client_auth) {
        if (idx_server_client_rpk && idx_client_client_rpk) {
            if (!TEST_long_eq(SSL_get_verify_result(serverssl), server_verify_result))
                goto end;
            if (!TEST_ptr(SSL_get0_peer_rpk(serverssl)))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(serverssl), TLSEXT_cert_type_rpk))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(clientssl), TLSEXT_cert_type_rpk))
                goto end;
        } else {
            if (!TEST_ptr(SSL_get0_peer_certificate(serverssl)))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(serverssl), TLSEXT_cert_type_x509))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(clientssl), TLSEXT_cert_type_x509))
                goto end;
        }
    }

    if (resumption) {
        EVP_PKEY *client_pkey = NULL;
        EVP_PKEY *server_pkey = NULL;

        if (!TEST_ptr((client_sess = SSL_get1_session(clientssl)))
                || !TEST_ptr((client_pkey = SSL_SESSION_get0_peer_rpk(client_sess))))
            goto end;
        if (client_auth) {
            if (!TEST_ptr((server_sess = SSL_get1_session(serverssl)))
                || !TEST_ptr((server_pkey = SSL_SESSION_get0_peer_rpk(server_sess))))
            goto end;
        }
        SSL_shutdown(clientssl);
        SSL_shutdown(serverssl);
        SSL_free(clientssl);
        SSL_free(serverssl);
        serverssl = clientssl = NULL;

        if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                          NULL, NULL))
                || !TEST_true(SSL_set_session(clientssl, client_sess)))
            goto end;

        /* Set private key (and maybe certificate) */
        if (!TEST_int_eq(SSL_use_PrivateKey_file(serverssl, privkey_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_use_certificate_file(serverssl, cert_file, SSL_FILETYPE_PEM), 1))
            goto end;
        if (!TEST_int_eq(SSL_check_private_key(serverssl), 1))
            goto end;
        if (!TEST_int_gt(SSL_dane_enable(serverssl, "example.com"), 0))
            goto end;
        if (!TEST_int_gt(SSL_dane_enable(clientssl, "example.com"), 0))
            goto end;

        switch (idx) {
        default:
            break;
        case 11:
            if (!TEST_true(SSL_add_expected_rpk(clientssl, client_pkey)))
                goto end;
            break;
        case 12:
            if (!TEST_true(SSL_add_expected_rpk(clientssl, client_pkey)))
                goto end;
            SSL_set_options(clientssl, SSL_OP_NO_TICKET);
            SSL_set_options(serverssl, SSL_OP_NO_TICKET);
            break;
        case 13:
            if (!TEST_true(SSL_add_expected_rpk(clientssl, client_pkey)))
                goto end;
            if (!TEST_true(SSL_add_expected_rpk(serverssl, server_pkey)))
                goto end;
            /* Use the same key for client auth */
            if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
                goto end;
            if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
                goto end;
            if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
                goto end;
            SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
            break;
        case 14:
            if (!TEST_true(SSL_add_expected_rpk(clientssl, client_pkey)))
                goto end;
            if (!TEST_true(SSL_add_expected_rpk(serverssl, server_pkey)))
                goto end;
            /* Use the same key for client auth */
            if (!TEST_int_eq(SSL_use_PrivateKey_file(clientssl, privkey_file, SSL_FILETYPE_PEM), 1))
                goto end;
            if (!TEST_int_eq(SSL_use_certificate_file(clientssl, cert_file, SSL_FILETYPE_PEM), 1))
                goto end;
            if (!TEST_int_eq(SSL_check_private_key(clientssl), 1))
                goto end;
            SSL_set_verify(serverssl, SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, rpk_verify_server_cb);
            SSL_set_options(serverssl, SSL_OP_NO_TICKET);
            SSL_set_options(clientssl, SSL_OP_NO_TICKET);
            break;
        }

        ret = create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE);
        if (!TEST_true(ret))
            goto end;
        verify = SSL_get_verify_result(clientssl);
        if (!TEST_int_eq(client_expected, verify))
            goto end;
        if (!TEST_true(SSL_session_reused(clientssl)))
            goto end;

        if (!TEST_ptr(SSL_get0_peer_rpk(clientssl)))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(serverssl), TLSEXT_cert_type_rpk))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_server_cert_type(clientssl), TLSEXT_cert_type_rpk))
            goto end;

        if (client_auth) {
            if (!TEST_ptr(SSL_get0_peer_rpk(serverssl)))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(serverssl), TLSEXT_cert_type_rpk))
                goto end;
            if (!TEST_int_eq(SSL_get_negotiated_client_cert_type(clientssl), TLSEXT_cert_type_rpk))
                goto end;
        }
    }

    testresult = 1;

 end:
    OSSL_PROVIDER_unload(defctxnull);
    defctxnull = NULL;
    SSL_SESSION_free(client_sess);
    SSL_SESSION_free(server_sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    X509_free(x509);
    X509_free(other_x509);
    X509_free(root_x509);

    if (testresult == 0) {
        TEST_info("idx_ss_rpk=%d, idx_sc_rpk=%d, idx_cs_rpk=%d, idx_cc_rpk=%d, idx_cert=%d, idx_prot=%d, idx=%d",
                  idx_server_server_rpk, idx_server_client_rpk,
                  idx_client_server_rpk, idx_client_client_rpk,
                  idx_cert, idx_prot, idx);
    }
    return testresult;
}

static int test_rpk_api(void)
{
    int ret = 0;
    SSL_CTX *cctx = NULL, *sctx = NULL;
    unsigned char cert_type_dups[] = { TLSEXT_cert_type_rpk,
                                       TLSEXT_cert_type_x509,
                                       TLSEXT_cert_type_x509 };
    unsigned char cert_type_bad[] = { 0xFF };
    unsigned char cert_type_extra[] = { TLSEXT_cert_type_rpk,
                                        TLSEXT_cert_type_x509,
                                        0xFF };
    unsigned char cert_type_unsup[] = { TLSEXT_cert_type_pgp,
                                        TLSEXT_cert_type_1609dot2 };
    unsigned char cert_type_just_x509[] = { TLSEXT_cert_type_x509 };
    unsigned char cert_type_just_rpk[] = { TLSEXT_cert_type_rpk };

    if (!TEST_true(create_ssl_ctx_pair(NULL,
                                       TLS_server_method(), TLS_client_method(),
                                       TLS1_2_VERSION, TLS1_2_VERSION,
                                       &sctx, &cctx, NULL, NULL)))
        goto end;

    if (!TEST_false(SSL_CTX_set1_server_cert_type(sctx, cert_type_dups, sizeof(cert_type_dups))))
        goto end;

    if (!TEST_false(SSL_CTX_set1_server_cert_type(sctx, cert_type_bad, sizeof(cert_type_bad))))
        goto end;

    if (!TEST_false(SSL_CTX_set1_server_cert_type(sctx, cert_type_extra, sizeof(cert_type_extra))))
        goto end;

    if (!TEST_false(SSL_CTX_set1_server_cert_type(sctx, cert_type_unsup, sizeof(cert_type_unsup))))
        goto end;

    if (!TEST_true(SSL_CTX_set1_server_cert_type(sctx, cert_type_just_x509, sizeof(cert_type_just_x509))))
        goto end;

    if (!TEST_true(SSL_CTX_set1_server_cert_type(sctx, cert_type_just_rpk, sizeof(cert_type_just_rpk))))
        goto end;

    ret = 1;
 end:
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return ret;
}
OPT_TEST_DECLARE_USAGE("certdir\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(certsdir = test_get_argument(0)))
        return 0;

    rootcert = test_mk_file_path(certsdir, "rootcert.pem");
    if (rootcert == NULL)
        goto err;

    cert = test_mk_file_path(certsdir, "servercert.pem");
    if (cert == NULL)
        goto err;

    privkey = test_mk_file_path(certsdir, "serverkey.pem");
    if (privkey == NULL)
        goto err;

    cert2 = test_mk_file_path(certsdir, "server-ecdsa-cert.pem");
    if (cert2 == NULL)
        goto err;

    privkey2 = test_mk_file_path(certsdir, "server-ecdsa-key.pem");
    if (privkey2 == NULL)
        goto err;

    cert448 = test_mk_file_path(certsdir, "server-ed448-cert.pem");
    if (cert2 == NULL)
        goto err;

    privkey448 = test_mk_file_path(certsdir, "server-ed448-key.pem");
    if (privkey2 == NULL)
        goto err;

    cert25519 = test_mk_file_path(certsdir, "server-ed25519-cert.pem");
    if (cert2 == NULL)
        goto err;

    privkey25519 = test_mk_file_path(certsdir, "server-ed25519-key.pem");
    if (privkey2 == NULL)
        goto err;

    libctx = OSSL_LIB_CTX_new();
    if (libctx == NULL)
        goto err;

    ADD_TEST(test_rpk_api);
    ADD_ALL_TESTS(test_rpk, RPK_TESTS * RPK_DIMS);
    return 1;

 err:
    return 0;
}

void cleanup_tests(void)
{
    OPENSSL_free(rootcert);
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    OPENSSL_free(cert2);
    OPENSSL_free(privkey2);
    OPENSSL_free(cert448);
    OPENSSL_free(privkey448);
    OPENSSL_free(cert25519);
    OPENSSL_free(privkey25519);
    OSSL_LIB_CTX_free(libctx);
 }
