/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <openssl/crypto.h>
#include "internal/provider.h"
#include "testutil.h"

extern OSSL_provider_init_fn PROVIDER_INIT_FUNCTION_NAME;

static char buf[256];
static OSSL_PARAM greeting_request[] = {
    { "greeting", OSSL_PARAM_UTF8_STRING, buf, sizeof(buf), 0 },
    { NULL, 0, NULL, 0, 0 }
};

static int test_provider(OSSL_PROVIDER *prov, const char *expected_greeting)
{
    const char *greeting = "no greeting received";
    int ret = 0;

    ret =
        TEST_true(ossl_provider_activate(prov, 1, 0))
        && TEST_true(ossl_provider_get_params(prov, greeting_request))
        && TEST_ptr(greeting = greeting_request[0].data)
        && TEST_size_t_gt(greeting_request[0].data_size, 0)
        && TEST_str_eq(greeting, expected_greeting)
        && TEST_true(ossl_provider_deactivate(prov, 1));

    TEST_info("Got this greeting: %s\n", greeting);
    ossl_provider_free(prov);
    return ret;
}

static const char *expected_greeting1(const char *name)
{
    static char expected_greeting[256] = "";

    BIO_snprintf(expected_greeting, sizeof(expected_greeting),
                 "Hello OpenSSL %.20s, greetings from %s!",
                 OPENSSL_VERSION_STR, name);

    return expected_greeting;
}

static int test_builtin_provider(void)
{
    const char *name = "p_test_builtin";
    OSSL_PROVIDER *prov = NULL;
    int ret;

    /*
     * We set properties that we know the providers we are using don't have.
     * This should mean that the p_test provider will fail any fetches - which
     * is something we test inside the provider.
     */
    EVP_set_default_properties(NULL, "fips=yes");

    ret =
        TEST_ptr(prov =
                 ossl_provider_new(NULL, name, PROVIDER_INIT_FUNCTION_NAME, NULL, 0))
        && test_provider(prov, expected_greeting1(name));

    EVP_set_default_properties(NULL, "");

    return ret;
}

#ifndef NO_PROVIDER_MODULE
static int test_loaded_provider(void)
{
    const char *name = "p_test";
    OSSL_PROVIDER *prov = NULL;

    return
        TEST_ptr(prov = ossl_provider_new(NULL, name, NULL, NULL, 0))
        && test_provider(prov, expected_greeting1(name));
}

# ifndef OPENSSL_NO_AUTOLOAD_CONFIG
static int test_configured_provider(void)
{
    const char *name = "p_test_configured";
    OSSL_PROVIDER *prov = NULL;
    /* This MUST match the config file */
    const char *expected_greeting =
        "Hello OpenSSL, greetings from Test Provider";

    return
        TEST_ptr(prov = ossl_provider_find(NULL, name, 0))
        && test_provider(prov, expected_greeting);
}
# endif
#endif

static int test_cache_flushes(void)
{
    OSSL_LIB_CTX *ctx;
    OSSL_PROVIDER *prov = NULL;
    EVP_MD *md = NULL;
    int ret = 0;

    if (!TEST_ptr(ctx = OSSL_LIB_CTX_new())
            || !TEST_ptr(prov = OSSL_PROVIDER_load(ctx, "default"))
            || !TEST_true(OSSL_PROVIDER_available(ctx, "default"))
            || !TEST_ptr(md = EVP_MD_fetch(ctx, "SHA256", NULL)))
        goto err;
    EVP_MD_free(md);
    md = NULL;
    OSSL_PROVIDER_unload(prov);
    prov = NULL;

    if (!TEST_false(OSSL_PROVIDER_available(ctx, "default")))
        goto err;

    if (!TEST_ptr_null(md = EVP_MD_fetch(ctx, "SHA256", NULL))) {
        const char *provname = OSSL_PROVIDER_get0_name(EVP_MD_get0_provider(md));

        if (OSSL_PROVIDER_available(NULL, provname))
            TEST_info("%s provider is available\n", provname);
        else
            TEST_info("%s provider is not available\n", provname);
    }

    ret = 1;
 err:
    OSSL_PROVIDER_unload(prov);
    EVP_MD_free(md);
    OSSL_LIB_CTX_free(ctx);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_builtin_provider);
#ifndef NO_PROVIDER_MODULE
    ADD_TEST(test_loaded_provider);
# ifndef OPENSSL_NO_AUTOLOAD_CONFIG
    ADD_TEST(test_configured_provider);
# endif
#endif
    ADD_TEST(test_cache_flushes);
    return 1;
}

