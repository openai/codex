/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/provider.h>
#include "testutil.h"

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *libprov = NULL;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return test_options;
}

static int test_fips_version(int n)
{
    const char *version = test_get_argument(n);

    if (!TEST_ptr(version))
        return 0;
    return TEST_int_eq(fips_provider_version_match(libctx, version), 1);
}

int setup_tests(void)
{
    char *config_file = NULL;
    OPTION_CHOICE o;
    int n;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    if (!test_get_libctx(&libctx, NULL, config_file, &libprov, NULL))
        return 0;

    n = test_get_argument_count();
    if (n == 0)
        return 0;

    ADD_ALL_TESTS(test_fips_version, n);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(libprov);
    OSSL_LIB_CTX_free(libctx);
}
