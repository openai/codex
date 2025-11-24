/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_engine.h"
#include "internal/quic_port.h"
#include "quic_engine_local.h"
#include "quic_port_local.h"
#include "../ssl_local.h"

/*
 * QUIC Engine
 * ===========
 */
static int qeng_init(QUIC_ENGINE *qeng, uint64_t reactor_flags);
static void qeng_cleanup(QUIC_ENGINE *qeng);
static void qeng_tick(QUIC_TICK_RESULT *res, void *arg, uint32_t flags);

DEFINE_LIST_OF_IMPL(port, QUIC_PORT);

QUIC_ENGINE *ossl_quic_engine_new(const QUIC_ENGINE_ARGS *args)
{
    QUIC_ENGINE *qeng;

    if ((qeng = OPENSSL_zalloc(sizeof(QUIC_ENGINE))) == NULL)
        return NULL;

    qeng->libctx            = args->libctx;
    qeng->propq             = args->propq;
    qeng->mutex             = args->mutex;

    if (!qeng_init(qeng, args->reactor_flags)) {
        OPENSSL_free(qeng);
        return NULL;
    }

    return qeng;
}

void ossl_quic_engine_free(QUIC_ENGINE *qeng)
{
    if (qeng == NULL)
        return;

    qeng_cleanup(qeng);
    OPENSSL_free(qeng);
}

static int qeng_init(QUIC_ENGINE *qeng, uint64_t reactor_flags)
{
    return ossl_quic_reactor_init(&qeng->rtor, qeng_tick, qeng,
                                  qeng->mutex,
                                  ossl_time_zero(), reactor_flags);
}

static void qeng_cleanup(QUIC_ENGINE *qeng)
{
    assert(ossl_list_port_num(&qeng->port_list) == 0);
    ossl_quic_reactor_cleanup(&qeng->rtor);
}

QUIC_REACTOR *ossl_quic_engine_get0_reactor(QUIC_ENGINE *qeng)
{
    return &qeng->rtor;
}

CRYPTO_MUTEX *ossl_quic_engine_get0_mutex(QUIC_ENGINE *qeng)
{
    return qeng->mutex;
}

OSSL_TIME ossl_quic_engine_get_time(QUIC_ENGINE *qeng)
{
    if (qeng->now_cb == NULL)
        return ossl_time_now();

    return qeng->now_cb(qeng->now_cb_arg);
}

OSSL_TIME ossl_quic_engine_make_real_time(QUIC_ENGINE *qeng, OSSL_TIME tm)
{
    OSSL_TIME offset;

    if (qeng->now_cb != NULL
            && !ossl_time_is_zero(tm)
            && !ossl_time_is_infinite(tm)) {

        offset = qeng->now_cb(qeng->now_cb_arg);

        /* If tm is earlier than offset then tm will end up as "now" */
        tm = ossl_time_add(ossl_time_subtract(tm, offset), ossl_time_now());
    }

    return tm;
}

void ossl_quic_engine_set_time_cb(QUIC_ENGINE *qeng,
                                  OSSL_TIME (*now_cb)(void *arg),
                                  void *now_cb_arg)
{
    qeng->now_cb = now_cb;
    qeng->now_cb_arg = now_cb_arg;
}

void ossl_quic_engine_set_inhibit_tick(QUIC_ENGINE *qeng, int inhibit)
{
    qeng->inhibit_tick = (inhibit != 0);
}

OSSL_LIB_CTX *ossl_quic_engine_get0_libctx(QUIC_ENGINE *qeng)
{
    return qeng->libctx;
}

const char *ossl_quic_engine_get0_propq(QUIC_ENGINE *qeng)
{
    return qeng->propq;
}

void ossl_quic_engine_update_poll_descriptors(QUIC_ENGINE *qeng, int force)
{
    QUIC_PORT *port;

    /*
     * TODO(QUIC MULTIPORT): The implementation of
     * ossl_quic_port_update_poll_descriptors assumes an engine only ever has a
     * single port for now due to reactor limitations. This limitation will be
     * removed in future.
     *
     * TODO(QUIC MULTIPORT): Consider only iterating the port list when dirty at
     * the engine level in future when we can have multiple ports. This is not
     * important currently as the port list has a single entry.
     */
    OSSL_LIST_FOREACH(port, port, &qeng->port_list)
        ossl_quic_port_update_poll_descriptors(port, force);
}

/*
 * QUIC Engine: Child Object Lifecycle Management
 * ==============================================
 */

QUIC_PORT *ossl_quic_engine_create_port(QUIC_ENGINE *qeng,
                                        const QUIC_PORT_ARGS *args)
{
    QUIC_PORT_ARGS largs = *args;

    if (ossl_list_port_num(&qeng->port_list) > 0)
        /* TODO(QUIC MULTIPORT): We currently support only one port. */
        return NULL;

    if (largs.engine != NULL)
        return NULL;

    largs.engine = qeng;
    return ossl_quic_port_new(&largs);
}

/*
 * QUIC Engine: Ticker-Mutator
 * ===========================
 */

/*
 * The central ticker function called by the reactor. This does everything, or
 * at least everything network I/O related. Best effort - not allowed to fail
 * "loudly".
 */
static void qeng_tick(QUIC_TICK_RESULT *res, void *arg, uint32_t flags)
{
    QUIC_ENGINE *qeng = arg;
    QUIC_PORT *port;

    res->net_read_desired     = 0;
    res->net_write_desired    = 0;
    res->notify_other_threads = 0;
    res->tick_deadline        = ossl_time_infinite();

    if (qeng->inhibit_tick)
        return;

    /* Iterate through all ports and service them. */
    OSSL_LIST_FOREACH(port, port, &qeng->port_list) {
        QUIC_TICK_RESULT subr = {0};

        ossl_quic_port_subtick(port, &subr, flags);
        ossl_quic_tick_result_merge_into(res, &subr);
    }
}
