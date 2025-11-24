/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <limits.h>
#include <openssl/store.h>
#include <openssl/ui.h>
#include "testutil.h"

#ifndef PATH_MAX
# if defined(_WIN32) && defined(_MAX_PATH)
#  define PATH_MAX _MAX_PATH
# else
#  define PATH_MAX 4096
# endif
#endif

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_INPUTDIR,
    OPT_INFILE,
    OPT_SM2FILE,
    OPT_DATADIR,
    OPT_TEST_ENUM
} OPTION_CHOICE;

static const char *inputdir = NULL;
static const char *infile = NULL;
static const char *sm2file = NULL;
static const char *datadir = NULL;

static int test_store_open(void)
{
    int ret = 0;
    OSSL_STORE_CTX *sctx = NULL;
    OSSL_STORE_SEARCH *search = NULL;
    UI_METHOD *ui_method = NULL;
    char *input = test_mk_file_path(inputdir, infile);

    ret = TEST_ptr(input)
          && TEST_ptr(search = OSSL_STORE_SEARCH_by_alias("nothing"))
          && TEST_ptr(ui_method= UI_create_method("DummyUI"))
          && TEST_ptr(sctx = OSSL_STORE_open_ex(input, NULL, NULL, ui_method,
                                                NULL, NULL, NULL, NULL))
          && TEST_false(OSSL_STORE_find(sctx, NULL))
          && TEST_true(OSSL_STORE_find(sctx, search));
    UI_destroy_method(ui_method);
    OSSL_STORE_SEARCH_free(search);
    OSSL_STORE_close(sctx);
    OPENSSL_free(input);
    return ret;
}

#ifndef OPENSSL_NO_WINSTORE
/*
 * This is the one of the root certificate authorities from the 
 * microsoft cert store.  We use it to extract the subject name
 * so that we can search for it in the store
 */
static const unsigned char mscert[] = {
  0x30, 0x82, 0x01, 0x3b, 0x30, 0x81, 0xee, 0xa0, 0x03, 0x02, 0x01, 0x02,
  0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x30, 0x81,
  0x88, 0x31, 0x0b, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x06, 0x13, 0x02,
  0x55, 0x53, 0x31, 0x13, 0x30, 0x11, 0x06, 0x03, 0x55, 0x04, 0x08, 0x13,
  0x0a, 0x57, 0x61, 0x73, 0x68, 0x69, 0x6e, 0x67, 0x74, 0x6f, 0x6e, 0x31,
  0x10, 0x30, 0x0e, 0x06, 0x03, 0x55, 0x04, 0x07, 0x13, 0x07, 0x52, 0x65,
  0x64, 0x6d, 0x6f, 0x6e, 0x64, 0x31, 0x1e, 0x30, 0x1c, 0x06, 0x03, 0x55,
  0x04, 0x0a, 0x13, 0x15, 0x4d, 0x69, 0x63, 0x72, 0x6f, 0x73, 0x6f, 0x66,
  0x74, 0x20, 0x43, 0x6f, 0x72, 0x70, 0x6f, 0x72, 0x61, 0x74, 0x69, 0x6f,
  0x6e, 0x31, 0x32, 0x30, 0x30, 0x06, 0x03, 0x55, 0x04, 0x03, 0x13, 0x29,
  0x4d, 0x69, 0x63, 0x72, 0x6f, 0x73, 0x6f, 0x66, 0x74, 0x20, 0x52, 0x6f,
  0x6f, 0x74, 0x20, 0x43, 0x65, 0x72, 0x74, 0x69, 0x66, 0x69, 0x63, 0x61,
  0x74, 0x65, 0x20, 0x41, 0x75, 0x74, 0x68, 0x6f, 0x72, 0x69, 0x74, 0x79,
  0x20, 0x32, 0x30, 0x31, 0x31, 0x30, 0x20, 0x17, 0x0d, 0x32, 0x35, 0x30,
  0x34, 0x32, 0x39, 0x30, 0x35, 0x34, 0x33, 0x33, 0x30, 0x5a, 0x18, 0x0f,
  0x32, 0x31, 0x32, 0x35, 0x30, 0x34, 0x32, 0x39, 0x30, 0x35, 0x34, 0x33,
  0x33, 0x30, 0x5a, 0x30, 0x00, 0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b,
  0x65, 0x70, 0x03, 0x21, 0x00, 0x21, 0x86, 0x38, 0x28, 0x7d, 0xad, 0x35,
  0xad, 0xc3, 0x91, 0x07, 0x41, 0x65, 0x02, 0xe1, 0x72, 0x79, 0xf4, 0x0d,
  0xd5, 0xe2, 0xbb, 0xba, 0xd8, 0x81, 0x06, 0xad, 0xa0, 0x7b, 0x2a, 0x41,
  0x09, 0xa3, 0x02, 0x30, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70,
  0x03, 0x41, 0x00, 0x2e, 0xbb, 0xb9, 0x55, 0x59, 0x24, 0xb0, 0xdb, 0x18,
  0xd0, 0xb4, 0xa6, 0x8b, 0xa8, 0xbf, 0x4c, 0xbd, 0x83, 0x27, 0xe7, 0xc2,
  0xc8, 0x13, 0xc2, 0x81, 0xbe, 0xdf, 0x04, 0x7b, 0x09, 0x87, 0x08, 0x53,
  0x97, 0xd6, 0x7c, 0xb7, 0x6c, 0xa6, 0x78, 0x63, 0x3c, 0x27, 0x29, 0x81,
  0x85, 0xd0, 0x73, 0x92, 0x2d, 0x5c, 0x33, 0x8e, 0x88, 0x18, 0x9b, 0x1c,
  0xcf, 0xc3, 0x4a, 0xf0, 0x82, 0x05, 0x05
};
#define TEST_CERT_LEN 1521

static int test_store_open_winstore(void)
{
    int ret = 0;
    OSSL_STORE_CTX *sctx = NULL;
    OSSL_STORE_SEARCH *search = NULL;
    UI_METHOD *ui_method = NULL;
    OSSL_STORE_INFO *info = NULL;
    X509* testcert = NULL;
    const unsigned char* certptr = mscert;

    /*
     * Test the winstore, by opening it, searching for the MS root certificate
     * and ensure that it was found.  Note that we have to search by
     * subject name, as winstore only allows searches by that method
     */
    ret = TEST_ptr(testcert = d2i_X509(NULL, &certptr, TEST_CERT_LEN))
        && TEST_ptr(search = OSSL_STORE_SEARCH_by_name(X509_get_issuer_name(testcert)))
        && TEST_ptr(ui_method = UI_create_method("DummyUI"))
        && TEST_ptr(sctx = OSSL_STORE_open_ex("org.openssl.winstore:", NULL,
            NULL, ui_method, NULL, NULL,
            NULL, NULL))
        && TEST_true(OSSL_STORE_find(sctx, search))
        && TEST_ptr(info = OSSL_STORE_load(sctx));
    UI_destroy_method(ui_method);
    OSSL_STORE_INFO_free(info);
    OSSL_STORE_SEARCH_free(search);
    OSSL_STORE_close(sctx);
    X509_free(testcert);
    return ret;
}
#endif

static int test_store_search_by_key_fingerprint_fail(void)
{
    int ret;
    OSSL_STORE_SEARCH *search = NULL;

    ret = TEST_ptr_null(search = OSSL_STORE_SEARCH_by_key_fingerprint(
                                     EVP_sha256(), NULL, 0));
    OSSL_STORE_SEARCH_free(search);
    return ret;
}

static int get_params(const char *uri, const char *type)
{
    EVP_PKEY *pkey = NULL;
    OSSL_STORE_CTX *ctx = NULL;
    OSSL_STORE_INFO *info;
    int ret = 0;

    ctx = OSSL_STORE_open_ex(uri, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
    if (!TEST_ptr(ctx))
        goto err;

    while (!OSSL_STORE_eof(ctx)
            && (info = OSSL_STORE_load(ctx)) != NULL
            && pkey == NULL) {
        if (OSSL_STORE_INFO_get_type(info) == OSSL_STORE_INFO_PARAMS) {
            pkey = OSSL_STORE_INFO_get1_PARAMS(info);
        }
        OSSL_STORE_INFO_free(info);
        info = NULL;
    }

    if (pkey != NULL)
        ret = EVP_PKEY_is_a(pkey, type);
    EVP_PKEY_free(pkey);

 err:
    OSSL_STORE_close(ctx);
    return ret;
}

static int test_store_get_params(int idx)
{
    const char *type;
    const char *urifmt;
    char uri[PATH_MAX];

    switch (idx) {
#ifndef OPENSSL_NO_DH
    case 0:
        type = "DH";
        break;
    case 1:
        type = "DHX";
        break;
#else
    case 0:
    case 1:
        return 1;
#endif
    case 2:
#ifndef OPENSSL_NO_DSA
        type = "DSA";
        break;
#else
        return 1;
#endif
    default:
        TEST_error("Invalid test index");
        return 0;
    }

    urifmt = "%s/%s-params.pem";
#ifdef __VMS
    {
        char datadir_end = datadir[strlen(datadir) - 1];

        if (datadir_end == ':' || datadir_end == ']' || datadir_end == '>')
            urifmt = "%s%s-params.pem";
    }
#endif
    if (!TEST_true(BIO_snprintf(uri, sizeof(uri), urifmt, datadir, type)))
        return 0;

    TEST_info("Testing uri: %s", uri);
    if (!TEST_true(get_params(uri, type)))
        return 0;

    return 1;
}

/*
 * This test verifies that calling OSSL_STORE_ATTACH does not set an
 * "unregistered scheme" error when called.
 */
static int test_store_attach_unregistered_scheme(void)
{
    int ret;
    OSSL_STORE_CTX *store_ctx = NULL;
    OSSL_PROVIDER *provider = NULL;
    OSSL_LIB_CTX *libctx = NULL;
    BIO *bio = NULL;
    char *input = test_mk_file_path(inputdir, sm2file);

    ret = TEST_ptr(input)
          && TEST_ptr(libctx = OSSL_LIB_CTX_new())
          && TEST_ptr(provider = OSSL_PROVIDER_load(libctx, "default"))
          && TEST_ptr(bio = BIO_new_file(input, "r"))
          && TEST_ptr(store_ctx = OSSL_STORE_attach(bio, "file", libctx, NULL,
                                                    NULL, NULL, NULL, NULL, NULL))
          && TEST_int_ne(ERR_GET_LIB(ERR_peek_error()), ERR_LIB_OSSL_STORE)
          && TEST_int_ne(ERR_GET_REASON(ERR_peek_error()),
                         OSSL_STORE_R_UNREGISTERED_SCHEME);

    BIO_free(bio);
    OSSL_STORE_close(store_ctx);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
    OPENSSL_free(input);
    return ret;
}

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "dir", OPT_INPUTDIR, '/' },
        { "in", OPT_INFILE, '<' },
        { "sm2", OPT_SM2FILE, '<' },
        { "data", OPT_DATADIR, 's' },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_INPUTDIR:
            inputdir = opt_arg();
            break;
        case OPT_INFILE:
            infile = opt_arg();
            break;
        case OPT_SM2FILE:
            sm2file = opt_arg();
            break;
        case OPT_DATADIR:
            datadir = opt_arg();
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    if (datadir == NULL) {
        TEST_error("No data directory specified");
        return 0;
    }
    if (inputdir == NULL) {
        TEST_error("No input directory specified");
        return 0;
    }

    if (infile != NULL)
        ADD_TEST(test_store_open);
#ifndef OPENSSL_NO_WINSTORE
    ADD_TEST(test_store_open_winstore);
#endif
    ADD_TEST(test_store_search_by_key_fingerprint_fail);
    ADD_ALL_TESTS(test_store_get_params, 3);
    if (sm2file != NULL)
        ADD_TEST(test_store_attach_unregistered_scheme);
    return 1;
}
