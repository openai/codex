/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/ssl.h>
#include "helpers/quictestlib.h"
#include "internal/quic_error.h"
#include "testutil.h"

static char *cert = NULL;
static char *privkey = NULL;

/*
 * Inject NEW_CONNECTION_ID frame
 */
static size_t ncid_injected;
static int add_ncid_frame_cb(QTEST_FAULT *fault, QUIC_PKT_HDR *hdr,
                             unsigned char *buf, size_t len, void *cbarg)
{
    /*
     * We inject NEW_CONNECTION_ID frame to trigger change of the DCID.
     * The connection id length must be 8, otherwise the tserver won't be
     * able to receive packets with this new id.
     */
    static unsigned char new_conn_id_frame[] = {
        0x18,                           /* Type */
        0x01,                           /* Sequence Number */
        0x01,                           /* Retire Prior To */
        0x08,                           /* Connection ID Length */
        0x33, 0x44, 0x55, 0x66, 0xde, 0xad, 0xbe, 0xef, /* Connection ID */
        0xab, 0xcd, 0xef, 0x01, 0x12, 0x32, 0x23, 0x45, /* Stateless Reset Token */
        0x56, 0x06, 0x08, 0x89, 0xa1, 0xb2, 0xc3, 0xd4
    };

    /* We only ever add the unknown frame to one packet */
    if (ncid_injected++)
        return 1;

    return qtest_fault_prepend_frame(fault, new_conn_id_frame,
                                     sizeof(new_conn_id_frame));
}

static int test_ncid_frame(int fail)
{
    int testresult = 0;
    SSL_CTX *cctx = SSL_CTX_new(OSSL_QUIC_client_method());
    QUIC_TSERVER *qtserv = NULL;
    SSL *cssl = NULL;
    char *msg = "Hello World!";
    size_t msglen = strlen(msg);
    unsigned char buf[80];
    size_t byteswritten;
    size_t bytesread;
    QTEST_FAULT *fault = NULL;
    static const QUIC_CONN_ID conn_id = {
        0x08,
        {0x33, 0x44, 0x55, 0x66, 0xde, 0xad, 0xbe, 0xef}
    };

    ncid_injected = 0;
    if (!TEST_ptr(cctx))
        goto err;

    if (!TEST_true(qtest_create_quic_objects(NULL, cctx, NULL, cert, privkey, 0,
                                             &qtserv, &cssl, &fault, NULL)))
        goto err;

    if (!TEST_true(qtest_create_quic_connection(qtserv, cssl)))
        goto err;

    if (!TEST_int_eq(SSL_write(cssl, msg, msglen), msglen))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    if (!TEST_true(ossl_quic_tserver_read(qtserv, 0, buf, sizeof(buf),
                                          &bytesread)))
        goto err;

    /*
     * We assume the entire message is read from the server in one go. In
     * theory this could get fragmented but its a small message so we assume
     * not.
     */
    if (!TEST_mem_eq(msg, msglen, buf, bytesread))
        goto err;

    /*
     * Write a message from the server to the client and add
     * a NEW_CONNECTION_ID frame.
     */
    if (!TEST_true(qtest_fault_set_packet_plain_listener(fault,
                                                         add_ncid_frame_cb,
                                                         NULL)))
        goto err;
    if (!fail && !TEST_true(ossl_quic_tserver_set_new_local_cid(qtserv, &conn_id)))
        goto err;
    if (!TEST_true(ossl_quic_tserver_write(qtserv, 0,
                                           (unsigned char *)msg, msglen,
                                           &byteswritten)))
        goto err;

    if (!TEST_true(ncid_injected))
        goto err;

    if (!TEST_size_t_eq(msglen, byteswritten))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    if (!TEST_true(SSL_handle_events(cssl)))
        goto err;

    if (!TEST_int_eq(SSL_read(cssl, buf, sizeof(buf)), msglen))
        goto err;

    if (!TEST_mem_eq(msg, msglen, buf, bytesread))
        goto err;

    if (!TEST_int_eq(SSL_write(cssl, msg, msglen), msglen))
        goto err;

    ossl_quic_tserver_tick(qtserv);
    if (!TEST_true(ossl_quic_tserver_read(qtserv, 0, buf, sizeof(buf),
                                          &bytesread)))
        goto err;

    if (fail) {
        if (!TEST_size_t_eq(bytesread, 0))
            goto err;
    } else {
        if (!TEST_mem_eq(msg, msglen, buf, bytesread))
            goto err;
    }

    testresult = 1;
 err:
    qtest_fault_free(fault);
    SSL_free(cssl);
    ossl_quic_tserver_free(qtserv);
    SSL_CTX_free(cctx);
    return testresult;
}

OPT_TEST_DECLARE_USAGE("certsdir\n")

int setup_tests(void)
{
    char *certsdir = NULL;

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

    ADD_ALL_TESTS(test_ncid_frame, 2);

    return 1;

 err:
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    return 0;
}

void cleanup_tests(void)
{
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
}
