/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "quic_obj_local.h"
#include "quic_local.h"
#include "internal/ssl_unwrap.h"

static int obj_update_cache(QUIC_OBJ *obj);

int ossl_quic_obj_init(QUIC_OBJ *obj,
                       SSL_CTX *ctx,
                       int type,
                       SSL *parent_obj,
                       QUIC_ENGINE *engine,
                       QUIC_PORT *port)
{
    int is_event_leader = (engine != NULL);
    int is_port_leader  = (port != NULL);

    if (!ossl_assert(obj != NULL && !obj->init_done && SSL_TYPE_IS_QUIC(type)
                     && (parent_obj == NULL || IS_QUIC(parent_obj))))
        return 0;

    /* Event leader is always the root object. */
    if (!ossl_assert(!is_event_leader || parent_obj == NULL))
        return 0;

    if (!ossl_ssl_init(&obj->ssl, ctx, ctx->method, type))
        goto err;

    obj->domain_flags       = ctx->domain_flags;
    obj->parent_obj         = (QUIC_OBJ *)parent_obj;
    obj->is_event_leader    = is_event_leader;
    obj->is_port_leader     = is_port_leader;
    obj->engine             = engine;
    obj->port               = port;
    obj->req_blocking_mode  = QUIC_BLOCKING_MODE_INHERIT;
    if (!obj_update_cache(obj))
        goto err;

    obj->init_done          = 1;
    return 1;

err:
    obj->is_event_leader = 0;
    obj->is_port_leader  = 0;
    return 0;
}

static int obj_update_cache(QUIC_OBJ *obj)
{
    QUIC_OBJ *p;

    for (p = obj; p != NULL && !p->is_event_leader;
         p = p->parent_obj)
        if (!ossl_assert(p == obj || p->init_done))
            return 0;

    if (!ossl_assert(p != NULL))
        return 0;

    /*
     * Offset of ->ssl is guaranteed to be 0 but the NULL check makes ubsan
     * happy.
     */
    obj->cached_event_leader    = p;
    obj->engine                 = p->engine;

    for (p = obj; p != NULL && !p->is_port_leader;
         p = p->parent_obj);

    obj->cached_port_leader     = p;
    obj->port                   = (p != NULL) ? p->port : NULL;
    return 1;
}

SSL_CONNECTION *ossl_quic_obj_get0_handshake_layer(QUIC_OBJ *obj)
{
    assert(obj != NULL && obj->init_done);

    if (obj->ssl.type != SSL_TYPE_QUIC_CONNECTION)
        return NULL;

    return SSL_CONNECTION_FROM_SSL_ONLY(((QUIC_CONNECTION *)obj)->tls);
}

/* (Returns a cached result.) */
int ossl_quic_obj_can_support_blocking(const QUIC_OBJ *obj)
{
    QUIC_REACTOR *rtor;

    assert(obj != NULL);
    rtor = ossl_quic_obj_get0_reactor(obj);

    if ((obj->domain_flags
            & (SSL_DOMAIN_FLAG_LEGACY_BLOCKING | SSL_DOMAIN_FLAG_BLOCKING)) == 0)
        return 0;

    return ossl_quic_reactor_can_poll_r(rtor)
        || ossl_quic_reactor_can_poll_w(rtor);
}

int ossl_quic_obj_desires_blocking(const QUIC_OBJ *obj)
{
    unsigned int req_blocking_mode;

    assert(obj != NULL);
    for (; (req_blocking_mode = obj->req_blocking_mode) == QUIC_BLOCKING_MODE_INHERIT
           && obj->parent_obj != NULL; obj = obj->parent_obj);

    return req_blocking_mode != QUIC_BLOCKING_MODE_NONBLOCKING;
}

int ossl_quic_obj_blocking(const QUIC_OBJ *obj)
{
    assert(obj != NULL);

    if (!ossl_quic_obj_desires_blocking(obj))
        return 0;

    ossl_quic_engine_update_poll_descriptors(ossl_quic_obj_get0_engine(obj),
                                             /*force=*/0);
    return ossl_quic_obj_can_support_blocking(obj);
}

void ossl_quic_obj_set_blocking_mode(QUIC_OBJ *obj, unsigned int mode)
{
    assert(obj != NULL);

    obj->req_blocking_mode = mode;
}
