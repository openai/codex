/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "internal/nelem.h"

#include <openssl/pkcs12.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/pem.h>

#include "testutil.h"
#include "helpers/pkcs12.h"

static OSSL_LIB_CTX *testctx = NULL;
static OSSL_PROVIDER *nullprov = NULL;

static int test_null_args(void)
{
    return TEST_false(PKCS12_parse(NULL, NULL, NULL, NULL, NULL));
}

static PKCS12 *PKCS12_load(const char *fpath)
{
    BIO *bio = NULL;
    PKCS12 *p12 = NULL;

    bio = BIO_new_file(fpath, "rb");
    if (!TEST_ptr(bio))
        goto err;

    p12 = PKCS12_init_ex(NID_pkcs7_data, testctx, "provider=default");
    if (!TEST_ptr(p12))
        goto err;

    if (!TEST_true(p12 == d2i_PKCS12_bio(bio, &p12)))
        goto err;

    BIO_free(bio);

    return p12;

err:
    BIO_free(bio);
    PKCS12_free(p12);
    return NULL;
}

static const char *in_file = NULL;
static const char *in_pass = "";
static int has_key = 0;
static int has_cert = 0;
static int has_ca = 0;

static int changepass(PKCS12 *p12, EVP_PKEY *key, X509 *cert, STACK_OF(X509) *ca)
{
    int ret = 0;
    PKCS12 *p12new = NULL;
    EVP_PKEY *key2 = NULL;
    X509 *cert2 = NULL;
    STACK_OF(X509) *ca2 = NULL;
    BIO *bio = NULL;

    if (!TEST_true(PKCS12_newpass(p12, in_pass, "NEWPASS")))
        goto err;
    if (!TEST_ptr(bio = BIO_new(BIO_s_mem())))
        goto err;
    if (!TEST_true(i2d_PKCS12_bio(bio, p12)))
        goto err;
    if (!TEST_ptr(p12new = PKCS12_init_ex(NID_pkcs7_data, testctx, "provider=default")))
        goto err;
    if (!TEST_ptr(d2i_PKCS12_bio(bio, &p12new)))
        goto err;
    if (!TEST_true(PKCS12_parse(p12new, "NEWPASS", &key2, &cert2, &ca2)))
        goto err;
    if (has_key) {
        if (!TEST_ptr(key2) || !TEST_int_eq(EVP_PKEY_eq(key, key2), 1))
            goto err;
    }
    if (has_cert) {
        if (!TEST_ptr(cert2) || !TEST_int_eq(X509_cmp(cert, cert2), 0))
            goto err;
    }
    ret = 1;
err:
    BIO_free(bio);
    PKCS12_free(p12new);
    EVP_PKEY_free(key2);
    X509_free(cert2);
    OSSL_STACK_OF_X509_free(ca2);
    return ret;
}

static int pkcs12_parse_test(void)
{
    int ret = 0;
    PKCS12 *p12 = NULL;
    EVP_PKEY *key = NULL;
    X509 *cert = NULL;
    STACK_OF(X509) *ca = NULL;

    if (in_file != NULL) {
        p12 = PKCS12_load(in_file);
        if (!TEST_ptr(p12))
            goto err;

        if (!TEST_true(PKCS12_parse(p12, in_pass, &key, &cert, &ca)))
            goto err;

        if ((has_key && !TEST_ptr(key)) || (!has_key && !TEST_ptr_null(key)))
            goto err;
        if ((has_cert && !TEST_ptr(cert)) || (!has_cert && !TEST_ptr_null(cert)))
            goto err;
        if ((has_ca && !TEST_ptr(ca)) || (!has_ca && !TEST_ptr_null(ca)))
            goto err;
        if (has_key && !changepass(p12, key, cert, ca))
            goto err;
    }
    ret = 1;
err:
    PKCS12_free(p12);
    EVP_PKEY_free(key);
    X509_free(cert);
    OSSL_STACK_OF_X509_free(ca);
    return TEST_true(ret);
}

static int pkcs12_create_cb(PKCS12_SAFEBAG *bag, void *cbarg)
{
    int cb_ret = *((int*)cbarg);
    return cb_ret;
}

static PKCS12 *pkcs12_create_ex2_setup(EVP_PKEY **key, X509 **cert, STACK_OF(X509) **ca)
{
    PKCS12 *p12 = NULL;
    p12 = PKCS12_load("out6.p12");
    if (!TEST_ptr(p12))
        goto err;

    if (!TEST_true(PKCS12_parse(p12, "", key, cert, ca)))
        goto err;

    return p12;
err:
    PKCS12_free(p12);
    return NULL;
}

static int pkcs12_create_ex2_test(int test)
{
    int ret = 0, cb_ret = 0;
    PKCS12 *ptr = NULL, *p12 = NULL;
    EVP_PKEY *key = NULL;
    X509 *cert = NULL;
    STACK_OF(X509) *ca = NULL;

    p12 = pkcs12_create_ex2_setup(&key, &cert, &ca);
    if (!TEST_ptr(p12))
        goto err;

    if (test == 0) {
        /* Confirm PKCS12_create_ex2 returns NULL */
        ptr = PKCS12_create_ex2(NULL, NULL, NULL,
                                NULL, NULL, NID_undef, NID_undef,
                                0, 0, 0,
                                testctx, NULL,
                                NULL, NULL);
        if (TEST_ptr(ptr))
            goto err;

        /* Can't proceed without a valid cert at least */
        if (!TEST_ptr(cert))
            goto err;

        /* Specified call back called - return success */
        cb_ret = 1;
        ptr = PKCS12_create_ex2(NULL, NULL, NULL,
                                cert, NULL, NID_undef, NID_undef,
                                0, 0, 0,
                                testctx, NULL,
                                pkcs12_create_cb, (void*)&cb_ret);
        /* PKCS12 successfully created */
        if (!TEST_ptr(ptr))
            goto err;
    } else if (test == 1) {
        /* Specified call back called - return error*/
        cb_ret = -1;
        ptr = PKCS12_create_ex2(NULL, NULL, NULL,
                                cert, NULL, NID_undef, NID_undef,
                                0, 0, 0,
                                testctx, NULL,
                                pkcs12_create_cb, (void*)&cb_ret);
        /* PKCS12 not created */
       if (TEST_ptr(ptr))
            goto err;
    } else if (test == 2) {
        /* Specified call back called - return failure */
        cb_ret = 0;
        ptr = PKCS12_create_ex2(NULL, NULL, NULL,
                                cert, NULL, NID_undef, NID_undef,
                                0, 0, 0,
                                testctx, NULL,
                                pkcs12_create_cb, (void*)&cb_ret);
        /* PKCS12 successfully created */
        if (!TEST_ptr(ptr))
            goto err;
    }

    ret = 1;
err:
    PKCS12_free(p12);
    PKCS12_free(ptr);
    EVP_PKEY_free(key);
    X509_free(cert);
    OSSL_STACK_OF_X509_free(ca);
    return TEST_true(ret);
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_IN_FILE,
    OPT_IN_PASS,
    OPT_IN_HAS_KEY,
    OPT_IN_HAS_CERT,
    OPT_IN_HAS_CA,
    OPT_LEGACY,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "in",   OPT_IN_FILE,   '<', "PKCS12 input file" },
        { "pass",   OPT_IN_PASS,   's', "PKCS12 input file password" },
        { "has-key",   OPT_IN_HAS_KEY,  'n', "Whether the input file does contain an user key" },
        { "has-cert",   OPT_IN_HAS_CERT, 'n', "Whether the input file does contain an user certificate" },
        { "has-ca",   OPT_IN_HAS_CA,   'n', "Whether the input file does contain other certificate" },
        { "legacy",  OPT_LEGACY,  '-', "Test the legacy APIs" },
        { NULL }
    };
    return options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_IN_FILE:
            in_file = opt_arg();
            break;
        case OPT_IN_PASS:
            in_pass = opt_arg();
            break;
        case OPT_LEGACY:
            break;
        case OPT_IN_HAS_KEY:
            has_key = opt_int_arg();
            break;
        case OPT_IN_HAS_CERT:
            has_cert = opt_int_arg();
            break;
        case OPT_IN_HAS_CA:
            has_ca = opt_int_arg();
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    if (!test_get_libctx(&testctx, &nullprov, NULL, NULL, NULL)) {
        OSSL_LIB_CTX_free(testctx);
        testctx = NULL;
        return 0;
    }

    ADD_TEST(test_null_args);
    ADD_TEST(pkcs12_parse_test);
    ADD_ALL_TESTS(pkcs12_create_ex2_test, 3);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_LIB_CTX_free(testctx);
    OSSL_PROVIDER_unload(nullprov);
}
