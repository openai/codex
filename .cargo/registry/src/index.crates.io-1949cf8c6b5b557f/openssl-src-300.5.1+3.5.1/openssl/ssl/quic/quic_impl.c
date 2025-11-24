/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/macros.h>
#include <openssl/objects.h>
#include <openssl/sslerr.h>
#include <crypto/rand.h>
#include "quic_local.h"
#include "internal/hashfunc.h"
#include "internal/ssl_unwrap.h"
#include "internal/quic_tls.h"
#include "internal/quic_rx_depack.h"
#include "internal/quic_error.h"
#include "internal/quic_engine.h"
#include "internal/quic_port.h"
#include "internal/quic_reactor_wait_ctx.h"
#include "internal/time.h"

typedef struct qctx_st QCTX;

static void qc_cleanup(QUIC_CONNECTION *qc, int have_lock);
static void aon_write_finish(QUIC_XSO *xso);
static int create_channel(QUIC_CONNECTION *qc, SSL_CTX *ctx);
static QUIC_XSO *create_xso_from_stream(QUIC_CONNECTION *qc, QUIC_STREAM *qs);
static QUIC_CONNECTION *create_qc_from_incoming_conn(QUIC_LISTENER *ql, QUIC_CHANNEL *ch);
static int qc_try_create_default_xso_for_write(QCTX *ctx);
static int qc_wait_for_default_xso_for_read(QCTX *ctx, int peek);
static void qctx_lock(QCTX *qctx);
static void qctx_unlock(QCTX *qctx);
static void qctx_lock_for_io(QCTX *ctx);
static int quic_do_handshake(QCTX *ctx);
static void qc_update_reject_policy(QUIC_CONNECTION *qc);
static void qc_touch_default_xso(QUIC_CONNECTION *qc);
static void qc_set_default_xso(QUIC_CONNECTION *qc, QUIC_XSO *xso, int touch);
static void qc_set_default_xso_keep_ref(QUIC_CONNECTION *qc, QUIC_XSO *xso,
                                        int touch, QUIC_XSO **old_xso);
static SSL *quic_conn_stream_new(QCTX *ctx, uint64_t flags, int need_lock);
static int quic_validate_for_write(QUIC_XSO *xso, int *err);
static int quic_mutation_allowed(QUIC_CONNECTION *qc, int req_active);
static void qctx_maybe_autotick(QCTX *ctx);
static int qctx_should_autotick(QCTX *ctx);

/*
 * QCTX is a utility structure which provides information we commonly wish to
 * unwrap upon an API call being dispatched to us, namely:
 *
 *   - a pointer to the QUIC_CONNECTION (regardless of whether a QCSO or QSSO
 *     was passed);
 *   - a pointer to any applicable QUIC_XSO (e.g. if a QSSO was passed, or if
 *     a QCSO with a default stream was passed);
 *   - whether a QSSO was passed (xso == NULL must not be used to determine this
 *     because it may be non-NULL when a QCSO is passed if that QCSO has a
 *     default stream);
 *   - a pointer to a QUIC_LISTENER object, if one is relevant;
 *   - whether we are in "I/O context", meaning that non-normal errors can
 *     be reported via SSL_get_error() as well as via ERR. Functions such as
 *     SSL_read(), SSL_write() and SSL_do_handshake() are "I/O context"
 *     functions which are allowed to change the value returned by
 *     SSL_get_error. However, other functions (including functions which call
 *     SSL_do_handshake() implicitly) are not allowed to change the return value
 *     of SSL_get_error.
 */
struct qctx_st {
    QUIC_OBJ        *obj;
    QUIC_DOMAIN     *qd;
    QUIC_LISTENER   *ql;
    QUIC_CONNECTION *qc;
    QUIC_XSO        *xso;
    int             is_stream, is_listener, is_domain, in_io;
};

QUIC_NEEDS_LOCK
static void quic_set_last_error(QCTX *ctx, int last_error)
{
    if (!ctx->in_io)
        return;

    if (ctx->is_stream && ctx->xso != NULL)
        ctx->xso->last_error = last_error;
    else if (!ctx->is_stream && ctx->qc != NULL)
        ctx->qc->last_error = last_error;
}

/*
 * Raise a 'normal' error, meaning one that can be reported via SSL_get_error()
 * rather than via ERR. Note that normal errors must always be raised while
 * holding a lock.
 */
QUIC_NEEDS_LOCK
static int quic_raise_normal_error(QCTX *ctx,
                                   int err)
{
    assert(ctx->in_io);
    quic_set_last_error(ctx, err);

    return 0;
}

/*
 * Raise a 'non-normal' error, meaning any error that is not reported via
 * SSL_get_error() and must be reported via ERR.
 *
 * qc should be provided if available. In exceptional circumstances when qc is
 * not known NULL may be passed. This should generally only happen when an
 * expect_...() function defined below fails, which generally indicates a
 * dispatch error or caller error.
 *
 * ctx should be NULL if the connection lock is not held.
 */
static int quic_raise_non_normal_error(QCTX *ctx,
                                       const char *file,
                                       int line,
                                       const char *func,
                                       int reason,
                                       const char *fmt,
                                       ...)
{
    va_list args;

    if (ctx != NULL) {
        quic_set_last_error(ctx, SSL_ERROR_SSL);

        if (reason == SSL_R_PROTOCOL_IS_SHUTDOWN && ctx->qc != NULL)
            ossl_quic_channel_restore_err_state(ctx->qc->ch);
    }

    ERR_new();
    ERR_set_debug(file, line, func);

    va_start(args, fmt);
    ERR_vset_error(ERR_LIB_SSL, reason, fmt, args);
    va_end(args);

    return 0;
}

#define QUIC_RAISE_NORMAL_ERROR(ctx, err)                       \
    quic_raise_normal_error((ctx), (err))

#define QUIC_RAISE_NON_NORMAL_ERROR(ctx, reason, msg)           \
    quic_raise_non_normal_error((ctx),                          \
                                OPENSSL_FILE, OPENSSL_LINE,     \
                                OPENSSL_FUNC,                   \
                                (reason),                       \
                                (msg))
/*
 * Flags for expect_quic_as:
 *
 *   QCTX_C
 *      The input SSL object may be a QCSO.
 *
 *   QCTX_S
 *      The input SSL object may be a QSSO or a QCSO with a default stream
 *      attached.
 *
 *      (Note this means there is no current way to require an SSL object with a
 *      QUIC stream which is not a QCSO; a QCSO with a default stream attached
 *      is always considered to satisfy QCTX_S.)
 *
 *   QCTX_AUTO_S
 *      The input SSL object may be a QSSO or a QCSO with a default stream
 *      attached. If no default stream is currently attached to a QCSO,
 *      one may be auto-created if possible.
 *
 *      If QCTX_REMOTE_INIT is set, an auto-created default XSO is
 *      initiated by the remote party (i.e., local party reads first).
 *
 *      If it is not set, an auto-created default XSO is
 *      initiated by the local party (i.e., local party writes first).
 *
 *   QCTX_L
 *      The input SSL object may be a QLSO.
 *
 *   QCTX_LOCK
 *      If and only if the function returns successfully, the ctx
 *      is guaranteed to be locked.
 *
 *   QCTX_IO
 *      Begin an I/O context. If not set, begins a non-I/O context.
 *      This determines whether SSL_get_error() is updated; the value it returns
 *      is modified only by an I/O call.
 *
 *   QCTX_NO_ERROR
 *      Don't raise an error if the object type is wrong. Should not be used in
 *      conjunction with any flags that may raise errors not related to a wrong
 *      object type.
 */
#define QCTX_C              (1U << 0)
#define QCTX_S              (1U << 1)
#define QCTX_L              (1U << 2)
#define QCTX_AUTO_S         (1U << 3)
#define QCTX_REMOTE_INIT    (1U << 4)
#define QCTX_LOCK           (1U << 5)
#define QCTX_IO             (1U << 6)
#define QCTX_D              (1U << 7)
#define QCTX_NO_ERROR       (1U << 8)

/*
 * Called when expect_quic failed. Used to diagnose why such a call failed and
 * raise a reasonable error code based on the configured preconditions in flags.
 */
static int wrong_type(const SSL *s, uint32_t flags)
{
    const uint32_t mask = QCTX_C | QCTX_S | QCTX_L | QCTX_D;
    int code = ERR_R_UNSUPPORTED;

    if ((flags & QCTX_NO_ERROR) != 0)
        return 1;
    else if ((flags & mask) == QCTX_D)
        code = SSL_R_DOMAIN_USE_ONLY;
    else if ((flags & mask) == QCTX_L)
        code = SSL_R_LISTENER_USE_ONLY;
    else if ((flags & mask) == QCTX_C)
        code = SSL_R_CONN_USE_ONLY;
    else if ((flags & mask) == QCTX_S
             || (flags & mask) == (QCTX_C | QCTX_S))
        code = SSL_R_NO_STREAM;

    return QUIC_RAISE_NON_NORMAL_ERROR(NULL, code, NULL);
}

/*
 * Given a QDSO, QCSO, QSSO or QLSO, initialises a QCTX, determining the
 * contextually applicable QUIC_LISTENER, QUIC_CONNECTION and QUIC_XSO
 * pointers.
 *
 * After this returns 1, all fields of the passed QCTX are initialised.
 * Returns 0 on failure. This function is intended to be used to provide API
 * semantics and as such, it invokes QUIC_RAISE_NON_NORMAL_ERROR() on failure
 * unless the QCTX_NO_ERROR flag is set.
 *
 * The flags argument controls the preconditions and postconditions of this
 * function. See above for the different flags.
 *
 * The fields of a QCTX are initialised as follows depending on the identity of
 * the SSL object, and assuming the preconditions demanded by the flags field as
 * described above are met:
 *
 *                  QDSO        QLSO        QCSO        QSSO
 *   qd             non-NULL    maybe       maybe       maybe
 *   ql             NULL        non-NULL    maybe       maybe
 *   qc             NULL        NULL        non-NULL    non-NULL
 *   xso            NULL        NULL        maybe       non-NULL
 *   is_stream      0           0           0           1
 *   is_listener    0           1           0           0
 *   is_domain      1           0           0           0
 *
 */
static int expect_quic_as(const SSL *s, QCTX *ctx, uint32_t flags)
{
    int ok = 0, locked = 0, lock_requested = ((flags & QCTX_LOCK) != 0);
    QUIC_DOMAIN *qd;
    QUIC_LISTENER *ql;
    QUIC_CONNECTION *qc;
    QUIC_XSO *xso;

    if ((flags & QCTX_AUTO_S) != 0)
        flags |= QCTX_S;

    ctx->obj            = NULL;
    ctx->qd             = NULL;
    ctx->ql             = NULL;
    ctx->qc             = NULL;
    ctx->xso            = NULL;
    ctx->is_stream      = 0;
    ctx->is_listener    = 0;
    ctx->is_domain      = 0;
    ctx->in_io          = ((flags & QCTX_IO) != 0);

    if (s == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_PASSED_NULL_PARAMETER, NULL);
        goto err;
    }

    switch (s->type) {
    case SSL_TYPE_QUIC_DOMAIN:
        if ((flags & QCTX_D) == 0) {
            wrong_type(s, flags);
            goto err;
        }

        qd                  = (QUIC_DOMAIN *)s;
        ctx->obj            = &qd->obj;
        ctx->qd             = qd;
        ctx->is_domain      = 1;
        break;

    case SSL_TYPE_QUIC_LISTENER:
        if ((flags & QCTX_L) == 0) {
            wrong_type(s, flags);
            goto err;
        }

        ql                  = (QUIC_LISTENER *)s;
        ctx->obj            = &ql->obj;
        ctx->qd             = ql->domain;
        ctx->ql             = ql;
        ctx->is_listener    = 1;
        break;

    case SSL_TYPE_QUIC_CONNECTION:
        qc                  = (QUIC_CONNECTION *)s;
        ctx->obj            = &qc->obj;
        ctx->qd             = qc->domain;
        ctx->ql             = qc->listener; /* never changes, so can be read without lock */
        ctx->qc             = qc;

        if ((flags & QCTX_AUTO_S) != 0) {
            if ((flags & QCTX_IO) != 0)
                qctx_lock_for_io(ctx);
            else
                qctx_lock(ctx);

            locked = 1;
        }

        if ((flags & QCTX_AUTO_S) != 0 && qc->default_xso == NULL) {
            if (!quic_mutation_allowed(qc, /*req_active=*/0)) {
                QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
                goto err;
            }

            /* If we haven't finished the handshake, try to advance it. */
            if (quic_do_handshake(ctx) < 1)
                /* ossl_quic_do_handshake raised error here */
                goto err;

            if ((flags & QCTX_REMOTE_INIT) != 0) {
                if (!qc_wait_for_default_xso_for_read(ctx, /*peek=*/0))
                    goto err;
            } else {
                if (!qc_try_create_default_xso_for_write(ctx))
                    goto err;
            }
        }

        if ((flags & QCTX_C) == 0
            && (qc->default_xso == NULL || (flags & QCTX_S) == 0)) {
            wrong_type(s, flags);
            goto err;
        }

        ctx->xso            = qc->default_xso;
        break;

    case SSL_TYPE_QUIC_XSO:
        if ((flags & QCTX_S) == 0) {
            wrong_type(s, flags);
            goto err;
        }

        xso                 = (QUIC_XSO *)s;
        ctx->obj            = &xso->obj;
        ctx->qd             = xso->conn->domain;
        ctx->ql             = xso->conn->listener;
        ctx->qc             = xso->conn;
        ctx->xso            = xso;
        ctx->is_stream      = 1;
        break;

    default:
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    if (lock_requested && !locked) {
        if ((flags & QCTX_IO) != 0)
            qctx_lock_for_io(ctx);
        else
            qctx_lock(ctx);

        locked = 1;
    }

    ok = 1;
err:
    if (locked && (!ok || !lock_requested))
        qctx_unlock(ctx);

    return ok;
}

static int is_quic_c(const SSL *s, QCTX *ctx, int raiseerrs)
{
    uint32_t flags = QCTX_C;

    if (!raiseerrs)
        flags |= QCTX_NO_ERROR;
    return expect_quic_as(s, ctx, flags);
}

/* Same as expect_quic_cs except that errors are not raised if raiseerrs == 0 */
static int is_quic_cs(const SSL *s, QCTX *ctx, int raiseerrs)
{
    uint32_t flags = QCTX_C | QCTX_S;

    if (!raiseerrs)
        flags |= QCTX_NO_ERROR;
    return expect_quic_as(s, ctx, flags);
}

static int expect_quic_cs(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_C | QCTX_S);
}

static int expect_quic_csl(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_C | QCTX_S | QCTX_L);
}

static int expect_quic_csld(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_C | QCTX_S | QCTX_L | QCTX_D);
}

#define expect_quic_any expect_quic_csld

static int expect_quic_listener(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_L);
}

static int expect_quic_domain(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_D);
}

/*
 * Like expect_quic_cs(), but requires a QUIC_XSO be contextually available. In
 * other words, requires that the passed QSO be a QSSO or a QCSO with a default
 * stream.
 *
 * remote_init determines if we expect the default XSO to be remotely created or
 * not. If it is -1, do not instantiate a default XSO if one does not yet exist.
 *
 * Channel mutex is acquired and retained on success.
 */
QUIC_ACQUIRES_LOCK
static int ossl_unused expect_quic_with_stream_lock(const SSL *s, int remote_init,
                                                    int in_io, QCTX *ctx)
{
    uint32_t flags = QCTX_S | QCTX_LOCK;

    if (remote_init >= 0)
        flags |= QCTX_AUTO_S;

    if (remote_init > 0)
        flags |= QCTX_REMOTE_INIT;

    if (in_io)
        flags |= QCTX_IO;

    return expect_quic_as(s, ctx, flags);
}

/*
 * Like expect_quic_cs(), but fails if called on a QUIC_XSO. ctx->xso may still
 * be non-NULL if the QCSO has a default stream.
 */
static int ossl_unused expect_quic_conn_only(const SSL *s, QCTX *ctx)
{
    return expect_quic_as(s, ctx, QCTX_C);
}

/*
 * Ensures that the domain mutex is held for a method which touches channel
 * state.
 *
 * Precondition: Domain mutex is not held (unchecked)
 */
static void qctx_lock(QCTX *ctx)
{
#if defined(OPENSSL_THREADS)
    assert(ctx->obj != NULL);
    ossl_crypto_mutex_lock(ossl_quic_obj_get0_mutex(ctx->obj));
#endif
}

/* Precondition: Channel mutex is held (unchecked) */
QUIC_NEEDS_LOCK
static void qctx_unlock(QCTX *ctx)
{
#if defined(OPENSSL_THREADS)
    assert(ctx->obj != NULL);
    ossl_crypto_mutex_unlock(ossl_quic_obj_get0_mutex(ctx->obj));
#endif
}

static void qctx_lock_for_io(QCTX *ctx)
{
    qctx_lock(ctx);
    ctx->in_io = 1;

    /*
     * We are entering an I/O function so we must update the values returned by
     * SSL_get_error and SSL_want. Set no error. This will be overridden later
     * if a call to QUIC_RAISE_NORMAL_ERROR or QUIC_RAISE_NON_NORMAL_ERROR
     * occurs during the API call.
     */
    quic_set_last_error(ctx, SSL_ERROR_NONE);
}

/*
 * This predicate is the criterion which should determine API call rejection for
 * *most* mutating API calls, particularly stream-related operations for send
 * parts.
 *
 * A call is rejected (this function returns 0) if shutdown is in progress
 * (stream flushing), or we are in a TERMINATING or TERMINATED state. If
 * req_active=1, the connection must be active (i.e., the IDLE state is also
 * rejected).
 */
static int quic_mutation_allowed(QUIC_CONNECTION *qc, int req_active)
{
    if (qc->shutting_down || ossl_quic_channel_is_term_any(qc->ch))
        return 0;

    if (req_active && !ossl_quic_channel_is_active(qc->ch))
        return 0;

    return 1;
}

static int qctx_is_top_level(QCTX *ctx)
{
    return ctx->obj->parent_obj == NULL;
}

static int qctx_blocking(QCTX *ctx)
{
    return ossl_quic_obj_blocking(ctx->obj);
}

/*
 * Block until a predicate is met.
 *
 * Precondition: Must have a channel.
 * Precondition: Must hold channel lock (unchecked).
 */
QUIC_NEEDS_LOCK
static int block_until_pred(QCTX *ctx,
                            int (*pred)(void *arg), void *pred_arg,
                            uint32_t flags)
{
    QUIC_ENGINE *qeng;
    QUIC_REACTOR *rtor;

    qeng = ossl_quic_obj_get0_engine(ctx->obj);
    assert(qeng != NULL);

    /*
     * Any attempt to block auto-disables tick inhibition as otherwise we will
     * hang around forever.
     */
    ossl_quic_engine_set_inhibit_tick(qeng, 0);

    rtor = ossl_quic_engine_get0_reactor(qeng);
    return ossl_quic_reactor_block_until_pred(rtor, pred, pred_arg, flags);
}

/*
 * QUIC Front-End I/O API: Initialization
 * ======================================
 *
 *         SSL_new                  => ossl_quic_new
 *                                     ossl_quic_init
 *         SSL_reset                => ossl_quic_reset
 *         SSL_clear                => ossl_quic_clear
 *                                     ossl_quic_deinit
 *         SSL_free                 => ossl_quic_free
 *
 *         SSL_set_options          => ossl_quic_set_options
 *         SSL_get_options          => ossl_quic_get_options
 *         SSL_clear_options        => ossl_quic_clear_options
 *
 */

/* SSL_new */
SSL *ossl_quic_new(SSL_CTX *ctx)
{
    QUIC_CONNECTION *qc = NULL;
    SSL_CONNECTION *sc = NULL;

    /*
     * QUIC_server_method should not be used with SSL_new.
     * It should only be used with SSL_new_listener.
     */
    if (ctx->method == OSSL_QUIC_server_method()) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED, NULL);
        return NULL;
    }

    qc = OPENSSL_zalloc(sizeof(*qc));
    if (qc == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        return NULL;
    }

    /* Create the QUIC domain mutex. */
#if defined(OPENSSL_THREADS)
    if ((qc->mutex = ossl_crypto_mutex_new()) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }
#endif

    /* Create the handshake layer. */
    qc->tls = ossl_ssl_connection_new_int(ctx, &qc->obj.ssl, TLS_method());
    if (qc->tls == NULL || (sc = SSL_CONNECTION_FROM_SSL(qc->tls)) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* override the user_ssl of the inner connection */
    sc->s3.flags |= TLS1_FLAGS_QUIC | TLS1_FLAGS_QUIC_INTERNAL;

    /* Restrict options derived from the SSL_CTX. */
    sc->options &= OSSL_QUIC_PERMITTED_OPTIONS_CONN;
    sc->pha_enabled = 0;

    /* Determine mode of operation. */
#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
    qc->is_thread_assisted
        = ((ctx->domain_flags & SSL_DOMAIN_FLAG_THREAD_ASSISTED) != 0);
#endif

    qc->as_server       = 0;
    qc->as_server_state = qc->as_server;

    if (!create_channel(qc, ctx))
        goto err;

    ossl_quic_channel_set_msg_callback(qc->ch, ctx->msg_callback, &qc->obj.ssl);
    ossl_quic_channel_set_msg_callback_arg(qc->ch, ctx->msg_callback_arg);

    /* Initialise the QUIC_CONNECTION's QUIC_OBJ base. */
    if (!ossl_quic_obj_init(&qc->obj, ctx, SSL_TYPE_QUIC_CONNECTION, NULL,
                            qc->engine, qc->port)) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* Initialise libssl APL-related state. */
    qc->default_stream_mode     = SSL_DEFAULT_STREAM_MODE_AUTO_BIDI;
    qc->default_ssl_mode        = qc->obj.ssl.ctx->mode;
    qc->default_ssl_options     = qc->obj.ssl.ctx->options & OSSL_QUIC_PERMITTED_OPTIONS;
    qc->incoming_stream_policy  = SSL_INCOMING_STREAM_POLICY_AUTO;
    qc->last_error              = SSL_ERROR_NONE;

    qc_update_reject_policy(qc);

    /*
     * We do not create the default XSO yet. The reason for this is that the
     * stream ID of the default XSO will depend on whether the stream is client
     * or server-initiated, which depends on who transmits first. Since we do
     * not know whether the application will be using a client-transmits-first
     * or server-transmits-first protocol, we defer default XSO creation until
     * the client calls SSL_read() or SSL_write(). If it calls SSL_read() first,
     * we take that as a cue that the client is expecting a server-initiated
     * stream, and vice versa if SSL_write() is called first.
     */
    return &qc->obj.ssl;

err:
    if (qc != NULL) {
        qc_cleanup(qc, /*have_lock=*/0);
        OPENSSL_free(qc);
    }
    return NULL;
}

QUIC_NEEDS_LOCK
static void quic_unref_port_bios(QUIC_PORT *port)
{
    BIO *b;

    b = ossl_quic_port_get_net_rbio(port);
    BIO_free_all(b);

    b = ossl_quic_port_get_net_wbio(port);
    BIO_free_all(b);
}

QUIC_NEEDS_LOCK
static void qc_cleanup(QUIC_CONNECTION *qc, int have_lock)
{
    SSL_free(qc->tls);
    qc->tls = NULL;

    ossl_quic_channel_free(qc->ch);
    qc->ch = NULL;

    if (qc->port != NULL && qc->listener == NULL && qc->pending == 0) { /* TODO */
        quic_unref_port_bios(qc->port);
        ossl_quic_port_free(qc->port);
        qc->port = NULL;

        ossl_quic_engine_free(qc->engine);
        qc->engine = NULL;
    }

#if defined(OPENSSL_THREADS)
    if (have_lock)
        /* tsan doesn't like freeing locked mutexes */
        ossl_crypto_mutex_unlock(qc->mutex);

    if (qc->listener == NULL && qc->pending == 0)
        ossl_crypto_mutex_free(&qc->mutex);
#endif
}

/* SSL_free */
QUIC_TAKES_LOCK
static void quic_free_listener(QCTX *ctx)
{
    quic_unref_port_bios(ctx->ql->port);
    ossl_quic_port_drop_incoming(ctx->ql->port);
    ossl_quic_port_free(ctx->ql->port);

    if (ctx->ql->domain == NULL) {
        ossl_quic_engine_free(ctx->ql->engine);
#if defined(OPENSSL_THREADS)
        ossl_crypto_mutex_free(&ctx->ql->mutex);
#endif
    } else {
        SSL_free(&ctx->ql->domain->obj.ssl);
    }
}

/* SSL_free */
QUIC_TAKES_LOCK
static void quic_free_domain(QCTX *ctx)
{
    ossl_quic_engine_free(ctx->qd->engine);
#if defined(OPENSSL_THREADS)
    ossl_crypto_mutex_free(&ctx->qd->mutex);
#endif
}

QUIC_TAKES_LOCK
void ossl_quic_free(SSL *s)
{
    QCTX ctx;
    int is_default;

    /* We should never be called on anything but a QSO. */
    if (!expect_quic_any(s, &ctx))
        return;

    if (ctx.is_domain) {
        quic_free_domain(&ctx);
        return;
    }

    if (ctx.is_listener) {
        quic_free_listener(&ctx);
        return;
    }

    qctx_lock(&ctx);

    if (ctx.is_stream) {
        /*
         * When a QSSO is freed, the XSO is freed immediately, because the XSO
         * itself only contains API personality layer data. However the
         * underlying QUIC_STREAM is not freed immediately but is instead marked
         * as deleted for later collection.
         */

        assert(ctx.qc->num_xso > 0);
        --ctx.qc->num_xso;

        /* If a stream's send part has not been finished, auto-reset it. */
        if ((   ctx.xso->stream->send_state == QUIC_SSTREAM_STATE_READY
             || ctx.xso->stream->send_state == QUIC_SSTREAM_STATE_SEND)
            && !ossl_quic_sstream_get_final_size(ctx.xso->stream->sstream, NULL))
            ossl_quic_stream_map_reset_stream_send_part(ossl_quic_channel_get_qsm(ctx.qc->ch),
                                                        ctx.xso->stream, 0);

        /* Do STOP_SENDING for the receive part, if applicable. */
        if (   ctx.xso->stream->recv_state == QUIC_RSTREAM_STATE_RECV
            || ctx.xso->stream->recv_state == QUIC_RSTREAM_STATE_SIZE_KNOWN)
            ossl_quic_stream_map_stop_sending_recv_part(ossl_quic_channel_get_qsm(ctx.qc->ch),
                                                        ctx.xso->stream, 0);

        /* Update stream state. */
        ctx.xso->stream->deleted = 1;
        ossl_quic_stream_map_update_state(ossl_quic_channel_get_qsm(ctx.qc->ch),
                                          ctx.xso->stream);

        is_default = (ctx.xso == ctx.qc->default_xso);
        qctx_unlock(&ctx);

        /*
         * Unref the connection in most cases; the XSO has a ref to the QC and
         * not vice versa. But for a default XSO, to avoid circular references,
         * the QC refs the XSO but the XSO does not ref the QC. If we are the
         * default XSO, we only get here when the QC is being torn down anyway,
         * so don't call SSL_free(qc) as we are already in it.
         */
        if (!is_default)
            SSL_free(&ctx.qc->obj.ssl);

        /* Note: SSL_free calls OPENSSL_free(xso) for us */
        return;
    }

    /*
     * Free the default XSO, if any. The QUIC_STREAM is not deleted at this
     * stage, but is freed during the channel free when the whole QSM is freed.
     */
    if (ctx.qc->default_xso != NULL) {
        QUIC_XSO *xso = ctx.qc->default_xso;

        qctx_unlock(&ctx);
        SSL_free(&xso->obj.ssl);
        qctx_lock(&ctx);
        ctx.qc->default_xso = NULL;
    }

    /* Ensure we have no remaining XSOs. */
    assert(ctx.qc->num_xso == 0);

#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
    if (ctx.qc->is_thread_assisted && ctx.qc->started) {
        ossl_quic_thread_assist_wait_stopped(&ctx.qc->thread_assist);
        ossl_quic_thread_assist_cleanup(&ctx.qc->thread_assist);
    }
#endif

    /*
     * Note: SSL_free (that called this function) calls OPENSSL_free(ctx.qc) for
     * us
     */
    qc_cleanup(ctx.qc, /*have_lock=*/1);
    /* Note: SSL_free calls OPENSSL_free(qc) for us */

    if (ctx.qc->listener != NULL)
        SSL_free(&ctx.qc->listener->obj.ssl);
    if (ctx.qc->domain != NULL)
        SSL_free(&ctx.qc->domain->obj.ssl);
}

/* SSL method init */
int ossl_quic_init(SSL *s)
{
    /* Same op as SSL_clear, forward the call. */
    return ossl_quic_clear(s);
}

/* SSL method deinit */
void ossl_quic_deinit(SSL *s)
{
    /* No-op. */
}

/* SSL_clear (ssl_reset method) */
int ossl_quic_reset(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_any(s, &ctx))
        return 0;

    ERR_raise(ERR_LIB_SSL, ERR_R_UNSUPPORTED);
    return 0;
}

/* ssl_clear method (unused) */
int ossl_quic_clear(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_any(s, &ctx))
        return 0;

    ERR_raise(ERR_LIB_SSL, ERR_R_UNSUPPORTED);
    return 0;
}

int ossl_quic_set_override_now_cb(SSL *s,
                                  OSSL_TIME (*now_cb)(void *arg),
                                  void *now_cb_arg)
{
    QCTX ctx;

    if (!expect_quic_any(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    ossl_quic_engine_set_time_cb(ctx.obj->engine, now_cb, now_cb_arg);

    qctx_unlock(&ctx);
    return 1;
}

void ossl_quic_conn_force_assist_thread_wake(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return;

#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
    if (ctx.qc->is_thread_assisted && ctx.qc->started)
        ossl_quic_thread_assist_notify_deadline_changed(&ctx.qc->thread_assist);
#endif
}

QUIC_NEEDS_LOCK
static void qc_touch_default_xso(QUIC_CONNECTION *qc)
{
    qc->default_xso_created = 1;
    qc_update_reject_policy(qc);
}

/*
 * Changes default XSO. Allows caller to keep reference to the old default XSO
 * (if any). Reference to new XSO is transferred from caller.
 */
QUIC_NEEDS_LOCK
static void qc_set_default_xso_keep_ref(QUIC_CONNECTION *qc, QUIC_XSO *xso,
                                        int touch,
                                        QUIC_XSO **old_xso)
{
    int refs;

    *old_xso = NULL;

    if (qc->default_xso != xso) {
        *old_xso = qc->default_xso; /* transfer old XSO ref to caller */

        qc->default_xso = xso;

        if (xso == NULL) {
            /*
             * Changing to not having a default XSO. XSO becomes standalone and
             * now has a ref to the QC.
             */
            if (!ossl_assert(SSL_up_ref(&qc->obj.ssl)))
                return;
        } else {
            /*
             * Changing from not having a default XSO to having one. The new XSO
             * will have had a reference to the QC we need to drop to avoid a
             * circular reference.
             *
             * Currently we never change directly from one default XSO to
             * another, though this function would also still be correct if this
             * weren't the case.
             */
            assert(*old_xso == NULL);

            CRYPTO_DOWN_REF(&qc->obj.ssl.references, &refs);
            assert(refs > 0);
        }
    }

    if (touch)
        qc_touch_default_xso(qc);
}

/*
 * Changes default XSO, releasing the reference to any previous default XSO.
 * Reference to new XSO is transferred from caller.
 */
QUIC_NEEDS_LOCK
static void qc_set_default_xso(QUIC_CONNECTION *qc, QUIC_XSO *xso, int touch)
{
    QUIC_XSO *old_xso = NULL;

    qc_set_default_xso_keep_ref(qc, xso, touch, &old_xso);

    if (old_xso != NULL)
        SSL_free(&old_xso->obj.ssl);
}

QUIC_NEEDS_LOCK
static void xso_update_options(QUIC_XSO *xso)
{
    int cleanse = ((xso->ssl_options & SSL_OP_CLEANSE_PLAINTEXT) != 0);

    if (xso->stream->rstream != NULL)
        ossl_quic_rstream_set_cleanse(xso->stream->rstream, cleanse);

    if (xso->stream->sstream != NULL)
        ossl_quic_sstream_set_cleanse(xso->stream->sstream, cleanse);
}

/*
 * SSL_set_options
 * ---------------
 *
 * Setting options on a QCSO
 *   - configures the handshake-layer options;
 *   - configures the default data-plane options for new streams;
 *   - configures the data-plane options on the default XSO, if there is one.
 *
 * Setting options on a QSSO
 *   - configures data-plane options for that stream only.
 */
QUIC_TAKES_LOCK
static uint64_t quic_mask_or_options(SSL *ssl, uint64_t mask_value, uint64_t or_value)
{
    QCTX ctx;
    uint64_t hs_mask_value, hs_or_value, ret;

    if (!expect_quic_cs(ssl, &ctx))
        return 0;

    qctx_lock(&ctx);

    if (!ctx.is_stream) {
        /*
         * If we were called on the connection, we apply any handshake option
         * changes.
         */
        hs_mask_value = (mask_value & OSSL_QUIC_PERMITTED_OPTIONS_CONN);
        hs_or_value   = (or_value   & OSSL_QUIC_PERMITTED_OPTIONS_CONN);

        SSL_clear_options(ctx.qc->tls, hs_mask_value);
        SSL_set_options(ctx.qc->tls, hs_or_value);

        /* Update defaults for new streams. */
        ctx.qc->default_ssl_options
            = ((ctx.qc->default_ssl_options & ~mask_value) | or_value)
              & OSSL_QUIC_PERMITTED_OPTIONS;
    }

    ret = ctx.qc->default_ssl_options;
    if (ctx.xso != NULL) {
        ctx.xso->ssl_options
            = ((ctx.xso->ssl_options & ~mask_value) | or_value)
            & OSSL_QUIC_PERMITTED_OPTIONS_STREAM;

        xso_update_options(ctx.xso);

        if (ctx.is_stream)
            ret = ctx.xso->ssl_options;
    }

    qctx_unlock(&ctx);
    return ret;
}

uint64_t ossl_quic_set_options(SSL *ssl, uint64_t options)
{
    return quic_mask_or_options(ssl, 0, options);
}

/* SSL_clear_options */
uint64_t ossl_quic_clear_options(SSL *ssl, uint64_t options)
{
    return quic_mask_or_options(ssl, options, 0);
}

/* SSL_get_options */
uint64_t ossl_quic_get_options(const SSL *ssl)
{
    return quic_mask_or_options((SSL *)ssl, 0, 0);
}

/*
 * QUIC Front-End I/O API: Network BIO Configuration
 * =================================================
 *
 * Handling the different BIOs is difficult:
 *
 *   - It is more or less a requirement that we use non-blocking network I/O;
 *     we need to be able to have timeouts on recv() calls, and make best effort
 *     (non blocking) send() and recv() calls.
 *
 *     The only sensible way to do this is to configure the socket into
 *     non-blocking mode. We could try to do select() before calling send() or
 *     recv() to get a guarantee that the call will not block, but this will
 *     probably run into issues with buggy OSes which generate spurious socket
 *     readiness events. In any case, relying on this to work reliably does not
 *     seem sane.
 *
 *     Timeouts could be handled via setsockopt() socket timeout options, but
 *     this depends on OS support and adds another syscall to every network I/O
 *     operation. It also has obvious thread safety concerns if we want to move
 *     to concurrent use of a single socket at some later date.
 *
 *     Some OSes support a MSG_DONTWAIT flag which allows a single I/O option to
 *     be made non-blocking. However some OSes (e.g. Windows) do not support
 *     this, so we cannot rely on this.
 *
 *     As such, we need to configure any FD in non-blocking mode. This may
 *     confound users who pass a blocking socket to libssl. However, in practice
 *     it would be extremely strange for a user of QUIC to pass an FD to us,
 *     then also try and send receive traffic on the same socket(!). Thus the
 *     impact of this should be limited, and can be documented.
 *
 *   - We support both blocking and non-blocking operation in terms of the API
 *     presented to the user. One prospect is to set the blocking mode based on
 *     whether the socket passed to us was already in blocking mode. However,
 *     Windows has no API for determining if a socket is in blocking mode (!),
 *     therefore this cannot be done portably. Currently therefore we expose an
 *     explicit API call to set this, and default to blocking mode.
 *
 *   - We need to determine our initial destination UDP address. The "natural"
 *     way for a user to do this is to set the peer variable on a BIO_dgram.
 *     However, this has problems because BIO_dgram's peer variable is used for
 *     both transmission and reception. This means it can be constantly being
 *     changed to a malicious value (e.g. if some random unrelated entity on the
 *     network starts sending traffic to us) on every read call. This is not a
 *     direct issue because we use the 'stateless' BIO_sendmmsg and BIO_recvmmsg
 *     calls only, which do not use this variable. However, we do need to let
 *     the user specify the peer in a 'normal' manner. The compromise here is
 *     that we grab the current peer value set at the time the write BIO is set
 *     and do not read the value again.
 *
 *   - We also need to support memory BIOs (e.g. BIO_dgram_pair) or custom BIOs.
 *     Currently we do this by only supporting non-blocking mode.
 *
 */

/*
 * Determines what initial destination UDP address we should use, if possible.
 * If this fails the client must set the destination address manually, or use a
 * BIO which does not need a destination address.
 */
static int csm_analyse_init_peer_addr(BIO *net_wbio, BIO_ADDR *peer)
{
    if (BIO_dgram_detect_peer_addr(net_wbio, peer) <= 0)
        return 0;

    return 1;
}

static int
quic_set0_net_rbio(QUIC_OBJ *obj, BIO *net_rbio)
{
    QUIC_PORT *port;
    BIO *old_rbio = NULL;

    port = ossl_quic_obj_get0_port(obj);
    old_rbio = ossl_quic_port_get_net_rbio(port);
    if (old_rbio == net_rbio)
        return 0;

    if (!ossl_quic_port_set_net_rbio(port, net_rbio))
        return 0;

    BIO_free_all(old_rbio);
    if (net_rbio != NULL)
        BIO_set_nbio(net_rbio, 1); /* best effort autoconfig */

    return 1;
}

static int
quic_set0_net_wbio(QUIC_OBJ *obj, BIO *net_wbio)
{
    QUIC_PORT *port;
    BIO *old_wbio = NULL;

    port = ossl_quic_obj_get0_port(obj);
    old_wbio = ossl_quic_port_get_net_wbio(port);
    if (old_wbio == net_wbio)
        return 0;

    if (!ossl_quic_port_set_net_wbio(port, net_wbio))
        return 0;

    BIO_free_all(old_wbio);
    if (net_wbio != NULL)
        BIO_set_nbio(net_wbio, 1); /* best effort autoconfig */

    return 1;
}

void ossl_quic_conn_set0_net_rbio(SSL *s, BIO *net_rbio)
{
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return;

    /* Returns 0 if no change. */
    if (!quic_set0_net_rbio(ctx.obj, net_rbio))
        return;
}

void ossl_quic_conn_set0_net_wbio(SSL *s, BIO *net_wbio)
{
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return;

    /* Returns 0 if no change. */
    if (!quic_set0_net_wbio(ctx.obj, net_wbio))
        return;
}

BIO *ossl_quic_conn_get_net_rbio(const SSL *s)
{
    QCTX ctx;
    QUIC_PORT *port;

    if (!expect_quic_csl(s, &ctx))
        return NULL;

    port = ossl_quic_obj_get0_port(ctx.obj);
    assert(port != NULL);
    return ossl_quic_port_get_net_rbio(port);
}

BIO *ossl_quic_conn_get_net_wbio(const SSL *s)
{
    QCTX ctx;
    QUIC_PORT *port;

    if (!expect_quic_csl(s, &ctx))
        return NULL;

    port = ossl_quic_obj_get0_port(ctx.obj);
    assert(port != NULL);
    return ossl_quic_port_get_net_wbio(port);
}

int ossl_quic_conn_get_blocking_mode(const SSL *s)
{
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    return qctx_blocking(&ctx);
}

QUIC_TAKES_LOCK
int ossl_quic_conn_set_blocking_mode(SSL *s, int blocking)
{
    int ret = 0;
    unsigned int mode;
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    /* Sanity check - can we support the request given the current network BIO? */
    if (blocking) {
        /*
         * If called directly on a top-level object (QCSO or QLSO), update our
         * information on network BIO capabilities.
         */
        if (qctx_is_top_level(&ctx))
            ossl_quic_engine_update_poll_descriptors(ctx.obj->engine, /*force=*/1);

        /* Cannot enable blocking mode if we do not have pollable FDs. */
        if (!ossl_quic_obj_can_support_blocking(ctx.obj)) {
            ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);
            goto out;
        }
    }

    mode = (blocking != 0)
        ? QUIC_BLOCKING_MODE_BLOCKING
        : QUIC_BLOCKING_MODE_NONBLOCKING;

    ossl_quic_obj_set_blocking_mode(ctx.obj, mode);

    ret = 1;
out:
    qctx_unlock(&ctx);
    return ret;
}

int ossl_quic_conn_set_initial_peer_addr(SSL *s,
                                         const BIO_ADDR *peer_addr)
{
    QCTX ctx;

    if (!expect_quic_cs(s, &ctx))
        return 0;

    if (ctx.qc->started)
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED,
                                       NULL);

    if (peer_addr == NULL) {
        BIO_ADDR_clear(&ctx.qc->init_peer_addr);
        return 1;
    }

    return BIO_ADDR_copy(&ctx.qc->init_peer_addr, peer_addr);
}

/*
 * QUIC Front-End I/O API: Asynchronous I/O Management
 * ===================================================
 *
 *   (BIO/)SSL_handle_events        => ossl_quic_handle_events
 *   (BIO/)SSL_get_event_timeout    => ossl_quic_get_event_timeout
 *   (BIO/)SSL_get_poll_fd          => ossl_quic_get_poll_fd
 *
 */

/* SSL_handle_events; performs QUIC I/O and timeout processing. */
QUIC_TAKES_LOCK
int ossl_quic_handle_events(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_any(s, &ctx))
        return 0;

    qctx_lock(&ctx);
    ossl_quic_reactor_tick(ossl_quic_obj_get0_reactor(ctx.obj), 0);
    qctx_unlock(&ctx);
    return 1;
}

/*
 * SSL_get_event_timeout. Get the time in milliseconds until the SSL object
 * should next have events handled by the application by calling
 * SSL_handle_events(). tv is set to 0 if the object should have events handled
 * immediately. If no timeout is currently active, *is_infinite is set to 1 and
 * the value of *tv is undefined.
 */
QUIC_TAKES_LOCK
int ossl_quic_get_event_timeout(SSL *s, struct timeval *tv, int *is_infinite)
{
    QCTX ctx;
    QUIC_REACTOR *reactor;
    OSSL_TIME deadline;
    OSSL_TIME basetime;

    if (!expect_quic_any(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    reactor = ossl_quic_obj_get0_reactor(ctx.obj);
    deadline = ossl_quic_reactor_get_tick_deadline(reactor);

    if (ossl_time_is_infinite(deadline)) {
        qctx_unlock(&ctx);
        *is_infinite = 1;

        /*
         * Robustness against faulty applications that don't check *is_infinite;
         * harmless long timeout.
         */
        tv->tv_sec  = 1000000;
        tv->tv_usec = 0;
        return 1;
    }

    basetime = ossl_quic_engine_get_time(ctx.obj->engine);

    qctx_unlock(&ctx);

    *tv = ossl_time_to_timeval(ossl_time_subtract(deadline, basetime));
    *is_infinite = 0;

    return 1;
}

/* SSL_get_rpoll_descriptor */
int ossl_quic_get_rpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *desc)
{
    QCTX ctx;
    QUIC_PORT *port = NULL;
    BIO *net_rbio;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    port = ossl_quic_obj_get0_port(ctx.obj);
    net_rbio = ossl_quic_port_get_net_rbio(port);
    if (desc == NULL || net_rbio == NULL)
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                       NULL);

    return BIO_get_rpoll_descriptor(net_rbio, desc);
}

/* SSL_get_wpoll_descriptor */
int ossl_quic_get_wpoll_descriptor(SSL *s, BIO_POLL_DESCRIPTOR *desc)
{
    QCTX ctx;
    QUIC_PORT *port = NULL;
    BIO *net_wbio;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    port = ossl_quic_obj_get0_port(ctx.obj);
    net_wbio = ossl_quic_port_get_net_wbio(port);
    if (desc == NULL || net_wbio == NULL)
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                       NULL);

    return BIO_get_wpoll_descriptor(net_wbio, desc);
}

/* SSL_net_read_desired */
QUIC_TAKES_LOCK
int ossl_quic_get_net_read_desired(SSL *s)
{
    QCTX ctx;
    int ret;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    qctx_lock(&ctx);
    ret = ossl_quic_reactor_net_read_desired(ossl_quic_obj_get0_reactor(ctx.obj));
    qctx_unlock(&ctx);
    return ret;
}

/* SSL_net_write_desired */
QUIC_TAKES_LOCK
int ossl_quic_get_net_write_desired(SSL *s)
{
    int ret;
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    qctx_lock(&ctx);
    ret = ossl_quic_reactor_net_write_desired(ossl_quic_obj_get0_reactor(ctx.obj));
    qctx_unlock(&ctx);
    return ret;
}

/*
 * QUIC Front-End I/O API: Connection Lifecycle Operations
 * =======================================================
 *
 *         SSL_do_handshake         => ossl_quic_do_handshake
 *         SSL_set_connect_state    => ossl_quic_set_connect_state
 *         SSL_set_accept_state     => ossl_quic_set_accept_state
 *         SSL_shutdown             => ossl_quic_shutdown
 *         SSL_ctrl                 => ossl_quic_ctrl
 *   (BIO/)SSL_connect              => ossl_quic_connect
 *   (BIO/)SSL_accept               => ossl_quic_accept
 *
 */

QUIC_NEEDS_LOCK
static void qc_shutdown_flush_init(QUIC_CONNECTION *qc)
{
    QUIC_STREAM_MAP *qsm;

    if (qc->shutting_down)
        return;

    qsm = ossl_quic_channel_get_qsm(qc->ch);

    ossl_quic_stream_map_begin_shutdown_flush(qsm);
    qc->shutting_down = 1;
}

/* Returns 1 if all shutdown-flush streams have been done with. */
QUIC_NEEDS_LOCK
static int qc_shutdown_flush_finished(QUIC_CONNECTION *qc)
{
    QUIC_STREAM_MAP *qsm = ossl_quic_channel_get_qsm(qc->ch);

    return qc->shutting_down
        && ossl_quic_stream_map_is_shutdown_flush_finished(qsm);
}

/* SSL_shutdown */
static int quic_shutdown_wait(void *arg)
{
    QUIC_CONNECTION *qc = arg;

    return ossl_quic_channel_is_terminated(qc->ch);
}

/* Returns 1 if shutdown flush process has finished or is inapplicable. */
static int quic_shutdown_flush_wait(void *arg)
{
    QUIC_CONNECTION *qc = arg;

    return ossl_quic_channel_is_term_any(qc->ch)
        || qc_shutdown_flush_finished(qc);
}

static int quic_shutdown_peer_wait(void *arg)
{
    QUIC_CONNECTION *qc = arg;
    return ossl_quic_channel_is_term_any(qc->ch);
}

QUIC_TAKES_LOCK
int ossl_quic_conn_shutdown(SSL *s, uint64_t flags,
                            const SSL_SHUTDOWN_EX_ARGS *args,
                            size_t args_len)
{
    int ret;
    QCTX ctx;
    int stream_flush = ((flags & SSL_SHUTDOWN_FLAG_NO_STREAM_FLUSH) == 0);
    int no_block = ((flags & SSL_SHUTDOWN_FLAG_NO_BLOCK) != 0);
    int wait_peer = ((flags & SSL_SHUTDOWN_FLAG_WAIT_PEER) != 0);

    if (!expect_quic_cs(s, &ctx))
        return -1;

    if (ctx.is_stream) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_CONN_USE_ONLY, NULL);
        return -1;
    }

    qctx_lock(&ctx);

    if (ossl_quic_channel_is_terminated(ctx.qc->ch)) {
        qctx_unlock(&ctx);
        return 1;
    }

    /* Phase 1: Stream Flushing */
    if (!wait_peer && stream_flush) {
        qc_shutdown_flush_init(ctx.qc);

        if (!qc_shutdown_flush_finished(ctx.qc)) {
            if (!no_block && qctx_blocking(&ctx)) {
                ret = block_until_pred(&ctx, quic_shutdown_flush_wait, ctx.qc, 0);
                if (ret < 1) {
                    ret = 0;
                    goto err;
                }
            } else {
                qctx_maybe_autotick(&ctx);
            }
        }

        if (!qc_shutdown_flush_finished(ctx.qc)) {
            qctx_unlock(&ctx);
            return 0; /* ongoing */
        }
    }

    /* Phase 2: Connection Closure */
    if (wait_peer && !ossl_quic_channel_is_term_any(ctx.qc->ch)) {
        if (!no_block && qctx_blocking(&ctx)) {
            ret = block_until_pred(&ctx, quic_shutdown_peer_wait, ctx.qc, 0);
            if (ret < 1) {
                ret = 0;
                goto err;
            }
        } else {
            qctx_maybe_autotick(&ctx);
        }

        if (!ossl_quic_channel_is_term_any(ctx.qc->ch)) {
            ret = 0; /* peer hasn't closed yet - still not done */
            goto err;
        }

        /*
         * We are at least terminating - go through the normal process of
         * waiting until we are in the TERMINATED state.
         */
    }

    /* Block mutation ops regardless of if we did stream flush. */
    ctx.qc->shutting_down = 1;

    /*
     * This call is a no-op if we are already terminating, so it doesn't
     * affect the wait_peer case.
     */
    ossl_quic_channel_local_close(ctx.qc->ch,
                                  args != NULL ? args->quic_error_code : 0,
                                  args != NULL ? args->quic_reason : NULL);

    SSL_set_shutdown(ctx.qc->tls, SSL_SENT_SHUTDOWN);

    if (ossl_quic_channel_is_terminated(ctx.qc->ch)) {
        qctx_unlock(&ctx);
        return 1;
    }

    /* Phase 3: Terminating Wait Time */
    if (!no_block && qctx_blocking(&ctx)
        && (flags & SSL_SHUTDOWN_FLAG_RAPID) == 0) {
        ret = block_until_pred(&ctx, quic_shutdown_wait, ctx.qc, 0);
        if (ret < 1) {
            ret = 0;
            goto err;
        }
    } else {
        qctx_maybe_autotick(&ctx);
    }

    ret = ossl_quic_channel_is_terminated(ctx.qc->ch);
err:
    qctx_unlock(&ctx);
    return ret;
}

/* SSL_ctrl */
long ossl_quic_ctrl(SSL *s, int cmd, long larg, void *parg)
{
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    switch (cmd) {
    case SSL_CTRL_MODE:
        if (ctx.is_listener)
            return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);

        /* If called on a QCSO, update the default mode. */
        if (!ctx.is_stream)
            ctx.qc->default_ssl_mode |= (uint32_t)larg;

        /*
         * If we were called on a QSSO or have a default stream, we also update
         * that.
         */
        if (ctx.xso != NULL) {
            /* Cannot enable EPW while AON write in progress. */
            if (ctx.xso->aon_write_in_progress)
                larg &= ~SSL_MODE_ENABLE_PARTIAL_WRITE;

            ctx.xso->ssl_mode |= (uint32_t)larg;
            return ctx.xso->ssl_mode;
        }

        return ctx.qc->default_ssl_mode;
    case SSL_CTRL_CLEAR_MODE:
        if (ctx.is_listener)
            return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);

        if (!ctx.is_stream)
            ctx.qc->default_ssl_mode &= ~(uint32_t)larg;

        if (ctx.xso != NULL) {
            ctx.xso->ssl_mode &= ~(uint32_t)larg;
            return ctx.xso->ssl_mode;
        }

        return ctx.qc->default_ssl_mode;

    case SSL_CTRL_SET_MSG_CALLBACK_ARG:
        if (ctx.is_listener)
            return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);

        ossl_quic_channel_set_msg_callback_arg(ctx.qc->ch, parg);
        /* This ctrl also needs to be passed to the internal SSL object */
        return SSL_ctrl(ctx.qc->tls, cmd, larg, parg);

    case DTLS_CTRL_GET_TIMEOUT: /* DTLSv1_get_timeout */
        {
            int is_infinite;

            if (!ossl_quic_get_event_timeout(s, parg, &is_infinite))
                return 0;

            return !is_infinite;
        }
    case DTLS_CTRL_HANDLE_TIMEOUT: /* DTLSv1_handle_timeout */
        /* For legacy compatibility with DTLS calls. */
        return ossl_quic_handle_events(s) == 1 ? 1 : -1;

        /* Mask ctrls we shouldn't support for QUIC. */
    case SSL_CTRL_GET_READ_AHEAD:
    case SSL_CTRL_SET_READ_AHEAD:
    case SSL_CTRL_SET_MAX_SEND_FRAGMENT:
    case SSL_CTRL_SET_SPLIT_SEND_FRAGMENT:
    case SSL_CTRL_SET_MAX_PIPELINES:
        return 0;

    default:
        /*
         * Probably a TLS related ctrl. Send back to the frontend SSL_ctrl
         * implementation. Either SSL_ctrl will handle it itself by direct
         * access into handshake layer state, or failing that, it will be passed
         * to the handshake layer via the SSL_METHOD vtable. If the ctrl is not
         * supported by anything, the handshake layer's ctrl method will finally
         * return 0.
         */
        if (ctx.is_listener)
            return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);

        return ossl_ctrl_internal(&ctx.qc->obj.ssl, cmd, larg, parg, /*no_quic=*/1);
    }
}

/* SSL_set_connect_state */
int ossl_quic_set_connect_state(SSL *s, int raiseerrs)
{
    QCTX ctx;

    if (!is_quic_c(s, &ctx, raiseerrs))
        return 0;

    if (ctx.qc->as_server_state == 0)
        return 1;

    /* Cannot be changed after handshake started */
    if (ctx.qc->started) {
        if (raiseerrs)
            QUIC_RAISE_NON_NORMAL_ERROR(NULL, SSL_R_INVALID_COMMAND, NULL);
        return 0;
    }

    ctx.qc->as_server_state = 0;
    return 1;
}

/* SSL_set_accept_state */
int ossl_quic_set_accept_state(SSL *s, int raiseerrs)
{
    QCTX ctx;

    if (!is_quic_c(s, &ctx, raiseerrs))
        return 0;

    if (ctx.qc->as_server_state == 1)
        return 1;

    /* Cannot be changed after handshake started */
    if (ctx.qc->started) {
        if (raiseerrs)
            QUIC_RAISE_NON_NORMAL_ERROR(NULL, SSL_R_INVALID_COMMAND, NULL);
        return 0;
    }

    ctx.qc->as_server_state = 1;
    return 1;
}

/* SSL_do_handshake */
struct quic_handshake_wait_args {
    QUIC_CONNECTION     *qc;
};

static int tls_wants_non_io_retry(QUIC_CONNECTION *qc)
{
    int want = SSL_want(qc->tls);

    if (want == SSL_X509_LOOKUP
            || want == SSL_CLIENT_HELLO_CB
            || want == SSL_RETRY_VERIFY)
        return 1;

    return 0;
}

static int quic_handshake_wait(void *arg)
{
    struct quic_handshake_wait_args *args = arg;

    if (!quic_mutation_allowed(args->qc, /*req_active=*/1))
        return -1;

    if (ossl_quic_channel_is_handshake_complete(args->qc->ch))
        return 1;

    if (tls_wants_non_io_retry(args->qc))
        return 1;

    return 0;
}

static int configure_channel(QUIC_CONNECTION *qc)
{
    assert(qc->ch != NULL);

    if (!ossl_quic_channel_set_peer_addr(qc->ch, &qc->init_peer_addr))
        return 0;

    return 1;
}

static int need_notifier_for_domain_flags(uint64_t domain_flags)
{
    return (domain_flags & SSL_DOMAIN_FLAG_THREAD_ASSISTED) != 0
        || ((domain_flags & SSL_DOMAIN_FLAG_MULTI_THREAD) != 0
            && (domain_flags & SSL_DOMAIN_FLAG_BLOCKING) != 0);
}

QUIC_NEEDS_LOCK
static int create_channel(QUIC_CONNECTION *qc, SSL_CTX *ctx)
{
    QUIC_ENGINE_ARGS engine_args = {0};
    QUIC_PORT_ARGS port_args = {0};

    engine_args.libctx        = ctx->libctx;
    engine_args.propq         = ctx->propq;
#if defined(OPENSSL_THREADS)
    engine_args.mutex         = qc->mutex;
#endif

    if (need_notifier_for_domain_flags(ctx->domain_flags))
        engine_args.reactor_flags |= QUIC_REACTOR_FLAG_USE_NOTIFIER;

    qc->engine = ossl_quic_engine_new(&engine_args);
    if (qc->engine == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        return 0;
    }

    port_args.channel_ctx = ctx;
    qc->port = ossl_quic_engine_create_port(qc->engine, &port_args);
    if (qc->port == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        ossl_quic_engine_free(qc->engine);
        return 0;
    }

    qc->ch = ossl_quic_port_create_outgoing(qc->port, qc->tls);
    if (qc->ch == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        ossl_quic_port_free(qc->port);
        ossl_quic_engine_free(qc->engine);
        return 0;
    }

    return 1;
}

/*
 * Configures a channel with the information we have accumulated via calls made
 * to us from the application prior to starting a handshake attempt.
 */
QUIC_NEEDS_LOCK
static int ensure_channel_started(QCTX *ctx)
{
    QUIC_CONNECTION *qc = ctx->qc;

    if (!qc->started) {
        if (!configure_channel(qc)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR,
                                        "failed to configure channel");
            return 0;
        }

        if (!ossl_quic_channel_start(qc->ch)) {
            ossl_quic_channel_restore_err_state(qc->ch);
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR,
                                        "failed to start channel");
            return 0;
        }

#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
        if (qc->is_thread_assisted)
            if (!ossl_quic_thread_assist_init_start(&qc->thread_assist, qc->ch)) {
                QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR,
                                            "failed to start assist thread");
                return 0;
            }
#endif
    }

    qc->started = 1;
    return 1;
}

QUIC_NEEDS_LOCK
static int quic_do_handshake(QCTX *ctx)
{
    int ret;
    QUIC_CONNECTION *qc = ctx->qc;
    QUIC_PORT *port;
    BIO *net_rbio, *net_wbio;

    if (ossl_quic_channel_is_handshake_complete(qc->ch))
        /* Handshake already completed. */
        return 1;

    if (!quic_mutation_allowed(qc, /*req_active=*/0))
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);

    if (qc->as_server != qc->as_server_state) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_PASSED_INVALID_ARGUMENT, NULL);
        return -1; /* Non-protocol error */
    }

    port = ossl_quic_obj_get0_port(ctx->obj);
    net_rbio = ossl_quic_port_get_net_rbio(port);
    net_wbio = ossl_quic_port_get_net_wbio(port);
    if (net_rbio == NULL || net_wbio == NULL) {
        /* Need read and write BIOs. */
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_BIO_NOT_SET, NULL);
        return -1; /* Non-protocol error */
    }

    if (!qc->started && ossl_quic_port_is_addressed_w(port)
        && BIO_ADDR_family(&qc->init_peer_addr) == AF_UNSPEC) {
        /*
         * We are trying to connect and are using addressed mode, which means we
         * need an initial peer address; if we do not have a peer address yet,
         * we should try to autodetect one.
         *
         * We do this as late as possible because some BIOs (e.g. BIO_s_connect)
         * may not be able to provide us with a peer address until they have
         * finished their own processing. They may not be able to perform this
         * processing until an application has finished configuring that BIO
         * (e.g. with setter calls), which might happen after SSL_set_bio is
         * called.
         */
        if (!csm_analyse_init_peer_addr(net_wbio, &qc->init_peer_addr))
            /* best effort */
            BIO_ADDR_clear(&qc->init_peer_addr);
        else
            ossl_quic_channel_set_peer_addr(qc->ch, &qc->init_peer_addr);
    }

    if (!qc->started
        && ossl_quic_port_is_addressed_w(port)
        && BIO_ADDR_family(&qc->init_peer_addr) == AF_UNSPEC) {
        /*
         * If we still don't have a peer address in addressed mode, we can't do
         * anything.
         */
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_REMOTE_PEER_ADDRESS_NOT_SET, NULL);
        return -1; /* Non-protocol error */
    }

    /*
     * Start connection process. Note we may come here multiple times in
     * non-blocking mode, which is fine.
     */
    if (!ensure_channel_started(ctx)) /* raises on failure */
        return -1; /* Non-protocol error */

    if (ossl_quic_channel_is_handshake_complete(qc->ch))
        /* The handshake is now done. */
        return 1;

    if (!qctx_blocking(ctx)) {
        /* Try to advance the reactor. */
        qctx_maybe_autotick(ctx);

        if (ossl_quic_channel_is_handshake_complete(qc->ch))
            /* The handshake is now done. */
            return 1;

        if (ossl_quic_channel_is_term_any(qc->ch)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
            return 0;
        } else if (ossl_quic_obj_desires_blocking(&qc->obj)) {
            /*
             * As a special case when doing a handshake when blocking mode is
             * desired yet not available, see if the network BIOs have become
             * poll descriptor-enabled. This supports BIOs such as BIO_s_connect
             * which do late creation of socket FDs and therefore cannot expose
             * a poll descriptor until after a network BIO is set on the QCSO.
             */
            ossl_quic_engine_update_poll_descriptors(qc->obj.engine, /*force=*/1);
        }
    }

    /*
     * We are either in blocking mode or just entered it due to the code above.
     */
    if (qctx_blocking(ctx)) {
        /* In blocking mode, wait for the handshake to complete. */
        struct quic_handshake_wait_args args;

        args.qc     = qc;

        ret = block_until_pred(ctx, quic_handshake_wait, &args, 0);
        if (!quic_mutation_allowed(qc, /*req_active=*/1)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
            return 0; /* Shutdown before completion */
        } else if (ret <= 0) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
            return -1; /* Non-protocol error */
        }

        if (tls_wants_non_io_retry(qc)) {
            QUIC_RAISE_NORMAL_ERROR(ctx, SSL_get_error(qc->tls, 0));
            return -1;
        }

        assert(ossl_quic_channel_is_handshake_complete(qc->ch));
        return 1;
    }

    if (tls_wants_non_io_retry(qc)) {
        QUIC_RAISE_NORMAL_ERROR(ctx, SSL_get_error(qc->tls, 0));
        return -1;
    }

    /*
     * Otherwise, indicate that the handshake isn't done yet.
     * We can only get here in non-blocking mode.
     */
    QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_WANT_READ);
    return -1; /* Non-protocol error */
}

QUIC_TAKES_LOCK
int ossl_quic_do_handshake(SSL *s)
{
    int ret;
    QCTX ctx;

    if (!expect_quic_cs(s, &ctx))
        return 0;

    qctx_lock_for_io(&ctx);

    ret = quic_do_handshake(&ctx);
    qctx_unlock(&ctx);
    return ret;
}

/* SSL_connect */
int ossl_quic_connect(SSL *s)
{
    /* Ensure we are in connect state (no-op if non-idle). */
    if (!ossl_quic_set_connect_state(s, 1))
        return -1;

    /* Begin or continue the handshake */
    return ossl_quic_do_handshake(s);
}

/* SSL_accept */
int ossl_quic_accept(SSL *s)
{
    /* Ensure we are in accept state (no-op if non-idle). */
    if (!ossl_quic_set_accept_state(s, 1))
        return -1;

    /* Begin or continue the handshake */
    return ossl_quic_do_handshake(s);
}

/*
 * QUIC Front-End I/O API: Stream Lifecycle Operations
 * ===================================================
 *
 *         SSL_stream_new       => ossl_quic_conn_stream_new
 *
 */

/*
 * Try to create the default XSO if it doesn't already exist. Returns 1 if the
 * default XSO was created. Returns 0 if it was not (e.g. because it already
 * exists). Note that this is NOT an error condition.
 */
QUIC_NEEDS_LOCK
static int qc_try_create_default_xso_for_write(QCTX *ctx)
{
    uint64_t flags = 0;
    QUIC_CONNECTION *qc = ctx->qc;

    if (qc->default_xso_created
        || qc->default_stream_mode == SSL_DEFAULT_STREAM_MODE_NONE)
        /*
         * We only do this once. If the user detaches a previously created
         * default XSO we don't auto-create another one.
         */
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_NO_STREAM, NULL);

    /* Create a locally-initiated stream. */
    if (qc->default_stream_mode == SSL_DEFAULT_STREAM_MODE_AUTO_UNI)
        flags |= SSL_STREAM_FLAG_UNI;

    qc_set_default_xso(qc, (QUIC_XSO *)quic_conn_stream_new(ctx, flags,
                                                            /*needs_lock=*/0),
                       /*touch=*/0);
    if (qc->default_xso == NULL)
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);

    qc_touch_default_xso(qc);
    return 1;
}

struct quic_wait_for_stream_args {
    QUIC_CONNECTION *qc;
    QUIC_STREAM     *qs;
    QCTX            *ctx;
    uint64_t        expect_id;
};

QUIC_NEEDS_LOCK
static int quic_wait_for_stream(void *arg)
{
    struct quic_wait_for_stream_args *args = arg;

    if (!quic_mutation_allowed(args->qc, /*req_active=*/1)) {
        /* If connection is torn down due to an error while blocking, stop. */
        QUIC_RAISE_NON_NORMAL_ERROR(args->ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        return -1;
    }

    args->qs = ossl_quic_stream_map_get_by_id(ossl_quic_channel_get_qsm(args->qc->ch),
                                              args->expect_id | QUIC_STREAM_DIR_BIDI);
    if (args->qs == NULL)
        args->qs = ossl_quic_stream_map_get_by_id(ossl_quic_channel_get_qsm(args->qc->ch),
                                                  args->expect_id | QUIC_STREAM_DIR_UNI);

    if (args->qs != NULL)
        return 1; /* stream now exists */

    return 0; /* did not get a stream, keep trying */
}

QUIC_NEEDS_LOCK
static int qc_wait_for_default_xso_for_read(QCTX *ctx, int peek)
{
    /* Called on a QCSO and we don't currently have a default stream. */
    uint64_t expect_id;
    QUIC_CONNECTION *qc = ctx->qc;
    QUIC_STREAM *qs;
    int res;
    struct quic_wait_for_stream_args wargs;
    OSSL_RTT_INFO rtt_info;

    /*
     * If default stream functionality is disabled or we already detached
     * one, don't make another default stream and just fail.
     */
    if (qc->default_xso_created
        || qc->default_stream_mode == SSL_DEFAULT_STREAM_MODE_NONE)
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_NO_STREAM, NULL);

    /*
     * The peer may have opened a stream since we last ticked. So tick and
     * see if the stream with ordinal 0 (remote, bidi/uni based on stream
     * mode) exists yet. QUIC stream IDs must be allocated in order, so the
     * first stream created by a peer must have an ordinal of 0.
     */
    expect_id = qc->as_server
        ? QUIC_STREAM_INITIATOR_CLIENT
        : QUIC_STREAM_INITIATOR_SERVER;

    qs = ossl_quic_stream_map_get_by_id(ossl_quic_channel_get_qsm(qc->ch),
                                        expect_id | QUIC_STREAM_DIR_BIDI);
    if (qs == NULL)
        qs = ossl_quic_stream_map_get_by_id(ossl_quic_channel_get_qsm(qc->ch),
                                            expect_id | QUIC_STREAM_DIR_UNI);

    if (qs == NULL) {
        qctx_maybe_autotick(ctx);

        qs = ossl_quic_stream_map_get_by_id(ossl_quic_channel_get_qsm(qc->ch),
                                            expect_id);
    }

    if (qs == NULL) {
        if (peek)
            return 0;

        if (ossl_quic_channel_is_term_any(qc->ch)) {
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        } else if (!qctx_blocking(ctx)) {
            /* Non-blocking mode, so just bail immediately. */
            return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_WANT_READ);
        }

        /* Block until we have a stream. */
        wargs.qc        = qc;
        wargs.qs        = NULL;
        wargs.ctx       = ctx;
        wargs.expect_id = expect_id;

        res = block_until_pred(ctx, quic_wait_for_stream, &wargs, 0);
        if (res == 0)
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
        else if (res < 0 || wargs.qs == NULL)
            /* quic_wait_for_stream raised error here */
            return 0;

        qs = wargs.qs;
    }

    /*
     * We now have qs != NULL. Remove it from the incoming stream queue so that
     * it isn't also returned by any future SSL_accept_stream calls.
     */
    ossl_statm_get_rtt_info(ossl_quic_channel_get_statm(qc->ch), &rtt_info);
    ossl_quic_stream_map_remove_from_accept_queue(ossl_quic_channel_get_qsm(qc->ch),
                                                  qs, rtt_info.smoothed_rtt);

    /*
     * Now make qs the default stream, creating the necessary XSO.
     */
    qc_set_default_xso(qc, create_xso_from_stream(qc, qs), /*touch=*/0);
    if (qc->default_xso == NULL)
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);

    qc_touch_default_xso(qc); /* inhibits default XSO */
    return 1;
}

QUIC_NEEDS_LOCK
static QUIC_XSO *create_xso_from_stream(QUIC_CONNECTION *qc, QUIC_STREAM *qs)
{
    QUIC_XSO *xso = NULL;

    if ((xso = OPENSSL_zalloc(sizeof(*xso))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }

    if (!ossl_quic_obj_init(&xso->obj, qc->obj.ssl.ctx, SSL_TYPE_QUIC_XSO,
                            &qc->obj.ssl, NULL, NULL)) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* XSO refs QC */
    if (!SSL_up_ref(&qc->obj.ssl)) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_SSL_LIB, NULL);
        goto err;
    }

    xso->conn       = qc;
    xso->ssl_mode   = qc->default_ssl_mode;
    xso->ssl_options
        = qc->default_ssl_options & OSSL_QUIC_PERMITTED_OPTIONS_STREAM;
    xso->last_error = SSL_ERROR_NONE;

    xso->stream     = qs;

    ++qc->num_xso;
    xso_update_options(xso);
    return xso;

err:
    OPENSSL_free(xso);
    return NULL;
}

struct quic_new_stream_wait_args {
    QUIC_CONNECTION *qc;
    int is_uni;
};

static int quic_new_stream_wait(void *arg)
{
    struct quic_new_stream_wait_args *args = arg;
    QUIC_CONNECTION *qc = args->qc;

    if (!quic_mutation_allowed(qc, /*req_active=*/1))
        return -1;

    if (ossl_quic_channel_is_new_local_stream_admissible(qc->ch, args->is_uni))
        return 1;

    return 0;
}

/* locking depends on need_lock */
static SSL *quic_conn_stream_new(QCTX *ctx, uint64_t flags, int need_lock)
{
    int ret;
    QUIC_CONNECTION *qc = ctx->qc;
    QUIC_XSO *xso = NULL;
    QUIC_STREAM *qs = NULL;
    int is_uni = ((flags & SSL_STREAM_FLAG_UNI) != 0);
    int no_blocking = ((flags & SSL_STREAM_FLAG_NO_BLOCK) != 0);
    int advance = ((flags & SSL_STREAM_FLAG_ADVANCE) != 0);

    if (need_lock)
        qctx_lock(ctx);

    if (!quic_mutation_allowed(qc, /*req_active=*/0)) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        goto err;
    }

    if (!advance
        && !ossl_quic_channel_is_new_local_stream_admissible(qc->ch, is_uni)) {
        struct quic_new_stream_wait_args args;

        /*
         * Stream count flow control currently doesn't permit this stream to be
         * opened.
         */
        if (no_blocking || !qctx_blocking(ctx)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_STREAM_COUNT_LIMITED, NULL);
            goto err;
        }

        args.qc     = qc;
        args.is_uni = is_uni;

        /* Blocking mode - wait until we can get a stream. */
        ret = block_until_pred(ctx, quic_new_stream_wait, &args, 0);
        if (!quic_mutation_allowed(qc, /*req_active=*/1)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
            goto err; /* Shutdown before completion */
        } else if (ret <= 0) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
            goto err; /* Non-protocol error */
        }
    }

    qs = ossl_quic_channel_new_stream_local(qc->ch, is_uni);
    if (qs == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    xso = create_xso_from_stream(qc, qs);
    if (xso == NULL)
        goto err;

    qc_touch_default_xso(qc); /* inhibits default XSO */
    if (need_lock)
        qctx_unlock(ctx);

    return &xso->obj.ssl;

err:
    OPENSSL_free(xso);
    ossl_quic_stream_map_release(ossl_quic_channel_get_qsm(qc->ch), qs);
    if (need_lock)
        qctx_unlock(ctx);

    return NULL;

}

QUIC_TAKES_LOCK
SSL *ossl_quic_conn_stream_new(SSL *s, uint64_t flags)
{
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return NULL;

    return quic_conn_stream_new(&ctx, flags, /*need_lock=*/1);
}

/*
 * QUIC Front-End I/O API: Steady-State Operations
 * ===============================================
 *
 * Here we dispatch calls to the steady-state front-end I/O API functions; that
 * is, the functions used during the established phase of a QUIC connection
 * (e.g. SSL_read, SSL_write).
 *
 * Each function must handle both blocking and non-blocking modes. As discussed
 * above, all QUIC I/O is implemented using non-blocking mode internally.
 *
 *         SSL_get_error        => partially implemented by ossl_quic_get_error
 *         SSL_want             => ossl_quic_want
 *   (BIO/)SSL_read             => ossl_quic_read
 *   (BIO/)SSL_write            => ossl_quic_write
 *         SSL_pending          => ossl_quic_pending
 *         SSL_stream_conclude  => ossl_quic_conn_stream_conclude
 *         SSL_key_update       => ossl_quic_key_update
 */

/* SSL_get_error */
int ossl_quic_get_error(const SSL *s, int i)
{
    QCTX ctx;
    int net_error, last_error;

    /* SSL_get_errors() should not raise new errors */
    if (!is_quic_cs(s, &ctx, 0 /* suppress errors */))
        return SSL_ERROR_SSL;

    qctx_lock(&ctx);
    net_error = ossl_quic_channel_net_error(ctx.qc->ch);
    last_error = ctx.is_stream ? ctx.xso->last_error : ctx.qc->last_error;
    qctx_unlock(&ctx);

    if (net_error)
        return SSL_ERROR_SYSCALL;

    return last_error;
}

/* Converts a code returned by SSL_get_error to a code returned by SSL_want. */
static int error_to_want(int error)
{
    switch (error) {
    case SSL_ERROR_WANT_CONNECT: /* never used - UDP is connectionless */
    case SSL_ERROR_WANT_ACCEPT:  /* never used - UDP is connectionless */
    case SSL_ERROR_ZERO_RETURN:
    default:
        return SSL_NOTHING;

    case SSL_ERROR_WANT_READ:
        return SSL_READING;

    case SSL_ERROR_WANT_WRITE:
        return SSL_WRITING;

    case SSL_ERROR_WANT_RETRY_VERIFY:
        return SSL_RETRY_VERIFY;

    case SSL_ERROR_WANT_CLIENT_HELLO_CB:
        return SSL_CLIENT_HELLO_CB;

    case SSL_ERROR_WANT_X509_LOOKUP:
        return SSL_X509_LOOKUP;
    }
}

/* SSL_want */
int ossl_quic_want(const SSL *s)
{
    QCTX ctx;
    int w;

    if (!expect_quic_cs(s, &ctx))
        return SSL_NOTHING;

    qctx_lock(&ctx);

    w = error_to_want(ctx.is_stream ? ctx.xso->last_error : ctx.qc->last_error);

    qctx_unlock(&ctx);
    return w;
}

/*
 * SSL_write
 * ---------
 *
 * The set of functions below provide the implementation of the public SSL_write
 * function. We must handle:
 *
 *   - both blocking and non-blocking operation at the application level,
 *     depending on how we are configured;
 *
 *   - SSL_MODE_ENABLE_PARTIAL_WRITE being on or off;
 *
 *   - SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER.
 *
 */
QUIC_NEEDS_LOCK
static void quic_post_write(QUIC_XSO *xso, int did_append,
                            int did_append_all, uint64_t flags,
                            int do_tick)
{
    /*
     * We have appended at least one byte to the stream.
     * Potentially mark stream as active, depending on FC.
     */
    if (did_append)
        ossl_quic_stream_map_update_state(ossl_quic_channel_get_qsm(xso->conn->ch),
                                          xso->stream);

    if (did_append_all && (flags & SSL_WRITE_FLAG_CONCLUDE) != 0)
        ossl_quic_sstream_fin(xso->stream->sstream);

    /*
     * Try and send.
     *
     * TODO(QUIC FUTURE): It is probably inefficient to try and do this
     * immediately, plus we should eventually consider Nagle's algorithm.
     */
    if (do_tick)
        ossl_quic_reactor_tick(ossl_quic_channel_get_reactor(xso->conn->ch), 0);
}

struct quic_write_again_args {
    QUIC_XSO            *xso;
    const unsigned char *buf;
    size_t              len;
    size_t              total_written;
    int                 err;
    uint64_t            flags;
};

/*
 * Absolute maximum write buffer size, enforced to prevent a rogue peer from
 * deliberately inducing DoS. This has been chosen based on the optimal buffer
 * size for an RTT of 500ms and a bandwidth of 100 Mb/s.
 */
#define MAX_WRITE_BUF_SIZE      (6 * 1024 * 1024)

/*
 * Ensure spare buffer space available (up until a limit, at least).
 */
QUIC_NEEDS_LOCK
static int sstream_ensure_spare(QUIC_SSTREAM *sstream, uint64_t spare)
{
    size_t cur_sz = ossl_quic_sstream_get_buffer_size(sstream);
    size_t avail = ossl_quic_sstream_get_buffer_avail(sstream);
    size_t spare_ = (spare > SIZE_MAX) ? SIZE_MAX : (size_t)spare;
    size_t new_sz, growth;

    if (spare_ <= avail || cur_sz == MAX_WRITE_BUF_SIZE)
        return 1;

    growth = spare_ - avail;
    if (cur_sz + growth > MAX_WRITE_BUF_SIZE)
        new_sz = MAX_WRITE_BUF_SIZE;
    else
        new_sz = cur_sz + growth;

    return ossl_quic_sstream_set_buffer_size(sstream, new_sz);
}

/*
 * Append to a QUIC_STREAM's QUIC_SSTREAM, ensuring buffer space is expanded
 * as needed according to flow control.
 */
QUIC_NEEDS_LOCK
static int xso_sstream_append(QUIC_XSO *xso, const unsigned char *buf,
                              size_t len, size_t *actual_written)
{
    QUIC_SSTREAM *sstream = xso->stream->sstream;
    uint64_t cur = ossl_quic_sstream_get_cur_size(sstream);
    uint64_t cwm = ossl_quic_txfc_get_cwm(&xso->stream->txfc);
    uint64_t permitted = (cwm >= cur ? cwm - cur : 0);

    if (len > permitted)
        len = (size_t)permitted;

    if (!sstream_ensure_spare(sstream, len))
        return 0;

    return ossl_quic_sstream_append(sstream, buf, len, actual_written);
}

QUIC_NEEDS_LOCK
static int quic_write_again(void *arg)
{
    struct quic_write_again_args *args = arg;
    size_t actual_written = 0;

    if (!quic_mutation_allowed(args->xso->conn, /*req_active=*/1))
        /* If connection is torn down due to an error while blocking, stop. */
        return -2;

    if (!quic_validate_for_write(args->xso, &args->err))
        /*
         * Stream may have become invalid for write due to connection events
         * while we blocked.
         */
        return -2;

    args->err = ERR_R_INTERNAL_ERROR;
    if (!xso_sstream_append(args->xso, args->buf, args->len, &actual_written))
        return -2;

    quic_post_write(args->xso, actual_written > 0,
                    args->len == actual_written, args->flags, 0);

    args->buf           += actual_written;
    args->len           -= actual_written;
    args->total_written += actual_written;

    if (args->len == 0)
        /* Written everything, done. */
        return 1;

    /* Not written everything yet, keep trying. */
    return 0;
}

QUIC_NEEDS_LOCK
static int quic_write_blocking(QCTX *ctx, const void *buf, size_t len,
                               uint64_t flags, size_t *written)
{
    int res;
    QUIC_XSO *xso = ctx->xso;
    struct quic_write_again_args args;
    size_t actual_written = 0;

    /* First make a best effort to append as much of the data as possible. */
    if (!xso_sstream_append(xso, buf, len, &actual_written)) {
        /* Stream already finished or allocation error. */
        *written = 0;
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
    }

    quic_post_write(xso, actual_written > 0, actual_written == len, flags, 1);

    /*
     * Record however much data we wrote
     */
    *written = actual_written;

    if (actual_written == len) {
        /* Managed to append everything on the first try. */
        return 1;
    }

    /*
     * We did not manage to append all of the data immediately, so the stream
     * buffer has probably filled up. This means we need to block until some of
     * it is freed up.
     */
    args.xso            = xso;
    args.buf            = (const unsigned char *)buf + actual_written;
    args.len            = len - actual_written;
    args.total_written  = 0;
    args.err            = ERR_R_INTERNAL_ERROR;
    args.flags          = flags;

    res = block_until_pred(ctx, quic_write_again, &args, 0);
    if (res <= 0) {
        if (!quic_mutation_allowed(xso->conn, /*req_active=*/1))
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        else
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, args.err, NULL);
    }

    /*
     * When waiting on extra buffer space to be available, args.total_written
     * holds the amount of remaining data we requested to write, which will be
     * something less than the len parameter passed in, however much we wrote
     * here, add it to the value that we wrote when we initially called
     * xso_sstream_append
     */
    *written += args.total_written;
    return 1;
}

/*
 * Functions to manage All-or-Nothing (AON) (that is, non-ENABLE_PARTIAL_WRITE)
 * write semantics.
 */
static void aon_write_begin(QUIC_XSO *xso, const unsigned char *buf,
                            size_t buf_len, size_t already_sent)
{
    assert(!xso->aon_write_in_progress);

    xso->aon_write_in_progress = 1;
    xso->aon_buf_base          = buf;
    xso->aon_buf_pos           = already_sent;
    xso->aon_buf_len           = buf_len;
}

static void aon_write_finish(QUIC_XSO *xso)
{
    xso->aon_write_in_progress   = 0;
    xso->aon_buf_base            = NULL;
    xso->aon_buf_pos             = 0;
    xso->aon_buf_len             = 0;
}

QUIC_NEEDS_LOCK
static int quic_write_nonblocking_aon(QCTX *ctx, const void *buf,
                                      size_t len, uint64_t flags,
                                      size_t *written)
{
    QUIC_XSO *xso = ctx->xso;
    const void *actual_buf;
    size_t actual_len, actual_written = 0;
    int accept_moving_buffer
        = ((xso->ssl_mode & SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER) != 0);

    if (xso->aon_write_in_progress) {
        /*
         * We are in the middle of an AON write (i.e., a previous write did not
         * manage to append all data to the SSTREAM and we have Enable Partial
         * Write (EPW) mode disabled.)
         */
        if ((!accept_moving_buffer && xso->aon_buf_base != buf)
            || len != xso->aon_buf_len)
            /*
             * Pointer must not have changed if we are not in accept moving
             * buffer mode. Length must never change.
             */
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_BAD_WRITE_RETRY, NULL);

        actual_buf = (unsigned char *)buf + xso->aon_buf_pos;
        actual_len = len - xso->aon_buf_pos;
        assert(actual_len > 0);
    } else {
        actual_buf = buf;
        actual_len = len;
    }

    /* First make a best effort to append as much of the data as possible. */
    if (!xso_sstream_append(xso, actual_buf, actual_len, &actual_written)) {
        /* Stream already finished or allocation error. */
        *written = 0;
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
    }

    quic_post_write(xso, actual_written > 0, actual_written == actual_len,
                    flags, qctx_should_autotick(ctx));

    if (actual_written == actual_len) {
        /* We have sent everything. */
        if (xso->aon_write_in_progress) {
            /*
             * We have sent everything, and we were in the middle of an AON
             * write. The output write length is the total length of the AON
             * buffer, not however many bytes we managed to write to the stream
             * in this call.
             */
            *written = xso->aon_buf_len;
            aon_write_finish(xso);
        } else {
            *written = actual_written;
        }

        return 1;
    }

    if (xso->aon_write_in_progress) {
        /*
         * AON write is in progress but we have not written everything yet. We
         * may have managed to send zero bytes, or some number of bytes less
         * than the total remaining which need to be appended during this
         * AON operation.
         */
        xso->aon_buf_pos += actual_written;
        assert(xso->aon_buf_pos < xso->aon_buf_len);
        return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_WANT_WRITE);
    }

    /*
     * Not in an existing AON operation but partial write is not enabled, so we
     * need to begin a new AON operation. However we needn't bother if we didn't
     * actually append anything.
     */
    if (actual_written > 0)
        aon_write_begin(xso, buf, len, actual_written);

    /*
     * AON - We do not publicly admit to having appended anything until AON
     * completes.
     */
    *written = 0;
    return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_WANT_WRITE);
}

QUIC_NEEDS_LOCK
static int quic_write_nonblocking_epw(QCTX *ctx, const void *buf, size_t len,
                                      uint64_t flags, size_t *written)
{
    QUIC_XSO *xso = ctx->xso;

    /* Simple best effort operation. */
    if (!xso_sstream_append(xso, buf, len, written)) {
        /* Stream already finished or allocation error. */
        *written = 0;
        return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
    }

    quic_post_write(xso, *written > 0, *written == len, flags,
                    qctx_should_autotick(ctx));

    if (*written == 0)
        /* SSL_write_ex returns 0 if it didn't write anything. */
        return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_WANT_WRITE);

    return 1;
}

QUIC_NEEDS_LOCK
static int quic_validate_for_write(QUIC_XSO *xso, int *err)
{
    QUIC_STREAM_MAP *qsm;

    if (xso == NULL || xso->stream == NULL) {
        *err = ERR_R_INTERNAL_ERROR;
        return 0;
    }

    switch (xso->stream->send_state) {
    default:
    case QUIC_SSTREAM_STATE_NONE:
        *err = SSL_R_STREAM_RECV_ONLY;
        return 0;

    case QUIC_SSTREAM_STATE_READY:
        qsm = ossl_quic_channel_get_qsm(xso->conn->ch);

        if (!ossl_quic_stream_map_ensure_send_part_id(qsm, xso->stream)) {
            *err = ERR_R_INTERNAL_ERROR;
            return 0;
        }

        /* FALLTHROUGH */
    case QUIC_SSTREAM_STATE_SEND:
    case QUIC_SSTREAM_STATE_DATA_SENT:
        if (ossl_quic_sstream_get_final_size(xso->stream->sstream, NULL)) {
            *err = SSL_R_STREAM_FINISHED;
            return 0;
        }
        return 1;

    case QUIC_SSTREAM_STATE_DATA_RECVD:
        *err = SSL_R_STREAM_FINISHED;
        return 0;

    case QUIC_SSTREAM_STATE_RESET_SENT:
    case QUIC_SSTREAM_STATE_RESET_RECVD:
        *err = SSL_R_STREAM_RESET;
        return 0;
    }
}

QUIC_TAKES_LOCK
int ossl_quic_write_flags(SSL *s, const void *buf, size_t len,
                          uint64_t flags, size_t *written)
{
    int ret;
    QCTX ctx;
    int partial_write, err;

    *written = 0;

    if (len == 0) {
        /* Do not autocreate default XSO for zero-length writes. */
        if (!expect_quic_cs(s, &ctx))
            return 0;

        qctx_lock_for_io(&ctx);
    } else {
        if (!expect_quic_with_stream_lock(s, /*remote_init=*/0, /*io=*/1, &ctx))
            return 0;
    }

    partial_write = ((ctx.xso != NULL)
        ? ((ctx.xso->ssl_mode & SSL_MODE_ENABLE_PARTIAL_WRITE) != 0) : 0);

    if ((flags & ~SSL_WRITE_FLAG_CONCLUDE) != 0) {
        ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_UNSUPPORTED_WRITE_FLAG, NULL);
        goto out;
    }

    if (!quic_mutation_allowed(ctx.qc, /*req_active=*/0)) {
        ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        goto out;
    }

    /*
     * If we haven't finished the handshake, try to advance it.
     * We don't accept writes until the handshake is completed.
     */
    if (quic_do_handshake(&ctx) < 1) {
        ret = 0;
        goto out;
    }

    /* Ensure correct stream state, stream send part not concluded, etc. */
    if (len > 0 && !quic_validate_for_write(ctx.xso, &err)) {
        ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, err, NULL);
        goto out;
    }

    if (len == 0) {
        if ((flags & SSL_WRITE_FLAG_CONCLUDE) != 0)
            quic_post_write(ctx.xso, 0, 1, flags,
                            qctx_should_autotick(&ctx));

        ret = 1;
        goto out;
    }

    if (qctx_blocking(&ctx))
        ret = quic_write_blocking(&ctx, buf, len, flags, written);
    else if (partial_write)
        ret = quic_write_nonblocking_epw(&ctx, buf, len, flags, written);
    else
        ret = quic_write_nonblocking_aon(&ctx, buf, len, flags, written);

out:
    qctx_unlock(&ctx);
    return ret;
}

QUIC_TAKES_LOCK
int ossl_quic_write(SSL *s, const void *buf, size_t len, size_t *written)
{
    return ossl_quic_write_flags(s, buf, len, 0, written);
}

/*
 * SSL_read
 * --------
 */
struct quic_read_again_args {
    QCTX            *ctx;
    QUIC_STREAM     *stream;
    void            *buf;
    size_t          len;
    size_t          *bytes_read;
    int             peek;
};

QUIC_NEEDS_LOCK
static int quic_validate_for_read(QUIC_XSO *xso, int *err, int *eos)
{
    QUIC_STREAM_MAP *qsm;

    *eos = 0;

    if (xso == NULL || xso->stream == NULL) {
        *err = ERR_R_INTERNAL_ERROR;
        return 0;
    }

    switch (xso->stream->recv_state) {
    default:
    case QUIC_RSTREAM_STATE_NONE:
        *err = SSL_R_STREAM_SEND_ONLY;
        return 0;

    case QUIC_RSTREAM_STATE_RECV:
    case QUIC_RSTREAM_STATE_SIZE_KNOWN:
    case QUIC_RSTREAM_STATE_DATA_RECVD:
        return 1;

    case QUIC_RSTREAM_STATE_DATA_READ:
        *eos = 1;
        return 0;

    case QUIC_RSTREAM_STATE_RESET_RECVD:
        qsm = ossl_quic_channel_get_qsm(xso->conn->ch);
        ossl_quic_stream_map_notify_app_read_reset_recv_part(qsm, xso->stream);

        /* FALLTHROUGH */
    case QUIC_RSTREAM_STATE_RESET_READ:
        *err = SSL_R_STREAM_RESET;
        return 0;
    }
}

QUIC_NEEDS_LOCK
static int quic_read_actual(QCTX *ctx,
                            QUIC_STREAM *stream,
                            void *buf, size_t buf_len,
                            size_t *bytes_read,
                            int peek)
{
    int is_fin = 0, err, eos;
    QUIC_CONNECTION *qc = ctx->qc;

    if (!quic_validate_for_read(ctx->xso, &err, &eos)) {
        if (eos) {
            ctx->xso->retired_fin = 1;
            return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_ZERO_RETURN);
        } else {
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, err, NULL);
        }
    }

    if (peek) {
        if (!ossl_quic_rstream_peek(stream->rstream, buf, buf_len,
                                    bytes_read, &is_fin))
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);

    } else {
        if (!ossl_quic_rstream_read(stream->rstream, buf, buf_len,
                                    bytes_read, &is_fin))
            return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
    }

    if (!peek) {
        if (*bytes_read > 0) {
            /*
             * We have read at least one byte from the stream. Inform stream-level
             * RXFC of the retirement of controlled bytes. Update the active stream
             * status (the RXFC may now want to emit a frame granting more credit to
             * the peer).
             */
            OSSL_RTT_INFO rtt_info;

            ossl_statm_get_rtt_info(ossl_quic_channel_get_statm(qc->ch), &rtt_info);

            if (!ossl_quic_rxfc_on_retire(&stream->rxfc, *bytes_read,
                                          rtt_info.smoothed_rtt))
                return QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_INTERNAL_ERROR, NULL);
        }

        if (is_fin && !peek) {
            QUIC_STREAM_MAP *qsm = ossl_quic_channel_get_qsm(ctx->qc->ch);

            ossl_quic_stream_map_notify_totally_read(qsm, ctx->xso->stream);
        }

        if (*bytes_read > 0)
            ossl_quic_stream_map_update_state(ossl_quic_channel_get_qsm(qc->ch),
                                              stream);
    }

    if (*bytes_read == 0 && is_fin) {
        ctx->xso->retired_fin = 1;
        return QUIC_RAISE_NORMAL_ERROR(ctx, SSL_ERROR_ZERO_RETURN);
    }

    return 1;
}

QUIC_NEEDS_LOCK
static int quic_read_again(void *arg)
{
    struct quic_read_again_args *args = arg;

    if (!quic_mutation_allowed(args->ctx->qc, /*req_active=*/1)) {
        /* If connection is torn down due to an error while blocking, stop. */
        QUIC_RAISE_NON_NORMAL_ERROR(args->ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        return -1;
    }

    if (!quic_read_actual(args->ctx, args->stream,
                          args->buf, args->len, args->bytes_read,
                          args->peek))
        return -1;

    if (*args->bytes_read > 0)
        /* got at least one byte, the SSL_read op can finish now */
        return 1;

    return 0; /* did not read anything, keep trying */
}

QUIC_TAKES_LOCK
static int quic_read(SSL *s, void *buf, size_t len, size_t *bytes_read, int peek)
{
    int ret, res;
    QCTX ctx;
    struct quic_read_again_args args;

    *bytes_read = 0;

    if (!expect_quic_cs(s, &ctx))
        return 0;

    qctx_lock_for_io(&ctx);

    /* If we haven't finished the handshake, try to advance it. */
    if (quic_do_handshake(&ctx) < 1) {
        ret = 0; /* ossl_quic_do_handshake raised error here */
        goto out;
    }

    if (ctx.xso == NULL) {
        /*
         * Called on a QCSO and we don't currently have a default stream.
         *
         * Wait until we get a stream initiated by the peer (blocking mode) or
         * fail if we don't have one yet (non-blocking mode).
         */
        if (!qc_wait_for_default_xso_for_read(&ctx, /*peek=*/0)) {
            ret = 0; /* error already raised here */
            goto out;
        }

        ctx.xso = ctx.qc->default_xso;
    }

    if (!quic_read_actual(&ctx, ctx.xso->stream, buf, len, bytes_read, peek)) {
        ret = 0; /* quic_read_actual raised error here */
        goto out;
    }

    if (*bytes_read > 0) {
        /*
         * Even though we succeeded, tick the reactor here to ensure we are
         * handling other aspects of the QUIC connection.
         */
        if (quic_mutation_allowed(ctx.qc, /*req_active=*/0))
            qctx_maybe_autotick(&ctx);

        ret = 1;
    } else if (!quic_mutation_allowed(ctx.qc, /*req_active=*/0)) {
        ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        goto out;
    } else if (qctx_blocking(&ctx)) {
        /*
         * We were not able to read anything immediately, so our stream
         * buffer is empty. This means we need to block until we get
         * at least one byte.
         */
        args.ctx        = &ctx;
        args.stream     = ctx.xso->stream;
        args.buf        = buf;
        args.len        = len;
        args.bytes_read = bytes_read;
        args.peek       = peek;

        res = block_until_pred(&ctx, quic_read_again, &args, 0);
        if (res == 0) {
            ret = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_INTERNAL_ERROR, NULL);
            goto out;
        } else if (res < 0) {
            ret = 0; /* quic_read_again raised error here */
            goto out;
        }

        ret = 1;
    } else {
        /*
         * We did not get any bytes and are not in blocking mode.
         * Tick to see if this delivers any more.
         */
        qctx_maybe_autotick(&ctx);

        /* Try the read again. */
        if (!quic_read_actual(&ctx, ctx.xso->stream, buf, len, bytes_read, peek)) {
            ret = 0; /* quic_read_actual raised error here */
            goto out;
        }

        if (*bytes_read > 0)
            ret = 1; /* Succeeded this time. */
        else
            ret = QUIC_RAISE_NORMAL_ERROR(&ctx, SSL_ERROR_WANT_READ);
    }

out:
    qctx_unlock(&ctx);
    return ret;
}

int ossl_quic_read(SSL *s, void *buf, size_t len, size_t *bytes_read)
{
    return quic_read(s, buf, len, bytes_read, 0);
}

int ossl_quic_peek(SSL *s, void *buf, size_t len, size_t *bytes_read)
{
    return quic_read(s, buf, len, bytes_read, 1);
}

/*
 * SSL_pending
 * -----------
 */

QUIC_TAKES_LOCK
static size_t ossl_quic_pending_int(const SSL *s, int check_channel)
{
    QCTX ctx;
    size_t avail = 0;

    if (!expect_quic_cs(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    if (!ctx.qc->started)
        goto out;

    if (ctx.xso == NULL) {
        /* No XSO yet, but there might be a default XSO eligible to be created. */
        if (qc_wait_for_default_xso_for_read(&ctx, /*peek=*/1)) {
            ctx.xso = ctx.qc->default_xso;
        } else {
            QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_NO_STREAM, NULL);
            goto out;
        }
    }

    if (ctx.xso->stream == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_INTERNAL_ERROR, NULL);
        goto out;
    }

    if (check_channel)
        avail = ossl_quic_stream_recv_pending(ctx.xso->stream,
                                              /*include_fin=*/1)
             || ossl_quic_channel_has_pending(ctx.qc->ch)
             || ossl_quic_channel_is_term_any(ctx.qc->ch);
    else
        avail = ossl_quic_stream_recv_pending(ctx.xso->stream,
                                              /*include_fin=*/0);

out:
    qctx_unlock(&ctx);
    return avail;
}

size_t ossl_quic_pending(const SSL *s)
{
    return ossl_quic_pending_int(s, /*check_channel=*/0);
}

int ossl_quic_has_pending(const SSL *s)
{
    /* Do we have app-side pending data or pending URXEs or RXEs? */
    return ossl_quic_pending_int(s, /*check_channel=*/1) > 0;
}

/*
 * SSL_stream_conclude
 * -------------------
 */
QUIC_TAKES_LOCK
int ossl_quic_conn_stream_conclude(SSL *s)
{
    QCTX ctx;
    QUIC_STREAM *qs;
    int err;

    if (!expect_quic_with_stream_lock(s, /*remote_init=*/0, /*io=*/0, &ctx))
        return 0;

    qs = ctx.xso->stream;

    if (!quic_mutation_allowed(ctx.qc, /*req_active=*/1)) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
    }

    if (!quic_validate_for_write(ctx.xso, &err)) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, err, NULL);
    }

    if (ossl_quic_sstream_get_final_size(qs->sstream, NULL)) {
        qctx_unlock(&ctx);
        return 1;
    }

    ossl_quic_sstream_fin(qs->sstream);
    quic_post_write(ctx.xso, 1, 0, 0, qctx_should_autotick(&ctx));
    qctx_unlock(&ctx);
    return 1;
}

/*
 * SSL_inject_net_dgram
 * --------------------
 */
QUIC_TAKES_LOCK
int SSL_inject_net_dgram(SSL *s, const unsigned char *buf,
                         size_t buf_len,
                         const BIO_ADDR *peer,
                         const BIO_ADDR *local)
{
    int ret = 0;
    QCTX ctx;
    QUIC_DEMUX *demux;
    QUIC_PORT *port;

    if (!expect_quic_csl(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    port = ossl_quic_obj_get0_port(ctx.obj);
    if (port == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_UNSUPPORTED, NULL);
        goto err;
    }

    demux = ossl_quic_port_get0_demux(port);
    ret = ossl_quic_demux_inject(demux, buf, buf_len, peer, local);

err:
    qctx_unlock(&ctx);
    return ret;
}

/*
 * SSL_get0_connection
 * -------------------
 */
SSL *ossl_quic_get0_connection(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_cs(s, &ctx))
        return NULL;

    return &ctx.qc->obj.ssl;
}

/*
 * SSL_get0_listener
 * -----------------
 */
SSL *ossl_quic_get0_listener(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_csl(s, &ctx))
        return NULL;

    return ctx.ql != NULL ? &ctx.ql->obj.ssl : NULL;
}

/*
 * SSL_get0_domain
 * ---------------
 */
SSL *ossl_quic_get0_domain(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_any(s, &ctx))
        return NULL;

    return ctx.qd != NULL ? &ctx.qd->obj.ssl : NULL;
}

/*
 * SSL_get_domain_flags
 * --------------------
 */
int ossl_quic_get_domain_flags(const SSL *ssl, uint64_t *domain_flags)
{
    QCTX ctx;

    if (!expect_quic_any(ssl, &ctx))
        return 0;

    if (domain_flags != NULL)
        *domain_flags = ctx.obj->domain_flags;

    return 1;
}

/*
 * SSL_get_stream_type
 * -------------------
 */
int ossl_quic_get_stream_type(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_cs(s, &ctx))
        return SSL_STREAM_TYPE_BIDI;

    if (ctx.xso == NULL) {
        /*
         * If deferred XSO creation has yet to occur, proceed according to the
         * default stream mode. If AUTO_BIDI or AUTO_UNI is set, we cannot know
         * what kind of stream will be created yet, so return BIDI on the basis
         * that at this time, the client still has the option of calling
         * SSL_read() or SSL_write() first.
         */
        if (ctx.qc->default_xso_created
            || ctx.qc->default_stream_mode == SSL_DEFAULT_STREAM_MODE_NONE)
            return SSL_STREAM_TYPE_NONE;
        else
            return SSL_STREAM_TYPE_BIDI;
    }

    if (ossl_quic_stream_is_bidi(ctx.xso->stream))
        return SSL_STREAM_TYPE_BIDI;

    if (ossl_quic_stream_is_server_init(ctx.xso->stream) != ctx.qc->as_server)
        return SSL_STREAM_TYPE_READ;
    else
        return SSL_STREAM_TYPE_WRITE;
}

/*
 * SSL_get_stream_id
 * -----------------
 */
QUIC_TAKES_LOCK
uint64_t ossl_quic_get_stream_id(SSL *s)
{
    QCTX ctx;
    uint64_t id;

    if (!expect_quic_with_stream_lock(s, /*remote_init=*/-1, /*io=*/0, &ctx))
        return UINT64_MAX;

    id = ctx.xso->stream->id;
    qctx_unlock(&ctx);

    return id;
}

/*
 * SSL_is_stream_local
 * -------------------
 */
QUIC_TAKES_LOCK
int ossl_quic_is_stream_local(SSL *s)
{
    QCTX ctx;
    int is_local;

    if (!expect_quic_with_stream_lock(s, /*remote_init=*/-1, /*io=*/0, &ctx))
        return -1;

    is_local = ossl_quic_stream_is_local_init(ctx.xso->stream);
    qctx_unlock(&ctx);

    return is_local;
}

/*
 * SSL_set_default_stream_mode
 * ---------------------------
 */
QUIC_TAKES_LOCK
int ossl_quic_set_default_stream_mode(SSL *s, uint32_t mode)
{
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    if (ctx.qc->default_xso_created) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED,
                                       "too late to change default stream mode");
    }

    switch (mode) {
    case SSL_DEFAULT_STREAM_MODE_NONE:
    case SSL_DEFAULT_STREAM_MODE_AUTO_BIDI:
    case SSL_DEFAULT_STREAM_MODE_AUTO_UNI:
        ctx.qc->default_stream_mode = mode;
        break;
    default:
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                       "bad default stream type");
    }

    qctx_unlock(&ctx);
    return 1;
}

/*
 * SSL_detach_stream
 * -----------------
 */
QUIC_TAKES_LOCK
SSL *ossl_quic_detach_stream(SSL *s)
{
    QCTX ctx;
    QUIC_XSO *xso = NULL;

    if (!expect_quic_conn_only(s, &ctx))
        return NULL;

    qctx_lock(&ctx);

    /* Calling this function inhibits default XSO autocreation. */
    /* QC ref to any default XSO is transferred to us and to caller. */
    qc_set_default_xso_keep_ref(ctx.qc, NULL, /*touch=*/1, &xso);

    qctx_unlock(&ctx);

    return xso != NULL ? &xso->obj.ssl : NULL;
}

/*
 * SSL_attach_stream
 * -----------------
 */
QUIC_TAKES_LOCK
int ossl_quic_attach_stream(SSL *conn, SSL *stream)
{
    QCTX ctx;
    QUIC_XSO *xso;
    int nref;

    if (!expect_quic_conn_only(conn, &ctx))
        return 0;

    if (stream == NULL || stream->type != SSL_TYPE_QUIC_XSO)
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_NULL_PARAMETER,
                                       "stream to attach must be a valid QUIC stream");

    xso = (QUIC_XSO *)stream;

    qctx_lock(&ctx);

    if (ctx.qc->default_xso != NULL) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED,
                                       "connection already has a default stream");
    }

    /*
     * It is a caller error for the XSO being attached as a default XSO to have
     * more than one ref.
     */
    if (!CRYPTO_GET_REF(&xso->obj.ssl.references, &nref)) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_INTERNAL_ERROR,
                                       "ref");
    }

    if (nref != 1) {
        qctx_unlock(&ctx);
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                       "stream being attached must have "
                                       "only 1 reference");
    }

    /* Caller's reference to the XSO is transferred to us. */
    /* Calling this function inhibits default XSO autocreation. */
    qc_set_default_xso(ctx.qc, xso, /*touch=*/1);

    qctx_unlock(&ctx);
    return 1;
}

/*
 * SSL_set_incoming_stream_policy
 * ------------------------------
 */
QUIC_NEEDS_LOCK
static int qc_get_effective_incoming_stream_policy(QUIC_CONNECTION *qc)
{
    switch (qc->incoming_stream_policy) {
        case SSL_INCOMING_STREAM_POLICY_AUTO:
            if ((qc->default_xso == NULL && !qc->default_xso_created)
                || qc->default_stream_mode == SSL_DEFAULT_STREAM_MODE_NONE)
                return SSL_INCOMING_STREAM_POLICY_ACCEPT;
            else
                return SSL_INCOMING_STREAM_POLICY_REJECT;

        default:
            return qc->incoming_stream_policy;
    }
}

QUIC_NEEDS_LOCK
static void qc_update_reject_policy(QUIC_CONNECTION *qc)
{
    int policy = qc_get_effective_incoming_stream_policy(qc);
    int enable_reject = (policy == SSL_INCOMING_STREAM_POLICY_REJECT);

    ossl_quic_channel_set_incoming_stream_auto_reject(qc->ch,
                                                      enable_reject,
                                                      qc->incoming_stream_aec);
}

QUIC_TAKES_LOCK
int ossl_quic_set_incoming_stream_policy(SSL *s, int policy,
                                         uint64_t aec)
{
    int ret = 1;
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    switch (policy) {
    case SSL_INCOMING_STREAM_POLICY_AUTO:
    case SSL_INCOMING_STREAM_POLICY_ACCEPT:
    case SSL_INCOMING_STREAM_POLICY_REJECT:
        ctx.qc->incoming_stream_policy = policy;
        ctx.qc->incoming_stream_aec    = aec;
        break;

    default:
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT, NULL);
        ret = 0;
        break;
    }

    qc_update_reject_policy(ctx.qc);
    qctx_unlock(&ctx);
    return ret;
}

/*
 * SSL_get_value, SSL_set_value
 * ----------------------------
 */
QUIC_TAKES_LOCK
static int qc_getset_idle_timeout(QCTX *ctx, uint32_t class_,
                                  uint64_t *p_value_out, uint64_t *p_value_in)
{
    int ret = 0;
    uint64_t value_out = 0, value_in;

    qctx_lock(ctx);

    switch (class_) {
    case SSL_VALUE_CLASS_FEATURE_REQUEST:
        value_out = ossl_quic_channel_get_max_idle_timeout_request(ctx->qc->ch);

        if (p_value_in != NULL) {
            value_in = *p_value_in;
            if (value_in > OSSL_QUIC_VLINT_MAX) {
                QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                            NULL);
                goto err;
            }

            if (ossl_quic_channel_have_generated_transport_params(ctx->qc->ch)) {
                QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_FEATURE_NOT_RENEGOTIABLE,
                                            NULL);
                goto err;
            }

            ossl_quic_channel_set_max_idle_timeout_request(ctx->qc->ch, value_in);
        }
        break;

    case SSL_VALUE_CLASS_FEATURE_PEER_REQUEST:
    case SSL_VALUE_CLASS_FEATURE_NEGOTIATED:
        if (p_value_in != NULL) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_UNSUPPORTED_CONFIG_VALUE_OP,
                                        NULL);
            goto err;
        }

        if (!ossl_quic_channel_is_handshake_complete(ctx->qc->ch)) {
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_FEATURE_NEGOTIATION_NOT_COMPLETE,
                                        NULL);
            goto err;
        }

        value_out = (class_ == SSL_VALUE_CLASS_FEATURE_NEGOTIATED)
            ? ossl_quic_channel_get_max_idle_timeout_actual(ctx->qc->ch)
            : ossl_quic_channel_get_max_idle_timeout_peer_request(ctx->qc->ch);
        break;

    default:
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_UNSUPPORTED_CONFIG_VALUE_CLASS,
                                    NULL);
        goto err;
    }

    ret = 1;
err:
    qctx_unlock(ctx);
    if (ret && p_value_out != NULL)
        *p_value_out = value_out;

    return ret;
}

QUIC_TAKES_LOCK
static int qc_get_stream_avail(QCTX *ctx, uint32_t class_,
                               int is_uni, int is_remote,
                               uint64_t *value)
{
    int ret = 0;

    if (class_ != SSL_VALUE_CLASS_GENERIC) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_UNSUPPORTED_CONFIG_VALUE_CLASS,
                                    NULL);
        return 0;
    }

    qctx_lock(ctx);

    *value = is_remote
        ? ossl_quic_channel_get_remote_stream_count_avail(ctx->qc->ch, is_uni)
        : ossl_quic_channel_get_local_stream_count_avail(ctx->qc->ch, is_uni);

    ret = 1;
    qctx_unlock(ctx);
    return ret;
}

QUIC_NEEDS_LOCK
static int qctx_should_autotick(QCTX *ctx)
{
    int event_handling_mode;
    QUIC_OBJ *obj = ctx->obj;

    for (; (event_handling_mode = obj->event_handling_mode) == SSL_VALUE_EVENT_HANDLING_MODE_INHERIT
           && obj->parent_obj != NULL; obj = obj->parent_obj);

    return event_handling_mode != SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT;
}

QUIC_NEEDS_LOCK
static void qctx_maybe_autotick(QCTX *ctx)
{
    if (!qctx_should_autotick(ctx))
        return;

    ossl_quic_reactor_tick(ossl_quic_obj_get0_reactor(ctx->obj), 0);
}

QUIC_TAKES_LOCK
static int qc_getset_event_handling(QCTX *ctx, uint32_t class_,
                                    uint64_t *p_value_out,
                                    uint64_t *p_value_in)
{
    int ret = 0;
    uint64_t value_out = 0;

    qctx_lock(ctx);

    if (class_ != SSL_VALUE_CLASS_GENERIC) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_UNSUPPORTED_CONFIG_VALUE_CLASS,
                                    NULL);
        goto err;
    }

    if (p_value_in != NULL) {
        switch (*p_value_in) {
        case SSL_VALUE_EVENT_HANDLING_MODE_INHERIT:
        case SSL_VALUE_EVENT_HANDLING_MODE_IMPLICIT:
        case SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT:
            break;
        default:
            QUIC_RAISE_NON_NORMAL_ERROR(ctx, ERR_R_PASSED_INVALID_ARGUMENT,
                                        NULL);
            goto err;
        }

        value_out = *p_value_in;
        ctx->obj->event_handling_mode = (int)value_out;
    } else {
        value_out = ctx->obj->event_handling_mode;
    }

    ret = 1;
err:
    qctx_unlock(ctx);
    if (ret && p_value_out != NULL)
        *p_value_out = value_out;

    return ret;
}

QUIC_TAKES_LOCK
static int qc_get_stream_write_buf_stat(QCTX *ctx, uint32_t class_,
                                        uint64_t *p_value_out,
                                        size_t (*getter)(QUIC_SSTREAM *sstream))
{
    int ret = 0;
    size_t value = 0;

    qctx_lock(ctx);

    if (class_ != SSL_VALUE_CLASS_GENERIC) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_UNSUPPORTED_CONFIG_VALUE_CLASS,
                                    NULL);
        goto err;
    }

    if (ctx->xso == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_NO_STREAM, NULL);
        goto err;
    }

    if (!ossl_quic_stream_has_send(ctx->xso->stream)) {
        QUIC_RAISE_NON_NORMAL_ERROR(ctx, SSL_R_STREAM_RECV_ONLY, NULL);
        goto err;
    }

    if (ossl_quic_stream_has_send_buffer(ctx->xso->stream))
        value = getter(ctx->xso->stream->sstream);

    ret = 1;
err:
    qctx_unlock(ctx);
    *p_value_out = (uint64_t)value;
    return ret;
}

QUIC_NEEDS_LOCK
static int expect_quic_for_value(SSL *s, QCTX *ctx, uint32_t id)
{
    switch (id) {
    case SSL_VALUE_EVENT_HANDLING_MODE:
    case SSL_VALUE_STREAM_WRITE_BUF_SIZE:
    case SSL_VALUE_STREAM_WRITE_BUF_USED:
    case SSL_VALUE_STREAM_WRITE_BUF_AVAIL:
        return expect_quic_cs(s, ctx);
    default:
        return expect_quic_conn_only(s, ctx);
    }
}

QUIC_TAKES_LOCK
int ossl_quic_get_value_uint(SSL *s, uint32_t class_, uint32_t id,
                             uint64_t *value)
{
    QCTX ctx;

    if (!expect_quic_for_value(s, &ctx, id))
        return 0;

    if (value == NULL)
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx,
                                           ERR_R_PASSED_INVALID_ARGUMENT, NULL);

    switch (id) {
    case SSL_VALUE_QUIC_IDLE_TIMEOUT:
        return qc_getset_idle_timeout(&ctx, class_, value, NULL);

    case SSL_VALUE_QUIC_STREAM_BIDI_LOCAL_AVAIL:
        return qc_get_stream_avail(&ctx, class_, /*uni=*/0, /*remote=*/0, value);
    case SSL_VALUE_QUIC_STREAM_BIDI_REMOTE_AVAIL:
        return qc_get_stream_avail(&ctx, class_, /*uni=*/0, /*remote=*/1, value);
    case SSL_VALUE_QUIC_STREAM_UNI_LOCAL_AVAIL:
        return qc_get_stream_avail(&ctx, class_, /*uni=*/1, /*remote=*/0, value);
    case SSL_VALUE_QUIC_STREAM_UNI_REMOTE_AVAIL:
        return qc_get_stream_avail(&ctx, class_, /*uni=*/1, /*remote=*/1, value);

    case SSL_VALUE_EVENT_HANDLING_MODE:
        return qc_getset_event_handling(&ctx, class_, value, NULL);

    case SSL_VALUE_STREAM_WRITE_BUF_SIZE:
        return qc_get_stream_write_buf_stat(&ctx, class_, value,
                                            ossl_quic_sstream_get_buffer_size);
    case SSL_VALUE_STREAM_WRITE_BUF_USED:
        return qc_get_stream_write_buf_stat(&ctx, class_, value,
                                            ossl_quic_sstream_get_buffer_used);
    case SSL_VALUE_STREAM_WRITE_BUF_AVAIL:
        return qc_get_stream_write_buf_stat(&ctx, class_, value,
                                            ossl_quic_sstream_get_buffer_avail);

    default:
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx,
                                           SSL_R_UNSUPPORTED_CONFIG_VALUE, NULL);
    }

    return 1;
}

QUIC_TAKES_LOCK
int ossl_quic_set_value_uint(SSL *s, uint32_t class_, uint32_t id,
                             uint64_t value)
{
    QCTX ctx;

    if (!expect_quic_for_value(s, &ctx, id))
        return 0;

    switch (id) {
    case SSL_VALUE_QUIC_IDLE_TIMEOUT:
        return qc_getset_idle_timeout(&ctx, class_, NULL, &value);

    case SSL_VALUE_EVENT_HANDLING_MODE:
        return qc_getset_event_handling(&ctx, class_, NULL, &value);

    default:
        return QUIC_RAISE_NON_NORMAL_ERROR(&ctx,
                                           SSL_R_UNSUPPORTED_CONFIG_VALUE, NULL);
    }

    return 1;
}

/*
 * SSL_accept_stream
 * -----------------
 */
struct wait_for_incoming_stream_args {
    QCTX            *ctx;
    QUIC_STREAM     *qs;
};

QUIC_NEEDS_LOCK
static int wait_for_incoming_stream(void *arg)
{
    struct wait_for_incoming_stream_args *args = arg;
    QUIC_CONNECTION *qc = args->ctx->qc;
    QUIC_STREAM_MAP *qsm = ossl_quic_channel_get_qsm(qc->ch);

    if (!quic_mutation_allowed(qc, /*req_active=*/1)) {
        /* If connection is torn down due to an error while blocking, stop. */
        QUIC_RAISE_NON_NORMAL_ERROR(args->ctx, SSL_R_PROTOCOL_IS_SHUTDOWN, NULL);
        return -1;
    }

    args->qs = ossl_quic_stream_map_peek_accept_queue(qsm);
    if (args->qs != NULL)
        return 1; /* got a stream */

    return 0; /* did not get a stream, keep trying */
}

QUIC_TAKES_LOCK
SSL *ossl_quic_accept_stream(SSL *s, uint64_t flags)
{
    QCTX ctx;
    int ret;
    SSL *new_s = NULL;
    QUIC_STREAM_MAP *qsm;
    QUIC_STREAM *qs;
    QUIC_XSO *xso;
    OSSL_RTT_INFO rtt_info;

    if (!expect_quic_conn_only(s, &ctx))
        return NULL;

    qctx_lock(&ctx);

    if (qc_get_effective_incoming_stream_policy(ctx.qc)
        == SSL_INCOMING_STREAM_POLICY_REJECT) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED, NULL);
        goto out;
    }

    qsm = ossl_quic_channel_get_qsm(ctx.qc->ch);

    qs = ossl_quic_stream_map_peek_accept_queue(qsm);
    if (qs == NULL) {
        if (qctx_blocking(&ctx)
            && (flags & SSL_ACCEPT_STREAM_NO_BLOCK) == 0) {
            struct wait_for_incoming_stream_args args;

            args.ctx = &ctx;
            args.qs = NULL;

            ret = block_until_pred(&ctx, wait_for_incoming_stream, &args, 0);
            if (ret == 0) {
                QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_INTERNAL_ERROR, NULL);
                goto out;
            } else if (ret < 0 || args.qs == NULL) {
                goto out;
            }

            qs = args.qs;
        } else {
            goto out;
        }
    }

    xso = create_xso_from_stream(ctx.qc, qs);
    if (xso == NULL)
        goto out;

    ossl_statm_get_rtt_info(ossl_quic_channel_get_statm(ctx.qc->ch), &rtt_info);
    ossl_quic_stream_map_remove_from_accept_queue(qsm, qs,
                                                  rtt_info.smoothed_rtt);
    new_s = &xso->obj.ssl;

    /* Calling this function inhibits default XSO autocreation. */
    qc_touch_default_xso(ctx.qc); /* inhibits default XSO */

out:
    qctx_unlock(&ctx);
    return new_s;
}

/*
 * SSL_get_accept_stream_queue_len
 * -------------------------------
 */
QUIC_TAKES_LOCK
size_t ossl_quic_get_accept_stream_queue_len(SSL *s)
{
    QCTX ctx;
    size_t v;

    if (!expect_quic_conn_only(s, &ctx))
        return 0;

    qctx_lock(&ctx);

    v = ossl_quic_stream_map_get_total_accept_queue_len(ossl_quic_channel_get_qsm(ctx.qc->ch));

    qctx_unlock(&ctx);
    return v;
}

/*
 * SSL_stream_reset
 * ----------------
 */
int ossl_quic_stream_reset(SSL *ssl,
                           const SSL_STREAM_RESET_ARGS *args,
                           size_t args_len)
{
    QCTX ctx;
    QUIC_STREAM_MAP *qsm;
    QUIC_STREAM *qs;
    uint64_t error_code;
    int ok, err;

    if (!expect_quic_with_stream_lock(ssl, /*remote_init=*/0, /*io=*/0, &ctx))
        return 0;

    qsm         = ossl_quic_channel_get_qsm(ctx.qc->ch);
    qs          = ctx.xso->stream;
    error_code  = (args != NULL ? args->quic_error_code : 0);

    if (!quic_validate_for_write(ctx.xso, &err)) {
        ok = QUIC_RAISE_NON_NORMAL_ERROR(&ctx, err, NULL);
        goto err;
    }

    ok = ossl_quic_stream_map_reset_stream_send_part(qsm, qs, error_code);
    if (ok)
        ctx.xso->requested_reset = 1;

err:
    qctx_unlock(&ctx);
    return ok;
}

/*
 * SSL_get_stream_read_state
 * -------------------------
 */
static void quic_classify_stream(QUIC_CONNECTION *qc,
                                 QUIC_STREAM *qs,
                                 int is_write,
                                 int *state,
                                 uint64_t *app_error_code)
{
    int local_init;
    uint64_t final_size;

    local_init = (ossl_quic_stream_is_server_init(qs) == qc->as_server);

    if (app_error_code != NULL)
        *app_error_code = UINT64_MAX;
    else
        app_error_code = &final_size; /* throw away value */

    if (!ossl_quic_stream_is_bidi(qs) && local_init != is_write) {
        /*
         * Unidirectional stream and this direction of transmission doesn't
         * exist.
         */
        *state = SSL_STREAM_STATE_WRONG_DIR;
    } else if (ossl_quic_channel_is_term_any(qc->ch)) {
        /* Connection already closed. */
        *state = SSL_STREAM_STATE_CONN_CLOSED;
    } else if (!is_write && qs->recv_state == QUIC_RSTREAM_STATE_DATA_READ) {
        /* Application has read a FIN. */
        *state = SSL_STREAM_STATE_FINISHED;
    } else if ((!is_write && qs->stop_sending)
               || (is_write && ossl_quic_stream_send_is_reset(qs))) {
        /*
         * Stream has been reset locally. FIN takes precedence over this for the
         * read case as the application need not care if the stream is reset
         * after a FIN has been successfully processed.
         */
        *state          = SSL_STREAM_STATE_RESET_LOCAL;
        *app_error_code = !is_write
            ? qs->stop_sending_aec
            : qs->reset_stream_aec;
    } else if ((!is_write && ossl_quic_stream_recv_is_reset(qs))
               || (is_write && qs->peer_stop_sending)) {
        /*
         * Stream has been reset remotely. */
        *state          = SSL_STREAM_STATE_RESET_REMOTE;
        *app_error_code = !is_write
            ? qs->peer_reset_stream_aec
            : qs->peer_stop_sending_aec;
    } else if (is_write && ossl_quic_sstream_get_final_size(qs->sstream,
                                                            &final_size)) {
        /*
         * Stream has been finished. Stream reset takes precedence over this for
         * the write case as peer may not have received all data.
         */
        *state = SSL_STREAM_STATE_FINISHED;
    } else {
        /* Stream still healthy. */
        *state = SSL_STREAM_STATE_OK;
    }
}

static int quic_get_stream_state(SSL *ssl, int is_write)
{
    QCTX ctx;
    int state;

    if (!expect_quic_with_stream_lock(ssl, /*remote_init=*/-1, /*io=*/0, &ctx))
        return SSL_STREAM_STATE_NONE;

    quic_classify_stream(ctx.qc, ctx.xso->stream, is_write, &state, NULL);
    qctx_unlock(&ctx);
    return state;
}

int ossl_quic_get_stream_read_state(SSL *ssl)
{
    return quic_get_stream_state(ssl, /*is_write=*/0);
}

/*
 * SSL_get_stream_write_state
 * --------------------------
 */
int ossl_quic_get_stream_write_state(SSL *ssl)
{
    return quic_get_stream_state(ssl, /*is_write=*/1);
}

/*
 * SSL_get_stream_read_error_code
 * ------------------------------
 */
static int quic_get_stream_error_code(SSL *ssl, int is_write,
                                      uint64_t *app_error_code)
{
    QCTX ctx;
    int state;

    if (!expect_quic_with_stream_lock(ssl, /*remote_init=*/-1, /*io=*/0, &ctx))
        return -1;

    quic_classify_stream(ctx.qc, ctx.xso->stream, /*is_write=*/0,
                         &state, app_error_code);

    qctx_unlock(&ctx);
    switch (state) {
        case SSL_STREAM_STATE_FINISHED:
             return 0;
        case SSL_STREAM_STATE_RESET_LOCAL:
        case SSL_STREAM_STATE_RESET_REMOTE:
             return 1;
        default:
             return -1;
    }
}

int ossl_quic_get_stream_read_error_code(SSL *ssl, uint64_t *app_error_code)
{
    return quic_get_stream_error_code(ssl, /*is_write=*/0, app_error_code);
}

/*
 * SSL_get_stream_write_error_code
 * -------------------------------
 */
int ossl_quic_get_stream_write_error_code(SSL *ssl, uint64_t *app_error_code)
{
    return quic_get_stream_error_code(ssl, /*is_write=*/1, app_error_code);
}

/*
 * Write buffer size mutation
 * --------------------------
 */
int ossl_quic_set_write_buffer_size(SSL *ssl, size_t size)
{
    int ret = 0;
    QCTX ctx;

    if (!expect_quic_with_stream_lock(ssl, /*remote_init=*/-1, /*io=*/0, &ctx))
        return 0;

    if (!ossl_quic_stream_has_send(ctx.xso->stream)) {
        /* Called on a unidirectional receive-only stream - error. */
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED, NULL);
        goto out;
    }

    if (!ossl_quic_stream_has_send_buffer(ctx.xso->stream)) {
        /*
         * If the stream has a send part but we have disposed of it because we
         * no longer need it, this is a no-op.
         */
        ret = 1;
        goto out;
    }

    if (!ossl_quic_sstream_set_buffer_size(ctx.xso->stream->sstream, size)) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_INTERNAL_ERROR, NULL);
        goto out;
    }

    ret = 1;

out:
    qctx_unlock(&ctx);
    return ret;
}

/*
 * SSL_get_conn_close_info
 * -----------------------
 */
int ossl_quic_get_conn_close_info(SSL *ssl,
                                  SSL_CONN_CLOSE_INFO *info,
                                  size_t info_len)
{
    QCTX ctx;
    const QUIC_TERMINATE_CAUSE *tc;

    if (!expect_quic_conn_only(ssl, &ctx))
        return -1;

    tc = ossl_quic_channel_get_terminate_cause(ctx.qc->ch);
    if (tc == NULL)
        return 0;

    info->error_code    = tc->error_code;
    info->frame_type    = tc->frame_type;
    info->reason        = tc->reason;
    info->reason_len    = tc->reason_len;
    info->flags         = 0;
    if (!tc->remote)
        info->flags |= SSL_CONN_CLOSE_FLAG_LOCAL;
    if (!tc->app)
        info->flags |= SSL_CONN_CLOSE_FLAG_TRANSPORT;
    return 1;
}

/*
 * SSL_key_update
 * --------------
 */
int ossl_quic_key_update(SSL *ssl, int update_type)
{
    QCTX ctx;

    if (!expect_quic_conn_only(ssl, &ctx))
        return 0;

    switch (update_type) {
    case SSL_KEY_UPDATE_NOT_REQUESTED:
        /*
         * QUIC signals peer key update implicily by triggering a local
         * spontaneous TXKU. Silently upgrade this to SSL_KEY_UPDATE_REQUESTED.
         */
    case SSL_KEY_UPDATE_REQUESTED:
        break;

    default:
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, ERR_R_PASSED_INVALID_ARGUMENT, NULL);
        return 0;
    }

    qctx_lock(&ctx);

    /* Attempt to perform a TXKU. */
    if (!ossl_quic_channel_trigger_txku(ctx.qc->ch)) {
        QUIC_RAISE_NON_NORMAL_ERROR(&ctx, SSL_R_TOO_MANY_KEY_UPDATES, NULL);
        qctx_unlock(&ctx);
        return 0;
    }

    qctx_unlock(&ctx);
    return 1;
}

/*
 * SSL_get_key_update_type
 * -----------------------
 */
int ossl_quic_get_key_update_type(const SSL *s)
{
    /*
     * We always handle key updates immediately so a key update is never
     * pending.
     */
    return SSL_KEY_UPDATE_NONE;
}

/**
 * @brief Allocates an SSL object for a user from a QUIC channel.
 *
 * This function creates a new QUIC_CONNECTION object based on an incoming
 * connection associated with the provided QUIC_LISTENER. If the connection
 * creation fails, the function returns NULL. Otherwise, it returns a pointer
 * to the SSL object associated with the newly created connection.
 *
 * Note: This function is a registered port callback made from
 * ossl_quic_new_listener and ossl_quic_new_listener_from, and allows for
 * pre-allocation of the user_ssl object when a channel is created, rather than
 * when it is accepted
 *
 * @param ch  Pointer to the QUIC_CHANNEL representing the incoming connection.
 * @param arg Pointer to a QUIC_LISTENER used to create the connection.
 *
 * @return Pointer to the SSL object on success, or NULL on failure.
 */
static SSL *alloc_port_user_ssl(QUIC_CHANNEL *ch, void *arg)
{
    QUIC_LISTENER *ql = arg;
    QUIC_CONNECTION *qc = create_qc_from_incoming_conn(ql, ch);

    return (qc == NULL) ? NULL : &qc->obj.ssl;
}

/*
 * QUIC Front-End I/O API: Listeners
 * =================================
 */

/*
 * SSL_new_listener
 * ----------------
 */
SSL *ossl_quic_new_listener(SSL_CTX *ctx, uint64_t flags)
{
    QUIC_LISTENER *ql = NULL;
    QUIC_ENGINE_ARGS engine_args = {0};
    QUIC_PORT_ARGS port_args = {0};

    if ((ql = OPENSSL_zalloc(sizeof(*ql))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }

#if defined(OPENSSL_THREADS)
    if ((ql->mutex = ossl_crypto_mutex_new()) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }
#endif

    engine_args.libctx  = ctx->libctx;
    engine_args.propq   = ctx->propq;
#if defined(OPENSSL_THREADS)
    engine_args.mutex   = ql->mutex;
#endif

    if (need_notifier_for_domain_flags(ctx->domain_flags))
        engine_args.reactor_flags |= QUIC_REACTOR_FLAG_USE_NOTIFIER;

    if ((ql->engine = ossl_quic_engine_new(&engine_args)) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    port_args.channel_ctx       = ctx;
    port_args.is_multi_conn     = 1;
    port_args.get_conn_user_ssl = alloc_port_user_ssl;
    port_args.user_ssl_arg = ql;
    if ((flags & SSL_LISTENER_FLAG_NO_VALIDATE) == 0)
        port_args.do_addr_validation = 1;
    ql->port = ossl_quic_engine_create_port(ql->engine, &port_args);
    if (ql->port == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* TODO(QUIC FUTURE): Implement SSL_LISTENER_FLAG_NO_ACCEPT */

    ossl_quic_port_set_allow_incoming(ql->port, 1);

    /* Initialise the QUIC_LISTENER's object header. */
    if (!ossl_quic_obj_init(&ql->obj, ctx, SSL_TYPE_QUIC_LISTENER, NULL,
                            ql->engine, ql->port))
        goto err;

    return &ql->obj.ssl;

err:
    if (ql != NULL)
        ossl_quic_engine_free(ql->engine);

#if defined(OPENSSL_THREADS)
    ossl_crypto_mutex_free(&ql->mutex);
#endif
    OPENSSL_free(ql);
    return NULL;
}

/*
 * SSL_new_listener_from
 * ---------------------
 */
SSL *ossl_quic_new_listener_from(SSL *ssl, uint64_t flags)
{
    QCTX ctx;
    QUIC_LISTENER *ql = NULL;
    QUIC_PORT_ARGS port_args = {0};

    if (!expect_quic_domain(ssl, &ctx))
        return NULL;

    if (!SSL_up_ref(&ctx.qd->obj.ssl))
        return NULL;

    qctx_lock(&ctx);

    if ((ql = OPENSSL_zalloc(sizeof(*ql))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }

    port_args.channel_ctx       = ssl->ctx;
    port_args.is_multi_conn     = 1;
    port_args.get_conn_user_ssl = alloc_port_user_ssl;
    port_args.user_ssl_arg = ql;
    if ((flags & SSL_LISTENER_FLAG_NO_VALIDATE) == 0)
        port_args.do_addr_validation = 1;
    ql->port = ossl_quic_engine_create_port(ctx.qd->engine, &port_args);
    if (ql->port == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    ql->domain  = ctx.qd;
    ql->engine  = ctx.qd->engine;
#if defined(OPENSSL_THREADS)
    ql->mutex   = ctx.qd->mutex;
#endif

    /*
     * TODO(QUIC FUTURE): Implement SSL_LISTENER_FLAG_NO_ACCEPT
     * Given that we have apis to create client SSL objects from
     * server SSL objects (see SSL_new_from_listener), we have aspirations
     * to enable a flag that allows for the creation of the latter, but not
     * be used to do accept any connections.  This is a placeholder for the
     * implementation of that flag
     */

    ossl_quic_port_set_allow_incoming(ql->port, 1);

    /* Initialise the QUIC_LISTENER's object header. */
    if (!ossl_quic_obj_init(&ql->obj, ssl->ctx, SSL_TYPE_QUIC_LISTENER,
                            &ctx.qd->obj.ssl, NULL, ql->port))
        goto err;

    qctx_unlock(&ctx);
    return &ql->obj.ssl;

err:
    if (ql != NULL)
        ossl_quic_port_free(ql->port);

    OPENSSL_free(ql);
    qctx_unlock(&ctx);
    SSL_free(&ctx.qd->obj.ssl);

    return NULL;
}

/*
 * SSL_new_from_listener
 * ---------------------
 * code here is derived from ossl_quic_new(). The `ssl` argument is
 * a listener object which already comes with QUIC port/engine. The newly
 * created QUIC connection object (QCSO) is going to share the port/engine
 * with listener (`ssl`).  The `ssl` also becomes a parent of QCSO created
 * by this function. The caller uses QCSO instance to connect to
 * remote QUIC server.
 *
 * The QCSO created here requires us to also create a channel so we
 * can connect to remote server.
 */
SSL *ossl_quic_new_from_listener(SSL *ssl, uint64_t flags)
{
    QCTX ctx;
    QUIC_CONNECTION *qc = NULL;
    QUIC_LISTENER *ql;
    SSL_CONNECTION *sc = NULL;

    if (flags != 0)
        return NULL;

    if (!expect_quic_listener(ssl, &ctx))
        return NULL;

    if (!SSL_up_ref(&ctx.ql->obj.ssl))
        return NULL;

    qctx_lock(&ctx);

    ql = ctx.ql;

    /*
     * listeners (server) contexts don't typically
     * allocate a token cache because they don't need
     * to store them, but here we are using a server side
     * ctx as a client, so we should allocate one now
     */
    if (ssl->ctx->tokencache == NULL)
        if ((ssl->ctx->tokencache = ossl_quic_new_token_store()) == NULL)
            goto err;

    if ((qc = OPENSSL_zalloc(sizeof(*qc))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }

    /*
     * NOTE: setting a listener here is needed so `qc_cleanup()` does the right
     * thing. Setting listener to ql avoids premature destruction of port in
     * qc_cleanup()
     */
    qc->listener = ql;
    qc->engine = ql->engine;
    qc->port = ql->port;
/* create channel */
#if defined(OPENSSL_THREADS)
    /* this is the engine mutex */
    qc->mutex = ql->mutex;
#endif
#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)
    qc->is_thread_assisted
    = ((ql->obj.domain_flags & SSL_DOMAIN_FLAG_THREAD_ASSISTED) != 0);
#endif

    /* Create the handshake layer. */
    qc->tls = ossl_ssl_connection_new_int(ql->obj.ssl.ctx, NULL, TLS_method());
    if (qc->tls == NULL || (sc = SSL_CONNECTION_FROM_SSL(qc->tls)) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }
    sc->s3.flags |= TLS1_FLAGS_QUIC | TLS1_FLAGS_QUIC_INTERNAL;

    qc->default_ssl_options = OSSL_QUIC_PERMITTED_OPTIONS;
    qc->last_error = SSL_ERROR_NONE;

    /*
     * This is QCSO, we don't expect to accept connections
     * on success the channel assumes ownership of tls, we need
     * to grab reference for qc.
     */
    qc->ch = ossl_quic_port_create_outgoing(qc->port, qc->tls);

    ossl_quic_channel_set_msg_callback(qc->ch, ql->obj.ssl.ctx->msg_callback, &qc->obj.ssl);
    ossl_quic_channel_set_msg_callback_arg(qc->ch, ql->obj.ssl.ctx->msg_callback_arg);

    /*
     * We deliberately pass NULL for engine and port, because we don't want to
     * to turn QCSO we create here into an event leader, nor port leader.
     * Both those roles are occupied already by listener (`ssl`) we use
     * to create a new QCSO here.
     */
    if (!ossl_quic_obj_init(&qc->obj, ql->obj.ssl.ctx,
                            SSL_TYPE_QUIC_CONNECTION,
                            &ql->obj.ssl, NULL, NULL)) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* Initialise libssl APL-related state. */
    qc->default_stream_mode = SSL_DEFAULT_STREAM_MODE_AUTO_BIDI;
    qc->default_ssl_mode = qc->obj.ssl.ctx->mode;
    qc->default_ssl_options = qc->obj.ssl.ctx->options & OSSL_QUIC_PERMITTED_OPTIONS;
    qc->incoming_stream_policy = SSL_INCOMING_STREAM_POLICY_AUTO;
    qc->last_error = SSL_ERROR_NONE;

    qc_update_reject_policy(qc);

    qctx_unlock(&ctx);

    return &qc->obj.ssl;

err:
    if (qc != NULL) {
        qc_cleanup(qc, /* have_lock= */ 0);
        OPENSSL_free(qc);
    }
    qctx_unlock(&ctx);
    SSL_free(&ctx.ql->obj.ssl);

    return NULL;
}

/*
 * SSL_listen
 * ----------
 */
QUIC_NEEDS_LOCK
static int ql_listen(QUIC_LISTENER *ql)
{
    if (ql->listening)
        return 1;

    ossl_quic_port_set_allow_incoming(ql->port, 1);
    ql->listening = 1;
    return 1;
}

QUIC_TAKES_LOCK
int ossl_quic_listen(SSL *ssl)
{
    QCTX ctx;
    int ret;

    if (!expect_quic_listener(ssl, &ctx))
        return 0;

    qctx_lock_for_io(&ctx);

    ret = ql_listen(ctx.ql);

    qctx_unlock(&ctx);
    return ret;
}

/*
 * SSL_accept_connection
 * ---------------------
 */
static int quic_accept_connection_wait(void *arg)
{
    QUIC_PORT *port = arg;

    if (!ossl_quic_port_is_running(port))
        return -1;

    if (ossl_quic_port_have_incoming(port))
        return 1;

    return 0;
}

QUIC_TAKES_LOCK
SSL *ossl_quic_accept_connection(SSL *ssl, uint64_t flags)
{
    int ret;
    QCTX ctx;
    SSL *conn_ssl = NULL;
    SSL_CONNECTION *conn = NULL;
    QUIC_CHANNEL *new_ch = NULL;
    QUIC_CONNECTION *qc;
    int no_block = ((flags & SSL_ACCEPT_CONNECTION_NO_BLOCK) != 0);

    if (!expect_quic_listener(ssl, &ctx))
        return NULL;

    qctx_lock_for_io(&ctx);

    if (!ql_listen(ctx.ql))
        goto out;

    /* Wait for an incoming connection if needed. */
    new_ch = ossl_quic_port_pop_incoming(ctx.ql->port);
    if (new_ch == NULL && ossl_quic_port_is_running(ctx.ql->port)) {
        if (!no_block && qctx_blocking(&ctx)) {
            ret = block_until_pred(&ctx, quic_accept_connection_wait,
                                   ctx.ql->port, 0);
            if (ret < 1)
                goto out;
        } else {
            qctx_maybe_autotick(&ctx);
        }

        if (!ossl_quic_port_is_running(ctx.ql->port))
            goto out;

        new_ch = ossl_quic_port_pop_incoming(ctx.ql->port);
    }

    if (new_ch == NULL && ossl_quic_port_is_running(ctx.ql->port)) {
        /* No connections already queued. */
        ossl_quic_reactor_tick(ossl_quic_engine_get0_reactor(ctx.ql->engine), 0);

        new_ch = ossl_quic_port_pop_incoming(ctx.ql->port);
    }

    /*
     * port_make_channel pre-allocates our user_ssl for us for each newly
     * created channel, so once we pop the new channel from the port above
     * we just need to extract it
     */
    if (new_ch == NULL
        || (conn_ssl = ossl_quic_channel_get0_tls(new_ch)) == NULL
        || (conn = SSL_CONNECTION_FROM_SSL(conn_ssl)) == NULL
        || (conn_ssl = SSL_CONNECTION_GET_USER_SSL(conn)) == NULL)
        goto out;
    qc = (QUIC_CONNECTION *)conn_ssl;
    qc->listener = ctx.ql;
    qc->pending = 0;
    if (!SSL_up_ref(&ctx.ql->obj.ssl)) {
        SSL_free(conn_ssl);
        SSL_free(ossl_quic_channel_get0_tls(new_ch));
        conn_ssl = NULL;
    }

out:
    qctx_unlock(&ctx);
    return conn_ssl;
}

static QUIC_CONNECTION *create_qc_from_incoming_conn(QUIC_LISTENER *ql, QUIC_CHANNEL *ch)
{
    QUIC_CONNECTION *qc = NULL;

    if ((qc = OPENSSL_zalloc(sizeof(*qc))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }

    if (!ossl_quic_obj_init(&qc->obj, ql->obj.ssl.ctx,
                            SSL_TYPE_QUIC_CONNECTION,
                            &ql->obj.ssl, NULL, NULL)) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    ossl_quic_channel_get_peer_addr(ch, &qc->init_peer_addr); /* best effort */
    qc->pending                 = 1;
    qc->engine                  = ql->engine;
    qc->port                    = ql->port;
    qc->ch                      = ch;
#if defined(OPENSSL_THREADS)
    qc->mutex                   = ql->mutex;
#endif
    qc->tls                     = ossl_quic_channel_get0_tls(ch);
    qc->started                 = 1;
    qc->as_server               = 1;
    qc->as_server_state         = 1;
    qc->default_stream_mode     = SSL_DEFAULT_STREAM_MODE_AUTO_BIDI;
    qc->default_ssl_options     = ql->obj.ssl.ctx->options & OSSL_QUIC_PERMITTED_OPTIONS;
    qc->incoming_stream_policy  = SSL_INCOMING_STREAM_POLICY_AUTO;
    qc->last_error              = SSL_ERROR_NONE;
    qc_update_reject_policy(qc);
    return qc;

err:
    OPENSSL_free(qc);
    return NULL;
}

DEFINE_LHASH_OF_EX(QUIC_TOKEN);

struct ssl_token_store_st {
    LHASH_OF(QUIC_TOKEN) *cache;
    CRYPTO_REF_COUNT references;
    CRYPTO_MUTEX *mutex;
};

static unsigned long quic_token_hash(const QUIC_TOKEN *item)
{
    return (unsigned long)ossl_fnv1a_hash(item->hashkey, item->hashkey_len);
}

static int quic_token_cmp(const QUIC_TOKEN *a, const QUIC_TOKEN *b)
{
    if (a->hashkey_len != b->hashkey_len)
        return 1;
    return memcmp(a->hashkey, b->hashkey, a->hashkey_len);
}

SSL_TOKEN_STORE *ossl_quic_new_token_store(void)
{
    int ok = 0;
    SSL_TOKEN_STORE *newcache = OPENSSL_zalloc(sizeof(SSL_TOKEN_STORE));

    if (newcache == NULL)
        goto out;

    newcache->cache = lh_QUIC_TOKEN_new(quic_token_hash, quic_token_cmp);
    if (newcache->cache == NULL)
        goto out;

#if defined(OPENSSL_THREADS)
    if ((newcache->mutex = ossl_crypto_mutex_new()) == NULL)
        goto out;
#endif

    if (!CRYPTO_NEW_REF(&newcache->references, 1))
        goto out;

    ok = 1;
out:
    if (!ok) {
        ossl_quic_free_token_store(newcache);
        newcache = NULL;
    }
    return newcache;
}

static void free_this_token(QUIC_TOKEN *tok)
{
    ossl_quic_free_peer_token(tok);
}

void ossl_quic_free_token_store(SSL_TOKEN_STORE *hdl)
{
    int refs;

    if (hdl == NULL)
        return;

    if (!CRYPTO_DOWN_REF(&hdl->references, &refs))
        return;

    if (refs > 0)
        return;

    /* last reference, we can clean up */
    ossl_crypto_mutex_free(&hdl->mutex);
    lh_QUIC_TOKEN_doall(hdl->cache, free_this_token);
    lh_QUIC_TOKEN_free(hdl->cache);
    OPENSSL_free(hdl);
    return;
}

/**
 * @brief build a new QUIC_TOKEN
 *
 * This function creates a new token storage structure for saving in our
 * tokencache
 *
 * In an effort to make allocation and freeing of these tokens a bit faster
 * We do them in a single allocation in this format
 * +---------------+        --\
 * |   hashkey *   |---|      |
 * |   hashkey_len |   |      | QUIC_TOKEN
 * |   token *     |---|--|   |
 * |   token_len   |   |  |   |
 * +---------------+<--|  | --/
 * |  hashkey buf  |      |
 * |               |      |
 * |---------------|<-----|
 * |  token buf    |
 * |               |
 * +---------------+
 *
 * @param peer - the peer address that sent the token
 * @param token - the buffer holding the token
 * @param token_len - the size of token
 *
 * @returns a QUIC_TOKEN pointer or NULL on error
 */
static QUIC_TOKEN *ossl_quic_build_new_token(BIO_ADDR *peer, uint8_t *token,
                                             size_t token_len)
{
    QUIC_TOKEN *new_token;
    size_t hashkey_len = 0;
    size_t addr_len = 0;
    int family;
    unsigned short port;
    int *famptr;
    unsigned short *portptr;
    uint8_t *addrptr;

    if ((token != NULL && token_len == 0) || (token == NULL && token_len != 0))
        return NULL;

    if (!BIO_ADDR_rawaddress(peer, NULL, &addr_len))
        return NULL;
    family = BIO_ADDR_family(peer);
    port = BIO_ADDR_rawport(peer);

    hashkey_len += sizeof(int); /* hashkey(family) */
    hashkey_len += sizeof(unsigned short); /* hashkey(port) */
    hashkey_len += addr_len; /* hashkey(address) */

    new_token = OPENSSL_zalloc(sizeof(QUIC_TOKEN) + hashkey_len + token_len);
    if (new_token == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&new_token->references, 1)) {
        OPENSSL_free(new_token);
        return NULL;
    }

    new_token->hashkey_len = hashkey_len;
    /* hashkey is allocated inline, immediately after the QUIC_TOKEN struct */
    new_token->hashkey = (uint8_t *)(new_token + 1);
    /* token buffer follows the hashkey in the inline allocation */
    new_token->token = new_token->hashkey + hashkey_len;
    new_token->token_len = token_len;
    famptr = (int *)new_token->hashkey;
    portptr = (unsigned short *)(famptr + 1);
    addrptr = (uint8_t *)(portptr + 1);
    *famptr = family;
    *portptr = port;
    if (!BIO_ADDR_rawaddress(peer, addrptr, NULL)) {
        ossl_quic_free_peer_token(new_token);
        return NULL;
    }
    if (token != NULL)
        memcpy(new_token->token, token, token_len);
    return new_token;
}

int ossl_quic_set_peer_token(SSL_CTX *ctx, BIO_ADDR *peer,
                             const uint8_t *token, size_t token_len)
{
    SSL_TOKEN_STORE *c = ctx->tokencache;
    QUIC_TOKEN *tok, *old = NULL;

    if (ctx->tokencache == NULL)
        return 0;

    tok = ossl_quic_build_new_token(peer, (uint8_t *)token, token_len);
    if (tok == NULL)
        return 0;

    /* we might be sharing this cache, lock it */
    ossl_crypto_mutex_lock(c->mutex);

    old = lh_QUIC_TOKEN_retrieve(c->cache, tok);
    if (old != NULL) {
        lh_QUIC_TOKEN_delete(c->cache, old);
        ossl_quic_free_peer_token(old);
    }
    lh_QUIC_TOKEN_insert(c->cache, tok);

    ossl_crypto_mutex_unlock(c->mutex);
    return 1;
}

int ossl_quic_get_peer_token(SSL_CTX *ctx, BIO_ADDR *peer,
                             QUIC_TOKEN **token)
{
    SSL_TOKEN_STORE *c = ctx->tokencache;
    QUIC_TOKEN *key = NULL;
    QUIC_TOKEN *tok = NULL;
    int ret;
    int rc = 0;

    if (c == NULL)
        return 0;

    key = ossl_quic_build_new_token(peer, NULL, 0);
    if (key == NULL)
        return 0;

    ossl_crypto_mutex_lock(c->mutex);
    tok = lh_QUIC_TOKEN_retrieve(c->cache, key);
    if (tok != NULL) {
        *token = tok;
        CRYPTO_UP_REF(&tok->references, &ret);
        rc = 1;
    }

    ossl_crypto_mutex_unlock(c->mutex);
    ossl_quic_free_peer_token(key);
    return rc;
}

void ossl_quic_free_peer_token(QUIC_TOKEN *token)
{
    int refs = 0;

    if (!CRYPTO_DOWN_REF(&token->references, &refs))
        return;

    if (refs > 0)
        return;

    CRYPTO_FREE_REF(&token->references);
    OPENSSL_free(token);
}

/*
 * SSL_get_accept_connection_queue_len
 * -----------------------------------
 */
QUIC_TAKES_LOCK
size_t ossl_quic_get_accept_connection_queue_len(SSL *ssl)
{
    QCTX ctx;
    int ret;

    if (!expect_quic_listener(ssl, &ctx))
        return 0;

    qctx_lock(&ctx);

    ret = ossl_quic_port_get_num_incoming_channels(ctx.ql->port);

    qctx_unlock(&ctx);
    return ret;
}

/*
 * QUIC Front-End I/O API: Domains
 * ===============================
 */

/*
 * SSL_new_domain
 * --------------
 */
SSL *ossl_quic_new_domain(SSL_CTX *ctx, uint64_t flags)
{
    QUIC_DOMAIN *qd = NULL;
    QUIC_ENGINE_ARGS engine_args = {0};
    uint64_t domain_flags;

    domain_flags = ctx->domain_flags;
    if ((flags & (SSL_DOMAIN_FLAG_SINGLE_THREAD
                  | SSL_DOMAIN_FLAG_MULTI_THREAD
                  | SSL_DOMAIN_FLAG_THREAD_ASSISTED)) != 0)
        domain_flags = flags;
    else
        domain_flags = ctx->domain_flags | flags;

    if (!ossl_adjust_domain_flags(domain_flags, &domain_flags))
        return NULL;

    if ((qd = OPENSSL_zalloc(sizeof(*qd))) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        return NULL;
    }

#if defined(OPENSSL_THREADS)
    if ((qd->mutex = ossl_crypto_mutex_new()) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_CRYPTO_LIB, NULL);
        goto err;
    }
#endif

    engine_args.libctx  = ctx->libctx;
    engine_args.propq   = ctx->propq;
#if defined(OPENSSL_THREADS)
    engine_args.mutex   = qd->mutex;
#endif

    if (need_notifier_for_domain_flags(domain_flags))
        engine_args.reactor_flags |= QUIC_REACTOR_FLAG_USE_NOTIFIER;

    if ((qd->engine = ossl_quic_engine_new(&engine_args)) == NULL) {
        QUIC_RAISE_NON_NORMAL_ERROR(NULL, ERR_R_INTERNAL_ERROR, NULL);
        goto err;
    }

    /* Initialise the QUIC_DOMAIN's object header. */
    if (!ossl_quic_obj_init(&qd->obj, ctx, SSL_TYPE_QUIC_DOMAIN, NULL,
                            qd->engine, NULL))
        goto err;

    ossl_quic_obj_set_domain_flags(&qd->obj, domain_flags);
    return &qd->obj.ssl;

err:
    ossl_quic_engine_free(qd->engine);
#if defined(OPENSSL_THREADS)
    ossl_crypto_mutex_free(&qd->mutex);
#endif
    OPENSSL_free(qd);
    return NULL;
}

/*
 * QUIC Front-End I/O API: SSL_CTX Management
 * ==========================================
 */

long ossl_quic_ctx_ctrl(SSL_CTX *ctx, int cmd, long larg, void *parg)
{
    switch (cmd) {
    default:
        return ssl3_ctx_ctrl(ctx, cmd, larg, parg);
    }
}

long ossl_quic_callback_ctrl(SSL *s, int cmd, void (*fp) (void))
{
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return 0;

    switch (cmd) {
    case SSL_CTRL_SET_MSG_CALLBACK:
        ossl_quic_channel_set_msg_callback(ctx.qc->ch, (ossl_msg_cb)fp,
                                           &ctx.qc->obj.ssl);
        /* This callback also needs to be set on the internal SSL object */
        return ssl3_callback_ctrl(ctx.qc->tls, cmd, fp);;

    default:
        /* Probably a TLS related ctrl. Defer to our internal SSL object */
        return ssl3_callback_ctrl(ctx.qc->tls, cmd, fp);
    }
}

long ossl_quic_ctx_callback_ctrl(SSL_CTX *ctx, int cmd, void (*fp) (void))
{
    return ssl3_ctx_callback_ctrl(ctx, cmd, fp);
}

int ossl_quic_renegotiate_check(SSL *ssl, int initok)
{
    /* We never do renegotiation. */
    return 0;
}

const SSL_CIPHER *ossl_quic_get_cipher_by_char(const unsigned char *p)
{
    const SSL_CIPHER *ciph = ssl3_get_cipher_by_char(p);

    if ((ciph->algorithm2 & SSL_QUIC) == 0)
        return NULL;

    return ciph;
}

/*
 * These functions define the TLSv1.2 (and below) ciphers that are supported by
 * the SSL_METHOD. Since QUIC only supports TLSv1.3 we don't support any.
 */

int ossl_quic_num_ciphers(void)
{
    return 0;
}

const SSL_CIPHER *ossl_quic_get_cipher(unsigned int u)
{
    return NULL;
}

/*
 * SSL_get_shutdown()
 * ------------------
 */
int ossl_quic_get_shutdown(const SSL *s)
{
    QCTX ctx;
    int shut = 0;

    if (!expect_quic_conn_only(s, &ctx))
        return 0;

    if (ossl_quic_channel_is_term_any(ctx.qc->ch)) {
        shut |= SSL_SENT_SHUTDOWN;
        if (!ossl_quic_channel_is_closing(ctx.qc->ch))
            shut |= SSL_RECEIVED_SHUTDOWN;
    }

    return shut;
}

/*
 * QUIC Polling Support APIs
 * =========================
 */

/* Do we have the R (read) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_r(QUIC_XSO *xso)
{
    int fin = 0;
    size_t avail = 0;

    /*
     * If a stream has had the fin bit set on the last packet
     * received, then we need to return a 1 here to raise
     * SSL_POLL_EVENT_R, so that the stream can have its completion
     * detected and closed gracefully by an application.
     * However, if the client reads the data via SSL_read[_ex], that api
     * provides no stream status, and as a result the stream state moves to
     * QUIC_RSTREAM_STATE_DATA_READ, and the receive buffer is freed, which
     * stored the fin state, so its not directly know-able here.  Instead
     * check for the stream state being QUIC_RSTREAM_STATE_DATA_READ, which
     * is only set if the last stream frame received had the fin bit set, and
     * the client read the data.  This catches our poll/read/poll case
     */
    if (xso->stream->recv_state == QUIC_RSTREAM_STATE_DATA_READ)
        return 1;

    return ossl_quic_stream_has_recv_buffer(xso->stream)
        && ossl_quic_rstream_available(xso->stream->rstream, &avail, &fin)
        && (avail > 0 || (fin && !xso->retired_fin));
}

/* Do we have the ER (exception: read) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_er(QUIC_XSO *xso)
{
    return ossl_quic_stream_has_recv(xso->stream)
        && ossl_quic_stream_recv_is_reset(xso->stream)
        && !xso->retired_fin;
}

/* Do we have the W (write) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_w(QUIC_XSO *xso)
{
    return !xso->conn->shutting_down
        && ossl_quic_stream_has_send_buffer(xso->stream)
        && ossl_quic_sstream_get_buffer_avail(xso->stream->sstream)
        && !ossl_quic_sstream_get_final_size(xso->stream->sstream, NULL)
        && ossl_quic_txfc_get_cwm(&xso->stream->txfc)
           > ossl_quic_sstream_get_cur_size(xso->stream->sstream)
        && quic_mutation_allowed(xso->conn, /*req_active=*/1);
}

/* Do we have the EW (exception: write) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_ew(QUIC_XSO *xso)
{
    return ossl_quic_stream_has_send(xso->stream)
        && xso->stream->peer_stop_sending
        && !xso->requested_reset
        && !xso->conn->shutting_down;
}

/* Do we have the EC (exception: connection) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_ec(QUIC_CONNECTION *qc)
{
    return ossl_quic_channel_is_term_any(qc->ch);
}

/* Do we have the ECD (exception: connection drained) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_ecd(QUIC_CONNECTION *qc)
{
    return ossl_quic_channel_is_terminated(qc->ch);
}

/* Do we have the IS (incoming: stream) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_is(QUIC_CONNECTION *qc, int is_uni)
{
    return ossl_quic_stream_map_get_accept_queue_len(ossl_quic_channel_get_qsm(qc->ch),
                                                     is_uni);
}

/* Do we have the OS (outgoing: stream) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_os(QUIC_CONNECTION *qc, int is_uni)
{
    /* Is it currently possible for us to make an outgoing stream? */
    return quic_mutation_allowed(qc, /*req_active=*/1)
        && ossl_quic_channel_get_local_stream_count_avail(qc->ch, is_uni) > 0;
}

/* Do we have the EL (exception: listener) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_el(QUIC_LISTENER *ql)
{
    return !ossl_quic_port_is_running(ql->port);
}

/* Do we have the IC (incoming: connection) condition? */
QUIC_NEEDS_LOCK
static int test_poll_event_ic(QUIC_LISTENER *ql)
{
    return ossl_quic_port_get_num_incoming_channels(ql->port) > 0;
}

QUIC_TAKES_LOCK
int ossl_quic_conn_poll_events(SSL *ssl, uint64_t events, int do_tick,
                               uint64_t *p_revents)
{
    QCTX ctx;
    uint64_t revents = 0;

    if (!expect_quic_csl(ssl, &ctx))
        return 0;

    qctx_lock(&ctx);

    if (ctx.qc != NULL && !ctx.qc->started) {
        /* We can only try to write on non-started connection. */
        if ((events & SSL_POLL_EVENT_W) != 0)
            revents |= SSL_POLL_EVENT_W;
        goto end;
    }

    if (do_tick)
        ossl_quic_reactor_tick(ossl_quic_obj_get0_reactor(ctx.obj), 0);

    if (ctx.xso != NULL) {
        /* SSL object has a stream component. */

        if ((events & SSL_POLL_EVENT_R) != 0
            && test_poll_event_r(ctx.xso))
            revents |= SSL_POLL_EVENT_R;

        if ((events & SSL_POLL_EVENT_ER) != 0
            && test_poll_event_er(ctx.xso))
            revents |= SSL_POLL_EVENT_ER;

        if ((events & SSL_POLL_EVENT_W) != 0
            && test_poll_event_w(ctx.xso))
            revents |= SSL_POLL_EVENT_W;

        if ((events & SSL_POLL_EVENT_EW) != 0
            && test_poll_event_ew(ctx.xso))
            revents |= SSL_POLL_EVENT_EW;
    }

    if (ctx.qc != NULL && !ctx.is_stream) {
        if ((events & SSL_POLL_EVENT_EC) != 0
            && test_poll_event_ec(ctx.qc))
            revents |= SSL_POLL_EVENT_EC;

        if ((events & SSL_POLL_EVENT_ECD) != 0
            && test_poll_event_ecd(ctx.qc))
            revents |= SSL_POLL_EVENT_ECD;

        if ((events & SSL_POLL_EVENT_ISB) != 0
            && test_poll_event_is(ctx.qc, /*uni=*/0))
            revents |= SSL_POLL_EVENT_ISB;

        if ((events & SSL_POLL_EVENT_ISU) != 0
            && test_poll_event_is(ctx.qc, /*uni=*/1))
            revents |= SSL_POLL_EVENT_ISU;

        if ((events & SSL_POLL_EVENT_OSB) != 0
            && test_poll_event_os(ctx.qc, /*uni=*/0))
            revents |= SSL_POLL_EVENT_OSB;

        if ((events & SSL_POLL_EVENT_OSU) != 0
            && test_poll_event_os(ctx.qc, /*uni=*/1))
            revents |= SSL_POLL_EVENT_OSU;
    }

    if (ctx.is_listener) {
        if ((events & SSL_POLL_EVENT_EL) != 0
            && test_poll_event_el(ctx.ql))
            revents |= SSL_POLL_EVENT_EL;

        if ((events & SSL_POLL_EVENT_IC) != 0
            && test_poll_event_ic(ctx.ql))
            revents |= SSL_POLL_EVENT_IC;
    }

 end:
    qctx_unlock(&ctx);
    *p_revents = revents;
    return 1;
}

QUIC_TAKES_LOCK
int ossl_quic_get_notifier_fd(SSL *ssl)
{
    QCTX ctx;
    QUIC_REACTOR *rtor;
    RIO_NOTIFIER *nfy;
    int nfd = -1;

    if (!expect_quic_any(ssl, &ctx))
        return -1;

    qctx_lock(&ctx);
    rtor = ossl_quic_obj_get0_reactor(ctx.obj);
    nfy = ossl_quic_reactor_get0_notifier(rtor);
    if (nfy == NULL)
        goto end;
    nfd = ossl_rio_notifier_as_fd(nfy);

 end:
    qctx_unlock(&ctx);
    return nfd;
}

QUIC_TAKES_LOCK
void ossl_quic_enter_blocking_section(SSL *ssl, QUIC_REACTOR_WAIT_CTX *wctx)
{
    QCTX ctx;
    QUIC_REACTOR *rtor;

    if (!expect_quic_any(ssl, &ctx))
        return;

    qctx_lock(&ctx);
    rtor = ossl_quic_obj_get0_reactor(ctx.obj);
    ossl_quic_reactor_wait_ctx_enter(wctx, rtor);
    qctx_unlock(&ctx);
}

QUIC_TAKES_LOCK
void ossl_quic_leave_blocking_section(SSL *ssl, QUIC_REACTOR_WAIT_CTX *wctx)
{
    QCTX ctx;
    QUIC_REACTOR *rtor;

    if (!expect_quic_any(ssl, &ctx))
        return;

    qctx_lock(&ctx);
    rtor = ossl_quic_obj_get0_reactor(ctx.obj);
    ossl_quic_reactor_wait_ctx_leave(wctx, rtor);
    qctx_unlock(&ctx);
}

/*
 * Internal Testing APIs
 * =====================
 */

QUIC_CHANNEL *ossl_quic_conn_get_channel(SSL *s)
{
    QCTX ctx;

    if (!expect_quic_conn_only(s, &ctx))
        return NULL;

    return ctx.qc->ch;
}

int ossl_quic_set_diag_title(SSL_CTX *ctx, const char *title)
{
#ifndef OPENSSL_NO_QLOG
    OPENSSL_free(ctx->qlog_title);
    ctx->qlog_title = NULL;

    if (title == NULL)
        return 1;

    if ((ctx->qlog_title = OPENSSL_strdup(title)) == NULL)
        return 0;
#endif

    return 1;
}
