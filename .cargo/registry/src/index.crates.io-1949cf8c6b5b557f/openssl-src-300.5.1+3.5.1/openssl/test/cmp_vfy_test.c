/*
 * Copyright 2007-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"
#include "../crypto/crmf/crmf_local.h" /* for manipulating POPO signature */

static const char *server_f;
static const char *client_f;
static const char *endentity1_f;
static const char *endentity2_f;
static const char *root_f;
static const char *intermediate_f;
static const char *ir_protected_f;
static const char *ir_unprotected_f;
static const char *ir_rmprotection_f;
static const char *ip_waiting_f;
static const char *instacert_f;
static const char *instaca_f;
static const char *ir_protected_0_extracerts;
static const char *ir_protected_2_extracerts;

typedef struct test_fixture {
    const char *test_case_name;
    int expected;
    OSSL_CMP_CTX *cmp_ctx;
    OSSL_CMP_MSG *msg;
    X509 *cert;
    ossl_cmp_allow_unprotected_cb_t allow_unprotected_cb;
    int additional_arg;
} CMP_VFY_TEST_FIXTURE;

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *default_null_provider = NULL, *provider = NULL;

static void tear_down(CMP_VFY_TEST_FIXTURE *fixture)
{
    OSSL_CMP_MSG_free(fixture->msg);
    OSSL_CMP_CTX_free(fixture->cmp_ctx);
    OPENSSL_free(fixture);
}

static time_t test_time_valid = 0, test_time_after_expiration = 0;

static CMP_VFY_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    X509_STORE *ts;
    CMP_VFY_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;

    ts = X509_STORE_new();
    fixture->test_case_name = test_case_name;
    if (ts == NULL
            || !TEST_ptr(fixture->cmp_ctx = OSSL_CMP_CTX_new(libctx, NULL))
            || !OSSL_CMP_CTX_set0_trusted(fixture->cmp_ctx, ts)
            || !OSSL_CMP_CTX_set_log_cb(fixture->cmp_ctx, print_to_bio_out)) {
        tear_down(fixture);
        X509_STORE_free(ts);
        return NULL;
    }
    X509_VERIFY_PARAM_set_time(X509_STORE_get0_param(ts), test_time_valid);
    X509_STORE_set_verify_cb(ts, X509_STORE_CTX_print_verify_cb);
    return fixture;
}

static X509 *srvcert = NULL;
static X509 *clcert = NULL;
/* chain */
static X509 *endentity1 = NULL, *endentity2 = NULL,
    *intermediate = NULL, *root = NULL;
/* INSTA chain */
static X509 *insta_cert = NULL, *instaca_cert = NULL;

static unsigned char rand_data[OSSL_CMP_TRANSACTIONID_LENGTH];
static OSSL_CMP_MSG *ir_unprotected, *ir_rmprotection;

/* secret value used for IP_waitingStatus_PBM.der */
static const unsigned char sec_1[] = {
    '9', 'p', 'p', '8', '-', 'b', '3', '5', 'i', '-', 'X', 'd', '3',
    'Q', '-', 'u', 'd', 'N', 'R'
};

static int flip_bit(ASN1_BIT_STRING *bitstr)
{
    int bit_num = 7;
    int bit = ASN1_BIT_STRING_get_bit(bitstr, bit_num);

    return ASN1_BIT_STRING_set_bit(bitstr, bit_num, !bit);
}

static int execute_verify_popo_test(CMP_VFY_TEST_FIXTURE *fixture)
{
    if ((fixture->msg = load_pkimsg(ir_protected_f, libctx)) == NULL)
        return 0;
    if (fixture->expected == 0) {
        const OSSL_CRMF_MSGS *reqs = fixture->msg->body->value.ir;
        const OSSL_CRMF_MSG *req = sk_OSSL_CRMF_MSG_value(reqs, 0);

        if (req == NULL || !flip_bit(req->popo->value.signature->signature))
            return 0;
    }
    return TEST_int_eq(fixture->expected,
                       ossl_cmp_verify_popo(fixture->cmp_ctx, fixture->msg,
                                            fixture->additional_arg));
}

static int test_verify_popo(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_verify_popo_test, tear_down);
    return result;
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_verify_popo_bad(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    EXECUTE_TEST(execute_verify_popo_test, tear_down);
    return result;
}
#endif

/* indirectly checks also OSSL_CMP_validate_msg() */
static int execute_validate_msg_test(CMP_VFY_TEST_FIXTURE *fixture)
{
    int res = TEST_int_eq(fixture->expected,
                          ossl_cmp_msg_check_update(fixture->cmp_ctx,
                                                    fixture->msg, NULL, 0));
    X509 *validated = OSSL_CMP_CTX_get0_validatedSrvCert(fixture->cmp_ctx);

    return res && (!fixture->expected || TEST_ptr_eq(validated, fixture->cert));
}

static int execute_validate_cert_path_test(CMP_VFY_TEST_FIXTURE *fixture)
{
    X509_STORE *ts = OSSL_CMP_CTX_get0_trusted(fixture->cmp_ctx);
    int res = TEST_int_eq(fixture->expected,
                          OSSL_CMP_validate_cert_path(fixture->cmp_ctx,
                                                      ts, fixture->cert));

    OSSL_CMP_CTX_print_errors(fixture->cmp_ctx);
    return res;
}

static int test_validate_msg_mac_alg_protection(int miss, int wrong)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = NULL;

    fixture->expected = !miss && !wrong;
    if (!TEST_true(miss ? OSSL_CMP_CTX_set0_trusted(fixture->cmp_ctx, NULL)
                   : OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx, sec_1,
                                                   wrong ? 4 : sizeof(sec_1)))
            || !TEST_ptr(fixture->msg = load_pkimsg(ip_waiting_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

static int test_validate_msg_mac_alg_protection_ok(void)
{
    return test_validate_msg_mac_alg_protection(0, 0);
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_mac_alg_protection_missing(void)
{
    return test_validate_msg_mac_alg_protection(1, 0);
}

static int test_validate_msg_mac_alg_protection_wrong(void)
{
    return test_validate_msg_mac_alg_protection(0, 1);
}

static int test_validate_msg_mac_alg_protection_bad(void)
{
    const unsigned char sec_bad[] = {
        '9', 'p', 'p', '8', '-', 'b', '3', '5', 'i', '-', 'X', 'd', '3',
        'Q', '-', 'u', 'd', 'N', 'r'
    };

    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = NULL;
    fixture->expected = 0;

    if (!TEST_true(OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx, sec_bad,
                                                 sizeof(sec_bad)))
            || !TEST_ptr(fixture->msg = load_pkimsg(ip_waiting_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}
#endif

static int add_trusted(OSSL_CMP_CTX *ctx, X509 *cert)
{
    return X509_STORE_add_cert(OSSL_CMP_CTX_get0_trusted(ctx), cert);
}

static int add_untrusted(OSSL_CMP_CTX *ctx, X509 *cert)
{
    return X509_add_cert(OSSL_CMP_CTX_get0_untrusted(ctx), cert,
                         X509_ADD_FLAG_UP_REF);
}

static int test_validate_msg_signature_partial_chain(int expired)
{
    X509_STORE *ts;

    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = srvcert;

    ts = OSSL_CMP_CTX_get0_trusted(fixture->cmp_ctx);
    fixture->expected = !expired;
    if (ts == NULL
            || !TEST_ptr(fixture->msg = load_pkimsg(ir_protected_f, libctx))
            || !add_trusted(fixture->cmp_ctx, srvcert)) {
        tear_down(fixture);
        fixture = NULL;
    } else {
        X509_VERIFY_PARAM *vpm = X509_STORE_get0_param(ts);

        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_PARTIAL_CHAIN);
        if (expired)
            X509_VERIFY_PARAM_set_time(vpm, test_time_after_expiration);
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

static int test_validate_msg_signature_trusted_ok(void)
{
    return test_validate_msg_signature_partial_chain(0);
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_signature_trusted_expired(void)
{
    return test_validate_msg_signature_partial_chain(1);
}
#endif

static int test_validate_msg_signature_srvcert(int bad_sig, int miss, int wrong)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = srvcert;
    fixture->expected = !bad_sig && !wrong && !miss;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_protected_f, libctx))
        || !TEST_true(miss ? OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx,
                                                           sec_1, sizeof(sec_1))
                      :  OSSL_CMP_CTX_set1_srvCert(fixture->cmp_ctx,
                                                   wrong? clcert : srvcert))
        || (bad_sig && !flip_bit(fixture->msg->protection))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_signature_srvcert_missing(void)
{
    return test_validate_msg_signature_srvcert(0, 1, 0);
}
#endif

static int test_validate_msg_signature_srvcert_wrong(void)
{
    return test_validate_msg_signature_srvcert(0, 0, 1);
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_signature_bad(void)
{
    return test_validate_msg_signature_srvcert(1, 0, 0);
}
#endif

static int test_validate_msg_signature_sender_cert_srvcert(void)
{
    return test_validate_msg_signature_srvcert(0, 0, 0);
}

static int test_validate_msg_signature_sender_cert_untrusted(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = insta_cert;
    fixture->expected = 1;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_protected_0_extracerts, libctx))
            || !add_trusted(fixture->cmp_ctx, instaca_cert)
            || !add_untrusted(fixture->cmp_ctx, insta_cert)) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

static int test_validate_msg_signature_sender_cert_trusted(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = insta_cert;
    fixture->expected = 1;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_protected_0_extracerts, libctx))
            || !add_trusted(fixture->cmp_ctx, instaca_cert)
            || !add_trusted(fixture->cmp_ctx, insta_cert)) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

static int test_validate_msg_signature_sender_cert_extracert(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_protected_2_extracerts, libctx))
            || !add_trusted(fixture->cmp_ctx, instaca_cert)) {
        tear_down(fixture);
        fixture = NULL;
    } else {
        fixture->cert = sk_X509_value(fixture->msg->extraCerts, 1); /* Insta CA */
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_signature_sender_cert_absent(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    if (!TEST_ptr(fixture->msg =
                  load_pkimsg(ir_protected_0_extracerts, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}
#endif

static int test_validate_with_sender(const X509_NAME *name, int expected)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->cert = srvcert;
    fixture->expected = expected;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_protected_f, libctx))
        || !TEST_true(OSSL_CMP_CTX_set1_expected_sender(fixture->cmp_ctx, name))
        || !TEST_true(OSSL_CMP_CTX_set1_srvCert(fixture->cmp_ctx, srvcert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}

static int test_validate_msg_signature_expected_sender(void)
{
    return test_validate_with_sender(X509_get_subject_name(srvcert), 1);
}

static int test_validate_msg_signature_unexpected_sender(void)
{
    return test_validate_with_sender(X509_get_subject_name(root), 0);
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_validate_msg_unprotected_request(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_unprotected_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_validate_msg_test, tear_down);
    return result;
}
#endif

static void setup_path(CMP_VFY_TEST_FIXTURE **fixture, X509 *wrong, int expired)
{
    (*fixture)->cert = endentity2;
    (*fixture)->expected = wrong == NULL && !expired;
    if (expired) {
        X509_STORE *ts = OSSL_CMP_CTX_get0_trusted((*fixture)->cmp_ctx);
        X509_VERIFY_PARAM *vpm = X509_STORE_get0_param(ts);

        X509_VERIFY_PARAM_set_time(vpm, test_time_after_expiration);
    }
    if (!add_trusted((*fixture)->cmp_ctx, wrong == NULL ? root : wrong)
            || !add_untrusted((*fixture)->cmp_ctx, endentity1)
            || !add_untrusted((*fixture)->cmp_ctx, intermediate)) {
        tear_down((*fixture));
        (*fixture) = NULL;
    }
}

static int test_validate_cert_path_ok(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_path(&fixture, NULL, 0);
    EXECUTE_TEST(execute_validate_cert_path_test, tear_down);
    return result;
}

static int test_validate_cert_path_wrong_anchor(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_path(&fixture, srvcert /* wrong/non-root cert */, 0);
    EXECUTE_TEST(execute_validate_cert_path_test, tear_down);
    return result;
}

static int test_validate_cert_path_expired(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_path(&fixture, NULL, 1);
    EXECUTE_TEST(execute_validate_cert_path_test, tear_down);
    return result;
}

static int execute_msg_check_test(CMP_VFY_TEST_FIXTURE *fixture)
{
    const OSSL_CMP_PKIHEADER *hdr = OSSL_CMP_MSG_get0_header(fixture->msg);
    const ASN1_OCTET_STRING *tid = OSSL_CMP_HDR_get0_transactionID(hdr);

    if (!TEST_int_eq(fixture->expected,
                     ossl_cmp_msg_check_update(fixture->cmp_ctx,
                                               fixture->msg,
                                               fixture->allow_unprotected_cb,
                                               fixture->additional_arg)))
        return 0;

    if (fixture->expected == 0) /* error expected already during above check */
        return 1;
    return
        TEST_int_eq(0,
                    ASN1_OCTET_STRING_cmp(ossl_cmp_hdr_get0_senderNonce(hdr),
                                          fixture->cmp_ctx->recipNonce))
        && TEST_int_eq(0,
                       ASN1_OCTET_STRING_cmp(tid,
                                             fixture->cmp_ctx->transactionID));
}

static int allow_unprotected(const OSSL_CMP_CTX *ctx, const OSSL_CMP_MSG *msg,
                             int invalid_protection, int allow)
{
    return allow;
}

static void setup_check_update(CMP_VFY_TEST_FIXTURE **fixture, int expected,
                               ossl_cmp_allow_unprotected_cb_t cb, int arg,
                               const unsigned char *trid_data,
                               const unsigned char *nonce_data)
{
    OSSL_CMP_CTX *ctx = (*fixture)->cmp_ctx;
    int nonce_len = OSSL_CMP_SENDERNONCE_LENGTH;

    (*fixture)->expected = expected;
    (*fixture)->allow_unprotected_cb = cb;
    (*fixture)->additional_arg = arg;
    (*fixture)->msg = OSSL_CMP_MSG_dup(ir_rmprotection);
    if ((*fixture)->msg == NULL
        || (nonce_data != NULL
            && !ossl_cmp_asn1_octet_string_set1_bytes(&ctx->senderNonce,
                                                      nonce_data, nonce_len))) {
        tear_down((*fixture));
        (*fixture) = NULL;
    } else if (trid_data != NULL) {
        ASN1_OCTET_STRING *trid = ASN1_OCTET_STRING_new();

        if (trid == NULL
            || !ASN1_OCTET_STRING_set(trid, trid_data,
                                      OSSL_CMP_TRANSACTIONID_LENGTH)
            || !OSSL_CMP_CTX_set1_transactionID(ctx, trid)) {
            tear_down((*fixture));
            (*fixture) = NULL;
        }
        ASN1_OCTET_STRING_free(trid);
    }
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_msg_check_no_protection_no_cb(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 0, NULL, 0, NULL, NULL);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}

static int test_msg_check_no_protection_restrictive_cb(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 0, allow_unprotected, 0, NULL, NULL);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}
#endif

static int test_msg_check_no_protection_permissive_cb(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 1, allow_unprotected, 1, NULL, NULL);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}

static int test_msg_check_transaction_id(void)
{
    /* Transaction id belonging to CMP_IR_rmprotection.der */
    const unsigned char trans_id[OSSL_CMP_TRANSACTIONID_LENGTH] = {
        0x39, 0xB6, 0x90, 0x28, 0xC4, 0xBC, 0x7A, 0xF6,
        0xBE, 0xC6, 0x4A, 0x88, 0x97, 0xA6, 0x95, 0x0B
    };

    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 1, allow_unprotected, 1, trans_id, NULL);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_msg_check_transaction_id_bad(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 0, allow_unprotected, 1, rand_data, NULL);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}
#endif

static int test_msg_check_recipient_nonce(void)
{
    /* Recipient nonce belonging to CMP_IP_ir_rmprotection.der */
    const unsigned char rec_nonce[OSSL_CMP_SENDERNONCE_LENGTH] = {
        0x48, 0xF1, 0x71, 0x1F, 0xE5, 0xAF, 0x1C, 0x8B,
        0x21, 0x97, 0x5C, 0x84, 0x74, 0x49, 0xBA, 0x32
    };

    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 1, allow_unprotected, 1, NULL, rec_nonce);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
static int test_msg_check_recipient_nonce_bad(void)
{
    SETUP_TEST_FIXTURE(CMP_VFY_TEST_FIXTURE, set_up);
    setup_check_update(&fixture, 0, allow_unprotected, 1, NULL, rand_data);
    EXECUTE_TEST(execute_msg_check_test, tear_down);
    return result;
}
#endif

void cleanup_tests(void)
{
    X509_free(srvcert);
    X509_free(clcert);
    X509_free(endentity1);
    X509_free(endentity2);
    X509_free(intermediate);
    X509_free(root);
    X509_free(insta_cert);
    X509_free(instaca_cert);
    OSSL_CMP_MSG_free(ir_unprotected);
    OSSL_CMP_MSG_free(ir_rmprotection);
    OSSL_PROVIDER_unload(default_null_provider);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
    return;
}

#define USAGE "server.crt client.crt " \
    "EndEntity1.crt EndEntity2.crt " \
    "Root_CA.crt Intermediate_CA.crt " \
    "CMP_IR_protected.der CMP_IR_unprotected.der " \
    "IP_waitingStatus_PBM.der IR_rmprotection.der " \
    "insta.cert.pem insta_ca.cert.pem " \
    "IR_protected_0_extraCerts.der " \
    "IR_protected_2_extraCerts.der module_name [module_conf_file]\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    /* Set test time stamps */
    struct tm ts = { 0 };

    ts.tm_year = 2018 - 1900;      /* 2018 */
    ts.tm_mon = 1;                 /* February */
    ts.tm_mday = 18;               /* 18th */
    test_time_valid = mktime(&ts); /* February 18th 2018 */
    ts.tm_year += 10;              /* February 18th 2028 */
    test_time_after_expiration = mktime(&ts);

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH);
    if (!TEST_ptr(server_f = test_get_argument(0))
            || !TEST_ptr(client_f = test_get_argument(1))
            || !TEST_ptr(endentity1_f = test_get_argument(2))
            || !TEST_ptr(endentity2_f = test_get_argument(3))
            || !TEST_ptr(root_f = test_get_argument(4))
            || !TEST_ptr(intermediate_f = test_get_argument(5))
            || !TEST_ptr(ir_protected_f = test_get_argument(6))
            || !TEST_ptr(ir_unprotected_f = test_get_argument(7))
            || !TEST_ptr(ip_waiting_f = test_get_argument(8))
            || !TEST_ptr(ir_rmprotection_f = test_get_argument(9))
            || !TEST_ptr(instacert_f = test_get_argument(10))
            || !TEST_ptr(instaca_f = test_get_argument(11))
            || !TEST_ptr(ir_protected_0_extracerts = test_get_argument(12))
            || !TEST_ptr(ir_protected_2_extracerts = test_get_argument(13))) {
        TEST_error("usage: cmp_vfy_test %s", USAGE);
        return 0;
    }

    if (!test_arg_libctx(&libctx, &default_null_provider, &provider, 14, USAGE))
        return 0;

    /* Load certificates for cert chain */
    if (!TEST_ptr(endentity1 = load_cert_pem(endentity1_f, libctx))
            || !TEST_ptr(endentity2 = load_cert_pem(endentity2_f, libctx))
            || !TEST_ptr(root = load_cert_pem(root_f, NULL))
            || !TEST_ptr(intermediate = load_cert_pem(intermediate_f, libctx)))
        goto err;

    if (!TEST_ptr(insta_cert = load_cert_pem(instacert_f, libctx))
            || !TEST_ptr(instaca_cert = load_cert_pem(instaca_f, libctx)))
        goto err;

    /* Load certificates for message validation */
    if (!TEST_ptr(srvcert = load_cert_pem(server_f, libctx))
            || !TEST_ptr(clcert = load_cert_pem(client_f, libctx)))
        goto err;
    if (!TEST_int_eq(1, RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH)))
        goto err;
    if (!TEST_ptr(ir_unprotected = load_pkimsg(ir_unprotected_f, libctx))
            || !TEST_ptr(ir_rmprotection = load_pkimsg(ir_rmprotection_f,
                                                       libctx)))
        goto err;

    /* Message validation tests */
    ADD_TEST(test_verify_popo);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_verify_popo_bad);
#endif
    ADD_TEST(test_validate_msg_signature_trusted_ok);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_validate_msg_signature_trusted_expired);
    ADD_TEST(test_validate_msg_signature_srvcert_missing);
#endif
    ADD_TEST(test_validate_msg_signature_srvcert_wrong);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_validate_msg_signature_bad);
#endif
    ADD_TEST(test_validate_msg_signature_sender_cert_srvcert);
    ADD_TEST(test_validate_msg_signature_sender_cert_untrusted);
    ADD_TEST(test_validate_msg_signature_sender_cert_trusted);
    ADD_TEST(test_validate_msg_signature_sender_cert_extracert);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_validate_msg_signature_sender_cert_absent);
#endif
    ADD_TEST(test_validate_msg_signature_expected_sender);
    ADD_TEST(test_validate_msg_signature_unexpected_sender);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_validate_msg_unprotected_request);
#endif
    ADD_TEST(test_validate_msg_mac_alg_protection_ok);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_validate_msg_mac_alg_protection_missing);
    ADD_TEST(test_validate_msg_mac_alg_protection_wrong);
    ADD_TEST(test_validate_msg_mac_alg_protection_bad);
#endif

    /* Cert path validation tests */
    ADD_TEST(test_validate_cert_path_ok);
    ADD_TEST(test_validate_cert_path_expired);
    ADD_TEST(test_validate_cert_path_wrong_anchor);

#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_msg_check_no_protection_no_cb);
    ADD_TEST(test_msg_check_no_protection_restrictive_cb);
#endif
    ADD_TEST(test_msg_check_no_protection_permissive_cb);
    ADD_TEST(test_msg_check_transaction_id);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_msg_check_transaction_id_bad);
#endif
    ADD_TEST(test_msg_check_recipient_nonce);
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    ADD_TEST(test_msg_check_recipient_nonce_bad);
#endif

    return 1;

 err:
    cleanup_tests();
    return 0;

}
