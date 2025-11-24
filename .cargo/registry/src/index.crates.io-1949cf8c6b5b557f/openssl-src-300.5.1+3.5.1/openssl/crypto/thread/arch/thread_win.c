/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <internal/thread_arch.h>

#if defined(OPENSSL_THREADS_WINNT)
# include <process.h>
# include <windows.h>

static unsigned __stdcall thread_start_thunk(LPVOID vthread)
{
    CRYPTO_THREAD *thread;
    CRYPTO_THREAD_RETVAL ret;

    thread = (CRYPTO_THREAD *)vthread;

    thread->thread_id = GetCurrentThreadId();

    ret = thread->routine(thread->data);
    ossl_crypto_mutex_lock(thread->statelock);
    CRYPTO_THREAD_SET_STATE(thread, CRYPTO_THREAD_FINISHED);
    thread->retval = ret;
    ossl_crypto_condvar_signal(thread->condvar);
    ossl_crypto_mutex_unlock(thread->statelock);

    return 0;
}

int ossl_crypto_thread_native_spawn(CRYPTO_THREAD *thread)
{
    HANDLE *handle;

    handle = OPENSSL_zalloc(sizeof(*handle));
    if (handle == NULL)
        goto fail;

    *handle = (HANDLE)_beginthreadex(NULL, 0, &thread_start_thunk, thread, 0, NULL);
    if (*handle == NULL)
        goto fail;

    thread->handle = handle;
    return 1;

fail:
    thread->handle = NULL;
    OPENSSL_free(handle);
    return 0;
}

int ossl_crypto_thread_native_perform_join(CRYPTO_THREAD *thread, CRYPTO_THREAD_RETVAL *retval)
{
    DWORD thread_retval;
    HANDLE *handle;

    if (thread == NULL || thread->handle == NULL)
        return 0;

    handle = (HANDLE *) thread->handle;
    if (WaitForSingleObject(*handle, INFINITE) != WAIT_OBJECT_0)
        return 0;

    if (GetExitCodeThread(*handle, &thread_retval) == 0)
        return 0;

    /*
     * GetExitCodeThread call followed by this check is to make sure that
     * the thread exited properly. In particular, thread_retval may be
     * non-zero when exited via explicit ExitThread/TerminateThread or
     * if the thread is still active (returns STILL_ACTIVE (259)).
     */
    if (thread_retval != 0)
        return 0;

    if (CloseHandle(*handle) == 0)
        return 0;

    return 1;
}

int ossl_crypto_thread_native_exit(void)
{
    _endthreadex(0);
    return 1;
}

int ossl_crypto_thread_native_is_self(CRYPTO_THREAD *thread)
{
    return thread->thread_id == GetCurrentThreadId();
}

CRYPTO_MUTEX *ossl_crypto_mutex_new(void)
{
    CRITICAL_SECTION *mutex;

    if ((mutex = OPENSSL_zalloc(sizeof(*mutex))) == NULL)
        return NULL;
    InitializeCriticalSection(mutex);
    return (CRYPTO_MUTEX *)mutex;
}

void ossl_crypto_mutex_lock(CRYPTO_MUTEX *mutex)
{
    CRITICAL_SECTION *mutex_p;

    mutex_p = (CRITICAL_SECTION *)mutex;
    EnterCriticalSection(mutex_p);
}

int ossl_crypto_mutex_try_lock(CRYPTO_MUTEX *mutex)
{
    CRITICAL_SECTION *mutex_p;

    mutex_p = (CRITICAL_SECTION *)mutex;
    if (TryEnterCriticalSection(mutex_p))
        return 1;

    return 0;
}

void ossl_crypto_mutex_unlock(CRYPTO_MUTEX *mutex)
{
    CRITICAL_SECTION *mutex_p;

    mutex_p = (CRITICAL_SECTION *)mutex;
    LeaveCriticalSection(mutex_p);
}

void ossl_crypto_mutex_free(CRYPTO_MUTEX **mutex)
{
    CRITICAL_SECTION **mutex_p;

    mutex_p = (CRITICAL_SECTION **)mutex;
    if (*mutex_p != NULL)
        DeleteCriticalSection(*mutex_p);
    OPENSSL_free(*mutex_p);
    *mutex = NULL;
}

static int determine_timeout(OSSL_TIME deadline, DWORD *w_timeout_p)
{
    OSSL_TIME now, delta;
    uint64_t ms;

    if (ossl_time_is_infinite(deadline)) {
        *w_timeout_p = INFINITE;
        return 1;
    }

    now = ossl_time_now();
    delta = ossl_time_subtract(deadline, now);

    if (ossl_time_is_zero(delta))
        return 0;

    ms = ossl_time2ms(delta);

    /*
     * Amount of time we want to wait is too long for the 32-bit argument to
     * the Win32 API, so just wait as long as possible.
     */
    if (ms > (uint64_t)(INFINITE - 1))
        *w_timeout_p = INFINITE - 1;
    else
        *w_timeout_p = (DWORD)ms;

    return 1;
}

# if defined(OPENSSL_THREADS_WINNT_LEGACY)
#  include <assert.h>

/*
 * Win32, before Vista, did not have an OS-provided condition variable
 * construct. This leads to the need to construct our own condition variable
 * construct in order to support Windows XP.
 *
 * It is difficult to construct a condition variable construct using the
 * OS-provided primitives in a way that is both correct (avoiding race
 * conditions where broadcasts get lost) and fair.
 *
 * CORRECTNESS:
 *   A blocked thread is a thread which is calling wait(), between the
 *   precise instants at which the external mutex passed to wait() is
 *   unlocked and the instant at which it is relocked.
 *
 *   a)
 *     - If broadcast() is called, ALL blocked threads MUST be unblocked.
 *     - If signal() is called, at least one blocked thread MUST be unblocked.
 *
 *     (i.e.: a signal or broadcast must never get 'lost')
 *
 *   b)
 *     - If broadcast() or signal() is called, this must not cause a thread
 *       which is not blocked to return immediately from a subsequent
 *       call to wait().
 *
 * FAIRNESS:
 *   If broadcast() is called at time T1, all blocked threads must be unblocked
 *   before any thread which subsequently calls wait() at time T2 > T1 is
 *   unblocked.
 *
 *   An example of an implementation which lacks fairness is as follows:
 *
 *     t1 enters wait()
 *     t2 enters wait()
 *
 *     tZ calls broadcast()
 *
 *     t1 exits wait()
 *     t1 enters wait()
 *
 *     tZ calls broadcast()
 *
 *     t1 exits wait()
 *
 * IMPLEMENTATION:
 *
 *   The most suitable primitives available to us in Windows XP are semaphores,
 *   auto-reset events and manual-reset events. A solution based on semaphores
 *   is chosen.
 *
 *   PROBLEM. Designing a solution based on semaphores is non-trivial because,
 *   while it is easy to track the number of waiters in an interlocked data
 *   structure and then add that number to the semaphore, this does not
 *   guarantee fairness or correctness. Consider the following situation:
 *
 *     - t1 enters wait(), adding 1 to the wait counter & blocks on the semaphore
 *     - t2 enters wait(), adding 1 to the wait counter & blocks on the semaphore
 *     - tZ calls broadcast(), finds the wait counter is 2, adds 2 to the semaphore
 *
 *     - t1 exits wait()
 *     - t1 immediately reenters wait() and blocks on the semaphore
 *     - The semaphore is still positive due to also having been signalled
 *       for t2, therefore it is decremented
 *     - t1 exits wait() immediately; t2 is never woken
 *
 *   GENERATION COUNTERS. One naive solution to this is to use a generation
 *   counter. Each broadcast() invocation increments a generation counter. If
 *   the generation counter has not changed during a semaphore wait operation
 *   inside wait(), this indicates that no broadcast() call has been made in
 *   the meantime; therefore, the successful semaphore decrement must have
 *   'stolen' a wakeup from another thread which was waiting to wakeup from the
 *   prior broadcast() call but which had not yet had a chance to do so. The
 *   semaphore can then be reincremented and the wait() operation repeated.
 *
 *   However, this suffers from the obvious problem that without OS guarantees
 *   as to how semaphore readiness events are distributed amongst threads,
 *   there is no particular guarantee that the semaphore readiness event will
 *   not be immediately redistributed back to the same thread t1.
 *
 *   SOLUTION. A solution is chosen as follows. In its initial state, a
 *   condition variable can accept waiters, who wait for the semaphore
 *   normally. However, once broadcast() is called, the condition
 *   variable becomes 'closed'. Any existing blocked threads are unblocked,
 *   but any new calls to wait() will instead enter a blocking pre-wait stage.
 *   Pre-wait threads are not considered to be waiting (and the external
 *   mutex remains held). A call to wait() in pre-wait cannot progress
 *   to waiting until all threads due to be unblocked by the prior broadcast()
 *   call have returned and had a chance to execute.
 *
 *   This pre-wait does not affect a thread if it does not call wait()
 *   again until after all threads have had a chance to execute.
 *
 *   RESOURCE USAGE. Aside from an allocation for the condition variable
 *   structure, this solution uses two Win32 semaphores.
 *
 * FUTURE OPTIMISATIONS:
 *
 *   An optimised multi-generation implementation is possible at the cost of
 *   higher Win32 resource usage. Multiple 'buckets' could be defined, with
 *   usage rotating between buckets internally as buckets become closed.
 *   This would avoid the need for the prewait in more cases, depending
 *   on intensity of usage.
 *
 */
typedef struct legacy_condvar_st {
    CRYPTO_MUTEX    *int_m;       /* internal mutex */
    HANDLE          sema;         /* main wait semaphore */
    HANDLE          prewait_sema; /* prewait semaphore */
    /*
     * All of the following fields are protected by int_m.
     *
     * num_wake only ever increases by virtue of a corresponding decrease in
     * num_wait. num_wait can decrease for other reasons (for example due to a
     * wait operation timing out).
     */
    size_t          num_wait;     /* Num. threads currently blocked */
    size_t          num_wake;     /* Num. threads due to wake up */
    size_t          num_prewait;  /* Num. threads in prewait */
    size_t          gen;          /* Prewait generation */
    int             closed;       /* Is closed? */
} LEGACY_CONDVAR;

CRYPTO_CONDVAR *ossl_crypto_condvar_new(void)
{
    LEGACY_CONDVAR *cv;

    if ((cv = OPENSSL_malloc(sizeof(LEGACY_CONDVAR))) == NULL)
        return NULL;

    if ((cv->int_m = ossl_crypto_mutex_new()) == NULL) {
        OPENSSL_free(cv);
        return NULL;
    }

    if ((cv->sema = CreateSemaphoreA(NULL, 0, LONG_MAX, NULL)) == NULL) {
        ossl_crypto_mutex_free(&cv->int_m);
        OPENSSL_free(cv);
        return NULL;
    }

    if ((cv->prewait_sema = CreateSemaphoreA(NULL, 0, LONG_MAX, NULL)) == NULL) {
        CloseHandle(cv->sema);
        ossl_crypto_mutex_free(&cv->int_m);
        OPENSSL_free(cv);
        return NULL;
    }

    cv->num_wait      = 0;
    cv->num_wake      = 0;
    cv->num_prewait   = 0;
    cv->closed        = 0;

    return (CRYPTO_CONDVAR *)cv;
}

void ossl_crypto_condvar_free(CRYPTO_CONDVAR **cv_p)
{
    if (*cv_p != NULL) {
        LEGACY_CONDVAR *cv = *(LEGACY_CONDVAR **)cv_p;

        CloseHandle(cv->sema);
        CloseHandle(cv->prewait_sema);
        ossl_crypto_mutex_free(&cv->int_m);
        OPENSSL_free(cv);
    }

    *cv_p = NULL;
}

static uint32_t obj_wait(HANDLE h, OSSL_TIME deadline)
{
    DWORD timeout;

    if (!determine_timeout(deadline, &timeout))
        timeout = 1;

    return WaitForSingleObject(h, timeout);
}

void ossl_crypto_condvar_wait_timeout(CRYPTO_CONDVAR *cv_, CRYPTO_MUTEX *ext_m,
                                      OSSL_TIME deadline)
{
    LEGACY_CONDVAR *cv = (LEGACY_CONDVAR *)cv_;
    int closed, set_prewait = 0, have_orig_gen = 0;
    uint32_t rc;
    size_t orig_gen;

    /* Admission control - prewait until we can enter our actual wait phase. */
    do {
        ossl_crypto_mutex_lock(cv->int_m);

        closed = cv->closed;

        /*
         * Once prewait is over the prewait semaphore is signalled and
         * num_prewait is set to 0. Use a generation counter to track if we need
         * to remove a value we added to num_prewait when exiting (e.g. due to
         * timeout or failure of WaitForSingleObject).
         */
        if (!have_orig_gen) {
            orig_gen = cv->gen;
            have_orig_gen = 1;
        } else if (cv->gen != orig_gen) {
            set_prewait = 0;
            orig_gen = cv->gen;
        }

        if (!closed) {
            /* We can now be admitted. */
            ++cv->num_wait;
            if (set_prewait) {
                --cv->num_prewait;
                set_prewait = 0;
            }
        } else if (!set_prewait) {
            ++cv->num_prewait;
            set_prewait = 1;
        }

        ossl_crypto_mutex_unlock(cv->int_m);

        if (closed)
            if (obj_wait(cv->prewait_sema, deadline) != WAIT_OBJECT_0) {
                /*
                 * If we got WAIT_OBJECT_0 we are safe - num_prewait has been
                 * set to 0 and the semaphore has been consumed. On the other
                 * hand if we timed out, there may be a residual posting that
                 * was made just after we timed out. However in the worst case
                 * this will just cause an internal spurious wakeup here in the
                 * future, so we do not care too much about this. We treat
                 * failure and timeout cases as the same, and simply exit in
                 * this case.
                 */
                ossl_crypto_mutex_lock(cv->int_m);
                if (set_prewait && cv->gen == orig_gen)
                    --cv->num_prewait;
                ossl_crypto_mutex_unlock(cv->int_m);
                return;
            }
    } while (closed);

    /*
     * Unlock external mutex. Do not do this until we have been admitted, as we
     * must guarantee we wake if broadcast is called at any time after ext_m is
     * unlocked.
     */
    ossl_crypto_mutex_unlock(ext_m);

    for (;;) {
        /* Wait. */
        rc = obj_wait(cv->sema, deadline);

        /* Reacquire internal mutex and probe state. */
        ossl_crypto_mutex_lock(cv->int_m);

        if (cv->num_wake > 0) {
            /*
             * A wake token is available, so we can wake up. Consume the token
             * and get out of here. We don't care what WaitForSingleObject
             * returned here (e.g. if it timed out coincidentally). In the
             * latter case a signal might be left in the semaphore which causes
             * a future WaitForSingleObject call to return immediately, but in
             * this case we will just loop again.
             */
            --cv->num_wake;
            if (cv->num_wake == 0 && cv->closed) {
                /*
                 * We consumed the last wake token, so we can now open the
                 * condition variable for new admissions.
                 */
                cv->closed = 0;
                if (cv->num_prewait > 0) {
                    ReleaseSemaphore(cv->prewait_sema, (LONG)cv->num_prewait, NULL);
                    cv->num_prewait = 0;
                    ++cv->gen;
                }
            }
        } else if (rc == WAIT_OBJECT_0) {
            /*
             * We got a wakeup from the semaphore but we did not have any wake
             * tokens. This ideally does not happen, but might if during a
             * previous wait() call the semaphore is posted just after
             * WaitForSingleObject returns due to a timeout (such that the
             * num_wake > 0 case is taken above). Just spin again. (It is worth
             * noting that repeated WaitForSingleObject calls is the only method
             * documented for decrementing a Win32 semaphore, so this is
             * basically the best possible strategy.)
             */
            ossl_crypto_mutex_unlock(cv->int_m);
            continue;
        } else {
            /*
             * Assume we timed out. The WaitForSingleObject call may also have
             * failed for some other reason, which we treat as a timeout.
             */
            assert(cv->num_wait > 0);
            --cv->num_wait;
        }

        break;
    }

    ossl_crypto_mutex_unlock(cv->int_m);
    ossl_crypto_mutex_lock(ext_m);
}

void ossl_crypto_condvar_wait(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *ext_m)
{
    ossl_crypto_condvar_wait_timeout(cv, ext_m, ossl_time_infinite());
}

void ossl_crypto_condvar_broadcast(CRYPTO_CONDVAR *cv_)
{
    LEGACY_CONDVAR *cv = (LEGACY_CONDVAR *)cv_;
    size_t num_wake;

    ossl_crypto_mutex_lock(cv->int_m);

    num_wake = cv->num_wait;
    if (num_wake == 0) {
        ossl_crypto_mutex_unlock(cv->int_m);
        return;
    }

    cv->num_wake  += num_wake;
    cv->num_wait  -= num_wake;
    cv->closed     = 1;

    ossl_crypto_mutex_unlock(cv->int_m);
    ReleaseSemaphore(cv->sema, num_wake, NULL);
}

void ossl_crypto_condvar_signal(CRYPTO_CONDVAR *cv_)
{
    LEGACY_CONDVAR *cv = (LEGACY_CONDVAR *)cv_;

    ossl_crypto_mutex_lock(cv->int_m);

    if (cv->num_wait == 0) {
        ossl_crypto_mutex_unlock(cv->int_m);
        return;
    }

    /*
     * We do not close the condition variable when merely signalling, as there
     * are no guaranteed fairness semantics here, unlike for a broadcast.
     */
    --cv->num_wait;
    ++cv->num_wake;

    ossl_crypto_mutex_unlock(cv->int_m);
    ReleaseSemaphore(cv->sema, 1, NULL);
}

# else

CRYPTO_CONDVAR *ossl_crypto_condvar_new(void)
{
    CONDITION_VARIABLE *cv_p;

    if ((cv_p = OPENSSL_zalloc(sizeof(*cv_p))) == NULL)
        return NULL;
    InitializeConditionVariable(cv_p);
    return (CRYPTO_CONDVAR *)cv_p;
}

void ossl_crypto_condvar_wait(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex)
{
    CONDITION_VARIABLE *cv_p;
    CRITICAL_SECTION *mutex_p;

    cv_p = (CONDITION_VARIABLE *)cv;
    mutex_p = (CRITICAL_SECTION *)mutex;
    SleepConditionVariableCS(cv_p, mutex_p, INFINITE);
}

void ossl_crypto_condvar_wait_timeout(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex,
                                      OSSL_TIME deadline)
{
    DWORD timeout;
    CONDITION_VARIABLE *cv_p = (CONDITION_VARIABLE *)cv;
    CRITICAL_SECTION *mutex_p = (CRITICAL_SECTION *)mutex;

    if (!determine_timeout(deadline, &timeout))
        timeout = 1;

    SleepConditionVariableCS(cv_p, mutex_p, timeout);
}

void ossl_crypto_condvar_broadcast(CRYPTO_CONDVAR *cv)
{
    CONDITION_VARIABLE *cv_p;

    cv_p = (CONDITION_VARIABLE *)cv;
    WakeAllConditionVariable(cv_p);
}

void ossl_crypto_condvar_signal(CRYPTO_CONDVAR *cv)
{
    CONDITION_VARIABLE *cv_p;

    cv_p = (CONDITION_VARIABLE *)cv;
    WakeConditionVariable(cv_p);
}

void ossl_crypto_condvar_free(CRYPTO_CONDVAR **cv)
{
    CONDITION_VARIABLE **cv_p;

    cv_p = (CONDITION_VARIABLE **)cv;
    OPENSSL_free(*cv_p);
    *cv_p = NULL;
}

# endif

void ossl_crypto_mem_barrier(void)
{
    MemoryBarrier();
}

#endif
