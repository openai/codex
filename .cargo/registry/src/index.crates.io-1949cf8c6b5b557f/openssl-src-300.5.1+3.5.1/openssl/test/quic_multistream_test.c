/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/ssl.h>
#include <openssl/quic.h>
#include <openssl/bio.h>
#include <openssl/lhash.h>
#include <openssl/rand.h>
#include "internal/quic_tserver.h"
#include "internal/quic_ssl.h"
#include "internal/quic_error.h"
#include "internal/quic_stream_map.h"
#include "internal/quic_engine.h"
#include "testutil.h"
#include "helpers/quictestlib.h"
#if defined(OPENSSL_THREADS)
# include "internal/thread_arch.h"
#endif
#include "internal/numbers.h"  /* UINT64_C */

static const char *certfile, *keyfile;

#if defined(_AIX)
/*
 * Some versions of AIX define macros for events and revents for use when
 * accessing pollfd structures (see Github issue #24236). That interferes
 * with our use of these names here. We simply undef them.
 */
# undef revents
# undef events
#endif

#if defined(OPENSSL_THREADS)
struct child_thread_args {
    struct helper *h;
    const struct script_op *script;
    const char *script_name;
    int thread_idx;

    CRYPTO_THREAD *t;
    CRYPTO_MUTEX *m;
    int testresult;
    int done;
    int s_checked_out;
};
#endif

typedef struct stream_info {
    const char      *name;
    SSL             *c_stream;
    uint64_t        s_stream_id;
} STREAM_INFO;

DEFINE_LHASH_OF_EX(STREAM_INFO);

struct helper {
    int                     s_fd;
    BIO                     *s_net_bio, *s_net_bio_own, *s_qtf_wbio, *s_qtf_wbio_own;
    /* The BIO_ADDR used for BIO_bind() */
    BIO_ADDR                *s_net_bio_orig_addr;
    /* The resulting address, which is the one to connect to */
    BIO_ADDR                *s_net_bio_addr;

    /*
     * When doing a blocking mode test run, s_priv always points to the TSERVER
     * and s is NULL when the main thread should not be touching s_priv.
     */
    QUIC_TSERVER            *s, *s_priv;
    LHASH_OF(STREAM_INFO)   *s_streams;

    int                     c_fd;
    BIO                     *c_net_bio, *c_net_bio_own;
    SSL_CTX                 *c_ctx;
    SSL                     *c_conn;
    LHASH_OF(STREAM_INFO)   *c_streams;

#if defined(OPENSSL_THREADS)
    struct child_thread_args    *threads;
    size_t                      num_threads;
    CRYPTO_MUTEX		*misc_m;
    CRYPTO_CONDVAR		*misc_cv;
#endif

    OSSL_TIME       start_time;

    /*
     * This is a duration recording the amount of time we have skipped forwards
     * for testing purposes relative to the real ossl_time_now() clock. We add
     * a quantity of time to this every time we skip some time.
     */
    CRYPTO_RWLOCK   *time_lock;
    OSSL_TIME       time_slip; /* protected by time_lock */

    QTEST_FAULT     *qtf;

    int             init, blocking, check_spin_again;
    int             free_order, need_injector;

    int (*qtf_packet_plain_cb)(struct helper *h, QUIC_PKT_HDR *hdr,
                               unsigned char *buf, size_t buf_len);
    int (*qtf_handshake_cb)(struct helper *h,
                            unsigned char *buf, size_t buf_len);
    int (*qtf_datagram_cb)(struct helper *h,
                           BIO_MSG *m, size_t stride);
    uint64_t inject_word0, inject_word1;
    uint64_t scratch0, scratch1, fail_count;
#if defined(OPENSSL_THREADS)
    struct {
        CRYPTO_THREAD   *t;
        CRYPTO_MUTEX    *m;
        CRYPTO_CONDVAR  *c;
        int             ready, stop;
    } server_thread;
    int s_checked_out;
#endif
};

struct helper_local {
    struct helper           *h;
    LHASH_OF(STREAM_INFO)   *c_streams;
    int                     thread_idx;
    const struct script_op  *check_op;
    int                     explicit_event_handling;
};

struct script_op {
    uint32_t        op;
    const void      *arg0;
    size_t          arg1;
    int             (*check_func)(struct helper *h, struct helper_local *hl);
    const char      *stream_name;
    uint64_t        arg2;
    int             (*qtf_packet_plain_cb)(struct helper *h, QUIC_PKT_HDR *hdr,
                                           unsigned char *buf, size_t buf_len);
    int             (*qtf_handshake_cb)(struct helper *h,
                                        unsigned char *buf, size_t buf_len);
    int             (*qtf_datagram_cb)(struct helper *h,
                                       BIO_MSG *m, size_t stride);
};

#define OPK_END                                     0
#define OPK_CHECK                                   1
#define OPK_C_SET_ALPN                              2
#define OPK_C_CONNECT_WAIT                          3
#define OPK_C_WRITE                                 4
#define OPK_S_WRITE                                 5
#define OPK_C_READ_EXPECT                           6
#define OPK_S_READ_EXPECT                           7
#define OPK_C_EXPECT_FIN                            8
#define OPK_S_EXPECT_FIN                            9
#define OPK_C_CONCLUDE                              10
#define OPK_S_CONCLUDE                              11
#define OPK_C_DETACH                                12
#define OPK_C_ATTACH                                13
#define OPK_C_NEW_STREAM                            14
#define OPK_S_NEW_STREAM                            15
#define OPK_C_ACCEPT_STREAM_WAIT                    16
#define OPK_C_ACCEPT_STREAM_NONE                    17
#define OPK_C_FREE_STREAM                           18
#define OPK_C_SET_DEFAULT_STREAM_MODE               19
#define OPK_C_SET_INCOMING_STREAM_POLICY            20
#define OPK_C_SHUTDOWN_WAIT                         21
#define OPK_C_EXPECT_CONN_CLOSE_INFO                22
#define OPK_S_EXPECT_CONN_CLOSE_INFO                23
#define OPK_S_BIND_STREAM_ID                        24
#define OPK_C_WAIT_FOR_DATA                         25
#define OPK_C_WRITE_FAIL                            26
#define OPK_S_WRITE_FAIL                            27
#define OPK_C_READ_FAIL                             28
#define OPK_C_STREAM_RESET                          29
#define OPK_S_ACCEPT_STREAM_WAIT                    30
#define OPK_NEW_THREAD                              31
#define OPK_BEGIN_REPEAT                            32
#define OPK_END_REPEAT                              33
#define OPK_S_UNBIND_STREAM_ID                      34
#define OPK_C_READ_FAIL_WAIT                        35
#define OPK_C_CLOSE_SOCKET                          36
#define OPK_C_EXPECT_SSL_ERR                        37
#define OPK_EXPECT_ERR_REASON                       38
#define OPK_EXPECT_ERR_LIB                          39
#define OPK_SLEEP                                   40
#define OPK_S_READ_FAIL                             41
#define OPK_S_SET_INJECT_PLAIN                      42
#define OPK_SET_INJECT_WORD                         43
#define OPK_C_INHIBIT_TICK                          44
#define OPK_C_SET_WRITE_BUF_SIZE                    45
#define OPK_S_SET_INJECT_HANDSHAKE                  46
#define OPK_S_NEW_TICKET                            47
#define OPK_C_SKIP_IF_UNBOUND                       48
#define OPK_S_SET_INJECT_DATAGRAM                   49
#define OPK_S_SHUTDOWN                              50
#define OPK_POP_ERR                                 51
#define OPK_C_WRITE_EX2                             52
#define OPK_SKIP_IF_BLOCKING                        53
#define OPK_C_STREAM_RESET_FAIL                     54

#define EXPECT_CONN_CLOSE_APP       (1U << 0)
#define EXPECT_CONN_CLOSE_REMOTE    (1U << 1)

/* OPK_C_NEW_STREAM */
#define ALLOW_FAIL (1U << 16)

#define C_BIDI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_CLIENT | QUIC_STREAM_DIR_BIDI)
#define S_BIDI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_SERVER | QUIC_STREAM_DIR_BIDI)
#define C_UNI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_CLIENT | QUIC_STREAM_DIR_UNI)
#define S_UNI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_SERVER | QUIC_STREAM_DIR_UNI)

#define ANY_ID UINT64_MAX

#define OP_END  \
    {OPK_END}
#define OP_CHECK(func, arg2)  \
    {OPK_CHECK, NULL, 0, (func), NULL, (arg2)},
#define OP_C_SET_ALPN(alpn) \
    {OPK_C_SET_ALPN, (alpn), 0, NULL, NULL},
#define OP_C_CONNECT_WAIT() \
    {OPK_C_CONNECT_WAIT, NULL, 0, NULL, NULL},
#define OP_C_CONNECT_WAIT_OR_FAIL() \
    {OPK_C_CONNECT_WAIT, NULL, 1, NULL, NULL},
#define OP_C_WRITE(stream_name, buf, buf_len)   \
    {OPK_C_WRITE, (buf), (buf_len), NULL, #stream_name},
#define OP_S_WRITE(stream_name, buf, buf_len)   \
    {OPK_S_WRITE, (buf), (buf_len), NULL, #stream_name},
#define OP_C_READ_EXPECT(stream_name, buf, buf_len)   \
    {OPK_C_READ_EXPECT, (buf), (buf_len), NULL, #stream_name},
#define OP_S_READ_EXPECT(stream_name, buf, buf_len)   \
    {OPK_S_READ_EXPECT, (buf), (buf_len), NULL, #stream_name},
#define OP_C_EXPECT_FIN(stream_name) \
    {OPK_C_EXPECT_FIN, NULL, 0, NULL, #stream_name},
#define OP_S_EXPECT_FIN(stream_name) \
    {OPK_S_EXPECT_FIN, NULL, 0, NULL, #stream_name},
#define OP_C_CONCLUDE(stream_name) \
    {OPK_C_CONCLUDE, NULL, 0, NULL, #stream_name},
#define OP_S_CONCLUDE(stream_name) \
    {OPK_S_CONCLUDE, NULL, 0, NULL, #stream_name},
#define OP_C_DETACH(stream_name) \
    {OPK_C_DETACH, NULL, 0, NULL, #stream_name},
#define OP_C_ATTACH(stream_name) \
    {OPK_C_ATTACH, NULL, 0, NULL, #stream_name},
#define OP_C_NEW_STREAM_BIDI(stream_name, expect_id) \
    {OPK_C_NEW_STREAM, NULL, 0, NULL, #stream_name, (expect_id)},
#define OP_C_NEW_STREAM_BIDI_EX(stream_name, expect_id, flags) \
    {OPK_C_NEW_STREAM, NULL, (flags), NULL, #stream_name, (expect_id)},
#define OP_C_NEW_STREAM_UNI(stream_name, expect_id) \
    {OPK_C_NEW_STREAM, NULL, SSL_STREAM_FLAG_UNI, \
     NULL, #stream_name, (expect_id)},
#define OP_C_NEW_STREAM_UNI_EX(stream_name, expect_id, flags) \
    {OPK_C_NEW_STREAM, NULL, (flags) | SSL_STREAM_FLAG_UNI, \
     NULL, #stream_name, (expect_id)},
#define OP_S_NEW_STREAM_BIDI(stream_name, expect_id) \
    {OPK_S_NEW_STREAM, NULL, 0, NULL, #stream_name, (expect_id)},
#define OP_S_NEW_STREAM_UNI(stream_name, expect_id) \
    {OPK_S_NEW_STREAM, NULL, 1, NULL, #stream_name, (expect_id)},
#define OP_C_ACCEPT_STREAM_WAIT(stream_name) \
    {OPK_C_ACCEPT_STREAM_WAIT, NULL, 0, NULL, #stream_name},
#define OP_C_ACCEPT_STREAM_NONE() \
    {OPK_C_ACCEPT_STREAM_NONE, NULL, 0, NULL, NULL},
#define OP_C_FREE_STREAM(stream_name) \
    {OPK_C_FREE_STREAM, NULL, 0, NULL, #stream_name},
#define OP_C_SET_DEFAULT_STREAM_MODE(mode) \
    {OPK_C_SET_DEFAULT_STREAM_MODE, NULL, (mode), NULL, NULL},
#define OP_C_SET_INCOMING_STREAM_POLICY(policy) \
    {OPK_C_SET_INCOMING_STREAM_POLICY, NULL, (policy), NULL, NULL},
#define OP_C_SHUTDOWN_WAIT(reason, flags) \
    {OPK_C_SHUTDOWN_WAIT, (reason), (flags), NULL, NULL},
#define OP_C_EXPECT_CONN_CLOSE_INFO(ec, app, remote)                \
    {OPK_C_EXPECT_CONN_CLOSE_INFO, NULL,                            \
        ((app) ? EXPECT_CONN_CLOSE_APP : 0) |                       \
        ((remote) ? EXPECT_CONN_CLOSE_REMOTE : 0),                  \
        NULL, NULL, (ec)},
#define OP_S_EXPECT_CONN_CLOSE_INFO(ec, app, remote) \
    {OPK_S_EXPECT_CONN_CLOSE_INFO, NULL,            \
        ((app) ? EXPECT_CONN_CLOSE_APP : 0) |       \
        ((remote) ? EXPECT_CONN_CLOSE_REMOTE : 0),  \
        NULL, NULL, (ec)},
#define OP_S_BIND_STREAM_ID(stream_name, stream_id) \
    {OPK_S_BIND_STREAM_ID, NULL, 0, NULL, #stream_name, (stream_id)},
#define OP_C_WAIT_FOR_DATA(stream_name) \
    {OPK_C_WAIT_FOR_DATA, NULL, 0, NULL, #stream_name},
#define OP_C_WRITE_FAIL(stream_name)  \
    {OPK_C_WRITE_FAIL, NULL, 0, NULL, #stream_name},
#define OP_S_WRITE_FAIL(stream_name)  \
    {OPK_S_WRITE_FAIL, NULL, 0, NULL, #stream_name},
#define OP_C_READ_FAIL(stream_name)  \
    {OPK_C_READ_FAIL, NULL, 0, NULL, #stream_name},
#define OP_S_READ_FAIL(stream_name, allow_zero_len)  \
    {OPK_S_READ_FAIL, NULL, (allow_zero_len), NULL, #stream_name},
#define OP_C_STREAM_RESET(stream_name, aec)  \
    {OPK_C_STREAM_RESET, NULL, 0, NULL, #stream_name, (aec)},
#define OP_C_STREAM_RESET_FAIL(stream_name, aec)  \
    {OPK_C_STREAM_RESET_FAIL, NULL, 0, NULL, #stream_name, (aec)},
#define OP_S_ACCEPT_STREAM_WAIT(stream_name)  \
    {OPK_S_ACCEPT_STREAM_WAIT, NULL, 0, NULL, #stream_name},
#define OP_NEW_THREAD(num_threads, script) \
    {OPK_NEW_THREAD, (script), (num_threads), NULL, NULL, 0 },
#define OP_BEGIN_REPEAT(n)  \
    {OPK_BEGIN_REPEAT, NULL, (n)},
#define OP_END_REPEAT() \
    {OPK_END_REPEAT},
#define OP_S_UNBIND_STREAM_ID(stream_name) \
    {OPK_S_UNBIND_STREAM_ID, NULL, 0, NULL, #stream_name},
#define OP_C_READ_FAIL_WAIT(stream_name) \
    {OPK_C_READ_FAIL_WAIT, NULL, 0, NULL, #stream_name},
#define OP_C_CLOSE_SOCKET() \
    {OPK_C_CLOSE_SOCKET},
#define OP_C_EXPECT_SSL_ERR(stream_name, err) \
    {OPK_C_EXPECT_SSL_ERR, NULL, (err), NULL, #stream_name},
#define OP_EXPECT_ERR_REASON(err) \
    {OPK_EXPECT_ERR_REASON, NULL, (err)},
#define OP_EXPECT_ERR_LIB(lib) \
    {OPK_EXPECT_ERR_LIB, NULL, (lib)},
#define OP_SLEEP(ms) \
    {OPK_SLEEP, NULL, 0, NULL, NULL, (ms)},
#define OP_S_SET_INJECT_PLAIN(f) \
    {OPK_S_SET_INJECT_PLAIN, NULL, 0, NULL, NULL, 0, (f)},
#define OP_SET_INJECT_WORD(w0, w1) \
    {OPK_SET_INJECT_WORD, NULL, (w0), NULL, NULL, (w1), NULL},
#define OP_C_INHIBIT_TICK(inhibit) \
    {OPK_C_INHIBIT_TICK, NULL, (inhibit), NULL, NULL, 0, NULL},
#define OP_C_SET_WRITE_BUF_SIZE(stream_name, size) \
    {OPK_C_SET_WRITE_BUF_SIZE, NULL, (size), NULL, #stream_name},
#define OP_S_SET_INJECT_HANDSHAKE(f) \
    {OPK_S_SET_INJECT_HANDSHAKE, NULL, 0, NULL, NULL, 0, NULL, (f)},
#define OP_S_NEW_TICKET() \
    {OPK_S_NEW_TICKET},
#define OP_C_SKIP_IF_UNBOUND(stream_name, n) \
    {OPK_C_SKIP_IF_UNBOUND, NULL, (n), NULL, #stream_name},
#define OP_S_SET_INJECT_DATAGRAM(f) \
    {OPK_S_SET_INJECT_DATAGRAM, NULL, 0, NULL, NULL, 0, NULL, NULL, (f)},
#define OP_S_SHUTDOWN(error_code) \
    {OPK_S_SHUTDOWN, NULL, (error_code)},
#define OP_POP_ERR() \
    {OPK_POP_ERR},
#define OP_C_WRITE_EX2(stream_name, buf, buf_len, flags) \
    {OPK_C_WRITE_EX2, (buf), (buf_len), NULL, #stream_name, (flags)},
#define OP_CHECK2(func, arg1, arg2) \
    {OPK_CHECK, NULL, (arg1), (func), NULL, (arg2)},
#define OP_SKIP_IF_BLOCKING(n) \
    {OPK_SKIP_IF_BLOCKING, NULL, (n), NULL, 0},

static OSSL_TIME get_time(void *arg)
{
    struct helper *h = arg;
    OSSL_TIME t;

    if (!TEST_true(CRYPTO_THREAD_read_lock(h->time_lock)))
        return ossl_time_zero();

    t = ossl_time_add(ossl_time_now(), h->time_slip);

    CRYPTO_THREAD_unlock(h->time_lock);
    return t;
}

static int skip_time_ms(struct helper *h, struct helper_local *hl)
{
    if (!TEST_true(CRYPTO_THREAD_write_lock(h->time_lock)))
        return 0;

    h->time_slip = ossl_time_add(h->time_slip, ossl_ms2time(hl->check_op->arg2));

    CRYPTO_THREAD_unlock(h->time_lock);
    return 1;
}

static QUIC_TSERVER *s_lock(struct helper *h, struct helper_local *hl);
static void s_unlock(struct helper *h, struct helper_local *hl);

#define ACQUIRE_S() s_lock(h, hl)
#define ACQUIRE_S_NOHL() s_lock(h, NULL)

static int check_rejected(struct helper *h, struct helper_local *hl)
{
    uint64_t stream_id = hl->check_op->arg2;

    if (!ossl_quic_tserver_stream_has_peer_stop_sending(ACQUIRE_S(), stream_id, NULL)
        || !ossl_quic_tserver_stream_has_peer_reset_stream(ACQUIRE_S(), stream_id, NULL)) {
        h->check_spin_again = 1;
        return 0;
    }

    return 1;
}

static int check_stream_reset(struct helper *h, struct helper_local *hl)
{
    uint64_t stream_id = hl->check_op->arg2, aec = 0;

    if (!ossl_quic_tserver_stream_has_peer_reset_stream(ACQUIRE_S(), stream_id, &aec)) {
        h->check_spin_again = 1;
        return 0;
    }

    return TEST_uint64_t_eq(aec, 42);
}

static int check_stream_stopped(struct helper *h, struct helper_local *hl)
{
    uint64_t stream_id = hl->check_op->arg2;

    if (!ossl_quic_tserver_stream_has_peer_stop_sending(ACQUIRE_S(), stream_id, NULL)) {
        h->check_spin_again = 1;
        return 0;
    }

    return 1;
}

static int override_key_update(struct helper *h, struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);

    ossl_quic_channel_set_txku_threshold_override(ch, hl->check_op->arg2);
    return 1;
}

static int trigger_key_update(struct helper *h, struct helper_local *hl)
{
    if (!TEST_true(SSL_key_update(h->c_conn, SSL_KEY_UPDATE_REQUESTED)))
        return 0;

    return 1;
}

static int check_key_update_ge(struct helper *h, struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);
    int64_t txke = (int64_t)ossl_quic_channel_get_tx_key_epoch(ch);
    int64_t rxke = (int64_t)ossl_quic_channel_get_rx_key_epoch(ch);
    int64_t diff = txke - rxke;

    /*
     * TXKE must always be equal to or ahead of RXKE.
     * It can be ahead of RXKE by at most 1.
     */
    if (!TEST_int64_t_ge(diff, 0) || !TEST_int64_t_le(diff, 1))
        return 0;

    /* Caller specifies a minimum number of RXKEs which must have happened. */
    if (!TEST_uint64_t_ge((uint64_t)rxke, hl->check_op->arg2))
        return 0;

    return 1;
}

static int check_key_update_lt(struct helper *h, struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);
    uint64_t txke = ossl_quic_channel_get_tx_key_epoch(ch);

    /* Caller specifies a maximum number of TXKEs which must have happened. */
    if (!TEST_uint64_t_lt(txke, hl->check_op->arg2))
        return 0;

    return 1;
}

static unsigned long stream_info_hash(const STREAM_INFO *info)
{
    return OPENSSL_LH_strhash(info->name);
}

static int stream_info_cmp(const STREAM_INFO *a, const STREAM_INFO *b)
{
    return strcmp(a->name, b->name);
}

static void cleanup_stream(STREAM_INFO *info)
{
    SSL_free(info->c_stream);
    OPENSSL_free(info);
}

static void helper_cleanup_streams(LHASH_OF(STREAM_INFO) **lh)
{
    if (*lh == NULL)
        return;

    lh_STREAM_INFO_doall(*lh, cleanup_stream);
    lh_STREAM_INFO_free(*lh);
    *lh = NULL;
}

#if defined(OPENSSL_THREADS)
static CRYPTO_THREAD_RETVAL run_script_child_thread(void *arg);

static int join_threads(struct child_thread_args *threads, size_t num_threads)
{
    int ok = 1;
    size_t i;
    CRYPTO_THREAD_RETVAL rv;

    for (i = 0; i < num_threads; ++i) {
        if (threads[i].t != NULL) {
            ossl_crypto_thread_native_join(threads[i].t, &rv);

            if (!threads[i].testresult)
                /* Do not log failure here, worker will do it. */
                ok = 0;

            ossl_crypto_thread_native_clean(threads[i].t);
            threads[i].t = NULL;
        }

        ossl_crypto_mutex_free(&threads[i].m);
    }

    return ok;
}

static int join_server_thread(struct helper *h)
{
    CRYPTO_THREAD_RETVAL rv;

    if (h->server_thread.t == NULL)
        return 1;

    ossl_crypto_mutex_lock(h->server_thread.m);
    h->server_thread.stop = 1;
    ossl_crypto_condvar_signal(h->server_thread.c);
    ossl_crypto_mutex_unlock(h->server_thread.m);

    ossl_crypto_thread_native_join(h->server_thread.t, &rv);
    ossl_crypto_thread_native_clean(h->server_thread.t);
    h->server_thread.t = NULL;
    return 1;
}

/* Ensure the server-state lock is currently held. Idempotent. */
static int *s_checked_out_p(struct helper *h, int thread_idx)
{
    return (thread_idx < 0) ? &h->s_checked_out
        : &h->threads[thread_idx].s_checked_out;
}

static QUIC_TSERVER *s_lock(struct helper *h, struct helper_local *hl)
{
    int *p_checked_out = s_checked_out_p(h, hl == NULL ? -1 : hl->thread_idx);

    if (h->server_thread.m == NULL || *p_checked_out)
        return h->s;

    ossl_crypto_mutex_lock(h->server_thread.m);
    h->s = h->s_priv;
    *p_checked_out = 1;
    return h->s;
}

/* Ensure the server-state lock is currently not held. Idempotent. */
static void s_unlock(struct helper *h, struct helper_local *hl)
{
    int *p_checked_out = s_checked_out_p(h, hl->thread_idx);

    if (h->server_thread.m == NULL || !*p_checked_out)
        return;

    *p_checked_out = 0;
    h->s = NULL;
    ossl_crypto_mutex_unlock(h->server_thread.m);
}

static unsigned int server_helper_thread(void *arg)
{
    struct helper *h = arg;

    ossl_crypto_mutex_lock(h->server_thread.m);

    for (;;) {
        int ready, stop;

        ready   = h->server_thread.ready;
        stop    = h->server_thread.stop;

        if (stop)
            break;

        if (!ready) {
            ossl_crypto_condvar_wait(h->server_thread.c, h->server_thread.m);
            continue;
        }

        ossl_quic_tserver_tick(h->s_priv);
        ossl_crypto_mutex_unlock(h->server_thread.m);
        /*
         * Give the main thread an opportunity to get the mutex, which is
         * sometimes necessary in some script operations.
         */
        OSSL_sleep(1);
        ossl_crypto_mutex_lock(h->server_thread.m);
    }

    ossl_crypto_mutex_unlock(h->server_thread.m);
    return 1;
}

#else

static QUIC_TSERVER *s_lock(struct helper *h, struct helper_local *hl)
{
    return h->s;
}

static void s_unlock(struct helper *h, struct helper_local *hl)
{}

#endif

static void helper_cleanup(struct helper *h)
{
#if defined(OPENSSL_THREADS)
    join_threads(h->threads, h->num_threads);
    join_server_thread(h);
    OPENSSL_free(h->threads);
    h->threads = NULL;
    h->num_threads = 0;
#endif

    if (h->free_order == 0) {
        /* order 0: streams, then conn */
        helper_cleanup_streams(&h->c_streams);

        SSL_free(h->c_conn);
        h->c_conn = NULL;
    } else {
        /* order 1: conn, then streams */
        SSL_free(h->c_conn);
        h->c_conn = NULL;

        helper_cleanup_streams(&h->c_streams);
    }

    helper_cleanup_streams(&h->s_streams);
    ossl_quic_tserver_free(h->s_priv);
    h->s_priv = h->s = NULL;

    BIO_free(h->s_net_bio_own);
    h->s_net_bio_own = NULL;

    BIO_free(h->c_net_bio_own);
    h->c_net_bio_own = NULL;

    BIO_free(h->s_qtf_wbio_own);
    h->s_qtf_wbio_own = NULL;

    qtest_fault_free(h->qtf);
    h->qtf = NULL;

    if (h->s_fd >= 0) {
        BIO_closesocket(h->s_fd);
        h->s_fd = -1;
    }

    if (h->c_fd >= 0) {
        BIO_closesocket(h->c_fd);
        h->c_fd = -1;
    }

    BIO_ADDR_free(h->s_net_bio_addr);
    h->s_net_bio_addr = NULL;
    BIO_ADDR_free(h->s_net_bio_orig_addr);
    h->s_net_bio_orig_addr = NULL;

    SSL_CTX_free(h->c_ctx);
    h->c_ctx = NULL;

    CRYPTO_THREAD_lock_free(h->time_lock);
    h->time_lock = NULL;

#if defined(OPENSSL_THREADS)
    ossl_crypto_mutex_free(&h->misc_m);
    ossl_crypto_condvar_free(&h->misc_cv);
    ossl_crypto_mutex_free(&h->server_thread.m);
    ossl_crypto_condvar_free(&h->server_thread.c);
#endif
}

static int helper_init(struct helper *h, const char *script_name,
                       int free_order, int blocking,
                       int need_injector)
{
    struct in_addr ina = {0};
    QUIC_TSERVER_ARGS s_args = {0};
    union BIO_sock_info_u info;
    char title[128];
    QTEST_DATA *bdata = NULL;

    memset(h, 0, sizeof(*h));
    h->c_fd = -1;
    h->s_fd = -1;
    h->free_order = free_order;
    h->blocking = blocking;
    h->need_injector = need_injector;
    h->time_slip = ossl_time_zero();

    bdata = OPENSSL_zalloc(sizeof(QTEST_DATA));
    if (bdata == NULL)
        goto err;

    if (!TEST_ptr(h->time_lock = CRYPTO_THREAD_lock_new()))
        goto err;

    if (!TEST_ptr(h->s_streams = lh_STREAM_INFO_new(stream_info_hash,
                                                    stream_info_cmp)))
        goto err;

    if (!TEST_ptr(h->c_streams = lh_STREAM_INFO_new(stream_info_hash,
                                                    stream_info_cmp)))
        goto err;

    ina.s_addr = htonl(0x7f000001UL);

    h->s_fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
    if (!TEST_int_ge(h->s_fd, 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(h->s_fd, 1)))
        goto err;

    if (!TEST_ptr(h->s_net_bio_orig_addr = BIO_ADDR_new())
        || !TEST_ptr(h->s_net_bio_addr = BIO_ADDR_new()))
        goto err;

    if (!TEST_true(BIO_ADDR_rawmake(h->s_net_bio_orig_addr, AF_INET,
                                    &ina, sizeof(ina), 0)))
        goto err;

    if (!TEST_true(BIO_bind(h->s_fd, h->s_net_bio_orig_addr, 0)))
        goto err;

    info.addr = h->s_net_bio_addr;
    if (!TEST_true(BIO_sock_info(h->s_fd, BIO_SOCK_INFO_ADDRESS, &info)))
        goto err;

    if (!TEST_int_gt(BIO_ADDR_rawport(h->s_net_bio_addr), 0))
        goto err;

    if  (!TEST_ptr(h->s_net_bio = h->s_net_bio_own = BIO_new_dgram(h->s_fd, 0)))
        goto err;

    if (!BIO_up_ref(h->s_net_bio))
        goto err;

    if (need_injector) {
        h->s_qtf_wbio = h->s_qtf_wbio_own = BIO_new(qtest_get_bio_method());
        if (!TEST_ptr(h->s_qtf_wbio))
            goto err;

        if (!TEST_ptr(BIO_push(h->s_qtf_wbio, h->s_net_bio)))
            goto err;

        s_args.net_wbio = h->s_qtf_wbio;
    } else {
        s_args.net_wbio = h->s_net_bio;
    }

    s_args.net_rbio     = h->s_net_bio;
    s_args.alpn         = NULL;
    s_args.now_cb       = get_time;
    s_args.now_cb_arg   = h;
    s_args.ctx          = NULL;

    if (!TEST_ptr(h->s_priv = ossl_quic_tserver_new(&s_args, certfile, keyfile)))
        goto err;

    if (!blocking)
        h->s = h->s_priv;

    if (need_injector) {
        h->qtf = qtest_create_injector(h->s_priv);
        if (!TEST_ptr(h->qtf))
            goto err;
        bdata->fault = h->qtf;
        BIO_set_data(h->s_qtf_wbio, bdata);
    }

    h->s_net_bio_own = NULL;
    h->s_qtf_wbio_own = NULL;

    h->c_fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
    if (!TEST_int_ge(h->c_fd, 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(h->c_fd, 1)))
        goto err;

    if (!TEST_ptr(h->c_net_bio = h->c_net_bio_own = BIO_new_dgram(h->c_fd, 0)))
        goto err;

    if (!TEST_true(BIO_dgram_set_peer(h->c_net_bio, h->s_net_bio_addr)))
        goto err;

    if (!TEST_ptr(h->c_ctx = SSL_CTX_new(OSSL_QUIC_client_method())))
        goto err;

    /* Set title for qlog purposes. */
    BIO_snprintf(title, sizeof(title), "quic_multistream_test: %s", script_name);
    if (!TEST_true(ossl_quic_set_diag_title(h->c_ctx, title)))
        goto err;

    if (!TEST_ptr(h->c_conn = SSL_new(h->c_ctx)))
        goto err;

    /* Use custom time function for virtual time skip. */
    if (!TEST_true(ossl_quic_set_override_now_cb(h->c_conn, get_time, h)))
        goto err;

    /* Takes ownership of our reference to the BIO. */
    SSL_set0_rbio(h->c_conn, h->c_net_bio);
    h->c_net_bio_own = NULL;

    if (!TEST_true(BIO_up_ref(h->c_net_bio)))
        goto err;

    SSL_set0_wbio(h->c_conn, h->c_net_bio);

    if (!TEST_true(SSL_set_blocking_mode(h->c_conn, h->blocking)))
        goto err;

#if defined(OPENSSL_THREADS)
    if (!TEST_ptr(h->misc_m = ossl_crypto_mutex_new()))
      goto err;
    if (!TEST_ptr(h->misc_cv = ossl_crypto_condvar_new()))
      goto err;
#endif

    if (h->blocking) {
#if defined(OPENSSL_THREADS)
        if (!TEST_ptr(h->server_thread.m = ossl_crypto_mutex_new()))
            goto err;

        if (!TEST_ptr(h->server_thread.c = ossl_crypto_condvar_new()))
            goto err;

        h->server_thread.t
            = ossl_crypto_thread_native_start(server_helper_thread, h, 1);
        if (!TEST_ptr(h->server_thread.t))
            goto err;
#else
        TEST_error("cannot support blocking mode without threads");
        goto err;
#endif
    }

    h->start_time   = ossl_time_now();
    h->init         = 1;
    return 1;

err:
    helper_cleanup(h);
    return 0;
}

static int helper_local_init(struct helper_local *hl, struct helper *h,
                             int thread_idx)
{
    hl->h                       = h;
    hl->c_streams               = NULL;
    hl->thread_idx              = thread_idx;
    hl->explicit_event_handling = 0;

    if (!TEST_ptr(h))
        return 0;

    if (thread_idx < 0) {
        hl->c_streams = h->c_streams;
    } else {
        if (!TEST_ptr(hl->c_streams = lh_STREAM_INFO_new(stream_info_hash,
                                                         stream_info_cmp)))
            return 0;
    }

    return 1;
}

static void helper_local_cleanup(struct helper_local *hl)
{
    if (hl->h == NULL)
        return;

    if (hl->thread_idx >= 0)
        helper_cleanup_streams(&hl->c_streams);

    hl->h = NULL;
}

static STREAM_INFO *get_stream_info(LHASH_OF(STREAM_INFO) *lh,
                                    const char *stream_name)
{
    STREAM_INFO key, *info;

    if (!TEST_ptr(stream_name))
        return NULL;

    if (!strcmp(stream_name, "DEFAULT"))
        return NULL;

    key.name = stream_name;
    info = lh_STREAM_INFO_retrieve(lh, &key);
    if (info == NULL) {
        info = OPENSSL_zalloc(sizeof(*info));
        if (info == NULL)
            return NULL;

        info->name          = stream_name;
        info->s_stream_id   = UINT64_MAX;
        lh_STREAM_INFO_insert(lh, info);
    }

    return info;
}

static int helper_local_set_c_stream(struct helper_local *hl,
                                     const char *stream_name,
                                     SSL *c_stream)
{
    STREAM_INFO *info = get_stream_info(hl->c_streams, stream_name);

    if (info == NULL)
        return 0;

    info->c_stream      = c_stream;
    info->s_stream_id   = UINT64_MAX;
    return 1;
}

static SSL *helper_local_get_c_stream(struct helper_local *hl,
                                      const char *stream_name)
{
    STREAM_INFO *info;

    if (!strcmp(stream_name, "DEFAULT"))
        return hl->h->c_conn;

    info = get_stream_info(hl->c_streams, stream_name);
    if (info == NULL)
        return NULL;

    return info->c_stream;
}

static int
helper_set_s_stream(struct helper *h, const char *stream_name,
                    uint64_t s_stream_id)
{
    STREAM_INFO *info;

    if (!strcmp(stream_name, "DEFAULT"))
        return 0;

    info = get_stream_info(h->s_streams, stream_name);
    if (info == NULL)
        return 0;

    info->c_stream      = NULL;
    info->s_stream_id   = s_stream_id;
    return 1;
}

static uint64_t helper_get_s_stream(struct helper *h, const char *stream_name)
{
    STREAM_INFO *info;

    if (!strcmp(stream_name, "DEFAULT"))
        return UINT64_MAX;

    info = get_stream_info(h->s_streams, stream_name);
    if (info == NULL)
        return UINT64_MAX;

    return info->s_stream_id;
}

static int helper_packet_plain_listener(QTEST_FAULT *qtf, QUIC_PKT_HDR *hdr,
                                        unsigned char *buf, size_t buf_len,
                                        void *arg)
{
    struct helper *h = arg;

    return h->qtf_packet_plain_cb(h, hdr, buf, buf_len);
}

static int helper_handshake_listener(QTEST_FAULT *fault,
                                     unsigned char *buf, size_t buf_len,
                                     void *arg)
{
    struct helper *h = arg;

    return h->qtf_handshake_cb(h, buf, buf_len);
}

static int helper_datagram_listener(QTEST_FAULT *fault,
                                    BIO_MSG *msg, size_t stride,
                                    void *arg)
{
    struct helper *h = arg;

    return h->qtf_datagram_cb(h, msg, stride);
}

static int is_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);

    return ec == SSL_ERROR_WANT_READ || ec == SSL_ERROR_WANT_WRITE;
}

static int check_consistent_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);
    int w = SSL_want(s);

    int ok = TEST_true(
        (ec == SSL_ERROR_NONE                 && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_ZERO_RETURN          && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_SSL                  && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_SYSCALL              && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_WANT_READ            && w == SSL_READING)
    ||  (ec == SSL_ERROR_WANT_WRITE           && w == SSL_WRITING)
    ||  (ec == SSL_ERROR_WANT_CLIENT_HELLO_CB && w == SSL_CLIENT_HELLO_CB)
    ||  (ec == SSL_ERROR_WANT_X509_LOOKUP     && w == SSL_X509_LOOKUP)
    ||  (ec == SSL_ERROR_WANT_RETRY_VERIFY    && w == SSL_RETRY_VERIFY)
    );

    if (!ok)
        TEST_error("got error=%d, want=%d", ec, w);

    return ok;
}

static int run_script_worker(struct helper *h, const struct script_op *script,
                             const char *script_name,
                             int thread_idx)
{
    int testresult = 0;
    unsigned char *tmp_buf = NULL;
    int connect_started = 0;
    size_t offset = 0;
    size_t op_idx = 0;
    const struct script_op *op = NULL;
    int no_advance = 0, first = 1;
#if defined(OPENSSL_THREADS)
    int end_wait_warning = 0;
#endif
    OSSL_TIME op_start_time = ossl_time_zero(), op_deadline = ossl_time_zero();
    struct helper_local hl_, *hl = &hl_;
#define REPEAT_SLOTS 8
    size_t repeat_stack_idx[REPEAT_SLOTS], repeat_stack_done[REPEAT_SLOTS];
    size_t repeat_stack_limit[REPEAT_SLOTS];
    size_t repeat_stack_len = 0;

    if (!TEST_true(helper_local_init(hl, h, thread_idx)))
        goto out;

#define COMMON_SPIN_AGAIN()                             \
    {                                                   \
        no_advance = 1;                                 \
        continue;                                       \
    }
#define S_SPIN_AGAIN()                                  \
    {                                                   \
        s_lock(h, hl);                                  \
        ossl_quic_tserver_tick(h->s);                   \
        COMMON_SPIN_AGAIN();                            \
    }
#define C_SPIN_AGAIN()                                  \
    {                                                   \
        if (h->blocking) {                              \
            TEST_error("spin again in blocking mode");  \
            goto out;                                   \
        }                                               \
        COMMON_SPIN_AGAIN();                            \
    }

    for (;;) {
        SSL *c_tgt              = h->c_conn;
        uint64_t s_stream_id    = UINT64_MAX;

        s_unlock(h, hl);

        if (no_advance) {
            no_advance = 0;
        } else {
            if (!first)
                ++op_idx;

            first           = 0;
            offset          = 0;
            op_start_time   = ossl_time_now();
            op_deadline     = ossl_time_add(op_start_time, ossl_ms2time(60000));
        }

        if (!TEST_int_le(ossl_time_compare(ossl_time_now(), op_deadline), 0)) {
            TEST_error("op %zu timed out on thread %d", op_idx + 1, thread_idx);
            goto out;
        }

        op = &script[op_idx];

        if (op->stream_name != NULL) {
            c_tgt = helper_local_get_c_stream(hl, op->stream_name);
            if (thread_idx < 0)
                s_stream_id = helper_get_s_stream(h, op->stream_name);
            else
                s_stream_id = UINT64_MAX;
        }

        if (thread_idx < 0) {
            if (!h->blocking) {
                ossl_quic_tserver_tick(h->s);
            }
#if defined(OPENSSL_THREADS)
            else if (h->blocking && !h->server_thread.ready) {
                ossl_crypto_mutex_lock(h->server_thread.m);
                h->server_thread.ready = 1;
                ossl_crypto_condvar_signal(h->server_thread.c);
                ossl_crypto_mutex_unlock(h->server_thread.m);
            }
            if (h->blocking)
                assert(h->s == NULL);
#endif
        }

        if (!hl->explicit_event_handling
            && (thread_idx >= 0 || connect_started))
            SSL_handle_events(h->c_conn);

        if (thread_idx >= 0) {
            /* Only allow certain opcodes on child threads. */
            switch (op->op) {
                case OPK_END:
                case OPK_CHECK:
                case OPK_C_ACCEPT_STREAM_WAIT:
                case OPK_C_NEW_STREAM:
                case OPK_C_READ_EXPECT:
                case OPK_C_EXPECT_FIN:
                case OPK_C_WRITE:
                case OPK_C_WRITE_EX2:
                case OPK_C_CONCLUDE:
                case OPK_C_FREE_STREAM:
                case OPK_BEGIN_REPEAT:
                case OPK_END_REPEAT:
                case OPK_C_READ_FAIL_WAIT:
                case OPK_C_EXPECT_SSL_ERR:
                case OPK_EXPECT_ERR_REASON:
                case OPK_EXPECT_ERR_LIB:
                case OPK_POP_ERR:
                case OPK_SLEEP:
                    break;

                default:
                    TEST_error("opcode %lu not allowed on child thread",
                               (unsigned long)op->op);
                    goto out;
            }
        }

        switch (op->op) {
        case OPK_END:
            if (!TEST_size_t_eq(repeat_stack_len, 0))
                goto out;

#if defined(OPENSSL_THREADS)
            if (thread_idx < 0) {
                int done;
                size_t i;

                for (i = 0; i < h->num_threads; ++i) {
                    if (h->threads[i].m == NULL)
                        continue;

                    ossl_crypto_mutex_lock(h->threads[i].m);
                    done = h->threads[i].done;
                    ossl_crypto_mutex_unlock(h->threads[i].m);

                    if (!done) {
                        if (!end_wait_warning) {
                            TEST_info("still waiting for other threads to finish (%zu)", i);
                            end_wait_warning = 1;
                        }

                        S_SPIN_AGAIN();
                    }
                }
            }
#endif

            TEST_info("script \"%s\" finished on thread %d", script_name, thread_idx);
            testresult = 1;
            goto out;

        case OPK_BEGIN_REPEAT:
            if (!TEST_size_t_lt(repeat_stack_len, OSSL_NELEM(repeat_stack_idx)))
                goto out;

            if (!TEST_size_t_gt(op->arg1, 0))
                goto out;

            repeat_stack_idx[repeat_stack_len] = op_idx + 1;
            repeat_stack_done[repeat_stack_len] = 0;
            repeat_stack_limit[repeat_stack_len] = op->arg1;
            ++repeat_stack_len;
            break;

        case OPK_C_SKIP_IF_UNBOUND:
            if (c_tgt != NULL)
                break;

            op_idx += op->arg1;
            break;

        case OPK_SKIP_IF_BLOCKING:
            if (!h->blocking)
                break;

            op_idx += op->arg1;
            break;

        case OPK_END_REPEAT:
            if (!TEST_size_t_gt(repeat_stack_len, 0))
                goto out;

            if (++repeat_stack_done[repeat_stack_len - 1]
                == repeat_stack_limit[repeat_stack_len - 1]) {
                --repeat_stack_len;
            } else {
                op_idx = repeat_stack_idx[repeat_stack_len - 1];
                no_advance = 1;
                continue;
            }

            break;

        case OPK_CHECK:
            {
                int ok;

                hl->check_op = op;
                ok = op->check_func(h, hl);
                hl->check_op = NULL;

                if (thread_idx < 0 && h->check_spin_again) {
                    h->check_spin_again = 0;
                    S_SPIN_AGAIN();
                }

                if (!TEST_true(ok))
                    goto out;
            }
            break;

        case OPK_C_SET_ALPN:
            {
                const char *alpn = op->arg0;
                size_t alpn_len = strlen(alpn);

                if (!TEST_size_t_le(alpn_len, UINT8_MAX)
                    || !TEST_ptr(tmp_buf = (unsigned char *)OPENSSL_malloc(alpn_len + 1)))
                    goto out;

                memcpy(tmp_buf + 1, alpn, alpn_len);
                tmp_buf[0] = (unsigned char)alpn_len;

                /* 0 is the success case for SSL_set_alpn_protos(). */
                if (!TEST_false(SSL_set_alpn_protos(h->c_conn, tmp_buf,
                                                    alpn_len + 1)))
                    goto out;

                OPENSSL_free(tmp_buf);
                tmp_buf = NULL;
            }
            break;

        case OPK_C_CONNECT_WAIT:
            {
                int ret;

                connect_started = 1;

                ret = SSL_connect(h->c_conn);
                if (!check_consistent_want(c_tgt, ret))
                    goto out;
                if (ret != 1) {
                    if (!h->blocking && is_want(h->c_conn, ret))
                        C_SPIN_AGAIN();

                    if (op->arg1 == 0 && !TEST_int_eq(ret, 1))
                        goto out;
                }
            }
            break;

        case OPK_C_WRITE:
            {
                size_t bytes_written = 0;
                int r;

                if (!TEST_ptr(c_tgt))
                    goto out;

                r = SSL_write_ex(c_tgt, op->arg0, op->arg1, &bytes_written);
                if (!TEST_true(r)
                    || !check_consistent_want(c_tgt, r)
                    || !TEST_size_t_eq(bytes_written, op->arg1))
                    goto out;
            }
            break;

        case OPK_C_WRITE_EX2:
            {
                size_t bytes_written = 0;
                int r;

                if (!TEST_ptr(c_tgt))
                    goto out;

                r = SSL_write_ex2(c_tgt, op->arg0, op->arg1, op->arg2,
                                  &bytes_written);
                if (!TEST_true(r)
                    || !check_consistent_want(c_tgt, r)
                    || !TEST_size_t_eq(bytes_written, op->arg1))
                    goto out;
            }
            break;

        case OPK_S_WRITE:
            {
                size_t bytes_written = 0;

                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                if (!TEST_true(ossl_quic_tserver_write(ACQUIRE_S(), s_stream_id,
                                                       op->arg0, op->arg1,
                                                       &bytes_written))
                    || !TEST_size_t_eq(bytes_written, op->arg1))
                    goto out;
            }
            break;

        case OPK_C_CONCLUDE:
            {
                if (!TEST_true(SSL_stream_conclude(c_tgt, 0)))
                    goto out;
            }
            break;

        case OPK_S_CONCLUDE:
            {
                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                ossl_quic_tserver_conclude(ACQUIRE_S(), s_stream_id);
            }
            break;

        case OPK_C_WAIT_FOR_DATA:
            {
                char buf[1];
                size_t bytes_read = 0;

                if (!TEST_ptr(c_tgt))
                    goto out;

                if (!SSL_peek_ex(c_tgt, buf, sizeof(buf), &bytes_read)
                    || bytes_read == 0)
                    C_SPIN_AGAIN();
            }
            break;

        case OPK_C_READ_EXPECT:
            {
                size_t bytes_read = 0;
                int r;

                if (op->arg1 > 0 && tmp_buf == NULL
                    && !TEST_ptr(tmp_buf = OPENSSL_malloc(op->arg1)))
                    goto out;

                r = SSL_read_ex(c_tgt, tmp_buf + offset, op->arg1 - offset,
                                &bytes_read);
                if (!check_consistent_want(c_tgt, r))
                    goto out;

                if (!r)
                    C_SPIN_AGAIN();

                if (bytes_read + offset != op->arg1) {
                    offset += bytes_read;
                    C_SPIN_AGAIN();
                }

                if (op->arg1 > 0
                    && !TEST_mem_eq(tmp_buf, op->arg1, op->arg0, op->arg1))
                    goto out;

                OPENSSL_free(tmp_buf);
                tmp_buf = NULL;
            }
            break;

        case OPK_S_READ_EXPECT:
            {
                size_t bytes_read = 0;

                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                if (op->arg1 > 0 && tmp_buf == NULL
                    && !TEST_ptr(tmp_buf = OPENSSL_malloc(op->arg1)))
                    goto out;

                if (!TEST_true(ossl_quic_tserver_read(ACQUIRE_S(), s_stream_id,
                                                      tmp_buf + offset,
                                                      op->arg1 - offset,
                                                      &bytes_read)))
                    goto out;

                if (bytes_read + offset != op->arg1) {
                    offset += bytes_read;
                    S_SPIN_AGAIN();
                }

                if (op->arg1 > 0
                    && !TEST_mem_eq(tmp_buf, op->arg1, op->arg0, op->arg1))
                    goto out;

                OPENSSL_free(tmp_buf);
                tmp_buf = NULL;
            }
            break;

        case OPK_C_EXPECT_FIN:
            {
                char buf[1];
                size_t bytes_read = 0;
                int r;

                r = SSL_read_ex(c_tgt, buf, sizeof(buf), &bytes_read);
                if (!check_consistent_want(c_tgt, r)
                    || !TEST_false(r)
                    || !TEST_size_t_eq(bytes_read, 0))
                    goto out;

                if (is_want(c_tgt, 0))
                    C_SPIN_AGAIN();

                if (!TEST_int_eq(SSL_get_error(c_tgt, 0),
                                 SSL_ERROR_ZERO_RETURN))
                    goto out;

                if (!TEST_int_eq(SSL_want(c_tgt), SSL_NOTHING))
                    goto out;
            }
            break;

        case OPK_S_EXPECT_FIN:
            {
                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                if (!ossl_quic_tserver_has_read_ended(ACQUIRE_S(), s_stream_id))
                    S_SPIN_AGAIN();
            }
            break;

        case OPK_C_DETACH:
            {
                SSL *c_stream;

                if (!TEST_ptr_null(c_tgt))
                    goto out; /* don't overwrite existing stream with same name */

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_ptr(c_stream = ossl_quic_detach_stream(h->c_conn)))
                    goto out;

                if (!TEST_true(helper_local_set_c_stream(hl, op->stream_name, c_stream)))
                    goto out;
            }
            break;

        case OPK_C_ATTACH:
            {
                if (!TEST_ptr(c_tgt))
                    goto out;

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_true(ossl_quic_attach_stream(h->c_conn, c_tgt)))
                    goto out;

                if (!TEST_true(helper_local_set_c_stream(hl, op->stream_name, NULL)))
                    goto out;
            }
            break;

        case OPK_C_NEW_STREAM:
            {
                SSL *c_stream;
                uint64_t flags = op->arg1;
                int allow_fail = ((flags & ALLOW_FAIL) != 0);

                flags &= ~(uint64_t)ALLOW_FAIL;

                if (!TEST_ptr_null(c_tgt))
                    goto out; /* don't overwrite existing stream with same name */

                if (!TEST_ptr(op->stream_name))
                    goto out;

                c_stream = SSL_new_stream(h->c_conn, flags);
                if (!allow_fail && !TEST_ptr(c_stream))
                    goto out;

                if (allow_fail && c_stream == NULL) {
                    if (!TEST_size_t_eq(ERR_GET_REASON(ERR_get_error()),
                                        SSL_R_STREAM_COUNT_LIMITED))
                        goto out;

                    ++h->fail_count;
                    break;
                }

                if (op->arg2 != UINT64_MAX
                    && !TEST_uint64_t_eq(SSL_get_stream_id(c_stream),
                                         op->arg2))
                    goto out;

                if (!TEST_true(helper_local_set_c_stream(hl, op->stream_name, c_stream)))
                    goto out;
            }
            break;

        case OPK_S_NEW_STREAM:
            {
                uint64_t stream_id = UINT64_MAX;

                if (!TEST_uint64_t_eq(s_stream_id, UINT64_MAX))
                    goto out; /* don't overwrite existing stream with same name */

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_true(ossl_quic_tserver_stream_new(ACQUIRE_S(),
                                                            op->arg1 > 0,
                                                            &stream_id)))
                    goto out;

                if (op->arg2 != UINT64_MAX
                    && !TEST_uint64_t_eq(stream_id, op->arg2))
                    goto out;

                if (!TEST_true(helper_set_s_stream(h, op->stream_name,
                                                   stream_id)))
                    goto out;
            }
            break;

        case OPK_C_ACCEPT_STREAM_WAIT:
            {
                SSL *c_stream;

                if (!TEST_ptr_null(c_tgt))
                    goto out; /* don't overwrite existing stream with same name */

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if ((c_stream = SSL_accept_stream(h->c_conn, 0)) == NULL)
                    C_SPIN_AGAIN();

                if (!TEST_true(helper_local_set_c_stream(hl, op->stream_name,
                                                          c_stream)))
                    goto out;
            }
            break;

        case OPK_S_ACCEPT_STREAM_WAIT:
            {
                uint64_t new_stream_id;

                if (!TEST_uint64_t_eq(s_stream_id, UINT64_MAX))
                    goto out;

                if (!TEST_ptr(op->stream_name))
                    goto out;

                new_stream_id = ossl_quic_tserver_pop_incoming_stream(ACQUIRE_S());
                if (new_stream_id == UINT64_MAX)
                    S_SPIN_AGAIN();

                if (!TEST_true(helper_set_s_stream(h, op->stream_name, new_stream_id)))
                    goto out;
            }
            break;

        case OPK_C_ACCEPT_STREAM_NONE:
            {
                SSL *c_stream;

                if (!TEST_ptr_null(c_stream = SSL_accept_stream(h->c_conn,
                                                                SSL_ACCEPT_STREAM_NO_BLOCK))) {
                    SSL_free(c_stream);
                    goto out;
                }
            }
            break;

        case OPK_C_FREE_STREAM:
            {
                if (!TEST_ptr(c_tgt)
                    || !TEST_true(!SSL_is_connection(c_tgt)))
                    goto out;

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_true(helper_local_set_c_stream(hl, op->stream_name, NULL)))
                    goto out;

                SSL_free(c_tgt);
                c_tgt = NULL;
            }
            break;

        case OPK_C_SET_DEFAULT_STREAM_MODE:
            {
                if (!TEST_ptr(c_tgt))
                    goto out;

                if (!TEST_true(SSL_set_default_stream_mode(c_tgt, op->arg1)))
                    goto out;
            }
            break;

        case OPK_C_SET_INCOMING_STREAM_POLICY:
            {
                if (!TEST_ptr(c_tgt))
                    goto out;

                if (!TEST_true(SSL_set_incoming_stream_policy(c_tgt,
                                                              op->arg1, 0)))
                    goto out;
            }
            break;

        case OPK_C_SHUTDOWN_WAIT:
            {
                int ret;
                QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);
                SSL_SHUTDOWN_EX_ARGS args = {0};

                ossl_quic_engine_set_inhibit_tick(ossl_quic_channel_get0_engine(ch), 0);

                if (!TEST_ptr(c_tgt))
                    goto out;

                args.quic_reason = (const char *)op->arg0;

                ret = SSL_shutdown_ex(c_tgt, op->arg1, &args, sizeof(args));
                if (!TEST_int_ge(ret, 0))
                    goto out;

                if (ret == 0)
                    C_SPIN_AGAIN();
            }
            break;

        case OPK_S_SHUTDOWN:
            {
                ossl_quic_tserver_shutdown(ACQUIRE_S(), op->arg1);
            }
            break;

        case OPK_C_EXPECT_CONN_CLOSE_INFO:
            {
                SSL_CONN_CLOSE_INFO cc_info = {0};
                int expect_app = (op->arg1 & EXPECT_CONN_CLOSE_APP) != 0;
                int expect_remote = (op->arg1 & EXPECT_CONN_CLOSE_REMOTE) != 0;
                uint64_t error_code = op->arg2;

                if (!TEST_ptr(c_tgt))
                    goto out;

                if (h->blocking
                    && !TEST_true(SSL_shutdown_ex(c_tgt,
                                                  SSL_SHUTDOWN_FLAG_WAIT_PEER,
                                                  NULL, 0)))
                    goto out;

                if (!SSL_get_conn_close_info(c_tgt, &cc_info, sizeof(cc_info)))
                    C_SPIN_AGAIN();

                if (!TEST_int_eq(expect_app,
                                 (cc_info.flags
                                  & SSL_CONN_CLOSE_FLAG_TRANSPORT) == 0)
                    || !TEST_int_eq(expect_remote,
                                    (cc_info.flags
                                     & SSL_CONN_CLOSE_FLAG_LOCAL) == 0)
                    || !TEST_uint64_t_eq(error_code, cc_info.error_code)) {
                    TEST_info("Connection close reason: %s", cc_info.reason);
                    goto out;
                }
            }
            break;

        case OPK_S_EXPECT_CONN_CLOSE_INFO:
            {
                const QUIC_TERMINATE_CAUSE *tc;
                int expect_app = (op->arg1 & EXPECT_CONN_CLOSE_APP) != 0;
                int expect_remote = (op->arg1 & EXPECT_CONN_CLOSE_REMOTE) != 0;
                uint64_t error_code = op->arg2;

                if (!ossl_quic_tserver_is_term_any(ACQUIRE_S())) {
                    ossl_quic_tserver_ping(ACQUIRE_S());
                    S_SPIN_AGAIN();
                }

                if (!TEST_ptr(tc = ossl_quic_tserver_get_terminate_cause(ACQUIRE_S())))
                    goto out;

                if (!TEST_uint64_t_eq(error_code, tc->error_code)
                    || !TEST_int_eq(expect_app, tc->app)
                    || !TEST_int_eq(expect_remote, tc->remote))
                    goto out;
            }
            break;

        case OPK_S_BIND_STREAM_ID:
            {
                if (!TEST_uint64_t_eq(s_stream_id, UINT64_MAX))
                    goto out;

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_true(helper_set_s_stream(h, op->stream_name, op->arg2)))
                    goto out;
            }
            break;

        case OPK_S_UNBIND_STREAM_ID:
            {
                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                if (!TEST_ptr(op->stream_name))
                    goto out;

                if (!TEST_true(helper_set_s_stream(h, op->stream_name, UINT64_MAX)))
                    goto out;
            }
            break;

        case OPK_C_WRITE_FAIL:
            {
                size_t bytes_written = 0;
                int r;

                if (!TEST_ptr(c_tgt))
                    goto out;

                r = SSL_write_ex(c_tgt, "apple", 5, &bytes_written);
                if (!TEST_false(r)
                    || !check_consistent_want(c_tgt, r))
                    goto out;
            }
            break;

        case OPK_S_WRITE_FAIL:
            {
                size_t bytes_written = 0;

                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                if (!TEST_false(ossl_quic_tserver_write(ACQUIRE_S(), s_stream_id,
                                                       (const unsigned char *)"apple", 5,
                                                       &bytes_written)))
                    goto out;
            }
            break;

        case OPK_C_READ_FAIL:
            {
                size_t bytes_read = 0;
                char buf[1];
                int r;

                if (!TEST_ptr(c_tgt))
                    goto out;

                r = SSL_read_ex(c_tgt, buf, sizeof(buf), &bytes_read);
                if (!TEST_false(r))
                    goto out;
                if (!check_consistent_want(c_tgt, r))
                    goto out;
            }
            break;

        case OPK_C_READ_FAIL_WAIT:
            {
                size_t bytes_read = 0;
                char buf[1];
                int r;

                if (!TEST_ptr(c_tgt))
                    goto out;

                r = SSL_read_ex(c_tgt, buf, sizeof(buf), &bytes_read);
                if (!TEST_false(r))
                    goto out;
                if (!check_consistent_want(c_tgt, r))
                    goto out;

                if (is_want(c_tgt, 0))
                    C_SPIN_AGAIN();
            }
            break;

        case OPK_S_READ_FAIL:
            {
                int ret;
                size_t bytes_read = 0;
                unsigned char buf[1];

                if (!TEST_uint64_t_ne(s_stream_id, UINT64_MAX))
                    goto out;

                ret = ossl_quic_tserver_read(ACQUIRE_S(), s_stream_id,
                                             buf, sizeof(buf),
                                             &bytes_read);
                if (!TEST_true(ret == 0 || (op->arg1 && bytes_read == 0)))
                    goto out;
            }
            break;

        case OPK_C_STREAM_RESET:
        case OPK_C_STREAM_RESET_FAIL:
            {
                SSL_STREAM_RESET_ARGS args = {0};

                if (!TEST_ptr(c_tgt))
                    goto out;

                args.quic_error_code = op->arg2;
                if (op->op == OPK_C_STREAM_RESET) {
                    if (!TEST_true(SSL_stream_reset(c_tgt, &args, sizeof(args))))
                        goto out;
                } else {
                    if (!TEST_false(SSL_stream_reset(c_tgt, &args, sizeof(args))))
                        goto out;
                }
            }
            break;

        case OPK_NEW_THREAD:
            {
#if !defined(OPENSSL_THREADS)
                /*
                 * If this test script requires threading and we do not have
                 * support for it, skip the rest of it.
                 */
                TEST_skip("threading not supported, skipping");
                testresult = 1;
                goto out;
#else
                size_t i;

                if (!TEST_ptr_null(h->threads)) {
                    TEST_error("max one NEW_THREAD operation per script");
                    goto out;
                }

                h->threads = OPENSSL_zalloc(op->arg1 * sizeof(struct child_thread_args));
                if (!TEST_ptr(h->threads))
                    goto out;

                h->num_threads = op->arg1;

                for (i = 0; i < op->arg1; ++i) {
                    h->threads[i].h            = h;
                    h->threads[i].script       = op->arg0;
                    h->threads[i].script_name  = script_name;
                    h->threads[i].thread_idx   = i;

                    h->threads[i].m = ossl_crypto_mutex_new();
                    if (!TEST_ptr(h->threads[i].m))
                        goto out;

                    h->threads[i].t
                        = ossl_crypto_thread_native_start(run_script_child_thread,
                                                          &h->threads[i], 1);
                    if (!TEST_ptr(h->threads[i].t))
                        goto out;
                }
#endif
            }
            break;

        case OPK_C_CLOSE_SOCKET:
            {
                BIO_closesocket(h->c_fd);
                h->c_fd = -1;
            }
            break;

        case OPK_C_EXPECT_SSL_ERR:
            {
                if (!TEST_size_t_eq((size_t)SSL_get_error(c_tgt, 0), op->arg1))
                    goto out;
                if (!TEST_int_eq(SSL_want(c_tgt), SSL_NOTHING))
                    goto out;
            }
            break;

        case OPK_EXPECT_ERR_REASON:
            {
                if (!TEST_size_t_eq((size_t)ERR_GET_REASON(ERR_peek_last_error()), op->arg1))
                    goto out;
            }
            break;

        case OPK_EXPECT_ERR_LIB:
            {
                if (!TEST_size_t_eq((size_t)ERR_GET_LIB(ERR_peek_last_error()), op->arg1))
                    goto out;
            }
            break;

        case OPK_POP_ERR:
            ERR_pop();
            break;

        case OPK_SLEEP:
            {
                OSSL_sleep(op->arg2);
            }
            break;

        case OPK_S_SET_INJECT_PLAIN:
            h->qtf_packet_plain_cb = op->qtf_packet_plain_cb;

            if (!TEST_true(qtest_fault_set_packet_plain_listener(h->qtf,
                                                                 h->qtf_packet_plain_cb != NULL ?
                                                                 helper_packet_plain_listener : NULL,
                                                                 h)))
                goto out;

            break;

        case OPK_S_SET_INJECT_HANDSHAKE:
            h->qtf_handshake_cb = op->qtf_handshake_cb;

            if (!TEST_true(qtest_fault_set_handshake_listener(h->qtf,
                                                              h->qtf_handshake_cb != NULL ?
                                                              helper_handshake_listener : NULL,
                                                              h)))
                goto out;

            break;

        case OPK_S_SET_INJECT_DATAGRAM:
            h->qtf_datagram_cb = op->qtf_datagram_cb;

            if (!TEST_true(qtest_fault_set_datagram_listener(h->qtf,
                                                             h->qtf_datagram_cb != NULL ?
                                                             helper_datagram_listener : NULL,
                                                             h)))
                goto out;

            break;

        case OPK_SET_INJECT_WORD:
            /*
             * Must hold server tick lock - callbacks can be called from other
             * thread when running test in blocking mode (tsan).
             */
            ACQUIRE_S();
            h->inject_word0 = op->arg1;
            h->inject_word1 = op->arg2;
            break;

        case OPK_C_INHIBIT_TICK:
            {
                QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);

                ossl_quic_engine_set_inhibit_tick(ossl_quic_channel_get0_engine(ch),
                                                  op->arg1);
            }
            break;

        case OPK_C_SET_WRITE_BUF_SIZE:
            if (!TEST_ptr(c_tgt))
                goto out;

            if (!TEST_true(ossl_quic_set_write_buffer_size(c_tgt, op->arg1)))
                goto out;

            break;

        case OPK_S_NEW_TICKET:
            if (!TEST_true(ossl_quic_tserver_new_ticket(ACQUIRE_S())))
                goto out;
            break;

        default:
            TEST_error("unknown op");
            goto out;
        }
    }

out:
    s_unlock(h, hl); /* idempotent */
    if (!testresult) {
        size_t i;
        const QUIC_TERMINATE_CAUSE *tcause;
        const char *e_str, *f_str;

        TEST_error("failed in script \"%s\" at op %zu, thread %d\n",
                   script_name, op_idx + 1, thread_idx);

        for (i = 0; i < repeat_stack_len; ++i)
            TEST_info("while repeating, iteration %zu of %zu, starting at script op %zu",
                      repeat_stack_done[i],
                      repeat_stack_limit[i],
                      repeat_stack_idx[i]);

        ERR_print_errors_fp(stderr);

        if (h->c_conn != NULL) {
            SSL_CONN_CLOSE_INFO cc_info = {0};

            if (SSL_get_conn_close_info(h->c_conn, &cc_info, sizeof(cc_info))) {
                e_str = ossl_quic_err_to_string(cc_info.error_code);
                f_str = ossl_quic_frame_type_to_string(cc_info.frame_type);

                if (e_str == NULL)
                    e_str = "?";
                if (f_str == NULL)
                    f_str = "?";

                TEST_info("client side is closed: %llu(%s)/%llu(%s), "
                          "%s, %s, reason: \"%s\"",
                          (unsigned long long)cc_info.error_code,
                          e_str,
                          (unsigned long long)cc_info.frame_type,
                          f_str,
                          (cc_info.flags & SSL_CONN_CLOSE_FLAG_LOCAL) != 0
                            ? "local" : "remote",
                          (cc_info.flags & SSL_CONN_CLOSE_FLAG_TRANSPORT) != 0
                            ? "transport" : "app",
                          cc_info.reason != NULL ? cc_info.reason : "-");
            }
        }

        tcause = (h->s != NULL
                  ? ossl_quic_tserver_get_terminate_cause(h->s) : NULL);
        if (tcause != NULL) {
            e_str = ossl_quic_err_to_string(tcause->error_code);
            f_str = ossl_quic_frame_type_to_string(tcause->frame_type);

            if (e_str == NULL)
                e_str = "?";
            if (f_str == NULL)
                f_str = "?";

            TEST_info("server side is closed: %llu(%s)/%llu(%s), "
                     "%s, %s, reason: \"%s\"",
                      (unsigned long long)tcause->error_code,
                      e_str,
                      (unsigned long long)tcause->frame_type,
                      f_str,
                      tcause->remote ? "remote" : "local",
                      tcause->app ? "app" : "transport",
                      tcause->reason != NULL ? tcause->reason : "-");
        }
    }

    OPENSSL_free(tmp_buf);
    helper_local_cleanup(hl);
    return testresult;
}

static int run_script(const struct script_op *script,
                      const char *script_name,
                      int free_order,
                      int blocking)
{
    int testresult = 0;
    struct helper h;

    if (!TEST_true(helper_init(&h, script_name,
                               free_order, blocking, 1)))
        goto out;

    if (!TEST_true(run_script_worker(&h, script, script_name, -1)))
        goto out;

#if defined(OPENSSL_THREADS)
    if (!TEST_true(join_threads(h.threads, h.num_threads)))
        goto out;
#endif

    testresult = 1;
out:
    helper_cleanup(&h);
    return testresult;
}

#if defined(OPENSSL_THREADS)
static CRYPTO_THREAD_RETVAL run_script_child_thread(void *arg)
{
    int testresult;
    struct child_thread_args *args = arg;

    testresult = run_script_worker(args->h, args->script,
                                   args->script_name,
                                   args->thread_idx);

    ossl_crypto_mutex_lock(args->m);
    args->testresult    = testresult;
    args->done          = 1;
    ossl_crypto_mutex_unlock(args->m);
    return 1;
}
#endif

/* 1. Simple single-stream test */
static const struct script_op script_1[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_C_CONCLUDE           (DEFAULT)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_EXPECT_FIN         (a)
    OP_S_WRITE              (a, "orange", 6)
    OP_S_CONCLUDE           (a)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)
    OP_C_EXPECT_FIN         (DEFAULT)
    OP_END
};

/* 2. Multi-stream test */
static const struct script_op script_2[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_INCOMING_STREAM_POLICY(SSL_INCOMING_STREAM_POLICY_ACCEPT)
    OP_C_WRITE              (DEFAULT,  "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_NEW_STREAM_BIDI    (b, C_BIDI_ID(1))
    OP_C_WRITE              (b, "flamingo", 8)
    OP_C_CONCLUDE           (b)
    OP_S_BIND_STREAM_ID     (b, C_BIDI_ID(1))
    OP_S_READ_EXPECT        (b, "flamingo", 8)
    OP_S_EXPECT_FIN         (b)
    OP_S_WRITE              (b, "gargoyle", 8)
    OP_S_CONCLUDE           (b)
    OP_C_READ_EXPECT        (b, "gargoyle", 8)
    OP_C_EXPECT_FIN         (b)

    OP_C_NEW_STREAM_UNI     (c, C_UNI_ID(0))
    OP_C_WRITE              (c, "elephant", 8)
    OP_C_CONCLUDE           (c)
    OP_S_BIND_STREAM_ID     (c, C_UNI_ID(0))
    OP_S_READ_EXPECT        (c, "elephant", 8)
    OP_S_EXPECT_FIN         (c)
    OP_S_WRITE_FAIL         (c)

    OP_C_ACCEPT_STREAM_NONE ()

    OP_S_NEW_STREAM_BIDI    (d, S_BIDI_ID(0))
    OP_S_WRITE              (d, "frog", 4)
    OP_S_CONCLUDE           (d)

    OP_C_ACCEPT_STREAM_WAIT (d)
    OP_C_ACCEPT_STREAM_NONE ()
    OP_C_READ_EXPECT        (d, "frog", 4)
    OP_C_EXPECT_FIN         (d)

    OP_S_NEW_STREAM_BIDI    (e, S_BIDI_ID(1))
    OP_S_WRITE              (e, "mixture", 7)
    OP_S_CONCLUDE           (e)

    OP_C_ACCEPT_STREAM_WAIT (e)
    OP_C_READ_EXPECT        (e, "mixture", 7)
    OP_C_EXPECT_FIN         (e)
    OP_C_WRITE              (e, "ramble", 6)
    OP_S_READ_EXPECT        (e, "ramble", 6)
    OP_C_CONCLUDE           (e)
    OP_S_EXPECT_FIN         (e)

    OP_S_NEW_STREAM_UNI     (f, S_UNI_ID(0))
    OP_S_WRITE              (f, "yonder", 6)
    OP_S_CONCLUDE           (f)

    OP_C_ACCEPT_STREAM_WAIT (f)
    OP_C_ACCEPT_STREAM_NONE ()
    OP_C_READ_EXPECT        (f, "yonder", 6)
    OP_C_EXPECT_FIN         (f)
    OP_C_WRITE_FAIL         (f)

    OP_C_SET_INCOMING_STREAM_POLICY(SSL_INCOMING_STREAM_POLICY_REJECT)
    OP_S_NEW_STREAM_BIDI    (g, S_BIDI_ID(2))
    OP_S_WRITE              (g, "unseen", 6)
    OP_S_CONCLUDE           (g)

    OP_C_ACCEPT_STREAM_NONE ()

    OP_C_SET_INCOMING_STREAM_POLICY(SSL_INCOMING_STREAM_POLICY_AUTO)
    OP_S_NEW_STREAM_BIDI    (h, S_BIDI_ID(3))
    OP_S_WRITE              (h, "UNSEEN", 6)
    OP_S_CONCLUDE           (h)

    OP_C_ACCEPT_STREAM_NONE ()

    /*
     * Streams g, h should have been rejected, so server should have got
     * STOP_SENDING/RESET_STREAM.
     */
    OP_CHECK                (check_rejected, S_BIDI_ID(2))
    OP_CHECK                (check_rejected, S_BIDI_ID(3))

    OP_END
};

/* 3. Default stream detach/reattach test */
static const struct script_op script_3[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_C_DETACH             (a)             /* DEFAULT becomes stream 'a' */
    OP_C_WRITE_FAIL         (DEFAULT)

    OP_C_WRITE              (a, "by", 2)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "appleby", 7)

    OP_S_WRITE              (a, "hello", 5)
    OP_C_READ_EXPECT        (a, "hello", 5)

    OP_C_WRITE_FAIL         (DEFAULT)
    OP_C_ATTACH             (a)
    OP_C_WRITE              (DEFAULT, "is here", 7)
    OP_S_READ_EXPECT        (a, "is here", 7)

    OP_C_DETACH             (a)
    OP_C_CONCLUDE           (a)
    OP_S_EXPECT_FIN         (a)

    OP_END
};

/* 4. Default stream mode test */
static const struct script_op script_4[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)
    OP_C_WRITE_FAIL         (DEFAULT)

    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_READ_FAIL          (DEFAULT)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_C_ATTACH             (a)
    OP_C_WRITE              (DEFAULT, "orange", 6)
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_END
};

/* 5. Test stream reset functionality */
static const struct script_op script_5[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)
    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_NEW_STREAM_BIDI    (b, C_BIDI_ID(1))

    OP_C_WRITE              (a, "apple", 5)
    OP_C_STREAM_RESET       (a, 42)

    OP_C_WRITE              (b, "strawberry", 10)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_BIND_STREAM_ID     (b, C_BIDI_ID(1))
    OP_S_READ_EXPECT        (b, "strawberry", 10)
    /* Reset disrupts read of already sent data */
    OP_S_READ_FAIL          (a, 0)
    OP_CHECK                (check_stream_reset, C_BIDI_ID(0))

    OP_END
};

/* 6. Test STOP_SENDING functionality */
static const struct script_op script_6[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)
    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_FREE_STREAM        (a)
    OP_C_ACCEPT_STREAM_NONE ()

    OP_CHECK                (check_stream_stopped, S_BIDI_ID(0))

    OP_END
};

/* 7. Unidirectional default stream mode test (client sends first) */
static const struct script_op script_7[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_AUTO_UNI)
    OP_C_WRITE              (DEFAULT, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_UNI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_WRITE_FAIL         (a)

    OP_END
};

/* 8. Unidirectional default stream mode test (server sends first) */
static const struct script_op script_8[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_AUTO_UNI)
    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)
    OP_C_READ_EXPECT        (DEFAULT, "apple", 5)
    OP_C_WRITE_FAIL         (DEFAULT)

    OP_END
};

/* 9. Unidirectional default stream mode test (server sends first on bidi) */
static const struct script_op script_9[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_AUTO_UNI)
    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "apple", 5)
    OP_C_READ_EXPECT        (DEFAULT, "apple", 5)
    OP_C_WRITE              (DEFAULT, "orange", 6)
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_END
};

/* 10. Shutdown */
static const struct script_op script_10[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_C_SHUTDOWN_WAIT      (NULL, 0)
    OP_C_EXPECT_CONN_CLOSE_INFO(0, 1, 0)
    OP_S_EXPECT_CONN_CLOSE_INFO(0, 1, 1)

    OP_END
};

/* 11. Many threads accepted on the same client connection */
static const struct script_op script_11_child[] = {
    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "foo", 3)
    OP_SLEEP                (10)
    OP_C_EXPECT_FIN         (a)

    OP_END
};

static const struct script_op script_11[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_NEW_THREAD           (5, script_11_child)

    OP_S_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_S_WRITE              (a, "foo", 3)
    OP_S_CONCLUDE           (a)

    OP_S_NEW_STREAM_BIDI    (b, ANY_ID)
    OP_S_WRITE              (b, "foo", 3)
    OP_S_CONCLUDE           (b)

    OP_S_NEW_STREAM_BIDI    (c, ANY_ID)
    OP_S_WRITE              (c, "foo", 3)
    OP_S_CONCLUDE           (c)

    OP_S_NEW_STREAM_BIDI    (d, ANY_ID)
    OP_S_WRITE              (d, "foo", 3)
    OP_S_CONCLUDE           (d)

    OP_S_NEW_STREAM_BIDI    (e, ANY_ID)
    OP_S_WRITE              (e, "foo", 3)
    OP_S_CONCLUDE           (e)

    OP_END
};

/* 12. Many threads initiated on the same client connection */
static const struct script_op script_12_child[] = {
    OP_C_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_C_WRITE              (a, "foo", 3)
    OP_C_CONCLUDE           (a)
    OP_C_FREE_STREAM        (a)

    OP_END
};

static const struct script_op script_12[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_NEW_THREAD           (5, script_12_child)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "foo", 3)
    OP_S_EXPECT_FIN         (a)
    OP_S_BIND_STREAM_ID     (b, C_BIDI_ID(1))
    OP_S_READ_EXPECT        (b, "foo", 3)
    OP_S_EXPECT_FIN         (b)
    OP_S_BIND_STREAM_ID     (c, C_BIDI_ID(2))
    OP_S_READ_EXPECT        (c, "foo", 3)
    OP_S_EXPECT_FIN         (c)
    OP_S_BIND_STREAM_ID     (d, C_BIDI_ID(3))
    OP_S_READ_EXPECT        (d, "foo", 3)
    OP_S_EXPECT_FIN         (d)
    OP_S_BIND_STREAM_ID     (e, C_BIDI_ID(4))
    OP_S_READ_EXPECT        (e, "foo", 3)
    OP_S_EXPECT_FIN         (e)

    OP_END
};

/* 13. Many threads accepted on the same client connection (stress test) */
static const struct script_op script_13_child[] = {
    OP_BEGIN_REPEAT         (10)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "foo", 3)
    OP_C_EXPECT_FIN         (a)
    OP_C_FREE_STREAM        (a)

    OP_END_REPEAT           ()

    OP_END
};

static const struct script_op script_13[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_NEW_THREAD           (5, script_13_child)

    OP_BEGIN_REPEAT         (50)

    OP_S_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_S_WRITE              (a, "foo", 3)
    OP_S_CONCLUDE           (a)
    OP_S_UNBIND_STREAM_ID   (a)

    OP_END_REPEAT           ()

    OP_END
};

/* 14. Many threads initiating on the same client connection (stress test) */
static const struct script_op script_14_child[] = {
    OP_BEGIN_REPEAT         (10)

    OP_C_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_C_WRITE              (a, "foo", 3)
    OP_C_CONCLUDE           (a)
    OP_C_FREE_STREAM        (a)

    OP_END_REPEAT           ()

    OP_END
};

static const struct script_op script_14[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_NEW_THREAD           (5, script_14_child)

    OP_BEGIN_REPEAT         (50)

    OP_S_ACCEPT_STREAM_WAIT (a)
    OP_S_READ_EXPECT        (a, "foo", 3)
    OP_S_EXPECT_FIN         (a)
    OP_S_UNBIND_STREAM_ID   (a)

    OP_END_REPEAT           ()

    OP_END
};

/* 15. Client sending large number of streams, MAX_STREAMS test */
static const struct script_op script_15[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    /*
     * This will cause a protocol violation to be raised by the server if we are
     * not handling the stream limit correctly on the TX side.
     */
    OP_BEGIN_REPEAT         (200)

    OP_C_NEW_STREAM_BIDI_EX (a, ANY_ID, SSL_STREAM_FLAG_ADVANCE)
    OP_C_WRITE              (a, "foo", 3)
    OP_C_CONCLUDE           (a)
    OP_C_FREE_STREAM        (a)

    OP_END_REPEAT           ()

    /* Prove the connection is still good. */
    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "bar", 3)
    OP_S_CONCLUDE           (a)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "bar", 3)
    OP_C_EXPECT_FIN         (a)

    /*
     * Drain the queue of incoming streams. We should be able to get all 200
     * even though only 100 can be initiated at a time.
     */
    OP_BEGIN_REPEAT         (200)

    OP_S_ACCEPT_STREAM_WAIT (b)
    OP_S_READ_EXPECT        (b, "foo", 3)
    OP_S_EXPECT_FIN         (b)
    OP_S_UNBIND_STREAM_ID   (b)

    OP_END_REPEAT           ()

    OP_END
};

/* 16. Server sending large number of streams, MAX_STREAMS test */
static const struct script_op script_16[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    /*
     * This will cause a protocol violation to be raised by the client if we are
     * not handling the stream limit correctly on the TX side.
     */
    OP_BEGIN_REPEAT         (200)

    OP_S_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_S_WRITE              (a, "foo", 3)
    OP_S_CONCLUDE           (a)
    OP_S_UNBIND_STREAM_ID   (a)

    OP_END_REPEAT           ()

    /* Prove that the connection is still good. */
    OP_C_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_C_WRITE              (a, "bar", 3)
    OP_C_CONCLUDE           (a)

    OP_S_ACCEPT_STREAM_WAIT (b)
    OP_S_READ_EXPECT        (b, "bar", 3)
    OP_S_EXPECT_FIN         (b)

    /* Drain the queue of incoming streams. */
    OP_BEGIN_REPEAT         (200)

    OP_C_ACCEPT_STREAM_WAIT (b)
    OP_C_READ_EXPECT        (b, "foo", 3)
    OP_C_EXPECT_FIN         (b)
    OP_C_FREE_STREAM        (b)

    OP_END_REPEAT           ()

    OP_END
};

/* 17. Key update test - unlimited */
static const struct script_op script_17[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_CHECK                (override_key_update, 1)

    OP_BEGIN_REPEAT         (200)

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_READ_EXPECT        (a, "apple", 5)

    /*
     * TXKU frequency is bounded by RTT because a previous TXKU needs to be
     * acknowledged by the peer first before another one can be begin. By
     * waiting this long, we eliminate any such concern and ensure as many key
     * updates as possible can occur for the purposes of this test.
     */
    OP_CHECK                (skip_time_ms,    100)

    OP_END_REPEAT           ()

    /* At least 5 RXKUs detected */
    OP_CHECK                (check_key_update_ge, 5)

    /*
     * Prove the connection is still healthy by sending something in both
     * directions.
     */
    OP_C_WRITE              (DEFAULT, "xyzzy", 5)
    OP_S_READ_EXPECT        (a, "xyzzy", 5)

    OP_S_WRITE              (a, "plugh", 5)
    OP_C_READ_EXPECT        (DEFAULT, "plugh", 5)

    OP_END
};

/* 18. Key update test - RTT-bounded */
static const struct script_op script_18[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_CHECK                (override_key_update, 1)

    OP_BEGIN_REPEAT         (200)

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_CHECK                (skip_time_ms,    8)

    OP_END_REPEAT           ()

    /*
     * This time we simulate far less time passing between writes, so there are
     * fewer opportunities to initiate TXKUs. Note that we ask for a TXKU every
     * 1 packet above, which is absurd; thus this ensures we only actually
     * generate TXKUs when we are allowed to.
     */
    OP_CHECK                (check_key_update_lt, 240)

    /*
     * Prove the connection is still healthy by sending something in both
     * directions.
     */
    OP_C_WRITE              (DEFAULT, "xyzzy", 5)
    OP_S_READ_EXPECT        (a, "xyzzy", 5)

    OP_S_WRITE              (a, "plugh", 5)
    OP_C_READ_EXPECT        (DEFAULT, "plugh", 5)

    OP_END
};

/* 19. Key update test - artificially triggered */
static const struct script_op script_19[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_C_WRITE              (DEFAULT, "orange", 6)
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_S_WRITE              (a, "strawberry", 10)
    OP_C_READ_EXPECT        (DEFAULT, "strawberry", 10)

    OP_CHECK                (check_key_update_lt, 1)
    OP_CHECK                (trigger_key_update, 0)

    OP_C_WRITE              (DEFAULT, "orange", 6)
    OP_S_READ_EXPECT        (a, "orange", 6)
    OP_S_WRITE              (a, "ok", 2)

    OP_C_READ_EXPECT        (DEFAULT, "ok", 2)
    OP_CHECK                (check_key_update_ge, 1)

    OP_END
};

/* 20. Multiple threads accept stream with socket forcibly closed (error test) */
static int script_20_trigger(struct helper *h, volatile uint64_t *counter)
{
#if defined(OPENSSL_THREADS)
    ossl_crypto_mutex_lock(h->misc_m);
    ++*counter;
    ossl_crypto_condvar_broadcast(h->misc_cv);
    ossl_crypto_mutex_unlock(h->misc_m);
#endif
    return 1;
}

static int script_20_wait(struct helper *h, volatile uint64_t *counter, uint64_t threshold)
{
#if defined(OPENSSL_THREADS)
    int stop = 0;

    ossl_crypto_mutex_lock(h->misc_m);
    while (!stop) {
        stop = (*counter >= threshold);
        if (stop)
            break;

        ossl_crypto_condvar_wait(h->misc_cv, h->misc_m);
    }

    ossl_crypto_mutex_unlock(h->misc_m);
#endif
    return 1;
}

static int script_20_trigger1(struct helper *h, struct helper_local *hl)
{
    return script_20_trigger(h, &h->scratch0);
}

static int script_20_wait1(struct helper *h, struct helper_local *hl)
{
    return script_20_wait(h, &h->scratch0, hl->check_op->arg2);
}

static int script_20_trigger2(struct helper *h, struct helper_local *hl)
{
    return script_20_trigger(h, &h->scratch1);
}

static int script_20_wait2(struct helper *h, struct helper_local *hl)
{
    return script_20_wait(h, &h->scratch1, hl->check_op->arg2);
}

static const struct script_op script_20_child[] = {
    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "foo", 3)

    OP_CHECK                (script_20_trigger1, 0)
    OP_CHECK                (script_20_wait2, 1)

    OP_C_READ_FAIL_WAIT     (a)
    OP_C_EXPECT_SSL_ERR     (a, SSL_ERROR_SYSCALL)

    OP_EXPECT_ERR_LIB       (ERR_LIB_SSL)
    OP_EXPECT_ERR_REASON    (SSL_R_PROTOCOL_IS_SHUTDOWN)

    OP_POP_ERR              ()
    OP_EXPECT_ERR_LIB       (ERR_LIB_SSL)
    OP_EXPECT_ERR_REASON    (SSL_R_QUIC_NETWORK_ERROR)

    OP_C_FREE_STREAM        (a)

    OP_END
};

static const struct script_op script_20[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_NEW_THREAD           (5, script_20_child)

    OP_BEGIN_REPEAT         (5)

    OP_S_NEW_STREAM_BIDI    (a, ANY_ID)
    OP_S_WRITE              (a, "foo", 3)
    OP_S_UNBIND_STREAM_ID   (a)

    OP_END_REPEAT           ()

    OP_CHECK                (script_20_wait1, 5)

    OP_C_CLOSE_SOCKET       ()
    OP_CHECK                (script_20_trigger2, 0)

    OP_END
};

/* 21. Fault injection - unknown frame in 1-RTT packet */
static int script_21_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[21];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != h->inject_word0)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1)))
        goto err;

    switch (h->inject_word1) {
    case OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE:
    case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
    case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
        /*
         * These cases to be formatted properly need a single uint64_t
         */
        if (!TEST_true(WPACKET_put_bytes_u64(&wpkt, (uint64_t)0)))
            goto err;
        break;
    case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI:
    case OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI:
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
    case OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED:
        /*
         * These cases require a single vlint
         */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        break;
    case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
    case OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA:
    case OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED:
        /*
         * These cases require 2 variable integers
         */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        break;
    case OSSL_QUIC_FRAME_TYPE_STREAM:
    case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
        /*
         * These cases require 3 variable integers
         */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;
        break;
    case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
        /*
         * Special case for new token
         */

        /* New token length, cannot be zero */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)1)))
            goto err;

        /* 1 bytes of token data, to match the above length */
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, (uint8_t)0)))
            goto err;
        break;
    case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
        /*
         * Special case for New Connection ids, has a combination
         * of vlints and fixed width values
         */

        /* seq number */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;

        /* retire prior to */
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, (uint64_t)0)))
            goto err;

        /* Connection id length, arbitrary at 1 bytes */
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, (uint8_t)1)))
            goto err;

        /* The connection id, to match the above length */
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, (uint8_t)0)))
            goto err;

        /* 16 bytes total for the SRT */
        if (!TEST_true(WPACKET_memset(&wpkt, 0, 16)))
            goto err;

        break;
    }

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_21[] = {
    OP_S_SET_INJECT_PLAIN   (script_21_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (QUIC_PKT_TYPE_1RTT, OSSL_QUIC_VLINT_MAX)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 22. Fault injection - non-zero packet header reserved bits */
static int script_22_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    if (h->inject_word0 == 0)
        return 1;

    hdr->reserved = 1;
    return 1;
}

static const struct script_op script_22[] = {
    OP_S_SET_INJECT_PLAIN   (script_22_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_PROTOCOL_VIOLATION,0,0)

    OP_END
};

/* 23. Fault injection - empty NEW_TOKEN */
static int script_23_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[16];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_NEW_TOKEN))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 0)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_23[] = {
    OP_S_SET_INJECT_PLAIN   (script_23_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 24. Fault injection - excess value of MAX_STREAMS_BIDI */
static int script_24_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[16];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, (((uint64_t)1) << 60) + 1)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_24[] = {
    OP_S_SET_INJECT_PLAIN   (script_24_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 25. Fault injection - excess value of MAX_STREAMS_UNI */
static const struct script_op script_25[] = {
    OP_S_SET_INJECT_PLAIN   (script_24_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 26. Fault injection - excess value of STREAMS_BLOCKED_BIDI */
static const struct script_op script_26[] = {
    OP_S_SET_INJECT_PLAIN   (script_24_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,0,0)

    OP_END
};

/* 27. Fault injection - excess value of STREAMS_BLOCKED_UNI */
static const struct script_op script_27[] = {
    OP_S_SET_INJECT_PLAIN   (script_24_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,0,0)

    OP_END
};

/* 28. Fault injection - received RESET_STREAM for send-only stream */
static int script_28_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[32];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /* stream ID */
                                               h->inject_word0 - 1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 123))
        || (h->inject_word1 == OSSL_QUIC_FRAME_TYPE_RESET_STREAM
           && !TEST_true(WPACKET_quic_write_vlint(&wpkt, 5)))) /* final size */
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_28[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "orange", 6)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_C_NEW_STREAM_UNI     (b, C_UNI_ID(0))
    OP_C_WRITE              (b, "apple", 5)

    OP_S_BIND_STREAM_ID     (b, C_UNI_ID(0))
    OP_S_READ_EXPECT        (b, "apple", 5)

    OP_SET_INJECT_WORD      (C_UNI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_RESET_STREAM)
    OP_S_WRITE              (a, "fruit", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 29. Fault injection - received RESET_STREAM for nonexistent send-only stream */
static const struct script_op script_29[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "orange", 6)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_C_NEW_STREAM_UNI     (b, C_UNI_ID(0))
    OP_C_WRITE              (b, "apple", 5)

    OP_S_BIND_STREAM_ID     (b, C_UNI_ID(0))
    OP_S_READ_EXPECT        (b, "apple", 5)

    OP_SET_INJECT_WORD      (C_UNI_ID(1) + 1, OSSL_QUIC_FRAME_TYPE_RESET_STREAM)
    OP_S_WRITE              (a, "fruit", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 30. Fault injection - received STOP_SENDING for receive-only stream */
static const struct script_op script_30[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (S_UNI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_STOP_SENDING)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 31. Fault injection - received STOP_SENDING for nonexistent receive-only stream */
static const struct script_op script_31[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_UNI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_STOP_SENDING)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 32. Fault injection - STREAM frame for nonexistent stream */
static int script_32_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[64];
    size_t written;
    uint64_t type = OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN, offset, flen, i;

    if (hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    switch (h->inject_word1) {
    default:
        return 0;
    case 0:
        return 1;
    case 1:
        offset  = 0;
        flen    = 0;
        break;
    case 2:
        offset  = (((uint64_t)1)<<62) - 1;
        flen    = 5;
        break;
    case 3:
        offset  = 1 * 1024 * 1024 * 1024; /* 1G */
        flen    = 5;
        break;
    case 4:
        offset  = 0;
        flen    = 1;
        break;
    }

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, type))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /* stream ID */
                                               h->inject_word0 - 1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, offset))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, flen)))
        goto err;

    for (i = 0; i < flen; ++i)
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x42)))
            goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_32[] = {
    OP_S_SET_INJECT_PLAIN   (script_32_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_UNI_ID(0) + 1, 1)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 33. Fault injection - STREAM frame with illegal offset */
static const struct script_op script_33[] = {
    OP_S_SET_INJECT_PLAIN   (script_32_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_BIDI_ID(0) + 1, 2)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 34. Fault injection - STREAM frame which exceeds FC */
static const struct script_op script_34[] = {
    OP_S_SET_INJECT_PLAIN   (script_32_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_BIDI_ID(0) + 1, 3)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FLOW_CONTROL_ERROR,0,0)

    OP_END
};

/* 35. Fault injection - MAX_STREAM_DATA for receive-only stream */
static const struct script_op script_35[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (S_UNI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 36. Fault injection - MAX_STREAM_DATA for nonexistent stream */
static const struct script_op script_36[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_BIDI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 37. Fault injection - STREAM_DATA_BLOCKED for send-only stream */
static const struct script_op script_37[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_UNI     (a, C_UNI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_UNI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_S_NEW_STREAM_UNI     (b, S_UNI_ID(0))
    OP_SET_INJECT_WORD      (C_UNI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED)
    OP_S_WRITE              (b, "orange", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 38. Fault injection - STREAM_DATA_BLOCKED for non-existent stream */
static const struct script_op script_38[] = {
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_UNI     (a, C_UNI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_UNI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (C_BIDI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED)

    OP_S_NEW_STREAM_UNI     (b, S_UNI_ID(0))
    OP_S_WRITE              (b, "orange", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 39. Fault injection - NEW_CONN_ID with zero-len CID */
static int script_39_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[64];
    size_t i, written;
    uint64_t seq_no = 0, retire_prior_to = 0;
    QUIC_CONN_ID new_cid = {0};
    QUIC_CHANNEL *ch = ossl_quic_tserver_get_channel(h->s_priv);

    if (hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    switch (h->inject_word1) {
    case 0:
        return 1;
    case 1:
        new_cid.id_len  = 0;
        break;
    case 2:
        new_cid.id_len  = 21;
        break;
    case 3:
        new_cid.id_len  = 1;
        new_cid.id[0]   = 0x55;

        seq_no          = 0;
        retire_prior_to = 1;
        break;
    case 4:
        /* Use our actual CID so we don't break connectivity. */
        ossl_quic_channel_get_diag_local_cid(ch, &new_cid);

        seq_no          = 2;
        retire_prior_to = 2;
        break;
    case 5:
        /*
         * Use a bogus CID which will need to be ignored if connectivity is to
         * be continued.
         */
        new_cid.id_len  = 8;
        new_cid.id[0]   = 0x55;

        seq_no          = 1;
        retire_prior_to = 1;
        break;
    }

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, seq_no)) /* seq no */
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, retire_prior_to)) /* retire prior to */
        || !TEST_true(WPACKET_put_bytes_u8(&wpkt, new_cid.id_len))) /* len */
        goto err;

    for (i = 0; i < new_cid.id_len && i < OSSL_NELEM(new_cid.id); ++i)
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, new_cid.id[i])))
            goto err;

    for (; i < new_cid.id_len; ++i)
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x55)))
            goto err;

    for (i = 0; i < QUIC_STATELESS_RESET_TOKEN_LEN; ++i)
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x42)))
            goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_39[] = {
    OP_S_SET_INJECT_PLAIN   (script_39_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (0, 1)
    OP_S_WRITE              (a, "orange", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 40. Shutdown flush test */
static const unsigned char script_40_data[1024] = "strawberry";

static const struct script_op script_40[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_C_INHIBIT_TICK       (1)
    OP_C_SET_WRITE_BUF_SIZE (a, 1024 * 100 * 3)

    OP_BEGIN_REPEAT         (100)

    OP_C_WRITE              (a, script_40_data, sizeof(script_40_data))

    OP_END_REPEAT           ()

    OP_C_CONCLUDE           (a)
    OP_C_SHUTDOWN_WAIT      (NULL, 0) /* disengages tick inhibition */

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_BEGIN_REPEAT         (100)

    OP_S_READ_EXPECT        (a, script_40_data, sizeof(script_40_data))

    OP_END_REPEAT           ()

    OP_S_EXPECT_FIN         (a)

    OP_C_EXPECT_CONN_CLOSE_INFO(0, 1, 0)
    OP_S_EXPECT_CONN_CLOSE_INFO(0, 1, 1)

    OP_END
};

/* 41. Fault injection - PATH_CHALLENGE yields PATH_RESPONSE */
static const uint64_t path_challenge = UINT64_C(0xbdeb9451169c83aa);

static int script_41_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[16];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1))
        || !TEST_true(WPACKET_put_bytes_u64(&wpkt, path_challenge)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written))
        || !TEST_size_t_eq(written, 9))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    --h->inject_word0;
    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static void script_41_trace(int write_p, int version, int content_type,
                            const void *buf, size_t len, SSL *ssl, void *arg)
{
    uint64_t frame_type, frame_data;
    int was_minimal;
    struct helper *h = arg;
    PACKET pkt;

    if (version != OSSL_QUIC1_VERSION
        || content_type != SSL3_RT_QUIC_FRAME_FULL
        || len < 1)
        return;

    if (!TEST_true(PACKET_buf_init(&pkt, buf, len))) {
        ++h->scratch1;
        return;
    }

    if (!TEST_true(ossl_quic_wire_peek_frame_header(&pkt, &frame_type,
                                                    &was_minimal))) {
        ++h->scratch1;
        return;
    }

    if (frame_type != OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE)
        return;

   if (!TEST_true(ossl_quic_wire_decode_frame_path_response(&pkt, &frame_data))
       || !TEST_uint64_t_eq(frame_data, path_challenge)) {
       ++h->scratch1;
        return;
   }

   ++h->scratch0;
}

static int script_41_setup(struct helper *h, struct helper_local *hl)
{
    ossl_quic_tserver_set_msg_callback(ACQUIRE_S(), script_41_trace, h);
    return 1;
}

static int script_41_check(struct helper *h, struct helper_local *hl)
{
    /* At least one valid challenge/response echo? */
    if (!TEST_uint64_t_gt(h->scratch0, 0))
        return 0;

    /* No failed tests? */
    if (!TEST_uint64_t_eq(h->scratch1, 0))
        return 0;

    return 1;
}

static const struct script_op script_41[] = {
    OP_S_SET_INJECT_PLAIN   (script_41_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_CHECK                (script_41_setup, 0)

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "strawberry", 10)
    OP_S_READ_EXPECT        (a, "strawberry", 10)

    OP_CHECK                (script_41_check, 0)
    OP_END
};

/* 42. Fault injection - CRYPTO frame with illegal offset */
static int script_42_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    unsigned char frame_buf[64];
    size_t written;
    WPACKET wpkt;

    if (h->inject_word0 == 0)
        return 1;

    --h->inject_word0;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_CRYPTO))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 1))
        || !TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x42)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_42[] = {
    OP_S_SET_INJECT_PLAIN   (script_42_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, (((uint64_t)1) << 62) - 1)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 43. Fault injection - CRYPTO frame exceeding FC */
static const struct script_op script_43[] = {
    OP_S_SET_INJECT_PLAIN   (script_42_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0x100000 /* 1 MiB */)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_CRYPTO_BUFFER_EXCEEDED,0,0)

    OP_END
};

/* 44. Fault injection - PADDING */
static int script_44_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[16];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(ossl_quic_wire_encode_padding(&wpkt, 1)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_44[] = {
    OP_S_SET_INJECT_PLAIN   (script_44_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)

    OP_S_WRITE              (a, "Strawberry", 10)
    OP_C_READ_EXPECT        (DEFAULT, "Strawberry", 10)

    OP_END
};

/* 45. PING must generate ACK */
static int force_ping(struct helper *h, struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_tserver_get_channel(ACQUIRE_S());

    h->scratch0 = ossl_quic_channel_get_diag_num_rx_ack(ch);

    if (!TEST_true(ossl_quic_tserver_ping(ACQUIRE_S())))
        return 0;

    return 1;
}

static int wait_incoming_acks_increased(struct helper *h, struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_tserver_get_channel(ACQUIRE_S());
    uint16_t count;

    count = ossl_quic_channel_get_diag_num_rx_ack(ch);

    if (count == h->scratch0) {
        h->check_spin_again = 1;
        return 0;
    }

    return 1;
}

static const struct script_op script_45[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_BEGIN_REPEAT         (2)

    OP_CHECK                (force_ping, 0)
    OP_CHECK                (wait_incoming_acks_increased, 0)

    OP_END_REPEAT           ()

    OP_S_WRITE              (a, "Strawberry", 10)
    OP_C_READ_EXPECT        (DEFAULT, "Strawberry", 10)

    OP_END
};

/* 46. Fault injection - ACK - malformed initial range */
static int script_46_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[16];
    size_t written;
    uint64_t type = 0, largest_acked = 0, first_range = 0, range_count = 0;
    uint64_t agap = 0, alen = 0;
    uint64_t ect0 = 0, ect1 = 0, ecnce = 0;

    if (h->inject_word0 == 0)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    type = OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN;

    switch (h->inject_word0) {
    case 1:
        largest_acked   = 100;
        first_range     = 101;
        range_count     = 0;
        break;
    case 2:
        largest_acked   = 100;
        first_range     = 80;
        /* [20..100]; [0..18]  */
        range_count     = 1;
        agap            = 0;
        alen            = 19;
        break;
    case 3:
        largest_acked   = 100;
        first_range     = 80;
        range_count     = 1;
        agap            = 18;
        alen            = 1;
        break;
    case 4:
        type            = OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN;
        largest_acked   = 100;
        first_range     = 1;
        range_count     = 0;
        break;
    case 5:
        type            = OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN;
        largest_acked   = 0;
        first_range     = 0;
        range_count     = 0;
        ect0            = 0;
        ect1            = 50;
        ecnce           = 200;
        break;
    }

    h->inject_word0 = 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, type))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, largest_acked))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /*ack_delay=*/0))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /*ack_range_count=*/range_count))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /*first_ack_range=*/first_range)))
        goto err;

    if (range_count > 0)
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, /*range[0].gap=*/agap))
            || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /*range[0].len=*/alen)))
            goto err;

    if (type == OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN)
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, ect0))
            || !TEST_true(WPACKET_quic_write_vlint(&wpkt, ect1))
            || !TEST_true(WPACKET_quic_write_vlint(&wpkt, ecnce)))
            goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_46[] = {
    OP_S_SET_INJECT_PLAIN   (script_46_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)

    OP_S_WRITE              (a, "Strawberry", 10)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 47. Fault injection - ACK - malformed subsequent range */
static const struct script_op script_47[] = {
    OP_S_SET_INJECT_PLAIN   (script_46_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (2, 0)

    OP_S_WRITE              (a, "Strawberry", 10)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 48. Fault injection - ACK - malformed subsequent range */
static const struct script_op script_48[] = {
    OP_S_SET_INJECT_PLAIN   (script_46_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (3, 0)

    OP_S_WRITE              (a, "Strawberry", 10)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 49. Fault injection - ACK - fictional PN */
static const struct script_op script_49[] = {
    OP_S_SET_INJECT_PLAIN   (script_46_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (4, 0)

    OP_S_WRITE              (a, "Strawberry", 10)
    OP_C_READ_EXPECT        (DEFAULT, "Strawberry", 10)

    OP_END
};

/* 50. Fault injection - ACK - duplicate PN */
static const struct script_op script_50[] = {
    OP_S_SET_INJECT_PLAIN   (script_46_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_BEGIN_REPEAT         (2)

    OP_SET_INJECT_WORD      (5, 0)

    OP_S_WRITE              (a, "Strawberry", 10)
    OP_C_READ_EXPECT        (DEFAULT, "Strawberry", 10)

    OP_END_REPEAT           ()

    OP_END
};

/* 51. Fault injection - PATH_RESPONSE is ignored */
static const struct script_op script_51[] = {
    OP_S_SET_INJECT_PLAIN   (script_41_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_END
};

/* 52. Fault injection - ignore BLOCKED frames with bogus values */
static int script_52_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    unsigned char frame_buf[64];
    size_t written;
    WPACKET wpkt;
    uint64_t type = h->inject_word1;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    --h->inject_word0;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, type)))
        goto err;

    if (type == OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED)
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, C_BIDI_ID(0))))
            goto err;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, 0xFFFFFF)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_52[] = {
    OP_S_SET_INJECT_PLAIN   (script_52_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_END
};

/* 53. Fault injection - excess CRYPTO buffer size */
static int script_53_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    size_t written;
    WPACKET wpkt;
    uint64_t offset = 0, data_len = 100;
    unsigned char *frame_buf = NULL;
    size_t frame_len, i;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    h->inject_word0 = 0;

    switch (h->inject_word1) {
    case 0:
        /*
         * Far out offset which will not have been reached during handshake.
         * This will not be delivered to the QUIC_TLS instance since it will be
         * waiting for in-order delivery of previous bytes. This tests our flow
         * control on CRYPTO stream buffering.
         */
        offset      = 100000;
        data_len    = 1;
        break;
    }

    frame_len = 1 + 8 + 8 + (size_t)data_len;
    if (!TEST_ptr(frame_buf = OPENSSL_malloc(frame_len)))
        return 0;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf, frame_len, 0)))
        goto err;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_CRYPTO))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, offset))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, data_len)))
        goto err;

    for (i = 0; i < data_len; ++i)
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x42)))
            goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    OPENSSL_free(frame_buf);
    return ok;
}

static const struct script_op script_53[] = {
    OP_S_SET_INJECT_PLAIN   (script_53_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)
    OP_S_WRITE              (a, "Strawberry", 10)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_CRYPTO_BUFFER_EXCEEDED,0,0)

    OP_END
};

/* 54. Fault injection - corrupted crypto stream data */
static int script_54_inject_handshake(struct helper *h,
                                      unsigned char *buf, size_t buf_len)
{
    size_t i;

    for (i = 0; i < buf_len; ++i)
        buf[i] ^= 0xff;

    return 1;
}

static const struct script_op script_54[] = {
    OP_S_SET_INJECT_HANDSHAKE(script_54_inject_handshake)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT_OR_FAIL()

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_CRYPTO_UNEXPECTED_MESSAGE,0,0)

    OP_END
};

/* 55. Fault injection - NEW_CONN_ID with >20 byte CID */
static const struct script_op script_55[] = {
    OP_S_SET_INJECT_PLAIN   (script_39_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (0, 2)
    OP_S_WRITE              (a, "orange", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 56. Fault injection - NEW_CONN_ID with seq no < retire prior to */
static const struct script_op script_56[] = {
    OP_S_SET_INJECT_PLAIN   (script_39_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (0, 3)
    OP_S_WRITE              (a, "orange", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

/* 57. Fault injection - NEW_CONN_ID with lower seq so ignored */
static const struct script_op script_57[] = {
    OP_S_SET_INJECT_PLAIN   (script_39_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (0, 4)
    OP_S_WRITE              (a, "orange", 5)
    OP_C_READ_EXPECT        (a, "orange", 5)

    OP_C_WRITE              (a, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    /*
     * Now we send a NEW_CONN_ID with a bogus CID. However the sequence number
     * is old so it should be ignored and we should still be able to
     * communicate.
     */
    OP_SET_INJECT_WORD      (0, 5)
    OP_S_WRITE              (a, "raspberry", 9)
    OP_C_READ_EXPECT        (a, "raspberry", 9)

    OP_C_WRITE              (a, "peach", 5)
    OP_S_READ_EXPECT        (a, "peach", 5)

    OP_END
};

/* 58. Fault injection - repeated HANDSHAKE_DONE */
static int script_58_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    unsigned char frame_buf[64];
    size_t written;
    WPACKET wpkt;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (h->inject_word0 == 1) {
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE)))
            goto err;
    } else {
        /* Needless multi-byte encoding */
        if (!TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x40))
            || !TEST_true(WPACKET_put_bytes_u8(&wpkt, 0x1E)))
            goto err;
    }

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_58[] = {
    OP_S_SET_INJECT_PLAIN   (script_58_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)

    OP_C_WRITE              (DEFAULT, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_END
};

/* 59. Fault injection - multi-byte frame encoding */
static const struct script_op script_59[] = {
    OP_S_SET_INJECT_PLAIN   (script_58_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (2, 0)

    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_PROTOCOL_VIOLATION,0,0)

    OP_END
};

/* 60. Connection close reason truncation */
static char long_reason[2048];

static int init_reason(struct helper *h, struct helper_local *hl)
{
    memset(long_reason, '~', sizeof(long_reason));
    memcpy(long_reason, "This is a long reason string.", 29);
    long_reason[OSSL_NELEM(long_reason) - 1] = '\0';
    return 1;
}

static int check_shutdown_reason(struct helper *h, struct helper_local *hl)
{
    const QUIC_TERMINATE_CAUSE *tc = ossl_quic_tserver_get_terminate_cause(ACQUIRE_S());

    if (tc == NULL) {
        h->check_spin_again = 1;
        return 0;
    }

    if (!TEST_size_t_ge(tc->reason_len, 50)
        || !TEST_mem_eq(long_reason, tc->reason_len,
                        tc->reason, tc->reason_len))
        return 0;

    return 1;
}

static const struct script_op script_60[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_WRITE              (DEFAULT, "apple", 5)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_CHECK                (init_reason, 0)
    OP_C_SHUTDOWN_WAIT      (long_reason, 0)
    OP_CHECK                (check_shutdown_reason, 0)

    OP_END
};

/* 61. Fault injection - RESET_STREAM exceeding stream count FC */
static int script_61_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[32];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word0))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, /* stream ID */
                                               h->inject_word1))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 123))
        || (h->inject_word0 == OSSL_QUIC_FRAME_TYPE_RESET_STREAM
           && !TEST_true(WPACKET_quic_write_vlint(&wpkt, 0)))) /* final size */
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_61[] = {
    OP_S_SET_INJECT_PLAIN   (script_61_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "orange", 6)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_SET_INJECT_WORD      (OSSL_QUIC_FRAME_TYPE_RESET_STREAM,
                             S_BIDI_ID(OSSL_QUIC_VLINT_MAX / 4))
    OP_S_WRITE              (a, "fruit", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,0,0)

    OP_END
};

/* 62. Fault injection - STOP_SENDING with high ID */
static const struct script_op script_62[] = {
    OP_S_SET_INJECT_PLAIN   (script_61_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "orange", 6)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "orange", 6)

    OP_SET_INJECT_WORD      (OSSL_QUIC_FRAME_TYPE_STOP_SENDING,
                             C_BIDI_ID(OSSL_QUIC_VLINT_MAX / 4))
    OP_S_WRITE              (a, "fruit", 5)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_STATE_ERROR,0,0)

    OP_END
};

/* 63. Fault injection - STREAM frame exceeding stream limit */
static const struct script_op script_63[] = {
    OP_S_SET_INJECT_PLAIN   (script_32_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (S_BIDI_ID(5000) + 1, 4)
    OP_S_WRITE              (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_STREAM_LIMIT_ERROR,0,0)

    OP_END
};

/* 64. Fault injection - STREAM - zero-length no-FIN is accepted */
static const struct script_op script_64[] = {
    OP_S_SET_INJECT_PLAIN   (script_32_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_UNI     (a, S_UNI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (S_BIDI_ID(20) + 1, 1)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)

    OP_END
};

/* 65. Fault injection - CRYPTO - zero-length is accepted */
static int script_65_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    unsigned char frame_buf[64];
    size_t written;
    WPACKET wpkt;

    if (h->inject_word0 == 0)
        return 1;

    --h->inject_word0;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_FRAME_TYPE_CRYPTO))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 0))
        || !TEST_true(WPACKET_quic_write_vlint(&wpkt, 0)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_65[] = {
    OP_S_SET_INJECT_PLAIN   (script_65_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, 0)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)

    OP_END
};

/* 66. Fault injection - large MAX_STREAM_DATA */
static int script_66_inject_plain(struct helper *h, QUIC_PKT_HDR *hdr,
                                  unsigned char *buf, size_t len)
{
    int ok = 0;
    WPACKET wpkt;
    unsigned char frame_buf[64];
    size_t written;

    if (h->inject_word0 == 0 || hdr->type != QUIC_PKT_TYPE_1RTT)
        return 1;

    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, h->inject_word1)))
        goto err;

    if (h->inject_word1 == OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
        if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, /* stream ID */
                                                h->inject_word0 - 1)))
            goto err;

    if (!TEST_true(WPACKET_quic_write_vlint(&wpkt, OSSL_QUIC_VLINT_MAX)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &written)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, written))
        goto err;

    ok = 1;
err:
    if (ok)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);
    return ok;
}

static const struct script_op script_66[] = {
    OP_S_SET_INJECT_PLAIN   (script_66_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (S_BIDI_ID(0) + 1, OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)
    OP_C_WRITE              (a, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_END
};

/* 67. Fault injection - large MAX_DATA */
static const struct script_op script_67[] = {
    OP_S_SET_INJECT_PLAIN   (script_66_inject_plain)
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "apple", 5)

    OP_C_ACCEPT_STREAM_WAIT (a)
    OP_C_READ_EXPECT        (a, "apple", 5)

    OP_SET_INJECT_WORD      (1, OSSL_QUIC_FRAME_TYPE_MAX_DATA)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)
    OP_C_WRITE              (a, "Strawberry", 10)
    OP_S_READ_EXPECT        (a, "Strawberry", 10)

    OP_END
};

/* 68. Fault injection - Unexpected TLS messages */
static int script_68_inject_handshake(struct helper *h, unsigned char *msg,
                                      size_t msglen)
{
    const unsigned char *data;
    size_t datalen;
    const unsigned char certreq[] = {
        SSL3_MT_CERTIFICATE_REQUEST,         /* CertificateRequest message */
        0, 0, 12,                            /* Length of message */
        1, 1,                                /* certificate_request_context */
        0, 8,                                /* Extensions block length */
        0, TLSEXT_TYPE_signature_algorithms, /* sig_algs extension*/
        0, 4,                                /* 4 bytes of sig algs extension*/
        0, 2,                                /* sigalgs list is 2 bytes long */
        8, 4                                 /* rsa_pss_rsae_sha256 */
    };
    const unsigned char keyupdate[] = {
        SSL3_MT_KEY_UPDATE,                  /* KeyUpdate message */
        0, 0, 1,                             /* Length of message */
        SSL_KEY_UPDATE_NOT_REQUESTED         /* update_not_requested */
    };

    /* We transform the NewSessionTicket message into something else */
    switch(h->inject_word0) {
    case 0:
        return 1;

    case 1:
        /* CertificateRequest message */
        data = certreq;
        datalen = sizeof(certreq);
        break;

    case 2:
        /* KeyUpdate message */
        data = keyupdate;
        datalen = sizeof(keyupdate);
        break;

    default:
        return 0;
    }

    if (!TEST_true(qtest_fault_resize_message(h->qtf,
                                              datalen - SSL3_HM_HEADER_LENGTH)))
        return 0;

    memcpy(msg, data, datalen);

    return 1;
}

/* Send a CerticateRequest message post-handshake */
static const struct script_op script_68[] = {
    OP_S_SET_INJECT_HANDSHAKE(script_68_inject_handshake)
    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT        ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI     (a, C_BIDI_ID(0))
    OP_C_WRITE               (a, "apple", 5)
    OP_S_BIND_STREAM_ID      (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT         (a, "apple", 5)

    OP_SET_INJECT_WORD       (1, 0)
    OP_S_NEW_TICKET          ()
    OP_S_WRITE               (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_PROTOCOL_VIOLATION, 0, 0)

    OP_END
};

/* 69. Send a TLS KeyUpdate message post-handshake */
static const struct script_op script_69[] = {
    OP_S_SET_INJECT_HANDSHAKE(script_68_inject_handshake)
    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT        ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI     (a, C_BIDI_ID(0))
    OP_C_WRITE               (a, "apple", 5)
    OP_S_BIND_STREAM_ID      (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT         (a, "apple", 5)

    OP_SET_INJECT_WORD       (2, 0)
    OP_S_NEW_TICKET          ()
    OP_S_WRITE               (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_CRYPTO_ERR_BEGIN
                                + SSL_AD_UNEXPECTED_MESSAGE, 0, 0)

    OP_END
};

static int set_max_early_data(struct helper *h, struct helper_local *hl)
{

    if (!TEST_true(ossl_quic_tserver_set_max_early_data(ACQUIRE_S(),
                                                        (uint32_t)hl->check_op->arg2)))
        return 0;

    return 1;
}

/* 70. Send a TLS NewSessionTicket message with invalid max_early_data */
static const struct script_op script_70[] = {
    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT        ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI     (a, C_BIDI_ID(0))
    OP_C_WRITE               (a, "apple", 5)
    OP_S_BIND_STREAM_ID      (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT         (a, "apple", 5)

    OP_CHECK                 (set_max_early_data, 0xfffffffe)
    OP_S_NEW_TICKET          ()
    OP_S_WRITE               (a, "orange", 6)

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_PROTOCOL_VIOLATION, 0, 0)

    OP_END
};

/* 71. Send a TLS NewSessionTicket message with valid max_early_data */
static const struct script_op script_71[] = {
    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT        ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI     (a, C_BIDI_ID(0))
    OP_C_WRITE               (a, "apple", 5)
    OP_S_BIND_STREAM_ID      (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT         (a, "apple", 5)

    OP_CHECK                 (set_max_early_data, 0xffffffff)
    OP_S_NEW_TICKET          ()
    OP_S_WRITE               (a, "orange", 6)
    OP_C_READ_EXPECT         (a, "orange", 6)

    OP_END
};

/* 72. Test that APL stops handing out streams after limit reached (bidi) */
static int script_72_check(struct helper *h, struct helper_local *hl)
{
    if (!TEST_uint64_t_ge(h->fail_count, 50))
        return 0;

    return 1;
}

static const struct script_op script_72[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    /*
     * Request more streams than a server will initially hand out and test that
     * they fail properly.
     */
    OP_BEGIN_REPEAT         (200)

    OP_C_NEW_STREAM_BIDI_EX (a, ANY_ID, ALLOW_FAIL | SSL_STREAM_FLAG_NO_BLOCK)
    OP_C_SKIP_IF_UNBOUND    (a, 2)
    OP_C_WRITE              (a, "apple", 5)
    OP_C_FREE_STREAM        (a)

    OP_END_REPEAT           ()

    OP_CHECK                (script_72_check, 0)

    OP_END
};

/* 73. Test that APL stops handing out streams after limit reached (uni) */
static const struct script_op script_73[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    /*
     * Request more streams than a server will initially hand out and test that
     * they fail properly.
     */
    OP_BEGIN_REPEAT         (200)

    OP_C_NEW_STREAM_UNI_EX  (a, ANY_ID, ALLOW_FAIL | SSL_STREAM_FLAG_NO_BLOCK)
    OP_C_SKIP_IF_UNBOUND    (a, 2)
    OP_C_WRITE              (a, "apple", 5)
    OP_C_FREE_STREAM        (a)

    OP_END_REPEAT           ()

    OP_CHECK                (script_72_check, 0)

    OP_END
};

/* 74. Version negotiation: QUIC_VERSION_1 ignored */
static int generate_version_neg(WPACKET *wpkt, uint32_t version)
{
    QUIC_PKT_HDR hdr = {0};

    hdr.type                = QUIC_PKT_TYPE_VERSION_NEG;
    hdr.version             = 0;
    hdr.fixed               = 1;
    hdr.dst_conn_id.id_len  = 0;
    hdr.src_conn_id.id_len  = 8;
    memset(hdr.src_conn_id.id, 0x55, 8);

    if (!TEST_true(ossl_quic_wire_encode_pkt_hdr(wpkt, 0, &hdr, NULL)))
        return 0;

    if (!TEST_true(WPACKET_put_bytes_u32(wpkt, version)))
        return 0;

    return 1;
}

static int server_gen_version_neg(struct helper *h, BIO_MSG *msg, size_t stride)
{
    int rc = 0, have_wpkt = 0;
    size_t l;
    WPACKET wpkt;
    BUF_MEM *buf = NULL;
    uint32_t version;

    switch (h->inject_word0) {
    case 0:
        return 1;
    case 1:
        version = QUIC_VERSION_1;
        break;
    default:
        version = 0x5432abcd;
        break;
    }

    if (!TEST_ptr(buf = BUF_MEM_new()))
        goto err;

    if (!TEST_true(WPACKET_init(&wpkt, buf)))
        goto err;

    have_wpkt = 1;

    generate_version_neg(&wpkt, version);

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &l)))
        goto err;

    if (!TEST_true(qtest_fault_resize_datagram(h->qtf, l)))
        return 0;

    memcpy(msg->data, buf->data, l);
    h->inject_word0 = 0;

    rc = 1;
err:
    if (have_wpkt)
        WPACKET_finish(&wpkt);

    BUF_MEM_free(buf);
    return rc;
}

static int do_mutation = 0;
static QUIC_PKT_HDR *hdr_to_free = NULL;

/*
 * Check packets to transmit, if we have an initial packet
 * Modify the version number to something incorrect
 * so that we trigger a version negotiation
 * Note, this is a use once function, it will only modify the
 * first INITIAL packet it sees, after which it needs to be
 * armed again
 */
static int script_74_alter_version(const QUIC_PKT_HDR *hdrin,
                                   const OSSL_QTX_IOVEC *iovecin, size_t numin,
                                   QUIC_PKT_HDR **hdrout,
                                   const OSSL_QTX_IOVEC **iovecout,
                                   size_t *numout,
                                   void *arg)
{
    *hdrout = OPENSSL_memdup(hdrin, sizeof(QUIC_PKT_HDR));
    *iovecout = iovecin;
    *numout = numin;
    hdr_to_free = *hdrout;

    if (do_mutation == 0)
        return 1;
    do_mutation = 0;

    if (hdrin->type == QUIC_PKT_TYPE_INITIAL)
        (*hdrout)->version = 0xdeadbeef;
    return 1;
}

static void script_74_finish_mutation(void *arg)
{
    OPENSSL_free(hdr_to_free);
}

/*
 * Enable the packet mutator for the client channel
 * So that when we send a Initial packet
 * We modify the version to be something invalid
 * to force a version negotiation
 */
static int script_74_arm_packet_mutator(struct helper *h,
                                        struct helper_local *hl)
{
    QUIC_CHANNEL *ch = ossl_quic_conn_get_channel(h->c_conn);

    do_mutation = 1;
    if (!ossl_quic_channel_set_mutator(ch, script_74_alter_version,
                                       script_74_finish_mutation,
                                       NULL))
        return 0;
    return 1;
}

static const struct script_op script_74[] = {
    OP_CHECK                (script_74_arm_packet_mutator, 0)
    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT        ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI     (a, C_BIDI_ID(0))
    OP_C_WRITE               (a, "apple", 5)
    OP_S_BIND_STREAM_ID      (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT         (a, "apple", 5)

    OP_END
};

/* 75. Version negotiation: Unknown version causes connection abort */
static const struct script_op script_75[] = {
    OP_S_SET_INJECT_DATAGRAM (server_gen_version_neg)
    OP_SET_INJECT_WORD       (2, 0)

    OP_C_SET_ALPN            ("ossltest")
    OP_C_CONNECT_WAIT_OR_FAIL()

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_CONNECTION_REFUSED,0,0)

    OP_END
};

/* 76. Test peer-initiated shutdown wait */
static int script_76_check(struct helper *h, struct helper_local *hl)
{
    if (!TEST_false(SSL_shutdown_ex(h->c_conn,
                                    SSL_SHUTDOWN_FLAG_WAIT_PEER
                                    | SSL_SHUTDOWN_FLAG_NO_BLOCK,
                                    NULL, 0)))
        return 0;

    return 1;
}

static const struct script_op script_76[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    /* Check a WAIT_PEER call doesn't succeed yet. */
    OP_CHECK                (script_76_check, 0)
    OP_S_SHUTDOWN           (42)

    OP_C_SHUTDOWN_WAIT      (NULL, SSL_SHUTDOWN_FLAG_WAIT_PEER)
    OP_C_EXPECT_CONN_CLOSE_INFO(42, 1, 1)

    OP_END
};

/* 77. Ensure default stream popping operates correctly */
static const struct script_op script_77[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_INCOMING_STREAM_POLICY(SSL_INCOMING_STREAM_POLICY_ACCEPT)

    OP_S_NEW_STREAM_BIDI    (a, S_BIDI_ID(0))
    OP_S_WRITE              (a, "Strawberry", 10)

    OP_C_READ_EXPECT        (DEFAULT, "Strawberry", 10)

    OP_S_NEW_STREAM_BIDI    (b, S_BIDI_ID(1))
    OP_S_WRITE              (b, "xyz", 3)

    OP_C_ACCEPT_STREAM_WAIT (b)
    OP_C_READ_EXPECT        (b, "xyz", 3)

    OP_END
};

/* 78. Post-connection session ticket handling */
static size_t new_session_count;

static int on_new_session(SSL *s, SSL_SESSION *sess)
{
    ++new_session_count;
    return 0; /* do not ref session, we aren't keeping it */
}

static int setup_session(struct helper *h, struct helper_local *hl)
{
    SSL_CTX_set_session_cache_mode(h->c_ctx, SSL_SESS_CACHE_BOTH);
    SSL_CTX_sess_set_new_cb(h->c_ctx, on_new_session);
    return 1;
}

static int trigger_late_session_ticket(struct helper *h, struct helper_local *hl)
{
    new_session_count = 0;

    if (!TEST_true(ossl_quic_tserver_new_ticket(ACQUIRE_S())))
        return 0;

    return 1;
}

static int check_got_session_ticket(struct helper *h, struct helper_local *hl)
{
    if (!TEST_size_t_gt(new_session_count, 0))
        return 0;

    return 1;
}

static int check_idle_timeout(struct helper *h, struct helper_local *hl);

static const struct script_op script_78[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_CHECK                (setup_session, 0)
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)

    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)

    OP_CHECK                (trigger_late_session_ticket, 0)

    OP_S_WRITE              (a, "Strawberry", 10)
    OP_C_READ_EXPECT        (a, "Strawberry", 10)

    OP_CHECK                (check_got_session_ticket, 0)
    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_NEGOTIATED, 30000)

    OP_END
};

/* 79. Optimised FIN test */
static const struct script_op script_79[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_WRITE_EX2          (DEFAULT, "apple", 5, SSL_WRITE_FLAG_CONCLUDE)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_EXPECT_FIN         (a)
    OP_S_WRITE              (a, "orange", 6)
    OP_S_CONCLUDE           (a)
    OP_C_READ_EXPECT        (DEFAULT, "orange", 6)
    OP_C_EXPECT_FIN         (DEFAULT)
    OP_END
};

/* 80. Stateless reset detection test */
static QUIC_STATELESS_RESET_TOKEN test_reset_token = {
    { 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
     0xde, 0xad, 0xbe, 0xef }};

/*
 * Generate a packet in the following format:
 * https://www.rfc-editor.org/rfc/rfc9000.html#name-stateless-reset
 * Stateless Reset {
 *  Fixed Bits (2): 1
 *  Unpredictable bits (38..)
 *  Stateless reset token (128)
 *  }
 */
static int script_80_send_stateless_reset(struct helper *h, QUIC_PKT_HDR *hdr,
                                          unsigned char *buf, size_t len)
{
    unsigned char databuf[64];

    if (h->inject_word1 == 0)
        return 1;

    h->inject_word1 = 0;

    fprintf(stderr, "Sending stateless reset\n");

    RAND_bytes(databuf, 64);
    databuf[0] = 0x40;
    memcpy(&databuf[48], test_reset_token.token,
           sizeof(test_reset_token.token));

    if (!TEST_int_eq(SSL_inject_net_dgram(h->c_conn, databuf, sizeof(databuf),
                                          NULL, h->s_net_bio_addr), 1))
        return 0;

    return 1;
}

static int script_80_gen_new_conn_id(struct helper *h, QUIC_PKT_HDR *hdr,
                                     unsigned char *buf, size_t len)
{
    int rc = 0;
    size_t l;
    unsigned char frame_buf[64];
    WPACKET wpkt;
    QUIC_CONN_ID new_cid = {0};
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid = {0};
    QUIC_CHANNEL *ch = ossl_quic_tserver_get_channel(ACQUIRE_S_NOHL());

    if (h->inject_word0 == 0)
        return 1;

    h->inject_word0 = 0;

    fprintf(stderr, "sending new conn id\n");
    if (!TEST_true(WPACKET_init_static_len(&wpkt, frame_buf,
                                           sizeof(frame_buf), 0)))
        return 0;

    ossl_quic_channel_get_diag_local_cid(ch, &new_cid);

    ncid.seq_num = 2;
    ncid.retire_prior_to = 2;
    ncid.conn_id = new_cid;
    memcpy(ncid.stateless_reset.token, test_reset_token.token,
           sizeof(test_reset_token.token));

    if (!TEST_true(ossl_quic_wire_encode_frame_new_conn_id(&wpkt, &ncid)))
        goto err;

    if (!TEST_true(WPACKET_get_total_written(&wpkt, &l)))
        goto err;

    if (!qtest_fault_prepend_frame(h->qtf, frame_buf, l))
        goto err;

    rc = 1;
err:
    if (rc)
        WPACKET_finish(&wpkt);
    else
        WPACKET_cleanup(&wpkt);

    return rc;
}

static int script_80_inject_pkt(struct helper *h, QUIC_PKT_HDR *hdr,
                                unsigned char *buf, size_t len)
{
    if (h->inject_word1 == 1)
        return script_80_send_stateless_reset(h, hdr, buf, len);
    else if (h->inject_word0 == 1)
        return script_80_gen_new_conn_id(h, hdr, buf, len);

    return 1;
}

static const struct script_op script_80[] = {
    OP_S_SET_INJECT_PLAIN       (script_80_inject_pkt)
    OP_C_SET_ALPN               ("ossltest")
    OP_C_CONNECT_WAIT           ()
    OP_C_WRITE                  (DEFAULT, "apple", 5)
    OP_C_CONCLUDE               (DEFAULT)
    OP_S_BIND_STREAM_ID         (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT            (a, "apple", 5)
    OP_SET_INJECT_WORD          (1, 0)
    OP_S_WRITE                  (a, "apple", 5)
    OP_C_READ_EXPECT            (DEFAULT, "apple", 5)
    OP_SET_INJECT_WORD          (0, 1)
    OP_S_WRITE                  (a, "apple", 5)
    OP_C_EXPECT_CONN_CLOSE_INFO (0, 0, 1)
    OP_END
};

/* 81. Idle timeout configuration */
static int modify_idle_timeout(struct helper *h, struct helper_local *hl)
{
    uint64_t v = 0;

    /* Test bad value is rejected. */
    if (!TEST_false(SSL_set_feature_request_uint(h->c_conn,
                                                 SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                                 (1ULL << 62))))
        return 0;

    /* Set value. */
    if (!TEST_true(SSL_set_feature_request_uint(h->c_conn,
                                                SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                                hl->check_op->arg2)))
        return 0;

    if (!TEST_true(SSL_get_feature_request_uint(h->c_conn,
                                                SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                                &v)))
        return 0;

    if (!TEST_uint64_t_eq(v, hl->check_op->arg2))
        return 0;

    return 1;
}

static int check_idle_timeout(struct helper *h, struct helper_local *hl)
{
    uint64_t v = 0;

    if (!TEST_true(SSL_get_value_uint(h->c_conn, hl->check_op->arg1,
                                      SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                      &v)))
        return 0;

    if (!TEST_uint64_t_eq(v, hl->check_op->arg2))
        return 0;

    return 1;
}

static const struct script_op script_81[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_CHECK                (modify_idle_timeout, 25000)
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_PEER_REQUEST, 30000)
    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_NEGOTIATED, 25000)

    OP_END
};

/* 82. Negotiated default idle timeout if not configured */
static const struct script_op script_82[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_PEER_REQUEST, 30000)
    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_NEGOTIATED, 30000)

    OP_END
};

/* 83. No late changes to idle timeout */
static int cannot_change_idle_timeout(struct helper *h, struct helper_local *hl)
{
    uint64_t v = 0;

    if (!TEST_true(SSL_get_feature_request_uint(h->c_conn,
                                                SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                                &v)))
        return 0;

    if (!TEST_uint64_t_eq(v, 30000))
        return 0;

    if (!TEST_false(SSL_set_feature_request_uint(h->c_conn,
                                                 SSL_VALUE_QUIC_IDLE_TIMEOUT,
                                                 5000)))
        return 0;

    return 1;
}

static const struct script_op script_83[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_CHECK                (cannot_change_idle_timeout, 0)
    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_PEER_REQUEST, 30000)
    OP_CHECK2               (check_idle_timeout,
                             SSL_VALUE_CLASS_FEATURE_NEGOTIATED, 30000)

    OP_END
};

/* 84. Test query of available streams */
static int check_avail_streams(struct helper *h, struct helper_local *hl)
{
    uint64_t v = 0;

    switch (hl->check_op->arg1) {
    case 0:
        if (!TEST_true(SSL_get_quic_stream_bidi_local_avail(h->c_conn, &v)))
            return 0;
        break;
    case 1:
        if (!TEST_true(SSL_get_quic_stream_bidi_remote_avail(h->c_conn, &v)))
            return 0;
        break;
    case 2:
        if (!TEST_true(SSL_get_quic_stream_uni_local_avail(h->c_conn, &v)))
            return 0;
        break;
    case 3:
        if (!TEST_true(SSL_get_quic_stream_uni_remote_avail(h->c_conn, &v)))
            return 0;
        break;
    default:
        return 0;
    }

    if (!TEST_uint64_t_eq(v, hl->check_op->arg2))
        return 0;

    return 1;
}

static int set_event_handling_mode_conn(struct helper *h, struct helper_local *hl);
static int reenable_test_event_handling(struct helper *h, struct helper_local *hl);

static int check_write_buf_stat(struct helper *h, struct helper_local *hl)
{
    SSL *c_a;
    uint64_t size, used, avail;

    if (!TEST_ptr(c_a = helper_local_get_c_stream(hl, "a")))
        return 0;

    if (!TEST_true(SSL_get_stream_write_buf_size(c_a, &size))
        || !TEST_true(SSL_get_stream_write_buf_used(c_a, &used))
        || !TEST_true(SSL_get_stream_write_buf_avail(c_a, &avail))
        || !TEST_uint64_t_ge(size, avail)
        || !TEST_uint64_t_ge(size, used)
        || !TEST_uint64_t_eq(avail + used, size))
        return 0;

    if (!TEST_uint64_t_eq(used, hl->check_op->arg1))
        return 0;

    return 1;
}

static const struct script_op script_84[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_CHECK2               (check_avail_streams, 0, 100)
    OP_CHECK2               (check_avail_streams, 1, 100)
    OP_CHECK2               (check_avail_streams, 2, 100)
    OP_CHECK2               (check_avail_streams, 3, 100)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))

    OP_CHECK2               (check_avail_streams, 0, 99)
    OP_CHECK2               (check_avail_streams, 1, 100)
    OP_CHECK2               (check_avail_streams, 2, 100)
    OP_CHECK2               (check_avail_streams, 3, 100)

    OP_C_NEW_STREAM_UNI     (b, C_UNI_ID(0))

    OP_CHECK2               (check_avail_streams, 0, 99)
    OP_CHECK2               (check_avail_streams, 1, 100)
    OP_CHECK2               (check_avail_streams, 2, 99)
    OP_CHECK2               (check_avail_streams, 3, 100)

    OP_S_NEW_STREAM_BIDI    (c, S_BIDI_ID(0))
    OP_S_WRITE              (c, "x", 1)

    OP_C_ACCEPT_STREAM_WAIT (c)
    OP_C_READ_EXPECT        (c, "x", 1)

    OP_CHECK2               (check_avail_streams, 0, 99)
    OP_CHECK2               (check_avail_streams, 1, 99)
    OP_CHECK2               (check_avail_streams, 2, 99)
    OP_CHECK2               (check_avail_streams, 3, 100)

    OP_S_NEW_STREAM_UNI     (d, S_UNI_ID(0))
    OP_S_WRITE              (d, "x", 1)

    OP_C_ACCEPT_STREAM_WAIT (d)
    OP_C_READ_EXPECT        (d, "x", 1)

    OP_CHECK2               (check_avail_streams, 0, 99)
    OP_CHECK2               (check_avail_streams, 1, 99)
    OP_CHECK2               (check_avail_streams, 2, 99)
    OP_CHECK2               (check_avail_streams, 3, 99)

    OP_CHECK2               (check_write_buf_stat, 0, 0)
    OP_CHECK                (set_event_handling_mode_conn,
                             SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT)
    OP_C_WRITE              (a, "apple", 5)
    OP_CHECK2               (check_write_buf_stat, 5, 0)

    OP_CHECK                (reenable_test_event_handling, 0)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)

    OP_END
};

/* 85. Test SSL_poll (lite, non-blocking) */
ossl_unused static int script_85_poll(struct helper *h, struct helper_local *hl)
{
    int ok = 1, ret, expected_ret = 1;
    static const struct timeval timeout = {0};
    size_t result_count, expected_result_count = 0;
    SSL_POLL_ITEM items[5] = {0}, *item = items;
    SSL *c_a, *c_b, *c_c, *c_d;
    size_t i;
    uint64_t mode, expected_revents[5] = {0};

    if (!TEST_ptr(c_a = helper_local_get_c_stream(hl, "a"))
        || !TEST_ptr(c_b = helper_local_get_c_stream(hl, "b"))
        || !TEST_ptr(c_c = helper_local_get_c_stream(hl, "c"))
        || !TEST_ptr(c_d = helper_local_get_c_stream(hl, "d")))
        return 0;

    item->desc    = SSL_as_poll_descriptor(c_a);
    item->events  = UINT64_MAX;
    item->revents = UINT64_MAX;
    ++item;

    item->desc    = SSL_as_poll_descriptor(c_b);
    item->events  = UINT64_MAX;
    item->revents = UINT64_MAX;
    ++item;

    item->desc    = SSL_as_poll_descriptor(c_c);
    item->events  = UINT64_MAX;
    item->revents = UINT64_MAX;
    ++item;

    item->desc    = SSL_as_poll_descriptor(c_d);
    item->events  = UINT64_MAX;
    item->revents = UINT64_MAX;
    ++item;

    item->desc    = SSL_as_poll_descriptor(h->c_conn);
    item->events  = UINT64_MAX;
    item->revents = UINT64_MAX;
    ++item;

    result_count = SIZE_MAX;
    ret = SSL_poll(items, OSSL_NELEM(items), sizeof(SSL_POLL_ITEM),
                   &timeout, 0,
                   &result_count);

    mode = hl->check_op->arg2;
    switch (mode) {
    case 0:
        /* No incoming data yet */
        expected_revents[0]     = SSL_POLL_EVENT_W;
        expected_revents[1]     = SSL_POLL_EVENT_W;
        expected_revents[2]     = SSL_POLL_EVENT_W;
        expected_revents[3]     = SSL_POLL_EVENT_W;
        expected_revents[4]     = SSL_POLL_EVENT_OS;
        expected_result_count   = 5;
        break;
    case 1:
        /* Expect more events */
        expected_revents[0]     = SSL_POLL_EVENT_W | SSL_POLL_EVENT_R;
        expected_revents[1]     = SSL_POLL_EVENT_W | SSL_POLL_EVENT_ER;
        expected_revents[2]     = SSL_POLL_EVENT_EW;
        expected_revents[3]     = SSL_POLL_EVENT_W;
        expected_revents[4]     = SSL_POLL_EVENT_OS | SSL_POLL_EVENT_ISB;
        expected_result_count   = 5;
        break;
    default:
        return 0;
    }

    if (!TEST_int_eq(ret, expected_ret)
        || !TEST_size_t_eq(result_count, expected_result_count))
        ok = 0;

    for (i = 0; i < OSSL_NELEM(items); ++i)
        if (!TEST_uint64_t_eq(items[i].revents, expected_revents[i])) {
            TEST_error("mismatch at index %zu in poll results, mode %d",
                       i, (int)mode);
            ok = 0;
        }

    return ok;
}

static const struct script_op script_85[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "flamingo", 8)

    OP_C_NEW_STREAM_BIDI    (b, C_BIDI_ID(1))
    OP_C_WRITE              (b, "orange", 6)

    OP_C_NEW_STREAM_BIDI    (c, C_BIDI_ID(2))
    OP_C_WRITE              (c, "Strawberry", 10)

    OP_C_NEW_STREAM_BIDI    (d, C_BIDI_ID(3))
    OP_C_WRITE              (d, "sync", 4)

    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_BIND_STREAM_ID     (b, C_BIDI_ID(1))
    OP_S_BIND_STREAM_ID     (c, C_BIDI_ID(2))
    OP_S_BIND_STREAM_ID     (d, C_BIDI_ID(3))

    /* Check nothing readable yet. */
    OP_CHECK                (script_85_poll, 0)

    /* Send something that will make client sockets readable. */
    OP_S_READ_EXPECT        (a, "flamingo", 8)
    OP_S_WRITE              (a, "herringbone", 11)

    /* Send something that will make 'b' reset. */
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_SET_INJECT_WORD      (C_BIDI_ID(1) + 1, OSSL_QUIC_FRAME_TYPE_RESET_STREAM)

    /* Ensure sync. */
    OP_S_READ_EXPECT        (d, "sync", 4)
    OP_S_WRITE              (d, "x", 1)
    OP_C_READ_EXPECT        (d, "x", 1)

    /* Send something that will make 'c' reset. */
    OP_S_SET_INJECT_PLAIN   (script_28_inject_plain)
    OP_SET_INJECT_WORD      (C_BIDI_ID(2) + 1, OSSL_QUIC_FRAME_TYPE_STOP_SENDING)

    OP_S_NEW_STREAM_BIDI    (z, S_BIDI_ID(0))
    OP_S_WRITE              (z, "z", 1)

    /* Ensure sync. */
    OP_S_WRITE              (d, "x", 1)
    OP_C_READ_EXPECT        (d, "x", 1)

    /* Check a is now readable. */
    OP_CHECK                (script_85_poll, 1)

    OP_END
};

/* 86. Event Handling Mode Configuration */
static int set_event_handling_mode_conn(struct helper *h, struct helper_local *hl)
{
    hl->explicit_event_handling = 1;
    return SSL_set_event_handling_mode(h->c_conn, hl->check_op->arg2);
}

static int reenable_test_event_handling(struct helper *h, struct helper_local *hl)
{
    hl->explicit_event_handling = 0;
    return 1;
}

static ossl_unused int set_event_handling_mode_stream(struct helper *h, struct helper_local *hl)
{
    SSL *ssl = helper_local_get_c_stream(hl, "a");

    if (!TEST_ptr(ssl))
        return 0;

    return SSL_set_event_handling_mode(ssl, hl->check_op->arg2);
}

static const struct script_op script_86[] = {
    OP_SKIP_IF_BLOCKING     (23)

    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()

    OP_C_SET_DEFAULT_STREAM_MODE(SSL_DEFAULT_STREAM_MODE_NONE)

    /* Turn on explicit handling mode. */
    OP_CHECK                (set_event_handling_mode_conn,
                             SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT)

    /*
     * Create a new stream and write data. This won't get sent
     * to the network net because we are in explicit mode
     * and we haven't called SSL_handle_events().
     */
    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)

    /* Put connection back into implicit handling mode. */
    OP_CHECK                (set_event_handling_mode_conn,
                             SSL_VALUE_EVENT_HANDLING_MODE_IMPLICIT)

    /* Override at stream level. */
    OP_CHECK                (set_event_handling_mode_stream,
                             SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT)
    OP_C_WRITE              (a, "orange", 6)
    OP_C_CONCLUDE           (a)

    /*
     * Confirm the data isn't going to arrive. OP_SLEEP is always undesirable
     * but we have no reasonable way to synchronise on something not arriving
     * given all network traffic is essentially stopped and there are no other
     * signals arriving from the peer which could be used for synchronisation.
     * Slow OSes will pass this anyway (fail-open).
     */
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))

    OP_BEGIN_REPEAT         (20)
    OP_S_READ_FAIL          (a, 1)
    OP_SLEEP                (10)
    OP_END_REPEAT           ()

    /* Now let the data arrive and confirm it arrives. */
    OP_CHECK                (reenable_test_event_handling, 0)
    OP_S_READ_EXPECT        (a, "appleorange", 11)
    OP_S_EXPECT_FIN         (a)

    /* Back into explicit mode. */
    OP_CHECK                (set_event_handling_mode_conn,
                             SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT)
    OP_S_WRITE              (a, "ok", 2)
    OP_C_READ_FAIL          (a)

    /* Works once event handling is done. */
    OP_CHECK                (reenable_test_event_handling, 0)
    OP_C_READ_EXPECT        (a, "ok", 2)

    OP_END
};


/* 87. Test stream reset functionality */
static const struct script_op script_87[] = {
    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT       ()
    OP_C_NEW_STREAM_BIDI    (a, C_BIDI_ID(0))
    OP_C_WRITE              (a, "apple", 5)
    OP_C_CONCLUDE           (a)
    OP_S_BIND_STREAM_ID     (a, C_BIDI_ID(0))
    OP_S_READ_EXPECT        (a, "apple", 5)
    OP_S_EXPECT_FIN         (a)
    OP_S_WRITE              (a, "orange", 6)
    OP_C_READ_EXPECT        (a, "orange", 6)
    OP_S_CONCLUDE           (a)
    OP_C_EXPECT_FIN         (a)
    OP_SLEEP                (1000)
    OP_C_STREAM_RESET_FAIL  (a, 42)
    OP_END
};

static const struct script_op *const scripts[] = {
    script_1,
    script_2,
    script_3,
    script_4,
    script_5,
    script_6,
    script_7,
    script_8,
    script_9,
    script_10,
    script_11,
    script_12,
    script_13,
    script_14,
    script_15,
    script_16,
    script_17,
    script_18,
    script_19,
    script_20,
    script_21,
    script_22,
    script_23,
    script_24,
    script_25,
    script_26,
    script_27,
    script_28,
    script_29,
    script_30,
    script_31,
    script_32,
    script_33,
    script_34,
    script_35,
    script_36,
    script_37,
    script_38,
    script_39,
    script_40,
    script_41,
    script_42,
    script_43,
    script_44,
    script_45,
    script_46,
    script_47,
    script_48,
    script_49,
    script_50,
    script_51,
    script_52,
    script_53,
    script_54,
    script_55,
    script_56,
    script_57,
    script_58,
    script_59,
    script_60,
    script_61,
    script_62,
    script_63,
    script_64,
    script_65,
    script_66,
    script_67,
    script_68,
    script_69,
    script_70,
    script_71,
    script_72,
    script_73,
    script_74,
    script_75,
    script_76,
    script_77,
    script_78,
    script_79,
    script_80,
    script_81,
    script_82,
    script_83,
    script_84,
    script_85,
    script_86,
    script_87
};

static int test_script(int idx)
{
    int script_idx, free_order, blocking;
    char script_name[64];

    free_order = idx % 2;
    idx /= 2;

    blocking = idx % 2;
    idx /= 2;

    script_idx = idx;

    if (blocking && free_order)
        return 1; /* don't need to test free_order twice */

#if !defined(OPENSSL_THREADS)
    if (blocking) {
        TEST_skip("cannot test in blocking mode without threads");
        return 1;
    }
#endif

    BIO_snprintf(script_name, sizeof(script_name), "script %d", script_idx + 1);

    TEST_info("Running script %d (order=%d, blocking=%d)", script_idx + 1,
              free_order, blocking);
    return run_script(scripts[script_idx], script_name, free_order, blocking);
}

/* Dynamically generated tests. */
static struct script_op dyn_frame_types_script[] = {
    OP_S_SET_INJECT_PLAIN   (script_21_inject_plain)
    OP_SET_INJECT_WORD      (0, 0) /* dynamic */

    OP_C_SET_ALPN           ("ossltest")
    OP_C_CONNECT_WAIT_OR_FAIL()

    OP_C_EXPECT_CONN_CLOSE_INFO(OSSL_QUIC_ERR_FRAME_ENCODING_ERROR,0,0)

    OP_END
};

struct forbidden_frame_type {
    uint64_t pkt_type, frame_type, expected_err;
};

static const struct forbidden_frame_type forbidden_frame_types[] = {
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_VLINT_MAX, OSSL_QUIC_ERR_FRAME_ENCODING_ERROR },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_VLINT_MAX, OSSL_QUIC_ERR_FRAME_ENCODING_ERROR },
    { QUIC_PKT_TYPE_1RTT, OSSL_QUIC_VLINT_MAX, OSSL_QUIC_ERR_FRAME_ENCODING_ERROR },

    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_STREAM, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_RESET_STREAM, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_STOP_SENDING, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_NEW_TOKEN, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_MAX_DATA, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_INITIAL, OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },

    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_STREAM, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_RESET_STREAM, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_STOP_SENDING, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_NEW_TOKEN, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_MAX_DATA, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
    { QUIC_PKT_TYPE_HANDSHAKE, OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },

    /* Client uses a zero-length CID so this is not allowed. */
    { QUIC_PKT_TYPE_1RTT, OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID, OSSL_QUIC_ERR_PROTOCOL_VIOLATION },
};

static ossl_unused int test_dyn_frame_types(int idx)
{
    size_t i;
    char script_name[64];
    struct script_op *s = dyn_frame_types_script;

    for (i = 0; i < OSSL_NELEM(dyn_frame_types_script); ++i)
        if (s[i].op == OPK_SET_INJECT_WORD) {
            s[i].arg1 = (size_t)forbidden_frame_types[idx].pkt_type;
            s[i].arg2 = forbidden_frame_types[idx].frame_type;
        } else if (s[i].op == OPK_C_EXPECT_CONN_CLOSE_INFO) {
            s[i].arg2 = forbidden_frame_types[idx].expected_err;
        }

    BIO_snprintf(script_name, sizeof(script_name),
                 "dyn script %d", idx);

    return run_script(dyn_frame_types_script, script_name, 0, 0);
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile\n")

int setup_tests(void)
{
#if defined (_PUT_MODEL_)
    return TEST_skip("QUIC is not supported by this build");
#endif

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(certfile = test_get_argument(0))
        || !TEST_ptr(keyfile = test_get_argument(1)))
        return 0;

    ADD_ALL_TESTS(test_dyn_frame_types, OSSL_NELEM(forbidden_frame_types));
    ADD_ALL_TESTS(test_script, OSSL_NELEM(scripts) * 2 * 2);
    return 1;
}
