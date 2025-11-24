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

static const char *newkey_f;
static const char *server_cert_f;
static const char *pkcs10_f;

typedef struct test_fixture {
    const char *test_case_name;
    OSSL_CMP_CTX *cmp_ctx;
    /* for msg create tests */
    int bodytype;
    int err_code;
    /* for certConf */
    int fail_info;
    /* for protection tests */
    OSSL_CMP_MSG *msg;
    int expected;
    /* for error and response messages */
    OSSL_CMP_PKISI *si;
} CMP_MSG_TEST_FIXTURE;

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *default_null_provider = NULL, *provider = NULL;

static unsigned char ref[CMP_TEST_REFVALUE_LENGTH];

static void tear_down(CMP_MSG_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX_free(fixture->cmp_ctx);
    OSSL_CMP_MSG_free(fixture->msg);
    OSSL_CMP_PKISI_free(fixture->si);
    OPENSSL_free(fixture);
}

#define SET_OPT_UNPROTECTED_SEND(ctx, val) \
    OSSL_CMP_CTX_set_option((ctx), OSSL_CMP_OPT_UNPROTECTED_SEND, (val))
static CMP_MSG_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_MSG_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;

    if (!TEST_ptr(fixture->cmp_ctx = OSSL_CMP_CTX_new(libctx, NULL))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 1))
            || !TEST_true(OSSL_CMP_CTX_set1_referenceValue(fixture->cmp_ctx,
                                                           ref, sizeof(ref)))) {
        tear_down(fixture);
        return NULL;
    }
    return fixture;
}

static EVP_PKEY *newkey = NULL;
static X509 *cert = NULL;

#define EXECUTE_MSG_CREATION_TEST(expr) \
    do { \
        OSSL_CMP_MSG *msg = NULL; \
        int good = fixture->expected != 0 ? \
            TEST_ptr(msg = (expr)) && TEST_true(valid_asn1_encoding(msg)) : \
            TEST_ptr_null(msg = (expr)); \
 \
        OSSL_CMP_MSG_free(msg); \
        ERR_print_errors_fp(stderr); \
        return good; \
    } while (0)

/*-
 * The following tests call a cmp message creation function.
 * if fixture->expected != 0:
 *         returns 1 if the message is created and syntactically correct.
 * if fixture->expected == 0
 *         returns 1 if message creation returns NULL
 */
static int execute_certreq_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_certreq_new(fixture->cmp_ctx,
                                                   fixture->bodytype,
                                                   NULL));
}

static int execute_errormsg_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_error_new(fixture->cmp_ctx, fixture->si,
                                                 fixture->err_code,
                                                 "details", 0));
}

static int execute_rr_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_rr_new(fixture->cmp_ctx));
}

static int execute_certconf_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_certConf_new
                              (fixture->cmp_ctx, OSSL_CMP_CERTREQID,
                               fixture->fail_info, NULL));
}

static int execute_genm_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_genm_new(fixture->cmp_ctx));
}

static int execute_pollreq_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_pollReq_new(fixture->cmp_ctx, 4711));
}

static int execute_pkimessage_create_test(CMP_MSG_TEST_FIXTURE *fixture)
{
    EXECUTE_MSG_CREATION_TEST(ossl_cmp_msg_create
                              (fixture->cmp_ctx, fixture->bodytype));
}

static int set1_newPkey(OSSL_CMP_CTX *ctx, EVP_PKEY *pkey)
{
    if (!EVP_PKEY_up_ref(pkey))
        return 0;

    if (!OSSL_CMP_CTX_set0_newPkey(ctx, 1, pkey)) {
        EVP_PKEY_free(pkey);
        return 0;
    }
    return 1;
}

static int test_cmp_create_ir_protection_set(void)
{
    OSSL_CMP_CTX *ctx;
    unsigned char secret[16];

    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);

    ctx = fixture->cmp_ctx;
    fixture->bodytype = OSSL_CMP_PKIBODY_IR;
    fixture->err_code = -1;
    fixture->expected = 1;
    if (!TEST_int_eq(1, RAND_bytes_ex(libctx, secret, sizeof(secret), 0))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(ctx, 0))
            || !TEST_true(set1_newPkey(ctx, newkey))
            || !TEST_true(OSSL_CMP_CTX_set1_secretValue(ctx, secret,
                                                        sizeof(secret)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_ir_protection_fails(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_IR;
    fixture->err_code = -1;
    fixture->expected = 0;
    if (!TEST_true(OSSL_CMP_CTX_set1_pkey(fixture->cmp_ctx, newkey))
            || !TEST_true(SET_OPT_UNPROTECTED_SEND(fixture->cmp_ctx, 0))
            /* newkey used by default for signing does not match cert: */
            || !TEST_true(OSSL_CMP_CTX_set1_cert(fixture->cmp_ctx, cert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_cr_without_key(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_CR;
    fixture->err_code = -1;
    fixture->expected = 0;
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_cr(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_CR;
    fixture->err_code = -1;
    fixture->expected = 1;
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_certreq_with_invalid_bodytype(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_RR;
    fixture->err_code = -1;
    fixture->expected = 0;
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_p10cr(void)
{
    OSSL_CMP_CTX *ctx;
    X509_REQ *p10cr = NULL;

    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    ctx = fixture->cmp_ctx;
    fixture->bodytype = OSSL_CMP_PKIBODY_P10CR;
    fixture->err_code = CMP_R_ERROR_CREATING_CERTREQ;
    fixture->expected = 1;
    if (!TEST_ptr(p10cr = load_csr_der(pkcs10_f, libctx))
            || !TEST_true(set1_newPkey(ctx, newkey))
            || !TEST_true(OSSL_CMP_CTX_set1_p10CSR(ctx, p10cr))) {
        tear_down(fixture);
        fixture = NULL;
    }
    X509_REQ_free(p10cr);
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_p10cr_null(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_P10CR;
    fixture->err_code = CMP_R_ERROR_CREATING_CERTREQ;
    fixture->expected = 0;
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_kur(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_KUR;
    fixture->err_code = -1;
    fixture->expected = 1;
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))
            || !TEST_true(OSSL_CMP_CTX_set1_oldCert(fixture->cmp_ctx, cert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_kur_without_oldcert(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->bodytype = OSSL_CMP_PKIBODY_KUR;
    fixture->err_code = -1;
    fixture->expected = 0;
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_certconf(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->fail_info = 0;
    fixture->expected = 1;
    if (!TEST_true(ossl_cmp_ctx_set0_newCert(fixture->cmp_ctx,
                                             X509_dup(cert)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certconf_create_test, tear_down);
    return result;
}

static int test_cmp_create_certconf_badAlg(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_badAlg;
    fixture->expected = 1;
    if (!TEST_true(ossl_cmp_ctx_set0_newCert(fixture->cmp_ctx,
                                             X509_dup(cert)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certconf_create_test, tear_down);
    return result;
}

static int test_cmp_create_certconf_fail_info_max(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_MAX;
    fixture->expected = 1;
    if (!TEST_true(ossl_cmp_ctx_set0_newCert(fixture->cmp_ctx,
                                             X509_dup(cert)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_certconf_create_test, tear_down);
    return result;
}

static int test_cmp_create_error_msg(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->si = OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_rejection,
                                          OSSL_CMP_PKIFAILUREINFO_systemFailure,
                                          NULL);
    fixture->err_code = -1;
    fixture->expected = 1; /* expected: message creation is successful */
    if (!TEST_true(set1_newPkey(fixture->cmp_ctx, newkey))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_errormsg_create_test, tear_down);
    return result;
}

static int test_cmp_create_pollreq(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_pollreq_create_test, tear_down);
    return result;
}

static int test_cmp_create_rr(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    if (!TEST_true(OSSL_CMP_CTX_set1_oldCert(fixture->cmp_ctx, cert))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_rr_create_test, tear_down);
    return result;
}

static int test_cmp_create_genm(void)
{
    OSSL_CMP_ITAV *iv = NULL;

    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    iv = OSSL_CMP_ITAV_create(OBJ_nid2obj(NID_id_it_implicitConfirm), NULL);
    if (!TEST_ptr(iv)
            || !TEST_true(OSSL_CMP_CTX_push0_genm_ITAV(fixture->cmp_ctx, iv))) {
        OSSL_CMP_ITAV_free(iv);
        tear_down(fixture);
        fixture = NULL;
    }

    EXECUTE_TEST(execute_genm_create_test, tear_down);
    return result;
}

static int execute_certrep_create(CMP_MSG_TEST_FIXTURE *fixture)
{
    OSSL_CMP_CTX *ctx = fixture->cmp_ctx;
    OSSL_CMP_CERTREPMESSAGE *crepmsg = OSSL_CMP_CERTREPMESSAGE_new();
    OSSL_CMP_CERTRESPONSE *read_cresp, *cresp = OSSL_CMP_CERTRESPONSE_new();
    X509 *certfromresp = NULL;
    int res = 0;

    if (crepmsg == NULL || cresp == NULL)
        goto err;
    if (!ASN1_INTEGER_set(cresp->certReqId, 99))
        goto err;
    if ((cresp->certifiedKeyPair = OSSL_CMP_CERTIFIEDKEYPAIR_new()) == NULL)
        goto err;
    cresp->certifiedKeyPair->certOrEncCert->type =
        OSSL_CMP_CERTORENCCERT_CERTIFICATE;
    if ((cresp->certifiedKeyPair->certOrEncCert->value.certificate =
         X509_dup(cert)) == NULL
            || !sk_OSSL_CMP_CERTRESPONSE_push(crepmsg->response, cresp))
        goto err;
    cresp = NULL;
    read_cresp = ossl_cmp_certrepmessage_get0_certresponse(crepmsg, 99);
    if (!TEST_ptr(read_cresp))
        goto err;
    if (!TEST_ptr_null(ossl_cmp_certrepmessage_get0_certresponse(crepmsg, 88)))
        goto err;
    certfromresp = ossl_cmp_certresponse_get1_cert(ctx, read_cresp);
    if (certfromresp == NULL || !TEST_int_eq(X509_cmp(cert, certfromresp), 0))
        goto err;

    res = 1;
 err:
    X509_free(certfromresp);
    OSSL_CMP_CERTRESPONSE_free(cresp);
    OSSL_CMP_CERTREPMESSAGE_free(crepmsg);
    return res;
}

static int test_cmp_create_certrep(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_certrep_create, tear_down);
    return result;
}

static int execute_rp_create(CMP_MSG_TEST_FIXTURE *fixture)
{
    OSSL_CMP_PKISI *si = OSSL_CMP_STATUSINFO_new(33, 44, "a text");
    X509_NAME *issuer = X509_NAME_new();
    ASN1_INTEGER *serial = ASN1_INTEGER_new();
    OSSL_CRMF_CERTID *cid = NULL;
    OSSL_CMP_MSG *rpmsg = NULL;
    int res = 0;

    if (si == NULL || issuer == NULL || serial == NULL)
        goto err;

    if (!X509_NAME_add_entry_by_txt(issuer, "CN", MBSTRING_ASC,
                                    (unsigned char *)"The Issuer", -1, -1, 0)
            || !ASN1_INTEGER_set(serial, 99)
            || (cid = OSSL_CRMF_CERTID_gen(issuer, serial)) == NULL
            || (rpmsg = ossl_cmp_rp_new(fixture->cmp_ctx, si, cid, 1)) == NULL)
        goto err;

    if (!TEST_ptr(ossl_cmp_revrepcontent_get_CertId(rpmsg->body->value.rp, 0)))
        goto err;

    if (!TEST_ptr(ossl_cmp_revrepcontent_get_pkisi(rpmsg->body->value.rp, 0)))
        goto err;

    res = 1;
 err:
    ASN1_INTEGER_free(serial);
    X509_NAME_free(issuer);
    OSSL_CRMF_CERTID_free(cid);
    OSSL_CMP_PKISI_free(si);
    OSSL_CMP_MSG_free(rpmsg);
    return res;
}

static int test_cmp_create_rp(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_rp_create, tear_down);
    return result;
}

static int execute_pollrep_create(CMP_MSG_TEST_FIXTURE *fixture)
{
    OSSL_CMP_MSG *pollrep;
    int res = 0;

    pollrep = ossl_cmp_pollRep_new(fixture->cmp_ctx, 77, 2000);
    if (!TEST_ptr(pollrep))
        return 0;
    if (!TEST_ptr(ossl_cmp_pollrepcontent_get0_pollrep(pollrep->body->
                                                       value.pollRep, 77)))
        goto err;
    if (!TEST_ptr_null(ossl_cmp_pollrepcontent_get0_pollrep(pollrep->body->
                                                            value.pollRep, 88)))
        goto err;

    res = 1;
 err:
    OSSL_CMP_MSG_free(pollrep);
    return res;
}

static int test_cmp_create_pollrep(void)
{
    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_pollrep_create, tear_down);
    return result;
}

static int test_cmp_pkimessage_create(int bodytype)
{
    X509_REQ *p10cr = NULL;

    SETUP_TEST_FIXTURE(CMP_MSG_TEST_FIXTURE, set_up);

    switch (fixture->bodytype = bodytype) {
    case OSSL_CMP_PKIBODY_P10CR:
        fixture->expected = 1;
        p10cr = load_csr_der(pkcs10_f, libctx);
        if (!TEST_true(OSSL_CMP_CTX_set1_p10CSR(fixture->cmp_ctx, p10cr))) {
            tear_down(fixture);
            fixture = NULL;
        }
        X509_REQ_free(p10cr);
        break;
    case OSSL_CMP_PKIBODY_IR:
    case OSSL_CMP_PKIBODY_IP:
    case OSSL_CMP_PKIBODY_CR:
    case OSSL_CMP_PKIBODY_CP:
    case OSSL_CMP_PKIBODY_KUR:
    case OSSL_CMP_PKIBODY_KUP:
    case OSSL_CMP_PKIBODY_RR:
    case OSSL_CMP_PKIBODY_RP:
    case OSSL_CMP_PKIBODY_PKICONF:
    case OSSL_CMP_PKIBODY_GENM:
    case OSSL_CMP_PKIBODY_GENP:
    case OSSL_CMP_PKIBODY_ERROR:
    case OSSL_CMP_PKIBODY_CERTCONF:
    case OSSL_CMP_PKIBODY_POLLREQ:
    case OSSL_CMP_PKIBODY_POLLREP:
        fixture->expected = 1;
        break;
    default:
        fixture->expected = 0;
        break;
    }

    EXECUTE_TEST(execute_pkimessage_create_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    EVP_PKEY_free(newkey);
    X509_free(cert);
    OSSL_PROVIDER_unload(default_null_provider);
    OSSL_PROVIDER_unload(provider);
    OSSL_LIB_CTX_free(libctx);
}

#define USAGE "new.key server.crt pkcs10.der module_name [module_conf_file]\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(newkey_f = test_get_argument(0))
            || !TEST_ptr(server_cert_f = test_get_argument(1))
            || !TEST_ptr(pkcs10_f = test_get_argument(2))) {
        TEST_error("usage: cmp_msg_test %s", USAGE);
        return 0;
    }

    if (!test_arg_libctx(&libctx, &default_null_provider, &provider, 3, USAGE))
        return 0;

    if (!TEST_ptr(newkey = load_pkey_pem(newkey_f, libctx))
            || !TEST_ptr(cert = load_cert_pem(server_cert_f, libctx))
            || !TEST_int_eq(1, RAND_bytes_ex(libctx, ref, sizeof(ref), 0))) {
        cleanup_tests();
        return 0;
    }

    /* Message creation tests */
    ADD_TEST(test_cmp_create_certreq_with_invalid_bodytype);
    ADD_TEST(test_cmp_create_ir_protection_fails);
    ADD_TEST(test_cmp_create_ir_protection_set);
    ADD_TEST(test_cmp_create_error_msg);
    ADD_TEST(test_cmp_create_certconf);
    ADD_TEST(test_cmp_create_certconf_badAlg);
    ADD_TEST(test_cmp_create_certconf_fail_info_max);
    ADD_TEST(test_cmp_create_kur);
    ADD_TEST(test_cmp_create_kur_without_oldcert);
    ADD_TEST(test_cmp_create_cr);
    ADD_TEST(test_cmp_create_cr_without_key);
    ADD_TEST(test_cmp_create_p10cr);
    ADD_TEST(test_cmp_create_p10cr_null);
    ADD_TEST(test_cmp_create_pollreq);
    ADD_TEST(test_cmp_create_rr);
    ADD_TEST(test_cmp_create_rp);
    ADD_TEST(test_cmp_create_genm);
    ADD_TEST(test_cmp_create_certrep);
    ADD_TEST(test_cmp_create_pollrep);
    ADD_ALL_TESTS_NOSUBTEST(test_cmp_pkimessage_create,
                            OSSL_CMP_PKIBODY_POLLREP + 1);
    return 1;
}
