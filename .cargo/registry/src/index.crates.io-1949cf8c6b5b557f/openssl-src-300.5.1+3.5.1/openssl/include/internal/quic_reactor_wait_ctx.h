/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_QUIC_REACTOR_WAIT_CTX_H
# define OSSL_QUIC_REACTOR_WAIT_CTX_H

# include "internal/quic_predef.h"
# include "internal/quic_reactor.h"
# include "internal/list.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC_REACTOR_WAIT_CTX
 * =====================
 *
 * In order to support inter-thread notification of events which may cause a
 * blocking call on another thread to be able to make forward progress, we need
 * to know when a thread enters and exits a blocking call. The details of why
 * this is involves subtleties of inter-thread synchronisation and a detailed
 * discussion can be found in the source of
 * ossl_quic_reactor_block_until_pred().
 *
 * The core mechanism for such tracking is
 * ossl_quic_reactor_(enter/leave)_blocking_section(), however this API does not
 * support recursive usage to keep the internal implementation simple. In some
 * cases, an API which can be used in a recursive fashion (with multiple
 * balanced calls to enter()/leave() on a single thread) is more convenient.
 *
 * This utility allows multiple blocking operations to be registered on a given
 * thread. Moreover, it allows multiple blocking operations to be registered
 * across an arbitrarily large number of QUIC_REACTORs from a given thread.
 *
 * In short, this allows multiple 'concurrent' blocking calls to be ongoing on a
 * given thread for a given reactor. While on the face of it the notion of
 * multiple concurrent blocking calls on a single thread makes no sense, the
 * implementation of our immediate-mode polling implementation (SSL_poll) makes
 * it natural for us to implement it simply by registering a blocking call per
 * SSL object passed in. Since multiple SSL objects may be passed to an SSL_poll
 * call, and some SSL objects may correspond to the same reactor, and other SSL
 * objects may correspond to a different reactor, we need to be able to
 * determine when a SSL_poll() call has finished with all of the SSL objects
 * *corresponding to a given reactor*.
 *
 * Doing this requires some ephemeral state tracking as a SSL_poll() call may
 * map to an arbitrarily large set of reactor objects. For now, we track this
 * separately from the reactor code as the state needed is only ephemeral and
 * this keeps the reactor internals simple.
 *
 * The concept used is that a thread allocates (on the stack) a
 * QUIC_REACTOR_WAIT_CTX before commencing a blocking operation, and then calls
 * ossl_quic_reactor_wait_ctx_enter() whenever encountering a reactor involved
 * in the imminent blocking operation. Later it must ensure it calls
 * ossl_quic_reactor_wait_ctx_leave() the same number of times for each reactor.
 * enter() and leave() may be called multiple times for the same reactor and
 * wait context so long as the number of calls is balanced. The last leave()
 * call for a given thread's wait context *and a given reactor* causes that
 * reactor to do the inter-thread notification housekeeping needed for
 * multithreaded blocking to work correctly.
 *
 * The gist is that a simple reactor-level counter of active concurrent blocking
 * calls across all threads is not accurate and we need an accurate count of how
 * many 'concurrent' blocking calls for a given reactor are active *on a given
 * thread* in order to avoid deadlocks. Conceptually, you can think of this as
 * refcounting a refcount (which is actually how it is implemented).
 *
 * Logically, a wait context is a map from a reactor pointer (i.e., a unique
 * identifier for the reactor) to the number of 'recursive' calls outstanding:
 *
 *   (QUIC_REACTOR *) -> (outstanding call count)
 *
 * When the count for a reactor transitions from 0 to a nonzero value, or vice
 * versa, ossl_quic_reactor_(enter/leave)_blocking_section() is called once.
 *
 * The internal implementation is based on linked lists as we expect the actual
 * number of reactors involved in a given blocking operation to be very small,
 * so spinning up a hashtable is not worthwhile.
 */
typedef struct quic_reactor_wait_slot_st QUIC_REACTOR_WAIT_SLOT;

DECLARE_LIST_OF(quic_reactor_wait_slot, QUIC_REACTOR_WAIT_SLOT);

struct quic_reactor_wait_ctx_st {
    OSSL_LIST(quic_reactor_wait_slot) slots;
};

/* Initialises a wait context. */
void ossl_quic_reactor_wait_ctx_init(QUIC_REACTOR_WAIT_CTX *ctx);

/* Uprefs a given reactor. */
int ossl_quic_reactor_wait_ctx_enter(QUIC_REACTOR_WAIT_CTX *ctx,
                                     QUIC_REACTOR *rtor);

/* Downrefs a given reactor. */
void ossl_quic_reactor_wait_ctx_leave(QUIC_REACTOR_WAIT_CTX *ctx,
                                      QUIC_REACTOR *rtor);

/*
 * Destroys a wait context. Must be called after calling init().
 *
 * Precondition: All prior calls to ossl_quic_reactor_wait_ctx_enter() must have
 * been balanced with corresponding leave() calls before calling this
 * (unchecked).
 */
void ossl_quic_reactor_wait_ctx_cleanup(QUIC_REACTOR_WAIT_CTX *ctx);

# endif

#endif
