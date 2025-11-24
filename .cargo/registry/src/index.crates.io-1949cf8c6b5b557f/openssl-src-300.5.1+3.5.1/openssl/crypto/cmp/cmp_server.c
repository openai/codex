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

/* general CMP server functions */

#include <openssl/asn1t.h>

#include "cmp_local.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/cmp.h>
#include <openssl/err.h>

/* the context for the generic CMP server */
struct ossl_cmp_srv_ctx_st {
    OSSL_CMP_CTX *ctx; /* CMP client context reused for transactionID etc. */
    void *custom_ctx;  /* application-specific server context */
    int certReqId;     /* of ir/cr/kur, OSSL_CMP_CERTREQID_NONE for p10cr */
    int polling;       /* current transaction is in polling mode */

    OSSL_CMP_SRV_cert_request_cb_t process_cert_request;
    OSSL_CMP_SRV_rr_cb_t process_rr;
    OSSL_CMP_SRV_genm_cb_t process_genm;
    OSSL_CMP_SRV_error_cb_t process_error;
    OSSL_CMP_SRV_certConf_cb_t process_certConf;
    OSSL_CMP_SRV_pollReq_cb_t process_pollReq;
    OSSL_CMP_SRV_delayed_delivery_cb_t delayed_delivery;
    OSSL_CMP_SRV_clean_transaction_cb_t clean_transaction;

    int sendUnprotectedErrors; /* Send error and rejection msgs unprotected */
    int acceptUnprotected;     /* Accept requests with no/invalid prot. */
    int acceptRAVerified;      /* Accept ir/cr/kur with POPO RAVerified */
    int grantImplicitConfirm;  /* Grant implicit confirmation if requested */

}; /* OSSL_CMP_SRV_CTX */

void OSSL_CMP_SRV_CTX_free(OSSL_CMP_SRV_CTX *srv_ctx)
{
    if (srv_ctx == NULL)
        return;

    OSSL_CMP_CTX_free(srv_ctx->ctx);
    OPENSSL_free(srv_ctx);
}

OSSL_CMP_SRV_CTX *OSSL_CMP_SRV_CTX_new(OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_CMP_SRV_CTX *ctx = OPENSSL_zalloc(sizeof(OSSL_CMP_SRV_CTX));

    if (ctx == NULL)
        goto err;

    if ((ctx->ctx = OSSL_CMP_CTX_new(libctx, propq)) == NULL)
        goto err;
    ctx->certReqId = OSSL_CMP_CERTREQID_INVALID;
    ctx->polling = 0;

    /* all other elements are initialized to 0 or NULL, respectively */
    return ctx;
 err:
    OSSL_CMP_SRV_CTX_free(ctx);
    return NULL;
}

int OSSL_CMP_SRV_CTX_init(OSSL_CMP_SRV_CTX *srv_ctx, void *custom_ctx,
                          OSSL_CMP_SRV_cert_request_cb_t process_cert_request,
                          OSSL_CMP_SRV_rr_cb_t process_rr,
                          OSSL_CMP_SRV_genm_cb_t process_genm,
                          OSSL_CMP_SRV_error_cb_t process_error,
                          OSSL_CMP_SRV_certConf_cb_t process_certConf,
                          OSSL_CMP_SRV_pollReq_cb_t process_pollReq)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->custom_ctx = custom_ctx;
    srv_ctx->process_cert_request = process_cert_request;
    srv_ctx->process_rr = process_rr;
    srv_ctx->process_genm = process_genm;
    srv_ctx->process_error = process_error;
    srv_ctx->process_certConf = process_certConf;
    srv_ctx->process_pollReq = process_pollReq;
    return 1;
}

int OSSL_CMP_SRV_CTX_init_trans(OSSL_CMP_SRV_CTX *srv_ctx,
                                OSSL_CMP_SRV_delayed_delivery_cb_t delay,
                                OSSL_CMP_SRV_clean_transaction_cb_t clean)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->delayed_delivery = delay;
    srv_ctx->clean_transaction = clean;
    return 1;
}

OSSL_CMP_CTX *OSSL_CMP_SRV_CTX_get0_cmp_ctx(const OSSL_CMP_SRV_CTX *srv_ctx)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return srv_ctx->ctx;
}

void *OSSL_CMP_SRV_CTX_get0_custom_ctx(const OSSL_CMP_SRV_CTX *srv_ctx)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return srv_ctx->custom_ctx;
}

int OSSL_CMP_SRV_CTX_set_send_unprotected_errors(OSSL_CMP_SRV_CTX *srv_ctx,
                                                 int val)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->sendUnprotectedErrors = val != 0;
    return 1;
}

int OSSL_CMP_SRV_CTX_set_accept_unprotected(OSSL_CMP_SRV_CTX *srv_ctx, int val)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->acceptUnprotected = val != 0;
    return 1;
}

int OSSL_CMP_SRV_CTX_set_accept_raverified(OSSL_CMP_SRV_CTX *srv_ctx, int val)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->acceptRAVerified = val != 0;
    return 1;
}

int OSSL_CMP_SRV_CTX_set_grant_implicit_confirm(OSSL_CMP_SRV_CTX *srv_ctx,
                                                int val)
{
    if (srv_ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    srv_ctx->grantImplicitConfirm = val != 0;
    return 1;
}

/* return error msg with waiting status if polling is initiated, else NULL */
static OSSL_CMP_MSG *delayed_delivery(OSSL_CMP_SRV_CTX *srv_ctx,
                                      const OSSL_CMP_MSG *req)
{
    int ret;
    unsigned long err;
    int status = OSSL_CMP_PKISTATUS_waiting,
        fail_info = 0, errorCode = 0;
    const char *txt = NULL, *details = NULL;
    OSSL_CMP_PKISI *si;
    OSSL_CMP_MSG *msg;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL
                     && srv_ctx->delayed_delivery != NULL))
        return NULL;

    ret = srv_ctx->delayed_delivery(srv_ctx, req);
    if (ret == 0)
        return NULL;
    if (ret == 1) {
        srv_ctx->polling = 1;
    } else {
        status = OSSL_CMP_PKISTATUS_rejection;
        fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_systemFailure;
        txt = "server application error";
        err = ERR_peek_error();
        errorCode = ERR_GET_REASON(err);
        details = ERR_reason_error_string(err);
    }

    si = OSSL_CMP_STATUSINFO_new(status, fail_info, txt);
    if (si == NULL)
        return NULL;

    msg = ossl_cmp_error_new(srv_ctx->ctx, si, errorCode, details,
                             srv_ctx->sendUnprotectedErrors);
    OSSL_CMP_PKISI_free(si);
    return msg;
}

/*
 * Processes an ir/cr/p10cr/kur and returns a certification response.
 * Only handles the first certification request contained in req
 * returns an ip/cp/kup on success and NULL on error
 */
static OSSL_CMP_MSG *process_cert_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                          const OSSL_CMP_MSG *req)
{
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_PKISI *si = NULL;
    X509 *certOut = NULL;
    EVP_PKEY *keyOut = NULL;
    STACK_OF(X509) *chainOut = NULL, *caPubs = NULL;
    const OSSL_CRMF_MSG *crm = NULL;
    const X509_REQ *p10cr = NULL;
    int bodytype;
    int certReqId, central_keygen;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;

    switch (OSSL_CMP_MSG_get_bodytype(req)) {
    case OSSL_CMP_PKIBODY_P10CR:
    case OSSL_CMP_PKIBODY_CR:
        bodytype = OSSL_CMP_PKIBODY_CP;
        break;
    case OSSL_CMP_PKIBODY_IR:
        bodytype = OSSL_CMP_PKIBODY_IP;
        break;
    case OSSL_CMP_PKIBODY_KUR:
        bodytype = OSSL_CMP_PKIBODY_KUP;
        break;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return NULL;
    }

    if (OSSL_CMP_MSG_get_bodytype(req) == OSSL_CMP_PKIBODY_P10CR) {
        certReqId = OSSL_CMP_CERTREQID_NONE; /* p10cr does not include an Id */
        p10cr = req->body->value.p10cr;
    } else {
        OSSL_CRMF_MSGS *reqs = req->body->value.ir; /* same for cr and kur */

        if (sk_OSSL_CRMF_MSG_num(reqs) != 1) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_REQUESTS_NOT_SUPPORTED);
            return NULL;
        }
        if ((crm = sk_OSSL_CRMF_MSG_value(reqs, 0)) == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_CERTREQMSG_NOT_FOUND);
            return NULL;
        }
        certReqId = OSSL_CRMF_MSG_get_certReqId(crm);
        if (certReqId != OSSL_CMP_CERTREQID) { /* so far, only possible value */
            ERR_raise(ERR_LIB_CMP, CMP_R_BAD_REQUEST_ID);
            return NULL;
        }
    }
    srv_ctx->certReqId = certReqId;

    central_keygen = OSSL_CRMF_MSG_centralkeygen_requested(crm, p10cr);
    if (central_keygen < 0)
        return NULL;
    if (central_keygen == 0
        && !ossl_cmp_verify_popo(srv_ctx->ctx, req, srv_ctx->acceptRAVerified)) {
        /* Proof of possession could not be verified */
        si = OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_rejection,
                                     1 << OSSL_CMP_PKIFAILUREINFO_badPOP,
                                     ERR_reason_error_string(ERR_peek_error()));
        if (si == NULL)
            return NULL;
    } else {
        OSSL_CMP_PKIHEADER *hdr = OSSL_CMP_MSG_get0_header(req);

        si = srv_ctx->process_cert_request(srv_ctx, req, certReqId, crm, p10cr,
                                           &certOut, &chainOut, &caPubs);
        if (si == NULL)
            goto err;
        if (ossl_cmp_pkisi_get_status(si) == OSSL_CMP_PKISTATUS_waiting)
            srv_ctx->polling = 1;
        /* set OSSL_CMP_OPT_IMPLICIT_CONFIRM if and only if transaction ends */
        if (!OSSL_CMP_CTX_set_option(srv_ctx->ctx,
                                     OSSL_CMP_OPT_IMPLICIT_CONFIRM,
                                     ossl_cmp_hdr_has_implicitConfirm(hdr)
                                         && srv_ctx->grantImplicitConfirm
                                         /* do not set if polling starts: */
                                         && certOut != NULL))
            goto err;
        if (central_keygen == 1
            && srv_ctx->ctx->newPkey_priv && srv_ctx->ctx->newPkey != NULL)
            keyOut = srv_ctx->ctx->newPkey;
    }

    msg = ossl_cmp_certrep_new(srv_ctx->ctx, bodytype, certReqId, si,
                               certOut, keyOut, NULL /* enc */, chainOut, caPubs,
                               srv_ctx->sendUnprotectedErrors);
    /* When supporting OSSL_CRMF_POPO_KEYENC, "enc" will need to be set */
    if (msg == NULL)
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_CERTREP);

 err:
    OSSL_CMP_PKISI_free(si);
    X509_free(certOut);
    OSSL_CMP_CTX_set0_newPkey(srv_ctx->ctx, 0, NULL);
    OSSL_STACK_OF_X509_free(chainOut);
    OSSL_STACK_OF_X509_free(caPubs);
    return msg;
}

static OSSL_CMP_MSG *process_rr(OSSL_CMP_SRV_CTX *srv_ctx,
                                const OSSL_CMP_MSG *req)
{
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_REVDETAILS *details;
    OSSL_CRMF_CERTID *certId = NULL;
    OSSL_CRMF_CERTTEMPLATE *tmpl;
    const X509_NAME *issuer;
    const ASN1_INTEGER *serial;
    OSSL_CMP_PKISI *si;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;

    if (sk_OSSL_CMP_REVDETAILS_num(req->body->value.rr) != 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_REQUESTS_NOT_SUPPORTED);
        return NULL;
    }
    details = sk_OSSL_CMP_REVDETAILS_value(req->body->value.rr, 0);
    if (details == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
        return NULL;
    }

    tmpl = details->certDetails;
    issuer = OSSL_CRMF_CERTTEMPLATE_get0_issuer(tmpl);
    serial = OSSL_CRMF_CERTTEMPLATE_get0_serialNumber(tmpl);
    if (issuer != NULL && serial != NULL
            && (certId = OSSL_CRMF_CERTID_gen(issuer, serial)) == NULL)
        return NULL;
    if ((si = srv_ctx->process_rr(srv_ctx, req, issuer, serial)) == NULL)
        goto err;

    if ((msg = ossl_cmp_rp_new(srv_ctx->ctx, si, certId,
                               srv_ctx->sendUnprotectedErrors)) == NULL)
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_RR);

 err:
    OSSL_CRMF_CERTID_free(certId);
    OSSL_CMP_PKISI_free(si);
    return msg;
}

/*
 * Processes genm and creates a genp message mirroring the contents of the
 * incoming message
 */
static OSSL_CMP_MSG *process_genm(OSSL_CMP_SRV_CTX *srv_ctx,
                                  const OSSL_CMP_MSG *req)
{
    OSSL_CMP_GENMSGCONTENT *itavs;
    OSSL_CMP_MSG *msg;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;

    if (!srv_ctx->process_genm(srv_ctx, req, req->body->value.genm, &itavs))
        return NULL;

    msg = ossl_cmp_genp_new(srv_ctx->ctx, itavs);
    sk_OSSL_CMP_ITAV_pop_free(itavs, OSSL_CMP_ITAV_free);
    return msg;
}

static OSSL_CMP_MSG *process_error(OSSL_CMP_SRV_CTX *srv_ctx,
                                   const OSSL_CMP_MSG *req)
{
    OSSL_CMP_ERRORMSGCONTENT *errorContent;
    OSSL_CMP_MSG *msg;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;
    errorContent = req->body->value.error;
    srv_ctx->process_error(srv_ctx, req, errorContent->pKIStatusInfo,
                           errorContent->errorCode, errorContent->errorDetails);

    if ((msg = ossl_cmp_pkiconf_new(srv_ctx->ctx)) == NULL)
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_PKICONF);
    return msg;
}

static OSSL_CMP_MSG *process_certConf(OSSL_CMP_SRV_CTX *srv_ctx,
                                      const OSSL_CMP_MSG *req)
{
    OSSL_CMP_CTX *ctx;
    OSSL_CMP_CERTCONFIRMCONTENT *ccc;
    int num;
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_CERTSTATUS *status = NULL;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;

    ctx = srv_ctx->ctx;
    ccc = req->body->value.certConf;
    num = sk_OSSL_CMP_CERTSTATUS_num(ccc);

    if (OSSL_CMP_CTX_get_option(ctx, OSSL_CMP_OPT_IMPLICIT_CONFIRM) == 1
            || ctx->status != OSSL_CMP_PKISTATUS_trans) {
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_UNEXPECTED_CERTCONF);
        return NULL;
    }

    if (num == 0) {
        ossl_cmp_err(ctx, "certificate rejected by client");
    } else {
        if (num > 1)
            ossl_cmp_warn(ctx, "All CertStatus but the first will be ignored");
        status = sk_OSSL_CMP_CERTSTATUS_value(ccc, 0);
    }

    if (status != NULL) {
        int certReqId = ossl_cmp_asn1_get_int(status->certReqId);
        ASN1_OCTET_STRING *certHash = status->certHash;
        OSSL_CMP_PKISI *si = status->statusInfo;

        if (certReqId != srv_ctx->certReqId) {
            ERR_raise(ERR_LIB_CMP, CMP_R_BAD_REQUEST_ID);
            return NULL;
        }
        if (!srv_ctx->process_certConf(srv_ctx, req, certReqId, certHash, si))
            return NULL; /* reason code may be: CMP_R_CERTHASH_UNMATCHED */

        if (si != NULL
            && ossl_cmp_pkisi_get_status(si) != OSSL_CMP_PKISTATUS_accepted) {
            int pki_status = ossl_cmp_pkisi_get_status(si);
            const char *str = ossl_cmp_PKIStatus_to_string(pki_status);

            ossl_cmp_log2(INFO, ctx, "certificate rejected by client %s %s",
                          str == NULL ? "without" : "with",
                          str == NULL ? "PKIStatus" : str);
        }
    }

    if ((msg = ossl_cmp_pkiconf_new(ctx)) == NULL)
        ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_PKICONF);
    return msg;
}

/* pollReq is handled separately, to avoid recursive call */
static OSSL_CMP_MSG *process_non_polling_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                                 const OSSL_CMP_MSG *req)
{
    OSSL_CMP_MSG *rsp = NULL;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL
                     && req->body != NULL))
        return NULL;

    switch (OSSL_CMP_MSG_get_bodytype(req)) {
    case OSSL_CMP_PKIBODY_IR:
    case OSSL_CMP_PKIBODY_CR:
    case OSSL_CMP_PKIBODY_P10CR:
    case OSSL_CMP_PKIBODY_KUR:
        if (srv_ctx->process_cert_request == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_cert_request(srv_ctx, req);
        break;
    case OSSL_CMP_PKIBODY_RR:
        if (srv_ctx->process_rr == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_rr(srv_ctx, req);
        break;
    case OSSL_CMP_PKIBODY_GENM:
        if (srv_ctx->process_genm == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_genm(srv_ctx, req);
        break;
    case OSSL_CMP_PKIBODY_ERROR:
        if (srv_ctx->process_error == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_error(srv_ctx, req);
        break;
    case OSSL_CMP_PKIBODY_CERTCONF:
        if (srv_ctx->process_certConf == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_certConf(srv_ctx, req);
        break;

    case OSSL_CMP_PKIBODY_POLLREQ:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        break;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        break;
    }

    return rsp;
}

static OSSL_CMP_MSG *process_pollReq(OSSL_CMP_SRV_CTX *srv_ctx,
                                     const OSSL_CMP_MSG *req)
{
    OSSL_CMP_POLLREQCONTENT *prc;
    OSSL_CMP_POLLREQ *pr;
    int certReqId;
    OSSL_CMP_MSG *orig_req;
    int64_t check_after = 0;
    OSSL_CMP_MSG *msg = NULL;

    if (!ossl_assert(srv_ctx != NULL && srv_ctx->ctx != NULL && req != NULL))
        return NULL;

    if (!srv_ctx->polling) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return NULL;
    }

    prc = req->body->value.pollReq;
    if (sk_OSSL_CMP_POLLREQ_num(prc) != 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_REQUESTS_NOT_SUPPORTED);
        return NULL;
    }

    pr = sk_OSSL_CMP_POLLREQ_value(prc, 0);
    certReqId = ossl_cmp_asn1_get_int(pr->certReqId);
    if (!srv_ctx->process_pollReq(srv_ctx, req, certReqId,
                                  &orig_req, &check_after))
        return NULL;

    if (orig_req != NULL) {
        srv_ctx->polling = 0;
        msg = process_non_polling_request(srv_ctx, orig_req);
        OSSL_CMP_MSG_free(orig_req);
    } else {
        if ((msg = ossl_cmp_pollRep_new(srv_ctx->ctx, certReqId,
                                        check_after)) == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_POLLREP);
    }
    return msg;
}

/*
 * Determine whether missing/invalid protection of request message is allowed.
 * Return 1 on acceptance, 0 on rejection, or -1 on (internal) error.
 */
static int unprotected_exception(const OSSL_CMP_CTX *ctx,
                                 const OSSL_CMP_MSG *req,
                                 int invalid_protection,
                                 int accept_unprotected_requests)
{
    if (!ossl_assert(ctx != NULL && req != NULL))
        return -1;

    if (accept_unprotected_requests) {
        ossl_cmp_log1(WARN, ctx, "ignoring %s protection of request message",
                      invalid_protection ? "invalid" : "missing");
        return 1;
    }
    if (OSSL_CMP_MSG_get_bodytype(req) == OSSL_CMP_PKIBODY_ERROR
        && OSSL_CMP_CTX_get_option(ctx, OSSL_CMP_OPT_UNPROTECTED_ERRORS) == 1) {
        ossl_cmp_warn(ctx, "ignoring missing protection of error message");
        return 1;
    }
    return 0;
}

/*
 * returns created message and NULL on internal error
 */
OSSL_CMP_MSG *OSSL_CMP_SRV_process_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                           const OSSL_CMP_MSG *req)
{
    OSSL_CMP_CTX *ctx;
    ASN1_OCTET_STRING *backup_secret;
    OSSL_CMP_PKIHEADER *hdr;
    int req_type, rsp_type;
    int req_verified = 0;
    OSSL_CMP_MSG *rsp = NULL;

    if (srv_ctx == NULL || srv_ctx->ctx == NULL
            || req == NULL || req->body == NULL
            || (hdr = OSSL_CMP_MSG_get0_header(req)) == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    ctx = srv_ctx->ctx;
    backup_secret = ctx->secretValue;
    req_type = OSSL_CMP_MSG_get_bodytype(req);
    ossl_cmp_log1(DEBUG, ctx,
                  "received %s", ossl_cmp_bodytype_to_string(req_type));

    /*
     * Some things need to be done already before validating the message in
     * order to be able to send an error message as far as needed and possible.
     */
    if (hdr->sender->type != GEN_DIRNAME) {
        ERR_raise(ERR_LIB_CMP, CMP_R_SENDER_GENERALNAME_TYPE_NOT_SUPPORTED);
        goto err;
    }
    if (!OSSL_CMP_CTX_set1_recipient(ctx, hdr->sender->d.directoryName))
        goto err;

    if (srv_ctx->polling && req_type != OSSL_CMP_PKIBODY_POLLREQ
            && req_type != OSSL_CMP_PKIBODY_ERROR) {
        ERR_raise(ERR_LIB_CMP, CMP_R_EXPECTED_POLLREQ);
        goto err;
    }

    switch (req_type) {
    case OSSL_CMP_PKIBODY_IR:
    case OSSL_CMP_PKIBODY_CR:
    case OSSL_CMP_PKIBODY_P10CR:
    case OSSL_CMP_PKIBODY_KUR:
    case OSSL_CMP_PKIBODY_RR:
    case OSSL_CMP_PKIBODY_GENM:
    case OSSL_CMP_PKIBODY_ERROR:
        if (ctx->transactionID != NULL) {
            char *tid = i2s_ASN1_OCTET_STRING(NULL, ctx->transactionID);

            if (tid != NULL)
                ossl_cmp_log1(WARN, ctx,
                              "Assuming that last transaction with ID=%s got aborted",
                              tid);
            OPENSSL_free(tid);
        }
        /* start of a new transaction, reset transactionID and senderNonce */
        if (!OSSL_CMP_CTX_set1_transactionID(ctx, NULL)
                || !OSSL_CMP_CTX_set1_senderNonce(ctx, NULL))
            goto err;

        if (srv_ctx->clean_transaction != NULL
                && !srv_ctx->clean_transaction(srv_ctx, NULL)) {
            ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
            goto err;
        }

        break;
    default:
        /* transactionID should be already initialized */
        if (ctx->transactionID == NULL) {
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
            ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
            goto err;
#endif
        }
    }

    req_verified = ossl_cmp_msg_check_update(ctx, req, unprotected_exception,
                                             srv_ctx->acceptUnprotected);
    if (ctx->secretValue != NULL && ctx->pkey != NULL
            && ossl_cmp_hdr_get_protection_nid(hdr) != NID_id_PasswordBasedMAC)
        ctx->secretValue = NULL; /* use MSG_SIG_ALG when protecting rsp */
    if (!req_verified)
        goto err;

    if (req_type == OSSL_CMP_PKIBODY_POLLREQ) {
        if (srv_ctx->process_pollReq == NULL)
            ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_PKIBODY);
        else
            rsp = process_pollReq(srv_ctx, req);
    } else {
        if (srv_ctx->delayed_delivery != NULL
            && (rsp = delayed_delivery(srv_ctx, req)) != NULL) {
            goto err;
        }
        rsp = process_non_polling_request(srv_ctx, req);
    }

 err:
    if (rsp == NULL) {
        /* on error, try to respond with CMP error message to client */
        const char *data = NULL, *reason = NULL;
        int flags = 0;
        unsigned long err = ERR_peek_error_data(&data, &flags);
        int fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_badRequest;
        /* fail_info is not very specific */
        OSSL_CMP_PKISI *si = NULL;

        if (!req_verified) {
            /*
             * Above ossl_cmp_msg_check_update() was not successfully executed,
             * which normally would set ctx->transactionID and ctx->recipNonce.
             * So anyway try to provide the right transactionID and recipNonce,
             * while ignoring any (extra) error in next two function calls.
             */
            if (ctx->transactionID == NULL)
                (void)OSSL_CMP_CTX_set1_transactionID(ctx, hdr->transactionID);
            (void)ossl_cmp_ctx_set1_recipNonce(ctx, hdr->senderNonce);
        }

        if ((flags & ERR_TXT_STRING) == 0 || *data == '\0')
            data = NULL;
        reason = ERR_reason_error_string(err);
        if ((si = OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_rejection,
                                          fail_info, reason)) != NULL) {
            rsp = ossl_cmp_error_new(srv_ctx->ctx, si, err,
                                     data, srv_ctx->sendUnprotectedErrors);
            OSSL_CMP_PKISI_free(si);
        }
    }
    OSSL_CMP_CTX_print_errors(ctx);
    ctx->secretValue = backup_secret;

    rsp_type =
        rsp != NULL ? OSSL_CMP_MSG_get_bodytype(rsp) : OSSL_CMP_PKIBODY_ERROR;
    if (rsp != NULL)
        ossl_cmp_log1(DEBUG, ctx,
                      "sending %s", ossl_cmp_bodytype_to_string(rsp_type));
    else
        ossl_cmp_log(ERR, ctx, "cannot send proper CMP response");

    /* determine whether to keep the transaction open or not */
    ctx->status = OSSL_CMP_PKISTATUS_trans;
    switch (rsp_type) {
    case OSSL_CMP_PKIBODY_IP:
    case OSSL_CMP_PKIBODY_CP:
    case OSSL_CMP_PKIBODY_KUP:
        if (OSSL_CMP_CTX_get_option(ctx, OSSL_CMP_OPT_IMPLICIT_CONFIRM) == 0)
            break;
        /* fall through */

    case OSSL_CMP_PKIBODY_ERROR:
        if (rsp != NULL && ossl_cmp_is_error_with_waiting(rsp))
            break;
        /* fall through */

    case OSSL_CMP_PKIBODY_RP:
    case OSSL_CMP_PKIBODY_PKICONF:
    case OSSL_CMP_PKIBODY_GENP:
        /* Other terminating response message types are not supported */
        srv_ctx->certReqId = OSSL_CMP_CERTREQID_INVALID;
        /* Prepare for next transaction, ignoring any errors here: */
        if (srv_ctx->clean_transaction != NULL)
            (void)srv_ctx->clean_transaction(srv_ctx, ctx->transactionID);
        (void)OSSL_CMP_CTX_set1_transactionID(ctx, NULL);
        (void)OSSL_CMP_CTX_set1_senderNonce(ctx, NULL);
        ctx->status = OSSL_CMP_PKISTATUS_unspecified; /* transaction closed */

    default: /* not closing transaction in other cases */
        break;
    }
    return rsp;
}

/*
 * Server interface that may substitute OSSL_CMP_MSG_http_perform at the client.
 * The OSSL_CMP_SRV_CTX must be set as client_ctx->transfer_cb_arg.
 * returns received message on success, else NULL and pushes an element on the
 * error stack.
 */
OSSL_CMP_MSG *OSSL_CMP_CTX_server_perform(OSSL_CMP_CTX *client_ctx,
                                          const OSSL_CMP_MSG *req)
{
    OSSL_CMP_SRV_CTX *srv_ctx = NULL;

    if (client_ctx == NULL || req == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }

    if ((srv_ctx = OSSL_CMP_CTX_get_transfer_cb_arg(client_ctx)) == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_TRANSFER_ERROR);
        return NULL;
    }

    return OSSL_CMP_SRV_process_request(srv_ctx, req);
}
