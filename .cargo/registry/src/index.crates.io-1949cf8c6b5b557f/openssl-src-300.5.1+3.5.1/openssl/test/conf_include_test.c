/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include <openssl/conf.h>
#include <openssl/err.h>
#include "testutil.h"

#ifdef _WIN32
# include <direct.h>
# define DIRSEP "/\\"
# ifndef __BORLANDC__
#  define chdir _chdir
# endif
# define DIRSEP_PRESERVE 0
#elif !defined(OPENSSL_NO_POSIX_IO)
# include <unistd.h>
# ifndef OPENSSL_SYS_VMS
#  define DIRSEP "/"
#  define DIRSEP_PRESERVE 0
# else
#  define DIRSEP "/]:"
#  define DIRSEP_PRESERVE 1
# endif
#else
/* the test does not work without chdir() */
# define chdir(x) (-1);
# define DIRSEP "/"
#  define DIRSEP_PRESERVE 0
#endif

/* changes path to that of the filename */
static char *change_path(const char *file)
{
    char *s = OPENSSL_strdup(file);
    char *p = s;
    char *last = NULL;
    int ret = 0;
    char *new_config_name = NULL;

    if (s == NULL)
        return NULL;

    while ((p = strpbrk(p, DIRSEP)) != NULL) {
        last = p++;
    }
    if (last == NULL)
        goto err;

    last[DIRSEP_PRESERVE] = 0;
    TEST_note("changing path to %s", s);

    ret = chdir(s);
    if (ret == 0)
        new_config_name = OPENSSL_strdup(last + DIRSEP_PRESERVE + 1);
 err:
    OPENSSL_free(s);
    return new_config_name;
}

/*
 * This test program checks the operation of the .include directive.
 */

static CONF *conf;
static BIO *in;
static int expect_failure = 0;
static int test_providers = 0;
static OSSL_LIB_CTX *libctx = NULL;
static char *rel_conf_file = NULL;

static int test_load_config(void)
{
    long errline;
    long val;
    char *str;
    long err;

    if (!TEST_int_gt(NCONF_load_bio(conf, in, &errline), 0)
        || !TEST_int_eq(err = ERR_peek_error(), 0)) {
        if (expect_failure)
            return 1;
        TEST_note("Failure loading the configuration at line %ld", errline);
        return 0;
    }
    if (expect_failure) {
        TEST_note("Failure expected but did not happen");
        return 0;
    }

    if (!TEST_int_gt(CONF_modules_load(conf, NULL, 0), 0)) {
        TEST_note("Failed in CONF_modules_load");
        return 0;
    }

    /* verify whether CA_default/default_days is set */
    val = 0;
    if (!TEST_int_eq(NCONF_get_number(conf, "CA_default", "default_days", &val), 1)
        || !TEST_int_eq(val, 365)) {
        TEST_note("default_days incorrect");
        return 0;
    }

    /* verify whether req/default_bits is set */
    val = 0;
    if (!TEST_int_eq(NCONF_get_number(conf, "req", "default_bits", &val), 1)
        || !TEST_int_eq(val, 2048)) {
        TEST_note("default_bits incorrect");
        return 0;
    }

    /* verify whether countryName_default is set correctly */
    str = NCONF_get_string(conf, "req_distinguished_name", "countryName_default");
    if (!TEST_ptr(str) || !TEST_str_eq(str, "AU")) {
        TEST_note("countryName_default incorrect");
        return 0;
    }

    if (test_providers != 0) {
        /* test for `active` directive in configuration file */
        val = 0;
        if (!TEST_int_eq(NCONF_get_number(conf, "null_sect", "activate", &val), 1)
            || !TEST_int_eq(val, 1)) {
            TEST_note("null provider not activated");
            return 0;
        }
        val = 0;
        if (!TEST_int_eq(NCONF_get_number(conf, "default_sect", "activate", &val), 1)
            || !TEST_int_eq(val, 1)) {
            TEST_note("default provider not activated");
            return 0;
        }
        val = 0;
        if (!TEST_int_eq(NCONF_get_number(conf, "legacy_sect", "activate", &val), 1)
            || !TEST_int_eq(val, 1)) {
            TEST_note("legacy provider not activated");
            return 0;
        }
    }
    return 1;
}

static int test_check_null_numbers(void)
{
#if defined(_BSD_SOURCE) \
        || (defined(_POSIX_C_SOURCE) && _POSIX_C_SOURCE >= 200112L) \
        || (defined(_XOPEN_SOURCE) && _XOPEN_SOURCE >= 600)
    long val = 0;

    /* Verify that a NULL config with a present environment variable returns
     * success and the value.
     */
    if (!TEST_int_eq(setenv("FNORD", "123", 1), 0)
            || !TEST_true(NCONF_get_number(NULL, "missing", "FNORD", &val))
            || !TEST_long_eq(val, 123)) {
        TEST_note("environment variable with NULL conf failed");
        return 0;
    }

    /*
     * Verify that a NULL config with a missing environment variable returns
     * a failure code.
     */
    if (!TEST_int_eq(unsetenv("FNORD"), 0)
            || !TEST_false(NCONF_get_number(NULL, "missing", "FNORD", &val))) {
        TEST_note("missing environment variable with NULL conf failed");
        return 0;
    }
#endif
    return 1;
}

static int test_check_overflow(void)
{
#if defined(_BSD_SOURCE) \
        || (defined(_POSIX_C_SOURCE) && _POSIX_C_SOURCE >= 200112L) \
        || (defined(_XOPEN_SOURCE) && _XOPEN_SOURCE >= 600)
    long val = 0;
    char max[(sizeof(long) * 8) / 3 + 3];
    char *p;

    p = max + BIO_snprintf(max, sizeof(max), "0%ld", LONG_MAX) - 1;
    setenv("FNORD", max, 1);
    if (!TEST_true(NCONF_get_number(NULL, "missing", "FNORD", &val))
            || !TEST_long_eq(val, LONG_MAX))
        return 0;

    while (++*p > '9')
        *p-- = '0';

    setenv("FNORD", max, 1);
    if (!TEST_false(NCONF_get_number(NULL, "missing", "FNORD", &val)))
        return 0;
#endif
    return 1;
}

static int test_available_providers(void)
{
    libctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(libctx))
        return 0;

    if (!TEST_ptr(rel_conf_file) || !OSSL_LIB_CTX_load_config(libctx, rel_conf_file)) {
        TEST_note("Failed to load config");
        return 0;
    }

    if (OSSL_PROVIDER_available(libctx, "default") != 1) {
        TEST_note("Default provider is missing");
        return 0;
    }
    if (OSSL_PROVIDER_available(libctx, "legacy") != 1) {
        TEST_note("Legacy provider is missing");
        return 0;
    }
    return 1;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_FAIL,
    OPT_TEST_PROV,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("conf_file\n"),
        { "f", OPT_FAIL, '-', "A failure is expected" },
        { "providers", OPT_TEST_PROV, '-',
          "Test for activated default and legacy providers"},
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    char *conf_file = NULL;
    OPTION_CHOICE o;

    if (!TEST_ptr(conf = NCONF_new(NULL)))
        return 0;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_FAIL:
            expect_failure = 1;
            break;
        case OPT_TEST_PROV:
            test_providers = 1;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    conf_file = test_get_argument(0);
    if (!TEST_ptr(conf_file)
        || !TEST_ptr(in = BIO_new_file(conf_file, "r"))) {
        TEST_note("Unable to open the file argument");
        return 0;
    }

    /*
     * For this test we need to chdir as we use relative
     * path names in the config files.
     */
    rel_conf_file = change_path(conf_file);
    if (!TEST_ptr(rel_conf_file)) {
        TEST_note("Unable to change path");
        return 0;
    }

    ADD_TEST(test_load_config);
    ADD_TEST(test_check_null_numbers);
    ADD_TEST(test_check_overflow);
    if (test_providers != 0)
        ADD_TEST(test_available_providers);

    return 1;
}

void cleanup_tests(void)
{
    OPENSSL_free(rel_conf_file);
    BIO_vfree(in);
    NCONF_free(conf);
    CONF_modules_unload(1);
}
