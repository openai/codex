/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/ssl.h>
#include <openssl/quic.h>
#include <openssl/bio.h>
#include "internal/common.h"
#include "internal/sockets.h"
#include "internal/quic_tserver.h"
#include "internal/quic_thread_assist.h"
#include "internal/quic_ssl.h"
#include "internal/time.h"
#include "testutil.h"

static const char msg1[] = "The quick brown fox jumped over the lazy dogs.";
static char msg2[1024], msg3[1024];
static OSSL_TIME fake_time;
static CRYPTO_RWLOCK *fake_time_lock;

static const char *certfile, *keyfile;

static int is_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);

    return ec == SSL_ERROR_WANT_READ || ec == SSL_ERROR_WANT_WRITE;
}

static unsigned char scratch_buf[2048];

static OSSL_TIME fake_now(void *arg)
{
    OSSL_TIME t;

    if (!CRYPTO_THREAD_read_lock(fake_time_lock))
        return ossl_time_zero();

    t = fake_time;

    CRYPTO_THREAD_unlock(fake_time_lock);
    return t;
}

static OSSL_TIME real_now(void *arg)
{
    return ossl_time_now();
}

static int do_test(int use_thread_assist, int use_fake_time, int use_inject)
{
    int testresult = 0, ret;
    int s_fd = -1, c_fd = -1;
    BIO *s_net_bio = NULL, *s_net_bio_own = NULL;
    BIO *c_net_bio = NULL, *c_net_bio_own = NULL;
    BIO *c_pair_own = NULL, *s_pair_own = NULL;
    QUIC_TSERVER_ARGS tserver_args = {0};
    QUIC_TSERVER *tserver = NULL;
    BIO_ADDR *s_addr_ = NULL;
    struct in_addr ina = {0};
    union BIO_sock_info_u s_info = {0};
    SSL_CTX *c_ctx = NULL;
    SSL *c_ssl = NULL;
    int c_connected = 0, c_write_done = 0, c_begin_read = 0, s_read_done = 0;
    int c_wait_eos = 0, c_done_eos = 0;
    int c_start_idle_test = 0, c_done_idle_test = 0;
    size_t l = 0, s_total_read = 0, s_total_written = 0, c_total_read = 0;
    size_t idle_units_done = 0;
    int s_begin_write = 0;
    OSSL_TIME start_time;
    unsigned char alpn[] = { 8, 'o', 's', 's', 'l', 't', 'e', 's', 't' };
    size_t limit_ms = 10000;

#if defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
    if (use_thread_assist) {
        TEST_skip("thread assisted mode not enabled");
        return 1;
    }
#endif

    ina.s_addr = htonl(0x7f000001UL);

    /* Setup test server. */
    s_fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
    if (!TEST_int_ge(s_fd, 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(s_fd, 1)))
        goto err;

    if (!TEST_ptr(s_addr_ = BIO_ADDR_new()))
        goto err;

    if (!TEST_true(BIO_ADDR_rawmake(s_addr_, AF_INET, &ina, sizeof(ina), 0)))
        goto err;

    if (!TEST_true(BIO_bind(s_fd, s_addr_, 0)))
        goto err;

    s_info.addr = s_addr_;
    if (!TEST_true(BIO_sock_info(s_fd, BIO_SOCK_INFO_ADDRESS, &s_info)))
        goto err;

    if (!TEST_int_gt(BIO_ADDR_rawport(s_addr_), 0))
        goto err;

    if (!TEST_ptr(s_net_bio = s_net_bio_own = BIO_new_dgram(s_fd, 0)))
        goto err;

    if (!BIO_up_ref(s_net_bio))
        goto err;

    fake_time = ossl_ms2time(1000);

    tserver_args.net_rbio = s_net_bio;
    tserver_args.net_wbio = s_net_bio;
    tserver_args.alpn = NULL;
    tserver_args.ctx = NULL;
    if (use_fake_time)
        tserver_args.now_cb = fake_now;

    if (!TEST_ptr(tserver = ossl_quic_tserver_new(&tserver_args, certfile,
                                                  keyfile))) {
        BIO_free(s_net_bio);
        goto err;
    }

    s_net_bio_own = NULL;

    if (use_inject) {
        /*
         * In inject mode we create a dgram pair to feed to the QUIC client on
         * the read side. We don't feed anything to this, it is just a
         * placeholder to give the client something which never returns any
         * datagrams.
         */
        if (!TEST_true(BIO_new_bio_dgram_pair(&c_pair_own, 5000,
                                              &s_pair_own, 5000)))
            goto err;
    }

    /* Setup test client. */
    c_fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
    if (!TEST_int_ge(c_fd, 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(c_fd, 1)))
        goto err;

    if (!TEST_ptr(c_net_bio = c_net_bio_own = BIO_new_dgram(c_fd, 0)))
        goto err;

    if (!BIO_dgram_set_peer(c_net_bio, s_addr_))
        goto err;

    if (!TEST_ptr(c_ctx = SSL_CTX_new(use_thread_assist
                                      ? OSSL_QUIC_client_thread_method()
                                      : OSSL_QUIC_client_method())))
        goto err;

    if (!TEST_ptr(c_ssl = SSL_new(c_ctx)))
        goto err;

    if (use_fake_time)
        if (!TEST_true(ossl_quic_set_override_now_cb(c_ssl, fake_now, NULL)))
            goto err;

    /* 0 is a success for SSL_set_alpn_protos() */
    if (!TEST_false(SSL_set_alpn_protos(c_ssl, alpn, sizeof(alpn))))
        goto err;

    /* Takes ownership of our reference to the BIO. */
    if (use_inject) {
        SSL_set0_rbio(c_ssl, c_pair_own);
        c_pair_own = NULL;
    } else {
        SSL_set0_rbio(c_ssl, c_net_bio);

        /* Get another reference to be transferred in the SSL_set0_wbio call. */
        if (!TEST_true(BIO_up_ref(c_net_bio))) {
            c_net_bio_own = NULL; /* SSL_free will free the first reference. */
            goto err;
        }
    }

    SSL_set0_wbio(c_ssl, c_net_bio);
    c_net_bio_own = NULL;

    if (!TEST_true(SSL_set_blocking_mode(c_ssl, 0)))
        goto err;

    /*
     * We use real time for the timeout not fake time. Otherwise with fake time
     * we could hit a hang if we never increment the fake time
     */
    start_time = real_now(NULL);

    for (;;) {
        if (ossl_time_compare(ossl_time_subtract(real_now(NULL), start_time),
                              ossl_ms2time(limit_ms)) >= 0) {
            TEST_error("timeout while attempting QUIC server test");
            goto err;
        }

        if (!c_start_idle_test) {
            ret = SSL_connect(c_ssl);
            if (!TEST_true(ret == 1 || is_want(c_ssl, ret)))
                goto err;

            if (ret == 1) {
                c_connected = 1;
            } else {
                /*
                 * keep timer ticking to keep handshake running.
                 * The timer is important for calculation of ping deadline.
                 * If things stall for whatever reason we at least send
                 * ACK eliciting ping to let peer know we are here ready
                 * to hear back.
                 */
                if (!TEST_true(CRYPTO_THREAD_write_lock(fake_time_lock)))
                    goto err;
                fake_time = ossl_time_add(fake_time, ossl_ms2time(100));
                CRYPTO_THREAD_unlock(fake_time_lock);
            }
        }

        if (c_connected && !c_write_done) {
            if (!TEST_int_eq(SSL_write(c_ssl, msg1, sizeof(msg1) - 1),
                             (int)sizeof(msg1) - 1))
                goto err;

            if (!TEST_true(SSL_stream_conclude(c_ssl, 0)))
                goto err;

            c_write_done = 1;
        }

        if (c_connected && c_write_done && !s_read_done) {
            if (!ossl_quic_tserver_read(tserver, 0,
                                        (unsigned char *)msg2 + s_total_read,
                                        sizeof(msg2) - s_total_read, &l)) {
                if (!TEST_true(ossl_quic_tserver_has_read_ended(tserver, 0)))
                    goto err;

                if (!TEST_mem_eq(msg1, sizeof(msg1) - 1, msg2, s_total_read))
                    goto err;

                s_begin_write = 1;
                s_read_done   = 1;
            } else {
                s_total_read += l;
                if (!TEST_size_t_le(s_total_read, sizeof(msg1) - 1))
                    goto err;
            }
        }

        if (s_begin_write && s_total_written < sizeof(msg1) - 1) {
            if (!TEST_true(ossl_quic_tserver_write(tserver, 0,
                                                   (unsigned char *)msg2 + s_total_written,
                                                   sizeof(msg1) - 1 - s_total_written, &l)))
                goto err;

            s_total_written += l;

            if (s_total_written == sizeof(msg1) - 1) {
                ossl_quic_tserver_conclude(tserver, 0);
                c_begin_read = 1;
            }
        }

        if (c_begin_read && c_total_read < sizeof(msg1) - 1) {
            ret = SSL_read_ex(c_ssl, msg3 + c_total_read,
                              sizeof(msg1) - 1 - c_total_read, &l);
            if (!TEST_true(ret == 1 || is_want(c_ssl, ret)))
                goto err;

            c_total_read += l;

            if (c_total_read == sizeof(msg1) - 1) {
                if (!TEST_mem_eq(msg1, sizeof(msg1) - 1,
                                 msg3, c_total_read))
                    goto err;

                c_wait_eos = 1;
            }
        }

        if (c_wait_eos && !c_done_eos) {
            unsigned char c;

            ret = SSL_read_ex(c_ssl, &c, sizeof(c), &l);
            if (!TEST_false(ret))
                goto err;

            /*
             * Allow the implementation to take as long as it wants to finally
             * notice EOS. Account for varied timings in OS networking stacks.
             */
            if (SSL_get_error(c_ssl, ret) != SSL_ERROR_WANT_READ) {
                if (!TEST_int_eq(SSL_get_error(c_ssl, ret),
                                 SSL_ERROR_ZERO_RETURN))
                    goto err;

                c_done_eos = 1;
                if (use_thread_assist && use_fake_time) {
                    if (!TEST_true(ossl_quic_tserver_is_connected(tserver)))
                        goto err;
                    c_start_idle_test = 1;
                    limit_ms = 120000; /* extend time limit */
                } else {
                    /* DONE */
                    break;
                }
            }
        }

        if (c_start_idle_test && !c_done_idle_test) {
            /* This is more than our default idle timeout of 30s. */
            if (idle_units_done < 600) {
                struct timeval tv;
                int isinf;

                if (!TEST_true(CRYPTO_THREAD_write_lock(fake_time_lock)))
                    goto err;
                fake_time = ossl_time_add(fake_time, ossl_ms2time(100));
                CRYPTO_THREAD_unlock(fake_time_lock);

                ++idle_units_done;
                ossl_quic_conn_force_assist_thread_wake(c_ssl);

                /*
                 * If the event timeout has expired then give the assistance
                 * thread a chance to catch up
                 */
                if (!TEST_true(SSL_get_event_timeout(c_ssl, &tv, &isinf)))
                    goto err;
                if (!isinf && ossl_time_compare(ossl_time_zero(),
                                                ossl_time_from_timeval(tv)) >= 0)
                    OSSL_sleep(100); /* Ensure CPU scheduling for test purposes */
            } else {
                c_done_idle_test = 1;
            }
        }

        if (c_done_idle_test) {
            /*
             * If we have finished the fake idling duration, the connection
             * should still be healthy in TA mode.
             */
            if (!TEST_true(ossl_quic_tserver_is_connected(tserver)))
                goto err;

            /* DONE */
            break;
        }

        /*
         * This is inefficient because we spin until things work without
         * blocking but this is just a test.
         */
        if (!c_start_idle_test || c_done_idle_test) {
            /* Inhibit manual ticking during idle test to test TA mode. */
            SSL_handle_events(c_ssl);
        }

        ossl_quic_tserver_tick(tserver);

        if (use_inject) {
            BIO_MSG rmsg = {0};
            size_t msgs_processed = 0;

            for (;;) {
                /*
                 * Manually spoonfeed received datagrams from the real BIO_dgram
                 * into QUIC via the injection interface, thereby testing the
                 * injection interface.
                 */
                rmsg.data       = scratch_buf;
                rmsg.data_len   = sizeof(scratch_buf);

                if (!BIO_recvmmsg(c_net_bio, &rmsg, sizeof(rmsg), 1, 0, &msgs_processed)
                    || msgs_processed == 0 || rmsg.data_len == 0)
                    break;

                if (!TEST_true(SSL_inject_net_dgram(c_ssl, rmsg.data, rmsg.data_len,
                                                    NULL, NULL)))
                    goto err;
            }
        }
    }

    testresult = 1;
err:
    SSL_free(c_ssl);
    SSL_CTX_free(c_ctx);
    ossl_quic_tserver_free(tserver);
    BIO_ADDR_free(s_addr_);
    BIO_free(s_net_bio_own);
    BIO_free(c_net_bio_own);
    BIO_free(c_pair_own);
    BIO_free(s_pair_own);
    if (s_fd >= 0)
        BIO_closesocket(s_fd);
    if (c_fd >= 0)
        BIO_closesocket(c_fd);
    return testresult;
}

static int test_tserver(int idx)
{
    int thread_assisted, use_fake_time, use_inject;

    thread_assisted = idx % 2;
    idx /= 2;

    use_inject = idx % 2;
    idx /= 2;

    use_fake_time = idx % 2;

    if (use_fake_time && !thread_assisted)
        return 1;

    return do_test(thread_assisted, use_fake_time, use_inject);
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(certfile = test_get_argument(0))
            || !TEST_ptr(keyfile = test_get_argument(1)))
        return 0;

    if ((fake_time_lock = CRYPTO_THREAD_lock_new()) == NULL)
        return 0;

    ADD_ALL_TESTS(test_tserver, 2 * 2 * 2);
    return 1;
}
