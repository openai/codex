/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_RCU_H
# define OPENSSL_RCU_H
# pragma once

#include "crypto/context.h"

typedef void (*rcu_cb_fn)(void *data);

typedef struct rcu_lock_st CRYPTO_RCU_LOCK;

CRYPTO_RCU_LOCK *ossl_rcu_lock_new(int num_writers, OSSL_LIB_CTX *ctx);
void ossl_rcu_lock_free(CRYPTO_RCU_LOCK *lock);
void ossl_rcu_read_lock(CRYPTO_RCU_LOCK *lock);
void ossl_rcu_write_lock(CRYPTO_RCU_LOCK *lock);
void ossl_rcu_write_unlock(CRYPTO_RCU_LOCK *lock);
void ossl_rcu_read_unlock(CRYPTO_RCU_LOCK *lock);
void ossl_synchronize_rcu(CRYPTO_RCU_LOCK *lock);
int ossl_rcu_call(CRYPTO_RCU_LOCK *lock, rcu_cb_fn cb, void *data);
void *ossl_rcu_uptr_deref(void **p);
void ossl_rcu_assign_uptr(void **p, void **v);
#define ossl_rcu_deref(p) ossl_rcu_uptr_deref((void **)p)
#define ossl_rcu_assign_ptr(p,v) ossl_rcu_assign_uptr((void **)p, (void **)v)

#endif
