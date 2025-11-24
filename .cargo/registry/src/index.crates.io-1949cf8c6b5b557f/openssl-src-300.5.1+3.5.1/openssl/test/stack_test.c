/*
 * Copyright 2017-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2017, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/safestack.h>
#include <openssl/err.h>
#include <openssl/crypto.h>

#include "internal/nelem.h"
#include "testutil.h"

/* The macros below generate unused functions which error out one of the clang
 * builds.  We disable this check here.
 */
#ifdef __clang__
#pragma clang diagnostic ignored "-Wunused-function"
#endif

typedef struct {
    int n;
    char c;
} SS;

typedef union {
    int n;
    char c;
} SU;

DEFINE_SPECIAL_STACK_OF(sint, int)
DEFINE_SPECIAL_STACK_OF_CONST(uchar, unsigned char)
DEFINE_STACK_OF(SS)
DEFINE_STACK_OF_CONST(SU)

static int int_compare(const int *const *a, const int *const *b)
{
    if (**a < **b)
        return -1;
    if (**a > **b)
        return 1;
    return 0;
}

static int test_int_stack(int reserve)
{
    static int v[] = { 1, 2, -4, 16, 999, 1, -173, 1, 9 };
    static int notpresent = -1;
    const int n = OSSL_NELEM(v);
    static struct {
        int value;
        int unsorted;
        int sorted;
        int ex;
    } finds[] = {
        { 2,    1,  5,  5   },
        { 9,    7,  6,  6   },
        { -173, 5,  0,  0   },
        { 999,  3,  8,  8   },
        { 0,   -1, -1,  1   }
    };
    const int n_finds = OSSL_NELEM(finds);
    static struct {
        int value;
        int ex;
    } exfinds[] = {
        { 3,    5   },
        { 1000, 8   },
        { 20,   8   },
        { -999, 0   },
        { -5,   0   },
        { 8,    5   }
    };
    const int n_exfinds = OSSL_NELEM(exfinds);
    STACK_OF(sint) *s = sk_sint_new_null();
    int i;
    int testresult = 0;

    if (!TEST_ptr(s)
        || (reserve > 0 && !TEST_true(sk_sint_reserve(s, 5 * reserve))))
        goto end;

    /* Check push and num */
    for (i = 0; i < n; i++) {
        if (!TEST_int_eq(sk_sint_num(s), i)) {
            TEST_info("int stack size %d", i);
            goto end;
        }
        sk_sint_push(s, v + i);
    }
    if (!TEST_int_eq(sk_sint_num(s), n))
        goto end;

    /* check the values */
    for (i = 0; i < n; i++)
        if (!TEST_ptr_eq(sk_sint_value(s, i), v + i)) {
            TEST_info("int value %d", i);
            goto end;
        }

    /* find unsorted -- the pointers are compared */
    for (i = 0; i < n_finds; i++) {
        int *val = (finds[i].unsorted == -1) ? &notpresent
                                             : v + finds[i].unsorted;

        if (!TEST_int_eq(sk_sint_find(s, val), finds[i].unsorted)) {
            TEST_info("int unsorted find %d", i);
            goto end;
        }
    }

    /* find_ex unsorted */
    for (i = 0; i < n_finds; i++) {
        int *val = (finds[i].unsorted == -1) ? &notpresent
                                             : v + finds[i].unsorted;

        if (!TEST_int_eq(sk_sint_find_ex(s, val), finds[i].unsorted)) {
            TEST_info("int unsorted find_ex %d", i);
            goto end;
        }
    }

    /* sorting */
    if (!TEST_false(sk_sint_is_sorted(s)))
        goto end;
    (void)sk_sint_set_cmp_func(s, &int_compare);
    sk_sint_sort(s);
    if (!TEST_true(sk_sint_is_sorted(s)))
        goto end;

    /* find sorted -- the value is matched so we don't need to locate it */
    for (i = 0; i < n_finds; i++)
        if (!TEST_int_eq(sk_sint_find(s, &finds[i].value), finds[i].sorted)) {
            TEST_info("int sorted find %d", i);
            goto end;
        }

    /* find_ex sorted */
    for (i = 0; i < n_finds; i++)
        if (!TEST_int_eq(sk_sint_find_ex(s, &finds[i].value), finds[i].ex)) {
            TEST_info("int sorted find_ex present %d", i);
            goto end;
        }
    for (i = 0; i < n_exfinds; i++)
        if (!TEST_int_eq(sk_sint_find_ex(s, &exfinds[i].value), exfinds[i].ex)) {
            TEST_info("int sorted find_ex absent %d", i);
            goto end;
        }

    /* shift */
    if (!TEST_ptr_eq(sk_sint_shift(s), v + 6))
        goto end;

    testresult = 1;
end:
    sk_sint_free(s);
    return testresult;
}

static int uchar_compare(const unsigned char *const *a,
                         const unsigned char *const *b)
{
    return **a - (signed int)**b;
}

static int test_uchar_stack(int reserve)
{
    static const unsigned char v[] = { 1, 3, 7, 5, 255, 0 };
    const int n = OSSL_NELEM(v);
    STACK_OF(uchar) *s = sk_uchar_new(&uchar_compare), *r = NULL;
    int i;
    int testresult = 0;

    if (!TEST_ptr(s)
        || (reserve > 0 && !TEST_true(sk_uchar_reserve(s, 5 * reserve))))
        goto end;

    /* unshift and num */
    for (i = 0; i < n; i++) {
        if (!TEST_int_eq(sk_uchar_num(s), i)) {
            TEST_info("uchar stack size %d", i);
            goto end;
        }
        sk_uchar_unshift(s, v + i);
    }
    if (!TEST_int_eq(sk_uchar_num(s), n))
        goto end;

    /* dup */
    r = sk_uchar_dup(NULL);
    if (sk_uchar_num(r) != 0)
        goto end;
    sk_uchar_free(r);
    r = sk_uchar_dup(s);
    if (!TEST_int_eq(sk_uchar_num(r), n))
        goto end;
    sk_uchar_sort(r);

    /* pop */
    for (i = 0; i < n; i++)
        if (!TEST_ptr_eq(sk_uchar_pop(s), v + i)) {
            TEST_info("uchar pop %d", i);
            goto end;
        }

    /* free -- we rely on the debug malloc to detect leakage here */
    sk_uchar_free(s);
    s = NULL;

    /* dup again */
    if (!TEST_int_eq(sk_uchar_num(r), n))
        goto end;

    /* zero */
    sk_uchar_zero(r);
    if (!TEST_int_eq(sk_uchar_num(r), 0))
        goto end;

    /* insert */
    sk_uchar_insert(r, v, 0);
    sk_uchar_insert(r, v + 2, -1);
    sk_uchar_insert(r, v + 1, 1);
    for (i = 0; i < 3; i++)
        if (!TEST_ptr_eq(sk_uchar_value(r, i), v + i)) {
            TEST_info("uchar insert %d", i);
            goto end;
        }

    /* delete */
    if (!TEST_ptr_null(sk_uchar_delete(r, 12)))
        goto end;
    if (!TEST_ptr_eq(sk_uchar_delete(r, 1), v + 1))
        goto end;

    /* set */
    (void)sk_uchar_set(r, 1, v + 1);
    for (i = 0; i < 2; i++)
        if (!TEST_ptr_eq(sk_uchar_value(r, i), v + i)) {
            TEST_info("uchar set %d", i);
            goto end;
        }

    testresult = 1;
end:
    sk_uchar_free(r);
    sk_uchar_free(s);
    return testresult;
}

static SS *SS_copy(const SS *p)
{
    SS *q = OPENSSL_malloc(sizeof(*q));

    if (q != NULL)
        memcpy(q, p, sizeof(*q));
    return q;
}

static void SS_free(SS *p) {
    OPENSSL_free(p);
}

static int test_SS_stack(void)
{
    STACK_OF(SS) *s = sk_SS_new_null();
    STACK_OF(SS) *r = NULL;
    SS *v[10], *p;
    const int n = OSSL_NELEM(v);
    int i;
    int testresult = 0;

    /* allocate and push */
    for (i = 0; i < n; i++) {
        v[i] = OPENSSL_malloc(sizeof(*v[i]));

        if (!TEST_ptr(v[i]))
            goto end;
        v[i]->n = i;
        v[i]->c = 'A' + i;
        if (!TEST_int_eq(sk_SS_num(s), i)) {
            TEST_info("SS stack size %d", i);
            goto end;
        }
        sk_SS_push(s, v[i]);
    }
    if (!TEST_int_eq(sk_SS_num(s), n))
        goto end;

    /* deepcopy */
    r = sk_SS_deep_copy(NULL, &SS_copy, &SS_free);
    if (sk_SS_num(r) != 0)
        goto end;
    sk_SS_free(r);
    r = sk_SS_deep_copy(s, &SS_copy, &SS_free);
    if (!TEST_ptr(r))
        goto end;
    for (i = 0; i < n; i++) {
        p = sk_SS_value(r, i);
        if (!TEST_ptr_ne(p, v[i])) {
            TEST_info("SS deepcopy non-copy %d", i);
            goto end;
        }
        if (!TEST_int_eq(p->n, v[i]->n)) {
            TEST_info("test SS deepcopy int %d", i);
            goto end;
        }
        if (!TEST_char_eq(p->c, v[i]->c)) {
            TEST_info("SS deepcopy char %d", i);
            goto end;
        }
    }

    /* pop_free - we rely on the malloc debug to catch the leak */
    sk_SS_pop_free(r, &SS_free);
    r = NULL;

    /* delete_ptr */
    p = sk_SS_delete_ptr(s, v[3]);
    if (!TEST_ptr(p))
        goto end;
    SS_free(p);
    if (!TEST_int_eq(sk_SS_num(s), n - 1))
        goto end;
    for (i = 0; i < n-1; i++)
        if (!TEST_ptr_eq(sk_SS_value(s, i), v[i<3 ? i : 1+i])) {
            TEST_info("SS delete ptr item %d", i);
            goto end;
        }

    testresult = 1;
end:
    sk_SS_pop_free(r, &SS_free);
    sk_SS_pop_free(s, &SS_free);
    return testresult;
}

static int test_SU_stack(void)
{
    STACK_OF(SU) *s = sk_SU_new_null();
    SU v[10];
    const int n = OSSL_NELEM(v);
    int i;
    int testresult = 0;

    /* allocate and push */
    for (i = 0; i < n; i++) {
        if ((i & 1) == 0)
            v[i].n = i;
        else
            v[i].c = 'A' + i;
        if (!TEST_int_eq(sk_SU_num(s), i)) {
            TEST_info("SU stack size %d", i);
            goto end;
        }
        sk_SU_push(s, v + i);
    }
    if (!TEST_int_eq(sk_SU_num(s), n))
        goto end;

    /* check the pointers are correct */
    for (i = 0; i < n; i++)
        if (!TEST_ptr_eq(sk_SU_value(s, i),  v + i)) {
            TEST_info("SU pointer check %d", i);
            goto end;
        }

    testresult = 1;
end:
    sk_SU_free(s);
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_int_stack, 4);
    ADD_ALL_TESTS(test_uchar_stack, 4);
    ADD_TEST(test_SS_stack);
    ADD_TEST(test_SU_stack);
    return 1;
}
