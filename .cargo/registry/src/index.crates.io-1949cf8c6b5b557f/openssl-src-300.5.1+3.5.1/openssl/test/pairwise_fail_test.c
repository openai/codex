/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/pem.h>
#include <openssl/core_names.h>
#include <openssl/self_test.h>
#include "testutil.h"

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_PROVIDER_NAME,
    OPT_CONFIG_FILE,
    OPT_PAIRWISETEST,
    OPT_DSAPARAM,
    OPT_TEST_ENUM
} OPTION_CHOICE;

struct self_test_arg {
    const char *type;
};

static OSSL_LIB_CTX *libctx = NULL;
static char *pairwise_name = NULL;
static char *dsaparam_file = NULL;
static struct self_test_arg self_test_args = { 0 };

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { "pairwise", OPT_PAIRWISETEST, 's',
          "Test keygen pairwise test failures" },
        { "dsaparam", OPT_DSAPARAM, 's', "DSA param file" },
        { NULL }
    };
    return test_options;
}

static int self_test_on_pairwise_fail(const OSSL_PARAM params[], void *arg)
{
    struct self_test_arg *args = arg;
    const OSSL_PARAM *p = NULL;
    const char *type = NULL, *phase = NULL;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_PHASE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        return 0;
    phase = (const char *)p->data;
    if (strcmp(phase, OSSL_SELF_TEST_PHASE_CORRUPT) == 0) {
        p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_TYPE);
        if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        type = (const char *)p->data;
        if (strcmp(type, args->type) == 0)
            return 0;
    }
    return 1;
}

static int setup_selftest_pairwise_failure(const char *type)
{
    int ret = 0;
    OSSL_PROVIDER *prov = NULL;

    if (!TEST_ptr(prov = OSSL_PROVIDER_load(libctx, "fips")))
        goto err;

    /* Setup a callback that corrupts the pairwise self tests and causes failures */
    self_test_args.type = type;
    OSSL_SELF_TEST_set_callback(libctx, self_test_on_pairwise_fail, &self_test_args);

    ret = 1;
err:
    OSSL_PROVIDER_unload(prov);
    return ret;
}

static int test_keygen_pairwise_failure(void)
{
    BIO *bio = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pParams = NULL;
    EVP_PKEY *pkey = NULL;
    const char *type = OSSL_SELF_TEST_TYPE_PCT;
    int ret = 0;

    if (strcmp(pairwise_name, "rsa") == 0) {
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr_null(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "RSA", (size_t)2048)))
            goto err;
    } else if (strncmp(pairwise_name, "ec", 2) == 0) {
        if (strcmp(pairwise_name, "eckat") == 0)
            type = OSSL_SELF_TEST_TYPE_PCT_KAT;
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr_null(pkey = EVP_PKEY_Q_keygen(libctx, NULL, "EC", "P-256")))
            goto err;
    } else if (strncmp(pairwise_name, "dsa", 3) == 0) {
        if (strcmp(pairwise_name, "dsakat") == 0)
            type = OSSL_SELF_TEST_TYPE_PCT_KAT;
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr(bio = BIO_new_file(dsaparam_file, "r")))
            goto err;
        if (!TEST_ptr(pParams = PEM_read_bio_Parameters_ex(bio, NULL, libctx, NULL)))
            goto err;
        if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pParams, NULL)))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
            goto err;
        if (!TEST_int_le(EVP_PKEY_keygen(ctx, &pkey), 0))
            goto err;
        if (!TEST_ptr_null(pkey))
            goto err;
    } else if (strncmp(pairwise_name, "eddsa", 5) == 0) {
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "ED25519", NULL)))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
            goto err;
        if (!TEST_int_le(EVP_PKEY_keygen(ctx, &pkey), 0))
            goto err;
        if (!TEST_ptr_null(pkey))
            goto err;
    } else if (strncmp(pairwise_name, "ml-dsa", 6) == 0) {
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "ML-DSA-87", NULL)))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
            goto err;
        if (!TEST_int_le(EVP_PKEY_keygen(ctx, &pkey), 0))
            goto err;
        if (!TEST_ptr_null(pkey))
            goto err;
    } else if (strncmp(pairwise_name, "slh-dsa", 7) == 0) {
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "SLH-DSA-SHA2-256f", NULL)))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
            goto err;
        if (!TEST_int_le(EVP_PKEY_keygen(ctx, &pkey), 0))
            goto err;
        if (!TEST_ptr_null(pkey))
            goto err;
    } else if (strncmp(pairwise_name, "ml-kem", 6) == 0) {
        if (!TEST_true(setup_selftest_pairwise_failure(type)))
            goto err;
        if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_name(libctx, "ML-KEM-1024", NULL)))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_keygen_init(ctx), 1))
            goto err;
        if (!TEST_int_le(EVP_PKEY_keygen(ctx, &pkey), 0))
            goto err;
        if (!TEST_ptr_null(pkey))
            goto err;
    }
    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(ctx);
    BIO_free(bio);
    EVP_PKEY_free(pParams);
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
        case OPT_PAIRWISETEST:
            pairwise_name = opt_arg();
            break;
        case OPT_DSAPARAM:
            dsaparam_file = opt_arg();
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
    if (!OSSL_LIB_CTX_load_config(libctx, config_file)) {
        opt_printf_stderr("Failed to load config\n");
        return 0;
    }
    ADD_TEST(test_keygen_pairwise_failure);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_LIB_CTX_free(libctx);
}
