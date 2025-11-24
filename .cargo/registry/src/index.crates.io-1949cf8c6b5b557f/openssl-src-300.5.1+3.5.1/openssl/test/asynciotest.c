/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <string.h>
#include <openssl/ssl.h>
#include <openssl/bio.h>
#include <openssl/err.h>

#include "internal/packet.h"

#include "helpers/ssltestlib.h"
#include "testutil.h"

/* Should we fragment records or not? 0 = no, !0 = yes*/
static int fragment = 0;

static char *cert = NULL;
static char *privkey = NULL;

static int async_new(BIO *bi);
static int async_free(BIO *a);
static int async_read(BIO *b, char *out, int outl);
static int async_write(BIO *b, const char *in, int inl);
static long async_ctrl(BIO *b, int cmd, long num, void *ptr);
static int async_gets(BIO *bp, char *buf, int size);
static int async_puts(BIO *bp, const char *str);

/* Choose a sufficiently large type likely to be unused for this custom BIO */
# define BIO_TYPE_ASYNC_FILTER  (0x80 | BIO_TYPE_FILTER)

static BIO_METHOD *methods_async = NULL;

struct async_ctrs {
    unsigned int rctr;
    unsigned int wctr;
};

static const BIO_METHOD *bio_f_async_filter(void)
{
    if (methods_async == NULL) {
        methods_async = BIO_meth_new(BIO_TYPE_ASYNC_FILTER, "Async filter");
        if (   methods_async == NULL
            || !BIO_meth_set_write(methods_async, async_write)
            || !BIO_meth_set_read(methods_async, async_read)
            || !BIO_meth_set_puts(methods_async, async_puts)
            || !BIO_meth_set_gets(methods_async, async_gets)
            || !BIO_meth_set_ctrl(methods_async, async_ctrl)
            || !BIO_meth_set_create(methods_async, async_new)
            || !BIO_meth_set_destroy(methods_async, async_free))
            return NULL;
    }
    return methods_async;
}

static int async_new(BIO *bio)
{
    struct async_ctrs *ctrs;

    ctrs = OPENSSL_zalloc(sizeof(struct async_ctrs));
    if (ctrs == NULL)
        return 0;

    BIO_set_data(bio, ctrs);
    BIO_set_init(bio, 1);
    return 1;
}

static int async_free(BIO *bio)
{
    struct async_ctrs *ctrs;

    if (bio == NULL)
        return 0;
    ctrs = BIO_get_data(bio);
    OPENSSL_free(ctrs);
    BIO_set_data(bio, NULL);
    BIO_set_init(bio, 0);

    return 1;
}

static int async_read(BIO *bio, char *out, int outl)
{
    struct async_ctrs *ctrs;
    int ret = 0;
    BIO *next = BIO_next(bio);

    if (outl <= 0)
        return 0;
    if (next == NULL)
        return 0;

    ctrs = BIO_get_data(bio);

    BIO_clear_retry_flags(bio);

    if (ctrs->rctr > 0) {
        ret = BIO_read(next, out, 1);
        if (ret <= 0 && BIO_should_read(next))
            BIO_set_retry_read(bio);
        ctrs->rctr = 0;
    } else {
        ctrs->rctr++;
        BIO_set_retry_read(bio);
    }

    return ret;
}

#define MIN_RECORD_LEN  6

#define CONTENTTYPEPOS  0
#define VERSIONHIPOS    1
#define VERSIONLOPOS    2
#define DATAPOS         5

static int async_write(BIO *bio, const char *in, int inl)
{
    struct async_ctrs *ctrs;
    int ret = 0;
    size_t written = 0;
    BIO *next = BIO_next(bio);

    if (inl <= 0)
        return 0;
    if (next == NULL)
        return 0;

    ctrs = BIO_get_data(bio);

    BIO_clear_retry_flags(bio);

    if (ctrs->wctr > 0) {
        ctrs->wctr = 0;
        if (fragment) {
            PACKET pkt;

            if (!PACKET_buf_init(&pkt, (const unsigned char *)in, inl))
                return -1;

            while (PACKET_remaining(&pkt) > 0) {
                PACKET payload, wholebody, sessionid, extensions;
                unsigned int contenttype, versionhi, versionlo, data;
                unsigned int msgtype = 0, negversion = 0;

                if (!PACKET_get_1(&pkt, &contenttype)
                        || !PACKET_get_1(&pkt, &versionhi)
                        || !PACKET_get_1(&pkt, &versionlo)
                        || !PACKET_get_length_prefixed_2(&pkt, &payload))
                    return -1;

                /* Pretend we wrote out the record header */
                written += SSL3_RT_HEADER_LENGTH;

                wholebody = payload;
                if (contenttype == SSL3_RT_HANDSHAKE
                        && !PACKET_get_1(&wholebody, &msgtype))
                    return -1;

                if (msgtype == SSL3_MT_SERVER_HELLO) {
                    if (!PACKET_forward(&wholebody,
                                            SSL3_HM_HEADER_LENGTH - 1)
                            || !PACKET_get_net_2(&wholebody, &negversion)
                               /* Skip random (32 bytes) */
                            || !PACKET_forward(&wholebody, 32)
                               /* Skip session id */
                            || !PACKET_get_length_prefixed_1(&wholebody,
                                                             &sessionid)
                               /*
                                * Skip ciphersuite (2 bytes) and compression
                                * method (1 byte)
                                */
                            || !PACKET_forward(&wholebody, 2 + 1)
                            || !PACKET_get_length_prefixed_2(&wholebody,
                                                             &extensions))
                        return -1;

                    /*
                     * Find the negotiated version in supported_versions
                     * extension, if present.
                     */
                    while (PACKET_remaining(&extensions)) {
                        unsigned int type;
                        PACKET extbody;

                        if (!PACKET_get_net_2(&extensions, &type)
                                || !PACKET_get_length_prefixed_2(&extensions,
                                &extbody))
                            return -1;

                        if (type == TLSEXT_TYPE_supported_versions
                                && (!PACKET_get_net_2(&extbody, &negversion)
                                    || PACKET_remaining(&extbody) != 0))
                            return -1;
                    }
                }

                while (PACKET_get_1(&payload, &data)) {
                    /* Create a new one byte long record for each byte in the
                     * record in the input buffer
                     */
                    char smallrec[MIN_RECORD_LEN] = {
                        0, /* Content type */
                        0, /* Version hi */
                        0, /* Version lo */
                        0, /* Length hi */
                        1, /* Length lo */
                        0  /* Data */
                    };

                    smallrec[CONTENTTYPEPOS] = contenttype;
                    smallrec[VERSIONHIPOS] = versionhi;
                    smallrec[VERSIONLOPOS] = versionlo;
                    smallrec[DATAPOS] = data;
                    ret = BIO_write(next, smallrec, MIN_RECORD_LEN);
                    if (ret <= 0)
                        return -1;
                    written++;
                }
                /*
                 * We can't fragment anything after the ServerHello (or CCS <=
                 * TLS1.2), otherwise we get a bad record MAC
                 */
                if (contenttype == SSL3_RT_CHANGE_CIPHER_SPEC
                        || (negversion == TLS1_3_VERSION
                            && msgtype == SSL3_MT_SERVER_HELLO)) {
                    fragment = 0;
                    break;
                }
            }
        }
        /* Write any data we have left after fragmenting */
        ret = 0;
        if ((int)written < inl) {
            ret = BIO_write(next, in + written, inl - written);
        }

        if (ret <= 0 && BIO_should_write(next))
            BIO_set_retry_write(bio);
        else
            ret += written;
    } else {
        ctrs->wctr++;
        BIO_set_retry_write(bio);
    }

    return ret;
}

static long async_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret;
    BIO *next = BIO_next(bio);

    if (next == NULL)
        return 0;

    switch (cmd) {
    case BIO_CTRL_DUP:
        ret = 0L;
        break;
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static int async_gets(BIO *bio, char *buf, int size)
{
    /* We don't support this - not needed anyway */
    return -1;
}

static int async_puts(BIO *bio, const char *str)
{
    return async_write(bio, str, strlen(str));
}

#define MAX_ATTEMPTS    100

static int test_asyncio(int test)
{
    SSL_CTX *serverctx = NULL, *clientctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    BIO *s_to_c_fbio = NULL, *c_to_s_fbio = NULL;
    int testresult = 0, ret;
    size_t i, j;
    const char testdata[] = "Test data";
    char buf[sizeof(testdata)];

    if (!TEST_true(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION, 0,
                                       &serverctx, &clientctx, cert, privkey)))
        goto end;

    /*
     * We do 2 test runs. The first time around we just do a normal handshake
     * with lots of async io going on. The second time around we also break up
     * all records so that the content is only one byte length (up until the
     * CCS)
     */
    if (test == 1)
        fragment = 1;


    s_to_c_fbio = BIO_new(bio_f_async_filter());
    c_to_s_fbio = BIO_new(bio_f_async_filter());
    if (!TEST_ptr(s_to_c_fbio)
            || !TEST_ptr(c_to_s_fbio)) {
        BIO_free(s_to_c_fbio);
        BIO_free(c_to_s_fbio);
        goto end;
    }

    /* BIOs get freed on error */
    if (!TEST_true(create_ssl_objects(serverctx, clientctx, &serverssl,
                                      &clientssl, s_to_c_fbio, c_to_s_fbio))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                          SSL_ERROR_NONE)))
        goto end;

    /*
     * Send and receive some test data. Do the whole thing twice to ensure
     * we hit at least one async event in both reading and writing
     */
    for (j = 0; j < 2; j++) {
        int len;

        /*
         * Write some test data. It should never take more than 2 attempts
         * (the first one might be a retryable fail).
         */
        for (ret = -1, i = 0, len = 0; len != sizeof(testdata) && i < 2;
            i++) {
            ret = SSL_write(clientssl, testdata + len,
                sizeof(testdata) - len);
            if (ret > 0) {
                len += ret;
            } else {
                int ssl_error = SSL_get_error(clientssl, ret);

                if (!TEST_false(ssl_error == SSL_ERROR_SYSCALL ||
                                ssl_error == SSL_ERROR_SSL))
                    goto end;
            }
        }
        if (!TEST_size_t_eq(len, sizeof(testdata)))
            goto end;

        /*
         * Now read the test data. It may take more attempts here because
         * it could fail once for each byte read, including all overhead
         * bytes from the record header/padding etc.
         */
        for (ret = -1, i = 0, len = 0; len != sizeof(testdata) &&
                i < MAX_ATTEMPTS; i++) {
            ret = SSL_read(serverssl, buf + len, sizeof(buf) - len);
            if (ret > 0) {
                len += ret;
            } else {
                int ssl_error = SSL_get_error(serverssl, ret);

                if (!TEST_false(ssl_error == SSL_ERROR_SYSCALL ||
                                ssl_error == SSL_ERROR_SSL))
                    goto end;
            }
        }
        if (!TEST_mem_eq(testdata, sizeof(testdata), buf, len))
            goto end;
    }

    /* Also frees the BIOs */
    SSL_free(clientssl);
    SSL_free(serverssl);
    clientssl = serverssl = NULL;

    testresult = 1;

 end:
    SSL_free(clientssl);
    SSL_free(serverssl);
    SSL_CTX_free(clientctx);
    SSL_CTX_free(serverctx);

    return testresult;
}

OPT_TEST_DECLARE_USAGE("certname privkey\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(cert = test_get_argument(0))
            || !TEST_ptr(privkey = test_get_argument(1)))
        return 0;

    ADD_ALL_TESTS(test_asyncio, 2);
    return 1;
}

void cleanup_tests(void)
{
    BIO_meth_free(methods_async);
}
