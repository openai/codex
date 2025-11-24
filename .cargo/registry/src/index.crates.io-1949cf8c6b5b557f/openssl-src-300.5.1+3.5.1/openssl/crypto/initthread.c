/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include "crypto/cryptlib.h"
#include "prov/providercommon.h"
#include "internal/thread_once.h"
#include "crypto/context.h"

#ifdef FIPS_MODULE
#include "prov/provider_ctx.h"

/*
 * Thread aware code may want to be told about thread stop events. We register
 * to hear about those thread stop events when we see a new thread has started.
 * We call the ossl_init_thread_start function to do that. In the FIPS provider
 * we have our own copy of ossl_init_thread_start, which cascades notifications
 * about threads stopping from libcrypto to all the code in the FIPS provider
 * that needs to know about it.
 *
 * The FIPS provider tells libcrypto about which threads it is interested in
 * by calling "c_thread_start" which is a function pointer created during
 * provider initialisation (i.e. OSSL_provider_init).
 */
extern OSSL_FUNC_core_thread_start_fn *c_thread_start;
#endif

typedef struct thread_event_handler_st THREAD_EVENT_HANDLER;
struct thread_event_handler_st {
#ifndef FIPS_MODULE
    const void *index;
#endif
    void *arg;
    OSSL_thread_stop_handler_fn handfn;
    THREAD_EVENT_HANDLER *next;
};

#ifndef FIPS_MODULE
DEFINE_SPECIAL_STACK_OF(THREAD_EVENT_HANDLER_PTR, THREAD_EVENT_HANDLER *)

typedef struct global_tevent_register_st GLOBAL_TEVENT_REGISTER;
struct global_tevent_register_st {
    STACK_OF(THREAD_EVENT_HANDLER_PTR) *skhands;
    CRYPTO_RWLOCK *lock;
};

static GLOBAL_TEVENT_REGISTER *glob_tevent_reg = NULL;

static CRYPTO_ONCE tevent_register_runonce = CRYPTO_ONCE_STATIC_INIT;

DEFINE_RUN_ONCE_STATIC(create_global_tevent_register)
{
    glob_tevent_reg = OPENSSL_zalloc(sizeof(*glob_tevent_reg));
    if (glob_tevent_reg == NULL)
        return 0;

    glob_tevent_reg->skhands = sk_THREAD_EVENT_HANDLER_PTR_new_null();
    glob_tevent_reg->lock = CRYPTO_THREAD_lock_new();
    if (glob_tevent_reg->skhands == NULL || glob_tevent_reg->lock == NULL) {
        sk_THREAD_EVENT_HANDLER_PTR_free(glob_tevent_reg->skhands);
        CRYPTO_THREAD_lock_free(glob_tevent_reg->lock);
        OPENSSL_free(glob_tevent_reg);
        glob_tevent_reg = NULL;
        return 0;
    }

    return 1;
}

static GLOBAL_TEVENT_REGISTER *get_global_tevent_register(void)
{
    if (!RUN_ONCE(&tevent_register_runonce, create_global_tevent_register))
        return NULL;
    return glob_tevent_reg;
}
#endif

#ifndef FIPS_MODULE
static int  init_thread_push_handlers(THREAD_EVENT_HANDLER **hands);
static void init_thread_remove_handlers(THREAD_EVENT_HANDLER **handsin);
static void init_thread_destructor(void *hands);
static int  init_thread_deregister(void *arg, int all);
#endif
static void init_thread_stop(void *arg, THREAD_EVENT_HANDLER **hands);

static THREAD_EVENT_HANDLER **
init_get_thread_local(CRYPTO_THREAD_LOCAL *local, int alloc, int keep)
{
    THREAD_EVENT_HANDLER **hands = CRYPTO_THREAD_get_local(local);

    if (alloc) {
        if (hands == NULL) {

            if ((hands = OPENSSL_zalloc(sizeof(*hands))) == NULL)
                return NULL;

            if (!CRYPTO_THREAD_set_local(local, hands)) {
                OPENSSL_free(hands);
                return NULL;
            }

#ifndef FIPS_MODULE
            if (!init_thread_push_handlers(hands)) {
                CRYPTO_THREAD_set_local(local, NULL);
                OPENSSL_free(hands);
                return NULL;
            }
#endif
        }
    } else if (!keep) {
        CRYPTO_THREAD_set_local(local, NULL);
    }

    return hands;
}

#ifndef FIPS_MODULE
/*
 * Since per-thread-specific-data destructors are not universally
 * available, i.e. not on Windows, only below CRYPTO_THREAD_LOCAL key
 * is assumed to have destructor associated. And then an effort is made
 * to call this single destructor on non-pthread platform[s].
 *
 * Initial value is "impossible". It is used as guard value to shortcut
 * destructor for threads terminating before libcrypto is initialized or
 * after it's de-initialized. Access to the key doesn't have to be
 * serialized for the said threads, because they didn't use libcrypto
 * and it doesn't matter if they pick "impossible" or dereference real
 * key value and pull NULL past initialization in the first thread that
 * intends to use libcrypto.
 */
static union {
    long sane;
    CRYPTO_THREAD_LOCAL value;
} destructor_key = { -1 };

/*
 * The thread event handler list is a thread specific linked list
 * of callback functions which are invoked in list order by the
 * current thread in case of certain events. (Currently, there is
 * only one type of event, the 'thread stop' event.)
 *
 * We also keep a global reference to that linked list, so that we
 * can deregister handlers if necessary before all the threads are
 * stopped.
 */
static int init_thread_push_handlers(THREAD_EVENT_HANDLER **hands)
{
    int ret;
    GLOBAL_TEVENT_REGISTER *gtr;

    gtr = get_global_tevent_register();
    if (gtr == NULL)
        return 0;

    if (!CRYPTO_THREAD_write_lock(gtr->lock))
        return 0;
    ret = (sk_THREAD_EVENT_HANDLER_PTR_push(gtr->skhands, hands) != 0);
    CRYPTO_THREAD_unlock(gtr->lock);

    return ret;
}

static void init_thread_remove_handlers(THREAD_EVENT_HANDLER **handsin)
{
    GLOBAL_TEVENT_REGISTER *gtr;
    int i;

    gtr = get_global_tevent_register();
    if (gtr == NULL)
        return;
    if (!CRYPTO_THREAD_write_lock(gtr->lock))
        return;
    for (i = 0; i < sk_THREAD_EVENT_HANDLER_PTR_num(gtr->skhands); i++) {
        THREAD_EVENT_HANDLER **hands
            = sk_THREAD_EVENT_HANDLER_PTR_value(gtr->skhands, i);

        if (hands == handsin) {
            sk_THREAD_EVENT_HANDLER_PTR_delete(gtr->skhands, i);
            CRYPTO_THREAD_unlock(gtr->lock);
            return;
        }
    }
    CRYPTO_THREAD_unlock(gtr->lock);
    return;
}

static void init_thread_destructor(void *hands)
{
    init_thread_stop(NULL, (THREAD_EVENT_HANDLER **)hands);
    init_thread_remove_handlers(hands);
    OPENSSL_free(hands);
}

int ossl_init_thread(void)
{
    if (!CRYPTO_THREAD_init_local(&destructor_key.value,
                                  init_thread_destructor))
        return 0;

    return 1;
}

void ossl_cleanup_thread(void)
{
    init_thread_deregister(NULL, 1);
    CRYPTO_THREAD_cleanup_local(&destructor_key.value);
    destructor_key.sane = -1;
}

void OPENSSL_thread_stop_ex(OSSL_LIB_CTX *ctx)
{
    ctx = ossl_lib_ctx_get_concrete(ctx);
    /*
     * It would be nice if we could figure out a way to do this on all threads
     * that have used the OSSL_LIB_CTX when the context is freed. This is
     * currently not possible due to the use of thread local variables.
     */
    ossl_ctx_thread_stop(ctx);
}

void OPENSSL_thread_stop(void)
{
    if (destructor_key.sane != -1) {
        THREAD_EVENT_HANDLER **hands
            = init_get_thread_local(&destructor_key.value, 0, 0);
        init_thread_stop(NULL, hands);

        init_thread_remove_handlers(hands);
        OPENSSL_free(hands);
    }
}

void ossl_ctx_thread_stop(OSSL_LIB_CTX *ctx)
{
    if (destructor_key.sane != -1) {
        THREAD_EVENT_HANDLER **hands
            = init_get_thread_local(&destructor_key.value, 0, 1);
        init_thread_stop(ctx, hands);
    }
}

#else

static void ossl_arg_thread_stop(void *arg);

/* Register the current thread so that we are informed if it gets stopped */
int ossl_thread_register_fips(OSSL_LIB_CTX *libctx)
{
    return c_thread_start(FIPS_get_core_handle(libctx), ossl_arg_thread_stop,
                          libctx);
}

void *ossl_thread_event_ctx_new(OSSL_LIB_CTX *libctx)
{
    THREAD_EVENT_HANDLER **hands = NULL;
    CRYPTO_THREAD_LOCAL *tlocal = OPENSSL_zalloc(sizeof(*tlocal));

    if (tlocal == NULL)
        return NULL;

    if (!CRYPTO_THREAD_init_local(tlocal, NULL))
        goto deinit;

    hands = OPENSSL_zalloc(sizeof(*hands));
    if (hands == NULL)
        goto err;

    if (!CRYPTO_THREAD_set_local(tlocal, hands))
        goto err;

    /*
     * We should ideally call ossl_thread_register_fips() here. This function
     * is called during the startup of the FIPS provider and we need to ensure
     * that the main thread is registered to receive thread callbacks in order
     * to free |hands| that we allocated above. However we are too early in
     * the FIPS provider initialisation that FIPS_get_core_handle() doesn't work
     * yet. So we defer this to the main provider OSSL_provider_init_int()
     * function.
     */

    return tlocal;
 err:
    OPENSSL_free(hands);
    CRYPTO_THREAD_cleanup_local(tlocal);
 deinit:
    OPENSSL_free(tlocal);
    return NULL;
}

void ossl_thread_event_ctx_free(void *tlocal)
{
    CRYPTO_THREAD_cleanup_local(tlocal);
    OPENSSL_free(tlocal);
}

static void ossl_arg_thread_stop(void *arg)
{
    ossl_ctx_thread_stop((OSSL_LIB_CTX *)arg);
}

void ossl_ctx_thread_stop(OSSL_LIB_CTX *ctx)
{
    THREAD_EVENT_HANDLER **hands;
    CRYPTO_THREAD_LOCAL *local
        = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_THREAD_EVENT_HANDLER_INDEX);

    if (local == NULL)
        return;
    hands = init_get_thread_local(local, 0, 0);
    init_thread_stop(ctx, hands);
    OPENSSL_free(hands);
}
#endif /* FIPS_MODULE */


static void init_thread_stop(void *arg, THREAD_EVENT_HANDLER **hands)
{
    THREAD_EVENT_HANDLER *curr, *prev = NULL, *tmp;
#ifndef FIPS_MODULE
    GLOBAL_TEVENT_REGISTER *gtr;
#endif

    /* Can't do much about this */
    if (hands == NULL)
        return;

#ifndef FIPS_MODULE
    gtr = get_global_tevent_register();
    if (gtr == NULL)
        return;

    if (!CRYPTO_THREAD_write_lock(gtr->lock))
        return;
#endif

    curr = *hands;
    while (curr != NULL) {
        if (arg != NULL && curr->arg != arg) {
            prev = curr;
            curr = curr->next;
            continue;
        }
        curr->handfn(curr->arg);
        if (prev == NULL)
            *hands = curr->next;
        else
            prev->next = curr->next;

        tmp = curr;
        curr = curr->next;

        OPENSSL_free(tmp);
    }
#ifndef FIPS_MODULE
    CRYPTO_THREAD_unlock(gtr->lock);
#endif
}

int ossl_init_thread_start(const void *index, void *arg,
                           OSSL_thread_stop_handler_fn handfn)
{
    THREAD_EVENT_HANDLER **hands;
    THREAD_EVENT_HANDLER *hand;
#ifdef FIPS_MODULE
    OSSL_LIB_CTX *ctx = arg;

    /*
     * In FIPS mode the list of THREAD_EVENT_HANDLERs is unique per combination
     * of OSSL_LIB_CTX and thread. This is because in FIPS mode each
     * OSSL_LIB_CTX gets informed about thread stop events individually.
     */
    CRYPTO_THREAD_LOCAL *local
        = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_THREAD_EVENT_HANDLER_INDEX);
#else
    /*
     * Outside of FIPS mode the list of THREAD_EVENT_HANDLERs is unique per
     * thread, but may hold multiple OSSL_LIB_CTXs. We only get told about
     * thread stop events globally, so we have to ensure all affected
     * OSSL_LIB_CTXs are informed.
     */
    CRYPTO_THREAD_LOCAL *local = &destructor_key.value;
#endif

    hands = init_get_thread_local(local, 1, 0);
    if (hands == NULL)
        return 0;

#ifdef FIPS_MODULE
    if (*hands == NULL) {
        /*
         * We've not yet registered any handlers for this thread. We need to get
         * libcrypto to tell us about later thread stop events. c_thread_start
         * is a callback to libcrypto defined in fipsprov.c
         */
        if (!ossl_thread_register_fips(ctx))
            return 0;
    }
#endif

    hand = OPENSSL_malloc(sizeof(*hand));
    if (hand == NULL)
        return 0;

    hand->handfn = handfn;
    hand->arg = arg;
#ifndef FIPS_MODULE
    hand->index = index;
#endif
    hand->next = *hands;
    *hands = hand;

    return 1;
}

#ifndef FIPS_MODULE
static int init_thread_deregister(void *index, int all)
{
    GLOBAL_TEVENT_REGISTER *gtr;
    int i;

    gtr = get_global_tevent_register();
    if (gtr == NULL)
        return 0;
    if (!all) {
        if (!CRYPTO_THREAD_write_lock(gtr->lock))
            return 0;
    } else {
        glob_tevent_reg = NULL;
    }
    for (i = 0; i < sk_THREAD_EVENT_HANDLER_PTR_num(gtr->skhands); i++) {
        THREAD_EVENT_HANDLER **hands
            = sk_THREAD_EVENT_HANDLER_PTR_value(gtr->skhands, i);
        THREAD_EVENT_HANDLER *curr = NULL, *prev = NULL, *tmp;

        if (hands == NULL) {
            if (!all)
                CRYPTO_THREAD_unlock(gtr->lock);
            return 0;
        }
        curr = *hands;
        while (curr != NULL) {
            if (all || curr->index == index) {
                if (prev != NULL)
                    prev->next = curr->next;
                else
                    *hands = curr->next;
                tmp = curr;
                curr = curr->next;
                OPENSSL_free(tmp);
                continue;
            }
            prev = curr;
            curr = curr->next;
        }
        if (all)
            OPENSSL_free(hands);
    }
    if (all) {
        CRYPTO_THREAD_lock_free(gtr->lock);
        sk_THREAD_EVENT_HANDLER_PTR_free(gtr->skhands);
        OPENSSL_free(gtr);
    } else {
        CRYPTO_THREAD_unlock(gtr->lock);
    }
    return 1;
}

int ossl_init_thread_deregister(void *index)
{
    return init_thread_deregister(index, 0);
}
#endif
