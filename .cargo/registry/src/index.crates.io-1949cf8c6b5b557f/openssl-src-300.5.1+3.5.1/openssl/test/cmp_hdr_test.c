/*
 * Copyright 2007-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "helpers/cmp_testlib.h"

static unsigned char rand_data[OSSL_CMP_TRANSACTIONID_LENGTH];

typedef struct test_fixture {
    const char *test_case_name;
    int expected;
    OSSL_CMP_CTX *cmp_ctx;
    OSSL_CMP_PKIHEADER *hdr;

} CMP_HDR_TEST_FIXTURE;

static void tear_down(CMP_HDR_TEST_FIXTURE *fixture)
{
    OSSL_CMP_PKIHEADER_free(fixture->hdr);
    OSSL_CMP_CTX_free(fixture->cmp_ctx);
    OPENSSL_free(fixture);
}

static CMP_HDR_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CMP_HDR_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->cmp_ctx = OSSL_CMP_CTX_new(NULL, NULL)))
        goto err;
    if (!TEST_ptr(fixture->hdr = OSSL_CMP_PKIHEADER_new()))
        goto err;
    return fixture;

 err:
    tear_down(fixture);
    return NULL;
}

static int execute_HDR_set_get_pvno_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    int pvno = 77;

    if (!TEST_int_eq(ossl_cmp_hdr_set_pvno(fixture->hdr, pvno), 1))
        return 0;
    if (!TEST_int_eq(ossl_cmp_hdr_get_pvno(fixture->hdr), pvno))
        return 0;
    return 1;
}

static int test_HDR_set_get_pvno(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_set_get_pvno_test, tear_down);
    return result;
}

#define X509_NAME_ADD(n, rd, s) \
    X509_NAME_add_entry_by_txt((n), (rd), MBSTRING_ASC, (unsigned char *)(s), \
                               -1, -1, 0)

static int execute_HDR_get0_senderNonce_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    int res = 0;
    X509_NAME *sender = X509_NAME_new();
    ASN1_OCTET_STRING *sn;

    if (!TEST_ptr(sender))
        goto err;

    X509_NAME_ADD(sender, "CN", "A common sender name");
    if (!TEST_int_eq(OSSL_CMP_CTX_set1_subjectName(fixture->cmp_ctx, sender),
                     1))
        goto err;
    if (!TEST_int_eq(ossl_cmp_hdr_init(fixture->cmp_ctx, fixture->hdr),
                     1))
        goto err;
    sn = ossl_cmp_hdr_get0_senderNonce(fixture->hdr);
    if (!TEST_int_eq(ASN1_OCTET_STRING_cmp(fixture->cmp_ctx->senderNonce, sn),
                     0))
        goto err;

    res = 1;
err:
    X509_NAME_free(sender);

    return res;
}

static int test_HDR_get0_senderNonce(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_get0_senderNonce_test, tear_down);
    return result;
}

static int execute_HDR_set1_sender_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    int res = 0;
    X509_NAME *x509name = X509_NAME_new();

    if (!TEST_ptr(x509name))
        goto err;

    X509_NAME_ADD(x509name, "CN", "A common sender name");
    if (!TEST_int_eq(ossl_cmp_hdr_set1_sender(fixture->hdr, x509name), 1))
        goto err;

    if (!TEST_int_eq(fixture->hdr->sender->type, GEN_DIRNAME))
        goto err;

    if (!TEST_int_eq(X509_NAME_cmp(fixture->hdr->sender->d.directoryName,
                                   x509name), 0))
        goto err;

    res = 1;
err:
    X509_NAME_free(x509name);

    return res;
}

static int test_HDR_set1_sender(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_set1_sender_test, tear_down);
    return result;
}

static int execute_HDR_set1_recipient_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    int res = 0;
    X509_NAME *x509name = X509_NAME_new();

    if (!TEST_ptr(x509name))
        goto err;

    X509_NAME_ADD(x509name, "CN", "A common recipient name");
    if (!TEST_int_eq(ossl_cmp_hdr_set1_recipient(fixture->hdr, x509name), 1))
        goto err;

    if (!TEST_int_eq(fixture->hdr->recipient->type, GEN_DIRNAME))
        goto err;

    if (!TEST_int_eq(X509_NAME_cmp(fixture->hdr->recipient->d.directoryName,
                                   x509name), 0))
        goto err;

    res = 1;
err:
    X509_NAME_free(x509name);

    return res;
}

static int test_HDR_set1_recipient(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_set1_recipient_test, tear_down);
    return result;
}

static int execute_HDR_update_messageTime_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    struct tm hdrtm, tmptm;
    time_t hdrtime, before, after, now;

    now = time(NULL);
    /*
     * Trial and error reveals that passing the return value from gmtime
     * directly to mktime in a mingw 32 bit build gives unexpected results. To
     * work around this we take a copy of the return value first.
     */
    tmptm = *gmtime(&now);
    before = mktime(&tmptm);

    if (!TEST_true(ossl_cmp_hdr_update_messageTime(fixture->hdr)))
        return 0;
    if (!TEST_true(ASN1_TIME_to_tm(fixture->hdr->messageTime, &hdrtm)))
        return 0;

    hdrtime = mktime(&hdrtm);

    if (!TEST_time_t_le(before, hdrtime))
        return 0;
    now = time(NULL);
    tmptm = *gmtime(&now);
    after = mktime(&tmptm);

    return TEST_time_t_le(hdrtime, after);
}

static int test_HDR_update_messageTime(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_update_messageTime_test, tear_down);
    return result;
}

static int execute_HDR_set1_senderKID_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    ASN1_OCTET_STRING *senderKID = ASN1_OCTET_STRING_new();
    int res = 0;

    if (!TEST_ptr(senderKID))
        goto err;

    if (!TEST_int_eq(ASN1_OCTET_STRING_set(senderKID, rand_data,
                                           sizeof(rand_data)), 1))
        goto err;
    if (!TEST_int_eq(ossl_cmp_hdr_set1_senderKID(fixture->hdr, senderKID), 1))
        goto err;
    if (!TEST_int_eq(ASN1_OCTET_STRING_cmp(fixture->hdr->senderKID,
                                           senderKID), 0))
        goto err;
    res = 1;
 err:
    ASN1_OCTET_STRING_free(senderKID);
    return res;
}

static int test_HDR_set1_senderKID(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_set1_senderKID_test, tear_down);
    return result;
}

static int execute_HDR_push0_freeText_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    ASN1_UTF8STRING *text = ASN1_UTF8STRING_new();

    if (!TEST_ptr(text))
        return 0;

    if (!ASN1_STRING_set(text, "A free text", -1))
        goto err;

    if (!TEST_int_eq(ossl_cmp_hdr_push0_freeText(fixture->hdr, text), 1))
        goto err;

    if (!TEST_true(text == sk_ASN1_UTF8STRING_value(fixture->hdr->freeText, 0)))
        goto err;

    return 1;

 err:
    ASN1_UTF8STRING_free(text);
    return 0;
}

static int test_HDR_push0_freeText(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_push0_freeText_test, tear_down);
    return result;
}

static int execute_HDR_push1_freeText_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    ASN1_UTF8STRING *text = ASN1_UTF8STRING_new();
    ASN1_UTF8STRING *pushed_text;
    int res = 0;

    if (!TEST_ptr(text))
        goto err;

    if (!ASN1_STRING_set(text, "A free text", -1))
        goto err;

    if (!TEST_int_eq(ossl_cmp_hdr_push1_freeText(fixture->hdr, text), 1))
        goto err;

    pushed_text = sk_ASN1_UTF8STRING_value(fixture->hdr->freeText, 0);
    if (!TEST_int_eq(ASN1_STRING_cmp(text, pushed_text), 0))
        goto err;

    res = 1;
 err:
    ASN1_UTF8STRING_free(text);

    return res;
}

static int test_HDR_push1_freeText(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_push1_freeText_test, tear_down);
    return result;
}

static int
execute_HDR_generalInfo_push0_item_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    OSSL_CMP_ITAV *itav = OSSL_CMP_ITAV_new();

    if (!TEST_ptr(itav))
        return 0;

    if (!TEST_int_eq(ossl_cmp_hdr_generalInfo_push0_item(fixture->hdr, itav),
                     1))
        return 0;

    if (!TEST_true(itav == sk_OSSL_CMP_ITAV_value(fixture->hdr->generalInfo,
                                                  0)))
        return 0;

    return 1;
}

static int test_HDR_generalInfo_push0_item(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_generalInfo_push0_item_test, tear_down);
    return result;
}

static int
execute_HDR_generalInfo_push1_items_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    const char oid[] = "1.2.3.4";
    char buf[20];
    OSSL_CMP_ITAV *itav, *pushed_itav;
    STACK_OF(OSSL_CMP_ITAV) *itavs = NULL, *ginfo;
    ASN1_INTEGER *asn1int = ASN1_INTEGER_new();
    ASN1_TYPE *val = ASN1_TYPE_new();
    ASN1_TYPE *pushed_val;
    int res = 0;

    if (!TEST_ptr(asn1int))
        return 0;

    if (!TEST_ptr(val)
            || !TEST_true(ASN1_INTEGER_set(asn1int, 88))) {
        ASN1_INTEGER_free(asn1int);
        return 0;
    }

    ASN1_TYPE_set(val, V_ASN1_INTEGER, asn1int);
    if (!TEST_ptr(itav = OSSL_CMP_ITAV_create(OBJ_txt2obj(oid, 1), val))) {
        ASN1_TYPE_free(val);
        return 0;
    }
    if (!TEST_true(OSSL_CMP_ITAV_push0_stack_item(&itavs, itav))) {
        OSSL_CMP_ITAV_free(itav);
        return 0;
    }

    if (!TEST_int_eq(ossl_cmp_hdr_generalInfo_push1_items(fixture->hdr, itavs),
                     1))
        goto err;
    ginfo = fixture->hdr->generalInfo;
    pushed_itav = sk_OSSL_CMP_ITAV_value(ginfo, 0);
    OBJ_obj2txt(buf, sizeof(buf), OSSL_CMP_ITAV_get0_type(pushed_itav), 0);
    if (!TEST_int_eq(memcmp(oid, buf, sizeof(oid)), 0))
        goto err;

    pushed_val = OSSL_CMP_ITAV_get0_value(sk_OSSL_CMP_ITAV_value(ginfo, 0));
    if (!TEST_int_eq(ASN1_TYPE_cmp(itav->infoValue.other, pushed_val), 0))
        goto err;

    res = 1;

 err:
    sk_OSSL_CMP_ITAV_pop_free(itavs, OSSL_CMP_ITAV_free);
    return res;
}

static int test_HDR_generalInfo_push1_items(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    EXECUTE_TEST(execute_HDR_generalInfo_push1_items_test, tear_down);
    return result;
}

static int
execute_HDR_set_and_check_implicitConfirm_test(CMP_HDR_TEST_FIXTURE
                                               * fixture)
{
    return TEST_false(ossl_cmp_hdr_has_implicitConfirm(fixture->hdr))
        && TEST_true(ossl_cmp_hdr_set_implicitConfirm(fixture->hdr))
        && TEST_true(ossl_cmp_hdr_has_implicitConfirm(fixture->hdr));
}

static int test_HDR_set_and_check_implicit_confirm(void)
{
    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    EXECUTE_TEST(execute_HDR_set_and_check_implicitConfirm_test, tear_down);
    return result;
}

static int execute_HDR_init_test(CMP_HDR_TEST_FIXTURE *fixture)
{
    ASN1_OCTET_STRING *header_nonce, *header_transactionID;
    ASN1_OCTET_STRING *ctx_nonce;

    if (!TEST_int_eq(fixture->expected,
                     ossl_cmp_hdr_init(fixture->cmp_ctx, fixture->hdr)))
        return 0;
    if (fixture->expected == 0)
        return 1;

    if (!TEST_int_eq(ossl_cmp_hdr_get_pvno(fixture->hdr), OSSL_CMP_PVNO))
        return 0;

    header_nonce = ossl_cmp_hdr_get0_senderNonce(fixture->hdr);
    if (!TEST_int_eq(0, ASN1_OCTET_STRING_cmp(header_nonce,
                                              fixture->cmp_ctx->senderNonce)))
        return 0;
    header_transactionID = OSSL_CMP_HDR_get0_transactionID(fixture->hdr);
    if (!TEST_true(ASN1_OCTET_STRING_cmp(header_transactionID,
                                         fixture->cmp_ctx->transactionID) == 0))
        return 0;

    header_nonce = OSSL_CMP_HDR_get0_recipNonce(fixture->hdr);
    ctx_nonce = fixture->cmp_ctx->recipNonce;
    if (ctx_nonce != NULL
            && (!TEST_ptr(header_nonce)
                    || !TEST_int_eq(0, ASN1_OCTET_STRING_cmp(header_nonce,
                                                             ctx_nonce))))
        return 0;

    return 1;
}

static int test_HDR_init_with_ref(void)
{
    unsigned char ref[CMP_TEST_REFVALUE_LENGTH];

    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);

    fixture->expected = 1;
    if (!TEST_int_eq(1, RAND_bytes(ref, sizeof(ref)))
            || !TEST_true(OSSL_CMP_CTX_set1_referenceValue(fixture->cmp_ctx,
                                                           ref, sizeof(ref)))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_TEST(execute_HDR_init_test, tear_down);
    return result;
}

static int test_HDR_init_with_subject(void)
{
    X509_NAME *subject = NULL;

    SETUP_TEST_FIXTURE(CMP_HDR_TEST_FIXTURE, set_up);
    fixture->expected = 1;
    if (!TEST_ptr(subject = X509_NAME_new())
            || !TEST_true(X509_NAME_ADD(subject, "CN", "Common Name"))
            || !TEST_true(OSSL_CMP_CTX_set1_subjectName(fixture->cmp_ctx,
                                                        subject))) {
        tear_down(fixture);
        fixture = NULL;
    }
    X509_NAME_free(subject);
    EXECUTE_TEST(execute_HDR_init_test, tear_down);
    return result;
}

void cleanup_tests(void)
{
    return;
}

int setup_tests(void)
{
    RAND_bytes(rand_data, OSSL_CMP_TRANSACTIONID_LENGTH);
    /* Message header tests */
    ADD_TEST(test_HDR_set_get_pvno);
    ADD_TEST(test_HDR_get0_senderNonce);
    ADD_TEST(test_HDR_set1_sender);
    ADD_TEST(test_HDR_set1_recipient);
    ADD_TEST(test_HDR_update_messageTime);
    ADD_TEST(test_HDR_set1_senderKID);
    ADD_TEST(test_HDR_push0_freeText);
    /* indirectly tests ossl_cmp_pkifreetext_push_str(): */
    ADD_TEST(test_HDR_push1_freeText);
    ADD_TEST(test_HDR_generalInfo_push0_item);
    ADD_TEST(test_HDR_generalInfo_push1_items);
    ADD_TEST(test_HDR_set_and_check_implicit_confirm);
    /* also tests public function OSSL_CMP_HDR_get0_transactionID(): */
    /* also tests public function OSSL_CMP_HDR_get0_recipNonce(): */
    /* also tests internal function ossl_cmp_hdr_get_pvno(): */
    ADD_TEST(test_HDR_init_with_ref);
    ADD_TEST(test_HDR_init_with_subject);
    return 1;
}
