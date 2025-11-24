/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/opensslconf.h>

#include <string.h>
#include <openssl/evp.h>
#include <openssl/ssl.h>
#include <openssl/tls1.h>
#include "testutil.h"

static int expect_failure = 0;

static int test_func(void)
{
    int ret = 0;
    SSL_CTX *ctx;

    ctx = SSL_CTX_new(TLS_method());
    if (expect_failure) {
        if (!TEST_ptr_null(ctx))
            goto err;
    } else {
        if (!TEST_ptr(ctx))
            return 0;
        if (!TEST_int_eq(SSL_CTX_get_min_proto_version(ctx), TLS1_2_VERSION)
            && !TEST_int_eq(SSL_CTX_get_max_proto_version(ctx), TLS1_2_VERSION)) {
            TEST_info("min/max version setting incorrect");
            goto err;
        }
    }
    ret = 1;
 err:
    SSL_CTX_free(ctx);
    return ret;
}

int global_init(void)
{
    if (!OPENSSL_init_ssl(OPENSSL_INIT_ENGINE_ALL_BUILTIN
                          | OPENSSL_INIT_LOAD_CONFIG, NULL))
        return 0;
    return 1;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_FAIL,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "f", OPT_FAIL, '-', "A failure is expected" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_FAIL:
            expect_failure = 1;
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    ADD_TEST(test_func);
    return 1;
}
