/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2018-2020
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or atf
 * https://www.openssl.org/source/license.html
 */

#include "apps.h"
#include "cmp_mock_srv.h"

#include <openssl/cmp.h>
#include <openssl/err.h>
#include <openssl/cmperr.h>

/* the context for the CMP mock server */
typedef struct {
    X509 *refCert;             /* cert to expect for oldCertID in kur/rr msg */
    X509 *certOut;             /* certificate to be returned in cp/ip/kup msg */
    EVP_PKEY *keyOut;          /* Private key to be returned for central keygen */
    X509_CRL *crlOut;          /* CRL to be returned in genp for crls */
    STACK_OF(X509) *chainOut;  /* chain of certOut to add to extraCerts field */
    STACK_OF(X509) *caPubsOut; /* used in caPubs of ip and in caCerts of genp */
    X509 *newWithNew;          /* to return in newWithNew of rootKeyUpdate */
    X509 *newWithOld;          /* to return in newWithOld of rootKeyUpdate */
    X509 *oldWithNew;          /* to return in oldWithNew of rootKeyUpdate */
    OSSL_CMP_PKISI *statusOut; /* status for ip/cp/kup/rp msg unless polling */
    int sendError;             /* send error response on given request type */
    OSSL_CMP_MSG *req;         /* original request message during polling */
    int pollCount;             /* number of polls before actual cert response */
    int curr_pollCount;        /* number of polls so far for current request */
    int checkAfterTime;        /* time the client should wait between polling */
} mock_srv_ctx;

static void mock_srv_ctx_free(mock_srv_ctx *ctx)
{
    if (ctx == NULL)
        return;

    OSSL_CMP_PKISI_free(ctx->statusOut);
    X509_free(ctx->refCert);
    X509_free(ctx->certOut);
    OSSL_STACK_OF_X509_free(ctx->chainOut);
    OSSL_STACK_OF_X509_free(ctx->caPubsOut);
    OSSL_CMP_MSG_free(ctx->req);
    OPENSSL_free(ctx);
}

static mock_srv_ctx *mock_srv_ctx_new(void)
{
    mock_srv_ctx *ctx = OPENSSL_zalloc(sizeof(mock_srv_ctx));

    if (ctx == NULL)
        goto err;

    if ((ctx->statusOut = OSSL_CMP_PKISI_new()) == NULL)
        goto err;

    ctx->sendError = -1;

    /* all other elements are initialized to 0 or NULL, respectively */
    return ctx;
 err:
    mock_srv_ctx_free(ctx);
    return NULL;
}

#define DEFINE_OSSL_SET1_CERT(FIELD) \
    int ossl_cmp_mock_srv_set1_##FIELD(OSSL_CMP_SRV_CTX *srv_ctx, \
                                       X509 *cert) \
    { \
        mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx); \
 \
        if (ctx == NULL) { \
            ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
            return 0; \
        } \
        if (cert == NULL || X509_up_ref(cert)) { \
            X509_free(ctx->FIELD); \
            ctx->FIELD = cert; \
            return 1; \
        } \
        return 0; \
    }

DEFINE_OSSL_SET1_CERT(refCert)
DEFINE_OSSL_SET1_CERT(certOut)

int ossl_cmp_mock_srv_set1_keyOut(OSSL_CMP_SRV_CTX *srv_ctx, EVP_PKEY *pkey)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (pkey != NULL && !EVP_PKEY_up_ref(pkey))
        return 0;
    EVP_PKEY_free(ctx->keyOut);
    ctx->keyOut = pkey;
    return 1;
}

int ossl_cmp_mock_srv_set1_crlOut(OSSL_CMP_SRV_CTX *srv_ctx,
                                  X509_CRL *crl)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (crl != NULL && !X509_CRL_up_ref(crl))
        return 0;
    X509_CRL_free(ctx->crlOut);
    ctx->crlOut = crl;
    return 1;
}

int ossl_cmp_mock_srv_set1_chainOut(OSSL_CMP_SRV_CTX *srv_ctx,
                                    STACK_OF(X509) *chain)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    STACK_OF(X509) *chain_copy = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (chain != NULL && (chain_copy = X509_chain_up_ref(chain)) == NULL)
        return 0;
    OSSL_STACK_OF_X509_free(ctx->chainOut);
    ctx->chainOut = chain_copy;
    return 1;
}

int ossl_cmp_mock_srv_set1_caPubsOut(OSSL_CMP_SRV_CTX *srv_ctx,
                                     STACK_OF(X509) *caPubs)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    STACK_OF(X509) *caPubs_copy = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (caPubs != NULL && (caPubs_copy = X509_chain_up_ref(caPubs)) == NULL)
        return 0;
    OSSL_STACK_OF_X509_free(ctx->caPubsOut);
    ctx->caPubsOut = caPubs_copy;
    return 1;
}

DEFINE_OSSL_SET1_CERT(newWithNew)
DEFINE_OSSL_SET1_CERT(newWithOld)
DEFINE_OSSL_SET1_CERT(oldWithNew)

int ossl_cmp_mock_srv_set_statusInfo(OSSL_CMP_SRV_CTX *srv_ctx, int status,
                                     int fail_info, const char *text)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    OSSL_CMP_PKISI *si;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if ((si = OSSL_CMP_STATUSINFO_new(status, fail_info, text)) == NULL)
        return 0;
    OSSL_CMP_PKISI_free(ctx->statusOut);
    ctx->statusOut = si;
    return 1;
}

int ossl_cmp_mock_srv_set_sendError(OSSL_CMP_SRV_CTX *srv_ctx, int bodytype)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    /* might check bodytype, but this would require exporting all body types */
    ctx->sendError = bodytype;
    return 1;
}

int ossl_cmp_mock_srv_set_pollCount(OSSL_CMP_SRV_CTX *srv_ctx, int count)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (count < 0) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return 0;
    }
    ctx->pollCount = count;
    return 1;
}

int ossl_cmp_mock_srv_set_checkAfterTime(OSSL_CMP_SRV_CTX *srv_ctx, int sec)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    ctx->checkAfterTime = sec;
    return 1;
}

/* determine whether to delay response to (non-polling) request */
static int delayed_delivery(OSSL_CMP_SRV_CTX *srv_ctx, const OSSL_CMP_MSG *req)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    int req_type = OSSL_CMP_MSG_get_bodytype(req);

    if (ctx == NULL || req == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return -1;
    }

    /*
     * For ir/cr/p10cr/kur delayed delivery is handled separately in
     * process_cert_request
     */
    if (req_type == OSSL_CMP_IR
        || req_type == OSSL_CMP_CR
        || req_type == OSSL_CMP_P10CR
        || req_type == OSSL_CMP_KUR
        /* Client may use error to abort the ongoing polling */
        || req_type == OSSL_CMP_ERROR)
        return 0;

    if (ctx->pollCount > 0 && ctx->curr_pollCount == 0) {
        /* start polling */
        if ((ctx->req = OSSL_CMP_MSG_dup(req)) == NULL)
            return -1;
        return 1;
    }
    return 0;
}

/* check for matching reference cert components, as far as given */
static int refcert_cmp(const X509 *refcert,
                       const X509_NAME *issuer, const ASN1_INTEGER *serial)
{
    const X509_NAME *ref_issuer;
    const ASN1_INTEGER *ref_serial;

    if (refcert == NULL)
        return 1;
    ref_issuer = X509_get_issuer_name(refcert);
    ref_serial = X509_get0_serialNumber(refcert);
    return (ref_issuer == NULL || X509_NAME_cmp(issuer, ref_issuer) == 0)
        && (ref_serial == NULL || ASN1_INTEGER_cmp(serial, ref_serial) == 0);
}

/* reset the state that belongs to a transaction */
static int clean_transaction(OSSL_CMP_SRV_CTX *srv_ctx,
                             ossl_unused const ASN1_OCTET_STRING *id)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    ctx->curr_pollCount = 0;
    OSSL_CMP_MSG_free(ctx->req);
    ctx->req = NULL;
    return 1;
}

static OSSL_CMP_PKISI *process_cert_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                            const OSSL_CMP_MSG *cert_req,
                                            ossl_unused int certReqId,
                                            const OSSL_CRMF_MSG *crm,
                                            const X509_REQ *p10cr,
                                            X509 **certOut,
                                            STACK_OF(X509) **chainOut,
                                            STACK_OF(X509) **caPubs)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    int bodytype, central_keygen;
    OSSL_CMP_PKISI *si = NULL;
    EVP_PKEY *keyOut = NULL;

    if (ctx == NULL || cert_req == NULL
            || certOut == NULL || chainOut == NULL || caPubs == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    bodytype = OSSL_CMP_MSG_get_bodytype(cert_req);
    if (ctx->sendError == 1 || ctx->sendError == bodytype) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return NULL;
    }

    *certOut = NULL;
    *chainOut = NULL;
    *caPubs = NULL;

    if (ctx->pollCount > 0 && ctx->curr_pollCount == 0) {
        /* start polling */
        if ((ctx->req = OSSL_CMP_MSG_dup(cert_req)) == NULL)
            return NULL;
        return OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_waiting, 0, NULL);
    }
    if (ctx->curr_pollCount >= ctx->pollCount)
        /* give final response after polling */
        ctx->curr_pollCount = 0;

    /* accept cert profile for cr messages only with the configured name */
    if (OSSL_CMP_MSG_get_bodytype(cert_req) == OSSL_CMP_CR) {
        STACK_OF(OSSL_CMP_ITAV) *itavs =
            OSSL_CMP_HDR_get0_geninfo_ITAVs(OSSL_CMP_MSG_get0_header(cert_req));
        int i;

        for (i = 0; i < sk_OSSL_CMP_ITAV_num(itavs); i++) {
            OSSL_CMP_ITAV *itav = sk_OSSL_CMP_ITAV_value(itavs, i);
            ASN1_OBJECT *obj = OSSL_CMP_ITAV_get0_type(itav);
            STACK_OF(ASN1_UTF8STRING) *strs;
            ASN1_UTF8STRING *str;
            const char *data;

            if (OBJ_obj2nid(obj) == NID_id_it_certProfile) {
                if (!OSSL_CMP_ITAV_get0_certProfile(itav, &strs))
                    return NULL;
                if (sk_ASN1_UTF8STRING_num(strs) < 1) {
                    ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_CERTPROFILE);
                    return NULL;
                }
                str = sk_ASN1_UTF8STRING_value(strs, 0);
                if (str == NULL
                    || (data =
                        (const char *)ASN1_STRING_get0_data(str)) == NULL) {
                    ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_INVALID_ARGUMENT);
                    return NULL;
                }
                if (strcmp(data, "profile1") != 0) {
                    ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_CERTPROFILE);
                    return NULL;
                }
                break;
            }
        }
    }

    /* accept cert update request only for the reference cert, if given */
    if (bodytype == OSSL_CMP_KUR
            && crm != NULL /* thus not p10cr */ && ctx->refCert != NULL) {
        const OSSL_CRMF_CERTID *cid = OSSL_CRMF_MSG_get0_regCtrl_oldCertID(crm);

        if (cid == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_CERTID);
            return NULL;
        }
        if (!refcert_cmp(ctx->refCert,
                         OSSL_CRMF_CERTID_get0_issuer(cid),
                         OSSL_CRMF_CERTID_get0_serialNumber(cid))) {
            ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_CERTID);
            return NULL;
        }
    }

    if (ctx->certOut != NULL
            && (*certOut = X509_dup(ctx->certOut)) == NULL)
        /* Should return a cert produced from request template, see FR #16054 */
        goto err;

    central_keygen = OSSL_CRMF_MSG_centralkeygen_requested(crm, p10cr);
    if (central_keygen < 0)
        goto err;
    if (central_keygen == 1
        && (ctx->keyOut == NULL
            || (keyOut = EVP_PKEY_dup(ctx->keyOut)) == NULL
            || !OSSL_CMP_CTX_set0_newPkey(OSSL_CMP_SRV_CTX_get0_cmp_ctx(srv_ctx),
                                          1 /* priv */, keyOut))) {
        EVP_PKEY_free(keyOut);
        goto err;
    }
    /*
     * Note that this uses newPkey to return the private key
     * and does not check whether the 'popo' field is absent.
     */

    if (ctx->chainOut != NULL
            && (*chainOut = X509_chain_up_ref(ctx->chainOut)) == NULL)
        goto err;
    if (ctx->caPubsOut != NULL /* OSSL_CMP_PKIBODY_IP not visible here */
            && (*caPubs = X509_chain_up_ref(ctx->caPubsOut)) == NULL)
        goto err;
    if (ctx->statusOut != NULL
            && (si = OSSL_CMP_PKISI_dup(ctx->statusOut)) == NULL)
        goto err;
    return si;

 err:
    X509_free(*certOut);
    *certOut = NULL;
    OSSL_STACK_OF_X509_free(*chainOut);
    *chainOut = NULL;
    OSSL_STACK_OF_X509_free(*caPubs);
    *caPubs = NULL;
    return NULL;
}

static OSSL_CMP_PKISI *process_rr(OSSL_CMP_SRV_CTX *srv_ctx,
                                  const OSSL_CMP_MSG *rr,
                                  const X509_NAME *issuer,
                                  const ASN1_INTEGER *serial)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL || rr == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    if (ctx->sendError == 1
            || ctx->sendError == OSSL_CMP_MSG_get_bodytype(rr)) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return NULL;
    }

    /* allow any RR derived from CSR which does not include issuer and serial */
    if ((issuer != NULL || serial != NULL)
        /* accept revocation only for the reference cert, if given */
            && !refcert_cmp(ctx->refCert, issuer, serial)) {
        ERR_raise_data(ERR_LIB_CMP, CMP_R_REQUEST_NOT_ACCEPTED,
                       "wrong certificate to revoke");
        return NULL;
    }
    return OSSL_CMP_PKISI_dup(ctx->statusOut);
}

/* return -1 for error, 0 for no update available */
static int check_client_crl(const STACK_OF(OSSL_CMP_CRLSTATUS) *crlStatusList,
                            const X509_CRL *crl)
{
    OSSL_CMP_CRLSTATUS *crlstatus;
    DIST_POINT_NAME *dpn = NULL;
    GENERAL_NAMES *issuer = NULL;
    ASN1_TIME *thisupd = NULL;

    if (sk_OSSL_CMP_CRLSTATUS_num(crlStatusList) != 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_CRLSTATUSLIST);
        return -1;
    }
    if (crl == NULL)
        return 0;

    crlstatus = sk_OSSL_CMP_CRLSTATUS_value(crlStatusList, 0);
    if (!OSSL_CMP_CRLSTATUS_get0(crlstatus, &dpn, &issuer, &thisupd))
        return -1;

    if (issuer != NULL) {
        GENERAL_NAME *gn = sk_GENERAL_NAME_value(issuer, 0);

        if (gn != NULL && gn->type == GEN_DIRNAME) {
            X509_NAME *gen_name = gn->d.dirn;

            if (X509_NAME_cmp(gen_name, X509_CRL_get_issuer(crl)) != 0) {
                ERR_raise(ERR_LIB_CMP, CMP_R_UNKNOWN_CRL_ISSUER);
                return -1;
            }
        } else {  
            ERR_raise(ERR_LIB_CMP, CMP_R_SENDER_GENERALNAME_TYPE_NOT_SUPPORTED);  
            return -1; /* error according to RFC 9483 section 4.3.4 */  
        }
    }

    return thisupd == NULL
        || ASN1_TIME_compare(thisupd, X509_CRL_get0_lastUpdate(crl)) < 0;
}

static OSSL_CMP_ITAV *process_genm_itav(mock_srv_ctx *ctx, int req_nid,
                                        const OSSL_CMP_ITAV *req)
{
    OSSL_CMP_ITAV *rsp = NULL;

    switch (req_nid) {
    case NID_id_it_caCerts:
        rsp = OSSL_CMP_ITAV_new_caCerts(ctx->caPubsOut);
        break;
    case NID_id_it_rootCaCert:
        {
            X509 *rootcacert = NULL;

            if (!OSSL_CMP_ITAV_get0_rootCaCert(req, &rootcacert))
                return NULL;

            if (rootcacert != NULL
                && X509_NAME_cmp(X509_get_subject_name(rootcacert),
                                 X509_get_subject_name(ctx->newWithNew)) != 0)
                /* The subjects do not match */
                rsp = OSSL_CMP_ITAV_new_rootCaKeyUpdate(NULL, NULL, NULL);
            else
                rsp = OSSL_CMP_ITAV_new_rootCaKeyUpdate(ctx->newWithNew,
                                                        ctx->newWithOld,
                                                        ctx->oldWithNew);
        }
        break;
    case NID_id_it_crlStatusList:
        {
            STACK_OF(OSSL_CMP_CRLSTATUS) *crlstatuslist = NULL;
            int res = 0;

            if (!OSSL_CMP_ITAV_get0_crlStatusList(req, &crlstatuslist))
                return NULL;

            res = check_client_crl(crlstatuslist, ctx->crlOut);
            if (res < 0)
                rsp = NULL;
            else
                rsp = OSSL_CMP_ITAV_new_crls(res == 0 ? NULL : ctx->crlOut);
        }
        break;
    case NID_id_it_certReqTemplate:
        {
            OSSL_CRMF_CERTTEMPLATE *reqtemp;
            OSSL_CMP_ATAVS *keyspec = NULL;
            X509_ALGOR *keyalg = NULL;
            OSSL_CMP_ATAV *rsakeylen, *eckeyalg;
            int ok = 0;

            if ((reqtemp = OSSL_CRMF_CERTTEMPLATE_new()) == NULL)
                return NULL;

            if (!OSSL_CRMF_CERTTEMPLATE_fill(reqtemp, NULL, NULL,
                                             X509_get_issuer_name(ctx->refCert),
                                             NULL))
                goto crt_err;

            if ((keyalg = X509_ALGOR_new()) == NULL)
                goto crt_err;

            (void)X509_ALGOR_set0(keyalg, OBJ_nid2obj(NID_X9_62_id_ecPublicKey),
                                  V_ASN1_UNDEF, NULL); /* cannot fail */

            eckeyalg = OSSL_CMP_ATAV_new_algId(keyalg);
            rsakeylen = OSSL_CMP_ATAV_new_rsaKeyLen(4096);
            ok = OSSL_CMP_ATAV_push1(&keyspec, eckeyalg)
                 && OSSL_CMP_ATAV_push1(&keyspec, rsakeylen);
            OSSL_CMP_ATAV_free(eckeyalg);
            OSSL_CMP_ATAV_free(rsakeylen);
            X509_ALGOR_free(keyalg);

            if (!ok)
                goto crt_err;

            rsp = OSSL_CMP_ITAV_new0_certReqTemplate(reqtemp, keyspec);
            return rsp;

        crt_err:
            OSSL_CRMF_CERTTEMPLATE_free(reqtemp);
            OSSL_CMP_ATAVS_free(keyspec);
            return NULL;
        }
        break;
    default:
        rsp = OSSL_CMP_ITAV_dup(req);
    }
    return rsp;
}

static int process_genm(OSSL_CMP_SRV_CTX *srv_ctx,
                        const OSSL_CMP_MSG *genm,
                        const STACK_OF(OSSL_CMP_ITAV) *in,
                        STACK_OF(OSSL_CMP_ITAV) **out)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (ctx == NULL || genm == NULL || in == NULL || out == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (ctx->sendError == 1
            || ctx->sendError == OSSL_CMP_MSG_get_bodytype(genm)
            || sk_OSSL_CMP_ITAV_num(in) > 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return 0;
    }
    if (sk_OSSL_CMP_ITAV_num(in) == 1) {
        OSSL_CMP_ITAV *req = sk_OSSL_CMP_ITAV_value(in, 0), *rsp;
        ASN1_OBJECT *obj = OSSL_CMP_ITAV_get0_type(req);

        if ((*out = sk_OSSL_CMP_ITAV_new_reserve(NULL, 1)) == NULL)
            return 0;
        rsp = process_genm_itav(ctx, OBJ_obj2nid(obj), req);
        if (rsp != NULL && sk_OSSL_CMP_ITAV_push(*out, rsp))
            return 1;
        sk_OSSL_CMP_ITAV_free(*out);
        return 0;
    }

    *out = sk_OSSL_CMP_ITAV_deep_copy(in, OSSL_CMP_ITAV_dup,
                                      OSSL_CMP_ITAV_free);
    return *out != NULL;
}

static void process_error(OSSL_CMP_SRV_CTX *srv_ctx, const OSSL_CMP_MSG *error,
                          const OSSL_CMP_PKISI *statusInfo,
                          const ASN1_INTEGER *errorCode,
                          const OSSL_CMP_PKIFREETEXT *errorDetails)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    char buf[OSSL_CMP_PKISI_BUFLEN];
    char *sibuf;
    int i;

    if (ctx == NULL || error == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return;
    }

    BIO_printf(bio_err, "mock server received error:\n");

    if (statusInfo == NULL) {
        BIO_printf(bio_err, "pkiStatusInfo absent\n");
    } else {
        sibuf = OSSL_CMP_snprint_PKIStatusInfo(statusInfo, buf, sizeof(buf));
        BIO_printf(bio_err, "pkiStatusInfo: %s\n",
                   sibuf != NULL ? sibuf: "<invalid>");
    }

    if (errorCode == NULL)
        BIO_printf(bio_err, "errorCode absent\n");
    else
        BIO_printf(bio_err, "errorCode: %ld\n", ASN1_INTEGER_get(errorCode));

    if (sk_ASN1_UTF8STRING_num(errorDetails) <= 0) {
        BIO_printf(bio_err, "errorDetails absent\n");
    } else {
        BIO_printf(bio_err, "errorDetails: ");
        for (i = 0; i < sk_ASN1_UTF8STRING_num(errorDetails); i++) {
            if (i > 0)
                BIO_printf(bio_err, ", ");
            ASN1_STRING_print_ex(bio_err,
                                 sk_ASN1_UTF8STRING_value(errorDetails, i),
                                 ASN1_STRFLGS_ESC_QUOTE);
        }
        BIO_printf(bio_err, "\n");
    }
}

static int process_certConf(OSSL_CMP_SRV_CTX *srv_ctx,
                            const OSSL_CMP_MSG *certConf,
                            ossl_unused int certReqId,
                            const ASN1_OCTET_STRING *certHash,
                            const OSSL_CMP_PKISI *si)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);
    ASN1_OCTET_STRING *digest;

    if (ctx == NULL || certConf == NULL || certHash == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (ctx->sendError == 1
            || ctx->sendError == OSSL_CMP_MSG_get_bodytype(certConf)
            || ctx->certOut == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return 0;
    }

    if ((digest = X509_digest_sig(ctx->certOut, NULL, NULL)) == NULL)
        return 0;
    if (ASN1_OCTET_STRING_cmp(certHash, digest) != 0) {
        ASN1_OCTET_STRING_free(digest);
        ERR_raise(ERR_LIB_CMP, CMP_R_CERTHASH_UNMATCHED);
        return 0;
    }
    ASN1_OCTET_STRING_free(digest);
    return 1;
}

/* return 0 on failure, 1 on success, setting *req or otherwise *check_after */
static int process_pollReq(OSSL_CMP_SRV_CTX *srv_ctx,
                           const OSSL_CMP_MSG *pollReq,
                           ossl_unused int certReqId,
                           OSSL_CMP_MSG **req, int64_t *check_after)
{
    mock_srv_ctx *ctx = OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx);

    if (req != NULL)
        *req = NULL;
    if (ctx == NULL || pollReq == NULL
            || req == NULL || check_after == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (ctx->sendError == 1
            || ctx->sendError == OSSL_CMP_MSG_get_bodytype(pollReq)) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return 0;
    }
    if (ctx->req == NULL) { /* not currently in polling mode */
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_POLLREQ);
        return 0;
    }

    if (++ctx->curr_pollCount >= ctx->pollCount) {
        /* end polling */
        *req = ctx->req;
        ctx->req = NULL;
        *check_after = 0;
    } else {
        *check_after = ctx->checkAfterTime;
    }
    return 1;
}

OSSL_CMP_SRV_CTX *ossl_cmp_mock_srv_new(OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_CMP_SRV_CTX *srv_ctx = OSSL_CMP_SRV_CTX_new(libctx, propq);
    mock_srv_ctx *ctx = mock_srv_ctx_new();

    if (srv_ctx != NULL && ctx != NULL
            && OSSL_CMP_SRV_CTX_init(srv_ctx, ctx, process_cert_request,
                                     process_rr, process_genm, process_error,
                                     process_certConf, process_pollReq)
            && OSSL_CMP_SRV_CTX_init_trans(srv_ctx,
                                           delayed_delivery, clean_transaction))
        return srv_ctx;

    mock_srv_ctx_free(ctx);
    OSSL_CMP_SRV_CTX_free(srv_ctx);
    return NULL;
}

void ossl_cmp_mock_srv_free(OSSL_CMP_SRV_CTX *srv_ctx)
{
    if (srv_ctx != NULL)
        mock_srv_ctx_free(OSSL_CMP_SRV_CTX_get0_custom_ctx(srv_ctx));
    OSSL_CMP_SRV_CTX_free(srv_ctx);
}
