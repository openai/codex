/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Regression tests for ASN.1 parsing bugs. */

#include <stdio.h>
#include <string.h>

#include "testutil.h"

#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "internal/nelem.h"

static const ASN1_ITEM *item_type;
static const char *test_file;

typedef enum {
    ASN1_UNKNOWN,
    ASN1_OK,
    ASN1_BIO,
    ASN1_DECODE,
    ASN1_ENCODE,
    ASN1_COMPARE
} expected_error_t;

typedef struct {
    const char *str;
    expected_error_t code;
} error_enum;

static expected_error_t expected_error = ASN1_UNKNOWN;

static int test_bad_asn1(void)
{
    BIO *bio = NULL;
    ASN1_VALUE *value = NULL;
    int ret = 0;
    unsigned char buf[2048];
    const unsigned char *buf_ptr = buf;
    unsigned char *der = NULL;
    int derlen;
    int len;

    bio = BIO_new_file(test_file, "r");
    if (!TEST_ptr(bio))
        return 0;

    if (expected_error == ASN1_BIO) {
        if (TEST_ptr_null(ASN1_item_d2i_bio(item_type, bio, NULL)))
            ret = 1;
        goto err;
    }

    /*
     * Unless we are testing it we don't use ASN1_item_d2i_bio because it
     * performs sanity checks on the input and can reject it before the
     * decoder is called.
     */
    len = BIO_read(bio, buf, sizeof(buf));
    if (!TEST_int_ge(len, 0))
        goto err;

    value = ASN1_item_d2i(NULL, &buf_ptr, len, item_type);
    if (value == NULL) {
        if (TEST_int_eq(expected_error, ASN1_DECODE))
            ret = 1;
        goto err;
    }

    derlen = ASN1_item_i2d(value, &der, item_type);

    if (der == NULL || derlen < 0) {
        if (TEST_int_eq(expected_error, ASN1_ENCODE))
            ret = 1;
        goto err;
    }

    if (derlen != len || memcmp(der, buf, derlen) != 0) {
        if (TEST_int_eq(expected_error, ASN1_COMPARE))
            ret = 1;
        goto err;
    }

    if (TEST_int_eq(expected_error, ASN1_OK))
        ret = 1;

 err:
    /* Don't indicate success for memory allocation errors */
    if (ret == 1
        && !TEST_false(ERR_GET_REASON(ERR_peek_error()) == ERR_R_MALLOC_FAILURE))
        ret = 0;
    BIO_free(bio);
    OPENSSL_free(der);
    ASN1_item_free(value, item_type);
    return ret;
}

OPT_TEST_DECLARE_USAGE("item_name expected_error test_file.der\n")

/*
 * Usage: d2i_test <name> <type> <file>, e.g.
 * d2i_test generalname bad_generalname.der
 */
int setup_tests(void)
{
    const char *test_type_name;
    const char *expected_error_string;

    size_t i;

    static error_enum expected_errors[] = {
        {"OK", ASN1_OK},
        {"BIO", ASN1_BIO},
        {"decode", ASN1_DECODE},
        {"encode", ASN1_ENCODE},
        {"compare", ASN1_COMPARE}
    };

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(test_type_name = test_get_argument(0))
            || !TEST_ptr(expected_error_string = test_get_argument(1))
            || !TEST_ptr(test_file = test_get_argument(2)))
        return 0;

    item_type = ASN1_ITEM_lookup(test_type_name);

    if (item_type == NULL) {
        TEST_error("Unknown type %s", test_type_name);
        TEST_note("Supported types:");
        for (i = 0;; i++) {
            const ASN1_ITEM *it = ASN1_ITEM_get(i);

            if (it == NULL)
                break;
            TEST_note("\t%s", it->sname);
        }
        return 0;
    }

    for (i = 0; i < OSSL_NELEM(expected_errors); i++) {
        if (strcmp(expected_errors[i].str, expected_error_string) == 0) {
            expected_error = expected_errors[i].code;
            break;
        }
    }

    if (expected_error == ASN1_UNKNOWN) {
        TEST_error("Unknown expected error %s\n", expected_error_string);
        return 0;
    }

    ADD_TEST(test_bad_asn1);
    return 1;
}
