/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "testutil.h"
#include <openssl/ssl.h>

typedef struct {
    int proto;
    int min_version;
    int max_version;
    int min_ok;
    int max_ok;
    int expected_min;
    int expected_max;
} version_test;

#define PROTO_TLS  0
#define PROTO_DTLS 1
#define PROTO_QUIC 2

/*
 * If a version is valid for *any* protocol then setting the min/max protocol is
 * expected to return success, even if that version is not valid for *this*
 * protocol. However it only has an effect if it is valid for *this* protocol -
 * otherwise it is ignored.
 */
static const version_test version_testdata[] = {
    /* proto     min                     max                     ok    expected min        expected max */
    {PROTO_TLS,  0,                      0,                      1, 1, 0,                  0},
    {PROTO_TLS,  SSL3_VERSION,           TLS1_3_VERSION,         1, 1, SSL3_VERSION,       TLS1_3_VERSION},
    {PROTO_TLS,  TLS1_VERSION,           TLS1_3_VERSION,         1, 1, TLS1_VERSION,       TLS1_3_VERSION},
    {PROTO_TLS,  TLS1_VERSION,           TLS1_2_VERSION,         1, 1, TLS1_VERSION,       TLS1_2_VERSION},
    {PROTO_TLS,  TLS1_2_VERSION,         TLS1_2_VERSION,         1, 1, TLS1_2_VERSION,     TLS1_2_VERSION},
    {PROTO_TLS,  TLS1_2_VERSION,         TLS1_1_VERSION,         1, 1, TLS1_2_VERSION,     TLS1_1_VERSION},
    {PROTO_TLS,  SSL3_VERSION - 1,       TLS1_3_VERSION,         0, 1, 0,                  TLS1_3_VERSION},
    {PROTO_TLS,  SSL3_VERSION,           TLS1_3_VERSION + 1,     1, 0, SSL3_VERSION,       0},
#ifndef OPENSSL_NO_DTLS
    {PROTO_TLS,  DTLS1_VERSION,          DTLS1_2_VERSION,        1, 1, 0,                  0},
#endif
    {PROTO_TLS,  OSSL_QUIC1_VERSION,     OSSL_QUIC1_VERSION,     0, 0, 0,                  0},
    {PROTO_TLS,  7,                      42,                     0, 0, 0,                  0},
    {PROTO_DTLS, 0,                      0,                      1, 1, 0,                  0},
    {PROTO_DTLS, DTLS1_VERSION,          DTLS1_2_VERSION,        1, 1, DTLS1_VERSION,      DTLS1_2_VERSION},
#ifndef OPENSSL_NO_DTLS1_2
    {PROTO_DTLS, DTLS1_2_VERSION,        DTLS1_2_VERSION,        1, 1, DTLS1_2_VERSION,    DTLS1_2_VERSION},
#endif
#ifndef OPENSSL_NO_DTLS1
    {PROTO_DTLS, DTLS1_VERSION,          DTLS1_VERSION,          1, 1, DTLS1_VERSION,      DTLS1_VERSION},
#endif
#if !defined(OPENSSL_NO_DTLS1) && !defined(OPENSSL_NO_DTLS1_2)
    {PROTO_DTLS, DTLS1_2_VERSION,        DTLS1_VERSION,          1, 1, DTLS1_2_VERSION,    DTLS1_VERSION},
#endif
    {PROTO_DTLS, DTLS1_VERSION + 1,      DTLS1_2_VERSION,        0, 1, 0,                  DTLS1_2_VERSION},
    {PROTO_DTLS, DTLS1_VERSION,          DTLS1_2_VERSION - 1,    1, 0, DTLS1_VERSION,      0},
    {PROTO_DTLS, TLS1_VERSION,           TLS1_3_VERSION,         1, 1, 0,                  0},
    {PROTO_DTLS, OSSL_QUIC1_VERSION,     OSSL_QUIC1_VERSION,     0, 0, 0,                  0},
    /* These functions never have an effect when called on a QUIC object */
    {PROTO_QUIC, 0,                      0,                      1, 1, 0,                  0},
    {PROTO_QUIC, OSSL_QUIC1_VERSION,     OSSL_QUIC1_VERSION,     0, 0, 0,                  0},
    {PROTO_QUIC, OSSL_QUIC1_VERSION,     OSSL_QUIC1_VERSION + 1, 0, 0, 0,                  0},
    {PROTO_QUIC, TLS1_VERSION,           TLS1_3_VERSION,         1, 1, 0,                  0},
#ifndef OPENSSL_NO_DTLS
    {PROTO_QUIC, DTLS1_VERSION,          DTLS1_2_VERSION,        1, 1, 0,                  0},
#endif
};

static int test_set_min_max_version(int idx_tst)
{
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    int testresult = 0;
    version_test t = version_testdata[idx_tst];
    const SSL_METHOD *meth = NULL;

    switch (t.proto) {
    case PROTO_TLS:
        meth = TLS_client_method();
        break;

#ifndef OPENSSL_NO_DTLS
    case PROTO_DTLS:
        meth = DTLS_client_method();
        break;
#endif

#ifndef OPENSSL_NO_QUIC
    case PROTO_QUIC:
        meth = OSSL_QUIC_client_method();
        break;
#endif
    }

    if (meth == NULL)
        return TEST_skip("Protocol not supported");

    ctx = SSL_CTX_new(meth);
    if (ctx == NULL)
        goto end;

    ssl = SSL_new(ctx);
    if (ssl == NULL)
        goto end;

    if (!TEST_int_eq(SSL_CTX_set_min_proto_version(ctx, t.min_version), t.min_ok))
        goto end;
    if (!TEST_int_eq(SSL_CTX_set_max_proto_version(ctx, t.max_version), t.max_ok))
        goto end;
    if (!TEST_int_eq(SSL_CTX_get_min_proto_version(ctx), t.expected_min))
        goto end;
    if (!TEST_int_eq(SSL_CTX_get_max_proto_version(ctx), t.expected_max))
        goto end;

    if (!TEST_int_eq(SSL_set_min_proto_version(ssl, t.min_version), t.min_ok))
        goto end;
    if (!TEST_int_eq(SSL_set_max_proto_version(ssl, t.max_version), t.max_ok))
        goto end;
    if (!TEST_int_eq(SSL_get_min_proto_version(ssl), t.expected_min))
        goto end;
    if (!TEST_int_eq(SSL_get_max_proto_version(ssl), t.expected_max))
        goto end;

    testresult = 1;

 end:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_set_min_max_version, sizeof(version_testdata) / sizeof(version_test));
    return 1;
}
