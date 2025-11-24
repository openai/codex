/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <openssl/provider.h>
#include <openssl/param_build.h>
#include "testutil.h"

extern OSSL_provider_init_fn PROVIDER_INIT_FUNCTION_NAME;

static char buf[256];
static OSSL_PARAM greeting_request[] = {
    { "greeting", OSSL_PARAM_UTF8_STRING, buf, sizeof(buf) },
    { NULL, 0, NULL, 0, 0 }
};

static unsigned int digestsuccess = 0;
static OSSL_PARAM digest_check[] = {
    { "digest-check", OSSL_PARAM_UNSIGNED_INTEGER, &digestsuccess,
      sizeof(digestsuccess) },
    { NULL, 0, NULL, 0, 0 }
};

static unsigned int stopsuccess = 0;
static OSSL_PARAM stop_property_mirror[] = {
    { "stop-property-mirror", OSSL_PARAM_UNSIGNED_INTEGER, &stopsuccess,
      sizeof(stopsuccess) },
    { NULL, 0, NULL, 0, 0 }
};

static int test_provider(OSSL_LIB_CTX **libctx, const char *name,
                         OSSL_PROVIDER *legacy)
{
    OSSL_PROVIDER *prov = NULL;
    const char *greeting = NULL;
    char expected_greeting[256];
    int ok = 0;
    long err;
    int dolegacycheck = (legacy != NULL);
    OSSL_PROVIDER *deflt = NULL, *base = NULL;

    BIO_snprintf(expected_greeting, sizeof(expected_greeting),
                 "Hello OpenSSL %.20s, greetings from %s!",
                 OPENSSL_VERSION_STR, name);


    /*
     * We set properties that we know the providers we are using don't have.
     * This should mean that the p_test provider will fail any fetches - which
     * is something we test inside the provider.
     */
    EVP_set_default_properties(*libctx, "fips=yes");
    /*
     * Check that it is possible to have a built-in provider mirrored in
     * a child lib ctx.
     */
    if (!TEST_ptr(base = OSSL_PROVIDER_load(*libctx, "base")))
        goto err;
    if (!TEST_ptr(prov = OSSL_PROVIDER_load(*libctx, name)))
        goto err;

    /*
     * Once the provider is loaded we clear the default properties and fetches
     * should start working again.
     */
    EVP_set_default_properties(*libctx, "");
    if (dolegacycheck) {
        if (!TEST_true(OSSL_PROVIDER_get_params(prov, digest_check))
                || !TEST_true(digestsuccess))
            goto err;

        /*
         * Check that a provider can prevent property mirroring if it sets its
         * own properties explicitly
         */
        if (!TEST_true(OSSL_PROVIDER_get_params(prov, stop_property_mirror))
                || !TEST_true(stopsuccess))
            goto err;
        EVP_set_default_properties(*libctx, "fips=yes");
        if (!TEST_true(OSSL_PROVIDER_get_params(prov, digest_check))
                || !TEST_true(digestsuccess))
            goto err;
        EVP_set_default_properties(*libctx, "");
    }
    if (!TEST_true(OSSL_PROVIDER_get_params(prov, greeting_request))
            || !TEST_ptr(greeting = greeting_request[0].data)
            || !TEST_size_t_gt(greeting_request[0].data_size, 0)
            || !TEST_str_eq(greeting, expected_greeting))
        goto err;

    /* Make sure we got the error we were expecting */
    err = ERR_peek_last_error();
    if (!TEST_int_gt(err, 0)
            || !TEST_int_eq(ERR_GET_REASON(err), 1))
        goto err;

    OSSL_PROVIDER_unload(legacy);
    legacy = NULL;

    if (dolegacycheck) {
        /* Legacy provider should also be unloaded from child libctx */
        if (!TEST_true(OSSL_PROVIDER_get_params(prov, digest_check))
                || !TEST_false(digestsuccess))
            goto err;
        /*
         * Loading the legacy provider again should make it available again in
         * the child libctx. Loading and unloading the default provider should
         * have no impact on the child because the child loads it explicitly
         * before this point.
         */
        legacy = OSSL_PROVIDER_load(*libctx, "legacy");
        deflt = OSSL_PROVIDER_load(*libctx, "default");
        if (!TEST_ptr(deflt)
                || !TEST_true(OSSL_PROVIDER_available(*libctx, "default")))
            goto err;
        OSSL_PROVIDER_unload(deflt);
        deflt = NULL;
        if (!TEST_ptr(legacy)
                || !TEST_false(OSSL_PROVIDER_available(*libctx, "default"))
                || !TEST_true(OSSL_PROVIDER_get_params(prov, digest_check))
                || !TEST_true(digestsuccess))
        goto err;
        OSSL_PROVIDER_unload(legacy);
        legacy = NULL;
    }

    if (!TEST_true(OSSL_PROVIDER_unload(base)))
        goto err;
    base = NULL;
    if (!TEST_true(OSSL_PROVIDER_unload(prov)))
        goto err;
    prov = NULL;

    /*
     * We must free the libctx to force the provider to really be unloaded from
     * memory
     */
    OSSL_LIB_CTX_free(*libctx);
    *libctx = NULL;

    /* We print out all the data to make sure it can still be accessed */
    ERR_print_errors_fp(stderr);
    ok = 1;
 err:
    OSSL_PROVIDER_unload(base);
    OSSL_PROVIDER_unload(deflt);
    OSSL_PROVIDER_unload(legacy);
    legacy = NULL;
    OSSL_PROVIDER_unload(prov);
    OSSL_LIB_CTX_free(*libctx);
    *libctx = NULL;
    return ok;
}

#ifndef NO_PROVIDER_MODULE
static int test_provider_ex(OSSL_LIB_CTX **libctx, const char *name)
{
    OSSL_PROVIDER *prov = NULL;
    const char *greeting = NULL;
    int ok = 0;
    long err;
    const char custom_buf[] = "Custom greeting";
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_true(OSSL_PARAM_BLD_push_utf8_string(bld, "greeting", custom_buf,
                                                      strlen(custom_buf)))
        || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld))) {
        goto err;
    }

    if (!TEST_ptr(prov = OSSL_PROVIDER_load_ex(*libctx, name, params)))
        goto err;

    if (!TEST_true(OSSL_PROVIDER_get_params(prov, greeting_request))
            || !TEST_ptr(greeting = greeting_request[0].data)
            || !TEST_size_t_gt(greeting_request[0].data_size, 0)
            || !TEST_str_eq(greeting, custom_buf))
        goto err;

    /* Make sure we got the error we were expecting */
    err = ERR_peek_last_error();
    if (!TEST_int_gt(err, 0)
            || !TEST_int_eq(ERR_GET_REASON(err), 1))
        goto err;

    if (!TEST_true(OSSL_PROVIDER_unload(prov)))
        goto err;
    prov = NULL;

    /*
     * We must free the libctx to force the provider to really be unloaded from
     * memory
     */
    OSSL_LIB_CTX_free(*libctx);
    *libctx = NULL;

    /* We print out all the data to make sure it can still be accessed */
    ERR_print_errors_fp(stderr);
    ok = 1;
 err:
    OSSL_PARAM_BLD_free(bld);
    OSSL_PARAM_free(params);
    OSSL_PROVIDER_unload(prov);
    OSSL_LIB_CTX_free(*libctx);
    *libctx = NULL;
    return ok;
}
#endif

static int test_builtin_provider(void)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    const char *name = "p_test_builtin";
    int ok;

    ok =
        TEST_ptr(libctx)
        && TEST_true(OSSL_PROVIDER_add_builtin(libctx, name,
                                               PROVIDER_INIT_FUNCTION_NAME))
        && test_provider(&libctx, name, NULL);

    OSSL_LIB_CTX_free(libctx);

    return ok;
}

/* Test relies on fetching the MD4 digest from the legacy provider */
#ifndef OPENSSL_NO_MD4
static int test_builtin_provider_with_child(void)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    const char *name = "p_test";
    OSSL_PROVIDER *legacy;

    if (!TEST_ptr(libctx))
        return 0;

    legacy = OSSL_PROVIDER_load(libctx, "legacy");
    if (legacy == NULL) {
        /*
         * In this case we assume we've been built with "no-legacy" and skip
         * this test (there is no OPENSSL_NO_LEGACY)
         */
        OSSL_LIB_CTX_free(libctx);
        return 1;
    }

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, name,
                                             PROVIDER_INIT_FUNCTION_NAME))) {
        OSSL_PROVIDER_unload(legacy);
        OSSL_LIB_CTX_free(libctx);
        return 0;
    }

    /* test_provider will free libctx and unload legacy as part of the test */
    return test_provider(&libctx, name, legacy);
}
#endif

#ifndef NO_PROVIDER_MODULE
static int test_loaded_provider(void)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    const char *name = "p_test";
    int res = 0;

    if (!TEST_ptr(libctx))
        return 0;

    /* test_provider will free libctx as part of the test */
    res = test_provider(&libctx, name, NULL);

    libctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(libctx))
        return 0;

    /* test_provider_ex will free libctx as part of the test */
    res = res && test_provider_ex(&libctx, name);

    return res;
}
#endif

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_LOADED,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "loaded", OPT_LOADED, '-', "Run test with a loaded provider" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    int loaded = 0;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_TEST_CASES:
            break;
        case OPT_LOADED:
            loaded = 1;
            break;
        default:
            return 0;
        }
    }

    if (!loaded) {
        ADD_TEST(test_builtin_provider);
#ifndef OPENSSL_NO_MD4
        ADD_TEST(test_builtin_provider_with_child);
#endif
    }
#ifndef NO_PROVIDER_MODULE
    else {
        ADD_TEST(test_loaded_provider);
    }
#endif
    return 1;
}

