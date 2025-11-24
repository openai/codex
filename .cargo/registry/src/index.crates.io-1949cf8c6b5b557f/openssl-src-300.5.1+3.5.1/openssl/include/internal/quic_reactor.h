/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_QUIC_REACTOR_H
# define OSSL_QUIC_REACTOR_H

# include "internal/time.h"
# include "internal/sockets.h"
# include "internal/quic_predef.h"
# include "internal/thread_arch.h"
# include "internal/rio_notifier.h"
# include <openssl/bio.h>

# ifndef OPENSSL_NO_QUIC

/*
 * Core I/O Reactor Framework
 * ==========================
 *
 * Manages use of async network I/O which the QUIC stack is built on. The core
 * mechanic looks like this:
 *
 *   - There is a pollable FD for both the read and write side respectively.
 *     Readability and writeability of these FDs respectively determines when
 *     network I/O is available.
 *
 *   - The reactor can export these FDs to the user, as well as flags indicating
 *     whether the user should listen for readability, writeability, or neither.
 *
 *   - The reactor can export a timeout indication to the user, indicating when
 *     the reactor should be called (via libssl APIs) regardless of whether
 *     the network socket has become ready.
 *
 * The reactor is based around a tick callback which is essentially the mutator
 * function. The mutator attempts to do whatever it can, attempting to perform
 * network I/O to the extent currently feasible. When done, the mutator returns
 * information to the reactor indicating when it should be woken up again:
 *
 *   - Should it be woken up when network RX is possible?
 *   - Should it be woken up when network TX is possible?
 *   - Should it be woken up no later than some deadline X?
 *
 * The intention is that ALL I/O-related SSL_* functions with side effects (e.g.
 * SSL_read/SSL_write) consist of three phases:
 *
 *   - Optionally mutate the QUIC machine's state.
 *   - Optionally tick the QUIC reactor.
 *   - Optionally mutate the QUIC machine's state.
 *
 * For example, SSL_write is a mutation (appending to a stream buffer) followed
 * by an optional tick (generally expected as we may want to send the data
 * immediately, though not strictly needed if transmission is being deferred due
 * to Nagle's algorithm, etc.).
 *
 * SSL_read is also a mutation and in principle does not need to tick the
 * reactor, but it generally will anyway to ensure that the reactor is regularly
 * ticked by an application which is only reading and not writing.
 *
 * If the SSL object is being used in blocking mode, SSL_read may need to block
 * if no data is available yet, and SSL_write may need to block if buffers
 * are full.
 *
 * The internals of the QUIC I/O engine always use asynchronous I/O. If the
 * application desires blocking semantics, we handle this by adding a blocking
 * adaptation layer on top of our internal asynchronous I/O API as exposed by
 * the reactor interface.
 */
struct quic_tick_result_st {
    OSSL_TIME   tick_deadline;
    char        net_read_desired;
    char        net_write_desired;
    char        notify_other_threads;
};

static ossl_inline ossl_unused void
ossl_quic_tick_result_merge_into(QUIC_TICK_RESULT *r,
                                 const QUIC_TICK_RESULT *src)
{
    r->net_read_desired     = r->net_read_desired  || src->net_read_desired;
    r->net_write_desired    = r->net_write_desired || src->net_write_desired;
    r->notify_other_threads = r->notify_other_threads || src->notify_other_threads;
    r->tick_deadline        = ossl_time_min(r->tick_deadline, src->tick_deadline);
}

struct quic_reactor_st {
    /*
     * BIO poll descriptors which can be polled. poll_r is a poll descriptor
     * which becomes readable when the QUIC state machine can potentially do
     * work, and poll_w is a poll descriptor which becomes writable when the
     * QUIC state machine can potentially do work. Generally, either of these
     * conditions means that SSL_tick() should be called, or another SSL
     * function which implicitly calls SSL_tick() (e.g. SSL_read/SSL_write()).
     */
    BIO_POLL_DESCRIPTOR poll_r, poll_w;
    OSSL_TIME tick_deadline; /* ossl_time_infinite() if none currently applicable */

    void (*tick_cb)(QUIC_TICK_RESULT *res, void *arg, uint32_t flags);
    void *tick_cb_arg;

    /* The mutex used for ticking. Not owned by the reactor. */
    CRYPTO_MUTEX *mutex;

    /* Used to notify other threads. Valid only if have_notifier is set. */
    RIO_NOTIFIER notifier;

    /*
     * Condvar to assist synchronising use of the notifier. Valid only if
     * have_notifier is set.
     */
    CRYPTO_CONDVAR *notifier_cv;

    /*
     * Count of the current number of blocking waiters. Like everything else,
     * this is protected by the caller's mutex (i.e., the engine mutex).
     */
    size_t cur_blocking_waiters;

    /*
     * These are true if we would like to know when we can read or write from
     * the network respectively.
     */
    unsigned int net_read_desired   : 1;
    unsigned int net_write_desired  : 1;

    /*
     * Are the read and write poll descriptors we are currently configured with
     * things we can actually poll?
     */
    unsigned int can_poll_r : 1;
    unsigned int can_poll_w : 1;

    /* 1 if notifier is present and initialised. */
    unsigned int have_notifier : 1;

    /* 1 if a block_until_pred call has put the notifier in the signalled state. */
    unsigned int signalled_notifier : 1;
};

/* Create an OS notifier? */
#define QUIC_REACTOR_FLAG_USE_NOTIFIER      (1U << 0)

int ossl_quic_reactor_init(QUIC_REACTOR *rtor,
                           void (*tick_cb)(QUIC_TICK_RESULT *res, void *arg,
                                           uint32_t flags),
                           void *tick_cb_arg,
                           CRYPTO_MUTEX *mutex,
                           OSSL_TIME initial_tick_deadline,
                           uint64_t flags);

void ossl_quic_reactor_cleanup(QUIC_REACTOR *rtor);

void ossl_quic_reactor_set_poll_r(QUIC_REACTOR *rtor,
                                  const BIO_POLL_DESCRIPTOR *r);

void ossl_quic_reactor_set_poll_w(QUIC_REACTOR *rtor,
                                  const BIO_POLL_DESCRIPTOR *w);

const BIO_POLL_DESCRIPTOR *ossl_quic_reactor_get_poll_r(const QUIC_REACTOR *rtor);
const BIO_POLL_DESCRIPTOR *ossl_quic_reactor_get_poll_w(const QUIC_REACTOR *rtor);

int ossl_quic_reactor_can_poll_r(const QUIC_REACTOR *rtor);
int ossl_quic_reactor_can_poll_w(const QUIC_REACTOR *rtor);

int ossl_quic_reactor_can_support_poll_descriptor(const QUIC_REACTOR *rtor,
                                                  const BIO_POLL_DESCRIPTOR *d);

int ossl_quic_reactor_net_read_desired(QUIC_REACTOR *rtor);
int ossl_quic_reactor_net_write_desired(QUIC_REACTOR *rtor);

OSSL_TIME ossl_quic_reactor_get_tick_deadline(QUIC_REACTOR *rtor);

/*
 * Do whatever work can be done, and as much work as can be done. This involves
 * e.g. seeing if we can read anything from the network (if we want to), seeing
 * if we can write anything to the network (if we want to), etc.
 *
 * If the CHANNEL_ONLY flag is set, this indicates that we should only
 * touch state which is synchronised by the channel mutex.
 */
#define QUIC_REACTOR_TICK_FLAG_CHANNEL_ONLY  (1U << 0)

int ossl_quic_reactor_tick(QUIC_REACTOR *rtor, uint32_t flags);

RIO_NOTIFIER *ossl_quic_reactor_get0_notifier(QUIC_REACTOR *rtor);

/*
 * Blocking I/O Adaptation Layer
 * =============================
 *
 * The blocking I/O adaptation layer implements blocking I/O on top of our
 * asynchronous core.
 */

/*
 * ossl_quic_reactor_block_until_pred
 * ----------------------------------
 *
 * The core mechanism of the Blocking I/O Adaption Layer is block_until_pred(),
 * which does not return until pred() returns a value other than 0. The blocker
 * uses OS I/O synchronisation primitives (e.g. poll(2)) and ticks the reactor
 * until the predicate is satisfied. The blocker is not required to call pred()
 * more than once between tick calls.
 *
 * When pred returns a non-zero value, that value is returned by this function.
 * This can be used to allow pred() to indicate error conditions and short
 * circuit the blocking process.
 *
 * A return value of -1 is reserved for network polling errors. Therefore this
 * return value should not be used by pred() if ambiguity is not desired. Note
 * that the predicate function can always arrange its own output mechanism, for
 * example by passing a structure of its own as the argument.
 *
 * If the SKIP_FIRST_TICK flag is set, the first call to reactor_tick() before
 * the first call to pred() is skipped. This is useful if it is known that
 * ticking the reactor again will not be useful (e.g. because it has already
 * been done).
 *
 * This function assumes a write lock is held for the entire QUIC_CHANNEL. If
 * mutex is non-NULL, it must be a lock currently held for write; it will be
 * unlocked during any sleep, and then relocked for write afterwards.
 *
 * This function must not be called by a thread currently using
 * ossl_quic_reactor_(enter/leave)_blocking_section() as this function also uses
 * those functions (see below); it is assumed if a caller is using those
 * functions it is implementing blocking semantics itself. There is no need to
 * use those functions if using this function.
 *
 * Precondition:   If a reactor mutex is being used, it must be held (unchecked)
 * Postcondition:  If a reactor mutex is being used, it is held
 * Invariant:      The current thread does not have an outstanding
 *                   ossl_quic_reactor_enter_blocking_section() call (unchecked)
 */
#define SKIP_FIRST_TICK     (1U << 0)

int ossl_quic_reactor_block_until_pred(QUIC_REACTOR *rtor,
                                       int (*pred)(void *arg), void *pred_arg,
                                       uint32_t flags);

/*
 * ossl_quic_reactor_(enter/leave)_blocking_section
 * ------------------------------------------------
 *
 * This is used by blocking code outside of the reactor itself to inform the
 * reactor of when a thread begins or ends a blocking call. This is used by the
 * reactor so it knows if a tick means other threads might need to be woken up
 * via the notifier. The reactor mutex must be held while calling these
 * functions.
 *
 * The number of 'active' calls to these functions (i.e., the number of enter
 * calls which have yet to be matched with a subsequent leave call) must *at all
 * times* equal the number of threads blocking on the reactor. In other words, a
 * single thread is not permitted to use these functions "recursively". Failing
 * to adhere to this rule will result in deadlock.
 *
 * This means that if a caller has the concept of multiple concurrent blocking
 * calls on the same thread on the same reactor (which may occur in some
 * SSL_poll-related circumstances) it must do its own housekeeping to ensure it
 * only calls enter() once. See quic_reactor_wait_ctx.h for a utility which can
 * be used to accomplish this.
 *
 * ossl_quic_reactor_enter_blocking_section:
 *   Precondition:   The current thread does not have an outstanding
 *                     ossl_quic_reactor_enter_blocking_section() call (unchecked)
 *   Postcondition:  The current thread has an outstanding
 *                     ossl_quic_reactor_enter_blocking_section() call
 *
 * ossl_quic_reactor_leave_blocking_section:
 *   Precondition:   The current thread has an outstanding
 *                     ossl_quic_reactor_enter_blocking_section() call (unchecked)
 *   Postcondition:  The current thread does not have an outstanding
 *                     ossl_quic_reactor_enter_blocking_section() call
 *
 */
void ossl_quic_reactor_enter_blocking_section(QUIC_REACTOR *rtor);
void ossl_quic_reactor_leave_blocking_section(QUIC_REACTOR *rtor);

# endif

#endif
