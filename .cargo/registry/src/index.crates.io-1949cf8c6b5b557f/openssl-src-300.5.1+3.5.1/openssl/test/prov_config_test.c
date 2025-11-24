/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <sys/stat.h>
#include <openssl/evp.h>
#include <openssl/conf.h>
#include "testutil.h"

static char *configfile = NULL;
static char *recurseconfigfile = NULL;
static char *pathedconfig = NULL;

/*
 * Test to make sure there are no leaks or failures from loading the config
 * file twice.
 */
static int test_double_config(void)
{
    OSSL_LIB_CTX *ctx = OSSL_LIB_CTX_new();
    int testresult = 0;
    EVP_MD *sha256 = NULL;

    if (!TEST_ptr(ctx))
        return 0;

    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, configfile)))
        goto err;
    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, configfile)))
        goto err;

    /* Check we can actually fetch something */
    sha256 = EVP_MD_fetch(ctx, "SHA2-256", NULL);
    if (!TEST_ptr(sha256))
        goto err;

    testresult = 1;
 err:
    EVP_MD_free(sha256);
    OSSL_LIB_CTX_free(ctx);
    return testresult;
}

static int test_recursive_config(void)
{
    OSSL_LIB_CTX *ctx = OSSL_LIB_CTX_new();
    int testresult = 0;
    unsigned long err;

    if (!TEST_ptr(ctx))
        goto err;

    if (!TEST_false(OSSL_LIB_CTX_load_config(ctx, recurseconfigfile)))
        goto err;

    err = ERR_peek_error();
    /* We expect to get a recursion error here */
    if (ERR_GET_REASON(err) == CONF_R_RECURSIVE_SECTION_REFERENCE)
        testresult = 1;
 err:
    OSSL_LIB_CTX_free(ctx);
    return testresult;
}

#define P_TEST_PATH "/../test/p_test.so"
static int test_path_config(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    OSSL_PROVIDER *prov;
    int testresult = 0;
    struct stat sbuf;
    char *module_path = getenv("OPENSSL_MODULES");
    char *full_path = NULL;
    int rc;

    if (!TEST_ptr(module_path))
        return 0;

    full_path = OPENSSL_zalloc(strlen(module_path) + strlen(P_TEST_PATH) + 1);
    if (!TEST_ptr(full_path))
        return 0;

    strcpy(full_path, module_path);
    full_path = strcat(full_path, P_TEST_PATH);
    TEST_info("full path is %s", full_path);
    rc = stat(full_path, &sbuf);
    OPENSSL_free(full_path);
    if (rc == -1)
        return TEST_skip("Skipping modulepath test as provider not present");

    if (!TEST_ptr(pathedconfig))
        return 0;

    ctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(ctx))
        return 0;

    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, pathedconfig)))
        goto err;

    /* attempt to manually load the test provider */
    if (!TEST_ptr(prov = OSSL_PROVIDER_load(ctx, "test")))
        goto err;

    OSSL_PROVIDER_unload(prov);

    testresult = 1;
 err:
    OSSL_LIB_CTX_free(ctx);
    return testresult;
}

OPT_TEST_DECLARE_USAGE("configfile\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(configfile = test_get_argument(0)))
        return 0;

    if (!TEST_ptr(recurseconfigfile = test_get_argument(1)))
        return 0;

    if (!TEST_ptr(pathedconfig = test_get_argument(2)))
        return 0;

    ADD_TEST(test_recursive_config);
    ADD_TEST(test_double_config);
    ADD_TEST(test_path_config);
    return 1;
}
