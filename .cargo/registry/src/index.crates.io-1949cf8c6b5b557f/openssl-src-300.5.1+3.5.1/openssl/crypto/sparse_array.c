/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/bn.h>
#include "crypto/sparse_array.h"

/*
 * How many bits are used to index each level in the tree structure?
 * This setting determines the number of pointers stored in each node of the
 * tree used to represent the sparse array.  Having more pointers reduces the
 * depth of the tree but potentially wastes more memory.  That is, this is a
 * direct space versus time tradeoff.
 *
 * The default is to use four bits which means that there are 16
 * pointers in each tree node.
 *
 * The library builder is also permitted to define other sizes in the closed
 * interval [2, sizeof(ossl_uintmax_t) * 8].  Space use generally scales
 * exponentially with the block size, although the implementation only
 * creates enough blocks to support the largest used index.  The depth is:
 *      ceil(log_2(largest index) / 2^{block size})
 * E.g. with a block size of 4, and a largest index of 1000, the depth
 * will be three.
 */
#ifndef OPENSSL_SA_BLOCK_BITS
# define OPENSSL_SA_BLOCK_BITS           4
#elif OPENSSL_SA_BLOCK_BITS < 2 || OPENSSL_SA_BLOCK_BITS > (BN_BITS2 - 1)
# error OPENSSL_SA_BLOCK_BITS is out of range
#endif

/*
 * From the number of bits, work out:
 *    the number of pointers in a tree node;
 *    a bit mask to quickly extract an index and
 *    the maximum depth of the tree structure.
  */
#define SA_BLOCK_MAX            (1 << OPENSSL_SA_BLOCK_BITS)
#define SA_BLOCK_MASK           (SA_BLOCK_MAX - 1)
#define SA_BLOCK_MAX_LEVELS     (((int)sizeof(ossl_uintmax_t) * 8 \
                                  + OPENSSL_SA_BLOCK_BITS - 1) \
                                 / OPENSSL_SA_BLOCK_BITS)

struct sparse_array_st {
    int levels;
    ossl_uintmax_t top;
    size_t nelem;
    void **nodes;
};

OPENSSL_SA *ossl_sa_new(void)
{
    OPENSSL_SA *res = OPENSSL_zalloc(sizeof(*res));

    return res;
}

static void sa_doall(const OPENSSL_SA *sa, void (*node)(void **),
                     void (*leaf)(ossl_uintmax_t, void *, void *), void *arg)
{
    int i[SA_BLOCK_MAX_LEVELS];
    void *nodes[SA_BLOCK_MAX_LEVELS];
    ossl_uintmax_t idx = 0;
    int l = 0;

    i[0] = 0;
    nodes[0] = sa->nodes;
    while (l >= 0) {
        const int n = i[l];
        void ** const p = nodes[l];

        if (n >= SA_BLOCK_MAX) {
            if (p != NULL && node != NULL)
                (*node)(p);
            l--;
            idx >>= OPENSSL_SA_BLOCK_BITS;
        } else {
            i[l] = n + 1;
            if (p != NULL && p[n] != NULL) {
                idx = (idx & ~SA_BLOCK_MASK) | n;
                if (l < sa->levels - 1) {
                    i[++l] = 0;
                    nodes[l] = p[n];
                    idx <<= OPENSSL_SA_BLOCK_BITS;
                } else if (leaf != NULL) {
                    (*leaf)(idx, p[n], arg);
                }
            }
        }
    }
}

static void sa_free_node(void **p)
{
    OPENSSL_free(p);
}

static void sa_free_leaf(ossl_uintmax_t n, void *p, void *arg)
{
    OPENSSL_free(p);
}

void ossl_sa_free(OPENSSL_SA *sa)
{
    if (sa != NULL) {
        sa_doall(sa, &sa_free_node, NULL, NULL);
        OPENSSL_free(sa);
    }
}

void ossl_sa_free_leaves(OPENSSL_SA *sa)
{
    sa_doall(sa, &sa_free_node, &sa_free_leaf, NULL);
    OPENSSL_free(sa);
}

/* Wrap this in a structure to avoid compiler warnings */
struct trampoline_st {
    void (*func)(ossl_uintmax_t, void *);
};

static void trampoline(ossl_uintmax_t n, void *l, void *arg)
{
    ((const struct trampoline_st *)arg)->func(n, l);
}

void ossl_sa_doall(const OPENSSL_SA *sa, void (*leaf)(ossl_uintmax_t, void *))
{
    struct trampoline_st tramp;

    tramp.func = leaf;
    if (sa != NULL)
        sa_doall(sa, NULL, &trampoline, &tramp);
}

void ossl_sa_doall_arg(const OPENSSL_SA *sa,
                          void (*leaf)(ossl_uintmax_t, void *, void *),
                          void *arg)
{
    if (sa != NULL)
        sa_doall(sa, NULL, leaf, arg);
}

size_t ossl_sa_num(const OPENSSL_SA *sa)
{
    return sa == NULL ? 0 : sa->nelem;
}

void *ossl_sa_get(const OPENSSL_SA *sa, ossl_uintmax_t n)
{
    int level;
    void **p, *r = NULL;

    if (sa == NULL || sa->nelem == 0)
        return NULL;

    if (n <= sa->top) {
        p = sa->nodes;
        for (level = sa->levels - 1; p != NULL && level > 0; level--)
            p = (void **)p[(n >> (OPENSSL_SA_BLOCK_BITS * level))
                           & SA_BLOCK_MASK];
        r = p == NULL ? NULL : p[n & SA_BLOCK_MASK];
    }
    return r;
}

static ossl_inline void **alloc_node(void)
{
    return OPENSSL_zalloc(SA_BLOCK_MAX * sizeof(void *));
}

int ossl_sa_set(OPENSSL_SA *sa, ossl_uintmax_t posn, void *val)
{
    int i, level = 1;
    ossl_uintmax_t n = posn;
    void **p;

    if (sa == NULL)
        return 0;

    for (level = 1; level < SA_BLOCK_MAX_LEVELS; level++)
        if ((n >>= OPENSSL_SA_BLOCK_BITS) == 0)
            break;

    for (;sa->levels < level; sa->levels++) {
        p = alloc_node();
        if (p == NULL)
            return 0;
        p[0] = sa->nodes;
        sa->nodes = p;
    }
    if (sa->top < posn)
        sa->top = posn;

    p = sa->nodes;
    for (level = sa->levels - 1; level > 0; level--) {
        i = (posn >> (OPENSSL_SA_BLOCK_BITS * level)) & SA_BLOCK_MASK;
        if (p[i] == NULL && (p[i] = alloc_node()) == NULL)
            return 0;
        p = p[i];
    }
    p += posn & SA_BLOCK_MASK;
    if (val == NULL && *p != NULL)
        sa->nelem--;
    else if (val != NULL && *p == NULL)
        sa->nelem++;
    *p = val;
    return 1;
}
