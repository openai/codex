/*
 * Copyright 2015-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This must be the first #include file */
#include "../async_local.h"

#ifdef ASYNC_POSIX

# include <stddef.h>
# include <unistd.h>
# include <openssl/err.h>
# include <openssl/crypto.h>

#define STACKSIZE       32768

static CRYPTO_RWLOCK *async_mem_lock;

static void *async_stack_alloc(size_t *num);
static void async_stack_free(void *addr);

int async_local_init(void)
{
    async_mem_lock = CRYPTO_THREAD_lock_new();
    return async_mem_lock != NULL;
}

void async_local_deinit(void)
{
    CRYPTO_THREAD_lock_free(async_mem_lock);
}

static int allow_customize = 1;
static ASYNC_stack_alloc_fn stack_alloc_impl = async_stack_alloc;
static ASYNC_stack_free_fn stack_free_impl = async_stack_free;

int ASYNC_is_capable(void)
{
    ucontext_t ctx;

    /*
     * Some platforms provide getcontext() but it does not work (notably
     * MacOSX PPC64). Check for a working getcontext();
     */
    return getcontext(&ctx) == 0;
}

int ASYNC_set_mem_functions(ASYNC_stack_alloc_fn alloc_fn,
                            ASYNC_stack_free_fn free_fn)
{
    OPENSSL_init_crypto(OPENSSL_INIT_ASYNC, NULL);

    if (!CRYPTO_THREAD_write_lock(async_mem_lock))
        return 0;
    if (!allow_customize) {
        CRYPTO_THREAD_unlock(async_mem_lock);
        return 0;
    }
    CRYPTO_THREAD_unlock(async_mem_lock);

    if (alloc_fn != NULL)
        stack_alloc_impl = alloc_fn;
    if (free_fn != NULL)
        stack_free_impl = free_fn;
    return 1;
}

void ASYNC_get_mem_functions(ASYNC_stack_alloc_fn *alloc_fn,
                             ASYNC_stack_free_fn *free_fn)
{
    if (alloc_fn != NULL)
        *alloc_fn = stack_alloc_impl;
    if (free_fn != NULL)
        *free_fn = stack_free_impl;
}

static void *async_stack_alloc(size_t *num)
{
    return OPENSSL_malloc(*num);
}

static void async_stack_free(void *addr)
{
    OPENSSL_free(addr);
}

void async_local_cleanup(void)
{
}

int async_fibre_makecontext(async_fibre *fibre)
{
#ifndef USE_SWAPCONTEXT
    fibre->env_init = 0;
#endif
    if (getcontext(&fibre->fibre) == 0) {
        size_t num = STACKSIZE;

        /*
         *  Disallow customisation after the first
         *  stack is allocated.
         */
        if (allow_customize) {
            if (!CRYPTO_THREAD_write_lock(async_mem_lock))
                return 0;
            allow_customize = 0;
            CRYPTO_THREAD_unlock(async_mem_lock);
        }

        fibre->fibre.uc_stack.ss_sp = stack_alloc_impl(&num);
        if (fibre->fibre.uc_stack.ss_sp != NULL) {
            fibre->fibre.uc_stack.ss_size = num;
            fibre->fibre.uc_link = NULL;
            makecontext(&fibre->fibre, async_start_func, 0);
            return 1;
        }
    } else {
        fibre->fibre.uc_stack.ss_sp = NULL;
    }
    return 0;
}

void async_fibre_free(async_fibre *fibre)
{
    stack_free_impl(fibre->fibre.uc_stack.ss_sp);
    fibre->fibre.uc_stack.ss_sp = NULL;
}

#endif
