/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/configuration.h>
#include <openssl/bio.h>
#include "internal/e_os.h" /* For struct timeval */
#include "quictestlib.h"
#include "ssltestlib.h"
#include "../testutil.h"
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
# include "../threadstest.h"
#endif
#include "internal/quic_ssl.h"
#include "internal/quic_wire_pkt.h"
#include "internal/quic_record_tx.h"
#include "internal/quic_error.h"
#include "internal/packet.h"
#include "internal/tsan_assist.h"

#define GROWTH_ALLOWANCE 1024

struct noise_args_data_st {
    BIO *cbio;
    BIO *sbio;
    BIO *tracebio;
    int flags;
};

struct qtest_fault {
    QUIC_TSERVER *qtserv;

    /* Plain packet mutations */
    /* Header for the plaintext packet */
    QUIC_PKT_HDR pplainhdr;
    /* iovec for the plaintext packet data buffer */
    OSSL_QTX_IOVEC pplainio;
    /* Allocated size of the plaintext packet data buffer */
    size_t pplainbuf_alloc;
    qtest_fault_on_packet_plain_cb pplaincb;
    void *pplaincbarg;

    /* Handshake message mutations */
    /* Handshake message buffer */
    unsigned char *handbuf;
    /* Allocated size of the handshake message buffer */
    size_t handbufalloc;
    /* Actual length of the handshake message */
    size_t handbuflen;
    qtest_fault_on_handshake_cb handshakecb;
    void *handshakecbarg;
    qtest_fault_on_enc_ext_cb encextcb;
    void *encextcbarg;

    /* Cipher packet mutations */
    qtest_fault_on_packet_cipher_cb pciphercb;
    void *pciphercbarg;

    /* Datagram mutations */
    qtest_fault_on_datagram_cb datagramcb;
    void *datagramcbarg;
    /* The currently processed message */
    BIO_MSG msg;
    /* Allocated size of msg data buffer */
    size_t msgalloc;
    struct noise_args_data_st noiseargs;
};

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
static int client_ready = 0;
static CRYPTO_CONDVAR *client_ready_cond = NULL;
static CRYPTO_MUTEX *client_ready_mutex = NULL;
#endif

static void packet_plain_finish(void *arg);
static void handshake_finish(void *arg);
static OSSL_TIME qtest_get_time(void);
static void qtest_reset_time(void);

static int using_fake_time = 0;
static OSSL_TIME fake_now;
static CRYPTO_RWLOCK *fake_now_lock = NULL;
static OSSL_TIME start_time;

static OSSL_TIME fake_now_cb(void *arg)
{
    return qtest_get_time();
}

static void noise_msg_callback(int write_p, int version, int content_type,
                               const void *buf, size_t len, SSL *ssl,
                               void *arg)
{
    struct noise_args_data_st *noiseargs = (struct noise_args_data_st *)arg;

    if (content_type == SSL3_RT_QUIC_FRAME_FULL) {
        PACKET pkt;
        uint64_t frame_type;

        if (!PACKET_buf_init(&pkt, buf, len))
            return;

        if (!ossl_quic_wire_peek_frame_header(&pkt, &frame_type, NULL))
            return;

        if (frame_type == OSSL_QUIC_FRAME_TYPE_PING) {
            /*
             * If either endpoint issues a ping frame then we are in danger
             * of our noise being too much such that the connection itself
             * fails. We back off on the noise for a bit to avoid that.
             */
            (void)BIO_ctrl(noiseargs->cbio, BIO_CTRL_NOISE_BACK_OFF, 1, NULL);
            (void)BIO_ctrl(noiseargs->sbio, BIO_CTRL_NOISE_BACK_OFF, 1, NULL);
        }
    }

#ifndef OPENSSL_NO_SSL_TRACE
    if ((noiseargs->flags & QTEST_FLAG_CLIENT_TRACE) != 0
            && !SSL_is_server(ssl))
        SSL_trace(write_p, version, content_type, buf, len, ssl,
                  noiseargs->tracebio);
#endif
}

int qtest_create_quic_objects(OSSL_LIB_CTX *libctx, SSL_CTX *clientctx,
                              SSL_CTX *serverctx, char *certfile, char *keyfile,
                              int flags, QUIC_TSERVER **qtserv, SSL **cssl,
                              QTEST_FAULT **fault, BIO **tracebio)
{
    /* ALPN value as recognised by QUIC_TSERVER */
    unsigned char alpn[] = { 8, 'o', 's', 's', 'l', 't', 'e', 's', 't' };
    QUIC_TSERVER_ARGS tserver_args = {0};
    BIO *cbio = NULL, *sbio = NULL, *fisbio = NULL;
    BIO_ADDR *peeraddr = NULL;
    struct in_addr ina = {0};
    BIO *tmpbio = NULL;
    QTEST_DATA *bdata = NULL;

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
    if (client_ready_cond == NULL) {
        client_ready_cond = ossl_crypto_condvar_new();
        if (client_ready_cond == NULL)
            return 0;
    }

    if (client_ready_mutex == NULL) {
        client_ready_mutex = ossl_crypto_mutex_new();
        if (client_ready_mutex == NULL) {
            ossl_crypto_condvar_free(&client_ready_cond);
            client_ready_cond = NULL;
            return 0;
        }
    }
#endif

    bdata = OPENSSL_zalloc(sizeof(QTEST_DATA));
    if (bdata == NULL)
        return 0;

    *qtserv = NULL;

    if (fault != NULL) {
        *fault = OPENSSL_zalloc(sizeof(**fault));
        if (*fault == NULL)
            goto err;
        bdata->fault = *fault;
    }

    if (*cssl == NULL) {
        *cssl = SSL_new(clientctx);
        if (!TEST_ptr(*cssl))
            goto err;
    }

#ifndef OPENSSL_NO_SSL_TRACE
    if ((flags & QTEST_FLAG_CLIENT_TRACE) != 0) {
        tmpbio = BIO_new_fp(stdout, BIO_NOCLOSE);
        if (!TEST_ptr(tmpbio))
            goto err;

        SSL_set_msg_callback(*cssl, SSL_trace);
        SSL_set_msg_callback_arg(*cssl, tmpbio);
    }
#endif
    if (tracebio != NULL)
        *tracebio = tmpbio;

    /* SSL_set_alpn_protos returns 0 for success! */
    if (!TEST_false(SSL_set_alpn_protos(*cssl, alpn, sizeof(alpn))))
        goto err;

    if (!TEST_ptr(peeraddr = BIO_ADDR_new()))
        goto err;

    if ((flags & QTEST_FLAG_BLOCK) != 0) {
#if !defined(OPENSSL_NO_POSIX_IO)
        int cfd, sfd;

        /*
         * For blocking mode we need to create actual sockets rather than doing
         * everything in memory
         */
        if (!TEST_true(create_test_sockets(&cfd, &sfd, SOCK_DGRAM, peeraddr)))
            goto err;
        cbio = BIO_new_dgram(cfd, 1);
        if (!TEST_ptr(cbio)) {
            close(cfd);
            close(sfd);
            goto err;
        }
        sbio = BIO_new_dgram(sfd, 1);
        if (!TEST_ptr(sbio)) {
            close(sfd);
            goto err;
        }
#else
        goto err;
#endif
    } else {
        BIO_ADDR *localaddr = NULL;

        if (!TEST_true(BIO_new_bio_dgram_pair(&cbio, 0, &sbio, 0)))
            goto err;

        if (!TEST_true(BIO_dgram_set_caps(cbio, BIO_DGRAM_CAP_HANDLES_DST_ADDR))
                || !TEST_true(BIO_dgram_set_caps(sbio, BIO_DGRAM_CAP_HANDLES_DST_ADDR)))
            goto err;

        if (!TEST_ptr(localaddr = BIO_ADDR_new()))
            goto err;
        /* Dummy client local addresses */
        if (!TEST_true(BIO_ADDR_rawmake(localaddr, AF_INET, &ina, sizeof(ina),
                                        htons(0)))) {
            BIO_ADDR_free(localaddr);
            goto err;
        }
        if (!TEST_int_eq(BIO_dgram_set0_local_addr(cbio, localaddr), 1)) {
            BIO_ADDR_free(localaddr);
            goto err;
        }
        /* Dummy server address */
        if (!TEST_true(BIO_ADDR_rawmake(peeraddr, AF_INET, &ina, sizeof(ina),
                                        htons(0))))
            goto err;
    }

    if ((flags & QTEST_FLAG_PACKET_SPLIT) != 0) {
        BIO *pktsplitbio = BIO_new(bio_f_pkt_split_dgram_filter());

        if (!TEST_ptr(pktsplitbio))
            goto err;
        cbio = BIO_push(pktsplitbio, cbio);
        BIO_set_data(pktsplitbio, bdata);

        pktsplitbio = BIO_new(bio_f_pkt_split_dgram_filter());
        if (!TEST_ptr(pktsplitbio))
            goto err;
        sbio = BIO_push(pktsplitbio, sbio);
        BIO_set_data(pktsplitbio, bdata);
    }

    if ((flags & QTEST_FLAG_NOISE) != 0) {
        BIO *noisebio;
        struct bio_noise_now_cb_st now_cb = { fake_now_cb, NULL };

        /*
         * It is an error to not have a QTEST_FAULT object when introducing noise
         */
        if (!TEST_ptr(fault))
            goto err;

        noisebio = BIO_new(bio_f_noisy_dgram_filter());

        if (!TEST_ptr(noisebio))
            goto err;
        cbio = BIO_push(noisebio, cbio);
        if ((flags & QTEST_FLAG_FAKE_TIME) != 0) {
            if (!TEST_int_eq(BIO_ctrl(cbio, BIO_CTRL_NOISE_SET_NOW_CB,
                                      0, &now_cb), 1))
                goto err;
        }

        noisebio = BIO_new(bio_f_noisy_dgram_filter());

        if (!TEST_ptr(noisebio))
            goto err;
        sbio = BIO_push(noisebio, sbio);
        if ((flags & QTEST_FLAG_FAKE_TIME) != 0) {
            if (!TEST_int_eq(BIO_ctrl(sbio, BIO_CTRL_NOISE_SET_NOW_CB,
                                      0, &now_cb), 1))
                goto err;
        }

        (void)BIO_ctrl(sbio, BIO_CTRL_NOISE_BACK_OFF, 2, NULL);

        (*fault)->noiseargs.cbio = cbio;
        (*fault)->noiseargs.sbio = sbio;
        (*fault)->noiseargs.tracebio = tmpbio;
        (*fault)->noiseargs.flags = flags;

        SSL_set_msg_callback(*cssl, noise_msg_callback);
        SSL_set_msg_callback_arg(*cssl, &(*fault)->noiseargs);
    }

    SSL_set_bio(*cssl, cbio, cbio);

    if (!TEST_true(SSL_set_blocking_mode(*cssl,
                                         (flags & QTEST_FLAG_BLOCK) != 0 ? 1 : 0)))
        goto err;

    if (!TEST_true(SSL_set1_initial_peer_addr(*cssl, peeraddr)))
        goto err;

    fisbio = BIO_new(qtest_get_bio_method());
    if (!TEST_ptr(fisbio))
        goto err;

    BIO_set_data(fisbio, bdata);

    if (!BIO_up_ref(sbio))
        goto err;
    if (!TEST_ptr(BIO_push(fisbio, sbio))) {
        BIO_free(sbio);
        goto err;
    }

    tserver_args.libctx = libctx;
    tserver_args.net_rbio = sbio;
    tserver_args.net_wbio = fisbio;
    tserver_args.alpn = NULL;
    if (serverctx != NULL && !TEST_true(SSL_CTX_up_ref(serverctx)))
        goto err;
    tserver_args.ctx = serverctx;
    if (fake_now_lock == NULL) {
        fake_now_lock = CRYPTO_THREAD_lock_new();
        if (fake_now_lock == NULL)
            goto err;
    }
    if ((flags & QTEST_FLAG_FAKE_TIME) != 0) {
        using_fake_time = 1;
        qtest_reset_time();
        tserver_args.now_cb = fake_now_cb;
        (void)ossl_quic_set_override_now_cb(*cssl, fake_now_cb, NULL);
    } else {
        using_fake_time = 0;
    }

    if (!TEST_ptr(*qtserv = ossl_quic_tserver_new(&tserver_args, certfile,
                                                  keyfile)))
        goto err;

    bdata->short_conn_id_len = ossl_quic_tserver_get_short_header_conn_id_len(*qtserv);
    /* Ownership of fisbio and sbio is now held by *qtserv */
    sbio = NULL;
    fisbio = NULL;

    if ((flags & QTEST_FLAG_NOISE) != 0)
        ossl_quic_tserver_set_msg_callback(*qtserv, noise_msg_callback,
                                           &(*fault)->noiseargs);

    if (fault != NULL)
        (*fault)->qtserv = *qtserv;

    BIO_ADDR_free(peeraddr);

    return 1;
 err:
    SSL_CTX_free(tserver_args.ctx);
    BIO_ADDR_free(peeraddr);
    BIO_free_all(cbio);
    BIO_free_all(fisbio);
    BIO_free_all(sbio);
    SSL_free(*cssl);
    *cssl = NULL;
    ossl_quic_tserver_free(*qtserv);
    if (fault != NULL)
        OPENSSL_free(*fault);
    OPENSSL_free(bdata);
    BIO_free(tmpbio);
    if (tracebio != NULL)
        *tracebio = NULL;

    return 0;
}

void qtest_add_time(uint64_t millis)
{
    if (!CRYPTO_THREAD_write_lock(fake_now_lock))
        return;
    fake_now = ossl_time_add(fake_now, ossl_ms2time(millis));
    CRYPTO_THREAD_unlock(fake_now_lock);
}

static OSSL_TIME qtest_get_time(void)
{
    OSSL_TIME ret;

    if (!CRYPTO_THREAD_read_lock(fake_now_lock))
        return ossl_time_zero();
    ret = fake_now;
    CRYPTO_THREAD_unlock(fake_now_lock);
    return ret;
}

static void qtest_reset_time(void)
{
    if (!CRYPTO_THREAD_write_lock(fake_now_lock))
        return;
    fake_now = ossl_time_zero();
    CRYPTO_THREAD_unlock(fake_now_lock);
    /* zero time can have a special meaning, bump it */
    qtest_add_time(1);
}

void qtest_start_stopwatch(void)
{
    start_time = qtest_get_time();
}

uint64_t qtest_get_stopwatch_time(void)
{
    return ossl_time2ms(ossl_time_subtract(qtest_get_time(), start_time));
}

QTEST_FAULT *qtest_create_injector(QUIC_TSERVER *ts)
{
    QTEST_FAULT *f;

    f = OPENSSL_zalloc(sizeof(*f));
    if (f == NULL)
        return NULL;

    f->qtserv = ts;
    return f;

}

int qtest_supports_blocking(void)
{
#if !defined(OPENSSL_NO_POSIX_IO) && defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
    return 1;
#else
    return 0;
#endif
}

#define MAXLOOPS    1000

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
static int globserverret = 0;
static TSAN_QUALIFIER int abortserverthread = 0;
static QUIC_TSERVER *globtserv;
static const thread_t thread_zero;
static void run_server_thread(void)
{
    /*
     * This will operate in a busy loop because the server does not block,
     * but should be acceptable because it is local and we expect this to be
     * fast
     */
    globserverret = qtest_create_quic_connection(globtserv, NULL);
}
#endif

int qtest_wait_for_timeout(SSL *s, QUIC_TSERVER *qtserv)
{
    struct timeval tv;
    OSSL_TIME ctimeout, stimeout, mintimeout, now;
    int cinf;

    /* We don't need to wait in blocking mode */
    if (s == NULL || SSL_get_blocking_mode(s))
        return 1;

    /* Don't wait if either BIO has data waiting */
    if (BIO_pending(SSL_get_rbio(s)) > 0
            || BIO_pending(ossl_quic_tserver_get0_rbio(qtserv)) > 0)
        return 1;

    /*
     * Neither endpoint has data waiting to be read. We assume data transmission
     * is instantaneous due to using mem based BIOs, so there is no data "in
     * flight" and no more data will be sent by either endpoint until some time
     * based event has occurred. Therefore, wait for a timeout to occur. This
     * might happen if we are using the noisy BIO and datagrams have been lost.
     */
    if (!SSL_get_event_timeout(s, &tv, &cinf))
        return 0;

    if (using_fake_time)
        now = qtest_get_time();
    else
        now = ossl_time_now();

    ctimeout = cinf ? ossl_time_infinite() : ossl_time_from_timeval(tv);
    stimeout = ossl_time_subtract(ossl_quic_tserver_get_deadline(qtserv), now);
    mintimeout = ossl_time_min(ctimeout, stimeout);
    if (ossl_time_is_infinite(mintimeout))
        return 0;

    if (using_fake_time)
        qtest_add_time(ossl_time2ms(mintimeout));
    else
        OSSL_sleep(ossl_time2ms(mintimeout));

    return 1;
}

int qtest_create_quic_connection_ex(QUIC_TSERVER *qtserv, SSL *clientssl,
                                    int wanterr)
{
    int retc = -1, rets = 0, abortctr = 0, ret = 0;
    int clienterr = 0, servererr = 0;
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)

    /*
     * Pointless initialisation to avoid bogus compiler warnings about using
     * t uninitialised
     */
    thread_t t = thread_zero;

    if (clientssl != NULL)
        abortserverthread = 0;

    /*
     * Only set the client_ready flag to zero if we are the client
     */
    if (clientssl != NULL) {
        ossl_crypto_mutex_lock(client_ready_mutex);
        client_ready = 0;
        ossl_crypto_mutex_unlock(client_ready_mutex);
    }
#endif

    if (!TEST_ptr(qtserv)) {
        goto err;
    } else if (clientssl == NULL) {
        retc = 1;
    } else if (SSL_get_blocking_mode(clientssl) > 0) {
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
        /*
         * clientssl is blocking. We will need a thread to complete the
         * connection
         */
        globtserv = qtserv;
        if (!TEST_true(run_thread(&t, run_server_thread)))
            goto err;

        qtserv = NULL;
        rets = 1;
#else
        TEST_error("No thread support in this build");
        goto err;
#endif
    }

    do {
        if (!clienterr && retc <= 0) {
            int err;

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
            ossl_crypto_mutex_lock(client_ready_mutex);
            client_ready = 1;
            ossl_crypto_condvar_broadcast(client_ready_cond);
            ossl_crypto_mutex_unlock(client_ready_mutex);
#endif
            retc = SSL_connect(clientssl);
            if (retc <= 0) {
                err = SSL_get_error(clientssl, retc);

                if (err == wanterr) {
                    retc = 1;
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
                    if (qtserv == NULL && rets > 0)
                        tsan_store(&abortserverthread, 1);
                    else
#endif
                        rets = 1;
                } else {
                    if (err != SSL_ERROR_WANT_READ
                            && err != SSL_ERROR_WANT_WRITE) {
                        TEST_info("SSL_connect() failed %d, %d", retc, err);
                        TEST_openssl_errors();
                        clienterr = 1;
                    }
                }
            }
        }

        qtest_add_time(1);
        if (clientssl != NULL)
            SSL_handle_events(clientssl);
        if (qtserv != NULL) {
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
            ossl_crypto_mutex_lock(client_ready_mutex);
            for (;;) {
                if (client_ready == 1)
                    break;
                ossl_crypto_condvar_wait(client_ready_cond, client_ready_mutex);
            }
            ossl_crypto_mutex_unlock(client_ready_mutex);
#endif
            ossl_quic_tserver_tick(qtserv);
        }

        if (!servererr && rets <= 0) {
            servererr = ossl_quic_tserver_is_term_any(qtserv);
            if (!servererr)
                rets = ossl_quic_tserver_is_handshake_confirmed(qtserv);
        }

        if (clienterr && servererr)
            goto err;

        if (clientssl != NULL && ++abortctr == MAXLOOPS) {
            TEST_info("No progress made");
            goto err;
        }

        if ((retc <= 0 && !clienterr) || (rets <= 0 && !servererr)) {
            if (!qtest_wait_for_timeout(clientssl, qtserv))
                goto err;
        }
    } while ((retc <= 0 && !clienterr)
             || (rets <= 0 && !servererr
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
                 && !tsan_load(&abortserverthread)
#endif
                ));

    if (qtserv == NULL && rets > 0) {
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
        /*
         * Make sure we unblock the server before we wait on completion here
         * in case it didn't happen in the connect loop above
         */
        ossl_crypto_mutex_lock(client_ready_mutex);
        client_ready = 1;
        ossl_crypto_condvar_broadcast(client_ready_cond);
        ossl_crypto_mutex_unlock(client_ready_mutex);
        if (!TEST_true(wait_for_thread(t)) || !TEST_true(globserverret))
            goto err;
#else
        TEST_error("Should not happen");
        goto err;
#endif
    }

    if (!clienterr && !servererr)
        ret = 1;
 err:
    return ret;
}

int qtest_create_quic_connection(QUIC_TSERVER *qtserv, SSL *clientssl)
{
    return qtest_create_quic_connection_ex(qtserv, clientssl, SSL_ERROR_NONE);
}

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
static TSAN_QUALIFIER int shutdowndone;

static void run_server_shutdown_thread(void)
{
    /*
     * This will operate in a busy loop because the server does not block,
     * but should be acceptable because it is local and we expect this to be
     * fast
     */
    do {
        ossl_quic_tserver_tick(globtserv);
    } while(!tsan_load(&shutdowndone));
}
#endif

int qtest_shutdown(QUIC_TSERVER *qtserv, SSL *clientssl)
{
    int tickserver = 1;
    int ret = 0;
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
    /*
     * Pointless initialisation to avoid bogus compiler warnings about using
     * t uninitialised
     */
    thread_t t = thread_zero;

    ossl_crypto_condvar_free(&client_ready_cond);
    client_ready_cond = NULL;
    ossl_crypto_mutex_free(&client_ready_mutex);
    client_ready_mutex = NULL;
#endif

    if (SSL_get_blocking_mode(clientssl) > 0) {
#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
        /*
         * clientssl is blocking. We will need a thread to complete the
         * connection
         */
        globtserv = qtserv;
        shutdowndone = 0;
        if (!TEST_true(run_thread(&t, run_server_shutdown_thread)))
            return 0;

        tickserver = 0;
#else
        TEST_error("No thread support in this build");
        return 0;
#endif
    }

    /* Busy loop in non-blocking mode. It should be quick because its local */
    for (;;) {
        int rc = SSL_shutdown(clientssl);

        if (rc == 1) {
            ret = 1;
            break;
        }

        if (rc < 0)
            break;

        if (tickserver)
            ossl_quic_tserver_tick(qtserv);
    }

#if defined(OPENSSL_THREADS) && !defined(CRYPTO_TDEBUG)
    tsan_store(&shutdowndone, 1);
    if (!tickserver) {
        if (!TEST_true(wait_for_thread(t)))
            ret = 0;
    }
#endif

    return ret;
}

int qtest_check_server_transport_err(QUIC_TSERVER *qtserv, uint64_t code)
{
    const QUIC_TERMINATE_CAUSE *cause;

    ossl_quic_tserver_tick(qtserv);

    /*
     * Check that the server has closed with the specified code from the client
     */
    if (!TEST_true(ossl_quic_tserver_is_term_any(qtserv)))
        return 0;

    cause = ossl_quic_tserver_get_terminate_cause(qtserv);
    if  (!TEST_ptr(cause)
            || !TEST_true(cause->remote)
            || !TEST_false(cause->app)
            || !TEST_uint64_t_eq(cause->error_code, code))
        return 0;

    return 1;
}

int qtest_check_server_protocol_err(QUIC_TSERVER *qtserv)
{
    return qtest_check_server_transport_err(qtserv, OSSL_QUIC_ERR_PROTOCOL_VIOLATION);
}

int qtest_check_server_frame_encoding_err(QUIC_TSERVER *qtserv)
{
    return qtest_check_server_transport_err(qtserv, OSSL_QUIC_ERR_FRAME_ENCODING_ERROR);
}

void qtest_fault_free(QTEST_FAULT *fault)
{
    if (fault == NULL)
        return;

    packet_plain_finish(fault);
    handshake_finish(fault);

    OPENSSL_free(fault);
}

static int packet_plain_mutate(const QUIC_PKT_HDR *hdrin,
                               const OSSL_QTX_IOVEC *iovecin, size_t numin,
                               QUIC_PKT_HDR **hdrout,
                               const OSSL_QTX_IOVEC **iovecout,
                               size_t *numout,
                               void *arg)
{
    QTEST_FAULT *fault = arg;
    size_t i, bufsz = 0;
    unsigned char *cur;
    int grow_allowance;

    /* Coalesce our data into a single buffer */

    /* First calculate required buffer size */
    for (i = 0; i < numin; i++)
        bufsz += iovecin[i].buf_len;

    fault->pplainio.buf_len = bufsz;

    /*
     * 1200 is QUIC payload length we use
     * bufsz is what we got from txp
     * 16 is the length of tag added by encryption
     * 14 long header (we assume token length is 0,
     * which is fine for server not so fine for client)
     */
    grow_allowance = 1200 - bufsz - 16 - 14;
    grow_allowance -= hdrin->dst_conn_id.id_len;
    grow_allowance -= hdrin->src_conn_id.id_len;
    assert(grow_allowance >= 0);
    bufsz += grow_allowance;

    fault->pplainio.buf = cur = OPENSSL_malloc(bufsz);
    if (cur == NULL) {
        fault->pplainio.buf_len = 0;
        return 0;
    }

    fault->pplainbuf_alloc = bufsz;

    /* Copy in the data from the input buffers */
    for (i = 0; i < numin; i++) {
        memcpy(cur, iovecin[i].buf, iovecin[i].buf_len);
        cur += iovecin[i].buf_len;
    }

    fault->pplainhdr = *hdrin;

    /*
     * Cast below is safe because we allocated the buffer
     * mutation is best effort. we can inject frame if
     * there is enough space. If there is not enough space
     * we must give up.
     */
    if (fault->pplaincb != NULL)
        fault->pplaincb(fault, &fault->pplainhdr,
                        (unsigned char *)fault->pplainio.buf,
                        fault->pplainio.buf_len, fault->pplaincbarg);

    *hdrout = &fault->pplainhdr;
    *iovecout = &fault->pplainio;
    *numout = 1;

    return 1;
}

static void packet_plain_finish(void *arg)
{
    QTEST_FAULT *fault = arg;

    /* Cast below is safe because we allocated the buffer */
    OPENSSL_free((unsigned char *)fault->pplainio.buf);
    fault->pplainio.buf_len = 0;
    fault->pplainbuf_alloc = 0;
    fault->pplainio.buf = NULL;
}

int qtest_fault_set_packet_plain_listener(QTEST_FAULT *fault,
                                          qtest_fault_on_packet_plain_cb pplaincb,
                                          void *pplaincbarg)
{
    fault->pplaincb = pplaincb;
    fault->pplaincbarg = pplaincbarg;

    return ossl_quic_tserver_set_plain_packet_mutator(fault->qtserv,
                                                      packet_plain_mutate,
                                                      packet_plain_finish,
                                                      fault);
}

/* To be called from a packet_plain_listener callback */
int qtest_fault_resize_plain_packet(QTEST_FAULT *fault, size_t newlen)
{
    unsigned char *buf;
    size_t oldlen = fault->pplainio.buf_len;

    /*
     * Alloc'd size should always be non-zero, so if this fails we've been
     * incorrectly called
     */
    if (fault->pplainbuf_alloc == 0)
        return 0;

    if (newlen > fault->pplainbuf_alloc) {
        /* This exceeds our growth allowance. Fail */
        return 0;
    }

    /* Cast below is safe because we allocated the buffer */
    buf = (unsigned char *)fault->pplainio.buf;

    if (newlen > oldlen) {
        /* Extend packet with 0 bytes */
        memset(buf + oldlen, 0, newlen - oldlen);
    } /* else we're truncating or staying the same */

    fault->pplainio.buf_len = newlen;
    fault->pplainhdr.len = newlen;

    return 1;
}

/*
 * Prepend frame data into a packet. To be called from a packet_plain_listener
 * callback
 */
int qtest_fault_prepend_frame(QTEST_FAULT *fault, const unsigned char *frame,
                              size_t frame_len)
{
    unsigned char *buf;
    size_t old_len;

    /*
     * Alloc'd size should always be non-zero, so if this fails we've been
     * incorrectly called
     */
    if (fault->pplainbuf_alloc == 0)
        return 0;

    /* Cast below is safe because we allocated the buffer */
    buf = (unsigned char *)fault->pplainio.buf;
    old_len = fault->pplainio.buf_len;

    /* Extend the size of the packet by the size of the new frame */
    if (!TEST_true(qtest_fault_resize_plain_packet(fault,
                                                   old_len + frame_len)))
        return 0;

    memmove(buf + frame_len, buf, old_len);
    memcpy(buf, frame, frame_len);

    return 1;
}

static int handshake_mutate(const unsigned char *msgin, size_t msginlen,
                            unsigned char **msgout, size_t *msgoutlen,
                            void *arg)
{
    QTEST_FAULT *fault = arg;
    unsigned char *buf;
    unsigned long payloadlen;
    unsigned int msgtype;
    PACKET pkt;

    buf = OPENSSL_malloc(msginlen + GROWTH_ALLOWANCE);
    if (buf == NULL)
        return 0;

    fault->handbuf = buf;
    fault->handbuflen = msginlen;
    fault->handbufalloc = msginlen + GROWTH_ALLOWANCE;
    memcpy(buf, msgin, msginlen);

    if (!PACKET_buf_init(&pkt, buf, msginlen)
            || !PACKET_get_1(&pkt, &msgtype)
            || !PACKET_get_net_3(&pkt, &payloadlen)
            || PACKET_remaining(&pkt) != payloadlen)
        return 0;

    /* Parse specific message types */
    switch (msgtype) {
    case SSL3_MT_ENCRYPTED_EXTENSIONS:
    {
        QTEST_ENCRYPTED_EXTENSIONS ee;

        if (fault->encextcb == NULL)
            break;

        /*
         * The EncryptedExtensions message is very simple. It just has an
         * extensions block in it and nothing else.
         */
        ee.extensions = (unsigned char *)PACKET_data(&pkt);
        ee.extensionslen = payloadlen;
        if (!fault->encextcb(fault, &ee, payloadlen, fault->encextcbarg))
            return 0;
    }

    default:
        /* No specific handlers for these message types yet */
        break;
    }

    if (fault->handshakecb != NULL
            && !fault->handshakecb(fault, buf, fault->handbuflen,
                                   fault->handshakecbarg))
        return 0;

    *msgout = buf;
    *msgoutlen = fault->handbuflen;

    return 1;
}

static void handshake_finish(void *arg)
{
    QTEST_FAULT *fault = arg;

    OPENSSL_free(fault->handbuf);
    fault->handbuf = NULL;
}

int qtest_fault_set_handshake_listener(QTEST_FAULT *fault,
                                       qtest_fault_on_handshake_cb handshakecb,
                                       void *handshakecbarg)
{
    fault->handshakecb = handshakecb;
    fault->handshakecbarg = handshakecbarg;

    return ossl_quic_tserver_set_handshake_mutator(fault->qtserv,
                                                   handshake_mutate,
                                                   handshake_finish,
                                                   fault);
}

int qtest_fault_set_hand_enc_ext_listener(QTEST_FAULT *fault,
                                          qtest_fault_on_enc_ext_cb encextcb,
                                          void *encextcbarg)
{
    fault->encextcb = encextcb;
    fault->encextcbarg = encextcbarg;

    return ossl_quic_tserver_set_handshake_mutator(fault->qtserv,
                                                   handshake_mutate,
                                                   handshake_finish,
                                                   fault);
}

/* To be called from a handshake_listener callback */
int qtest_fault_resize_handshake(QTEST_FAULT *fault, size_t newlen)
{
    unsigned char *buf;
    size_t oldlen = fault->handbuflen;

    /*
     * Alloc'd size should always be non-zero, so if this fails we've been
     * incorrectly called
     */
    if (fault->handbufalloc == 0)
        return 0;

    if (newlen > fault->handbufalloc) {
        /* This exceeds our growth allowance. Fail */
        return 0;
    }

    buf = (unsigned char *)fault->handbuf;

    if (newlen > oldlen) {
        /* Extend packet with 0 bytes */
        memset(buf + oldlen, 0, newlen - oldlen);
    } /* else we're truncating or staying the same */

    fault->handbuflen = newlen;
    return 1;
}

/* To be called from message specific listener callbacks */
int qtest_fault_resize_message(QTEST_FAULT *fault, size_t newlen)
{
    /* First resize the underlying message */
    if (!qtest_fault_resize_handshake(fault, newlen + SSL3_HM_HEADER_LENGTH))
        return 0;

    /* Fixup the handshake message header */
    fault->handbuf[1] = (unsigned char)((newlen >> 16) & 0xff);
    fault->handbuf[2] = (unsigned char)((newlen >>  8) & 0xff);
    fault->handbuf[3] = (unsigned char)((newlen      ) & 0xff);

    return 1;
}

int qtest_fault_delete_extension(QTEST_FAULT *fault,
                                 unsigned int exttype, unsigned char *ext,
                                 size_t *extlen,
                                 BUF_MEM *old_ext)
{
    PACKET pkt, sub, subext;
    WPACKET old_ext_wpkt;
    unsigned int type;
    const unsigned char *start, *end;
    size_t newlen, w;
    size_t msglen = fault->handbuflen;

    if (!PACKET_buf_init(&pkt, ext, *extlen))
        return 0;

    /* Extension block starts with 2 bytes for extension block length */
    if (!PACKET_as_length_prefixed_2(&pkt, &sub))
        return 0;

    do {
        start = PACKET_data(&sub);
        if (!PACKET_get_net_2(&sub, &type)
                || !PACKET_get_length_prefixed_2(&sub, &subext))
            return 0;
    } while (type != exttype);

    /* Found it */
    end = PACKET_data(&sub);

    if (old_ext != NULL) {
        if (!WPACKET_init(&old_ext_wpkt, old_ext))
            return 0;

        if (!WPACKET_memcpy(&old_ext_wpkt, PACKET_data(&subext),
                            PACKET_remaining(&subext))
            || !WPACKET_get_total_written(&old_ext_wpkt, &w)) {
            WPACKET_cleanup(&old_ext_wpkt);
            return 0;
        }

        WPACKET_finish(&old_ext_wpkt);
        old_ext->length = w;
    }

    /*
     * If we're not the last extension we need to move the rest earlier. The
     * cast below is safe because we own the underlying buffer and we're no
     * longer making PACKET calls.
     */
    if (end < ext + *extlen)
        memmove((unsigned char *)start, end, end - start);

    /*
     * Calculate new extensions payload length =
     * Original length
     * - 2 extension block length bytes
     * - length of removed extension
     */
    newlen = *extlen - 2 - (end - start);

    /* Fixup the length bytes for the extension block */
    ext[0] = (unsigned char)((newlen >> 8) & 0xff);
    ext[1] = (unsigned char)((newlen     ) & 0xff);

    /*
     * Length of the whole extension block is the new payload length plus the
     * 2 bytes for the length
     */
    *extlen = newlen + 2;

    /* We can now resize the message */
    if ((size_t)(end - start) + SSL3_HM_HEADER_LENGTH > msglen)
        return 0; /* Should not happen */
    msglen -= (end - start) + SSL3_HM_HEADER_LENGTH;
    if (!qtest_fault_resize_message(fault, msglen))
        return 0;

    return 1;
}

#define BIO_TYPE_CIPHER_PACKET_FILTER  (0x80 | BIO_TYPE_FILTER)

static BIO_METHOD *pcipherbiometh = NULL;

# define BIO_MSG_N(array, stride, n) (*(BIO_MSG *)((char *)(array) + (n)*(stride)))

static int pcipher_sendmmsg(BIO *b, BIO_MSG *msg, size_t stride,
                            size_t num_msg, uint64_t flags,
                            size_t *num_processed)
{
    BIO *next = BIO_next(b);
    ossl_ssize_t ret = 0;
    size_t i = 0, tmpnump;
    QUIC_PKT_HDR hdr;
    PACKET pkt;
    unsigned char *tmpdata;
    QTEST_DATA *bdata = NULL;

    if (next == NULL)
        return 0;

    bdata = BIO_get_data(b);
    if (bdata == NULL || bdata->fault == NULL
            || (bdata->fault->pciphercb == NULL && bdata->fault->datagramcb == NULL))
        return BIO_sendmmsg(next, msg, stride, num_msg, flags, num_processed);

    if (num_msg == 0) {
        *num_processed = 0;
        return 1;
    }

    for (i = 0; i < num_msg; ++i) {
        bdata->fault->msg = BIO_MSG_N(msg, stride, i);

        /* Take a copy of the data so that callbacks can modify it */
        tmpdata = OPENSSL_malloc(bdata->fault->msg.data_len + GROWTH_ALLOWANCE);
        if (tmpdata == NULL)
            return 0;
        memcpy(tmpdata, bdata->fault->msg.data, bdata->fault->msg.data_len);
        bdata->fault->msg.data = tmpdata;
        bdata->fault->msgalloc = bdata->fault->msg.data_len + GROWTH_ALLOWANCE;

        if (bdata->fault->pciphercb != NULL) {
            if (!PACKET_buf_init(&pkt, bdata->fault->msg.data, bdata->fault->msg.data_len))
                return 0;

            do {
                if (!ossl_quic_wire_decode_pkt_hdr(&pkt,
                                                   bdata->short_conn_id_len,
                                                   1, 0, &hdr, NULL, NULL))
                    goto out;

                /*
                 * hdr.data is const - but its our buffer so casting away the
                 * const is safe
                 */
                if (!bdata->fault->pciphercb(bdata->fault, &hdr,
                                             (unsigned char *)hdr.data, hdr.len,
                                             bdata->fault->pciphercbarg))
                    goto out;

                /*
                 * At the moment modifications to hdr by the callback
                 * are ignored. We might need to rewrite the QUIC header to
                 * enable tests to change this. We also don't yet have a
                 * mechanism for the callback to change the encrypted data
                 * length. It's not clear if that's needed or not.
                 */
            } while (PACKET_remaining(&pkt) > 0);
        }

        if (bdata->fault->datagramcb != NULL
                && !bdata->fault->datagramcb(bdata->fault, &bdata->fault->msg, stride,
                                             bdata->fault->datagramcbarg))
            goto out;

        if (!BIO_sendmmsg(next, &bdata->fault->msg, stride, 1, flags, &tmpnump)) {
            *num_processed = i;
            goto out;
        }

        OPENSSL_free(bdata->fault->msg.data);
        bdata->fault->msg.data = NULL;
        bdata->fault->msgalloc = 0;
    }

    *num_processed = i;
out:
    ret = i > 0;
    OPENSSL_free(bdata->fault->msg.data);
    bdata->fault->msg.data = NULL;
    return ret;
}

static long pcipher_ctrl(BIO *b, int cmd, long larg, void *parg)
{
    BIO *next = BIO_next(b);

    if (next == NULL)
        return -1;

    return BIO_ctrl(next, cmd, larg, parg);
}

static int pcipher_destroy(BIO *b)
{
    OPENSSL_free(BIO_get_data(b));
    return 1;
}

BIO_METHOD *qtest_get_bio_method(void)
{
    BIO_METHOD *tmp;

    if (pcipherbiometh != NULL)
        return pcipherbiometh;

    tmp = BIO_meth_new(BIO_TYPE_CIPHER_PACKET_FILTER, "Cipher Packet Filter");

    if (!TEST_ptr(tmp))
        return NULL;

    if (!TEST_true(BIO_meth_set_sendmmsg(tmp, pcipher_sendmmsg))
            || !TEST_true(BIO_meth_set_ctrl(tmp, pcipher_ctrl))
            || !TEST_true(BIO_meth_set_destroy(tmp, pcipher_destroy)))
        goto err;

    pcipherbiometh = tmp;
    tmp = NULL;
 err:
    BIO_meth_free(tmp);
    return pcipherbiometh;
}

int qtest_fault_set_packet_cipher_listener(QTEST_FAULT *fault,
                                           qtest_fault_on_packet_cipher_cb pciphercb,
                                           void *pciphercbarg)
{
    fault->pciphercb = pciphercb;
    fault->pciphercbarg = pciphercbarg;

    return 1;
}

int qtest_fault_set_datagram_listener(QTEST_FAULT *fault,
                                      qtest_fault_on_datagram_cb datagramcb,
                                      void *datagramcbarg)
{
    fault->datagramcb = datagramcb;
    fault->datagramcbarg = datagramcbarg;

    return 1;
}

/* To be called from a datagram_listener callback */
int qtest_fault_resize_datagram(QTEST_FAULT *fault, size_t newlen)
{
    if (newlen > fault->msgalloc)
            return 0;

    if (newlen > fault->msg.data_len)
        memset((unsigned char *)fault->msg.data + fault->msg.data_len, 0,
                newlen - fault->msg.data_len);

    fault->msg.data_len = newlen;

    return 1;
}

int qtest_fault_set_bw_limit(QTEST_FAULT *fault,
                             size_t ctos_bw, size_t stoc_bw,
                             int noise_rate)
{
    BIO *sbio = fault->noiseargs.sbio;
    BIO *cbio = fault->noiseargs.cbio;

    if (!TEST_ptr(sbio) || !TEST_ptr(cbio))
        return 0;
    if (!TEST_int_eq(BIO_ctrl(sbio, BIO_CTRL_NOISE_RATE, noise_rate, NULL), 1))
        return 0;
    if (!TEST_int_eq(BIO_ctrl(cbio, BIO_CTRL_NOISE_RATE, noise_rate, NULL), 1))
        return 0;
    /* We set the bandwidth limit on the sending side */
    if (!TEST_int_eq(BIO_ctrl(cbio, BIO_CTRL_NOISE_SEND_BANDWIDTH,
                              (long)ctos_bw, NULL), 1))
        return 0;
    if (!TEST_int_eq(BIO_ctrl(sbio, BIO_CTRL_NOISE_SEND_BANDWIDTH,
                              (long)stoc_bw, NULL), 1))
        return 0;
    return 1;
}


int bio_msg_copy(BIO_MSG *dst, BIO_MSG *src)
{
    /*
     * Note it is assumed that the originally allocated data sizes for dst and
     * src are the same
     */
    memcpy(dst->data, src->data, src->data_len);
    dst->data_len = src->data_len;
    dst->flags = src->flags;
    if (dst->local != NULL) {
        if (src->local != NULL) {
            if (!TEST_true(BIO_ADDR_copy(dst->local, src->local)))
                return 0;
        } else {
            BIO_ADDR_clear(dst->local);
        }
    }
    if (!TEST_true(BIO_ADDR_copy(dst->peer, src->peer)))
        return 0;

    return 1;
}
