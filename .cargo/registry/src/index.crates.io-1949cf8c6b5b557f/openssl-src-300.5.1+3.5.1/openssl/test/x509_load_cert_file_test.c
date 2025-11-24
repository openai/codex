/*
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/err.h>
#include <openssl/x509_vfy.h>

#include "testutil.h"

static const char *chain;
static const char *crl;

static int test_load_cert_file(void)
{
    int ret = 0, i;
    X509_STORE *store = NULL;
    X509_LOOKUP *lookup = NULL;
    STACK_OF(X509) *certs = NULL;
    STACK_OF(X509_OBJECT) *objs = NULL;

    if (!TEST_ptr(store = X509_STORE_new())
        || !TEST_ptr(lookup = X509_STORE_add_lookup(store, X509_LOOKUP_file()))
        || !TEST_true(X509_load_cert_file(lookup, chain, X509_FILETYPE_PEM))
        || !TEST_ptr(certs = X509_STORE_get1_all_certs(store))
        || !TEST_int_eq(sk_X509_num(certs), 4)
        || !TEST_ptr(objs = X509_STORE_get1_objects(store))
        || !TEST_int_eq(sk_X509_OBJECT_num(objs), 4))
        goto err;

    for (i = 0; i < sk_X509_OBJECT_num(objs); i++) {
        const X509_OBJECT *obj = sk_X509_OBJECT_value(objs, i);
        if (!TEST_int_eq(X509_OBJECT_get_type(obj), X509_LU_X509))
            goto err;
    }

    if (crl != NULL && !TEST_true(X509_load_crl_file(lookup, crl, X509_FILETYPE_PEM)))
        goto err;

    ret = 1;

err:
    OSSL_STACK_OF_X509_free(certs);
    sk_X509_OBJECT_pop_free(objs, X509_OBJECT_free);
    X509_STORE_free(store);
    return ret;
}

OPT_TEST_DECLARE_USAGE("cert.pem [crl.pem]\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    chain = test_get_argument(0);
    if (chain == NULL)
        return 0;

    crl = test_get_argument(1);

    ADD_TEST(test_load_cert_file);
    return 1;
}
