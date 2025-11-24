/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/pem.h>
#include <openssl/evp.h>
#include "testutil.h"

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *nullprov = NULL;
static OSSL_PROVIDER *libprov = NULL;
static const char *filename = NULL;
static pem_password_cb passcb;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_PROVIDER_NAME,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("file\n"),
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { "provider", OPT_PROVIDER_NAME, 's',
          "The provider to load (The default value is 'default')" },
        { OPT_HELP_STR, 1, '-', "file\tFile to decode.\n" },
        { NULL }
    };
    return test_options;
}

static int passcb(char *buf, int size, int rwflag, void *userdata)
{
    strcpy(buf, "pass");
    return strlen(buf);
}

static int test_decode_nonfipsalg(void)
{
    int ret = 0;
    EVP_PKEY *privkey = NULL;
    BIO *bio = NULL;

    /*
     * Apply the "fips=true" property to all fetches for the libctx.
     * We do this to test that we are using the propq override
     */
    EVP_default_properties_enable_fips(libctx, 1);

    if (!TEST_ptr(bio = BIO_new_file(filename, "r")))
        goto err;

    /*
     * If NULL is passed as the propq here it uses the global property "fips=true",
     * Which we expect to fail if the decode uses a non FIPS algorithm
     */
    if (!TEST_ptr_null(PEM_read_bio_PrivateKey_ex(bio, &privkey, &passcb, NULL, libctx, NULL)))
        goto err;

    /*
     * Pass if we override the libctx global prop query to optionally use fips=true
     * This assumes that the libctx contains the default provider
     */
    if (!TEST_ptr_null(PEM_read_bio_PrivateKey_ex(bio, &privkey, &passcb, NULL, libctx, "?fips=true")))
        goto err;

    ret = 1;
err:
    BIO_free(bio);
    EVP_PKEY_free(privkey);
    return ret;
}

int setup_tests(void)
{
    const char *prov_name = "default";
    char *config_file = NULL;
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_PROVIDER_NAME:
            prov_name = opt_arg();
            break;
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

    filename = test_get_argument(0);
    if (!test_get_libctx(&libctx, &nullprov, config_file, &libprov, prov_name))
        return 0;

    ADD_TEST(test_decode_nonfipsalg);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(libprov);
    OSSL_LIB_CTX_free(libctx);
    OSSL_PROVIDER_unload(nullprov);
}
