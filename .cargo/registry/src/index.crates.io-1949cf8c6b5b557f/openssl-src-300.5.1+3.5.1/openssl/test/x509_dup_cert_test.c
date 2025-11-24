/*
 * Copyright 2017-2020 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2017, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/err.h>
#include <openssl/x509_vfy.h>

#include "testutil.h"

static int test_509_dup_cert(int n)
{
    int ret = 0;
    X509_STORE *store = NULL;
    X509_LOOKUP *lookup = NULL;
    const char *cert_f = test_get_argument(n);

    if (TEST_ptr(store = X509_STORE_new())
        && TEST_ptr(lookup = X509_STORE_add_lookup(store, X509_LOOKUP_file()))
        && TEST_true(X509_load_cert_file(lookup, cert_f, X509_FILETYPE_PEM))
        && TEST_true(X509_load_cert_file(lookup, cert_f, X509_FILETYPE_PEM)))
        ret = 1;

    X509_STORE_free(store);
    return ret;
}

OPT_TEST_DECLARE_USAGE("cert.pem...\n")

int setup_tests(void)
{
    size_t n;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    n = test_get_argument_count();
    if (!TEST_int_gt(n, 0))
        return 0;

    ADD_ALL_TESTS(test_509_dup_cert, n);
    return 1;
}
