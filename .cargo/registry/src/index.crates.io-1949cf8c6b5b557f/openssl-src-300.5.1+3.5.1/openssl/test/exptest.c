/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "internal/nelem.h"

#include <openssl/bio.h>
#include <openssl/bn.h>
#include <openssl/rand.h>
#include <openssl/err.h>

#include "testutil.h"

#define NUM_BITS        (BN_BITS2 * 4)

#define BN_print_var(v) test_output_bignum(#v, v)

/*
 * Test that r == 0 in test_exp_mod_zero(). Returns one on success,
 * returns zero and prints debug output otherwise.
 */
static int a_is_zero_mod_one(const char *method, const BIGNUM *r,
                             const BIGNUM *a)
{
    if (!BN_is_zero(r)) {
        TEST_error("%s failed: a ** 0 mod 1 = r (should be 0)", method);
        BN_print_var(a);
        BN_print_var(r);
        return 0;
    }
    return 1;
}

/*
 * test_mod_exp_zero tests that x**0 mod 1 == 0. It returns zero on success.
 */
static int test_mod_exp_zero(void)
{
    BIGNUM *a = NULL, *p = NULL, *m = NULL;
    BIGNUM *r = NULL;
    BN_ULONG one_word = 1;
    BN_CTX *ctx = BN_CTX_new();
    int ret = 0, failed = 0;
    BN_MONT_CTX *mont = NULL;

    if (!TEST_ptr(m = BN_new())
        || !TEST_ptr(a = BN_new())
        || !TEST_ptr(p = BN_new())
        || !TEST_ptr(r = BN_new()))
        goto err;

    BN_one(m);
    BN_one(a);
    BN_zero(p);

    if (!TEST_true(BN_rand(a, 1024, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY)))
        goto err;

    if (!TEST_true(BN_mod_exp(r, a, p, m, ctx)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp", r, a)))
        failed = 1;

    if (!TEST_true(BN_mod_exp_recp(r, a, p, m, ctx)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_recp", r, a)))
        failed = 1;

    if (!TEST_true(BN_mod_exp_simple(r, a, p, m, ctx)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_simple", r, a)))
        failed = 1;

    if (!TEST_true(BN_mod_exp_mont(r, a, p, m, ctx, NULL)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_mont", r, a)))
        failed = 1;

    if (!TEST_true(BN_mod_exp_mont_consttime(r, a, p, m, ctx, NULL)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_mont_consttime", r, a)))
        failed = 1;

    if (!TEST_ptr(mont = BN_MONT_CTX_new()))
        goto err;

    ERR_set_mark();
    /* mont is not set but passed in */
    if (!TEST_false(BN_mod_exp_mont_consttime(r, p, a, m, ctx, mont)))
        goto err;
    if (!TEST_false(BN_mod_exp_mont(r, p, a, m, ctx, mont)))
        goto err;
    ERR_pop_to_mark();

    if (!TEST_true(BN_MONT_CTX_set(mont, m, ctx)))
        goto err;

    /* we compute 0 ** a mod 1 here, to execute code that uses mont */
    if (!TEST_true(BN_mod_exp_mont_consttime(r, p, a, m, ctx, mont)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_mont_consttime", r, a)))
        failed = 1;

    if (!TEST_true(BN_mod_exp_mont(r, p, a, m, ctx, mont)))
        goto err;

    if (!TEST_true(a_is_zero_mod_one("BN_mod_exp_mont", r, a)))
        failed = 1;

    /*
     * A different codepath exists for single word multiplication
     * in non-constant-time only.
     */
    if (!TEST_true(BN_mod_exp_mont_word(r, one_word, p, m, ctx, NULL)))
        goto err;

    if (!TEST_BN_eq_zero(r)) {
        TEST_error("BN_mod_exp_mont_word failed: "
                   "1 ** 0 mod 1 = r (should be 0)");
        BN_print_var(r);
        goto err;
    }

    ret = !failed;
 err:
    BN_free(r);
    BN_free(a);
    BN_free(p);
    BN_free(m);
    BN_MONT_CTX_free(mont);
    BN_CTX_free(ctx);

    return ret;
}

static int test_mod_exp(int round)
{
    BN_CTX *ctx;
    unsigned char c;
    int ret = 0;
    BIGNUM *r_mont = NULL;
    BIGNUM *r_mont_const = NULL;
    BIGNUM *r_recp = NULL;
    BIGNUM *r_simple = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *m = NULL;

    if (!TEST_ptr(ctx = BN_CTX_new()))
        goto err;

    if (!TEST_ptr(r_mont = BN_new())
        || !TEST_ptr(r_mont_const = BN_new())
        || !TEST_ptr(r_recp = BN_new())
        || !TEST_ptr(r_simple = BN_new())
        || !TEST_ptr(a = BN_new())
        || !TEST_ptr(b = BN_new())
        || !TEST_ptr(m = BN_new()))
        goto err;

    if (!TEST_int_gt(RAND_bytes(&c, 1), 0))
        goto err;
    c = (c % BN_BITS) - BN_BITS2;
    if (!TEST_true(BN_rand(a, NUM_BITS + c, BN_RAND_TOP_ONE,
                           BN_RAND_BOTTOM_ANY)))
        goto err;

    if (!TEST_int_gt(RAND_bytes(&c, 1), 0))
        goto err;
    c = (c % BN_BITS) - BN_BITS2;
    if (!TEST_true(BN_rand(b, NUM_BITS + c, BN_RAND_TOP_ONE,
                           BN_RAND_BOTTOM_ANY)))
        goto err;

    if (!TEST_int_gt(RAND_bytes(&c, 1), 0))
        goto err;
    c = (c % BN_BITS) - BN_BITS2;
    if (!TEST_true(BN_rand(m, NUM_BITS + c, BN_RAND_TOP_ONE,
                           BN_RAND_BOTTOM_ODD)))
        goto err;

    if (!TEST_true(BN_mod(a, a, m, ctx))
        || !TEST_true(BN_mod(b, b, m, ctx))
        || !TEST_true(BN_mod_exp_mont(r_mont, a, b, m, ctx, NULL))
        || !TEST_true(BN_mod_exp_recp(r_recp, a, b, m, ctx))
        || !TEST_true(BN_mod_exp_simple(r_simple, a, b, m, ctx))
        || !TEST_true(BN_mod_exp_mont_consttime(r_mont_const, a, b, m, ctx, NULL)))
        goto err;

    if (!TEST_BN_eq(r_simple, r_mont)
        || !TEST_BN_eq(r_simple, r_recp)
        || !TEST_BN_eq(r_simple, r_mont_const)) {
        if (BN_cmp(r_simple, r_mont) != 0)
            TEST_info("simple and mont results differ");
        if (BN_cmp(r_simple, r_mont_const) != 0)
            TEST_info("simple and mont const time results differ");
        if (BN_cmp(r_simple, r_recp) != 0)
            TEST_info("simple and recp results differ");

        BN_print_var(a);
        BN_print_var(b);
        BN_print_var(m);
        BN_print_var(r_simple);
        BN_print_var(r_recp);
        BN_print_var(r_mont);
        BN_print_var(r_mont_const);
        goto err;
    }

    ret = 1;
 err:
    BN_free(r_mont);
    BN_free(r_mont_const);
    BN_free(r_recp);
    BN_free(r_simple);
    BN_free(a);
    BN_free(b);
    BN_free(m);
    BN_CTX_free(ctx);

    return ret;
}

static int test_mod_exp_x2(int idx)
{
    BN_CTX *ctx;
    int ret = 0;
    BIGNUM *r_mont_const_x2_1 = NULL;
    BIGNUM *r_mont_const_x2_2 = NULL;
    BIGNUM *r_simple1 = NULL;
    BIGNUM *r_simple2 = NULL;
    BIGNUM *a1 = NULL;
    BIGNUM *b1 = NULL;
    BIGNUM *m1 = NULL;
    BIGNUM *a2 = NULL;
    BIGNUM *b2 = NULL;
    BIGNUM *m2 = NULL;
    int factor_size = 0;

    if (idx <= 100)
        factor_size = 1024;
    else if (idx <= 200)
        factor_size = 1536;
    else if (idx <= 300)
        factor_size = 2048;

    if (!TEST_ptr(ctx = BN_CTX_new()))
        goto err;

    if (!TEST_ptr(r_mont_const_x2_1 = BN_new())
        || !TEST_ptr(r_mont_const_x2_2 = BN_new())
        || !TEST_ptr(r_simple1 = BN_new())
        || !TEST_ptr(r_simple2 = BN_new())
        || !TEST_ptr(a1 = BN_new())
        || !TEST_ptr(b1 = BN_new())
        || !TEST_ptr(m1 = BN_new())
        || !TEST_ptr(a2 = BN_new())
        || !TEST_ptr(b2 = BN_new())
        || !TEST_ptr(m2 = BN_new()))
        goto err;

    BN_rand(a1, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY);
    BN_rand(b1, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY);
    BN_rand(m1, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ODD);
    BN_rand(a2, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY);
    BN_rand(b2, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY);
    BN_rand(m2, factor_size, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ODD);

    if (!TEST_true(BN_mod(a1, a1, m1, ctx))
        || !TEST_true(BN_mod(b1, b1, m1, ctx))
        || !TEST_true(BN_mod(a2, a2, m2, ctx))
        || !TEST_true(BN_mod(b2, b2, m2, ctx))
        || !TEST_true(BN_mod_exp_simple(r_simple1, a1, b1, m1, ctx))
        || !TEST_true(BN_mod_exp_simple(r_simple2, a2, b2, m2, ctx))
        || !TEST_true(BN_mod_exp_mont_consttime_x2(r_mont_const_x2_1, a1, b1, m1, NULL,
                                                   r_mont_const_x2_2, a2, b2, m2, NULL,
                                                   ctx)))
        goto err;

    if (!TEST_BN_eq(r_simple1, r_mont_const_x2_1)
        || !TEST_BN_eq(r_simple2, r_mont_const_x2_2)) {
        if (BN_cmp(r_simple1, r_mont_const_x2_1) != 0)
            TEST_info("simple and mont const time x2 (#1) results differ");
        if (BN_cmp(r_simple2, r_mont_const_x2_2) != 0)
            TEST_info("simple and mont const time x2 (#2) results differ");

        BN_print_var(a1);
        BN_print_var(b1);
        BN_print_var(m1);
        BN_print_var(a2);
        BN_print_var(b2);
        BN_print_var(m2);
        BN_print_var(r_simple1);
        BN_print_var(r_simple2);
        BN_print_var(r_mont_const_x2_1);
        BN_print_var(r_mont_const_x2_2);
        goto err;
    }

    ret = 1;
 err:
    BN_free(r_mont_const_x2_1);
    BN_free(r_mont_const_x2_2);
    BN_free(r_simple1);
    BN_free(r_simple2);
    BN_free(a1);
    BN_free(b1);
    BN_free(m1);
    BN_free(a2);
    BN_free(b2);
    BN_free(m2);
    BN_CTX_free(ctx);

    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_mod_exp_zero);
    ADD_ALL_TESTS(test_mod_exp, 200);
    ADD_ALL_TESTS(test_mod_exp_x2, 300);
    return 1;
}
