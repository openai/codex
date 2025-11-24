/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/macros.h>
#include "quic_local.h"
#include "internal/time.h"
#include "internal/thread.h"
#include "internal/thread_arch.h"
#include "internal/quic_thread_assist.h"

#if !defined(OPENSSL_NO_QUIC_THREAD_ASSIST)

/* Main loop for the QUIC assist thread. */
static unsigned int assist_thread_main(void *arg)
{
    QUIC_THREAD_ASSIST *qta = arg;
    CRYPTO_MUTEX *m = ossl_quic_channel_get_mutex(qta->ch);
    QUIC_REACTOR *rtor;
    QUIC_ENGINE *eng = ossl_quic_channel_get0_engine(qta->ch);

    ossl_crypto_mutex_lock(m);

    rtor = ossl_quic_channel_get_reactor(qta->ch);

    for (;;) {
        OSSL_TIME deadline;

        if (qta->teardown)
            break;

        deadline = ossl_quic_reactor_get_tick_deadline(rtor);
        /*
         * ossl_crypto_condvar_wait_timeout needs to use real time for the
         * deadline
         */
        deadline = ossl_quic_engine_make_real_time(eng, deadline);
        ossl_crypto_condvar_wait_timeout(qta->cv, m, deadline);

        /*
         * We have now been woken up. This can be for one of the following
         * reasons:
         *
         *   - We have been asked to teardown (qta->teardown is set);
         *   - The tick deadline has passed.
         *   - The tick deadline has changed.
         *
         * For robustness, this loop also handles spurious wakeups correctly
         * (which does not require any extra code).
         */
        if (qta->teardown)
            break;

        ossl_quic_reactor_tick(rtor, QUIC_REACTOR_TICK_FLAG_CHANNEL_ONLY);
    }

    ossl_crypto_mutex_unlock(m);
    return 1;
}

int ossl_quic_thread_assist_init_start(QUIC_THREAD_ASSIST *qta,
                                       QUIC_CHANNEL *ch)
{
    CRYPTO_MUTEX *mutex = ossl_quic_channel_get_mutex(ch);

    if (mutex == NULL)
        return 0;

    qta->ch         = ch;
    qta->teardown   = 0;
    qta->joined     = 0;

    qta->cv = ossl_crypto_condvar_new();
    if (qta->cv == NULL)
        return 0;

    qta->t = ossl_crypto_thread_native_start(assist_thread_main,
                                             qta, /*joinable=*/1);
    if (qta->t == NULL) {
        ossl_crypto_condvar_free(&qta->cv);
        return 0;
    }

    return 1;
}

int ossl_quic_thread_assist_stop_async(QUIC_THREAD_ASSIST *qta)
{
    if (!qta->teardown) {
        qta->teardown = 1;
        ossl_crypto_condvar_signal(qta->cv);
    }

    return 1;
}

int ossl_quic_thread_assist_wait_stopped(QUIC_THREAD_ASSIST *qta)
{
    CRYPTO_THREAD_RETVAL rv;
    CRYPTO_MUTEX *m = ossl_quic_channel_get_mutex(qta->ch);

    if (qta->joined)
        return 1;

    if (!ossl_quic_thread_assist_stop_async(qta))
        return 0;

    ossl_crypto_mutex_unlock(m);

    if (!ossl_crypto_thread_native_join(qta->t, &rv)) {
        ossl_crypto_mutex_lock(m);
        return 0;
    }

    qta->joined = 1;

    ossl_crypto_mutex_lock(m);
    return 1;
}

int ossl_quic_thread_assist_cleanup(QUIC_THREAD_ASSIST *qta)
{
    if (!ossl_assert(qta->joined))
        return 0;

    ossl_crypto_condvar_free(&qta->cv);
    ossl_crypto_thread_native_clean(qta->t);

    qta->ch     = NULL;
    qta->t      = NULL;
    return 1;
}

int ossl_quic_thread_assist_notify_deadline_changed(QUIC_THREAD_ASSIST *qta)
{
    if (qta->teardown)
        return 0;

    ossl_crypto_condvar_signal(qta->cv);
    return 1;
}

#endif
