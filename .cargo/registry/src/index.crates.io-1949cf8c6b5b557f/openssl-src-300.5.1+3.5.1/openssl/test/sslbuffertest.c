/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * We need access to the deprecated low level Engine APIs for legacy purposes
 * when the deprecated calls are not hidden
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
# define OPENSSL_SUPPRESS_DEPRECATED
#endif

#include <string.h>
#include <openssl/ssl.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/engine.h>

#ifndef OPENSSL_NO_QUIC
/* This test does not link libssl so avoid pulling in QUIC unwrappers. */
# define OPENSSL_NO_QUIC
#endif

/* We include internal headers so we can check if the buffers are allocated */
#include "../ssl/ssl_local.h"
#include "../ssl/record/record_local.h"
#include "internal/recordmethod.h"
#include "../ssl/record/methods/recmethod_local.h"
#include "internal/ssl_unwrap.h"

#include "internal/packet.h"

#include "helpers/ssltestlib.h"
#include "testutil.h"

struct async_ctrs {
    unsigned int rctr;
    unsigned int wctr;
};

static SSL_CTX *serverctx = NULL;
static SSL_CTX *clientctx = NULL;

#define MAX_ATTEMPTS    100

static int checkbuffers(SSL *s, int isalloced)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    OSSL_RECORD_LAYER *rrl = sc->rlayer.rrl;
    OSSL_RECORD_LAYER *wrl = sc->rlayer.wrl;

    if (isalloced)
        return rrl->rbuf.buf != NULL && wrl->wbuf[0].buf != NULL;

    return rrl->rbuf.buf == NULL && wrl->wbuf[0].buf == NULL;
}

/*
 * There are 9 passes in the tests
 * 0 = control test
 * tests during writes
 * 1 = free buffers
 * 2 = + allocate buffers after free
 * 3 = + allocate buffers again
 * 4 = + free buffers after allocation
 * tests during reads
 * 5 = + free buffers
 * 6 = + free buffers again
 * 7 = + allocate buffers after free
 * 8 = + free buffers after allocation
 */
static int test_func(int test)
{
    int result = 0;
    SSL *serverssl = NULL, *clientssl = NULL;
    int ret;
    size_t i, j;
    const char testdata[] = "Test data";
    char buf[sizeof(testdata)];

    if (!TEST_true(create_ssl_objects(serverctx, clientctx, &serverssl, &clientssl,
                                      NULL, NULL))) {
        TEST_error("Test %d failed: Create SSL objects failed\n", test);
        goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))) {
        TEST_error("Test %d failed: Create SSL connection failed\n", test);
        goto end;
    }

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
            /* test == 0 mean to free/allocate = control */
            if (test >= 1 && (!TEST_true(SSL_free_buffers(clientssl))
                              || !TEST_true(checkbuffers(clientssl, 0))))
                goto end;
            if (test >= 2 && (!TEST_true(SSL_alloc_buffers(clientssl))
                              || !TEST_true(checkbuffers(clientssl, 1))))
                goto end;
            /* allocate a second time */
            if (test >= 3 && (!TEST_true(SSL_alloc_buffers(clientssl))
                              || !TEST_true(checkbuffers(clientssl, 1))))
                goto end;
            if (test >= 4 && (!TEST_true(SSL_free_buffers(clientssl))
                              || !TEST_true(checkbuffers(clientssl, 0))))
                goto end;

            ret = SSL_write(clientssl, testdata + len,
                            sizeof(testdata) - len);
            if (ret > 0) {
                len += ret;
            } else {
                int ssl_error = SSL_get_error(clientssl, ret);

                if (ssl_error == SSL_ERROR_SYSCALL ||
                    ssl_error == SSL_ERROR_SSL) {
                    TEST_error("Test %d failed: Failed to write app data\n", test);
                    goto end;
                }
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
            if (test >= 5 && (!TEST_true(SSL_free_buffers(serverssl))
                              || !TEST_true(checkbuffers(serverssl, 0))))
                goto end;
            /* free a second time */
            if (test >= 6 && (!TEST_true(SSL_free_buffers(serverssl))
                              || !TEST_true(checkbuffers(serverssl, 0))))
                goto end;
            if (test >= 7 && (!TEST_true(SSL_alloc_buffers(serverssl))
                              || !TEST_true(checkbuffers(serverssl, 1))))
                goto end;
            if (test >= 8 && (!TEST_true(SSL_free_buffers(serverssl))
                              || !TEST_true(checkbuffers(serverssl, 0))))
                goto end;

            ret = SSL_read(serverssl, buf + len, sizeof(buf) - len);
            if (ret > 0) {
                len += ret;
            } else {
                int ssl_error = SSL_get_error(serverssl, ret);

                if (ssl_error == SSL_ERROR_SYSCALL ||
                    ssl_error == SSL_ERROR_SSL) {
                    TEST_error("Test %d failed: Failed to read app data\n", test);
                    goto end;
                }
            }
        }
        if (!TEST_mem_eq(buf, len, testdata, sizeof(testdata)))
            goto end;
    }

    result = 1;
 end:
    if (!result)
        ERR_print_errors_fp(stderr);

    SSL_free(clientssl);
    SSL_free(serverssl);

    return result;
}

/*
 * Test that attempting to free the buffers at points where they cannot be freed
 * works as expected
 * Test 0: Attempt to free buffers after a full record has been processed, but
 *         the application has only performed a partial read
 * Test 1: Attempt to free buffers after only a partial record header has been
 *         received
 * Test 2: Attempt to free buffers after a full record header but no record body
 * Test 3: Attempt to free buffers after a full record hedaer and partial record
 *         body
 * Test 4-7: We repeat tests 0-3 but including data from a second pipelined
 *           record
 */
static int test_free_buffers(int test)
{
    int result = 0;
    SSL *serverssl = NULL, *clientssl = NULL;
    const char testdata[] = "Test data";
    char buf[120];
    size_t written, readbytes;
    int i, pipeline = test > 3;
    ENGINE *e = NULL;

    if (pipeline) {
        e = load_dasync();
        if (e == NULL)
            goto end;
        test -= 4;
    }

    if (!TEST_true(create_ssl_objects(serverctx, clientctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (pipeline) {
        if (!TEST_true(SSL_set_cipher_list(serverssl, "AES128-SHA"))
                || !TEST_true(SSL_set_max_proto_version(serverssl,
                                                        TLS1_2_VERSION))
                || !TEST_true(SSL_set_max_pipelines(serverssl, 2)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    /*
     * For the non-pipeline case we write one record. For pipelining we write
     * two records.
     */
    for (i = 0; i <= pipeline; i++) {
        if (!TEST_true(SSL_write_ex(clientssl, testdata, strlen(testdata),
                                    &written)))
            goto end;
    }

    if (test == 0) {
        size_t readlen = 1;

        /*
         * Deliberately only read the first byte - so the remaining bytes are
         * still buffered. In the pipelining case we read as far as the first
         * byte from the second record.
         */
        if (pipeline)
            readlen += strlen(testdata);

        if (!TEST_true(SSL_read_ex(serverssl, buf, readlen, &readbytes))
                || !TEST_size_t_eq(readlen, readbytes))
            goto end;
    } else {
        BIO *tmp;
        size_t partial_len;

        /* Remove all the data that is pending for read by the server */
        tmp = SSL_get_rbio(serverssl);
        if (!TEST_true(BIO_read_ex(tmp, buf, sizeof(buf), &readbytes))
                || !TEST_size_t_lt(readbytes, sizeof(buf))
                || !TEST_size_t_gt(readbytes, SSL3_RT_HEADER_LENGTH))
            goto end;

        switch(test) {
        case 1:
            partial_len = SSL3_RT_HEADER_LENGTH - 1;
            break;
        case 2:
            partial_len = SSL3_RT_HEADER_LENGTH;
            break;
        case 3:
            partial_len = readbytes - 1;
            break;
        default:
            TEST_error("Invalid test index");
            goto end;
        }

        if (pipeline) {
            /* We happen to know the first record is 57 bytes long */
            const size_t first_rec_len = 57;

            if (test != 3)
                partial_len += first_rec_len;

            /*
             * Sanity check. If we got the record len right then this should
             * never fail.
             */
            if (!TEST_int_eq(buf[first_rec_len], SSL3_RT_APPLICATION_DATA))
                goto end;
        }

        /*
         * Put back just the partial record (plus the whole initial record in
         * the pipelining case)
         */
        if (!TEST_true(BIO_write_ex(tmp, buf, partial_len, &written)))
            goto end;

        if (pipeline) {
            /*
             * Attempt a read. This should pass but only return data from the
             * first record. Only a partial record is available for the second
             * record.
             */
            if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf),
                                        &readbytes))
                    || !TEST_size_t_eq(readbytes, strlen(testdata)))
                goto end;
        } else {
            /*
            * Attempt a read. This should fail because only a partial record is
            * available.
            */
            if (!TEST_false(SSL_read_ex(serverssl, buf, sizeof(buf),
                                        &readbytes)))
                goto end;
        }
    }

    /*
     * Attempting to free the buffers at this point should fail because they are
     * still in use
     */
    if (!TEST_false(SSL_free_buffers(serverssl)))
        goto end;

    result = 1;
 end:
    SSL_free(clientssl);
    SSL_free(serverssl);
#ifndef OPENSSL_NO_DYNAMIC_ENGINE
    if (e != NULL) {
        ENGINE_unregister_ciphers(e);
        ENGINE_finish(e);
        ENGINE_free(e);
    }
#endif
    return result;
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile\n")

int setup_tests(void)
{
    char *cert, *pkey;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(cert = test_get_argument(0))
            || !TEST_ptr(pkey = test_get_argument(1)))
        return 0;

    if (!create_ssl_ctx_pair(NULL, TLS_server_method(), TLS_client_method(),
                             TLS1_VERSION, 0,
                             &serverctx, &clientctx, cert, pkey)) {
        TEST_error("Failed to create SSL_CTX pair\n");
        return 0;
    }

    ADD_ALL_TESTS(test_func, 9);
#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DYNAMIC_ENGINE)
    ADD_ALL_TESTS(test_free_buffers, 8);
#else
    ADD_ALL_TESTS(test_free_buffers, 4);
#endif
    return 1;
}

void cleanup_tests(void)
{
    SSL_CTX_free(clientctx);
    SSL_CTX_free(serverctx);
}
