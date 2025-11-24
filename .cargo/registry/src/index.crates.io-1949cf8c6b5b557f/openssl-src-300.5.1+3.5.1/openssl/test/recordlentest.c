/*
 * Copyright 2017-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>

#include "helpers/ssltestlib.h"
#include "testutil.h"

static char *cert = NULL;
static char *privkey = NULL;

#define TEST_PLAINTEXT_OVERFLOW_OK      0
#define TEST_PLAINTEXT_OVERFLOW_NOT_OK  1
#define TEST_ENCRYPTED_OVERFLOW_TLS1_3_OK       2
#define TEST_ENCRYPTED_OVERFLOW_TLS1_3_NOT_OK   3
#define TEST_ENCRYPTED_OVERFLOW_TLS1_2_OK       4
#define TEST_ENCRYPTED_OVERFLOW_TLS1_2_NOT_OK   5

#define TOTAL_RECORD_OVERFLOW_TESTS 6

static int write_record(BIO *b, size_t len, uint8_t rectype, int recversion)
{
    unsigned char header[SSL3_RT_HEADER_LENGTH];
    size_t written;
    unsigned char buf[256];

    memset(buf, 0, sizeof(buf));

    header[0] = rectype;
    header[1] = (recversion >> 8) & 0xff;
    header[2] = recversion & 0xff;
    header[3] = (len >> 8) & 0xff;
    header[4] = len & 0xff;

    if (!BIO_write_ex(b, header, SSL3_RT_HEADER_LENGTH, &written)
            || written != SSL3_RT_HEADER_LENGTH)
        return 0;

    while (len > 0) {
        size_t outlen;

        if (len > sizeof(buf))
            outlen = sizeof(buf);
        else
            outlen = len;

        if (!BIO_write_ex(b, buf, outlen, &written)
                || written != outlen)
            return 0;

        len -= outlen;
    }

    return 1;
}

static int fail_due_to_record_overflow(int enc)
{
    long err = ERR_peek_error();
    int reason;

    if (enc)
        reason = SSL_R_ENCRYPTED_LENGTH_TOO_LONG;
    else
        reason = SSL_R_DATA_LENGTH_TOO_LONG;

    if (ERR_GET_LIB(err) == ERR_LIB_SSL
            && ERR_GET_REASON(err) == reason)
        return 1;

    return 0;
}

static int test_record_overflow(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    size_t len = 0;
    size_t written;
    int overf_expected;
    unsigned char buf;
    BIO *serverbio;
    int recversion;

#ifdef OPENSSL_NO_TLS1_2
    if (idx == TEST_ENCRYPTED_OVERFLOW_TLS1_2_OK
            || idx == TEST_ENCRYPTED_OVERFLOW_TLS1_2_NOT_OK)
        return 1;
#endif
#if defined(OPENSSL_NO_TLS1_3) \
    || (defined(OPENSSL_NO_EC) && defined(OPENSSL_NO_DH))
    if (idx == TEST_ENCRYPTED_OVERFLOW_TLS1_3_OK
            || idx == TEST_ENCRYPTED_OVERFLOW_TLS1_3_NOT_OK)
        return 1;
#endif

    if (!TEST_true(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (idx == TEST_ENCRYPTED_OVERFLOW_TLS1_2_OK
            || idx == TEST_ENCRYPTED_OVERFLOW_TLS1_2_NOT_OK) {
        len = SSL3_RT_MAX_ENCRYPTED_LENGTH;
#ifndef OPENSSL_NO_COMP
        len -= SSL3_RT_MAX_COMPRESSED_OVERHEAD;
#endif
        SSL_CTX_set_max_proto_version(sctx, TLS1_2_VERSION);
    } else if (idx == TEST_ENCRYPTED_OVERFLOW_TLS1_3_OK
               || idx == TEST_ENCRYPTED_OVERFLOW_TLS1_3_NOT_OK) {
        len = SSL3_RT_MAX_TLS13_ENCRYPTED_LENGTH;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    serverbio = SSL_get_rbio(serverssl);

    if (idx == TEST_PLAINTEXT_OVERFLOW_OK
            || idx == TEST_PLAINTEXT_OVERFLOW_NOT_OK) {
        len = SSL3_RT_MAX_PLAIN_LENGTH;

        if (idx == TEST_PLAINTEXT_OVERFLOW_NOT_OK)
            len++;

        if (!TEST_true(write_record(serverbio, len,
                                    SSL3_RT_HANDSHAKE, TLS1_VERSION)))
            goto end;

        if (!TEST_int_le(SSL_accept(serverssl), 0))
            goto end;

        overf_expected = (idx == TEST_PLAINTEXT_OVERFLOW_OK) ? 0 : 1;
        if (!TEST_int_eq(fail_due_to_record_overflow(0), overf_expected))
            goto end;

        goto success;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    if (idx == TEST_ENCRYPTED_OVERFLOW_TLS1_2_NOT_OK
            || idx == TEST_ENCRYPTED_OVERFLOW_TLS1_3_NOT_OK) {
        overf_expected = 1;
        len++;
    } else {
        overf_expected = 0;
    }

    recversion = TLS1_2_VERSION;

    if (!TEST_true(write_record(serverbio, len, SSL3_RT_APPLICATION_DATA,
                                recversion)))
        goto end;

    if (!TEST_false(SSL_read_ex(serverssl, &buf, sizeof(buf), &written)))
        goto end;

    if (!TEST_int_eq(fail_due_to_record_overflow(1), overf_expected))
        goto end;

 success:
    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(cert = test_get_argument(0))
            || !TEST_ptr(privkey = test_get_argument(1)))
        return 0;

    ADD_ALL_TESTS(test_record_overflow, TOTAL_RECORD_OVERFLOW_TESTS);
    return 1;
}

void cleanup_tests(void)
{
    bio_s_mempacket_test_free();
}
