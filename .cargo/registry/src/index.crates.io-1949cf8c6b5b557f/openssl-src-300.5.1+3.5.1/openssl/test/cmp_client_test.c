/*
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

#include "cmp_mock_srv.h"

static const char *server_key_f;
static const char *server_cert_f;
static const char *client_key_f;
static const char *client_cert_f;
static const char *pkcs10_f;

typedef struct test_fixture {
    const char *test_case_name;
    OSSL_CMP_CTX *cmp_ctx;
    OSSL_CMP_SRV_CTX *srv_ctx;
    int req_type;
    int expected;
    STACK_OF(X509) *caPubs;
} CMP_SES_TEST_FIXTURE;

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *default_null_provider = NULL, *provider = NULL;

static EVP_PKEY *server_key = NULL;
static X509 *server_cert = NULL;
static EVP_PKEY *client_key = NULL;
static X509 *client_cert = NULL;
static unsigned char ref[CMP_TEST_REFVALUE_LENGTH];

/*
 * For these unit tests, the client abandons message protection, and for
 * error messages the mock server does so as well.
 * Message protection and verification is tested in cmp_lib_test.c
 */

static void tear_down(CMP_SES_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX_free(fixture->cmp_ctx);
    ossl_cmp_mock_srv_free(fixture->srv_ctx);
    sk_X509_free(fixture->caPubs);
    OPENSSL_free(fixture);
}

static CMP_SES_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_SES_TEST_FIXTURE *fixture;
    OSSL_CMP_CTX *srv_cmp_ctx = NULL;
    OSSL_CMP_CTX *ctx = NULL; /* for client */

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->srv_ctx = ossl_cmp_mock_srv_new(libctx, NULL))
            || !OSSL_CMP_SRV_CTX_set_accept_unprotected(fixture->srv_ctx, 1)
            || !ossl_cmp_mock_srv_set1_refCert(fixture->srv_ctx, client_cert)
            || !ossl_cmp_mock_srv_set1_certOut(fixture->srv_ctx, client_cert)
            || (srv_cmp_ctx =
                OSSL_CMP_SRV_CTX_get0_cmp_ctx(fixture->srv_ctx)) == NULL
            || !OSSL_CMP_CTX_set1_cert(srv_cmp_ctx, server_cert)
            || !OSSL_CMP_CTX_set1_pkey(srv_cmp_ctx, server_key))
        goto err;
    if (!TEST_ptr(fixture->cmp_ctx = ctx = OSSL_CMP_CTX_new(libctx, NULL))
            || !OSSL_CMP_CTX_set_log_cb(fixture->cmp_ctx, print_to_bio_out)
            || !OSSL_CMP_CTX_set_transfer_cb(ctx, OSSL_CMP_CTX_server_perform)
            || !OSSL_CMP_CTX_set_transfer_cb_arg(ctx, fixture->srv_ctx)
            || !OSSL_CMP_CTX_set_option(ctx, OSSL_CMP_OPT_UNPROTECTED_SEND, 1)
            || !OSSL_CMP_CTX_set_option(ctx, OSSL_CMP_OPT_UNPROTECTED_ERRORS, 1)
            || !OSSL_CMP_CTX_set1_oldCert(ctx, client_cert)
            || !OSSL_CMP_CTX_set1_pkey(ctx, client_key)
            /* client_key is by default used also for newPkey */
            || !OSSL_CMP_CTX_set1_srvCert(ctx, server_cert)
            || !OSSL_CMP_CTX_set1_referenceValue(ctx, ref, sizeof(ref)))
        goto err;
    fixture->req_type = -1;
    return fixture;

 err:
    tear_down(fixture);
    return NULL;
}

static int execute_exec_RR_ses_test(CMP_SES_TEST_FIXTURE *fixt)
{
    return TEST_int_eq(OSSL_CMP_CTX_get_status(fixt->cmp_ctx),
                       OSSL_CMP_PKISTATUS_unspecified)
        && TEST_int_eq(OSSL_CMP_exec_RR_ses(fixt->cmp_ctx),
                       fixt->expected == OSSL_CMP_PKISTATUS_accepted)
        && TEST_int_eq(OSSL_CMP_CTX_get_status(fixt->cmp_ctx), fixt->expected);
}

static int execute_exec_GENM_ses_test_single(CMP_SES_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    ASN1_OBJECT *type = OBJ_txt2obj("1.3.6.1.5.5.7.4.2", 1);
    OSSL_CMP_ITAV *itav = OSSL_CMP_ITAV_create(type, NULL);
    STACK_OF(OSSL_CMP_ITAV) *itavs;

    OSSL_CMP_CTX_push0_genm_ITAV(ctx, itav);
    itavs = OSSL_CMP_exec_GENM_ses(ctx);

    sk_OSSL_CMP_ITAV_pop_free(itavs, OSSL_CMP_ITAV_free);
    return TEST_int_eq(OSSL_CMP_CTX_get_status(ctx), fixture->expected)
        && fixture->expected == OSSL_CMP_PKISTATUS_accepted ?
        TEST_ptr(itavs) : TEST_ptr_null(itavs);
}

static int execute_exec_GENM_ses_test(CMP_SES_TEST_FIXTURE *fixture)
{
    return execute_exec_GENM_ses_test_single(fixture)
        && OSSL_CMP_CTX_reinit(fixture->cmp_ctx)
        && execute_exec_GENM_ses_test_single(fixture);
}

static int execute_exec_certrequest_ses_test(CMP_SES_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    X509 *res = OSSL_CMP_exec_certreq(ctx, fixture->req_type, NULL);
    int status = OSSL_CMP_CTX_get_status(ctx);

    OSSL_CMP_CTX_print_errors(ctx);
    if (!TEST_int_eq(status, fixture->expected)
        && !(fixture->expected == OSSL_CMP_PKISTATUS_waiting
             && TEST_int_eq(status, OSSL_CMP_PKISTATUS_trans)))
        return 0;
    if (fixture->expected != OSSL_CMP_PKISTATUS_accepted)
        return TEST_ptr_null(res);

    if (!TEST_ptr(res) || !TEST_int_eq(X509_cmp(res, client_cert), 0))
        return 0;
    if (fixture->caPubs != NULL) {
        STACK_OF(X509) *caPubs = OSSL_CMP_CTX_get1_caPubs(fixture->cmp_ctx);
        int ret = TEST_int_eq(STACK_OF_X509_cmp(fixture->caPubs, caPubs), 0);

        OSSL_STACK_OF_X509_free(caPubs);
        return ret;
    }
    return 1;
}

static int test_exec_RR_ses(int request_error)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    if (request_error)
        OSSL_CMP_CTX_set1_oldCert(fixture->cmp_ctx, NULL);
    fixture->expected = request_error ? OSSL_CMP_PKISTATUS_request
        : OSSL_CMP_PKISTATUS_accepted;
    EXECUTE_TEST(execute_exec_RR_ses_test, tear_down);
    return result;
}

static int test_exec_RR_ses_ok(void)
{
    return test_exec_RR_ses(0);
}

static int test_exec_RR_ses_request_error(void)
{
    return test_exec_RR_ses(1);
}

static int test_exec_RR_ses_receive_error(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    ossl_cmp_mock_srv_set_statusInfo(fixture->srv_ctx,
                                     OSSL_CMP_PKISTATUS_rejection,
                                     OSSL_CMP_CTX_FAILINFO_signerNotTrusted,
                                     "test string");
    ossl_cmp_mock_srv_set_sendError(fixture->srv_ctx, OSSL_CMP_PKIBODY_RR);
    fixture->expected = OSSL_CMP_PKISTATUS_rejection;
    EXECUTE_TEST(execute_exec_RR_ses_test, tear_down);
    return result;
}

static int test_exec_IR_ses(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->req_type = OSSL_CMP_PKIBODY_IR;
    fixture->expected = OSSL_CMP_PKISTATUS_accepted;
    fixture->caPubs = sk_X509_new_null();
    if (!sk_X509_push(fixture->caPubs, server_cert)
        || !sk_X509_push(fixture->caPubs, server_cert)) {
        tear_down(fixture);
        return 0;
    }
    ossl_cmp_mock_srv_set1_caPubsOut(fixture->srv_ctx, fixture->caPubs);
    EXECUTE_TEST(execute_exec_certrequest_ses_test, tear_down);
    return result;
}

static int test_exec_REQ_ses_poll(int req_type, int check_after,
                                  int poll_count, int total_timeout,
                                  int expect)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->req_type = req_type;
    fixture->expected = expect;
    ossl_cmp_mock_srv_set_checkAfterTime(fixture->srv_ctx, check_after);
    ossl_cmp_mock_srv_set_pollCount(fixture->srv_ctx, poll_count);
    OSSL_CMP_CTX_set_option(fixture->cmp_ctx,
                            OSSL_CMP_OPT_TOTAL_TIMEOUT, total_timeout);

    if (req_type == OSSL_CMP_PKIBODY_IR) {
        EXECUTE_TEST(execute_exec_certrequest_ses_test, tear_down);
    } else if (req_type == OSSL_CMP_PKIBODY_GENM) {
        EXECUTE_TEST(execute_exec_GENM_ses_test, tear_down);
    }
    return result;
}

static int checkAfter = 1;
static int test_exec_IR_ses_poll_ok(void)
{
    return test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_IR, checkAfter, 2, 0,
                                  OSSL_CMP_PKISTATUS_accepted);
}

static int test_exec_IR_ses_poll_no_timeout(void)
{
    return test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_IR, checkAfter,
                                  2 /* pollCount */,
                                  checkAfter + 14, /* usually 4 is sufficient */
                                  OSSL_CMP_PKISTATUS_accepted);
}

static int test_exec_IR_ses_poll_total_timeout(void)
{
    return !test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_IR, checkAfter + 1,
                                   3 /* pollCount */, checkAfter + 6,
                                   OSSL_CMP_PKISTATUS_waiting);
}

static int test_exec_CR_ses(int implicit_confirm, int granted, int reject)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->req_type = OSSL_CMP_PKIBODY_CR;
    OSSL_CMP_CTX_set_option(fixture->cmp_ctx,
                            OSSL_CMP_OPT_IMPLICIT_CONFIRM, implicit_confirm);
    OSSL_CMP_SRV_CTX_set_grant_implicit_confirm(fixture->srv_ctx, granted);
    ossl_cmp_mock_srv_set_sendError(fixture->srv_ctx,
                                    reject ? OSSL_CMP_PKIBODY_CERTCONF : -1);
    fixture->expected = reject ? OSSL_CMP_PKISTATUS_rejection
        : OSSL_CMP_PKISTATUS_accepted;
    EXECUTE_TEST(execute_exec_certrequest_ses_test, tear_down);
    return result;
}

static int test_exec_CR_ses_explicit_confirm(void)
{
    return test_exec_CR_ses(0, 0, 0)
        && test_exec_CR_ses(0, 0, 1 /* reject */);
}

static int test_exec_CR_ses_implicit_confirm(void)
{
    return test_exec_CR_ses(1, 0, 0)
        && test_exec_CR_ses(1, 1 /* granted */, 0);
}

static int test_exec_KUR_ses(int transfer_error, int pubkey, int raverified)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->req_type = OSSL_CMP_PKIBODY_KUR;
    /* ctx->oldCert has already been set */

    if (transfer_error)
        OSSL_CMP_CTX_set_transfer_cb_arg(fixture->cmp_ctx, NULL);
    if (pubkey) {
        EVP_PKEY *key = raverified /* wrong key */ ? server_key : client_key;

        if (!EVP_PKEY_up_ref(key))
            return 0;

        OSSL_CMP_CTX_set0_newPkey(fixture->cmp_ctx, 0 /* not priv */, key);
        OSSL_CMP_SRV_CTX_set_accept_raverified(fixture->srv_ctx, 1);
    }
    if (pubkey || raverified)
        OSSL_CMP_CTX_set_option(fixture->cmp_ctx, OSSL_CMP_OPT_POPO_METHOD,
                                OSSL_CRMF_POPO_RAVERIFIED);
    fixture->expected = transfer_error ? OSSL_CMP_PKISTATUS_trans :
        raverified ? OSSL_CMP_PKISTATUS_rejection : OSSL_CMP_PKISTATUS_accepted;
    EXECUTE_TEST(execute_exec_certrequest_ses_test, tear_down);
    return result;
}

static int test_exec_KUR_ses_ok(void)
{
    return test_exec_KUR_ses(0, 0, 0);
}

static int test_exec_KUR_ses_transfer_error(void)
{
    return test_exec_KUR_ses(1, 0, 0);
}

static int test_exec_KUR_ses_wrong_popo(void)
{
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION /* cf ossl_cmp_verify_popo() */
    return test_exec_KUR_ses(0, 0, 1);
#else
    return 1;
#endif
}

static int test_exec_KUR_ses_pub(void)
{
    return test_exec_KUR_ses(0, 1, 0);
}

static int test_exec_KUR_ses_wrong_pub(void)
{
    return test_exec_KUR_ses(0, 1, 1);
}

static int test_certConf_cb(OSSL_CMP_CTX *ctx, X509 *cert, int fail_info,
                            const char **txt)
{
    int *reject = OSSL_CMP_CTX_get_certConf_cb_arg(ctx);

    if (*reject) {
        *txt = "not to my taste";
        fail_info = OSSL_CMP_PKIFAILUREINFO_badCertTemplate;
    }
    return fail_info;
}

static int test_exec_P10CR_ses(int reject)
{
    OSSL_CMP_CTX *ctx;
    X509_REQ *csr = NULL;

    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->req_type = OSSL_CMP_PKIBODY_P10CR;
    fixture->expected = reject ? OSSL_CMP_PKISTATUS_rejection
        : OSSL_CMP_PKISTATUS_accepted;
    ctx = fixture->cmp_ctx;
    if (!TEST_ptr(csr = load_csr_der(pkcs10_f, libctx))
        || !TEST_true(OSSL_CMP_CTX_set1_p10CSR(ctx, csr))
        || !TEST_true(OSSL_CMP_CTX_set_certConf_cb(ctx, test_certConf_cb))
        || !TEST_true(OSSL_CMP_CTX_set_certConf_cb_arg(ctx, &reject))) {
        tear_down(fixture);
        fixture = NULL;
    }
    X509_REQ_free(csr);
    EXECUTE_TEST(execute_exec_certrequest_ses_test, tear_down);
    return result;
}

static int test_exec_P10CR_ses_ok(void)
{
    return test_exec_P10CR_ses(0);
}

static int test_exec_P10CR_ses_reject(void)
{
    return test_exec_P10CR_ses(1);
}

static int execute_try_certreq_poll_test(CMP_SES_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    int check_after;
    const int CHECK_AFTER = 0;
    const int TYPE = OSSL_CMP_PKIBODY_KUR;

    ossl_cmp_mock_srv_set_pollCount(fixture->srv_ctx, 3);
    ossl_cmp_mock_srv_set_checkAfterTime(fixture->srv_ctx, CHECK_AFTER);
    return TEST_int_eq(-1, OSSL_CMP_try_certreq(ctx, TYPE, NULL, &check_after))
        && check_after == CHECK_AFTER
        && TEST_ptr_eq(OSSL_CMP_CTX_get0_newCert(ctx), NULL)
        && TEST_int_eq(-1, OSSL_CMP_try_certreq(ctx, TYPE, NULL, &check_after))
        && check_after == CHECK_AFTER
        && TEST_ptr_eq(OSSL_CMP_CTX_get0_newCert(ctx), NULL)
        && TEST_int_eq(fixture->expected,
                       OSSL_CMP_try_certreq(ctx, TYPE, NULL, NULL))
        && TEST_int_eq(0,
                       X509_cmp(OSSL_CMP_CTX_get0_newCert(ctx), client_cert));
}

static int test_try_certreq_poll(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_try_certreq_poll_test, tear_down);
    return result;
}

static int execute_try_certreq_poll_abort_test(CMP_SES_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    int check_after;
    const int CHECK_AFTER = 99;
    const int TYPE = OSSL_CMP_PKIBODY_CR;

    ossl_cmp_mock_srv_set_pollCount(fixture->srv_ctx, 3);
    ossl_cmp_mock_srv_set_checkAfterTime(fixture->srv_ctx, CHECK_AFTER);
    return TEST_int_eq(-1, OSSL_CMP_try_certreq(ctx, TYPE, NULL, &check_after))
        && check_after == CHECK_AFTER
        && TEST_ptr_eq(OSSL_CMP_CTX_get0_newCert(ctx), NULL)
        && TEST_int_eq(fixture->expected,
                       OSSL_CMP_try_certreq(ctx, -1 /* abort */, NULL, NULL))
        && TEST_ptr_eq(OSSL_CMP_CTX_get0_newCert(fixture->cmp_ctx), NULL);
}

static int test_try_certreq_poll_abort(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_try_certreq_poll_abort_test, tear_down);
    return result;
}

static int test_exec_GENM_ses_poll_ok(void)
{
    return test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_GENM, checkAfter, 2, 0,
                                  OSSL_CMP_PKISTATUS_accepted);
}

static int test_exec_GENM_ses_poll_no_timeout(void)
{
    return test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_GENM, checkAfter,
                                  1 /* pollCount */, checkAfter + 1,
                                  OSSL_CMP_PKISTATUS_accepted);
}

static int test_exec_GENM_ses_poll_total_timeout(void)
{
    return test_exec_REQ_ses_poll(OSSL_CMP_PKIBODY_GENM, checkAfter + 1,
                                  3 /* pollCount */, checkAfter + 2,
                                  OSSL_CMP_PKISTATUS_waiting);
}

static int test_exec_GENM_ses(int transfer_error, int total_timeout, int expect)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    if (transfer_error)
        OSSL_CMP_CTX_set_transfer_cb_arg(fixture->cmp_ctx, NULL);
    /*
     * cannot use OSSL_CMP_CTX_set_option(... OSSL_CMP_OPT_TOTAL_TIMEOUT)
     * here because this will correct total_timeout to be >= 0
     */
    fixture->cmp_ctx->total_timeout = total_timeout;
    fixture->expected = expect;
    EXECUTE_TEST(execute_exec_GENM_ses_test, tear_down);
    return result;
}

static int test_exec_GENM_ses_ok(void)
{
    return test_exec_GENM_ses(0, 0, OSSL_CMP_PKISTATUS_accepted);
}

static int test_exec_GENM_ses_transfer_error(void)
{
    return test_exec_GENM_ses(1, 0, OSSL_CMP_PKISTATUS_trans);
}

static int test_exec_GENM_ses_total_timeout(void)
{
    return test_exec_GENM_ses(0, -1, OSSL_CMP_PKISTATUS_trans);
}

static int execute_exchange_certConf_test(CMP_SES_TEST_FIXTURE *fixture)
{
    int res =
        ossl_cmp_exchange_certConf(fixture->cmp_ctx, OSSL_CMP_CERTREQID,
                                   OSSL_CMP_PKIFAILUREINFO_addInfoNotAvailable,
                                   "abcdefg");

    return TEST_int_eq(fixture->expected, res);
}

static int execute_exchange_error_test(CMP_SES_TEST_FIXTURE *fixture)
{
    int res =
        ossl_cmp_exchange_error(fixture->cmp_ctx,
                                OSSL_CMP_PKISTATUS_rejection,
                                1 << OSSL_CMP_PKIFAILUREINFO_unsupportedVersion,
                                "foo_status", 999, "foo_details");

    return TEST_int_eq(fixture->expected, res);
}

static int test_exchange_certConf(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->expected = 0; /* client should not send certConf immediately */
    if (!ossl_cmp_ctx_set0_newCert(fixture->cmp_ctx, X509_dup(client_cert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_exchange_certConf_test, tear_down);
    return result;
}

static int test_exchange_error(void)
{
    SETUP_TEST_FIXTURE(CMP_SES_TEST_FIXTURE, set_up);
    fixture->expected = 1; /* client may send error any time */
    EXECUTE_TEST(execute_exchange_error_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    X509_free(server_cert);
    EVP_PKEY_free(server_key);
    X509_free(client_cert);
    EVP_PKEY_free(client_key);
    OSSL_PROVIDER_unload(default_null_provider);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
    return;
}

#define USAGE "server.key server.crt client.key client.crt client.csr module_name [module_conf_file]\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(server_key_f = test_get_argument(0))
            || !TEST_ptr(server_cert_f = test_get_argument(1))
            || !TEST_ptr(client_key_f = test_get_argument(2))
            || !TEST_ptr(client_cert_f = test_get_argument(3))
            || !TEST_ptr(pkcs10_f = test_get_argument(4))) {
        TEST_error("usage: cmp_client_test %s", USAGE);
        return 0;
    }

    if (!test_arg_libctx(&libctx, &default_null_provider, &provider, 5, USAGE))
        return 0;

    if (!TEST_ptr(server_key = load_pkey_pem(server_key_f, libctx))
            || !TEST_ptr(server_cert = load_cert_pem(server_cert_f, libctx))
            || !TEST_ptr(client_key = load_pkey_pem(client_key_f, libctx))
            || !TEST_ptr(client_cert = load_cert_pem(client_cert_f, libctx))
            || !TEST_int_eq(1, RAND_bytes_ex(libctx, ref, sizeof(ref), 0))) {
        cleanup_tests();
        return 0;
    }

    ADD_TEST(test_exec_RR_ses_ok);
    ADD_TEST(test_exec_RR_ses_request_error);
    ADD_TEST(test_exec_RR_ses_receive_error);
    ADD_TEST(test_exec_CR_ses_explicit_confirm);
    ADD_TEST(test_exec_CR_ses_implicit_confirm);
    ADD_TEST(test_exec_IR_ses);
    ADD_TEST(test_exec_IR_ses_poll_ok);
    ADD_TEST(test_exec_IR_ses_poll_no_timeout);
    ADD_TEST(test_exec_IR_ses_poll_total_timeout);
    ADD_TEST(test_exec_KUR_ses_ok);
    ADD_TEST(test_exec_KUR_ses_transfer_error);
    ADD_TEST(test_exec_KUR_ses_wrong_popo);
    ADD_TEST(test_exec_KUR_ses_pub);
    ADD_TEST(test_exec_KUR_ses_wrong_pub);
    ADD_TEST(test_exec_P10CR_ses_ok);
    ADD_TEST(test_exec_P10CR_ses_reject);
    ADD_TEST(test_try_certreq_poll);
    ADD_TEST(test_try_certreq_poll_abort);
    ADD_TEST(test_exec_GENM_ses_ok);
    ADD_TEST(test_exec_GENM_ses_transfer_error);
    ADD_TEST(test_exec_GENM_ses_total_timeout);
    ADD_TEST(test_exec_GENM_ses_poll_ok);
    ADD_TEST(test_exec_GENM_ses_poll_no_timeout);
    ADD_TEST(test_exec_GENM_ses_poll_total_timeout);
    ADD_TEST(test_exchange_certConf);
    ADD_TEST(test_exchange_error);
    return 1;
}
