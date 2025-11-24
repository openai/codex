/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/err.h>
#include <assert.h>
#include "internal/priority_queue.h"
#include "internal/safe_math.h"
#include "internal/numbers.h"

OSSL_SAFE_MATH_UNSIGNED(size_t, size_t)

/*
 * Fundamental operations:
 *                        Binary Heap   Fibonacci Heap
 *  Get smallest            O(1)          O(1)
 *  Delete any              O(log n)      O(log n) average but worst O(n)
 *  Insert                  O(log n)      O(1)
 *
 * Not supported:
 *  Merge two structures    O(log n)      O(1)
 *  Decrease key            O(log n)      O(1)
 *  Increase key            O(log n)      ?
 *
 * The Fibonacci heap is quite a bit more complicated to implement and has
 * larger overhead in practice.  We favour the binary heap here.  A multi-way
 * (ternary or quaternary) heap might elicit a performance advantage via better
 * cache access patterns.
 */

struct pq_heap_st {
    void *data;     /* User supplied data pointer */
    size_t index;   /* Constant index in elements[] */
};

struct pq_elem_st {
    size_t posn;    /* Current index in heap[] or link in free list */
#ifndef NDEBUG
    int used;       /* Debug flag indicating that this is in use */
#endif
};

struct ossl_pqueue_st {
    struct pq_heap_st *heap;
    struct pq_elem_st *elements;
    int (*compare)(const void *, const void *);
    size_t htop;        /* Highest used heap element */
    size_t hmax;        /* Allocated heap & element space */
    size_t freelist;    /* Index into elements[], start of free element list */
};

/*
 * The initial and maximum number of elements in the heap.
 */
static const size_t min_nodes = 8;
static const size_t max_nodes =
        SIZE_MAX / (sizeof(struct pq_heap_st) > sizeof(struct pq_elem_st)
                    ? sizeof(struct pq_heap_st) : sizeof(struct pq_elem_st));

#ifndef NDEBUG
/* Some basic sanity checking of the data structure */
# define ASSERT_USED(pq, idx)                                               \
    assert(pq->elements[pq->heap[idx].index].used);                         \
    assert(pq->elements[pq->heap[idx].index].posn == idx)
# define ASSERT_ELEM_USED(pq, elem)                                         \
    assert(pq->elements[elem].used)
#else
# define ASSERT_USED(pq, idx)
# define ASSERT_ELEM_USED(pq, elem)
#endif

/*
 * Calculate the array growth based on the target size.
 *
 * The growth factor is a rational number and is defined by a numerator
 * and a denominator.  According to Andrew Koenig in his paper "Why Are
 * Vectors Efficient?" from JOOP 11(5) 1998, this factor should be less
 * than the golden ratio (1.618...).
 *
 * We use an expansion factor of 8 / 5 = 1.6
 */
static ossl_inline size_t compute_pqueue_growth(size_t target, size_t current)
{
    int err = 0;

    while (current < target) {
        if (current >= max_nodes)
            return 0;

        current = safe_muldiv_size_t(current, 8, 5, &err);
        if (err)
            return 0;
        if (current >= max_nodes)
            current = max_nodes;
    }
    return current;
}

static ossl_inline void pqueue_swap_elem(OSSL_PQUEUE *pq, size_t i, size_t j)
{
    struct pq_heap_st *h = pq->heap, t_h;
    struct pq_elem_st *e = pq->elements;

    ASSERT_USED(pq, i);
    ASSERT_USED(pq, j);

    t_h = h[i];
    h[i] = h[j];
    h[j] = t_h;

    e[h[i].index].posn = i;
    e[h[j].index].posn = j;
}

static ossl_inline void pqueue_move_elem(OSSL_PQUEUE *pq, size_t from, size_t to)
{
    struct pq_heap_st *h = pq->heap;
    struct pq_elem_st *e = pq->elements;

    ASSERT_USED(pq, from);

    h[to] = h[from];
    e[h[to].index].posn = to;
}

/*
 * Force the specified element to the front of the heap.  This breaks
 * the heap partial ordering pre-condition.
 */
static ossl_inline void pqueue_force_bottom(OSSL_PQUEUE *pq, size_t n)
{
    ASSERT_USED(pq, n);
    while (n > 0) {
        const size_t p = (n - 1) / 2;

        ASSERT_USED(pq, p);
        pqueue_swap_elem(pq, n, p);
        n = p;
    }
}

/*
 * Move an element down to its correct position to restore the partial
 * order pre-condition.
 */
static ossl_inline void pqueue_move_down(OSSL_PQUEUE *pq, size_t n)
{
    struct pq_heap_st *h = pq->heap;

    ASSERT_USED(pq, n);
    while (n > 0) {
        const size_t p = (n - 1) / 2;

        ASSERT_USED(pq, p);
        if (pq->compare(h[n].data, h[p].data) >= 0)
            break;
        pqueue_swap_elem(pq, n, p);
        n = p;
    }
}

/*
 * Move an element up to its correct position to restore the partial
 * order pre-condition.
 */
static ossl_inline void pqueue_move_up(OSSL_PQUEUE *pq, size_t n)
{
    struct pq_heap_st *h = pq->heap;
    size_t p = n * 2 + 1;

    ASSERT_USED(pq, n);
    if (pq->htop > p + 1) {
        ASSERT_USED(pq, p);
        ASSERT_USED(pq, p + 1);
        if (pq->compare(h[p].data, h[p + 1].data) > 0)
            p++;
    }
    while (pq->htop > p && pq->compare(h[p].data, h[n].data) < 0) {
        ASSERT_USED(pq, p);
        pqueue_swap_elem(pq, n, p);
        n = p;
        p = n * 2 + 1;
        if (pq->htop > p + 1) {
            ASSERT_USED(pq, p + 1);
            if (pq->compare(h[p].data, h[p + 1].data) > 0)
                p++;
        }
    }
}

int ossl_pqueue_push(OSSL_PQUEUE *pq, void *data, size_t *elem)
{
    size_t n, m;

    if (!ossl_pqueue_reserve(pq, 1))
        return 0;

    n = pq->htop++;
    m = pq->freelist;
    pq->freelist = pq->elements[m].posn;

    pq->heap[n].data = data;
    pq->heap[n].index = m;

    pq->elements[m].posn = n;
#ifndef NDEBUG
    pq->elements[m].used = 1;
#endif
    pqueue_move_down(pq, n);
    if (elem != NULL)
        *elem = m;
    return 1;
}

void *ossl_pqueue_peek(const OSSL_PQUEUE *pq)
{
    if (pq->htop > 0) {
        ASSERT_USED(pq, 0);
        return pq->heap->data;
    }
    return NULL;
}

void *ossl_pqueue_pop(OSSL_PQUEUE *pq)
{
    void *res;
    size_t elem;

    if (pq == NULL || pq->htop == 0)
        return NULL;

    ASSERT_USED(pq, 0);
    res = pq->heap->data;
    elem = pq->heap->index;

    if (--pq->htop != 0) {
        pqueue_move_elem(pq, pq->htop, 0);
        pqueue_move_up(pq, 0);
    }

    pq->elements[elem].posn = pq->freelist;
    pq->freelist = elem;
#ifndef NDEBUG
    pq->elements[elem].used = 0;
#endif
    return res;
}

void *ossl_pqueue_remove(OSSL_PQUEUE *pq, size_t elem)
{
    size_t n;

    if (pq == NULL || elem >= pq->hmax || pq->htop == 0)
        return 0;

    ASSERT_ELEM_USED(pq, elem);
    n = pq->elements[elem].posn;

    ASSERT_USED(pq, n);

    if (n == pq->htop - 1) {
        pq->elements[elem].posn = pq->freelist;
        pq->freelist = elem;
#ifndef NDEBUG
        pq->elements[elem].used = 0;
#endif
        return pq->heap[--pq->htop].data;
    }
    if (n > 0)
        pqueue_force_bottom(pq, n);
    return ossl_pqueue_pop(pq);
}

static void pqueue_add_freelist(OSSL_PQUEUE *pq, size_t from)
{
    struct pq_elem_st *e = pq->elements;
    size_t i;

#ifndef NDEBUG
    for (i = from; i < pq->hmax; i++)
        e[i].used = 0;
#endif
    e[from].posn = pq->freelist;
    for (i = from + 1; i < pq->hmax; i++)
        e[i].posn = i - 1;
    pq->freelist = pq->hmax - 1;
}

int ossl_pqueue_reserve(OSSL_PQUEUE *pq, size_t n)
{
    size_t new_max, cur_max;
    struct pq_heap_st *h;
    struct pq_elem_st *e;

    if (pq == NULL)
        return 0;
    cur_max = pq->hmax;
    if (pq->htop + n < cur_max)
        return 1;

    new_max = compute_pqueue_growth(n + cur_max, cur_max);
    if (new_max == 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    h = OPENSSL_realloc(pq->heap, new_max * sizeof(*pq->heap));
    if (h == NULL)
        return 0;
    pq->heap = h;

    e = OPENSSL_realloc(pq->elements, new_max * sizeof(*pq->elements));
    if (e == NULL)
        return 0;
    pq->elements = e;

    pq->hmax = new_max;
    pqueue_add_freelist(pq, cur_max);
    return 1;
}

OSSL_PQUEUE *ossl_pqueue_new(int (*compare)(const void *, const void *))
{
    OSSL_PQUEUE *pq;

    if (compare == NULL)
        return NULL;

    pq = OPENSSL_malloc(sizeof(*pq));
    if (pq == NULL)
        return NULL;
    pq->compare = compare;
    pq->hmax = min_nodes;
    pq->htop = 0;
    pq->freelist = 0;
    pq->heap = OPENSSL_malloc(sizeof(*pq->heap) * min_nodes);
    pq->elements = OPENSSL_malloc(sizeof(*pq->elements) * min_nodes);
    if (pq->heap == NULL || pq->elements == NULL) {
        ossl_pqueue_free(pq);
        return NULL;
    }
    pqueue_add_freelist(pq, 0);
    return pq;
}

void ossl_pqueue_free(OSSL_PQUEUE *pq)
{
    if (pq != NULL) {
        OPENSSL_free(pq->heap);
        OPENSSL_free(pq->elements);
        OPENSSL_free(pq);
    }
}

void ossl_pqueue_pop_free(OSSL_PQUEUE *pq, void (*freefunc)(void *))
{
    size_t i;

    if (pq != NULL) {
        for (i = 0; i < pq->htop; i++)
            (*freefunc)(pq->heap[i].data);
        ossl_pqueue_free(pq);
    }
}

size_t ossl_pqueue_num(const OSSL_PQUEUE *pq)
{
    return pq != NULL ? pq->htop : 0;
}
