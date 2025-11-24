/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_THREAD_ARCH_H
# define OSSL_INTERNAL_THREAD_ARCH_H
# include <openssl/configuration.h>
# include <openssl/e_os2.h>
# include "internal/time.h"

# if defined(_WIN32)
#  include <windows.h>
# endif

# if defined(OPENSSL_THREADS) && defined(OPENSSL_SYS_UNIX)
#  define OPENSSL_THREADS_POSIX
# elif defined(OPENSSL_THREADS) && defined(OPENSSL_SYS_VMS)
#  define OPENSSL_THREADS_POSIX
# elif defined(OPENSSL_THREADS) && defined(OPENSSL_SYS_WINDOWS) && \
    defined(_WIN32_WINNT)
#  if _WIN32_WINNT >= 0x0600
#   define OPENSSL_THREADS_WINNT
#  elif _WIN32_WINNT >= 0x0501
#   define OPENSSL_THREADS_WINNT
#   define OPENSSL_THREADS_WINNT_LEGACY
#  else
#   define OPENSSL_THREADS_NONE
#  endif
# else
#  define OPENSSL_THREADS_NONE
# endif

# include <openssl/crypto.h>

typedef struct crypto_mutex_st CRYPTO_MUTEX;
typedef struct crypto_condvar_st CRYPTO_CONDVAR;

CRYPTO_MUTEX *ossl_crypto_mutex_new(void);
void ossl_crypto_mutex_lock(CRYPTO_MUTEX *mutex);
int ossl_crypto_mutex_try_lock(CRYPTO_MUTEX *mutex);
void ossl_crypto_mutex_unlock(CRYPTO_MUTEX *mutex);
void ossl_crypto_mutex_free(CRYPTO_MUTEX **mutex);

CRYPTO_CONDVAR *ossl_crypto_condvar_new(void);
void ossl_crypto_condvar_wait(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex);
void ossl_crypto_condvar_wait_timeout(CRYPTO_CONDVAR *cv, CRYPTO_MUTEX *mutex,
                                      OSSL_TIME deadline);
void ossl_crypto_condvar_broadcast(CRYPTO_CONDVAR *cv);
void ossl_crypto_condvar_signal(CRYPTO_CONDVAR *cv);
void ossl_crypto_condvar_free(CRYPTO_CONDVAR **cv);

typedef uint32_t CRYPTO_THREAD_RETVAL;
typedef CRYPTO_THREAD_RETVAL (*CRYPTO_THREAD_ROUTINE)(void *);
typedef CRYPTO_THREAD_RETVAL (*CRYPTO_THREAD_ROUTINE_CB)(void *,
                                                         void (**)(void *),
                                                         void **);

# define CRYPTO_THREAD_NO_STATE   0UL
# define CRYPTO_THREAD_FINISHED   (1UL << 0)
# define CRYPTO_THREAD_JOIN_AWAIT (1UL << 1)
# define CRYPTO_THREAD_JOINED     (1UL << 2)

# define CRYPTO_THREAD_GET_STATE(THREAD, FLAG) ((THREAD)->state & (FLAG))
# define CRYPTO_THREAD_GET_ERROR(THREAD, FLAG) (((THREAD)->state >> 16) & (FLAG))

typedef struct crypto_thread_st {
    uint32_t state;
    void *data;
    CRYPTO_THREAD_ROUTINE routine;
    CRYPTO_THREAD_RETVAL retval;
    void *handle;
    CRYPTO_MUTEX *lock;
    CRYPTO_MUTEX *statelock;
    CRYPTO_CONDVAR *condvar;
    unsigned long thread_id;
    int joinable;
    OSSL_LIB_CTX *ctx;
} CRYPTO_THREAD;

# if defined(OPENSSL_THREADS)

#  define CRYPTO_THREAD_UNSET_STATE(THREAD, FLAG)                       \
    do {                                                                \
        (THREAD)->state &= ~(FLAG);                                     \
    } while ((void)0, 0)

#  define CRYPTO_THREAD_SET_STATE(THREAD, FLAG)                         \
    do {                                                                \
        (THREAD)->state |= (FLAG);                                      \
    } while ((void)0, 0)

#  define CRYPTO_THREAD_SET_ERROR(THREAD, FLAG)                         \
    do {                                                                \
        (THREAD)->state |= ((FLAG) << 16);                              \
    } while ((void)0, 0)

#  define CRYPTO_THREAD_UNSET_ERROR(THREAD, FLAG)                       \
    do {                                                                \
        (THREAD)->state &= ~((FLAG) << 16);                             \
    } while ((void)0, 0)

# else

#  define CRYPTO_THREAD_UNSET_STATE(THREAD, FLAG)
#  define CRYPTO_THREAD_SET_STATE(THREAD, FLAG)
#  define CRYPTO_THREAD_SET_ERROR(THREAD, FLAG)
#  define CRYPTO_THREAD_UNSET_ERROR(THREAD, FLAG)

# endif /* defined(OPENSSL_THREADS) */

CRYPTO_THREAD * ossl_crypto_thread_native_start(CRYPTO_THREAD_ROUTINE routine,
                                           void *data, int joinable);
int ossl_crypto_thread_native_spawn(CRYPTO_THREAD *thread);
int ossl_crypto_thread_native_join(CRYPTO_THREAD *thread,
                              CRYPTO_THREAD_RETVAL *retval);
int ossl_crypto_thread_native_perform_join(CRYPTO_THREAD *thread,
                                           CRYPTO_THREAD_RETVAL *retval);
int ossl_crypto_thread_native_exit(void);
int ossl_crypto_thread_native_is_self(CRYPTO_THREAD *thread);
int ossl_crypto_thread_native_clean(CRYPTO_THREAD *thread);

#endif /* OSSL_INTERNAL_THREAD_ARCH_H */
