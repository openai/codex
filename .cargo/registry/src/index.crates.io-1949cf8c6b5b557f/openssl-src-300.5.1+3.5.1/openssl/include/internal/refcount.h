/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_INTERNAL_REFCOUNT_H
# define OSSL_INTERNAL_REFCOUNT_H
# pragma once

# include <openssl/e_os2.h>
# include <openssl/trace.h>
# include <openssl/err.h>

# if defined(OPENSSL_THREADS) && !defined(OPENSSL_DEV_NO_ATOMICS)
#  if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L \
      && !defined(__STDC_NO_ATOMICS__)
#   include <stdatomic.h>
#   define HAVE_C11_ATOMICS
#  endif

#  if defined(HAVE_C11_ATOMICS) && defined(ATOMIC_INT_LOCK_FREE) \
      && ATOMIC_INT_LOCK_FREE > 0

#   define HAVE_ATOMICS 1

#   if defined(__has_feature)
#    if __has_feature(thread_sanitizer)
#     define OSSL_TSAN_BUILD
#    endif
#   endif

typedef struct {
    _Atomic int val;
} CRYPTO_REF_COUNT;

static inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = atomic_fetch_add_explicit(&refcnt->val, 1, memory_order_relaxed) + 1;
    return 1;
}

/*
 * Changes to shared structure other than reference counter have to be
 * serialized. And any kind of serialization implies a release fence. This
 * means that by the time reference counter is decremented all other
 * changes are visible on all processors. Hence decrement itself can be
 * relaxed. In case it hits zero, object will be destructed. Since it's
 * last use of the object, destructor programmer might reason that access
 * to mutable members doesn't have to be serialized anymore, which would
 * otherwise imply an acquire fence. Hence conditional acquire fence...
 */
static inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
#   ifdef OSSL_TSAN_BUILD
    /*
     * TSAN requires acq_rel as it indicates a false positive error when
     * the object that contains the refcount is freed otherwise.
     */
    *ret = atomic_fetch_sub_explicit(&refcnt->val, 1, memory_order_acq_rel) - 1;
#   else
    *ret = atomic_fetch_sub_explicit(&refcnt->val, 1, memory_order_release) - 1;
    if (*ret == 0)
        atomic_thread_fence(memory_order_acquire);
#   endif
    return 1;
}

static inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = atomic_load_explicit(&refcnt->val, memory_order_acquire);
    return 1;
}

#  elif defined(__GNUC__) && defined(__ATOMIC_RELAXED) && __GCC_ATOMIC_INT_LOCK_FREE > 0

#   define HAVE_ATOMICS 1

typedef struct {
    int val;
} CRYPTO_REF_COUNT;

static __inline__ int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = __atomic_fetch_add(&refcnt->val, 1, __ATOMIC_RELAXED) + 1;
    return 1;
}

static __inline__ int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = __atomic_fetch_sub(&refcnt->val, 1, __ATOMIC_RELEASE) - 1;
    if (*ret == 0)
        __atomic_thread_fence(__ATOMIC_ACQUIRE);
    return 1;
}

static __inline__ int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = __atomic_load_n(&refcnt->val, __ATOMIC_RELAXED);
    return 1;
}

#  elif defined(__ICL) && defined(_WIN32)
#   define HAVE_ATOMICS 1

typedef struct {
    volatile int val;
} CRYPTO_REF_COUNT;

static __inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd((void *)&refcnt->val, 1) + 1;
    return 1;
}

static __inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd((void *)&refcnt->val, -1) - 1;
    return 1;
}

static __inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd((void *)&refcnt->val, 0);
    return 1;
}

#  elif defined(_MSC_VER) && _MSC_VER>=1200

#   define HAVE_ATOMICS 1

typedef struct {
    volatile int val;
} CRYPTO_REF_COUNT;

#   if (defined(_M_ARM) && _M_ARM>=7 && !defined(_WIN32_WCE)) || defined(_M_ARM64)
#    include <intrin.h>
#    if defined(_M_ARM64) && !defined(_ARM_BARRIER_ISH)
#     define _ARM_BARRIER_ISH _ARM64_BARRIER_ISH
#    endif

static __inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd_nf(&refcnt->val, 1) + 1;
    return 1;
}

static __inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd(&refcnt->val, -1) - 1;
    return 1;
}

static __inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd_acq((void *)&refcnt->val, 0);
    return 1;
}

#   else
#    if !defined(_WIN32_WCE)
#     pragma intrinsic(_InterlockedExchangeAdd)
#    else
#     if _WIN32_WCE >= 0x600
       extern long __cdecl _InterlockedExchangeAdd(long volatile*, long);
#     else
       /* under Windows CE we still have old-style Interlocked* functions */
       extern long __cdecl InterlockedExchangeAdd(long volatile*, long);
#      define _InterlockedExchangeAdd InterlockedExchangeAdd
#     endif
#    endif

static __inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd(&refcnt->val, 1) + 1;
    return 1;
}

static __inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd(&refcnt->val, -1) - 1;
    return 1;
}

static __inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt, int *ret)
{
    *ret = _InterlockedExchangeAdd(&refcnt->val, 0);
    return 1;
}

#   endif

#  endif
# endif  /* !OPENSSL_DEV_NO_ATOMICS */

/*
 * All the refcounting implementations above define HAVE_ATOMICS, so if it's
 * still undefined here (such as when OPENSSL_DEV_NO_ATOMICS is defined), it
 * means we need to implement a fallback.  This fallback uses locks.
 */
# ifndef HAVE_ATOMICS

typedef struct {
    int val;
#  ifdef OPENSSL_THREADS
    CRYPTO_RWLOCK *lock;
#  endif
} CRYPTO_REF_COUNT;

#  ifdef OPENSSL_THREADS

static ossl_unused ossl_inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt,
                                                 int *ret)
{
    return CRYPTO_atomic_add(&refcnt->val, 1, ret, refcnt->lock);
}

static ossl_unused ossl_inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt,
                                                   int *ret)
{
    return CRYPTO_atomic_add(&refcnt->val, -1, ret, refcnt->lock);
}

static ossl_unused ossl_inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt,
                                                   int *ret)
{
    return CRYPTO_atomic_load_int(&refcnt->val, ret, refcnt->lock);
}

#   define CRYPTO_NEW_FREE_DEFINED  1
static ossl_unused ossl_inline int CRYPTO_NEW_REF(CRYPTO_REF_COUNT *refcnt, int n)
{
    refcnt->val = n;
    refcnt->lock = CRYPTO_THREAD_lock_new();
    if (refcnt->lock == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_CRYPTO_LIB);
        return 0;
    }
    return 1;
}

static ossl_unused ossl_inline void CRYPTO_FREE_REF(CRYPTO_REF_COUNT *refcnt)                                  \
{
    if (refcnt != NULL)
        CRYPTO_THREAD_lock_free(refcnt->lock);
}

#  else     /* OPENSSL_THREADS */

static ossl_unused ossl_inline int CRYPTO_UP_REF(CRYPTO_REF_COUNT *refcnt,
                                                 int *ret)
{
    refcnt->val++;
    *ret = refcnt->val;
    return 1;
}

static ossl_unused ossl_inline int CRYPTO_DOWN_REF(CRYPTO_REF_COUNT *refcnt,
                                                   int *ret)
{
    refcnt->val--;
    *ret = refcnt->val;
    return 1;
}

static ossl_unused ossl_inline int CRYPTO_GET_REF(CRYPTO_REF_COUNT *refcnt,
                                                   int *ret)
{
    *ret = refcnt->val;
    return 1;
}

#  endif    /* OPENSSL_THREADS */
# endif

# ifndef CRYPTO_NEW_FREE_DEFINED
static ossl_unused ossl_inline int CRYPTO_NEW_REF(CRYPTO_REF_COUNT *refcnt, int n)
{
    refcnt->val = n;
    return 1;
}

static ossl_unused ossl_inline void CRYPTO_FREE_REF(CRYPTO_REF_COUNT *refcnt)                                  \
{
}
# endif /* CRYPTO_NEW_FREE_DEFINED */
#undef CRYPTO_NEW_FREE_DEFINED

# if !defined(NDEBUG) && !defined(OPENSSL_NO_STDIO)
#  define REF_ASSERT_ISNT(test) \
    (void)((test) ? (OPENSSL_die("refcount error", __FILE__, __LINE__), 1) : 0)
# else
#  define REF_ASSERT_ISNT(i)
# endif

# define REF_PRINT_EX(text, count, object) \
    OSSL_TRACE3(REF_COUNT, "%p:%4d:%s\n", (object), (count), (text));
# define REF_PRINT_COUNT(text, val, object) \
    REF_PRINT_EX(text, val, (void *)object)

#endif
