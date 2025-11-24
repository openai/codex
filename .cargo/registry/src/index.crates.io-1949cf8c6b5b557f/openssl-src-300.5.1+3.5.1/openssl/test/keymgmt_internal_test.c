/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/bio.h>
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/pem.h>
#include <openssl/provider.h>
#include <openssl/core_names.h>
#include "internal/core.h"
#include "internal/nelem.h"
#include "crypto/evp.h"          /* For the internal API */
#include "testutil.h"

typedef struct {
    OSSL_LIB_CTX *ctx1;
    OSSL_PROVIDER *prov1;
    OSSL_LIB_CTX *ctx2;
    OSSL_PROVIDER *prov2;
} FIXTURE;

/* Collected arguments */
static const char *cert_filename = NULL;

static void tear_down(FIXTURE *fixture)
{
    if (fixture != NULL) {
        OSSL_PROVIDER_unload(fixture->prov1);
        OSSL_PROVIDER_unload(fixture->prov2);
        OSSL_LIB_CTX_free(fixture->ctx1);
        OSSL_LIB_CTX_free(fixture->ctx2);
        OPENSSL_free(fixture);
    }
}

static FIXTURE *set_up(const char *testcase_name)
{
    FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture)))
        || !TEST_ptr(fixture->ctx1 = OSSL_LIB_CTX_new())
        || !TEST_ptr(fixture->prov1 = OSSL_PROVIDER_load(fixture->ctx1,
                                                         "default"))
        || !TEST_ptr(fixture->ctx2 = OSSL_LIB_CTX_new())
        || !TEST_ptr(fixture->prov2 = OSSL_PROVIDER_load(fixture->ctx2,
                                                         "default"))) {
        tear_down(fixture);
        return NULL;
    }
    return fixture;
}

/* Array indexes */
#define N       0
#define E       1
#define D       2
#define P       3
#define Q       4
#define F3      5                /* Extra factor */
#define DP      6
#define DQ      7
#define E3      8                /* Extra exponent */
#define QINV    9
#define C2      10               /* Extra coefficient */

/*
 * We have to do this because OSSL_PARAM_get_ulong() can't handle params
 * holding data that isn't exactly sizeof(uint32_t) or sizeof(uint64_t),
 * and because the other end deals with BIGNUM, the resulting param might
 * be any size.  In this particular test, we know that the expected data
 * fits within an unsigned long, and we want to get the data in that form
 * to make testing of values easier.
 */
static int get_ulong_via_BN(const OSSL_PARAM *p, unsigned long *goal)
{
    BIGNUM *n = NULL;
    int ret = 1;                 /* Ever so hopeful */

    if (!TEST_true(OSSL_PARAM_get_BN(p, &n))
        || !TEST_int_ge(BN_bn2nativepad(n, (unsigned char *)goal, sizeof(*goal)), 0))
        ret = 0;
    BN_free(n);
    return ret;
}

static int export_cb(const OSSL_PARAM *params, void *arg)
{
    unsigned long *keydata = arg;
    const OSSL_PARAM *p = NULL;

    if (keydata == NULL)
        return 0;

    if (!TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_N))
        || !TEST_true(get_ulong_via_BN(p, &keydata[N]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_E))
        || !TEST_true(get_ulong_via_BN(p, &keydata[E]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_D))
        || !TEST_true(get_ulong_via_BN(p, &keydata[D])))
        return 0;

    if (!TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_FACTOR1))
        || !TEST_true(get_ulong_via_BN(p, &keydata[P]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_FACTOR2))
        || !TEST_true(get_ulong_via_BN(p, &keydata[Q]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_FACTOR3))
        || !TEST_true(get_ulong_via_BN(p, &keydata[F3])))
        return 0;

    if (!TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_EXPONENT1))
        || !TEST_true(get_ulong_via_BN(p, &keydata[DP]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_EXPONENT2))
        || !TEST_true(get_ulong_via_BN(p, &keydata[DQ]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_EXPONENT3))
        || !TEST_true(get_ulong_via_BN(p, &keydata[E3])))
        return 0;

    if (!TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_COEFFICIENT1))
        || !TEST_true(get_ulong_via_BN(p, &keydata[QINV]))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_COEFFICIENT2))
        || !TEST_true(get_ulong_via_BN(p, &keydata[C2])))
        return 0;

    return 1;
}

static int test_pass_rsa(FIXTURE *fixture)
{
    size_t i;
    int ret = 0;
    RSA *rsa = NULL;
    BIGNUM *bn1 = NULL, *bn2 = NULL, *bn3 = NULL;
    EVP_PKEY *pk = NULL, *dup_pk = NULL;
    EVP_KEYMGMT *km = NULL, *km1 = NULL, *km2 = NULL, *km3 = NULL;
    void *provkey = NULL, *provkey2 = NULL;
    BIGNUM *bn_primes[1] = { NULL };
    BIGNUM *bn_exps[1] = { NULL };
    BIGNUM *bn_coeffs[1] = { NULL };
    /*
     * 32-bit RSA key, extracted from this command,
     * executed with OpenSSL 1.0.2:
     * An extra factor was added just for testing purposes.
     *
     * openssl genrsa 32 | openssl rsa -text
     */
    static BN_ULONG expected[] = {
        0xbc747fc5,              /* N */
        0x10001,                 /* E */
        0x7b133399,              /* D */
        0xe963,                  /* P */
        0xceb7,                  /* Q */
        1,                       /* F3 */
        0x8599,                  /* DP */
        0xbd87,                  /* DQ */
        2,                       /* E3 */
        0xcc3b,                  /* QINV */
        3,                       /* C3 */
        0                        /* Extra, should remain zero */
    };
    static unsigned long keydata[OSSL_NELEM(expected)] = { 0, };

    if (!TEST_ptr(rsa = RSA_new()))
        goto err;

    if (!TEST_ptr(bn1 = BN_new())
        || !TEST_true(BN_set_word(bn1, expected[N]))
        || !TEST_ptr(bn2 = BN_new())
        || !TEST_true(BN_set_word(bn2, expected[E]))
        || !TEST_ptr(bn3 = BN_new())
        || !TEST_true(BN_set_word(bn3, expected[D]))
        || !TEST_true(RSA_set0_key(rsa, bn1, bn2, bn3)))
        goto err;

    if (!TEST_ptr(bn1 = BN_new())
        || !TEST_true(BN_set_word(bn1, expected[P]))
        || !TEST_ptr(bn2 = BN_new())
        || !TEST_true(BN_set_word(bn2, expected[Q]))
        || !TEST_true(RSA_set0_factors(rsa, bn1, bn2)))
        goto err;

    if (!TEST_ptr(bn1 = BN_new())
        || !TEST_true(BN_set_word(bn1, expected[DP]))
        || !TEST_ptr(bn2 = BN_new())
        || !TEST_true(BN_set_word(bn2, expected[DQ]))
        || !TEST_ptr(bn3 = BN_new())
        || !TEST_true(BN_set_word(bn3, expected[QINV]))
        || !TEST_true(RSA_set0_crt_params(rsa, bn1, bn2, bn3)))
        goto err;
    bn1 = bn2 = bn3 = NULL;

    if (!TEST_ptr(bn_primes[0] = BN_new())
        || !TEST_true(BN_set_word(bn_primes[0], expected[F3]))
        || !TEST_ptr(bn_exps[0] = BN_new())
        || !TEST_true(BN_set_word(bn_exps[0], expected[E3]))
        || !TEST_ptr(bn_coeffs[0] = BN_new())
        || !TEST_true(BN_set_word(bn_coeffs[0], expected[C2]))
        || !TEST_true(RSA_set0_multi_prime_params(rsa, bn_primes, bn_exps,
                                                  bn_coeffs, 1)))
        goto err;

    if (!TEST_ptr(pk = EVP_PKEY_new())
        || !TEST_true(EVP_PKEY_assign_RSA(pk, rsa)))
        goto err;
    rsa = NULL;

    if (!TEST_ptr(km1 = EVP_KEYMGMT_fetch(fixture->ctx1, "RSA", NULL))
        || !TEST_ptr(km2 = EVP_KEYMGMT_fetch(fixture->ctx2, "RSA", NULL))
        || !TEST_ptr(km3 = EVP_KEYMGMT_fetch(fixture->ctx1, "RSA-PSS", NULL))
        || !TEST_ptr_ne(km1, km2))
        goto err;

    for (;;) {
        ret = 0;
        km = km3;
        /* Check that we can't export an RSA key into an RSA-PSS keymanager */
        if (!TEST_ptr_null(provkey2 = evp_pkey_export_to_provider(pk, NULL,
                                                                  &km,
                                                                  NULL)))
            goto err;

        if (!TEST_ptr(provkey = evp_pkey_export_to_provider(pk, NULL, &km1,
                                                            NULL))
            || !TEST_true(evp_keymgmt_export(km2, provkey,
                                             OSSL_KEYMGMT_SELECT_KEYPAIR,
                                             &export_cb, keydata)))
            goto err;

        /*
         * At this point, the hope is that keydata will have all the numbers
         * from the key.
         */

        for (i = 0; i < OSSL_NELEM(expected); i++) {
            int rv = TEST_int_eq(expected[i], keydata[i]);

            if (!rv)
                TEST_info("i = %zu", i);
            else
                ret++;
        }

        ret = (ret == OSSL_NELEM(expected));

        if (!ret || dup_pk != NULL)
            break;

        if (!TEST_ptr(dup_pk = EVP_PKEY_dup(pk)))
            goto err;

        ret = TEST_int_eq(EVP_PKEY_eq(pk, dup_pk), 1);
        EVP_PKEY_free(pk);
        pk = dup_pk;
        if (!ret)
            goto err;
    }

 err:
    RSA_free(rsa);
    BN_free(bn1);
    BN_free(bn2);
    BN_free(bn3);
    EVP_PKEY_free(pk);
    EVP_KEYMGMT_free(km1);
    EVP_KEYMGMT_free(km2);
    EVP_KEYMGMT_free(km3);

    return ret;
}

static int (*tests[])(FIXTURE *) = {
    test_pass_rsa
};

static int test_pass_key(int n)
{
    SETUP_TEST_FIXTURE(FIXTURE, set_up);
    EXECUTE_TEST(tests[n], tear_down);
    return result;
}

static int test_evp_pkey_export_to_provider(int n)
{
    OSSL_LIB_CTX *libctx = NULL;
    OSSL_PROVIDER *prov = NULL;
    X509 *cert = NULL;
    BIO *bio = NULL;
    X509_PUBKEY *pubkey = NULL;
    EVP_KEYMGMT *keymgmt = NULL;
    EVP_PKEY *pkey = NULL;
    void *keydata = NULL;
    int ret = 0;

    if (!TEST_ptr(libctx = OSSL_LIB_CTX_new())
         || !TEST_ptr(prov = OSSL_PROVIDER_load(libctx, "default")))
        goto end;

    if ((bio = BIO_new_file(cert_filename, "r")) == NULL) {
        TEST_error("Couldn't open '%s' for reading\n", cert_filename);
        TEST_openssl_errors();
        goto end;
    }

    if ((cert = PEM_read_bio_X509(bio, NULL, NULL, NULL)) == NULL) {
        TEST_error("'%s' doesn't appear to be a X.509 certificate in PEM format\n",
                   cert_filename);
        TEST_openssl_errors();
        goto end;
    }

    pubkey = X509_get_X509_PUBKEY(cert);
    pkey = X509_PUBKEY_get0(pubkey);

    if (n == 0) {
        if (!TEST_ptr(keydata = evp_pkey_export_to_provider(pkey, NULL,
                                                            NULL, NULL)))
            goto end;
    } else if (n == 1) {
        if (!TEST_ptr(keydata = evp_pkey_export_to_provider(pkey, NULL,
                                                            &keymgmt, NULL)))
            goto end;
    } else {
        keymgmt = EVP_KEYMGMT_fetch(libctx, "RSA", NULL);

        if (!TEST_ptr(keydata = evp_pkey_export_to_provider(pkey, NULL,
                                                            &keymgmt, NULL)))
            goto end;
    }

    ret = 1;
 end:
    BIO_free(bio);
    X509_free(cert);
    EVP_KEYMGMT_free(keymgmt);
    OSSL_PROVIDER_unload(prov);
    OSSL_LIB_CTX_free(libctx);
    return ret;
}

int setup_tests(void)
{
    if (!TEST_ptr(cert_filename = test_get_argument(0)))
        return 0;

    ADD_ALL_TESTS(test_pass_key, 1);
    ADD_ALL_TESTS(test_evp_pkey_export_to_provider, 3);
    return 1;
}
