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

static const char *ir_protected_f;
static const char *genm_prot_Ed_f;
static const char *ir_unprotected_f;
static const char *ip_PBM_f;

typedef struct test_fixture {
    const char *test_case_name;
    OSSL_CMP_CTX *cmp_ctx;
    /* for protection tests */
    OSSL_CMP_MSG *msg;
    OSSL_CMP_PKISI *si; /* for error and response messages */
    EVP_PKEY *pubkey;
    unsigned char *mem;
    int memlen;
    X509 *cert;
    STACK_OF(X509) *certs;
    STACK_OF(X509) *chain;
    int with_ss;
    int callback_arg;
    int expected;
} CMP_PROTECT_TEST_FIXTURE;

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *default_null_provider = NULL, *provider = NULL;

static void tear_down(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    if (fixture != NULL) {
        OSSL_CMP_CTX_free(fixture->cmp_ctx);
        OSSL_CMP_MSG_free(fixture->msg);
        OSSL_CMP_PKISI_free(fixture->si);

        OPENSSL_free(fixture->mem);
        sk_X509_free(fixture->certs);
        sk_X509_free(fixture->chain);

        OPENSSL_free(fixture);
    }
}

static CMP_PROTECT_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_PROTECT_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->cmp_ctx = OSSL_CMP_CTX_new(libctx, NULL))) {
        tear_down(fixture);
        return NULL;
    }
    return fixture;
}

static EVP_PKEY *prot_RSA_key = NULL;
#ifndef OPENSSL_NO_ECX
static EVP_PKEY *prot_Ed_key = NULL;
static OSSL_CMP_MSG *genm_protected_Ed;
#endif
static EVP_PKEY *server_key = NULL;
static X509 *server_cert = NULL;
static unsigned char rand_data[OSSL_CMP_TRANSACTIONID_LENGTH];
static OSSL_CMP_MSG *ir_unprotected, *ir_protected;
static X509 *endentity1 = NULL, *endentity2 = NULL,
    *root = NULL, *intermediate = NULL;

static int execute_calc_protection_fails_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    ASN1_BIT_STRING *protection =
        ossl_cmp_calc_protection(fixture->cmp_ctx, fixture->msg);
    int res = TEST_ptr_null(protection);

    ASN1_BIT_STRING_free(protection);
    return res;
}

static int execute_calc_protection_pbmac_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    ASN1_BIT_STRING *protection =
        ossl_cmp_calc_protection(fixture->cmp_ctx, fixture->msg);
    int res = TEST_ptr(protection)
            && TEST_true(ASN1_STRING_cmp(protection,
                                         fixture->msg->protection) == 0);

    ASN1_BIT_STRING_free(protection);
    return res;
}

/*
 * This function works similarly to parts of verify_signature in cmp_vfy.c,
 * but without the need for an OSSL_CMP_CTX or an X509 certificate.
 */
static int verify_signature(OSSL_CMP_MSG *msg,
                            ASN1_BIT_STRING *protection,
                            EVP_PKEY *pkey, EVP_MD *digest)
{
    OSSL_CMP_PROTECTEDPART prot_part;

    prot_part.header = OSSL_CMP_MSG_get0_header(msg);
    prot_part.body = msg->body;
    return ASN1_item_verify_ex(ASN1_ITEM_rptr(OSSL_CMP_PROTECTEDPART),
                               msg->header->protectionAlg, protection,
                               &prot_part, NULL, pkey, libctx, NULL) > 0;
}

/* Calls OSSL_CMP_calc_protection and compares and verifies signature */
static int execute_calc_protection_signature_test(CMP_PROTECT_TEST_FIXTURE *
                                                  fixture)
{
    ASN1_BIT_STRING *protection =
        ossl_cmp_calc_protection(fixture->cmp_ctx, fixture->msg);
    int ret = (TEST_ptr(protection)
               && TEST_true(verify_signature(fixture->msg, protection,
                                             fixture->pubkey,
                                             fixture->cmp_ctx->digest)));

    ASN1_BIT_STRING_free(protection);
    return ret;
}

static int test_cmp_calc_protection_no_key_no_secret(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    if (!TEST_ptr(fixture->msg = load_pkimsg(ir_unprotected_f, libctx))
            || !TEST_ptr(fixture->msg->header->protectionAlg =
                         X509_ALGOR_new() /* no specific alg needed here */)) {
        tear_down(fixture);
        fixture = NULL;
    }

    EXECUTE_TEST(execute_calc_protection_fails_test, tear_down);
    return result;
}

static int test_cmp_calc_protection_pkey(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->pubkey = prot_RSA_key;
    if (!TEST_true(OSSL_CMP_CTX_set1_pkey(fixture->cmp_ctx, prot_RSA_key))
        || !TEST_ptr(fixture->msg = load_pkimsg(ir_protected_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_calc_protection_signature_test, tear_down);
    return result;
}

#ifndef OPENSSL_NO_ECX
static int test_cmp_calc_protection_pkey_Ed(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->pubkey = prot_Ed_key;
    if (!TEST_true(OSSL_CMP_CTX_set1_pkey(fixture->cmp_ctx, prot_Ed_key))
        || !TEST_ptr(fixture->msg = load_pkimsg(genm_prot_Ed_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_calc_protection_signature_test, tear_down);
    return result;
}
#endif

static int test_cmp_calc_protection_pbmac(void)
{
    unsigned char sec_insta[] = { 'i', 'n', 's', 't', 'a' };

    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    if (!TEST_true(OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx,
                                                 sec_insta, sizeof(sec_insta)))
            || !TEST_ptr(fixture->msg = load_pkimsg(ip_PBM_f, libctx))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_calc_protection_pbmac_test, tear_down);
    return result;
}
static int execute_MSG_protect_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    return TEST_int_eq(fixture->expected,
                       ossl_cmp_msg_protect(fixture->cmp_ctx, fixture->msg));
}

#define SET_OPT_UNPROTECTED_SEND(ctx, val) \
    OSSL_CMP_CTX_set_option((ctx), OSSL_CMP_OPT_UNPROTECTED_SEND, (val))
static int test_MSG_protect_unprotected_request(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);

    fixture->expected = 1;
    if (!TEST_ptr(fixture->msg = OSSL_CMP_MSG_dup(ir_unprotected))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 1))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_with_msg_sig_alg_protection_plus_rsa_key(void)
{
    const size_t size = sizeof(rand_data) / 2;

    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 1;

    if (!TEST_ptr(fixture->msg = OSSL_CMP_MSG_dup(ir_unprotected))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 0))
            /*
             * Use half of the 16 bytes of random input
             * for each reference and secret value
             */
            || !TEST_true(OSSL_CMP_CTX_set1_referenceValue(fixture->cmp_ctx,
                                                           rand_data, size))
            || !TEST_true(OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx,
                                                        rand_data + size,
                                                        size))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_with_certificate_and_key(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 1;

    if (!TEST_ptr(fixture->msg =
                  OSSL_CMP_MSG_dup(ir_unprotected))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 0))
            || !TEST_true(OSSL_CMP_CTX_set1_pkey(fixture->cmp_ctx, server_key))
            || !TEST_true(OSSL_CMP_CTX_set1_cert(fixture->cmp_ctx,
                                                 server_cert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_certificate_based_without_cert(void)
{
    OSSL_CMP_CTX *ctx;

    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    ctx = fixture->cmp_ctx;
    fixture->expected = 0;
    if (!TEST_ptr(fixture->msg =
                  OSSL_CMP_MSG_dup(ir_unprotected))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(ctx, 0))
            || !TEST_true(EVP_PKEY_up_ref(server_key))
            || !TEST_true(OSSL_CMP_CTX_set0_newPkey(ctx, 1, server_key))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_no_key_no_secret(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    if (!TEST_ptr(fixture->msg = OSSL_CMP_MSG_dup(ir_unprotected))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 0))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_pbmac_no_sender(int with_ref)
{
    static unsigned char secret[] = { 47, 11, 8, 15 };
    static unsigned char ref[] = { 0xca, 0xfe, 0xba, 0xbe };

    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = with_ref;
    if (!TEST_ptr(fixture->msg = OSSL_CMP_MSG_dup(ir_unprotected))
            || !SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 0)
            || !ossl_cmp_hdr_set1_sender(fixture->msg->header, NULL)
            || !OSSL_CMP_CTX_set1_secretValue(fixture->cmp_ctx,
                                              secret, sizeof(secret))
            || (!OSSL_CMP_CTX_set1_referenceValue(fixture->cmp_ctx,
                                                  with_ref ? ref : NULL,
                                                  sizeof(ref)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_protect_test, tear_down);
    return result;
}

static int test_MSG_protect_pbmac_no_sender_with_ref(void)
{
    return test_MSG_protect_pbmac_no_sender(1);
}

static int test_MSG_protect_pbmac_no_sender_no_ref(void)
{
    return test_MSG_protect_pbmac_no_sender(0);
}

static int execute_MSG_add_extraCerts_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    return TEST_true(ossl_cmp_msg_add_extraCerts(fixture->cmp_ctx,
                                                 fixture->msg));
}

static int test_MSG_add_extraCerts(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    if (!TEST_ptr(fixture->msg = OSSL_CMP_MSG_dup(ir_protected))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_MSG_add_extraCerts_test, tear_down);
    return result;
}

#ifndef OPENSSL_NO_EC
/* The cert chain tests use EC certs so we skip them in no-ec builds */
static int execute_cmp_build_cert_chain_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    int ret = 0;
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    X509_STORE *store;
    STACK_OF(X509) *chain =
        X509_build_chain(fixture->cert, fixture->certs, NULL,
                         fixture->with_ss, ctx->libctx, ctx->propq);

    if (TEST_ptr(chain)) {
        /* Check whether chain built is equal to the expected one */
        ret = TEST_int_eq(0, STACK_OF_X509_cmp(chain, fixture->chain));
        OSSL_STACK_OF_X509_free(chain);
    }
    if (!ret)
        return 0;

    if (TEST_ptr(store = X509_STORE_new())
            && TEST_true(X509_STORE_add_cert(store, root))) {
        X509_VERIFY_PARAM_set_flags(X509_STORE_get0_param(store),
                                    X509_V_FLAG_NO_CHECK_TIME);
        chain = X509_build_chain(fixture->cert, fixture->certs, store,
                                 fixture->with_ss, ctx->libctx, ctx->propq);
        ret = TEST_int_eq(fixture->expected, chain != NULL);
        if (ret && chain != NULL) {
            /* Check whether chain built is equal to the expected one */
            ret = TEST_int_eq(0, STACK_OF_X509_cmp(chain, fixture->chain));
            OSSL_STACK_OF_X509_free(chain);
        }
    }
    X509_STORE_free(store);
    return ret;
}

static int test_cmp_build_cert_chain(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    fixture->with_ss = 0;
    fixture->cert = endentity2;
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !TEST_ptr(fixture->chain = sk_X509_new_null())
            || !TEST_true(sk_X509_push(fixture->certs, endentity1))
            || !TEST_true(sk_X509_push(fixture->certs, root))
            || !TEST_true(sk_X509_push(fixture->certs, intermediate))
            || !TEST_true(sk_X509_push(fixture->chain, endentity2))
            || !TEST_true(sk_X509_push(fixture->chain, intermediate))) {
        tear_down(fixture);
        fixture = NULL;
    }
    if (fixture != NULL) {
        result = execute_cmp_build_cert_chain_test(fixture);
        fixture->with_ss = 1;
        if (result && TEST_true(sk_X509_push(fixture->chain, root)))
            result = execute_cmp_build_cert_chain_test(fixture);
    }
    tear_down(fixture);
    return result;
}

static int test_cmp_build_cert_chain_missing_intermediate(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    fixture->with_ss = 0;
    fixture->cert = endentity2;
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !TEST_ptr(fixture->chain = sk_X509_new_null())
            || !TEST_true(sk_X509_push(fixture->certs, endentity1))
            || !TEST_true(sk_X509_push(fixture->certs, root))
            || !TEST_true(sk_X509_push(fixture->chain, endentity2))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_cmp_build_cert_chain_test, tear_down);
    return result;
}

static int test_cmp_build_cert_chain_no_root(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    fixture->with_ss = 0;
    fixture->cert = endentity2;
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !TEST_ptr(fixture->chain = sk_X509_new_null())
            || !TEST_true(sk_X509_push(fixture->certs, endentity1))
            || !TEST_true(sk_X509_push(fixture->certs, intermediate))
            || !TEST_true(sk_X509_push(fixture->chain, endentity2))
            || !TEST_true(sk_X509_push(fixture->chain, intermediate))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_cmp_build_cert_chain_test, tear_down);
    return result;
}

static int test_cmp_build_cert_chain_only_root(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    fixture->with_ss = 0; /* still chain must include the only cert (root) */
    fixture->cert = root;
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !TEST_ptr(fixture->chain = sk_X509_new_null())
            || !TEST_true(sk_X509_push(fixture->certs, root))
            || !TEST_true(sk_X509_push(fixture->chain, root))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_cmp_build_cert_chain_test, tear_down);
    return result;
}

static int test_cmp_build_cert_chain_no_certs(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->expected = 0;
    fixture->with_ss = 0;
    fixture->cert = endentity2;
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !TEST_ptr(fixture->chain = sk_X509_new_null())
            || !TEST_true(sk_X509_push(fixture->chain, endentity2))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_cmp_build_cert_chain_test, tear_down);
    return result;
}
#endif /* OPENSSL_NO_EC */

static int execute_X509_STORE_test(CMP_PROTECT_TEST_FIXTURE *fixture)
{
    X509_STORE *store = X509_STORE_new();
    STACK_OF(X509) *sk = NULL;
    int res = 0;

    if (!TEST_true(ossl_cmp_X509_STORE_add1_certs(store,
                                                  fixture->certs,
                                                  fixture->callback_arg)))
        goto err;
    sk = X509_STORE_get1_all_certs(store);
    if (!TEST_int_eq(0, STACK_OF_X509_cmp(sk, fixture->chain)))
        goto err;
    res = 1;
 err:
    X509_STORE_free(store);
    OSSL_STACK_OF_X509_free(sk);
    return res;

}

static int test_X509_STORE(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->callback_arg = 0; /* self-issued allowed */
    if (!TEST_ptr(fixture->certs = sk_X509_new_null())
            || !sk_X509_push(fixture->certs, endentity1)
            || !sk_X509_push(fixture->certs, endentity2)
            || !sk_X509_push(fixture->certs, root)
            || !sk_X509_push(fixture->certs, intermediate)
            || !TEST_ptr(fixture->chain = sk_X509_dup(fixture->certs))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_X509_STORE_test, tear_down);
    return result;
}

static int test_X509_STORE_only_self_issued(void)
{
    SETUP_TEST_FIXTURE(CMP_PROTECT_TEST_FIXTURE, set_up);
    fixture->certs = sk_X509_new_null();
    fixture->chain = sk_X509_new_null();
    fixture->callback_arg = 1; /* only self-issued */
    if (!TEST_true(sk_X509_push(fixture->certs, endentity1))
            || !TEST_true(sk_X509_push(fixture->certs, endentity2))
            || !TEST_true(sk_X509_push(fixture->certs, root))
            || !TEST_true(sk_X509_push(fixture->certs, intermediate))
            || !TEST_true(sk_X509_push(fixture->chain, root))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_X509_STORE_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    EVP_PKEY_free(prot_RSA_key);
#ifndef OPENSSL_NO_ECX
    EVP_PKEY_free(prot_Ed_key);
    OSSL_CMP_MSG_free(genm_protected_Ed);
#endif
    EVP_PKEY_free(server_key);
    X509_free(server_cert);
    X509_free(endentity1);
    X509_free(endentity2);
    X509_free(root);
    X509_free(intermediate);
    OSSL_CMP_MSG_free(ir_protected);
    OSSL_CMP_MSG_free(ir_unprotected);
    OSSL_PROVIDER_unload(default_null_provider);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
}

#define USAGE "prot_RSA.pem IR_protected.der prot_Ed.pem " \
    "GENM_protected_Ed.der IR_unprotected.der IP_PBM.der " \
    "server.crt server.pem EndEntity1.crt EndEntity2.crt Root_CA.crt " \
    "Intermediate_CA.crt module_name [module_conf_file]\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    char *prot_RSA_f;
    char *prot_Ed_f;
    char *server_key_f;
    char *server_cert_f;
    char *endentity1_f;
    char *endentity2_f;
    char *root_f;
    char *intermediate_f;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH);
    if (!TEST_ptr(prot_RSA_f = test_get_argument(0))
            || !TEST_ptr(ir_protected_f = test_get_argument(1))
            || !TEST_ptr(prot_Ed_f = test_get_argument(2))
            || !TEST_ptr(genm_prot_Ed_f = test_get_argument(3))
            || !TEST_ptr(ir_unprotected_f = test_get_argument(4))
            || !TEST_ptr(ip_PBM_f = test_get_argument(5))
            || !TEST_ptr(server_cert_f = test_get_argument(6))
            || !TEST_ptr(server_key_f = test_get_argument(7))
            || !TEST_ptr(endentity1_f = test_get_argument(8))
            || !TEST_ptr(endentity2_f = test_get_argument(9))
            || !TEST_ptr(root_f = test_get_argument(10))
            || !TEST_ptr(intermediate_f = test_get_argument(11))) {
        TEST_error("usage: cmp_protect_test %s", USAGE);
        return 0;
    }

    if (!test_arg_libctx(&libctx, &default_null_provider, &provider, 12, USAGE))
        return 0;

    if (!TEST_ptr(server_key = load_pkey_pem(server_key_f, libctx))
            || !TEST_ptr(server_cert = load_cert_pem(server_cert_f, libctx)))
        return 0;

    if (!TEST_ptr(prot_RSA_key = load_pkey_pem(prot_RSA_f, libctx)))
        return 0;
#ifndef OPENSSL_NO_ECX
    if (!TEST_ptr(prot_Ed_key = load_pkey_pem(prot_Ed_f, libctx)))
        return 0;
#endif
    if (!TEST_ptr(ir_protected = load_pkimsg(ir_protected_f, libctx))
#ifndef OPENSSL_NO_ECX
        || !TEST_ptr(genm_protected_Ed = load_pkimsg(genm_prot_Ed_f, libctx))
#endif
        || !TEST_ptr(ir_unprotected = load_pkimsg(ir_unprotected_f, libctx)))
        return 0;
    if (!TEST_ptr(endentity1 = load_cert_pem(endentity1_f, libctx))
            || !TEST_ptr(endentity2 = load_cert_pem(endentity2_f, libctx))
            || !TEST_ptr(root = load_cert_pem(root_f, libctx))
            || !TEST_ptr(intermediate = load_cert_pem(intermediate_f, libctx)))
        return 0;
    if (!TEST_int_eq(1, RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH)))
        return 0;

    /* Message protection tests */
    ADD_TEST(test_cmp_calc_protection_no_key_no_secret);
    ADD_TEST(test_cmp_calc_protection_pkey);
#ifndef OPENSSL_NO_ECX
    ADD_TEST(test_cmp_calc_protection_pkey_Ed);
#endif
    ADD_TEST(test_cmp_calc_protection_pbmac);

    ADD_TEST(test_MSG_protect_with_msg_sig_alg_protection_plus_rsa_key);
    ADD_TEST(test_MSG_protect_with_certificate_and_key);
    ADD_TEST(test_MSG_protect_certificate_based_without_cert);
    ADD_TEST(test_MSG_protect_unprotected_request);
    ADD_TEST(test_MSG_protect_no_key_no_secret);
    ADD_TEST(test_MSG_protect_pbmac_no_sender_with_ref);
    ADD_TEST(test_MSG_protect_pbmac_no_sender_no_ref);
    ADD_TEST(test_MSG_add_extraCerts);

#ifndef OPENSSL_NO_EC
    ADD_TEST(test_cmp_build_cert_chain);
    ADD_TEST(test_cmp_build_cert_chain_only_root);
    ADD_TEST(test_cmp_build_cert_chain_no_root);
    ADD_TEST(test_cmp_build_cert_chain_missing_intermediate);
    ADD_TEST(test_cmp_build_cert_chain_no_certs);
#endif

    ADD_TEST(test_X509_STORE);
    ADD_TEST(test_X509_STORE_only_self_issued);

    return 1;
}
