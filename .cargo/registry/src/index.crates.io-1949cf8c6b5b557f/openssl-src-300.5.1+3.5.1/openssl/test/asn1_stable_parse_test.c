/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include "testutil.h"

static char *config_file = NULL;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { NULL }
    };
    return options;
}


/*
 * Test that parsing a config file with incorrect stable settings aren't parsed
 * and appropriate errors are raised
 */
static int test_asn1_stable_parse(void)
{
    int testret = 0;
    unsigned long errcode;
    OSSL_LIB_CTX *newctx = OSSL_LIB_CTX_new();

    if (!TEST_ptr(newctx))
        goto out;

    if (!TEST_int_eq(OSSL_LIB_CTX_load_config(newctx, config_file), 0))
        goto err;

    errcode = ERR_peek_error();
    if (ERR_GET_LIB(errcode) != ERR_LIB_ASN1)
        goto err;
    if (ERR_GET_REASON(errcode) != ASN1_R_INVALID_STRING_TABLE_VALUE)
        goto err;

    ERR_clear_error();

    testret = 1;
err:
    OSSL_LIB_CTX_free(newctx);
out:
    return testret;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        default:
            return 0;
        }
    }

    ADD_TEST(test_asn1_stable_parse);
    return 1;
}
