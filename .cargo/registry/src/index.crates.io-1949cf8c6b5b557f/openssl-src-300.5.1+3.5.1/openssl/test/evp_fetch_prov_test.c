/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * SHA256 low level APIs are deprecated for public use, but still ok for
 * internal use.  Note, that due to symbols not being exported, only the
 * #defines can be accessed.  In this case SHA256_CBLOCK.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/sha.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include "internal/sizes.h"
#include "testutil.h"

static char *config_file = NULL;
static char *alg = "digest";
static int use_default_ctx = 0;
static char *fetch_property = NULL;
static int expected_fetch_result = 1;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_ALG_FETCH_TYPE,
    OPT_FETCH_PROPERTY,
    OPT_FETCH_FAILURE,
    OPT_USE_DEFAULTCTX,
    OPT_CONFIG_FILE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("[provname...]\n"),
        { "config", OPT_CONFIG_FILE, '<', "The configuration file to use for the libctx" },
        { "type", OPT_ALG_FETCH_TYPE, 's', "The fetch type to test" },
        { "property", OPT_FETCH_PROPERTY, 's', "The fetch property e.g. provider=fips" },
        { "fetchfail", OPT_FETCH_FAILURE, '-', "fetch is expected to fail" },
        { "defaultctx", OPT_USE_DEFAULTCTX, '-',
          "Use the default context if this is set" },
        { OPT_HELP_STR, 1, '-', "file\tProvider names to explicitly load\n" },
        { NULL }
    };
    return test_options;
}

static int calculate_digest(const EVP_MD *md, const char *msg, size_t len,
                            const unsigned char *exptd)
{
    unsigned char out[SHA256_DIGEST_LENGTH];
    EVP_MD_CTX *ctx;
    int ret = 0;

    if (!TEST_ptr(ctx = EVP_MD_CTX_new())
            || !TEST_true(EVP_DigestInit_ex(ctx, md, NULL))
            || !TEST_true(EVP_DigestUpdate(ctx, msg, len))
            || !TEST_true(EVP_DigestFinal_ex(ctx, out, NULL))
            || !TEST_mem_eq(out, SHA256_DIGEST_LENGTH, exptd,
                            SHA256_DIGEST_LENGTH)
            || !TEST_true(md == EVP_MD_CTX_get0_md(ctx)))
        goto err;

    ret = 1;
 err:
    EVP_MD_CTX_free(ctx);
    return ret;
}

static int load_providers(OSSL_LIB_CTX **libctx, OSSL_PROVIDER *prov[])
{
    OSSL_LIB_CTX *ctx = NULL;
    int ret = 0;
    size_t i;

    ctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(ctx))
        goto err;

    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, config_file)))
        goto err;
    if (test_get_argument_count() > 2)
        goto err;

    for (i = 0; i < test_get_argument_count(); ++i) {
        char *provname = test_get_argument(i);
        prov[i] = OSSL_PROVIDER_load(ctx, provname);
        if (!TEST_ptr(prov[i]))
            goto err;
    }

    ret = 1;
    *libctx = ctx;
err:
    if (ret == 0)
        OSSL_LIB_CTX_free(ctx);
    return ret;
}

static void unload_providers(OSSL_LIB_CTX **libctx, OSSL_PROVIDER *prov[])
{
    if (prov[0] != NULL)
        OSSL_PROVIDER_unload(prov[0]);
    if (prov[1] != NULL)
        OSSL_PROVIDER_unload(prov[1]);
    /* Not normally needed, but we would like to test that
     * OPENSSL_thread_stop_ex() behaves as expected.
     */
    if (libctx != NULL && *libctx != NULL) {
        OPENSSL_thread_stop_ex(*libctx);
        OSSL_LIB_CTX_free(*libctx);
    }
}

static int test_legacy_provider_unloaded(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    int rc = 0;

    ctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(ctx))
        goto err;

    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, config_file)))
        goto err;

    if (!TEST_int_eq(OSSL_PROVIDER_available(ctx, "legacy"), 0))
        goto err;

    rc = 1;
err:
    OSSL_LIB_CTX_free(ctx);
    return rc;
}

static X509_ALGOR *make_algor(int nid)
{
    X509_ALGOR *algor;

    if (!TEST_ptr(algor = X509_ALGOR_new())
        || !TEST_true(X509_ALGOR_set0(algor, OBJ_nid2obj(nid),
                                      V_ASN1_UNDEF, NULL))) {
        X509_ALGOR_free(algor);
        return NULL;
    }
    return algor;
}

/*
 * Test EVP_MD_fetch()
 */
static int test_md(const EVP_MD *md)
{
    const char testmsg[] = "Hello world";
    const unsigned char exptd[] = {
      0x27, 0x51, 0x8b, 0xa9, 0x68, 0x30, 0x11, 0xf6, 0xb3, 0x96, 0x07, 0x2c,
      0x05, 0xf6, 0x65, 0x6d, 0x04, 0xf5, 0xfb, 0xc3, 0x78, 0x7c, 0xf9, 0x24,
      0x90, 0xec, 0x60, 0x6e, 0x50, 0x92, 0xe3, 0x26
    };

    return TEST_ptr(md)
        && TEST_true(EVP_MD_is_a(md, "SHA256"))
        && TEST_true(calculate_digest(md, testmsg, sizeof(testmsg), exptd))
        && TEST_int_eq(EVP_MD_get_size(md), SHA256_DIGEST_LENGTH)
        && TEST_int_eq(EVP_MD_get_block_size(md), SHA256_CBLOCK);
}

static int test_implicit_EVP_MD_fetch(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    OSSL_PROVIDER *prov[2] = {NULL, NULL};
    int ret = 0;

    ret = (use_default_ctx == 0 || load_providers(&ctx, prov))
        && test_md(EVP_sha256());

    unload_providers(&ctx, prov);
    return ret;
}

static int test_explicit_EVP_MD_fetch(const char *id)
{
    OSSL_LIB_CTX *ctx = NULL;
    EVP_MD *md = NULL;
    OSSL_PROVIDER *prov[2] = {NULL, NULL};
    int ret = 0;

    if (use_default_ctx == 0 && !load_providers(&ctx, prov))
        goto err;

    md = EVP_MD_fetch(ctx, id, fetch_property);
    if (expected_fetch_result != 0) {
        if (!test_md(md))
            goto err;

        /* Also test EVP_MD_up_ref() while we're doing this */
        if (!TEST_true(EVP_MD_up_ref(md)))
            goto err;
        /* Ref count should now be 2. Release first one here */
        EVP_MD_free(md);
    } else {
        if (!TEST_ptr_null(md))
            goto err;
    }
    ret = 1;

 err:
    EVP_MD_free(md);
    unload_providers(&ctx, prov);
    return ret;
}

static int test_explicit_EVP_MD_fetch_by_name(void)
{
    return test_explicit_EVP_MD_fetch("SHA256");
}

/*
 * idx 0: Allow names from OBJ_obj2txt()
 * idx 1: Force an OID in text form from OBJ_obj2txt()
 */
static int test_explicit_EVP_MD_fetch_by_X509_ALGOR(int idx)
{
    int ret = 0;
    X509_ALGOR *algor = make_algor(NID_sha256);
    const ASN1_OBJECT *obj;
    char id[OSSL_MAX_NAME_SIZE] = { 0 };

    if (algor == NULL)
        return 0;

    X509_ALGOR_get0(&obj, NULL, NULL, algor);
    switch (idx) {
    case 0:
        if (!TEST_int_gt(OBJ_obj2txt(id, sizeof(id), obj, 0), 0))
            goto end;
        break;
    case 1:
        if (!TEST_int_gt(OBJ_obj2txt(id, sizeof(id), obj, 1), 0))
            goto end;
        break;
    }

    ret = test_explicit_EVP_MD_fetch(id);
 end:
    X509_ALGOR_free(algor);
    return ret;
}

/*
 * Test EVP_CIPHER_fetch()
 */
static int encrypt_decrypt(const EVP_CIPHER *cipher, const unsigned char *msg,
                           size_t len)
{
    int ret = 0, ctlen, ptlen;
    EVP_CIPHER_CTX *ctx = NULL;
    unsigned char key[128 / 8];
    unsigned char ct[64], pt[64];

    memset(key, 0, sizeof(key));
    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new())
            || !TEST_true(EVP_CipherInit_ex(ctx, cipher, NULL, key, NULL, 1))
            || !TEST_true(EVP_CipherUpdate(ctx, ct, &ctlen, msg, len))
            || !TEST_true(EVP_CipherFinal_ex(ctx, ct, &ctlen))
            || !TEST_true(EVP_CipherInit_ex(ctx, cipher, NULL, key, NULL, 0))
            || !TEST_true(EVP_CipherUpdate(ctx, pt, &ptlen, ct, ctlen))
            || !TEST_true(EVP_CipherFinal_ex(ctx, pt, &ptlen))
            || !TEST_mem_eq(pt, ptlen, msg, len))
        goto err;

    ret = 1;
err:
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int test_cipher(const EVP_CIPHER *cipher)
{
    const unsigned char testmsg[] = "Hello world";

    return TEST_ptr(cipher)
        && TEST_true(encrypt_decrypt(cipher, testmsg, sizeof(testmsg)));
}

static int test_implicit_EVP_CIPHER_fetch(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    OSSL_PROVIDER *prov[2] = {NULL, NULL};
    int ret = 0;

    ret = (use_default_ctx == 0 || load_providers(&ctx, prov))
        && test_cipher(EVP_aes_128_cbc());

    unload_providers(&ctx, prov);
    return ret;
}

static int test_explicit_EVP_CIPHER_fetch(const char *id)
{
    OSSL_LIB_CTX *ctx = NULL;
    EVP_CIPHER *cipher = NULL;
    OSSL_PROVIDER *prov[2] = {NULL, NULL};
    int ret = 0;

    if (use_default_ctx == 0 && !load_providers(&ctx, prov))
        goto err;

    cipher = EVP_CIPHER_fetch(ctx, id, fetch_property);
    if (expected_fetch_result != 0) {
        if (!test_cipher(cipher))
            goto err;

        if (!TEST_true(EVP_CIPHER_up_ref(cipher)))
            goto err;
        /* Ref count should now be 2. Release first one here */
        EVP_CIPHER_free(cipher);
    } else {
        if (!TEST_ptr_null(cipher))
            goto err;
    }
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    unload_providers(&ctx, prov);
    return ret;
}

static int test_explicit_EVP_CIPHER_fetch_by_name(void)
{
    return test_explicit_EVP_CIPHER_fetch("AES-128-CBC");
}

/*
 * idx 0: Allow names from OBJ_obj2txt()
 * idx 1: Force an OID in text form from OBJ_obj2txt()
 */
static int test_explicit_EVP_CIPHER_fetch_by_X509_ALGOR(int idx)
{
    int ret = 0;
    X509_ALGOR *algor = make_algor(NID_aes_128_cbc);
    const ASN1_OBJECT *obj;
    char id[OSSL_MAX_NAME_SIZE] = { 0 };

    if (algor == NULL)
        return 0;

    X509_ALGOR_get0(&obj, NULL, NULL, algor);
    switch (idx) {
    case 0:
        if (!TEST_int_gt(OBJ_obj2txt(id, sizeof(id), obj, 0), 0))
            goto end;
        break;
    case 1:
        if (!TEST_int_gt(OBJ_obj2txt(id, sizeof(id), obj, 1), 0))
            goto end;
        break;
    }

    ret = test_explicit_EVP_CIPHER_fetch(id);
 end:
    X509_ALGOR_free(algor);
    return ret;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_ALG_FETCH_TYPE:
            alg = opt_arg();
            break;
        case OPT_FETCH_PROPERTY:
            fetch_property = opt_arg();
            break;
        case OPT_FETCH_FAILURE:
            expected_fetch_result = 0;
            break;
        case OPT_USE_DEFAULTCTX:
            use_default_ctx = 1;
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }
    ADD_TEST(test_legacy_provider_unloaded);
    if (strcmp(alg, "digest") == 0) {
        ADD_TEST(test_implicit_EVP_MD_fetch);
        ADD_TEST(test_explicit_EVP_MD_fetch_by_name);
        ADD_ALL_TESTS_NOSUBTEST(test_explicit_EVP_MD_fetch_by_X509_ALGOR, 2);
    } else {
        ADD_TEST(test_implicit_EVP_CIPHER_fetch);
        ADD_TEST(test_explicit_EVP_CIPHER_fetch_by_name);
        ADD_ALL_TESTS_NOSUBTEST(test_explicit_EVP_CIPHER_fetch_by_X509_ALGOR, 2);
    }
    return 1;
}
