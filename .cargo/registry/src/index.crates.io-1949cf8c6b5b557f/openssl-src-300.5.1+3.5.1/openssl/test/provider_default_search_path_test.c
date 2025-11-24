/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <openssl/provider.h>
#include "testutil.h"

static int test_default_libctx(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    char *path = "./some/path";
    const char *retrieved_path = NULL;
    int ok;

    ok = TEST_true(OSSL_PROVIDER_set_default_search_path(ctx, path))
        && TEST_ptr(retrieved_path = OSSL_PROVIDER_get0_default_search_path(ctx))
        && TEST_str_eq(path, retrieved_path);

    return ok;
}

static int test_explicit_libctx(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    char *def_libctx_path = "./some/path";
    char *path = "./another/location";
    const char *retrieved_defctx_path = NULL;
    const char *retrieved_path = NULL;
    int ok;

         /* Set search path for default context, then create a new context and set
            another path for it. Finally, get both paths and make sure they are
            still what we set and are separate. */
    ok = TEST_true(OSSL_PROVIDER_set_default_search_path(NULL, def_libctx_path))
        && TEST_ptr(ctx = OSSL_LIB_CTX_new())
        && TEST_true(OSSL_PROVIDER_set_default_search_path(ctx, path))
        && TEST_ptr(retrieved_defctx_path = OSSL_PROVIDER_get0_default_search_path(NULL))
        && TEST_str_eq(def_libctx_path, retrieved_defctx_path)
        && TEST_ptr(retrieved_path = OSSL_PROVIDER_get0_default_search_path(ctx))
        && TEST_str_eq(path, retrieved_path)
        && TEST_str_ne(retrieved_path, retrieved_defctx_path);

    OSSL_LIB_CTX_free(ctx);
    return ok;
}

int setup_tests(void)
{
    ADD_TEST(test_default_libctx);
    ADD_TEST(test_explicit_libctx);
    return 1;
}

