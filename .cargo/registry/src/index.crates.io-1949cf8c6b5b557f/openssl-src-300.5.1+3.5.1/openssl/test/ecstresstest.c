/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include "internal/nelem.h"
#include "testutil.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#define NUM_REPEATS "1000000"

static ossl_intmax_t num_repeats;
static int print_mode = 0;

#ifndef OPENSSL_NO_EC
# include <openssl/ec.h>
# include <openssl/err.h>
# include <openssl/obj_mac.h>
# include <openssl/objects.h>
# include <openssl/rand.h>
# include <openssl/bn.h>
# include <openssl/opensslconf.h>

static const char *kP256DefaultResult =
    "A1E24B223B8E81BC1FFF99BAFB909EDB895FACDE7D6DA5EF5E7B3255FB378E0F";

/*
 * Perform a deterministic walk on the curve, by starting from |point| and
 * using the X-coordinate of the previous point as the next scalar for
 * point multiplication.
 * Returns the X-coordinate of the end result or NULL on error.
 */
static BIGNUM *walk_curve(const EC_GROUP *group, EC_POINT *point,
                          ossl_intmax_t num)
{
    BIGNUM *scalar = NULL;
    ossl_intmax_t i;

    if (!TEST_ptr(scalar = BN_new())
            || !TEST_true(EC_POINT_get_affine_coordinates(group, point, scalar,
                                                          NULL, NULL)))
        goto err;

    for (i = 0; i < num; i++) {
        if (!TEST_true(EC_POINT_mul(group, point, NULL, point, scalar, NULL))
                || !TEST_true(EC_POINT_get_affine_coordinates(group, point,
                                                              scalar,
                                                              NULL, NULL)))
            goto err;
    }
    return scalar;

err:
    BN_free(scalar);
    return NULL;
}

static int test_curve(void)
{
    EC_GROUP *group = NULL;
    EC_POINT *point = NULL;
    BIGNUM *result = NULL, *expected_result = NULL;
    int ret = 0;

    /*
     * We currently hard-code P-256, though adaptation to other curves.
     * would be straightforward.
     */
    if (!TEST_ptr(group = EC_GROUP_new_by_curve_name(NID_X9_62_prime256v1))
            || !TEST_ptr(point = EC_POINT_dup(EC_GROUP_get0_generator(group),
                                              group))
            || !TEST_ptr(result = walk_curve(group, point, num_repeats)))
        goto err;

    if (print_mode) {
        BN_print(bio_out, result);
        BIO_printf(bio_out, "\n");
        ret = 1;
    } else {
        if (!TEST_true(BN_hex2bn(&expected_result, kP256DefaultResult))
                || !TEST_ptr(expected_result)
                || !TEST_BN_eq(result, expected_result))
            goto err;
        ret = 1;
    }

err:
    EC_GROUP_free(group);
    EC_POINT_free(point);
    BN_free(result);
    BN_free(expected_result);
    return ret;
}
#endif

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_NUM_REPEATS,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "num", OPT_NUM_REPEATS, 'M', "Number of repeats" },
        { NULL }
    };
    return test_options;
}

/*
 * Stress test the curve. If the '-num' argument is given, runs the loop
 * |num| times and prints the resulting X-coordinate. Otherwise runs the test
 * the default number of times and compares against the expected result.
 */
int setup_tests(void)
{
    OPTION_CHOICE o;

    if (!opt_intmax(NUM_REPEATS, &num_repeats)) {
        TEST_error("Cannot parse " NUM_REPEATS);
        return 0;
    }

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_NUM_REPEATS:
            if (!opt_intmax(opt_arg(), &num_repeats)
                    || num_repeats < 0)
                return 0;
            print_mode = 1;
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

#ifndef OPENSSL_NO_EC
    ADD_TEST(test_curve);
#endif
    return 1;
}
