/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include <string.h>
#include "helpers/ssltestlib.h"
#include "testutil.h"
#include "internal/packet.h"

static char *cert = NULL;
static char *privkey = NULL;

static BIO *s_to_c_fbio = NULL, *c_to_s_fbio = NULL;
static int chseen = 0, shseen = 0, sccsseen = 0, ccsaftersh = 0;
static int ccsbeforesh = 0, sappdataseen = 0, cappdataseen = 0, badccs = 0;
static int badvers = 0, badsessid = 0;

static unsigned char chsessid[SSL_MAX_SSL_SESSION_ID_LENGTH];
static size_t chsessidlen = 0;

static int watchccs_new(BIO *bi);
static int watchccs_free(BIO *a);
static int watchccs_read(BIO *b, char *out, int outl);
static int watchccs_write(BIO *b, const char *in, int inl);
static long watchccs_ctrl(BIO *b, int cmd, long num, void *ptr);
static int watchccs_gets(BIO *bp, char *buf, int size);
static int watchccs_puts(BIO *bp, const char *str);

/* Choose a sufficiently large type likely to be unused for this custom BIO */
# define BIO_TYPE_WATCHCCS_FILTER  (0x80 | BIO_TYPE_FILTER)

static BIO_METHOD *method_watchccs = NULL;

static const BIO_METHOD *bio_f_watchccs_filter(void)
{
    if (method_watchccs == NULL) {
        method_watchccs = BIO_meth_new(BIO_TYPE_WATCHCCS_FILTER,
                                       "Watch CCS filter");
        if (method_watchccs == NULL
            || !BIO_meth_set_write(method_watchccs, watchccs_write)
            || !BIO_meth_set_read(method_watchccs, watchccs_read)
            || !BIO_meth_set_puts(method_watchccs, watchccs_puts)
            || !BIO_meth_set_gets(method_watchccs, watchccs_gets)
            || !BIO_meth_set_ctrl(method_watchccs, watchccs_ctrl)
            || !BIO_meth_set_create(method_watchccs, watchccs_new)
            || !BIO_meth_set_destroy(method_watchccs, watchccs_free))
            return NULL;
    }
    return method_watchccs;
}

static int watchccs_new(BIO *bio)
{
    BIO_set_init(bio, 1);
    return 1;
}

static int watchccs_free(BIO *bio)
{
    BIO_set_init(bio, 0);
    return 1;
}

static int watchccs_read(BIO *bio, char *out, int outl)
{
    int ret = 0;
    BIO *next = BIO_next(bio);

    if (outl <= 0)
        return 0;
    if (next == NULL)
        return 0;

    BIO_clear_retry_flags(bio);

    ret = BIO_read(next, out, outl);
    if (ret <= 0 && BIO_should_read(next))
        BIO_set_retry_read(bio);

    return ret;
}

static int watchccs_write(BIO *bio, const char *in, int inl)
{
    int ret = 0;
    BIO *next = BIO_next(bio);
    PACKET pkt, msg, msgbody, sessionid;
    unsigned int rectype, recvers, msgtype, expectedrecvers;

    if (inl <= 0)
        return 0;
    if (next == NULL)
        return 0;

    BIO_clear_retry_flags(bio);

    if (!PACKET_buf_init(&pkt, (const unsigned char *)in, inl))
        return 0;

    /* We assume that we always write complete records each time */
    while (PACKET_remaining(&pkt)) {
        if (!PACKET_get_1(&pkt, &rectype)
                || !PACKET_get_net_2(&pkt, &recvers)
                || !PACKET_get_length_prefixed_2(&pkt, &msg))
            return 0;

        expectedrecvers = TLS1_2_VERSION;

        if (rectype == SSL3_RT_HANDSHAKE) {
            if (!PACKET_get_1(&msg, &msgtype)
                    || !PACKET_get_length_prefixed_3(&msg, &msgbody))
                return 0;
            if (msgtype == SSL3_MT_CLIENT_HELLO) {
                chseen++;

                /*
                 * Skip legacy_version (2 bytes) and Random (32 bytes) to read
                 * session_id.
                 */
                if (!PACKET_forward(&msgbody, 34)
                        || !PACKET_get_length_prefixed_1(&msgbody, &sessionid))
                    return 0;

                if (chseen == 1) {
                    expectedrecvers = TLS1_VERSION;

                    /* Save the session id for later */
                    chsessidlen = PACKET_remaining(&sessionid);
                    if (!PACKET_copy_bytes(&sessionid, chsessid, chsessidlen))
                        return 0;
                } else {
                    /*
                     * Check the session id for the second ClientHello is the
                     * same as the first one.
                     */
                    if (PACKET_remaining(&sessionid) != chsessidlen
                            || (chsessidlen > 0
                                && memcmp(chsessid, PACKET_data(&sessionid),
                                          chsessidlen) != 0))
                        badsessid = 1;
                }
            } else if (msgtype == SSL3_MT_SERVER_HELLO) {
                shseen++;
                /*
                 * Skip legacy_version (2 bytes) and Random (32 bytes) to read
                 * session_id.
                 */
                if (!PACKET_forward(&msgbody, 34)
                        || !PACKET_get_length_prefixed_1(&msgbody, &sessionid))
                    return 0;

                /*
                 * Check the session id is the same as the one in the
                 * ClientHello
                 */
                if (PACKET_remaining(&sessionid) != chsessidlen
                        || (chsessidlen > 0
                            && memcmp(chsessid, PACKET_data(&sessionid),
                                      chsessidlen) != 0))
                    badsessid = 1;
            }
        } else if (rectype == SSL3_RT_CHANGE_CIPHER_SPEC) {
            if (bio == s_to_c_fbio) {
                /*
                 * Server writing. We shouldn't have written any app data
                 * yet, and we should have seen both the ClientHello and the
                 * ServerHello
                 */
                if (!sappdataseen
                        && chseen == 1
                        && shseen == 1
                        && !sccsseen)
                    sccsseen = 1;
                else
                    badccs = 1;
            } else if (!cappdataseen) {
                /*
                 * Client writing. We shouldn't have written any app data
                 * yet, and we should have seen the ClientHello
                 */
                if (shseen == 1 && !ccsaftersh)
                    ccsaftersh = 1;
                else if (shseen == 0 && !ccsbeforesh)
                    ccsbeforesh = 1;
                else
                    badccs = 1;
            } else {
                badccs = 1;
            }
        } else if (rectype == SSL3_RT_APPLICATION_DATA) {
            if (bio == s_to_c_fbio)
                sappdataseen = 1;
            else
                cappdataseen = 1;
        }
        if (recvers != expectedrecvers)
            badvers = 1;
    }

    ret = BIO_write(next, in, inl);
    if (ret <= 0 && BIO_should_write(next))
        BIO_set_retry_write(bio);

    return ret;
}

static long watchccs_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    long ret;
    BIO *next = BIO_next(bio);

    if (next == NULL)
        return 0;

    switch (cmd) {
    case BIO_CTRL_DUP:
        ret = 0;
        break;
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static int watchccs_gets(BIO *bio, char *buf, int size)
{
    /* We don't support this - not needed anyway */
    return -1;
}

static int watchccs_puts(BIO *bio, const char *str)
{
    return watchccs_write(bio, str, strlen(str));
}

static int test_tls13ccs(int tst)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *sssl = NULL, *cssl = NULL;
    int ret = 0;
    const char msg[] = "Dummy data";
    char buf[80];
    size_t written, readbytes;
    SSL_SESSION *sess = NULL;

    chseen = shseen = sccsseen = ccsaftersh = ccsbeforesh = 0;
    sappdataseen = cappdataseen = badccs = badvers = badsessid = 0;
    chsessidlen = 0;

    if (!TEST_true(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
        || !TEST_true(SSL_CTX_set_max_early_data(sctx,
                                                 SSL3_RT_MAX_PLAIN_LENGTH)))
        goto err;

    /*
     * Test 0: Simple Handshake
     * Test 1: Simple Handshake, client middlebox compat mode disabled
     * Test 2: Simple Handshake, server middlebox compat mode disabled
     * Test 3: HRR Handshake
     * Test 4: HRR Handshake, client middlebox compat mode disabled
     * Test 5: HRR Handshake, server middlebox compat mode disabled
     * Test 6: Early data handshake
     * Test 7: Early data handshake, client middlebox compat mode disabled
     * Test 8: Early data handshake, server middlebox compat mode disabled
     * Test 9: Early data then HRR
     * Test 10: Early data then HRR, client middlebox compat mode disabled
     * Test 11: Early data then HRR, server middlebox compat mode disabled
     */
    switch (tst) {
    case 0:
    case 3:
    case 6:
    case 9:
        break;
    case 1:
    case 4:
    case 7:
    case 10:
        SSL_CTX_clear_options(cctx, SSL_OP_ENABLE_MIDDLEBOX_COMPAT);
        break;
    case 2:
    case 5:
    case 8:
    case 11:
        SSL_CTX_clear_options(sctx, SSL_OP_ENABLE_MIDDLEBOX_COMPAT);
        break;
    default:
        TEST_error("Invalid test value");
        goto err;
    }

    if (tst >= 6) {
        /* Get a session suitable for early_data */
        if (!TEST_true(create_ssl_objects(sctx, cctx, &sssl, &cssl, NULL, NULL))
                || !TEST_true(create_ssl_connection(sssl, cssl, SSL_ERROR_NONE)))
            goto err;
        sess = SSL_get1_session(cssl);
        if (!TEST_ptr(sess))
            goto err;
        SSL_shutdown(cssl);
        SSL_shutdown(sssl);
        SSL_free(sssl);
        SSL_free(cssl);
        sssl = cssl = NULL;
    }

    if ((tst >= 3 && tst <= 5) || tst >= 9) {
        /* HRR handshake */
#if defined(OPENSSL_NO_EC)
# if !defined(OPENSSL_NO_DH)
        if (!TEST_true(SSL_CTX_set1_groups_list(sctx, "ffdhe3072")))
            goto err;
# endif
#else
        if (!TEST_true(SSL_CTX_set1_groups_list(sctx, "P-384")))
            goto err;
#endif
    }

    s_to_c_fbio = BIO_new(bio_f_watchccs_filter());
    c_to_s_fbio = BIO_new(bio_f_watchccs_filter());
    if (!TEST_ptr(s_to_c_fbio)
            || !TEST_ptr(c_to_s_fbio)) {
        BIO_free(s_to_c_fbio);
        BIO_free(c_to_s_fbio);
        goto err;
    }

    /* BIOs get freed on error */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &sssl, &cssl, s_to_c_fbio,
                                      c_to_s_fbio)))
        goto err;

    if (tst >= 6) {
        /* Early data */
        if (!TEST_true(SSL_set_session(cssl, sess))
                || !TEST_true(SSL_write_early_data(cssl, msg, strlen(msg),
                                                   &written))
                || (tst <= 8
                    && !TEST_int_eq(SSL_read_early_data(sssl, buf,  sizeof(buf),
                                                &readbytes),
                                                SSL_READ_EARLY_DATA_SUCCESS)))
            goto err;
        if (tst <= 8) {
            if (!TEST_int_gt(SSL_connect(cssl), 0))
                goto err;
        } else {
            if (!TEST_int_le(SSL_connect(cssl), 0))
                goto err;
        }
        if (!TEST_int_eq(SSL_read_early_data(sssl, buf,  sizeof(buf),
                                             &readbytes),
                         SSL_READ_EARLY_DATA_FINISH))
            goto err;
    }

    /* Perform handshake (or complete it if doing early data ) */
    if (!TEST_true(create_ssl_connection(sssl, cssl, SSL_ERROR_NONE)))
        goto err;

    /*
     * Check there were no unexpected CCS messages, all record versions
     * were as expected, and that the session ids were reflected by the server
     * correctly.
     */
    if (!TEST_false(badccs) || !TEST_false(badvers) || !TEST_false(badsessid))
        goto err;

    switch (tst) {
    case 0:
        if (!TEST_true(sccsseen)
                || !TEST_true(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 1:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_eq(chsessidlen, 0))
            goto err;
        break;

    case 2:
        if (!TEST_false(sccsseen)
                || !TEST_true(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 3:
        if (!TEST_true(sccsseen)
                || !TEST_true(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 4:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_eq(chsessidlen, 0))
            goto err;
        break;

    case 5:
        if (!TEST_false(sccsseen)
                || !TEST_true(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 6:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_true(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 7:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_eq(chsessidlen, 0))
            goto err;
        break;

    case 8:
        if (!TEST_false(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_true(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 9:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_true(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;

    case 10:
        if (!TEST_true(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_false(ccsbeforesh)
                || !TEST_size_t_eq(chsessidlen, 0))
            goto err;
        break;

    case 11:
        if (!TEST_false(sccsseen)
                || !TEST_false(ccsaftersh)
                || !TEST_true(ccsbeforesh)
                || !TEST_size_t_gt(chsessidlen, 0))
            goto err;
        break;
    }

    ret = 1;
 err:
    SSL_SESSION_free(sess);
    SSL_free(sssl);
    SSL_free(cssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return ret;
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

    ADD_ALL_TESTS(test_tls13ccs, 12);

    return 1;
}

void cleanup_tests(void)
{
    BIO_meth_free(method_watchccs);
}
