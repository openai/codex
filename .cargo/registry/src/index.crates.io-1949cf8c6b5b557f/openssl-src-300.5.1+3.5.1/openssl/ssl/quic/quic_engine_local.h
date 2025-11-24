/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_ENGINE_LOCAL_H
# define OSSL_QUIC_ENGINE_LOCAL_H

# include "internal/quic_engine.h"
# include "internal/quic_reactor.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Engine Structure
 * =====================
 *
 * QUIC engine internals. It is intended that only the QUIC_ENGINE, QUIC_PORT
 * and QUIC_CHANNEL implementations be allowed to access this structure
 * directly.
 *
 * Other components should not include this header.
 */
DECLARE_LIST_OF(port, QUIC_PORT);

struct quic_engine_st {
    /* All objects in a QUIC event domain share the same (libctx, propq). */
    OSSL_LIB_CTX                    *libctx;
    const char                      *propq;

    /*
     * Master synchronisation mutex for the entire QUIC event domain. Used for
     * thread assisted mode synchronisation. We don't own this; the instantiator
     * of the engine passes it to us and is responsible for freeing it after
     * engine destruction.
     */
    CRYPTO_MUTEX                    *mutex;

    /* Callback used to get the current time. */
    OSSL_TIME                       (*now_cb)(void *arg);
    void                            *now_cb_arg;

    /* Asynchronous I/O reactor. */
    QUIC_REACTOR                    rtor;

    /* List of all child ports. */
    OSSL_LIST(port)                 port_list;

    /* Inhibit tick for testing purposes? */
    unsigned int                    inhibit_tick                    : 1;
};

# endif

#endif
