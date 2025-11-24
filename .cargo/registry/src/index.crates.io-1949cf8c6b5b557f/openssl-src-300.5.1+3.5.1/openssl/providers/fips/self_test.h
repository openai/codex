/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/types.h>
#include <openssl/self_test.h>

typedef struct self_test_post_params_st {
    /* FIPS module integrity check parameters */
    const char *module_filename;            /* Module file to perform MAC on */
    const char *module_checksum_data;       /* Expected module MAC integrity */

    /* Used for continuous tests */
    const char *conditional_error_check;

    /* BIO callbacks supplied to the FIPS provider */
    OSSL_FUNC_BIO_new_file_fn *bio_new_file_cb;
    OSSL_FUNC_BIO_new_membuf_fn *bio_new_buffer_cb;
    OSSL_FUNC_BIO_read_ex_fn *bio_read_ex_cb;
    OSSL_FUNC_BIO_free_fn *bio_free_cb;
    OSSL_CALLBACK *cb;
    void *cb_arg;
    OSSL_LIB_CTX *libctx;

} SELF_TEST_POST_PARAMS;

int SELF_TEST_post(SELF_TEST_POST_PARAMS *st, int on_demand_test);
int SELF_TEST_kats(OSSL_SELF_TEST *event, OSSL_LIB_CTX *libctx);

void SELF_TEST_disable_conditional_error_state(void);
