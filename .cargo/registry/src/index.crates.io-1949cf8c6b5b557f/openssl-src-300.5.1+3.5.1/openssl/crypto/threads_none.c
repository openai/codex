/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "internal/cryptlib.h"
#include "internal/rcu.h"
#include "rcu_internal.h"

#if !defined(OPENSSL_THREADS) || defined(CRYPTO_TDEBUG)

# if defined(OPENSSL_SYS_UNIX)
#  include <sys/types.h>
#  include <unistd.h>
# endif

struct rcu_lock_st {
    struct rcu_cb_item *cb_items;
};

CRYPTO_RCU_LOCK *ossl_rcu_lock_new(int num_writers,
                                   ossl_unused OSSL_LIB_CTX *ctx)
{
    struct rcu_lock_st *lock;

    lock = OPENSSL_zalloc(sizeof(*lock));
    return lock;
}

void ossl_rcu_lock_free(CRYPTO_RCU_LOCK *lock)
{
    OPENSSL_free(lock);
}

void ossl_rcu_read_lock(CRYPTO_RCU_LOCK *lock)
{
    return;
}

void ossl_rcu_write_lock(CRYPTO_RCU_LOCK *lock)
{
    return;
}

void ossl_rcu_write_unlock(CRYPTO_RCU_LOCK *lock)
{
    return;
}

void ossl_rcu_read_unlock(CRYPTO_RCU_LOCK *lock)
{
    return;
}

void ossl_synchronize_rcu(CRYPTO_RCU_LOCK *lock)
{
    struct rcu_cb_item *items = lock->cb_items;
    struct rcu_cb_item *tmp;

    lock->cb_items = NULL;

    while (items != NULL) {
        tmp = items->next;
        items->fn(items->data);
        OPENSSL_free(items);
        items = tmp;
    }
}

int ossl_rcu_call(CRYPTO_RCU_LOCK *lock, rcu_cb_fn cb, void *data)
{
    struct rcu_cb_item *new = OPENSSL_zalloc(sizeof(*new));

    if (new == NULL)
        return 0;

    new->fn = cb;
    new->data = data;
    new->next = lock->cb_items;
    lock->cb_items = new;
    return 1;
}

void *ossl_rcu_uptr_deref(void **p)
{
    return (void *)*p;
}

void ossl_rcu_assign_uptr(void **p, void **v)
{
    *(void **)p = *(void **)v;
}

CRYPTO_RWLOCK *CRYPTO_THREAD_lock_new(void)
{
    CRYPTO_RWLOCK *lock;

    if ((lock = CRYPTO_zalloc(sizeof(unsigned int), NULL, 0)) == NULL)
        /* Don't set error, to avoid recursion blowup. */
        return NULL;

    *(unsigned int *)lock = 1;

    return lock;
}

__owur int CRYPTO_THREAD_read_lock(CRYPTO_RWLOCK *lock)
{
    if (!ossl_assert(*(unsigned int *)lock == 1))
        return 0;
    return 1;
}

__owur int CRYPTO_THREAD_write_lock(CRYPTO_RWLOCK *lock)
{
    if (!ossl_assert(*(unsigned int *)lock == 1))
        return 0;
    return 1;
}

int CRYPTO_THREAD_unlock(CRYPTO_RWLOCK *lock)
{
    if (!ossl_assert(*(unsigned int *)lock == 1))
        return 0;
    return 1;
}

void CRYPTO_THREAD_lock_free(CRYPTO_RWLOCK *lock) {
    if (lock == NULL)
        return;

    *(unsigned int *)lock = 0;
    OPENSSL_free(lock);

    return;
}

int CRYPTO_THREAD_run_once(CRYPTO_ONCE *once, void (*init)(void))
{
    if (*once != 0)
        return 1;

    init();
    *once = 1;

    return 1;
}

# define OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX 256

struct thread_local_storage_entry {
    void *data;
    uint8_t used;
};

static struct thread_local_storage_entry thread_local_storage[OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX];

int CRYPTO_THREAD_init_local(CRYPTO_THREAD_LOCAL *key, void (*cleanup)(void *))
{
    int entry_idx = 0;

    for (entry_idx = 0; entry_idx < OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX; entry_idx++) {
        if (!thread_local_storage[entry_idx].used)
            break;
    }

    if (entry_idx == OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX)
        return 0;

    *key = entry_idx;
    thread_local_storage[*key].used = 1;
    thread_local_storage[*key].data = NULL;

    return 1;
}

void *CRYPTO_THREAD_get_local(CRYPTO_THREAD_LOCAL *key)
{
    if (*key >= OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX)
        return NULL;

    return thread_local_storage[*key].data;
}

int CRYPTO_THREAD_set_local(CRYPTO_THREAD_LOCAL *key, void *val)
{
    if (*key >= OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX)
        return 0;

    thread_local_storage[*key].data = val;

    return 1;
}

int CRYPTO_THREAD_cleanup_local(CRYPTO_THREAD_LOCAL *key)
{
    if (*key >= OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX)
        return 0;

    thread_local_storage[*key].used = 0;
    thread_local_storage[*key].data = NULL;
    *key = OPENSSL_CRYPTO_THREAD_LOCAL_KEY_MAX + 1;
    return 1;
}

CRYPTO_THREAD_ID CRYPTO_THREAD_get_current_id(void)
{
    return 0;
}

int CRYPTO_THREAD_compare_id(CRYPTO_THREAD_ID a, CRYPTO_THREAD_ID b)
{
    return (a == b);
}

int CRYPTO_atomic_add(int *val, int amount, int *ret, CRYPTO_RWLOCK *lock)
{
    *val += amount;
    *ret  = *val;

    return 1;
}

int CRYPTO_atomic_add64(uint64_t *val, uint64_t op, uint64_t *ret,
                        CRYPTO_RWLOCK *lock)
{
    *val += op;
    *ret  = *val;

    return 1;
}

int CRYPTO_atomic_and(uint64_t *val, uint64_t op, uint64_t *ret,
                      CRYPTO_RWLOCK *lock)
{
    *val &= op;
    *ret  = *val;

    return 1;
}

int CRYPTO_atomic_or(uint64_t *val, uint64_t op, uint64_t *ret,
                     CRYPTO_RWLOCK *lock)
{
    *val |= op;
    *ret  = *val;

    return 1;
}

int CRYPTO_atomic_load(uint64_t *val, uint64_t *ret, CRYPTO_RWLOCK *lock)
{
    *ret  = *val;

    return 1;
}

int CRYPTO_atomic_store(uint64_t *dst, uint64_t val, CRYPTO_RWLOCK *lock)
{
    *dst = val;

    return 1;
}

int CRYPTO_atomic_load_int(int *val, int *ret, CRYPTO_RWLOCK *lock)
{
    *ret = *val;

    return 1;
}

int openssl_init_fork_handlers(void)
{
    return 0;
}

int openssl_get_fork_id(void)
{
# if defined(OPENSSL_SYS_UNIX)
    return getpid();
# else
    return 0;
# endif
}
#endif
