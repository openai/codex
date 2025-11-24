/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_THREAD_ASSIST_H
# define OSSL_QUIC_THREAD_ASSIST_H

# include <openssl/ssl.h>
# include "internal/thread.h"
# include "internal/time.h"

# if defined(OPENSSL_NO_QUIC) || defined(OPENSSL_NO_THREAD_POOL)
#  define OPENSSL_NO_QUIC_THREAD_ASSIST
# endif

# ifndef OPENSSL_NO_QUIC_THREAD_ASSIST

/*
 * QUIC Thread Assisted Functionality
 * ==================================
 *
 * Where OS threading support is available, QUIC can optionally support a thread
 * assisted mode of operation. The purpose of this mode of operation is to
 * ensure that assorted timeout events which QUIC expects to be handled in a
 * timely manner can be handled without the application needing to ensure that
 * SSL_tick() is called  on  time. This is not needed if the application always
 * has a call blocking to SSL_read() or SSL_write() (or another I/O function) on
 * a QUIC SSL object, but if the application goes for long periods of time
 * without making any such call to a QUIC SSL object, libssl cannot ordinarily
 * guarantee that QUIC timeout events will be serviced in a timely fashion.
 * Thread assisted  mode is therefore of use to applications which do not always
 * have an ongoing call to an I/O function on a QUIC SSL object but also do not
 * want to have to arrange periodic ticking.
 *
 * A consequence of this is that the intrusiveness of thread assisted mode upon
 * the general architecture of our QUIC engine is actually fairly limited and
 * amounts to an automatic ticking of the QUIC engine when timeouts expire,
 * synchronised correctly with an application's own threads using locking.
 */
typedef struct quic_thread_assist_st {
    QUIC_CHANNEL *ch;
    CRYPTO_CONDVAR *cv;
    CRYPTO_THREAD *t;
    int teardown, joined;
} QUIC_THREAD_ASSIST;

/*
 * Initialise the thread assist object. The channel must have a valid mutex
 * configured on it which will be retrieved automatically. It is assumed that
 * the mutex is currently held when this function is called. This function does
 * not affect the state of the mutex.
 */
int ossl_quic_thread_assist_init_start(QUIC_THREAD_ASSIST *qta,
                                       QUIC_CHANNEL *ch);

/*
 * Request the thread assist helper to begin stopping the assist thread. This
 * returns before the teardown is complete. Idempotent; multiple calls to this
 * function are inconsequential.
 *
 * Precondition: channel mutex must be held (unchecked)
 */
int ossl_quic_thread_assist_stop_async(QUIC_THREAD_ASSIST *qta);

/*
 * Wait until the thread assist helper is torn down. This automatically implies
 * the effects of ossl_quic_thread_assist_stop_async(). Returns immediately
 * if the teardown has already completed.
 *
 * Precondition: channel mutex must be held (unchecked)
 */
int ossl_quic_thread_assist_wait_stopped(QUIC_THREAD_ASSIST *qta);

/*
 * Deallocates state associated with the thread assist helper.
 * ossl_quic_thread_assist_wait_stopped() must have returned successfully before
 * calling this. It does not matter whether the channel mutex is held or not.
 *
 * Precondition: ossl_quic_thread_assist_wait_stopped() has returned 1
 *               (asserted)
 */
int ossl_quic_thread_assist_cleanup(QUIC_THREAD_ASSIST *qta);

/*
 * Must be called to notify the assist thread if the channel deadline changes.
 *
 * Precondition: channel mutex must be held (unchecked)
 */
int ossl_quic_thread_assist_notify_deadline_changed(QUIC_THREAD_ASSIST *qta);

# endif

#endif
