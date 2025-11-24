/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include "internal/safe_math.h"
#include <openssl/stack.h>
#include <errno.h>
#include <openssl/e_os2.h>      /* For ossl_inline */

OSSL_SAFE_MATH_SIGNED(int, int)

/*
 * The initial number of nodes in the array.
 */
static const int min_nodes = 4;
static const int max_nodes = SIZE_MAX / sizeof(void *) < INT_MAX
    ? (int)(SIZE_MAX / sizeof(void *)) : INT_MAX;

struct stack_st {
    int num;
    const void **data;
    int sorted;
    int num_alloc;
    OPENSSL_sk_compfunc comp;
};

OPENSSL_sk_compfunc OPENSSL_sk_set_cmp_func(OPENSSL_STACK *sk,
                                            OPENSSL_sk_compfunc c)
{
    OPENSSL_sk_compfunc old = sk->comp;

    if (sk->comp != c)
        sk->sorted = 0;
    sk->comp = c;

    return old;
}

OPENSSL_STACK *OPENSSL_sk_dup(const OPENSSL_STACK *sk)
{
    OPENSSL_STACK *ret;

    if ((ret = OPENSSL_malloc(sizeof(*ret))) == NULL)
        goto err;

    if (sk == NULL) {
        ret->num = 0;
        ret->sorted = 0;
        ret->comp = NULL;
    } else {
        /* direct structure assignment */
        *ret = *sk;
    }

    if (sk == NULL || sk->num == 0) {
        /* postpone |ret->data| allocation */
        ret->data = NULL;
        ret->num_alloc = 0;
        return ret;
    }

    /* duplicate |sk->data| content */
    ret->data = OPENSSL_malloc(sizeof(*ret->data) * sk->num_alloc);
    if (ret->data == NULL)
        goto err;
    memcpy(ret->data, sk->data, sizeof(void *) * sk->num);
    return ret;

 err:
    OPENSSL_sk_free(ret);
    return NULL;
}

OPENSSL_STACK *OPENSSL_sk_deep_copy(const OPENSSL_STACK *sk,
                                    OPENSSL_sk_copyfunc copy_func,
                                    OPENSSL_sk_freefunc free_func)
{
    OPENSSL_STACK *ret;
    int i;

    if ((ret = OPENSSL_malloc(sizeof(*ret))) == NULL)
        goto err;

    if (sk == NULL) {
        ret->num = 0;
        ret->sorted = 0;
        ret->comp = NULL;
    } else {
        /* direct structure assignment */
        *ret = *sk;
    }

    if (sk == NULL || sk->num == 0) {
        /* postpone |ret| data allocation */
        ret->data = NULL;
        ret->num_alloc = 0;
        return ret;
    }

    ret->num_alloc = sk->num > min_nodes ? sk->num : min_nodes;
    ret->data = OPENSSL_zalloc(sizeof(*ret->data) * ret->num_alloc);
    if (ret->data == NULL)
        goto err;

    for (i = 0; i < ret->num; ++i) {
        if (sk->data[i] == NULL)
            continue;
        if ((ret->data[i] = copy_func(sk->data[i])) == NULL) {
            while (--i >= 0)
                if (ret->data[i] != NULL)
                    free_func((void *)ret->data[i]);
            goto err;
        }
    }
    return ret;

 err:
    OPENSSL_sk_free(ret);
    return NULL;
}

OPENSSL_STACK *OPENSSL_sk_new_null(void)
{
    return OPENSSL_sk_new_reserve(NULL, 0);
}

OPENSSL_STACK *OPENSSL_sk_new(OPENSSL_sk_compfunc c)
{
    return OPENSSL_sk_new_reserve(c, 0);
}

/*
 * Calculate the array growth based on the target size.
 *
 * The growth factor is a rational number and is defined by a numerator
 * and a denominator.  According to Andrew Koenig in his paper "Why Are
 * Vectors Efficient?" from JOOP 11(5) 1998, this factor should be less
 * than the golden ratio (1.618...).
 *
 * Considering only the Fibonacci ratios less than the golden ratio, the
 * number of steps from the minimum allocation to integer overflow is:
 *      factor  decimal    growths
 *       3/2     1.5          51
 *       8/5     1.6          45
 *      21/13    1.615...     44
 *
 * All larger factors have the same number of growths.
 *
 * 3/2 and 8/5 have nice power of two shifts, so seem like a good choice.
 */
static ossl_inline int compute_growth(int target, int current)
{
    int err = 0;

    while (current < target) {
        if (current >= max_nodes)
            return 0;

        current = safe_muldiv_int(current, 8, 5, &err);
        if (err != 0)
            return 0;
        if (current >= max_nodes)
            current = max_nodes;
    }
    return current;
}

/* internal STACK storage allocation */
static int sk_reserve(OPENSSL_STACK *st, int n, int exact)
{
    const void **tmpdata;
    int num_alloc;

    /* Check to see the reservation isn't exceeding the hard limit */
    if (n > max_nodes - st->num) {
        ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_MANY_RECORDS);
        return 0;
    }

    /* Figure out the new size */
    num_alloc = st->num + n;
    if (num_alloc < min_nodes)
        num_alloc = min_nodes;

    /* If |st->data| allocation was postponed */
    if (st->data == NULL) {
        /*
         * At this point, |st->num_alloc| and |st->num| are 0;
         * so |num_alloc| value is |n| or |min_nodes| if greater than |n|.
         */
        if ((st->data = OPENSSL_zalloc(sizeof(void *) * num_alloc)) == NULL)
            return 0;
        st->num_alloc = num_alloc;
        return 1;
    }

    if (!exact) {
        if (num_alloc <= st->num_alloc)
            return 1;
        num_alloc = compute_growth(num_alloc, st->num_alloc);
        if (num_alloc == 0) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_MANY_RECORDS);
            return 0;
        }
    } else if (num_alloc == st->num_alloc) {
        return 1;
    }

    tmpdata = OPENSSL_realloc((void *)st->data, sizeof(void *) * num_alloc);
    if (tmpdata == NULL)
        return 0;

    st->data = tmpdata;
    st->num_alloc = num_alloc;
    return 1;
}

OPENSSL_STACK *OPENSSL_sk_new_reserve(OPENSSL_sk_compfunc c, int n)
{
    OPENSSL_STACK *st = OPENSSL_zalloc(sizeof(OPENSSL_STACK));

    if (st == NULL)
        return NULL;

    st->comp = c;

    if (n <= 0)
        return st;

    if (!sk_reserve(st, n, 1)) {
        OPENSSL_sk_free(st);
        return NULL;
    }

    return st;
}

int OPENSSL_sk_reserve(OPENSSL_STACK *st, int n)
{
    if (st == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (n < 0)
        return 1;
    return sk_reserve(st, n, 1);
}

int OPENSSL_sk_insert(OPENSSL_STACK *st, const void *data, int loc)
{
    if (st == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (st->num == max_nodes) {
        ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_MANY_RECORDS);
        return 0;
    }

    if (!sk_reserve(st, 1, 0))
        return 0;

    if ((loc >= st->num) || (loc < 0)) {
        st->data[st->num] = data;
    } else {
        memmove(&st->data[loc + 1], &st->data[loc],
                sizeof(st->data[0]) * (st->num - loc));
        st->data[loc] = data;
    }
    st->num++;
    st->sorted = 0;
    return st->num;
}

static ossl_inline void *internal_delete(OPENSSL_STACK *st, int loc)
{
    const void *ret = st->data[loc];

    if (loc != st->num - 1)
        memmove(&st->data[loc], &st->data[loc + 1],
                sizeof(st->data[0]) * (st->num - loc - 1));
    st->num--;

    return (void *)ret;
}

void *OPENSSL_sk_delete_ptr(OPENSSL_STACK *st, const void *p)
{
    int i;

    if (st == NULL)
        return NULL;

    for (i = 0; i < st->num; i++)
        if (st->data[i] == p)
            return internal_delete(st, i);
    return NULL;
}

void *OPENSSL_sk_delete(OPENSSL_STACK *st, int loc)
{
    if (st == NULL || loc < 0 || loc >= st->num)
        return NULL;

    return internal_delete(st, loc);
}

static int internal_find(OPENSSL_STACK *st, const void *data,
                         int ret_val_options, int *pnum_matched)
{
    const void *r;
    int i, count = 0;
    int *pnum = pnum_matched;

    if (st == NULL || st->num == 0)
        return -1;

    if (pnum == NULL)
        pnum = &count;

    if (st->comp == NULL) {
        for (i = 0; i < st->num; i++)
            if (st->data[i] == data) {
                *pnum = 1;
                return i;
            }
        *pnum = 0;
        return -1;
    }

    if (data == NULL)
        return -1;

    if (!st->sorted) {
        int res = -1;

        for (i = 0; i < st->num; i++)
            if (st->comp(&data, st->data + i) == 0) {
                if (res == -1)
                    res = i;
                ++*pnum;
                /* Check if only one result is wanted and exit if so */
                if (pnum_matched == NULL)
                    return i;
            }
        if (res == -1)
            *pnum = 0;
        return res;
    }

    if (pnum_matched != NULL)
        ret_val_options |= OSSL_BSEARCH_FIRST_VALUE_ON_MATCH;
    r = ossl_bsearch(&data, st->data, st->num, sizeof(void *), st->comp,
                     ret_val_options);

    if (pnum_matched != NULL) {
        *pnum = 0;
        if (r != NULL) {
            const void **p = (const void **)r;

            while (p < st->data + st->num) {
                if (st->comp(&data, p) != 0)
                    break;
                ++*pnum;
                ++p;
            }
        }
    }

    return r == NULL ? -1 : (int)((const void **)r - st->data);
}

int OPENSSL_sk_find(OPENSSL_STACK *st, const void *data)
{
    return internal_find(st, data, OSSL_BSEARCH_FIRST_VALUE_ON_MATCH, NULL);
}

int OPENSSL_sk_find_ex(OPENSSL_STACK *st, const void *data)
{
    return internal_find(st, data, OSSL_BSEARCH_VALUE_ON_NOMATCH, NULL);
}

int OPENSSL_sk_find_all(OPENSSL_STACK *st, const void *data, int *pnum)
{
    return internal_find(st, data, OSSL_BSEARCH_FIRST_VALUE_ON_MATCH, pnum);
}

int OPENSSL_sk_push(OPENSSL_STACK *st, const void *data)
{
    if (st == NULL)
        return 0;
    return OPENSSL_sk_insert(st, data, st->num);
}

int OPENSSL_sk_unshift(OPENSSL_STACK *st, const void *data)
{
    return OPENSSL_sk_insert(st, data, 0);
}

void *OPENSSL_sk_shift(OPENSSL_STACK *st)
{
    if (st == NULL || st->num == 0)
        return NULL;
    return internal_delete(st, 0);
}

void *OPENSSL_sk_pop(OPENSSL_STACK *st)
{
    if (st == NULL || st->num == 0)
        return NULL;
    return internal_delete(st, st->num - 1);
}

void OPENSSL_sk_zero(OPENSSL_STACK *st)
{
    if (st == NULL || st->num == 0)
        return;
    memset(st->data, 0, sizeof(*st->data) * st->num);
    st->num = 0;
}

void OPENSSL_sk_pop_free(OPENSSL_STACK *st, OPENSSL_sk_freefunc func)
{
    int i;

    if (st == NULL)
        return;
    for (i = 0; i < st->num; i++)
        if (st->data[i] != NULL)
            func((char *)st->data[i]);
    OPENSSL_sk_free(st);
}

void OPENSSL_sk_free(OPENSSL_STACK *st)
{
    if (st == NULL)
        return;
    OPENSSL_free(st->data);
    OPENSSL_free(st);
}

int OPENSSL_sk_num(const OPENSSL_STACK *st)
{
    return st == NULL ? -1 : st->num;
}

void *OPENSSL_sk_value(const OPENSSL_STACK *st, int i)
{
    if (st == NULL || i < 0 || i >= st->num)
        return NULL;
    return (void *)st->data[i];
}

void *OPENSSL_sk_set(OPENSSL_STACK *st, int i, const void *data)
{
    if (st == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (i < 0 || i >= st->num) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT,
                       "i=%d", i);
        return NULL;
    }
    st->data[i] = data;
    st->sorted = 0;
    return (void *)st->data[i];
}

void OPENSSL_sk_sort(OPENSSL_STACK *st)
{
    if (st != NULL && !st->sorted && st->comp != NULL) {
        if (st->num > 1)
            qsort(st->data, st->num, sizeof(void *), st->comp);
        st->sorted = 1; /* empty or single-element stack is considered sorted */
    }
}

int OPENSSL_sk_is_sorted(const OPENSSL_STACK *st)
{
    return st == NULL ? 1 : st->sorted;
}
