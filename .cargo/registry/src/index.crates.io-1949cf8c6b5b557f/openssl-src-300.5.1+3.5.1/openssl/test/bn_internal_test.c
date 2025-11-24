/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>

#include <openssl/bn.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include "internal/nelem.h"
#include "internal/numbers.h"
#include "testutil.h"
#include "bn_prime.h"
#include "crypto/bn.h"

static BN_CTX *ctx;

static int test_is_prime_enhanced(void)
{
    int ret;
    int status = 0;
    BIGNUM *bn = NULL;

    ret = TEST_ptr(bn = BN_new())
          /* test passing a prime returns the correct status */
          && TEST_true(BN_set_word(bn, 11))
          /* return extra parameters related to composite */
          && TEST_true(ossl_bn_miller_rabin_is_prime(bn, 10, ctx, NULL, 1,
                                                     &status))
          && TEST_int_eq(status, BN_PRIMETEST_PROBABLY_PRIME);
    BN_free(bn);
    return ret;
}

static int composites[] = {
    9, 21, 77, 81, 265
};

static int test_is_composite_enhanced(int id)
{
    int ret;
    int status = 0;
    BIGNUM *bn = NULL;

    ret = TEST_ptr(bn = BN_new())
          /* negative tests for different composite numbers */
          && TEST_true(BN_set_word(bn, composites[id]))
          && TEST_true(ossl_bn_miller_rabin_is_prime(bn, 10, ctx, NULL, 1,
                                                     &status))
          && TEST_int_ne(status, BN_PRIMETEST_PROBABLY_PRIME);

    BN_free(bn);
    return ret;
}

/* Test that multiplying all the small primes from 3 to 751 equals a constant.
 * This test is mainly used to test that both 32 and 64 bit are correct.
 */
static int test_bn_small_factors(void)
{
    int ret = 0, i;
    BIGNUM *b = NULL;

    if (!(TEST_ptr(b = BN_new()) && TEST_true(BN_set_word(b, 3))))
        goto err;

    for (i = 1; i < NUMPRIMES; i++) {
        prime_t p = primes[i];
        if (p > 3 && p <= 751 && !BN_mul_word(b, p))
            goto err;
        if (p > 751)
            break;
    }
    ret = TEST_BN_eq(ossl_bn_get0_small_factors(), b);
err:
    BN_free(b);
    return ret;
}

int setup_tests(void)
{
    if (!TEST_ptr(ctx = BN_CTX_new()))
        return 0;

    ADD_TEST(test_is_prime_enhanced);
    ADD_ALL_TESTS(test_is_composite_enhanced, (int)OSSL_NELEM(composites));
    ADD_TEST(test_bn_small_factors);

    return 1;
}

void cleanup_tests(void)
{
    BN_CTX_free(ctx);
}

