/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include "testutil.h"

#define MYPROPERTIES "foo.bar=yes"

static OSSL_FUNC_provider_query_operation_fn testprov_query;
static OSSL_FUNC_digest_get_params_fn tmpmd_get_params;
static OSSL_FUNC_digest_digest_fn tmpmd_digest;

static int tmpmd_get_params(OSSL_PARAM params[])
{
    OSSL_PARAM *p = NULL;

    p = OSSL_PARAM_locate(params, OSSL_DIGEST_PARAM_BLOCK_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 1))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_DIGEST_PARAM_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 1))
        return 0;

    return 1;
}

static int tmpmd_digest(void *provctx, const unsigned char *in, size_t inl,
                 unsigned char *out, size_t *outl, size_t outsz)
{
    return 0;
}

static const OSSL_DISPATCH testprovmd_functions[] = {
    { OSSL_FUNC_DIGEST_GET_PARAMS, (void (*)(void))tmpmd_get_params },
    { OSSL_FUNC_DIGEST_DIGEST, (void (*)(void))tmpmd_digest },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM testprov_digests[] = {
    { "testprovmd", MYPROPERTIES, testprovmd_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *testprov_query(void *provctx,
                                          int operation_id,
                                          int *no_cache)
{
    *no_cache = 0;
    return operation_id == OSSL_OP_DIGEST ? testprov_digests : NULL;
}

static const OSSL_DISPATCH testprov_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))testprov_query },
    OSSL_DISPATCH_END
};

static int testprov_provider_init(const OSSL_CORE_HANDLE *handle,
                                  const OSSL_DISPATCH *in,
                                  const OSSL_DISPATCH **out,
                                  void **provctx)
{
    *provctx = (void *)handle;
    *out = testprov_dispatch_table;
    return 1;
}

enum {
    DEFAULT_PROPS_FIRST = 0,
    DEFAULT_PROPS_AFTER_LOAD,
    DEFAULT_PROPS_AFTER_FETCH,
    DEFAULT_PROPS_FINAL
};

static int test_default_props_and_providers(int propsorder)
{
    OSSL_LIB_CTX *libctx;
    OSSL_PROVIDER *testprov = NULL;
    EVP_MD *testprovmd = NULL;
    int res = 0;

    if (!TEST_ptr(libctx = OSSL_LIB_CTX_new())
            || !TEST_true(OSSL_PROVIDER_add_builtin(libctx, "testprov",
                                                    testprov_provider_init)))
        goto err;

    if (propsorder == DEFAULT_PROPS_FIRST
            && !TEST_true(EVP_set_default_properties(libctx, MYPROPERTIES)))
        goto err;

    if (!TEST_ptr(testprov = OSSL_PROVIDER_load(libctx, "testprov")))
        goto err;

    if (propsorder == DEFAULT_PROPS_AFTER_LOAD
            && !TEST_true(EVP_set_default_properties(libctx, MYPROPERTIES)))
        goto err;

    if (!TEST_ptr(testprovmd = EVP_MD_fetch(libctx, "testprovmd", NULL)))
        goto err;

    if (propsorder == DEFAULT_PROPS_AFTER_FETCH) {
        if (!TEST_true(EVP_set_default_properties(libctx, MYPROPERTIES)))
            goto err;
        EVP_MD_free(testprovmd);
        if (!TEST_ptr(testprovmd = EVP_MD_fetch(libctx, "testprovmd", NULL)))
            goto err;
    }

    res = 1;
 err:
    EVP_MD_free(testprovmd);
    OSSL_PROVIDER_unload(testprov);
    OSSL_LIB_CTX_free(libctx);
    return res;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_default_props_and_providers, DEFAULT_PROPS_FINAL);
    return 1;
}
