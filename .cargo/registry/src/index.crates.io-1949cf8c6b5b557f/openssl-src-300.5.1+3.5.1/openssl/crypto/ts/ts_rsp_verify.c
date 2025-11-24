/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/objects.h>
#include <openssl/ts.h>
#include <openssl/pkcs7.h>
#include "internal/cryptlib.h"
#include "internal/sizes.h"
#include "crypto/ess.h"
#include "ts_local.h"

static int ts_verify_cert(X509_STORE *store, STACK_OF(X509) *untrusted,
                          X509 *signer, STACK_OF(X509) **chain);
static int ts_check_signing_certs(const PKCS7_SIGNER_INFO *si,
                                  const STACK_OF(X509) *chain);

static int int_ts_RESP_verify_token(TS_VERIFY_CTX *ctx,
                                    PKCS7 *token, TS_TST_INFO *tst_info);
static int ts_check_status_info(TS_RESP *response);
static char *ts_get_status_text(STACK_OF(ASN1_UTF8STRING) *text);
static int ts_check_policy(const ASN1_OBJECT *req_oid,
                           const TS_TST_INFO *tst_info);
static int ts_compute_imprint(BIO *data, TS_TST_INFO *tst_info,
                              X509_ALGOR **md_alg,
                              unsigned char **imprint, unsigned *imprint_len);
static int ts_check_imprints(X509_ALGOR *algor_a,
                             const unsigned char *imprint_a, unsigned len_a,
                             TS_TST_INFO *tst_info);
static int ts_check_nonces(const ASN1_INTEGER *a, TS_TST_INFO *tst_info);
static int ts_check_signer_name(GENERAL_NAME *tsa_name, X509 *signer);
static int ts_find_name(STACK_OF(GENERAL_NAME) *gen_names,
                        GENERAL_NAME *name);

/*
 * This must be large enough to hold all values in ts_status_text (with
 * comma separator) or all text fields in ts_failure_info (also with comma).
 */
#define TS_STATUS_BUF_SIZE      256

/*
 * Local mapping between response codes and descriptions.
 */
static const char *ts_status_text[] = {
    "granted",
    "grantedWithMods",
    "rejection",
    "waiting",
    "revocationWarning",
    "revocationNotification"
};

#define TS_STATUS_TEXT_SIZE     OSSL_NELEM(ts_status_text)

static struct {
    int code;
    const char *text;
} ts_failure_info[] = {
    {TS_INFO_BAD_ALG, "badAlg"},
    {TS_INFO_BAD_REQUEST, "badRequest"},
    {TS_INFO_BAD_DATA_FORMAT, "badDataFormat"},
    {TS_INFO_TIME_NOT_AVAILABLE, "timeNotAvailable"},
    {TS_INFO_UNACCEPTED_POLICY, "unacceptedPolicy"},
    {TS_INFO_UNACCEPTED_EXTENSION, "unacceptedExtension"},
    {TS_INFO_ADD_INFO_NOT_AVAILABLE, "addInfoNotAvailable"},
    {TS_INFO_SYSTEM_FAILURE, "systemFailure"}
};


/*-
 * This function carries out the following tasks:
 *      - Checks if there is one and only one signer.
 *      - Search for the signing certificate in 'certs' and in the response.
 *      - Check the extended key usage and key usage fields of the signer
 *      certificate (done by the path validation).
 *      - Build and validate the certificate path.
 *      - Check if the certificate path meets the requirements of the
 *      SigningCertificate ESS signed attribute.
 *      - Verify the signature value.
 *      - Returns the signer certificate in 'signer', if 'signer' is not NULL.
 */
int TS_RESP_verify_signature(PKCS7 *token, STACK_OF(X509) *certs,
                             X509_STORE *store, X509 **signer_out)
{
    STACK_OF(PKCS7_SIGNER_INFO) *sinfos = NULL;
    PKCS7_SIGNER_INFO *si;
    STACK_OF(X509) *untrusted = NULL;
    STACK_OF(X509) *signers = NULL;
    X509 *signer;
    STACK_OF(X509) *chain = NULL;
    char buf[4096];
    int i, j = 0, ret = 0;
    BIO *p7bio = NULL;

    /* Some sanity checks first. */
    if (!token) {
        ERR_raise(ERR_LIB_TS, TS_R_INVALID_NULL_POINTER);
        goto err;
    }
    if (!PKCS7_type_is_signed(token)) {
        ERR_raise(ERR_LIB_TS, TS_R_WRONG_CONTENT_TYPE);
        goto err;
    }
    sinfos = PKCS7_get_signer_info(token);
    if (!sinfos || sk_PKCS7_SIGNER_INFO_num(sinfos) != 1) {
        ERR_raise(ERR_LIB_TS, TS_R_THERE_MUST_BE_ONE_SIGNER);
        goto err;
    }
    si = sk_PKCS7_SIGNER_INFO_value(sinfos, 0);
    if (PKCS7_get_detached(token)) {
        ERR_raise(ERR_LIB_TS, TS_R_NO_CONTENT);
        goto err;
    }

    /*
     * Get hold of the signer certificate, search only internal certificates
     * if it was requested.
     */
    signers = PKCS7_get0_signers(token, certs, 0);
    if (!signers || sk_X509_num(signers) != 1)
        goto err;
    signer = sk_X509_value(signers, 0);

    untrusted = sk_X509_new_reserve(NULL, sk_X509_num(certs)
                                    + sk_X509_num(token->d.sign->cert));
    if (untrusted == NULL
            || !X509_add_certs(untrusted, certs, 0)
            || !X509_add_certs(untrusted, token->d.sign->cert, 0))
        goto err;
    if (!ts_verify_cert(store, untrusted, signer, &chain))
        goto err;
    if (!ts_check_signing_certs(si, chain))
        goto err;
    p7bio = PKCS7_dataInit(token, NULL);

    /* We now have to 'read' from p7bio to calculate digests etc. */
    while ((i = BIO_read(p7bio, buf, sizeof(buf))) > 0)
        continue;

    j = PKCS7_signatureVerify(p7bio, token, si, signer);
    if (j <= 0) {
        ERR_raise(ERR_LIB_TS, TS_R_SIGNATURE_FAILURE);
        goto err;
    }

    if (signer_out) {
        if (!X509_up_ref(signer))
            goto err;

        *signer_out = signer;
    }
    ret = 1;

 err:
    BIO_free_all(p7bio);
    sk_X509_free(untrusted);
    OSSL_STACK_OF_X509_free(chain);
    sk_X509_free(signers);

    return ret;
}

/*
 * The certificate chain is returned in chain. Caller is responsible for
 * freeing the vector.
 */
static int ts_verify_cert(X509_STORE *store, STACK_OF(X509) *untrusted,
                          X509 *signer, STACK_OF(X509) **chain)
{
    X509_STORE_CTX *cert_ctx = NULL;
    int i;
    int ret = 0;

    *chain = NULL;
    cert_ctx = X509_STORE_CTX_new();
    if (cert_ctx == NULL) {
        ERR_raise(ERR_LIB_TS, ERR_R_X509_LIB);
        goto err;
    }
    if (!X509_STORE_CTX_init(cert_ctx, store, signer, untrusted))
        goto end;
    X509_STORE_CTX_set_purpose(cert_ctx, X509_PURPOSE_TIMESTAMP_SIGN);
    i = X509_verify_cert(cert_ctx);
    if (i <= 0) {
        int j = X509_STORE_CTX_get_error(cert_ctx);
        ERR_raise_data(ERR_LIB_TS, TS_R_CERTIFICATE_VERIFY_ERROR,
                       "Verify error:%s", X509_verify_cert_error_string(j));
        goto err;
    }
    *chain = X509_STORE_CTX_get1_chain(cert_ctx);
    ret = 1;
    goto end;

err:
    ret = 0;

end:
    X509_STORE_CTX_free(cert_ctx);
    return ret;
}

static ESS_SIGNING_CERT *ossl_ess_get_signing_cert(const PKCS7_SIGNER_INFO *si)
{
    ASN1_TYPE *attr;
    const unsigned char *p;

    attr = PKCS7_get_signed_attribute(si, NID_id_smime_aa_signingCertificate);
    if (attr == NULL)
        return NULL;
    p = attr->value.sequence->data;
    return d2i_ESS_SIGNING_CERT(NULL, &p, attr->value.sequence->length);
}

static
ESS_SIGNING_CERT_V2 *ossl_ess_get_signing_cert_v2(const PKCS7_SIGNER_INFO *si)
{
    ASN1_TYPE *attr;
    const unsigned char *p;

    attr = PKCS7_get_signed_attribute(si, NID_id_smime_aa_signingCertificateV2);
    if (attr == NULL)
        return NULL;
    p = attr->value.sequence->data;
    return d2i_ESS_SIGNING_CERT_V2(NULL, &p, attr->value.sequence->length);
}

static int ts_check_signing_certs(const PKCS7_SIGNER_INFO *si,
                                  const STACK_OF(X509) *chain)
{
    ESS_SIGNING_CERT *ss = ossl_ess_get_signing_cert(si);
    ESS_SIGNING_CERT_V2 *ssv2 = ossl_ess_get_signing_cert_v2(si);
    int ret = OSSL_ESS_check_signing_certs(ss, ssv2, chain, 1) > 0;

    ESS_SIGNING_CERT_free(ss);
    ESS_SIGNING_CERT_V2_free(ssv2);
    return ret;
}

/*-
 * Verifies whether 'response' contains a valid response with regards
 * to the settings of the context:
 *      - Gives an error message if the TS_TST_INFO is not present.
 *      - Calls _TS_RESP_verify_token to verify the token content.
 */
int TS_RESP_verify_response(TS_VERIFY_CTX *ctx, TS_RESP *response)
{
    PKCS7 *token = response->token;
    TS_TST_INFO *tst_info = response->tst_info;
    int ret = 0;

    if (!ts_check_status_info(response))
        goto err;
    if (!int_ts_RESP_verify_token(ctx, token, tst_info))
        goto err;
    ret = 1;

 err:
    return ret;
}

/*
 * Tries to extract a TS_TST_INFO structure from the PKCS7 token and
 * calls the internal int_TS_RESP_verify_token function for verifying it.
 */
int TS_RESP_verify_token(TS_VERIFY_CTX *ctx, PKCS7 *token)
{
    TS_TST_INFO *tst_info = PKCS7_to_TS_TST_INFO(token);
    int ret = 0;
    if (tst_info) {
        ret = int_ts_RESP_verify_token(ctx, token, tst_info);
        TS_TST_INFO_free(tst_info);
    }
    return ret;
}

/*-
 * Verifies whether the 'token' contains a valid timestamp token
 * with regards to the settings of the context. Only those checks are
 * carried out that are specified in the context:
 *      - Verifies the signature of the TS_TST_INFO.
 *      - Checks the version number of the response.
 *      - Check if the requested and returned policies math.
 *      - Check if the message imprints are the same.
 *      - Check if the nonces are the same.
 *      - Check if the TSA name matches the signer.
 *      - Check if the TSA name is the expected TSA.
 */
static int int_ts_RESP_verify_token(TS_VERIFY_CTX *ctx,
                                    PKCS7 *token, TS_TST_INFO *tst_info)
{
    X509 *signer = NULL;
    GENERAL_NAME *tsa_name = tst_info->tsa;
    X509_ALGOR *md_alg = NULL;
    unsigned char *imprint = NULL;
    unsigned imprint_len = 0;
    int ret = 0;
    int flags = ctx->flags;

    /* Some options require us to also check the signature */
    if (((flags & TS_VFY_SIGNER) && tsa_name != NULL)
            || (flags & TS_VFY_TSA_NAME)) {
        flags |= TS_VFY_SIGNATURE;
    }

    if ((flags & TS_VFY_SIGNATURE)
        && !TS_RESP_verify_signature(token, ctx->certs, ctx->store, &signer))
        goto err;
    if ((flags & TS_VFY_VERSION)
        && TS_TST_INFO_get_version(tst_info) != 1) {
        ERR_raise(ERR_LIB_TS, TS_R_UNSUPPORTED_VERSION);
        goto err;
    }
    if ((flags & TS_VFY_POLICY)
        && !ts_check_policy(ctx->policy, tst_info))
        goto err;
    if ((flags & TS_VFY_IMPRINT)
        && !ts_check_imprints(ctx->md_alg, ctx->imprint, ctx->imprint_len,
                              tst_info))
        goto err;
    if ((flags & TS_VFY_DATA)
        && (!ts_compute_imprint(ctx->data, tst_info,
                                &md_alg, &imprint, &imprint_len)
            || !ts_check_imprints(md_alg, imprint, imprint_len, tst_info)))
        goto err;
    if ((flags & TS_VFY_NONCE)
        && !ts_check_nonces(ctx->nonce, tst_info))
        goto err;
    if ((flags & TS_VFY_SIGNER)
        && tsa_name && !ts_check_signer_name(tsa_name, signer)) {
        ERR_raise(ERR_LIB_TS, TS_R_TSA_NAME_MISMATCH);
        goto err;
    }
    if ((flags & TS_VFY_TSA_NAME)
        && !ts_check_signer_name(ctx->tsa_name, signer)) {
        ERR_raise(ERR_LIB_TS, TS_R_TSA_UNTRUSTED);
        goto err;
    }
    ret = 1;

 err:
    X509_free(signer);
    X509_ALGOR_free(md_alg);
    OPENSSL_free(imprint);
    return ret;
}

static int ts_check_status_info(TS_RESP *response)
{
    TS_STATUS_INFO *info = response->status_info;
    long status = ASN1_INTEGER_get(info->status);
    const char *status_text = NULL;
    char *embedded_status_text = NULL;
    char failure_text[TS_STATUS_BUF_SIZE] = "";

    if (status == 0 || status == 1)
        return 1;

    /* There was an error, get the description in status_text. */
    if (0 <= status && status < (long) OSSL_NELEM(ts_status_text))
        status_text = ts_status_text[status];
    else
        status_text = "unknown code";

    if (sk_ASN1_UTF8STRING_num(info->text) > 0
        && (embedded_status_text = ts_get_status_text(info->text)) == NULL)
        return 0;

    /* Fill in failure_text with the failure information. */
    if (info->failure_info) {
        int i;
        int first = 1;
        for (i = 0; i < (int)OSSL_NELEM(ts_failure_info); ++i) {
            if (ASN1_BIT_STRING_get_bit(info->failure_info,
                                        ts_failure_info[i].code)) {
                if (!first)
                    strcat(failure_text, ",");
                else
                    first = 0;
                strcat(failure_text, ts_failure_info[i].text);
            }
        }
    }
    if (failure_text[0] == '\0')
        strcpy(failure_text, "unspecified");

    ERR_raise_data(ERR_LIB_TS, TS_R_NO_TIME_STAMP_TOKEN,
                   "status code: %s, status text: %s, failure codes: %s",
                   status_text,
                   embedded_status_text ? embedded_status_text : "unspecified",
                   failure_text);
    OPENSSL_free(embedded_status_text);

    return 0;
}

static char *ts_get_status_text(STACK_OF(ASN1_UTF8STRING) *text)
{
    return ossl_sk_ASN1_UTF8STRING2text(text, "/", TS_MAX_STATUS_LENGTH);
}

static int ts_check_policy(const ASN1_OBJECT *req_oid,
                           const TS_TST_INFO *tst_info)
{
    const ASN1_OBJECT *resp_oid = tst_info->policy_id;

    if (OBJ_cmp(req_oid, resp_oid) != 0) {
        ERR_raise(ERR_LIB_TS, TS_R_POLICY_MISMATCH);
        return 0;
    }

    return 1;
}

static int ts_compute_imprint(BIO *data, TS_TST_INFO *tst_info,
                              X509_ALGOR **md_alg,
                              unsigned char **imprint, unsigned *imprint_len)
{
    TS_MSG_IMPRINT *msg_imprint = tst_info->msg_imprint;
    X509_ALGOR *md_alg_resp = msg_imprint->hash_algo;
    EVP_MD *md = NULL;
    EVP_MD_CTX *md_ctx = NULL;
    unsigned char buffer[4096];
    char name[OSSL_MAX_NAME_SIZE];
    int length;

    *md_alg = NULL;
    *imprint = NULL;

    if ((*md_alg = X509_ALGOR_dup(md_alg_resp)) == NULL)
        goto err;

    OBJ_obj2txt(name, sizeof(name), md_alg_resp->algorithm, 0);

    (void)ERR_set_mark();
    md = EVP_MD_fetch(NULL, name, NULL);

    if (md == NULL)
        md = (EVP_MD *)EVP_get_digestbyname(name);

    if (md == NULL) {
        (void)ERR_clear_last_mark();
        goto err;
    }
    (void)ERR_pop_to_mark();

    length = EVP_MD_get_size(md);
    if (length <= 0)
        goto err;
    *imprint_len = length;
    if ((*imprint = OPENSSL_malloc(*imprint_len)) == NULL)
        goto err;

    md_ctx = EVP_MD_CTX_new();
    if (md_ctx == NULL) {
        ERR_raise(ERR_LIB_TS, ERR_R_EVP_LIB);
        goto err;
    }
    if (!EVP_DigestInit(md_ctx, md))
        goto err;
    EVP_MD_free(md);
    md = NULL;
    while ((length = BIO_read(data, buffer, sizeof(buffer))) > 0) {
        if (!EVP_DigestUpdate(md_ctx, buffer, length))
            goto err;
    }
    if (!EVP_DigestFinal(md_ctx, *imprint, NULL))
        goto err;
    EVP_MD_CTX_free(md_ctx);

    return 1;
 err:
    EVP_MD_CTX_free(md_ctx);
    EVP_MD_free(md);
    X509_ALGOR_free(*md_alg);
    *md_alg = NULL;
    OPENSSL_free(*imprint);
    *imprint_len = 0;
    *imprint = 0;
    return 0;
}

static int ts_check_imprints(X509_ALGOR *algor_a,
                             const unsigned char *imprint_a, unsigned len_a,
                             TS_TST_INFO *tst_info)
{
    TS_MSG_IMPRINT *b = tst_info->msg_imprint;
    X509_ALGOR *algor_b = b->hash_algo;
    int ret = 0;

    if (algor_a) {
        if (OBJ_cmp(algor_a->algorithm, algor_b->algorithm))
            goto err;

        /* The parameter must be NULL in both. */
        if ((algor_a->parameter
             && ASN1_TYPE_get(algor_a->parameter) != V_ASN1_NULL)
            || (algor_b->parameter
                && ASN1_TYPE_get(algor_b->parameter) != V_ASN1_NULL))
            goto err;
    }

    ret = len_a == (unsigned)ASN1_STRING_length(b->hashed_msg) &&
        memcmp(imprint_a, ASN1_STRING_get0_data(b->hashed_msg), len_a) == 0;
 err:
    if (!ret)
        ERR_raise(ERR_LIB_TS, TS_R_MESSAGE_IMPRINT_MISMATCH);
    return ret;
}

static int ts_check_nonces(const ASN1_INTEGER *a, TS_TST_INFO *tst_info)
{
    const ASN1_INTEGER *b = tst_info->nonce;

    if (!b) {
        ERR_raise(ERR_LIB_TS, TS_R_NONCE_NOT_RETURNED);
        return 0;
    }

    /* No error if a nonce is returned without being requested. */
    if (ASN1_INTEGER_cmp(a, b) != 0) {
        ERR_raise(ERR_LIB_TS, TS_R_NONCE_MISMATCH);
        return 0;
    }

    return 1;
}

/*
 * Check if the specified TSA name matches either the subject or one of the
 * subject alternative names of the TSA certificate.
 */
static int ts_check_signer_name(GENERAL_NAME *tsa_name, X509 *signer)
{
    STACK_OF(GENERAL_NAME) *gen_names = NULL;
    int idx = -1;
    int found = 0;

    if (tsa_name->type == GEN_DIRNAME
        && X509_name_cmp(tsa_name->d.dirn, X509_get_subject_name(signer)) == 0)
        return 1;
    gen_names = X509_get_ext_d2i(signer, NID_subject_alt_name, NULL, &idx);
    while (gen_names != NULL) {
        found = ts_find_name(gen_names, tsa_name) >= 0;
        if (found)
            break;
        /*
         * Get the next subject alternative name, although there should be no
         * more than one.
         */
        GENERAL_NAMES_free(gen_names);
        gen_names = X509_get_ext_d2i(signer, NID_subject_alt_name, NULL, &idx);
    }
    GENERAL_NAMES_free(gen_names);

    return found;
}

/* Returns 1 if name is in gen_names, 0 otherwise. */
static int ts_find_name(STACK_OF(GENERAL_NAME) *gen_names, GENERAL_NAME *name)
{
    int i, found;
    for (i = 0, found = 0; !found && i < sk_GENERAL_NAME_num(gen_names); ++i) {
        GENERAL_NAME *current = sk_GENERAL_NAME_value(gen_names, i);
        found = GENERAL_NAME_cmp(current, name) == 0;
    }
    return found ? i - 1 : -1;
}
