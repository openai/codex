/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_INTERNAL_THREAD_H
# define OPENSSL_INTERNAL_THREAD_H
# include <openssl/configuration.h>
# include <internal/thread_arch.h>
# include <openssl/e_os2.h>
# include <openssl/types.h>
# include <internal/cryptlib.h>
# include "crypto/context.h"

void *ossl_crypto_thread_start(OSSL_LIB_CTX *ctx, CRYPTO_THREAD_ROUTINE start,
                               void *data);
int ossl_crypto_thread_join(void *task, CRYPTO_THREAD_RETVAL *retval);
int ossl_crypto_thread_clean(void *vhandle);
uint64_t ossl_get_avail_threads(OSSL_LIB_CTX *ctx);

# if defined(OPENSSL_THREADS)

#  define OSSL_LIB_CTX_GET_THREADS(CTX)                                       \
    ossl_lib_ctx_get_data(CTX, OSSL_LIB_CTX_THREAD_INDEX);

typedef struct openssl_threads_st {
    uint64_t max_threads;
    uint64_t active_threads;
    CRYPTO_MUTEX *lock;
    CRYPTO_CONDVAR *cond_finished;
} OSSL_LIB_CTX_THREADS;

# endif /* defined(OPENSSL_THREADS) */

#endif /* OPENSSL_INTERNAL_THREAD_H */
