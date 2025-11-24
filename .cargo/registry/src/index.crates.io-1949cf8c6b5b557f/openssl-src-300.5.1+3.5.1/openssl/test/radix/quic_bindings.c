/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/lhash.h>
#include <assert.h>

#include "internal/quic_engine.h"
#include "internal/quic_channel.h"
#include "internal/quic_ssl.h"
#include "internal/quic_error.h"

/*
 * RADIX 6D QUIC Test Framework
 * =============================================================================
 *
 * The radix test framework is a six-dimension script-driven facility to support
 * execution of
 *
 *   multi-stream
 *   multi-client
 *   multi-server
 *   multi-thread
 *   multi-process
 *   multi-node
 *
 * test vignettes for QUIC. Unlike the older multistream test framework, it does
 * not assume a single client and a single server. Examples of vignettes
 * designed to be supported by the radix test framework in future include:
 *
 *      single client    <-> single server
 *      multiple clients <-> single server
 *      single client    <-> multiple servers
 *      multiple clients <-> multiple servers
 *
 * 'Multi-process' and 'multi-node' means there has been some consideration
 * given to support of multi-process and multi-node testing in the future,
 * though this is not currently supported.
 */

/*
 * An object is something associated with a name in the process-level state. The
 * process-level state primarily revolves around a global dictionary of SSL
 * objects.
 */
typedef struct radix_obj_st {
    char                *name;  /* owned, zero-terminated */
    SSL                 *ssl;   /* owns one reference */
    unsigned int        registered      : 1; /* in LHASH? */
    unsigned int        active          : 1; /* tick? */
} RADIX_OBJ;

DEFINE_LHASH_OF_EX(RADIX_OBJ);

/* Process-level state (i.e. "globals" in the normal sense of the word) */
typedef struct radix_process_st {
    size_t                  node_idx;
    size_t                  process_idx;
    size_t                  next_thread_idx;
    STACK_OF(RADIX_THREAD)  *threads;

    /* Process-global state. */
    CRYPTO_MUTEX            *gm;            /* global mutex */
    LHASH_OF(RADIX_OBJ)     *objs;          /* protected by gm */
    OSSL_TIME               time_slip;      /* protected by gm */
    BIO                     *keylog_out;    /* protected by gm */

    int                     done_join_all_threads;

    /*
     * Valid if done_join_all threads. Logical AND of all child worker results.
     */
    int                     thread_composite_testresult;
} RADIX_PROCESS;

#define NUM_SLOTS       8

/* Thread-level state within a process */
typedef struct radix_thread_st {
    RADIX_PROCESS       *rp;
    CRYPTO_THREAD       *t;
    unsigned char       *tmp_buf;
    size_t              tmp_buf_offset;
    size_t              thread_idx; /* 0=main thread */
    RADIX_OBJ           *slot[NUM_SLOTS];
    SSL                 *ssl[NUM_SLOTS];

    /* child thread spawn arguments */
    SCRIPT_INFO         *child_script_info;
    BIO                 *debug_bio;

    /* m protects all of the below values */
    CRYPTO_MUTEX        *m;
    int                 done;
    int                 testresult; /* valid if done */

    uint64_t            scratch0;
} RADIX_THREAD;

DEFINE_STACK_OF(RADIX_THREAD)

/* ssl reference is transferred. name is copied and is required. */
static RADIX_OBJ *RADIX_OBJ_new(const char *name, SSL *ssl)
{
    RADIX_OBJ *obj;

    if (!TEST_ptr(name) || !TEST_ptr(ssl))
        return NULL;

    if (!TEST_ptr(obj = OPENSSL_zalloc(sizeof(*obj))))
       return NULL;

    if (!TEST_ptr(obj->name = OPENSSL_strdup(name))) {
        OPENSSL_free(obj);
        return NULL;
    }

    obj->ssl  = ssl;
    return obj;
}

static void RADIX_OBJ_free(RADIX_OBJ *obj)
{
    if (obj == NULL)
        return;

    assert(!obj->registered);

    SSL_free(obj->ssl);
    OPENSSL_free(obj->name);
    OPENSSL_free(obj);
}

static unsigned long RADIX_OBJ_hash(const RADIX_OBJ *obj)
{
    return OPENSSL_LH_strhash(obj->name);
}

static int RADIX_OBJ_cmp(const RADIX_OBJ *a, const RADIX_OBJ *b)
{
    return strcmp(a->name, b->name);
}

static int RADIX_PROCESS_init(RADIX_PROCESS *rp, size_t node_idx, size_t process_idx)
{
    const char *keylog_path;

#if defined(OPENSSL_THREADS)
    if (!TEST_ptr(rp->gm = ossl_crypto_mutex_new()))
        goto err;
#endif

    if (!TEST_ptr(rp->objs = lh_RADIX_OBJ_new(RADIX_OBJ_hash, RADIX_OBJ_cmp)))
        goto err;

    if (!TEST_ptr(rp->threads = sk_RADIX_THREAD_new(NULL)))
        goto err;

    rp->keylog_out = NULL;
    keylog_path = ossl_safe_getenv("SSLKEYLOGFILE");
    if (keylog_path != NULL && *keylog_path != '\0'
        && !TEST_ptr(rp->keylog_out = BIO_new_file(keylog_path, "a")))
        goto err;

    rp->node_idx                = node_idx;
    rp->process_idx             = process_idx;
    rp->done_join_all_threads   = 0;
    rp->next_thread_idx         = 0;
    return 1;

err:
    lh_RADIX_OBJ_free(rp->objs);
    rp->objs = NULL;
    ossl_crypto_mutex_free(&rp->gm);
    return 0;
}

static const char *stream_state_to_str(int state)
{
    switch (state) {
    case SSL_STREAM_STATE_NONE:
        return "none";
    case SSL_STREAM_STATE_OK:
        return "OK";
    case SSL_STREAM_STATE_WRONG_DIR:
        return "wrong-dir";
    case SSL_STREAM_STATE_FINISHED:
        return "finished";
    case SSL_STREAM_STATE_RESET_LOCAL:
        return "reset-local";
    case SSL_STREAM_STATE_RESET_REMOTE:
        return "reset-remote";
    case SSL_STREAM_STATE_CONN_CLOSED:
        return "conn-closed";
    default:
        return "?";
    }
}

static void report_ssl_state(BIO *bio, const char *pfx, int is_write,
                             int state, uint64_t ec)
{
    const char *state_s = stream_state_to_str(state);

    BIO_printf(bio, "%s%-15s%s(%d)", pfx, is_write ? "Write state: " : "Read state: ",
        state_s, state);
    if (ec != UINT64_MAX)
        BIO_printf(bio, ", %llu", (unsigned long long)ec);
    BIO_printf(bio, "\n");
}

static void report_ssl(SSL *ssl, BIO *bio, const char *pfx)
{
    const char *type = "SSL";
    int is_quic = SSL_is_quic(ssl), is_conn = 0, is_listener = 0;
    SSL_CONN_CLOSE_INFO cc_info = {0};
    const char *e_str, *f_str;

    if (is_quic) {
        is_conn = SSL_is_connection(ssl);
        is_listener = SSL_is_listener(ssl);

        if (is_listener)
            type = "QLSO";
        else if (is_conn)
            type = "QCSO";
        else
            type = "QSSO";
    }

    BIO_printf(bio, "%sType:          %s\n", pfx, type);

    if (is_quic && is_conn
        && SSL_get_conn_close_info(ssl, &cc_info, sizeof(cc_info))) {

        e_str = ossl_quic_err_to_string(cc_info.error_code);
        f_str = ossl_quic_frame_type_to_string(cc_info.frame_type);

        if (e_str == NULL)
            e_str = "?";
        if (f_str == NULL)
            f_str = "?";

        BIO_printf(bio, "%sConnection is closed: %s(%llu)/%s(%llu), "
                   "%s, %s, reason: \"%s\"\n",
                   pfx,
                   e_str,
                   (unsigned long long)cc_info.error_code,
                   f_str,
                   (unsigned long long)cc_info.frame_type,
                   (cc_info.flags & SSL_CONN_CLOSE_FLAG_LOCAL) != 0
                     ? "local" : "remote",
                   (cc_info.flags & SSL_CONN_CLOSE_FLAG_TRANSPORT) != 0
                     ? "transport" : "app",
                   cc_info.reason != NULL ? cc_info.reason : "-");
    }

    if (is_quic && !is_listener) {
        uint64_t stream_id = SSL_get_stream_id(ssl), rec, wec;
        int rstate, wstate;

        if (stream_id != UINT64_MAX)
            BIO_printf(bio, "%sStream ID: %llu\n", pfx,
                       (unsigned long long)stream_id);

        rstate = SSL_get_stream_read_state(ssl);
        wstate = SSL_get_stream_write_state(ssl);

        if (SSL_get_stream_read_error_code(ssl, &rec) != 1)
            rec = UINT64_MAX;

        if (SSL_get_stream_write_error_code(ssl, &wec) != 1)
            wec = UINT64_MAX;

        report_ssl_state(bio, pfx, 0, rstate, rec);
        report_ssl_state(bio, pfx, 1, wstate, wec);
    }
}

static void report_obj(RADIX_OBJ *obj, void *arg)
{
    BIO *bio = arg;
    SSL *ssl = obj->ssl;

    BIO_printf(bio, "      - %-16s @ %p\n", obj->name, (void *)obj->ssl);
    ERR_set_mark();
    report_ssl(ssl, bio, "          ");
    ERR_pop_to_mark();
}

static void RADIX_THREAD_report_state(RADIX_THREAD *rt, BIO *bio)
{
    size_t i;

    BIO_printf(bio, "  Slots:\n");
    for (i = 0; i < NUM_SLOTS; ++i)
        if (rt->slot[i] == NULL)
            BIO_printf(bio, "  %3zu) <NULL>\n", i);
        else
            BIO_printf(bio, "  %3zu) '%s' (SSL: %p)\n", i,
                       rt->slot[i]->name,
                       (void *)rt->ssl[i]);
}

static void RADIX_PROCESS_report_state(RADIX_PROCESS *rp, BIO *bio,
                                       int verbose)
{
    BIO_printf(bio, "Final process state for node %zu, process %zu:\n",
               rp->node_idx, rp->process_idx);

    BIO_printf(bio, "  Threads (incl. main):        %zu\n",
               rp->next_thread_idx);
    BIO_printf(bio, "  Time slip:                   %llu ms\n",
               (unsigned long long)ossl_time2ms(rp->time_slip));

    BIO_printf(bio, "  Objects:\n");
    lh_RADIX_OBJ_doall_arg(rp->objs, report_obj, bio);

    if (verbose)
        RADIX_THREAD_report_state(sk_RADIX_THREAD_value(rp->threads, 0),
                                  bio_err);

    BIO_printf(bio, "\n==========================================="
               "===========================\n");
}

static void RADIX_PROCESS_report_thread_results(RADIX_PROCESS *rp, BIO *bio)
{
    size_t i;
    RADIX_THREAD *rt;
    char *p;
    long l;
    char pfx_buf[64];
    int rt_testresult;

    for (i = 1; i < (size_t)sk_RADIX_THREAD_num(rp->threads); ++i) {
        rt = sk_RADIX_THREAD_value(rp->threads, i);

        ossl_crypto_mutex_lock(rt->m);
        assert(rt->done);
        rt_testresult = rt->testresult;
        ossl_crypto_mutex_unlock(rt->m);

        BIO_printf(bio, "\n====(n%zu/p%zu/t%zu)============================"
                   "===========================\n"
                   "Result for child thread with index %zu:\n",
                   rp->node_idx, rp->process_idx, rt->thread_idx, rt->thread_idx);

        BIO_snprintf(pfx_buf, sizeof(pfx_buf), "#  -T-%2zu:\t# ", rt->thread_idx);
        BIO_set_prefix(bio_err, pfx_buf);

        l = BIO_get_mem_data(rt->debug_bio, &p);
        BIO_write(bio, p, l);
        BIO_printf(bio, "\n");
        BIO_set_prefix(bio_err, "# ");
        BIO_printf(bio, "==> Child thread with index %zu exited with %d\n",
                   rt->thread_idx, rt_testresult);
        if (!rt_testresult)
            RADIX_THREAD_report_state(rt, bio);
    }

    BIO_printf(bio, "\n==========================================="
               "===========================\n");
}

static int RADIX_THREAD_join(RADIX_THREAD *rt);

static int RADIX_PROCESS_join_all_threads(RADIX_PROCESS *rp, int *testresult)
{
    int ok = 1;
    size_t i;
    RADIX_THREAD *rt;
    int composite_testresult = 1;

    if (rp->done_join_all_threads) {
        *testresult = rp->thread_composite_testresult;
        return 1;
    }

    for (i = 1; i < (size_t)sk_RADIX_THREAD_num(rp->threads); ++i) {
        rt = sk_RADIX_THREAD_value(rp->threads, i);

        BIO_printf(bio_err, "==> Joining thread %zu\n", i);

        if (!TEST_true(RADIX_THREAD_join(rt)))
            ok = 0;

        if (!rt->testresult)
            composite_testresult = 0;
    }

    rp->thread_composite_testresult = composite_testresult;
    *testresult                     = composite_testresult;
    rp->done_join_all_threads       = 1;

    RADIX_PROCESS_report_thread_results(rp, bio_err);
    return ok;
}

static void cleanup_one(RADIX_OBJ *obj)
{
    obj->registered = 0;
    RADIX_OBJ_free(obj);
}

static void RADIX_THREAD_free(RADIX_THREAD *rt);

static void RADIX_PROCESS_cleanup(RADIX_PROCESS *rp)
{
    size_t i;

    assert(rp->done_join_all_threads);

    for (i = 0; i < (size_t)sk_RADIX_THREAD_num(rp->threads); ++i)
        RADIX_THREAD_free(sk_RADIX_THREAD_value(rp->threads, i));

    sk_RADIX_THREAD_free(rp->threads);
    rp->threads = NULL;

    lh_RADIX_OBJ_doall(rp->objs, cleanup_one);
    lh_RADIX_OBJ_free(rp->objs);
    rp->objs = NULL;

    BIO_free_all(rp->keylog_out);
    rp->keylog_out = NULL;
    ossl_crypto_mutex_free(&rp->gm);
}

static RADIX_OBJ *RADIX_PROCESS_get_obj(RADIX_PROCESS *rp, const char *name)
{
    RADIX_OBJ key;

    key.name = (char *)name;
    return lh_RADIX_OBJ_retrieve(rp->objs, &key);
}

static int RADIX_PROCESS_set_obj(RADIX_PROCESS *rp,
                                 const char *name, RADIX_OBJ *obj)
{
    RADIX_OBJ *existing;

    if (obj != NULL && !TEST_false(obj->registered))
        return 0;

    existing = RADIX_PROCESS_get_obj(rp, name);
    if (existing != NULL && obj != existing) {
        if (!TEST_true(existing->registered))
            return 0;

        lh_RADIX_OBJ_delete(rp->objs, existing);
        existing->registered = 0;
        RADIX_OBJ_free(existing);
    }

    if (obj != NULL) {
        lh_RADIX_OBJ_insert(rp->objs, obj);
        obj->registered = 1;
    }

    return 1;
}

static int RADIX_PROCESS_set_ssl(RADIX_PROCESS *rp, const char *name, SSL *ssl)
{
    RADIX_OBJ *obj;

    if (!TEST_ptr(obj = RADIX_OBJ_new(name, ssl)))
        return 0;

    if (!TEST_true(RADIX_PROCESS_set_obj(rp, name, obj))) {
        RADIX_OBJ_free(obj);
        return 0;
    }

    return 1;
}

static SSL *RADIX_PROCESS_get_ssl(RADIX_PROCESS *rp, const char *name)
{
    RADIX_OBJ *obj = RADIX_PROCESS_get_obj(rp, name);

    if (obj == NULL)
        return NULL;

    return obj->ssl;
}

static RADIX_THREAD *RADIX_THREAD_new(RADIX_PROCESS *rp)
{
    RADIX_THREAD *rt;

    if (!TEST_ptr(rp)
        || !TEST_ptr(rt = OPENSSL_zalloc(sizeof(*rt))))
        return 0;

    rt->rp          = rp;

#if defined(OPENSSL_THREADS)
    if (!TEST_ptr(rt->m = ossl_crypto_mutex_new())) {
        OPENSSL_free(rt);
        return 0;
    }
#endif

    if (!TEST_true(sk_RADIX_THREAD_push(rp->threads, rt))) {
        OPENSSL_free(rt);
        return 0;
    }

    rt->thread_idx  = rp->next_thread_idx++;
    assert(rt->thread_idx + 1 == (size_t)sk_RADIX_THREAD_num(rp->threads));
    return rt;
}

static void RADIX_THREAD_free(RADIX_THREAD *rt)
{
    if (rt == NULL)
        return;

    assert(rt->t == NULL);
    BIO_free_all(rt->debug_bio);
    OPENSSL_free(rt->tmp_buf);
    ossl_crypto_mutex_free(&rt->m);
    OPENSSL_free(rt);
}

static int RADIX_THREAD_join(RADIX_THREAD *rt)
{
    CRYPTO_THREAD_RETVAL rv;

    if (rt->t != NULL)
        ossl_crypto_thread_native_join(rt->t, &rv);

    ossl_crypto_thread_native_clean(rt->t);
    rt->t = NULL;

    if (!TEST_true(rt->done))
        return 0;

    return 1;
}

static RADIX_PROCESS        radix_process;
static CRYPTO_THREAD_LOCAL  radix_thread;

static void radix_thread_cleanup_tl(void *p)
{
    /* Should already have been cleaned up. */
    if (!TEST_ptr_null(p))
        abort();
}

static RADIX_THREAD *radix_get_thread(void)
{
    return CRYPTO_THREAD_get_local(&radix_thread);
}

static int radix_thread_init(RADIX_THREAD *rt)
{
    if (!TEST_ptr(rt)
        || !TEST_ptr_null(CRYPTO_THREAD_get_local(&radix_thread)))
        return 0;

    if (!TEST_true(CRYPTO_THREAD_set_local(&radix_thread, rt)))
        return 0;

    set_override_bio_out(rt->debug_bio);
    set_override_bio_err(rt->debug_bio);
    return 1;
}

static void radix_thread_cleanup(void)
{
    RADIX_THREAD *rt = radix_get_thread();

    if (!TEST_ptr(rt))
        return;

    if (!TEST_true(CRYPTO_THREAD_set_local(&radix_thread, NULL)))
        return;
}

static int bindings_process_init(size_t node_idx, size_t process_idx)
{
    RADIX_THREAD *rt;

    if (!TEST_true(RADIX_PROCESS_init(&radix_process, node_idx, process_idx)))
        return 0;

    if (!TEST_true(CRYPTO_THREAD_init_local(&radix_thread,
                                            radix_thread_cleanup_tl)))
        return 0;

    if (!TEST_ptr(rt = RADIX_THREAD_new(&radix_process)))
        return 0;

    /* Allocate structures for main thread. */
    return radix_thread_init(rt);
}

static int bindings_process_finish(int testresult_main)
{
    int testresult, testresult_child;

    if (!TEST_true(RADIX_PROCESS_join_all_threads(&radix_process,
                                                  &testresult_child)))
        return 0;

    testresult = testresult_main && testresult_child;
    RADIX_PROCESS_report_state(&radix_process, bio_err,
                               /*verbose=*/!testresult);
    radix_thread_cleanup(); /* cleanup main thread */
    RADIX_PROCESS_cleanup(&radix_process);

    if (testresult)
        BIO_printf(bio_err, "==> OK\n\n");
    else
        BIO_printf(bio_err, "==> ERROR (main=%d, children=%d)\n\n",
                   testresult_main, testresult_child);

    return testresult;
}

#define RP()    (&radix_process)
#define RT()    (radix_get_thread())

static OSSL_TIME get_time(void *arg)
{
    OSSL_TIME time_slip;

    ossl_crypto_mutex_lock(RP()->gm);
    time_slip = RP()->time_slip;
    ossl_crypto_mutex_unlock(RP()->gm);

    return ossl_time_add(ossl_time_now(), time_slip);
}

ossl_unused static void radix_skip_time(OSSL_TIME t)
{
    ossl_crypto_mutex_lock(RP()->gm);
    RP()->time_slip = ossl_time_add(RP()->time_slip, t);
    ossl_crypto_mutex_unlock(RP()->gm);
}

static void per_op_tick_obj(RADIX_OBJ *obj)
{
    if (obj->active)
        SSL_handle_events(obj->ssl);
}

static int do_per_op(TERP *terp, void *arg)
{
    lh_RADIX_OBJ_doall(RP()->objs, per_op_tick_obj);
    return 1;
}

static int bindings_adjust_terp_config(TERP_CONFIG *cfg)
{
    cfg->now_cb     = get_time;
    cfg->per_op_cb  = do_per_op;
    return 1;
}

static int expect_slot_ssl(FUNC_CTX *fctx, size_t idx, SSL **p_ssl)
{
    if (!TEST_size_t_lt(idx, NUM_SLOTS)
        || !TEST_ptr(*p_ssl = RT()->ssl[idx]))
        return 0;

    return 1;
}

#define REQUIRE_SSL_N(idx, ssl)                                 \
    do {                                                        \
        (ssl) = NULL; /* quiet uninitialized warnings */        \
        if (!TEST_true(expect_slot_ssl(fctx, (idx), &(ssl))))   \
            goto err;                                           \
    } while (0)
#define REQUIRE_SSL(ssl)    REQUIRE_SSL_N(0, (ssl))

#define REQUIRE_SSL_2(a, b)                                     \
    do {                                                        \
        REQUIRE_SSL_N(0, (a));                                  \
        REQUIRE_SSL_N(1, (b));                                  \
    } while (0)

#define REQUIRE_SSL_3(a, b, c)                                  \
    do {                                                        \
        REQUIRE_SSL_N(0, (a));                                  \
        REQUIRE_SSL_N(1, (b));                                  \
        REQUIRE_SSL_N(2, (c));                                  \
    } while (0)

#define REQUIRE_SSL_4(a, b, c, d)                               \
    do {                                                        \
        REQUIRE_SSL_N(0, (a));                                  \
        REQUIRE_SSL_N(1, (b));                                  \
        REQUIRE_SSL_N(2, (c));                                  \
        REQUIRE_SSL_N(3, (d));                                  \
    } while (0)

#define REQUIRE_SSL_5(a, b, c, d, e)                            \
    do {                                                        \
        REQUIRE_SSL_N(0, (a));                                  \
        REQUIRE_SSL_N(1, (b));                                  \
        REQUIRE_SSL_N(2, (c));                                  \
        REQUIRE_SSL_N(3, (d));                                  \
        REQUIRE_SSL_N(4, (e));                                  \
    } while (0)

#define C_BIDI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_CLIENT | QUIC_STREAM_DIR_BIDI)
#define S_BIDI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_SERVER | QUIC_STREAM_DIR_BIDI)
#define C_UNI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_CLIENT | QUIC_STREAM_DIR_UNI)
#define S_UNI_ID(ordinal) \
    (((ordinal) << 2) | QUIC_STREAM_INITIATOR_SERVER | QUIC_STREAM_DIR_UNI)

#if defined(OPENSSL_THREADS)

static int RADIX_THREAD_worker_run(RADIX_THREAD *rt)
{
    int ok = 0;
    TERP_CONFIG cfg = {0};

    cfg.debug_bio = rt->debug_bio;
    if (!TEST_true(bindings_adjust_terp_config(&cfg)))
        goto err;

    if (!TERP_run(rt->child_script_info, &cfg))
        goto err;

    ok = 1;
err:
    return ok;
}

static unsigned int RADIX_THREAD_worker_main(void *p)
{
    int testresult = 0;
    RADIX_THREAD *rt = p;

    if (!TEST_true(radix_thread_init(rt)))
        return 0;

    /* Wait until thread-specific init is done (e.g. setting rt->t) */
    ossl_crypto_mutex_lock(rt->m);
    ossl_crypto_mutex_unlock(rt->m);

    testresult = RADIX_THREAD_worker_run(rt);

    ossl_crypto_mutex_lock(rt->m);
    rt->testresult  = testresult;
    rt->done        = 1;
    ossl_crypto_mutex_unlock(rt->m);

    radix_thread_cleanup();
    return 1;
}

#endif

static void radix_activate_obj(RADIX_OBJ *obj)
{
    if (obj != NULL)
        obj->active = 1;
}

static void radix_activate_slot(size_t idx)
{
    if (idx >= NUM_SLOTS)
        return;

    radix_activate_obj(RT()->slot[idx]);
}

DEF_FUNC(hf_spawn_thread)
{
    int ok = 0;
    RADIX_THREAD *child_rt = NULL;
    SCRIPT_INFO *script_info = NULL;

    F_POP(script_info);
    if (!TEST_ptr(script_info))
        goto err;

#if !defined(OPENSSL_THREADS)
    TEST_skip("threading not supported, skipping");
    F_SKIP_REST();
#else
    if (!TEST_ptr(child_rt = RADIX_THREAD_new(&radix_process)))
        return 0;

    if (!TEST_ptr(child_rt->debug_bio = BIO_new(BIO_s_mem())))
        goto err;

    ossl_crypto_mutex_lock(child_rt->m);

    child_rt->child_script_info = script_info;
    if (!TEST_ptr(child_rt->t = ossl_crypto_thread_native_start(RADIX_THREAD_worker_main,
                                                                child_rt, 1))) {
        ossl_crypto_mutex_unlock(child_rt->m);
        goto err;
    }

    ossl_crypto_mutex_unlock(child_rt->m);
    ok = 1;
#endif
err:
    if (!ok)
        RADIX_THREAD_free(child_rt);

    return ok;
}

DEF_FUNC(hf_clear)
{
    RADIX_THREAD *rt = RT();
    size_t i;

    ossl_crypto_mutex_lock(RP()->gm);

    lh_RADIX_OBJ_doall(RP()->objs, cleanup_one);
    lh_RADIX_OBJ_flush(RP()->objs);

    for (i = 0; i < NUM_SLOTS; ++i) {
        rt->slot[i] = NULL;
        rt->ssl[i]  = NULL;
    }

    ossl_crypto_mutex_unlock(RP()->gm);
    return 1;
}

#define OP_SPAWN_THREAD(script_name)                            \
    (OP_PUSH_P(SCRIPT(script_name)), OP_FUNC(hf_spawn_thread))
#define OP_CLEAR()                                              \
    (OP_FUNC(hf_clear))
