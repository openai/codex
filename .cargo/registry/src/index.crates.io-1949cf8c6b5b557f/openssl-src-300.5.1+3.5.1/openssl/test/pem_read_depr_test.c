/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file tests deprecated APIs. Therefore we need to suppress deprecation
 * warnings.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/pem.h>
#include <openssl/bio.h>
#include <openssl/dh.h>
#include <openssl/dsa.h>
#include <openssl/rsa.h>

#include "testutil.h"

static const char *datadir;

static BIO *getfile(const char *filename)
{
    char *paramsfile = test_mk_file_path(datadir, filename);
    BIO *infile = NULL;

    if (!TEST_ptr(paramsfile))
        goto err;
    infile = BIO_new_file(paramsfile, "r");

 err:
    OPENSSL_free(paramsfile);
    return infile;
}

#ifndef OPENSSL_NO_DH
static int test_read_dh_params(void)
{
    int testresult = 0;
    BIO *infile = getfile("dhparams.pem");
    DH *dh = NULL;

    if (!TEST_ptr(infile))
        goto err;

    dh = PEM_read_bio_DHparams(infile, NULL, NULL, NULL);
    if (!TEST_ptr(dh))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    DH_free(dh);
    return testresult;
}

static int test_read_dh_x942_params(void)
{
    int testresult = 0;
    BIO *infile = getfile("x942params.pem");
    DH *dh = NULL;

    if (!TEST_ptr(infile))
        goto err;

    dh = PEM_read_bio_DHparams(infile, NULL, NULL, NULL);
    if (!TEST_ptr(dh))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    DH_free(dh);
    return testresult;
}
#endif

#ifndef OPENSSL_NO_DSA
static int test_read_dsa_params(void)
{
    int testresult = 0;
    BIO *infile = getfile("dsaparams.pem");
    DSA *dsa = NULL;

    if (!TEST_ptr(infile))
        goto err;

    dsa = PEM_read_bio_DSAparams(infile, NULL, NULL, NULL);
    if (!TEST_ptr(dsa))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    DSA_free(dsa);
    return testresult;
}

static int test_read_dsa_private(void)
{
    int testresult = 0;
    BIO *infile = getfile("dsaprivatekey.pem");
    DSA *dsa = NULL;

    if (!TEST_ptr(infile))
        goto err;

    dsa = PEM_read_bio_DSAPrivateKey(infile, NULL, NULL, NULL);
    if (!TEST_ptr(dsa))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    DSA_free(dsa);
    return testresult;
}

static int test_read_dsa_public(void)
{
    int testresult = 0;
    BIO *infile = getfile("dsapublickey.pem");
    DSA *dsa = NULL;

    if (!TEST_ptr(infile))
        goto err;

    dsa = PEM_read_bio_DSA_PUBKEY(infile, NULL, NULL, NULL);
    if (!TEST_ptr(dsa))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    DSA_free(dsa);
    return testresult;
}
#endif

static int test_read_rsa_private(void)
{
    int testresult = 0;
    BIO *infile = getfile("rsaprivatekey.pem");
    RSA *rsa = NULL;

    if (!TEST_ptr(infile))
        goto err;

    rsa = PEM_read_bio_RSAPrivateKey(infile, NULL, NULL, NULL);
    if (!TEST_ptr(rsa))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    RSA_free(rsa);
    return testresult;
}

static int test_read_rsa_public(void)
{
    int testresult = 0;
    BIO *infile = getfile("rsapublickey.pem");
    RSA *rsa = NULL;

    if (!TEST_ptr(infile))
        goto err;

    rsa = PEM_read_bio_RSA_PUBKEY(infile, NULL, NULL, NULL);
    if (!TEST_ptr(rsa))
        goto err;

    testresult = 1;

 err:
    BIO_free(infile);
    RSA_free(rsa);
    return testresult;
}

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(datadir = test_get_argument(0))) {
        TEST_error("Error getting data dir\n");
        return 0;
    }

#ifndef OPENSSL_NO_DH
    ADD_TEST(test_read_dh_params);
    ADD_TEST(test_read_dh_x942_params);
#endif
#ifndef OPENSSL_NO_DSA
    ADD_TEST(test_read_dsa_params);
    ADD_TEST(test_read_dsa_private);
    ADD_TEST(test_read_dsa_public);
#endif
    ADD_TEST(test_read_rsa_private);
    ADD_TEST(test_read_rsa_public);

    return 1;
}
