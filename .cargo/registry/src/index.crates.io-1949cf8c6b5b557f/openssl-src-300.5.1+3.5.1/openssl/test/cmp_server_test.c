/*
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2020
 * Copyright Siemens AG 2015-2020
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

typedef struct test_fixture {
    const char *test_case_name;
    int expected;
    OSSL_CMP_SRV_CTX *srv_ctx;
    OSSL_CMP_MSG *req;
} CMP_SRV_TEST_FIXTURE;

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *default_null_provider = NULL, *provider = NULL;
static OSSL_CMP_MSG *request = NULL;

static void tear_down(CMP_SRV_TEST_FIXTURE *fixture)
{
    OSSL_CMP_SRV_CTX_free(fixture->srv_ctx);
    OPENSSL_free(fixture);
}

static CMP_SRV_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_SRV_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->srv_ctx = OSSL_CMP_SRV_CTX_new(libctx, NULL)))
        goto err;
    return fixture;

 err:
    tear_down(fixture);
    return NULL;
}

static int dummy_errorCode = CMP_R_MULTIPLE_SAN_SOURCES; /* any reason code */

static OSSL_CMP_PKISI *process_cert_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                            const OSSL_CMP_MSG *cert_req,
                                            int certReqId,
                                            const OSSL_CRMF_MSG *crm,
                                            const X509_REQ *p10cr,
                                            X509 **certOut,
                                            STACK_OF(X509) **chainOut,
                                            STACK_OF(X509) **caPubs)
{
    ERR_raise(ERR_LIB_CMP, dummy_errorCode);
    return NULL;
}

static int execute_test_handle_request(CMP_SRV_TEST_FIXTURE *fixture)
{
    OSSL_CMP_SRV_CTX *ctx = fixture->srv_ctx;
    OSSL_CMP_CTX *client_ctx;
    OSSL_CMP_CTX *cmp_ctx;
    char *dummy_custom_ctx = "@test_dummy", *custom_ctx;
    OSSL_CMP_MSG *rsp = NULL;
    OSSL_CMP_ERRORMSGCONTENT *errorContent;
    int res = 0;

    if (!TEST_ptr(client_ctx = OSSL_CMP_CTX_new(libctx, NULL))
            || !TEST_true(OSSL_CMP_CTX_set_transfer_cb_arg(client_ctx, ctx)))
        goto end;

    if (!TEST_true(OSSL_CMP_SRV_CTX_init(ctx, dummy_custom_ctx,
                                         process_cert_request, NULL, NULL,
                                         NULL, NULL, NULL))
        || !TEST_true(OSSL_CMP_SRV_CTX_init_trans(ctx, NULL, NULL))
        || !TEST_ptr(custom_ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(ctx))
        || !TEST_int_eq(strcmp(custom_ctx, dummy_custom_ctx), 0))
        goto end;

    if (!TEST_true(OSSL_CMP_SRV_CTX_set_send_unprotected_errors(ctx, 0))
            || !TEST_true(OSSL_CMP_SRV_CTX_set_accept_unprotected(ctx, 0))
            || !TEST_true(OSSL_CMP_SRV_CTX_set_accept_raverified(ctx, 1))
            || !TEST_true(OSSL_CMP_SRV_CTX_set_grant_implicit_confirm(ctx, 1)))
        goto end;

    if (!TEST_ptr(cmp_ctx = OSSL_CMP_SRV_CTX_get0_cmp_ctx(ctx))
            || !OSSL_CMP_CTX_set1_referenceValue(cmp_ctx,
                                                 (unsigned char *)"server", 6)
            || !OSSL_CMP_CTX_set1_secretValue(cmp_ctx,
                                              (unsigned char *)"1234", 4))
        goto end;

    if (!TEST_ptr(rsp = OSSL_CMP_CTX_server_perform(client_ctx, fixture->req))
            || !TEST_int_eq(OSSL_CMP_MSG_get_bodytype(rsp),
                            OSSL_CMP_PKIBODY_ERROR)
            || !TEST_ptr(errorContent = rsp->body->value.error)
            || !TEST_int_eq(ASN1_INTEGER_get(errorContent->errorCode),
                            ERR_PACK(ERR_LIB_CMP, 0, dummy_errorCode)))
        goto end;

    res = 1;

 end:
    OSSL_CMP_MSG_free(rsp);
    OSSL_CMP_CTX_free(client_ctx);
    return res;
}

static int test_handle_request(void)
{
    SETUP_TEST_FIXTURE(CMP_SRV_TEST_FIXTURE, set_up);
    fixture->req = request;
    fixture->expected = 1;
    EXECUTE_TEST(execute_test_handle_request, tear_down);
    return result;
}

void cleanup_tests(void)
{
    OSSL_CMP_MSG_free(request);
    OSSL_PROVIDER_unload(default_null_provider);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
    return;
}

#define USAGE \
    "CR_protected_PBM_1234.der module_name [module_conf_file]\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    const char *request_f;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(request_f = test_get_argument(0))) {
        TEST_error("usage: cmp_server_test %s", USAGE);
        return 0;
    }

    if (!test_arg_libctx(&libctx, &default_null_provider, &provider, 1, USAGE))
        return 0;

    if (!TEST_ptr(request = load_pkimsg(request_f, libctx))) {
        cleanup_tests();
        return 0;
    }

    /*
     * this (indirectly) calls
     * OSSL_CMP_SRV_CTX_new(),
     * OSSL_CMP_SRV_CTX_free(),
     * OSSL_CMP_CTX_server_perform(),
     * OSSL_CMP_SRV_process_request(),
     * OSSL_CMP_SRV_CTX_init(),
     * OSSL_CMP_SRV_CTX_get0_cmp_ctx(),
     * OSSL_CMP_SRV_CTX_get0_custom_ctx(),
     * OSSL_CMP_SRV_CTX_set_send_unprotected_errors(),
     * OSSL_CMP_SRV_CTX_set_accept_unprotected(),
     * OSSL_CMP_SRV_CTX_set_accept_raverified(), and
     * OSSL_CMP_SRV_CTX_set_grant_implicit_confirm()
     */
    ADD_TEST(test_handle_request);
    return 1;
}
