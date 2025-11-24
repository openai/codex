/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include "testutil.h"

/*
 * Test that the default libctx does not get initialised when using a custom
 * libctx. We assume that this test application has been executed such that the
 * null provider is loaded via the config file.
 */
static int test_no_deflt_ctx_init(void)
{
    int testresult = 0;
    EVP_MD *md = NULL;
    OSSL_LIB_CTX *ctx = OSSL_LIB_CTX_new();

    if (!TEST_ptr(ctx))
        return 0;

    md = EVP_MD_fetch(ctx, "SHA2-256", NULL);
    if (!TEST_ptr(md))
        goto err;

    /*
     * Since we're using a non-default libctx above, the default libctx should
     * not have been initialised via config file, and so it is not too late to
     * use OPENSSL_INIT_NO_LOAD_CONFIG.
     */
    OPENSSL_init_crypto(OPENSSL_INIT_NO_LOAD_CONFIG, NULL);

    /*
     * If the config file was incorrectly loaded then the null provider will
     * have been initialised and the default provider loading will have been
     * blocked. If the config file was NOT loaded (as we expect) then the
     * default provider should be available.
     */
    if (!TEST_true(OSSL_PROVIDER_available(NULL, "default")))
        goto err;
    if (!TEST_false(OSSL_PROVIDER_available(NULL, "null")))
        goto err;

    testresult = 1;
 err:
    EVP_MD_free(md);
    OSSL_LIB_CTX_free(ctx);
    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_no_deflt_ctx_init);
    return 1;
}
