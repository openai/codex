/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include "testutil.h"

static int is_fips;
static int bad_fips;

static int test_is_fips_enabled(void)
{
    int is_fips_enabled, is_fips_loaded;
    EVP_MD *sha256 = NULL;

    /*
     * Check we're in FIPS mode when we're supposed to be. We do this early to
     * confirm that EVP_default_properties_is_fips_enabled() works even before
     * other function calls have auto-loaded the config file.
     */
    is_fips_enabled = EVP_default_properties_is_fips_enabled(NULL);
    is_fips_loaded = OSSL_PROVIDER_available(NULL, "fips");

    /*
     * Check we're in an expected state. EVP_default_properties_is_fips_enabled
     * can return true even if the FIPS provider isn't loaded - it is only based
     * on the default properties. However we only set those properties if also
     * loading the FIPS provider.
     */
    if (!TEST_int_eq(is_fips || bad_fips, is_fips_enabled)
            || !TEST_int_eq(is_fips && !bad_fips, is_fips_loaded))
        return 0;

    /*
     * Fetching an algorithm shouldn't change the state and should come from
     * expected provider.
     */
    sha256 = EVP_MD_fetch(NULL, "SHA2-256", NULL);
    if (bad_fips) {
        if (!TEST_ptr_null(sha256)) {
            EVP_MD_free(sha256);
            return 0;
        }
    } else {
        if (!TEST_ptr(sha256))
            return 0;
        if (is_fips
            && !TEST_str_eq(OSSL_PROVIDER_get0_name(EVP_MD_get0_provider(sha256)),
                            "fips")) {
            EVP_MD_free(sha256);
            return 0;
        }
        EVP_MD_free(sha256);
    }

    /* State should still be consistent */
    is_fips_enabled = EVP_default_properties_is_fips_enabled(NULL);
    if (!TEST_int_eq(is_fips || bad_fips, is_fips_enabled))
        return 0;

    return 1;
}

int setup_tests(void)
{
    size_t argc;
    char *arg1;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    argc = test_get_argument_count();
    switch (argc) {
    case 0:
        is_fips = 0;
        bad_fips = 0;
        break;
    case 1:
        arg1 = test_get_argument(0);
        if (strcmp(arg1, "fips") == 0) {
            is_fips = 1;
            bad_fips = 0;
            break;
        } else if (strcmp(arg1, "badfips") == 0) {
            /* Configured for FIPS, but the module fails to load */
            is_fips = 0;
            bad_fips = 1;
            break;
        }
        /* fall through */
    default:
        TEST_error("Invalid argument\n");
        return 0;
    }

    /* Must be the first test before any other libcrypto calls are made */
    ADD_TEST(test_is_fips_enabled);
    return 1;
}
