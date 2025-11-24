/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/pem.h>
#include <openssl/rsa.h>
#include <openssl/x509_acert.h>

#include "testutil.h"

static int test_print_acert(int idx)
{
    int ret = 0;
    const char *acert_file;
    X509_ACERT *acert = NULL;
    BIO *bp, *bout;

    if (!TEST_ptr(acert_file = test_get_argument(idx)))
        return 0;

    if (!TEST_ptr(bp = BIO_new_file(acert_file, "r")))
        return 0;

    if (!TEST_ptr(bout = BIO_new_fp(stderr, BIO_NOCLOSE)))
        goto err;

    if (!TEST_ptr(acert = PEM_read_bio_X509_ACERT(bp, NULL, NULL, NULL)))
        goto err;

    if (!TEST_int_eq(X509_ACERT_print(bout, acert), 1)) {
        goto err;
    }

    ret = 1;

err:
    BIO_free(bp);
    BIO_free(bout);
    X509_ACERT_free(acert);
    return ret;
}

static int test_acert_sign(void)
{
    int ret = 0;
    const char *acert_file;
    EVP_PKEY *pkey;
    BIO *bp = NULL;
    X509_ACERT *acert = NULL;

    if (!TEST_ptr(acert_file = test_get_argument(0)))
        return 0;

    if (!TEST_ptr(pkey = EVP_RSA_gen(2048)))
        return 0;

    if (!TEST_ptr(bp = BIO_new_file(acert_file, "r")))
        goto err;

    if (!TEST_ptr(acert = PEM_read_bio_X509_ACERT(bp, NULL, NULL, NULL)))
        goto err;

    if (!TEST_int_gt(X509_ACERT_sign(acert, pkey, EVP_sha256()), 0) ||
        !TEST_int_eq(X509_ACERT_verify(acert, pkey), 1))
        goto err;

    ret = 1;

err:
    BIO_free(bp);
    X509_ACERT_free(acert);
    EVP_PKEY_free(pkey);
    return ret;
}

/* IetfAttrSyntax structure with one value */
static const unsigned char attr_syntax_single[] = {
  0x30, 0x15, 0xa0, 0x09, 0x86, 0x07, 0x54, 0x65, 0x73, 0x74, 0x76, 0x61,
  0x6c, 0x30, 0x08, 0x0c, 0x06, 0x67, 0x72, 0x6f, 0x75, 0x70, 0x31
};

/* IetfAttrSyntax structure with multiple values of the same type */
static const unsigned char attr_syntax_multiple[] = {
  0x30, 0x1d, 0x30, 0x1b, 0x0c, 0x07, 0x67, 0x72, 0x6f, 0x75, 0x70, 0x20,
  0x31, 0x0c, 0x07, 0x67, 0x72, 0x6f, 0x75, 0x70, 0x20, 0x32, 0x0c, 0x07,
  0x67, 0x72, 0x6f, 0x75, 0x70, 0x20, 0x33
};

/* IetfAttrSyntax structure with multiple values of different types */
static const unsigned char attr_syntax_diff_type[] = {
  0x30, 0x11, 0x30, 0x0f, 0x04, 0x08, 0x64, 0x65, 0x61, 0x64, 0x63, 0x6f,
  0x64, 0x65, 0x0c, 0x03, 0x61, 0x61, 0x61
};

/* IetfAttrSyntax structure with an invalid/unsupported value type */
static const unsigned char attr_syntax_invalid_type[] = {
  0x30, 0x05, 0x30, 0x03, 0x02, 0x01, 0x0a
};

#define ADD_TEST_DATA(x, valid) {x, sizeof(x), valid}

struct ietf_type_test_data {
    const unsigned char *data;
    size_t len;
    int valid;
};

static const struct ietf_type_test_data ietf_syntax_tests[] = {
    ADD_TEST_DATA(attr_syntax_single, 1),
    ADD_TEST_DATA(attr_syntax_multiple, 1),
    ADD_TEST_DATA(attr_syntax_diff_type, 0),
    ADD_TEST_DATA(attr_syntax_invalid_type, 0),
};

static int test_object_group_attr(int idx)
{
    int ret = 0;
    OSSL_IETF_ATTR_SYNTAX *ias = NULL;
    BIO *bout = NULL;
    const unsigned char *p;
    const struct ietf_type_test_data *test = &ietf_syntax_tests[idx];

    if (!TEST_ptr(bout = BIO_new_fp(stderr, BIO_NOCLOSE)))
        goto done;

    p = test->data;

    ias = d2i_OSSL_IETF_ATTR_SYNTAX(NULL, &p, test->len);

    if ((test->valid && !TEST_ptr(ias))
            || (!test->valid && !TEST_ptr_null(ias)))
        goto done;

    if (ias != NULL
            && !TEST_int_eq(OSSL_IETF_ATTR_SYNTAX_print(bout, ias, 4), 1)) {
        OSSL_IETF_ATTR_SYNTAX_free(ias);
        goto done;
    }

    ret = 1;

done:
    OSSL_IETF_ATTR_SYNTAX_free(ias);
    BIO_free(bout);
    return ret;
}

OPT_TEST_DECLARE_USAGE("[<attribute certs (PEM)>...]\n")
int setup_tests(void)
{
    int cnt;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    cnt = test_get_argument_count();
    if (cnt < 1) {
        TEST_error("Must specify at least 1 attribute certificate file\n");
        return 0;
    }

    ADD_ALL_TESTS(test_print_acert, cnt);
    ADD_TEST(test_acert_sign);
    ADD_ALL_TESTS(test_object_group_attr, OSSL_NELEM(ietf_syntax_tests));

    return 1;
}
