/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <string.h>
#include <openssl/provider.h>
#include <openssl/params.h>
#include <openssl/core_names.h>
#include <openssl/self_test.h>
#include <openssl/evp.h>
#include <openssl/rsa.h>
#include "testutil.h"

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_PROVIDER_NAME,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

struct self_test_arg {
    int count;
};

static OSSL_LIB_CTX *libctx = NULL;
static char *provider_name = NULL;
static struct self_test_arg self_test_args = { 0 };

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "provider_name", OPT_PROVIDER_NAME, 's',
          "The name of the provider to load" },
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return test_options;
}

static int self_test_events(const OSSL_PARAM params[], void *arg,
                            const char *title, int corrupt)
{
    struct self_test_arg *args = arg;
    const OSSL_PARAM *p = NULL;
    const char *phase = NULL, *type = NULL, *desc = NULL;
    int ret = 0;

    if (args->count == 0)
        BIO_printf(bio_out, "\n%s\n", title);
    args->count++;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_PHASE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    phase = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_DESC);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    desc = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_TYPE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    type = (const char *)p->data;

    if (strcmp(phase, OSSL_SELF_TEST_PHASE_START) == 0)
        BIO_printf(bio_out, "%s : (%s) : ", desc, type);
    else if (strcmp(phase, OSSL_SELF_TEST_PHASE_PASS) == 0
             || strcmp(phase, OSSL_SELF_TEST_PHASE_FAIL) == 0)
        BIO_printf(bio_out, "%s\n", phase);
    /*
     * The self test code will internally corrupt the KAT test result if an
     * error is returned during the corrupt phase.
     */
    if (corrupt && strcmp(phase, OSSL_SELF_TEST_PHASE_CORRUPT) == 0)
        goto err;
    ret = 1;
err:
    return ret;
}

static int self_test_on_demand_fail(const OSSL_PARAM params[], void *arg)
{
    return self_test_events(params, arg, "On Demand Failure", 1);
}

static int self_test_on_demand(const OSSL_PARAM params[], void *arg)
{
    return self_test_events(params, arg, "On Demand", 0);
}

static int self_test_on_load(const OSSL_PARAM params[], void *arg)
{
    return self_test_events(params, arg, "On Loading", 0);
}

static int get_provider_params(const OSSL_PROVIDER *prov)
{
    int ret = 0;
    OSSL_PARAM params[5];
    char *name, *version, *buildinfo;
    int status;
    const OSSL_PARAM *gettable, *p;

    if (!TEST_ptr(gettable = OSSL_PROVIDER_gettable_params(prov))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(gettable, OSSL_PROV_PARAM_NAME))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(gettable, OSSL_PROV_PARAM_VERSION))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(gettable, OSSL_PROV_PARAM_STATUS))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(gettable, OSSL_PROV_PARAM_BUILDINFO)))
        goto end;

    params[0] = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_NAME, &name, 0);
    params[1] = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_VERSION,
                                              &version, 0);
    params[2] = OSSL_PARAM_construct_int(OSSL_PROV_PARAM_STATUS, &status);
    params[3] = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_BUILDINFO,
                                              &buildinfo, 0);
    params[4] = OSSL_PARAM_construct_end();
    OSSL_PARAM_set_all_unmodified(params);
    if (!TEST_true(OSSL_PROVIDER_get_params(prov, params)))
        goto end;
    if (!TEST_true(OSSL_PARAM_modified(params + 0))
        || !TEST_true(OSSL_PARAM_modified(params + 1))
        || !TEST_true(OSSL_PARAM_modified(params + 2))
        || !TEST_true(OSSL_PARAM_modified(params + 3))
        || !TEST_true(status == 1))
        goto end;

    ret = 1;
end:
    return ret;
}

static int test_provider_status(void)
{
    int ret = 0;
    unsigned int status = 0;
    OSSL_PROVIDER *prov = NULL;
    OSSL_PARAM params[2];
    EVP_MD *fetch = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *pkey = NULL;

    if (!TEST_ptr(prov = OSSL_PROVIDER_load(libctx, provider_name)))
        goto err;
    if (!get_provider_params(prov))
        goto err;

    /* Test that the provider status is ok */
    params[0] = OSSL_PARAM_construct_uint(OSSL_PROV_PARAM_STATUS, &status);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_true(OSSL_PROVIDER_get_params(prov, params))
        || !TEST_true(status == 1))
        goto err;
    if (!TEST_ptr(fetch = EVP_MD_fetch(libctx, "SHA256", NULL)))
        goto err;
    EVP_MD_free(fetch);
    fetch = NULL;
    /* Use RNG before triggering on-demand self tests */
    if (!TEST_ptr((pctx = EVP_PKEY_CTX_new_from_name(libctx, "RSA", NULL)))
        || !TEST_int_gt(EVP_PKEY_keygen_init(pctx), 0)
        || !TEST_int_gt(EVP_PKEY_CTX_set_rsa_keygen_bits(pctx, 2048), 0)
        || !TEST_int_gt(EVP_PKEY_keygen(pctx, &pkey), 0))
        goto err;
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(pctx);
    pkey = NULL;
    pctx = NULL;

    /* Test that the provider self test is ok */
    self_test_args.count = 0;
    OSSL_SELF_TEST_set_callback(libctx, self_test_on_demand, &self_test_args);
    if (!TEST_true(OSSL_PROVIDER_self_test(prov)))
        goto err;

    /* Setup a callback that corrupts the self tests and causes status failures */
    self_test_args.count = 0;
    OSSL_SELF_TEST_set_callback(libctx, self_test_on_demand_fail, &self_test_args);
    if (!TEST_false(OSSL_PROVIDER_self_test(prov)))
        goto err;
    if (!TEST_true(OSSL_PROVIDER_get_params(prov, params))
        || !TEST_uint_eq(status, 0))
        goto err;
    if (!TEST_ptr_null(fetch = EVP_MD_fetch(libctx, "SHA256", NULL)))
        goto err;

    ret = 1;
err:
    EVP_MD_free(fetch);
    OSSL_PROVIDER_unload(prov);
    return ret;
}

static int test_provider_gettable_params(void)
{
    OSSL_PROVIDER *prov;
    int ret;

    if (!TEST_ptr(prov = OSSL_PROVIDER_load(libctx, provider_name)))
        return 0;
    ret = get_provider_params(prov);
    OSSL_PROVIDER_unload(prov);
    return ret;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    char *config_file = NULL;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_PROVIDER_NAME:
            provider_name = opt_arg();
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    libctx = OSSL_LIB_CTX_new();
    if (libctx == NULL)
        return 0;

    if (strcmp(provider_name, "fips") == 0) {
        self_test_args.count = 0;
        OSSL_SELF_TEST_set_callback(libctx, self_test_on_load, &self_test_args);
        if (!OSSL_LIB_CTX_load_config(libctx, config_file)) {
            opt_printf_stderr("Failed to load config\n");
            return 0;
        }
        ADD_TEST(test_provider_status);
    } else {
        ADD_TEST(test_provider_gettable_params);
    }
    return 1;
}

void cleanup_tests(void)
{
    OSSL_LIB_CTX_free(libctx);
}
