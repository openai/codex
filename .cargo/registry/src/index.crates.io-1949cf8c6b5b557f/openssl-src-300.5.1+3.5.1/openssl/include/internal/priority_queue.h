/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PRIORITY_QUEUE_H
# define OSSL_INTERNAL_PRIORITY_QUEUE_H
# pragma once

# include <stdlib.h>
# include <openssl/e_os2.h>

# define PRIORITY_QUEUE_OF(type) OSSL_PRIORITY_QUEUE_ ## type

# define DEFINE_PRIORITY_QUEUE_OF_INTERNAL(type, ctype)                     \
    typedef struct ossl_priority_queue_st_ ## type PRIORITY_QUEUE_OF(type); \
    static ossl_unused ossl_inline PRIORITY_QUEUE_OF(type) *                \
    ossl_pqueue_##type##_new(int (*compare)(const ctype *, const ctype *))  \
    {                                                                       \
        return (PRIORITY_QUEUE_OF(type) *)ossl_pqueue_new(                  \
                    (int (*)(const void *, const void *))compare);          \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_pqueue_##type##_free(PRIORITY_QUEUE_OF(type) *pq)                  \
    {                                                                       \
        ossl_pqueue_free((OSSL_PQUEUE *)pq);                                \
    }                                                                       \
    static ossl_unused ossl_inline void                                     \
    ossl_pqueue_##type##_pop_free(PRIORITY_QUEUE_OF(type) *pq,              \
                                  void (*freefunc)(ctype *))                \
    {                                                                       \
        ossl_pqueue_pop_free((OSSL_PQUEUE *)pq, (void (*)(void *))freefunc);\
    }                                                                       \
    static ossl_unused ossl_inline int                                      \
    ossl_pqueue_##type##_reserve(PRIORITY_QUEUE_OF(type) *pq, size_t n)     \
    {                                                                       \
        return ossl_pqueue_reserve((OSSL_PQUEUE *)pq, n);                   \
    }                                                                       \
    static ossl_unused ossl_inline size_t                                   \
    ossl_pqueue_##type##_num(const PRIORITY_QUEUE_OF(type) *pq)             \
    {                                                                       \
        return ossl_pqueue_num((OSSL_PQUEUE *)pq);                          \
    }                                                                       \
    static ossl_unused ossl_inline int                                      \
    ossl_pqueue_##type##_push(PRIORITY_QUEUE_OF(type) *pq,                  \
                              ctype *data, size_t *elem)                    \
    {                                                                       \
        return ossl_pqueue_push((OSSL_PQUEUE *)pq, (void *)data, elem);     \
    }                                                                       \
    static ossl_unused ossl_inline ctype *                                  \
    ossl_pqueue_##type##_peek(const PRIORITY_QUEUE_OF(type) *pq)            \
    {                                                                       \
        return (type *)ossl_pqueue_peek((OSSL_PQUEUE *)pq);                 \
    }                                                                       \
    static ossl_unused ossl_inline ctype *                                  \
    ossl_pqueue_##type##_pop(PRIORITY_QUEUE_OF(type) *pq)                   \
    {                                                                       \
        return (type *)ossl_pqueue_pop((OSSL_PQUEUE *)pq);                  \
    }                                                                       \
    static ossl_unused ossl_inline ctype *                                  \
    ossl_pqueue_##type##_remove(PRIORITY_QUEUE_OF(type) *pq,                \
                                size_t elem)                                \
    {                                                                       \
        return (type *)ossl_pqueue_remove((OSSL_PQUEUE *)pq, elem);         \
    }                                                                       \
    struct ossl_priority_queue_st_ ## type

# define DEFINE_PRIORITY_QUEUE_OF(type) \
    DEFINE_PRIORITY_QUEUE_OF_INTERNAL(type, type)

typedef struct ossl_pqueue_st OSSL_PQUEUE;

OSSL_PQUEUE *ossl_pqueue_new(int (*compare)(const void *, const void *));
void ossl_pqueue_free(OSSL_PQUEUE *pq);
void ossl_pqueue_pop_free(OSSL_PQUEUE *pq, void (*freefunc)(void *));
int ossl_pqueue_reserve(OSSL_PQUEUE *pq, size_t n);

size_t ossl_pqueue_num(const OSSL_PQUEUE *pq);
int ossl_pqueue_push(OSSL_PQUEUE *pq, void *data, size_t *elem);
void *ossl_pqueue_peek(const OSSL_PQUEUE *pq);
void *ossl_pqueue_pop(OSSL_PQUEUE *pq);
void *ossl_pqueue_remove(OSSL_PQUEUE *pq, size_t elem);

#endif
