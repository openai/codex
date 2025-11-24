/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/rsa.h>
#include <openssl/bn.h>
#include "crypto/rsa.h"
#include "testutil.h"

static OSSL_PROVIDER *prov_null = NULL;
static OSSL_LIB_CTX *libctx = NULL;

static int test_rsa_x931_keygen(void)
{
    int ret = 0;
    BIGNUM *e = NULL;
    RSA *rsa = NULL;

    ret = TEST_ptr(rsa = ossl_rsa_new_with_ctx(libctx))
          && TEST_ptr(e = BN_new())
          && TEST_int_eq(BN_set_word(e, RSA_F4), 1)
          && TEST_int_eq(RSA_X931_generate_key_ex(rsa, 1024, e, NULL), 1);
    BN_free(e);
    RSA_free(rsa);
    return ret;
}

int setup_tests(void)
{
    if (!test_get_libctx(&libctx, &prov_null, NULL, NULL, NULL))
        return 0;

    ADD_TEST(test_rsa_x931_keygen);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(prov_null);
    OSSL_LIB_CTX_free(libctx);
}
