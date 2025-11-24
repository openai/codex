/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/pem.h>
#include <openssl/x509.h>

#include "testutil.h"

static char *certsDir = NULL;

/*
 * Test for the missing X509 version check discussed in issue #5738 and
 * added in PR #24677.
 * This test tries to verify a malformed CSR with the X509 version set
 * version 6, instead of 1. As this request is malformed, even its
 * signature is valid, the verification must fail.
 */
static int test_x509_req_detect_invalid_version(void)
{
    char *certFilePath;
    BIO *bio = NULL;
    EVP_PKEY *pkey = NULL;
    X509_REQ *req = NULL;
    int ret = 0;

    certFilePath = test_mk_file_path(certsDir, "x509-req-detect-invalid-version.pem");
    if (certFilePath == NULL)
        goto err;
    if (!TEST_ptr(bio = BIO_new_file(certFilePath, "r")))
        goto err;
    req = PEM_read_bio_X509_REQ(bio, NULL, 0, NULL);
    if (req == NULL) {
        ret = 1; /* success, reading PEM with invalid CSR data is allowed to fail. */
        goto err;
    }
    if (!TEST_ptr(pkey = X509_REQ_get_pubkey(req)))
        goto err;
    /* Verification MUST fail at this point. ret != 1. */
    if (!TEST_int_ne(X509_REQ_verify(req, pkey), 1))
        goto err;
    ret = 1; /* success */
err:
    EVP_PKEY_free(pkey);
    X509_REQ_free(req);
    BIO_free(bio);
    OPENSSL_free(certFilePath);
    return ret;
}

OPT_TEST_DECLARE_USAGE("certdir\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }
    if (!TEST_ptr(certsDir = test_get_argument(0)))
        return 0;

    ADD_TEST(test_x509_req_detect_invalid_version);
    return 1;
}

void cleanup_tests(void)
{
}
