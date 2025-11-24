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

#include "cmp_local.h"
#include "internal/cryptlib.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/bio.h>
#include <openssl/cmp.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/x509v3.h>
#include <openssl/cmp_util.h>

#define IS_CREP(t) ((t) == OSSL_CMP_PKIBODY_IP || (t) == OSSL_CMP_PKIBODY_CP \
                        || (t) == OSSL_CMP_PKIBODY_KUP)

/*-
 * Evaluate whether there's an exception (violating the standard) configured for
 * handling negative responses without protection or with invalid protection.
 * Returns 1 on acceptance, 0 on rejection, or -1 on (internal) error.
 */
static int unprotected_exception(const OSSL_CMP_CTX *ctx,
                                 const OSSL_CMP_MSG *rep,
                                 int invalid_protection,
                                 ossl_unused int expected_type)
{
    int rcvd_type = OSSL_CMP_MSG_get_bodytype(rep /* may be NULL */);
    const char *msg_type = NULL;

    if (!ossl_assert(ctx != NULL && rep != NULL))
        return -1;

    if (!ctx->unprotectedErrors)
        return 0;

    switch (rcvd_type) {
    case OSSL_CMP_PKIBODY_ERROR:
        msg_type = "error response";
        break;
    case OSSL_CMP_PKIBODY_RP:
        {
            OSSL_CMP_PKISI *si =
                ossl_cmp_revrepcontent_get_pkisi(rep->body->value.rp,
                                                 OSSL_CMP_REVREQSID);

            if (si == NULL)
                return -1;
            if (ossl_cmp_pkisi_get_status(si) == OSSL_CMP_PKISTATUS_rejection)
                msg_type = "revocation response message with rejection status";
            break;
        }
    case OSSL_CMP_PKIBODY_PKICONF:
        msg_type = "PKI Confirmation message";
        break;
    default:
        if (IS_CREP(rcvd_type)) {
            int any_rid = OSSL_CMP_CERTREQID_NONE;
            OSSL_CMP_CERTREPMESSAGE *crepmsg = rep->body->value.ip;
            OSSL_CMP_CERTRESPONSE *crep =
                ossl_cmp_certrepmessage_get0_certresponse(crepmsg, any_rid);

            if (sk_OSSL_CMP_CERTRESPONSE_num(crepmsg->response) > 1)
                return -1;
            if (crep == NULL)
                return -1;
            if (ossl_cmp_pkisi_get_status(crep->status)
                == OSSL_CMP_PKISTATUS_rejection)
                msg_type = "CertRepMessage with rejection status";
        }
    }
    if (msg_type == NULL)
        return 0;
    ossl_cmp_log2(WARN, ctx, "ignoring %s protection of %s",
                  invalid_protection ? "invalid" : "missing", msg_type);
    return 1;
}

/* Save error info from PKIStatusInfo field of a certresponse into ctx */
static int save_statusInfo(OSSL_CMP_CTX *ctx, OSSL_CMP_PKISI *si)
{
    int i;
    OSSL_CMP_PKIFREETEXT *ss;

    if (!ossl_assert(ctx != NULL && si != NULL))
        return 0;

    ctx->status = ossl_cmp_pkisi_get_status(si);
    if (ctx->status < OSSL_CMP_PKISTATUS_accepted)
        return 0;

    ctx->failInfoCode = ossl_cmp_pkisi_get_pkifailureinfo(si);

    if (!ossl_cmp_ctx_set0_statusString(ctx, sk_ASN1_UTF8STRING_new_null())
            || (ctx->statusString == NULL))
        return 0;

    ss = si->statusString; /* may be NULL */
    for (i = 0; i < sk_ASN1_UTF8STRING_num(ss); i++) {
        ASN1_UTF8STRING *str = sk_ASN1_UTF8STRING_value(ss, i);
        ASN1_UTF8STRING *dup = ASN1_STRING_dup(str);

        if (dup == NULL || !sk_ASN1_UTF8STRING_push(ctx->statusString, dup)) {
            ASN1_UTF8STRING_free(dup);
            return 0;
        }
    }
    return 1;
}

static int is_crep_with_waiting(const OSSL_CMP_MSG *resp, int rid)
{
    OSSL_CMP_CERTREPMESSAGE *crepmsg;
    OSSL_CMP_CERTRESPONSE *crep;
    int bt = OSSL_CMP_MSG_get_bodytype(resp);

    if (!IS_CREP(bt))
        return 0;

    crepmsg = resp->body->value.ip; /* same for cp and kup */
    crep = ossl_cmp_certrepmessage_get0_certresponse(crepmsg, rid);

    return (crep != NULL
            && ossl_cmp_pkisi_get_status(crep->status)
            == OSSL_CMP_PKISTATUS_waiting);
}

/*-
 * Perform the generic aspects of sending a request and receiving a response.
 * Returns 1 on success and provides the received PKIMESSAGE in *rep.
 * Returns 0 on error.
 * Regardless of success, caller is responsible for freeing *rep (unless NULL).
 */
static int send_receive_check(OSSL_CMP_CTX *ctx, const OSSL_CMP_MSG *req,
                              OSSL_CMP_MSG **rep, int expected_type)
{
    int begin_transaction =
        expected_type != OSSL_CMP_PKIBODY_POLLREP
        && expected_type != OSSL_CMP_PKIBODY_PKICONF;
    const char *req_type_str =
        ossl_cmp_bodytype_to_string(OSSL_CMP_MSG_get_bodytype(req));
    const char *expected_type_str = ossl_cmp_bodytype_to_string(expected_type);
    int bak_msg_timeout = ctx->msg_timeout;
    int bt;
    time_t now = time(NULL);
    int time_left;
    OSSL_CMP_transfer_cb_t transfer_cb = ctx->transfer_cb;

#ifndef OPENSSL_NO_HTTP
    if (transfer_cb == NULL)
        transfer_cb = OSSL_CMP_MSG_http_perform;
#endif
    *rep = NULL;

    if (ctx->total_timeout != 0 /* not waiting indefinitely */) {
        if (begin_transaction)
            ctx->end_time = now + ctx->total_timeout;
        if (now >= ctx->end_time) {
            ERR_raise(ERR_LIB_CMP, CMP_R_TOTAL_TIMEOUT);
            return 0;
        }
        if (!ossl_assert(ctx->end_time - now < INT_MAX)) {
            /* actually cannot happen due to assignment in initial_certreq() */
            ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
            return 0;
        }
        time_left = (int)(ctx->end_time - now);
        if (ctx->msg_timeout == 0 || time_left < ctx->msg_timeout)
            ctx->msg_timeout = time_left;
    }

    /* should print error queue since transfer_cb may call ERR_clear_error() */
    OSSL_CMP_CTX_print_errors(ctx);

    if (ctx->server != NULL)
        ossl_cmp_log1(INFO, ctx, "sending %s", req_type_str);

    *rep = (*transfer_cb)(ctx, req);
    ctx->msg_timeout = bak_msg_timeout;

    if (*rep == NULL) {
        ERR_raise_data(ERR_LIB_CMP,
                       ctx->total_timeout != 0 && time(NULL) >= ctx->end_time ?
                       CMP_R_TOTAL_TIMEOUT : CMP_R_TRANSFER_ERROR,
                       "request sent: %s, expected response: %s",
                       req_type_str, expected_type_str);
        return 0;
    }

    bt = OSSL_CMP_MSG_get_bodytype(*rep);
    /*
     * The body type in the 'bt' variable is not yet verified.
     * Still we use this preliminary value already for a progress report because
     * the following msg verification may also produce log entries and may fail.
     */
    ossl_cmp_log2(INFO, ctx, "received %s%s", ossl_cmp_bodytype_to_string(bt),
                  ossl_cmp_is_error_with_waiting(*rep) ? " (waiting)" : "");

    /* copy received extraCerts to ctx->extraCertsIn so they can be retrieved */
    if (bt != OSSL_CMP_PKIBODY_POLLREP && bt != OSSL_CMP_PKIBODY_PKICONF
            && !ossl_cmp_ctx_set1_extraCertsIn(ctx, (*rep)->extraCerts))
        return 0;

    if (!ossl_cmp_msg_check_update(ctx, *rep, unprotected_exception,
                                   expected_type))
        return 0;

    /*
     * rep can have the expected response type, which during polling is pollRep.
     * When polling, also any other non-error response (the final response)
     * is fine here. When not yet polling, delayed delivery may be initiated
     * by the server returning an error message with 'waiting' status (or a
     * response message of expected type ip/cp/kup with 'waiting' status).
     */
    if (bt == expected_type
        || (expected_type == OSSL_CMP_PKIBODY_POLLREP
            ? bt != OSSL_CMP_PKIBODY_ERROR
            : ossl_cmp_is_error_with_waiting(*rep)))
        return 1;

    /* received message type is not one of the expected ones (e.g., error) */
    ERR_raise(ERR_LIB_CMP, bt == OSSL_CMP_PKIBODY_ERROR ? CMP_R_RECEIVED_ERROR :
              CMP_R_UNEXPECTED_PKIBODY); /* in next line for mkerr.pl */

    if (bt != OSSL_CMP_PKIBODY_ERROR) {
        ERR_add_error_data(3, "message type is '",
                           ossl_cmp_bodytype_to_string(bt), "'");
    } else {
        OSSL_CMP_ERRORMSGCONTENT *emc = (*rep)->body->value.error;
        OSSL_CMP_PKISI *si = emc->pKIStatusInfo;
        char buf[OSSL_CMP_PKISI_BUFLEN];

        if (save_statusInfo(ctx, si)
                && OSSL_CMP_CTX_snprint_PKIStatus(ctx, buf,
                                                  sizeof(buf)) != NULL)
            ERR_add_error_data(1, buf);
        if (emc->errorCode != NULL
                && BIO_snprintf(buf, sizeof(buf), "; errorCode: %08lX",
                                ASN1_INTEGER_get(emc->errorCode)) > 0)
            ERR_add_error_data(1, buf);
        if (emc->errorDetails != NULL) {
            char *text = ossl_sk_ASN1_UTF8STRING2text(emc->errorDetails, ", ",
                                                      OSSL_CMP_PKISI_BUFLEN - 1);

            if (text != NULL && *text != '\0')
                ERR_add_error_data(2, "; errorDetails: ", text);
            OPENSSL_free(text);
        }
        if (ctx->status != OSSL_CMP_PKISTATUS_rejection) {
            ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKISTATUS);
            if (ctx->status == OSSL_CMP_PKISTATUS_waiting)
                ctx->status = OSSL_CMP_PKISTATUS_rejection;
        }
    }
    return 0;
}

/*-
 * When a 'waiting' PKIStatus has been received, this function is used to
 * poll, which should yield a pollRep or the final response.
 * On receiving a pollRep, which includes a checkAfter value, it return this
 * value if sleep == 0, else it sleeps as long as indicated and retries.
 *
 * A transaction timeout is enabled if ctx->total_timeout is != 0.
 * In this case polling will continue until the timeout is reached and then
 * polling is done a last time even if this is before the "checkAfter" time.
 *
 * Returns -1 on receiving pollRep if sleep == 0, setting the checkAfter value.
 * Returns 1 on success and provides the received PKIMESSAGE in *rep.
 *           In this case the caller is responsible for freeing *rep.
 * Returns 0 on error (which includes the cases that timeout has been reached
 *           or a response with 'waiting' status has been received).
 */
static int poll_for_response(OSSL_CMP_CTX *ctx, int sleep, int rid,
                             OSSL_CMP_MSG **rep, int *checkAfter)
{
    OSSL_CMP_MSG *preq = NULL;
    OSSL_CMP_MSG *prep = NULL;

    ossl_cmp_info(ctx,
                  "received 'waiting' PKIStatus, starting to poll for response");
    *rep = NULL;
    for (;;) {
        if ((preq = ossl_cmp_pollReq_new(ctx, rid)) == NULL)
            goto err;

        if (!send_receive_check(ctx, preq, &prep, OSSL_CMP_PKIBODY_POLLREP))
            goto err;

        /* handle potential pollRep */
        if (OSSL_CMP_MSG_get_bodytype(prep) == OSSL_CMP_PKIBODY_POLLREP) {
            OSSL_CMP_POLLREPCONTENT *prc = prep->body->value.pollRep;
            OSSL_CMP_POLLREP *pollRep = NULL;
            int64_t check_after;
            char str[OSSL_CMP_PKISI_BUFLEN];
            int len;

            if (sk_OSSL_CMP_POLLREP_num(prc) > 1) {
                ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_RESPONSES_NOT_SUPPORTED);
                goto err;
            }
            pollRep = ossl_cmp_pollrepcontent_get0_pollrep(prc, rid);
            if (pollRep == NULL)
                goto err;

            if (!ASN1_INTEGER_get_int64(&check_after, pollRep->checkAfter)) {
                ERR_raise(ERR_LIB_CMP, CMP_R_BAD_CHECKAFTER_IN_POLLREP);
                goto err;
            }
            if (check_after < 0 || (uint64_t)check_after
                > (sleep ? ULONG_MAX / 1000 : INT_MAX)) {
                ERR_raise(ERR_LIB_CMP, CMP_R_CHECKAFTER_OUT_OF_RANGE);
                if (BIO_snprintf(str, OSSL_CMP_PKISI_BUFLEN, "value = %jd",
                                 check_after) >= 0)
                    ERR_add_error_data(1, str);
                goto err;
            }

            if (pollRep->reason == NULL
                    || (len = BIO_snprintf(str, OSSL_CMP_PKISI_BUFLEN,
                                           " with reason = '")) < 0) {
                *str = '\0';
            } else {
                char *text = ossl_sk_ASN1_UTF8STRING2text(pollRep->reason, ", ",
                                                          sizeof(str) - len - 2);

                if (text == NULL
                        || BIO_snprintf(str + len, sizeof(str) - len,
                                        "%s'", text) < 0)
                    *str = '\0';
                OPENSSL_free(text);
            }
            ossl_cmp_log2(INFO, ctx,
                          "received polling response%s; checkAfter = %ld seconds",
                          str, check_after);

            if (ctx->total_timeout != 0) { /* timeout is not infinite */
                const int exp = OSSL_CMP_EXPECTED_RESP_TIME;
                int64_t time_left = (int64_t)(ctx->end_time - exp - time(NULL));

                if (time_left <= 0) {
                    ERR_raise(ERR_LIB_CMP, CMP_R_TOTAL_TIMEOUT);
                    goto err;
                }
                if (time_left < check_after)
                    check_after = time_left;
                /* poll one last time just when timeout was reached */
            }

            OSSL_CMP_MSG_free(preq);
            preq = NULL;
            OSSL_CMP_MSG_free(prep);
            prep = NULL;
            if (sleep) {
                OSSL_sleep((unsigned long)(1000 * check_after));
            } else {
                if (checkAfter != NULL)
                    *checkAfter = (int)check_after;
                return -1; /* exits the loop */
            }
        } else if (is_crep_with_waiting(prep, rid)
                   || ossl_cmp_is_error_with_waiting(prep)) {
            /* received status must not be 'waiting' */
            (void)ossl_cmp_exchange_error(ctx, OSSL_CMP_PKISTATUS_rejection,
                                          OSSL_CMP_CTX_FAILINFO_badRequest,
                                          "polling already started",
                                          0 /* errorCode */, NULL);
            ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKISTATUS);
            goto err;
        } else {
            ossl_cmp_info(ctx, "received final response after polling");
            if (!ossl_cmp_ctx_set1_first_senderNonce(ctx, NULL))
                return 0;
            break;
        }
    }
    if (prep == NULL)
        goto err;

    OSSL_CMP_MSG_free(preq);
    *rep = prep;

    return 1;
 err:
    (void)ossl_cmp_ctx_set1_first_senderNonce(ctx, NULL);
    OSSL_CMP_MSG_free(preq);
    OSSL_CMP_MSG_free(prep);
    return 0;
}

static int save_senderNonce_if_waiting(OSSL_CMP_CTX *ctx,
                                       const OSSL_CMP_MSG *rep, int rid)
{
    /*
     * Lightweight CMP Profile section 4.4 states: the senderNonce of the
     * preceding request message because this value will be needed for checking
     * the recipNonce of the final response to be received after polling.
     */
    if ((is_crep_with_waiting(rep, rid)
         || ossl_cmp_is_error_with_waiting(rep))
        && !ossl_cmp_ctx_set1_first_senderNonce(ctx, ctx->senderNonce))
        return 0;

    return 1;
}

/*
 * Send request and get response possibly with polling initiated by error msg.
 * Polling for ip/cp/kup/ with 'waiting' status is handled by cert_response().
 */
static int send_receive_also_delayed(OSSL_CMP_CTX *ctx, const OSSL_CMP_MSG *req,
                                     OSSL_CMP_MSG **rep, int expected_type)
{

    if (!send_receive_check(ctx, req, rep, expected_type))
        return 0;

    if (ossl_cmp_is_error_with_waiting(*rep)) {
        if (!save_senderNonce_if_waiting(ctx, *rep, OSSL_CMP_CERTREQID_NONE))
            return 0;
        /* not modifying ctx->status during certConf and error exchanges */
        if (expected_type != OSSL_CMP_PKIBODY_PKICONF
            && !save_statusInfo(ctx, (*rep)->body->value.error->pKIStatusInfo))
            return 0;

        OSSL_CMP_MSG_free(*rep);
        *rep = NULL;

        if (poll_for_response(ctx, 1 /* can sleep */, OSSL_CMP_CERTREQID_NONE,
                              rep, NULL /* checkAfter */) <= 0) {
            ERR_raise(ERR_LIB_CMP, CMP_R_POLLING_FAILED);
            return 0;
        }
    }
    if (OSSL_CMP_MSG_get_bodytype(*rep) != expected_type) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return 0;
    }

    return 1;
}
/*
 * Send certConf for IR, CR or KUR sequences and check response,
 * not modifying ctx->status during the certConf exchange
 */
int ossl_cmp_exchange_certConf(OSSL_CMP_CTX *ctx, int certReqId,
                               int fail_info, const char *txt)
{
    OSSL_CMP_MSG *certConf;
    OSSL_CMP_MSG *PKIconf = NULL;
    int res = 0;

    /* OSSL_CMP_certConf_new() also checks if all necessary options are set */
    certConf = ossl_cmp_certConf_new(ctx, certReqId, fail_info, txt);
    if (certConf == NULL)
        goto err;

    res = send_receive_also_delayed(ctx, certConf, &PKIconf,
                                    OSSL_CMP_PKIBODY_PKICONF);

 err:
    OSSL_CMP_MSG_free(certConf);
    OSSL_CMP_MSG_free(PKIconf);
    return res;
}

/* Send given error and check response */
int ossl_cmp_exchange_error(OSSL_CMP_CTX *ctx, int status, int fail_info,
                            const char *txt, int errorCode, const char *details)
{
    OSSL_CMP_MSG *error = NULL;
    OSSL_CMP_PKISI *si = NULL;
    OSSL_CMP_MSG *PKIconf = NULL;
    int res = 0;

    /* not overwriting ctx->status on error exchange */
    if ((si = OSSL_CMP_STATUSINFO_new(status, fail_info, txt)) == NULL)
        goto err;
    /* ossl_cmp_error_new() also checks if all necessary options are set */
    if ((error = ossl_cmp_error_new(ctx, si, errorCode, details, 0)) == NULL)
        goto err;

    res = send_receive_also_delayed(ctx, error,
                                    &PKIconf, OSSL_CMP_PKIBODY_PKICONF);

 err:
    OSSL_CMP_MSG_free(error);
    OSSL_CMP_PKISI_free(si);
    OSSL_CMP_MSG_free(PKIconf);
    return res;
}

/*-
 * Retrieve a copy of the certificate, if any, from the given CertResponse.
 * Take into account PKIStatusInfo of CertResponse in ctx, report it on error.
 * Returns NULL if not found or on error.
 */
static X509 *get1_cert_status(OSSL_CMP_CTX *ctx, int bodytype,
                              OSSL_CMP_CERTRESPONSE *crep)
{
    char buf[OSSL_CMP_PKISI_BUFLEN];
    X509 *crt = NULL;

    if (!ossl_assert(ctx != NULL && crep != NULL))
        return NULL;

    switch (ossl_cmp_pkisi_get_status(crep->status)) {
    case OSSL_CMP_PKISTATUS_waiting:
        ossl_cmp_err(ctx,
                     "received \"waiting\" status for cert when actually aiming to extract cert");
        ERR_raise(ERR_LIB_CMP, CMP_R_ENCOUNTERED_WAITING);
        goto err;
    case OSSL_CMP_PKISTATUS_grantedWithMods:
        ossl_cmp_warn(ctx, "received \"grantedWithMods\" for certificate");
        break;
    case OSSL_CMP_PKISTATUS_accepted:
        break;
        /* get all information in case of a rejection before going to error */
    case OSSL_CMP_PKISTATUS_rejection:
        ossl_cmp_err(ctx, "received \"rejection\" status rather than cert");
        ERR_raise(ERR_LIB_CMP, CMP_R_REQUEST_REJECTED_BY_SERVER);
        goto err;
    case OSSL_CMP_PKISTATUS_revocationWarning:
        ossl_cmp_warn(ctx,
                      "received \"revocationWarning\" - a revocation of the cert is imminent");
        break;
    case OSSL_CMP_PKISTATUS_revocationNotification:
        ossl_cmp_warn(ctx,
                      "received \"revocationNotification\" - a revocation of the cert has occurred");
        break;
    case OSSL_CMP_PKISTATUS_keyUpdateWarning:
        if (bodytype != OSSL_CMP_PKIBODY_KUR) {
            ERR_raise(ERR_LIB_CMP, CMP_R_ENCOUNTERED_KEYUPDATEWARNING);
            goto err;
        }
        break;
    default:
        ossl_cmp_log1(ERROR, ctx,
                      "received unsupported PKIStatus %d for certificate",
                      ctx->status);
        ERR_raise(ERR_LIB_CMP, CMP_R_UNKNOWN_PKISTATUS);
        goto err;
    }
    crt = ossl_cmp_certresponse_get1_cert(ctx, crep);
    if (crt == NULL) /* according to PKIStatus, we can expect a cert */
        ERR_raise(ERR_LIB_CMP, CMP_R_CERTIFICATE_NOT_FOUND);

    return crt;

 err:
    if (OSSL_CMP_CTX_snprint_PKIStatus(ctx, buf, sizeof(buf)) != NULL)
        ERR_add_error_data(1, buf);
    return NULL;
}

/*-
 * Callback fn validating that the new certificate can be verified, using
 * ctx->certConf_cb_arg, which has been initialized using opt_out_trusted, and
 * ctx->untrusted, which at this point already contains msg->extraCerts.
 * Returns 0 on acceptance, else a bit field reflecting PKIFailureInfo.
 * Quoting from RFC 4210 section 5.1. Overall PKI Message:
 *     The extraCerts field can contain certificates that may be useful to
 *     the recipient.  For example, this can be used by a CA or RA to
 *     present an end entity with certificates that it needs to verify its
 *     own new certificate (if, for example, the CA that issued the end
 *     entity's certificate is not a root CA for the end entity).  Note that
 *     this field does not necessarily contain a certification path; the
 *     recipient may have to sort, select from, or otherwise process the
 *     extra certificates in order to use them.
 * Note: While often handy, there is no hard requirement by CMP that
 * an EE must be able to validate the certificates it gets enrolled.
 */
int OSSL_CMP_certConf_cb(OSSL_CMP_CTX *ctx, X509 *cert, int fail_info,
                         const char **text)
{
    X509_STORE *out_trusted = OSSL_CMP_CTX_get_certConf_cb_arg(ctx);
    STACK_OF(X509) *chain = NULL;

    (void)text; /* make (artificial) use of var to prevent compiler warning */

    if (fail_info != 0) /* accept any error flagged by CMP core library */
        return fail_info;

    if (out_trusted == NULL) {
        ossl_cmp_debug(ctx, "trying to build chain for newly enrolled cert");
        chain = X509_build_chain(cert, ctx->untrusted, out_trusted,
                                 0, ctx->libctx, ctx->propq);
    } else {
        X509_STORE_CTX *csc = X509_STORE_CTX_new_ex(ctx->libctx, ctx->propq);

        ossl_cmp_debug(ctx, "validating newly enrolled cert");
        if (csc == NULL)
            goto err;
        if (!X509_STORE_CTX_init(csc, out_trusted, cert, ctx->untrusted))
            goto err;
        /* disable any cert status/revocation checking etc. */
        X509_VERIFY_PARAM_clear_flags(X509_STORE_CTX_get0_param(csc),
                                      ~(X509_V_FLAG_USE_CHECK_TIME
                                        | X509_V_FLAG_NO_CHECK_TIME
                                        | X509_V_FLAG_PARTIAL_CHAIN
                                        | X509_V_FLAG_POLICY_CHECK));
        if (X509_verify_cert(csc) <= 0)
            goto err;

        if (!ossl_x509_add_certs_new(&chain, X509_STORE_CTX_get0_chain(csc),
                                     X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP
                                     | X509_ADD_FLAG_NO_SS)) {
            sk_X509_free(chain);
            chain = NULL;
        }
    err:
        X509_STORE_CTX_free(csc);
    }

    if (sk_X509_num(chain) > 0)
        X509_free(sk_X509_shift(chain)); /* remove leaf (EE) cert */
    if (out_trusted != NULL) {
        if (chain == NULL) {
            ossl_cmp_err(ctx, "failed to validate newly enrolled cert");
            fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_incorrectData;
        } else {
            ossl_cmp_debug(ctx,
                           "success validating newly enrolled cert");
        }
    } else if (chain == NULL) {
        ossl_cmp_warn(ctx, "could not build approximate chain for newly enrolled cert, resorting to received extraCerts");
        chain = OSSL_CMP_CTX_get1_extraCertsIn(ctx);
    } else {
        ossl_cmp_debug(ctx,
                       "success building approximate chain for newly enrolled cert");
    }
    (void)ossl_cmp_ctx_set1_newChain(ctx, chain);
    OSSL_STACK_OF_X509_free(chain);

    return fail_info;
}

/*-
 * Perform the generic handling of certificate responses for IR/CR/KUR/P10CR.
 * |rid| must be OSSL_CMP_CERTREQID_NONE if not available, namely for p10cr
 * Returns -1 on receiving pollRep if sleep == 0, setting the checkAfter value.
 * Returns 1 on success and provides the received PKIMESSAGE in *resp.
 * Returns 0 on error (which includes the case that timeout has been reached).
 * Regardless of success, caller is responsible for freeing *resp (unless NULL).
 */
static int cert_response(OSSL_CMP_CTX *ctx, int sleep, int rid,
                         OSSL_CMP_MSG **resp, int *checkAfter,
                         ossl_unused int req_type,
                         ossl_unused int expected_type)
{
    EVP_PKEY *rkey = NULL;
    int fail_info = 0; /* no failure */
    const char *txt = NULL;
    OSSL_CMP_CERTREPMESSAGE *crepmsg = NULL;
    OSSL_CMP_CERTRESPONSE *crep = NULL;
    OSSL_CMP_certConf_cb_t cb;
    X509 *cert;
    char *subj = NULL;
    int ret = 1;
    int rcvd_type;
    OSSL_CMP_PKISI *si;

    if (!ossl_assert(ctx != NULL))
        return 0;

 retry:
    rcvd_type = OSSL_CMP_MSG_get_bodytype(*resp);
    if (IS_CREP(rcvd_type)) {
        crepmsg = (*resp)->body->value.ip; /* same for cp and kup */
        if (sk_OSSL_CMP_CERTRESPONSE_num(crepmsg->response) > 1) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_RESPONSES_NOT_SUPPORTED);
            return 0;
        }
        crep = ossl_cmp_certrepmessage_get0_certresponse(crepmsg, rid);
        if (crep == NULL)
            return 0;
        si = crep->status;

        if (rid == OSSL_CMP_CERTREQID_NONE) {
            /* for OSSL_CMP_PKIBODY_P10CR learn CertReqId from response */
            rid = ossl_cmp_asn1_get_int(crep->certReqId);
            if (rid < OSSL_CMP_CERTREQID_NONE) {
                ERR_raise(ERR_LIB_CMP, CMP_R_BAD_REQUEST_ID);
                return 0;
            }
        }
    } else if (rcvd_type == OSSL_CMP_PKIBODY_ERROR) {
        si = (*resp)->body->value.error->pKIStatusInfo;
    } else {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return 0;
    }

    if (!save_statusInfo(ctx, si))
        return 0;

    if (ossl_cmp_pkisi_get_status(si) == OSSL_CMP_PKISTATUS_waiting) {
        /*
         * Here we allow both and error message with waiting indication
         * as well as a certificate response with waiting indication, where
         * its flavor (ip, cp, or kup) may not strictly match ir/cr/p10cr/kur.
         */
        OSSL_CMP_MSG_free(*resp);
        *resp = NULL;
        if ((ret = poll_for_response(ctx, sleep, rid, resp, checkAfter)) != 0) {
            if (ret == -1) /* at this point implies sleep == 0 */
                return ret; /* waiting */
            goto retry; /* got some response other than pollRep */
        } else {
            ERR_raise(ERR_LIB_CMP, CMP_R_POLLING_FAILED);
            return 0;
        }
    }

    /* at this point, we have received ip/cp/kup/error without waiting */
    if (rcvd_type == OSSL_CMP_PKIBODY_ERROR) {
        ERR_raise(ERR_LIB_CMP, CMP_R_RECEIVED_ERROR);
        return 0;
    }
    /* here we are strict on the flavor of ip/cp/kup: must match request */
    if (rcvd_type != expected_type) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return 0;
    }

    cert = get1_cert_status(ctx, (*resp)->body->type, crep);
    if (cert == NULL) {
        ERR_add_error_data(1, "; cannot extract certificate from response");
        return 0;
    }
    if (!ossl_cmp_ctx_set0_newCert(ctx, cert)) {
        X509_free(cert);
        return 0;
    }

    /*
     * if the CMP server returned certificates in the caPubs field, copy them
     * to the context so that they can be retrieved if necessary
     */
    if (crepmsg != NULL && crepmsg->caPubs != NULL
            && !ossl_cmp_ctx_set1_caPubs(ctx, crepmsg->caPubs))
        return 0;

    subj = X509_NAME_oneline(X509_get_subject_name(cert), NULL, 0);
    rkey = ossl_cmp_ctx_get0_newPubkey(ctx);
    if (rkey != NULL
        /* X509_check_private_key() also works if rkey is just public key */
            && !(X509_check_private_key(ctx->newCert, rkey))) {
        fail_info = 1 << OSSL_CMP_PKIFAILUREINFO_incorrectData;
        txt = "public key in new certificate does not match our enrollment key";
        /*-
         * not calling (void)ossl_cmp_exchange_error(ctx,
         *                   OSSL_CMP_PKISTATUS_rejection, fail_info, txt)
         * not throwing CMP_R_CERTIFICATE_NOT_ACCEPTED with txt
         * not returning 0
         * since we better leave this for the certConf_cb to decide
         */
    }

    /*
     * Execute the certification checking callback function,
     * which can determine whether to accept a newly enrolled certificate.
     * It may overrule the pre-decision reflected in 'fail_info' and '*txt'.
     */
    cb = ctx->certConf_cb != NULL ? ctx->certConf_cb : OSSL_CMP_certConf_cb;
    if ((fail_info = cb(ctx, ctx->newCert, fail_info, &txt)) != 0
            && txt == NULL)
        txt = "CMP client did not accept it";
    if (fail_info != 0) /* immediately log error before any certConf exchange */
        ossl_cmp_log1(ERROR, ctx,
                      "rejecting newly enrolled cert with subject: %s", subj);
    /*
     * certConf exchange should better be moved to do_certreq_seq() such that
     * also more low-level errors with CertReqMessages get reported to server
     */
    if (!ctx->disableConfirm
            && !ossl_cmp_hdr_has_implicitConfirm((*resp)->header)) {
        if (!ossl_cmp_exchange_certConf(ctx, rid, fail_info, txt))
            ret = 0;
    }

    /* not throwing failure earlier as transfer_cb may call ERR_clear_error() */
    if (fail_info != 0) {
        ERR_raise_data(ERR_LIB_CMP, CMP_R_CERTIFICATE_NOT_ACCEPTED,
                       "rejecting newly enrolled cert with subject: %s; %s",
                       subj, txt);
        ctx->status = OSSL_CMP_PKISTATUS_rejection;
        ret = 0;
    }
    OPENSSL_free(subj);
    return ret;
}

static int initial_certreq(OSSL_CMP_CTX *ctx,
                           int req_type, const OSSL_CRMF_MSG *crm,
                           OSSL_CMP_MSG **p_rep, int rep_type)
{
    OSSL_CMP_MSG *req;
    int res;

    ctx->status = OSSL_CMP_PKISTATUS_request;
    if (!ossl_cmp_ctx_set0_newCert(ctx, NULL))
        return 0;

    /* also checks if all necessary options are set */
    if ((req = ossl_cmp_certreq_new(ctx, req_type, crm)) == NULL)
        return 0;

    ctx->status = OSSL_CMP_PKISTATUS_trans;
    res = send_receive_check(ctx, req, p_rep, rep_type);
    OSSL_CMP_MSG_free(req);
    return res;
}

int OSSL_CMP_try_certreq(OSSL_CMP_CTX *ctx, int req_type,
                         const OSSL_CRMF_MSG *crm, int *checkAfter)
{
    OSSL_CMP_MSG *rep = NULL;
    int is_p10 = req_type == OSSL_CMP_PKIBODY_P10CR;
    int rid = is_p10 ? OSSL_CMP_CERTREQID_NONE : OSSL_CMP_CERTREQID;
    int rep_type = is_p10 ? OSSL_CMP_PKIBODY_CP : req_type + 1;
    int res = 0;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (ctx->status != OSSL_CMP_PKISTATUS_waiting) { /* not polling already */
        if (!initial_certreq(ctx, req_type, crm, &rep, rep_type))
            goto err;

        if (!save_senderNonce_if_waiting(ctx, rep, rid))
            return 0;
    } else {
        if (req_type < 0)
            return ossl_cmp_exchange_error(ctx, OSSL_CMP_PKISTATUS_rejection,
                                           0, "polling aborted",
                                           0 /* errorCode */, "by application");
        res = poll_for_response(ctx, 0 /* no sleep */, rid, &rep, checkAfter);
        if (res <= 0) /* waiting or error */
            return res;
    }
    res = cert_response(ctx, 0 /* no sleep */, rid, &rep, checkAfter,
                        req_type, rep_type);

 err:
    OSSL_CMP_MSG_free(rep);
    return res;
}

/*-
 * Do the full sequence CR/IR/KUR/P10CR, CP/IP/KUP/CP,
 * certConf, PKIconf, and polling if required.
 * Will sleep as long as indicated by the server (according to checkAfter).
 * All enrollment options need to be present in the context.
 * Returns pointer to received certificate, or NULL if none was received.
 */
X509 *OSSL_CMP_exec_certreq(OSSL_CMP_CTX *ctx, int req_type,
                            const OSSL_CRMF_MSG *crm)
{
    OSSL_CMP_MSG *rep = NULL;
    int is_p10 = req_type == OSSL_CMP_PKIBODY_P10CR;
    int rid = is_p10 ? OSSL_CMP_CERTREQID_NONE : OSSL_CMP_CERTREQID;
    int rep_type = is_p10 ? OSSL_CMP_PKIBODY_CP : req_type + 1;
    X509 *result = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }

    if (!initial_certreq(ctx, req_type, crm, &rep, rep_type))
        goto err;

    if (!save_senderNonce_if_waiting(ctx, rep, rid))
        return 0;

    if (cert_response(ctx, 1 /* sleep */, rid, &rep, NULL, req_type, rep_type)
        <= 0)
        goto err;

    result = ctx->newCert;
 err:
    OSSL_CMP_MSG_free(rep);
    return result;
}

int OSSL_CMP_exec_RR_ses(OSSL_CMP_CTX *ctx)
{
    OSSL_CMP_MSG *rr = NULL;
    OSSL_CMP_MSG *rp = NULL;
    const int num_RevDetails = 1;
    const int rsid = OSSL_CMP_REVREQSID;
    OSSL_CMP_REVREPCONTENT *rrep = NULL;
    OSSL_CMP_PKISI *si = NULL;
    char buf[OSSL_CMP_PKISI_BUFLEN];
    int ret = 0;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return 0;
    }
    ctx->status = OSSL_CMP_PKISTATUS_request;
    if (ctx->oldCert == NULL && ctx->p10CSR == NULL
        && (ctx->serialNumber == NULL || ctx->issuer == NULL)) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_REFERENCE_CERT);
        return 0;
    }

    /* OSSL_CMP_rr_new() also checks if all necessary options are set */
    if ((rr = ossl_cmp_rr_new(ctx)) == NULL)
        goto end;

    ctx->status = OSSL_CMP_PKISTATUS_trans;
    if (!send_receive_also_delayed(ctx, rr, &rp, OSSL_CMP_PKIBODY_RP))
        goto end;

    rrep = rp->body->value.rp;
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    if (sk_OSSL_CMP_PKISI_num(rrep->status) != num_RevDetails) {
        ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_RP_COMPONENT_COUNT);
        goto end;
    }
#else
    if (sk_OSSL_CMP_PKISI_num(rrep->status) < 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_RP_COMPONENT_COUNT);
        goto end;
    }
#endif

    /* evaluate PKIStatus field */
    si = ossl_cmp_revrepcontent_get_pkisi(rrep, rsid);
    if (!save_statusInfo(ctx, si))
        goto err;
    switch (ossl_cmp_pkisi_get_status(si)) {
    case OSSL_CMP_PKISTATUS_accepted:
        ossl_cmp_info(ctx, "revocation accepted (PKIStatus=accepted)");
        ret = 1;
        break;
    case OSSL_CMP_PKISTATUS_grantedWithMods:
        ossl_cmp_info(ctx, "revocation accepted (PKIStatus=grantedWithMods)");
        ret = 1;
        break;
    case OSSL_CMP_PKISTATUS_rejection:
        ERR_raise(ERR_LIB_CMP, CMP_R_REQUEST_REJECTED_BY_SERVER);
        goto err;
    case OSSL_CMP_PKISTATUS_revocationWarning:
        ossl_cmp_info(ctx, "revocation accepted (PKIStatus=revocationWarning)");
        ret = 1;
        break;
    case OSSL_CMP_PKISTATUS_revocationNotification:
        /* interpretation as warning or error depends on CA */
        ossl_cmp_warn(ctx,
                      "revocation accepted (PKIStatus=revocationNotification)");
        ret = 1;
        break;
    case OSSL_CMP_PKISTATUS_waiting:
    case OSSL_CMP_PKISTATUS_keyUpdateWarning:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKISTATUS);
        goto err;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNKNOWN_PKISTATUS);
        goto err;
    }

    /* check any present CertId in optional revCerts field */
    if (sk_OSSL_CRMF_CERTID_num(rrep->revCerts) >= 1) {
        OSSL_CRMF_CERTID *cid;
        OSSL_CRMF_CERTTEMPLATE *tmpl =
            sk_OSSL_CMP_REVDETAILS_value(rr->body->value.rr, rsid)->certDetails;
        const X509_NAME *issuer = OSSL_CRMF_CERTTEMPLATE_get0_issuer(tmpl);
        const ASN1_INTEGER *serial =
            OSSL_CRMF_CERTTEMPLATE_get0_serialNumber(tmpl);

        if (sk_OSSL_CRMF_CERTID_num(rrep->revCerts) != num_RevDetails) {
            ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_RP_COMPONENT_COUNT);
            ret = 0;
            goto err;
        }
        if ((cid = ossl_cmp_revrepcontent_get_CertId(rrep, rsid)) == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_CERTID);
            ret = 0;
            goto err;
        }
        if (X509_NAME_cmp(issuer, OSSL_CRMF_CERTID_get0_issuer(cid)) != 0) {
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
            ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_CERTID_IN_RP);
            ret = 0;
            goto err;
#endif
        }
        if (ASN1_INTEGER_cmp(serial,
                             OSSL_CRMF_CERTID_get0_serialNumber(cid)) != 0) {
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
            ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_SERIAL_IN_RP);
            ret = 0;
            goto err;
#endif
        }
    }

    /* check number of any optionally present crls */
    if (rrep->crls != NULL && sk_X509_CRL_num(rrep->crls) != num_RevDetails) {
        ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_RP_COMPONENT_COUNT);
        ret = 0;
        goto err;
    }

 err:
    if (ret == 0
            && OSSL_CMP_CTX_snprint_PKIStatus(ctx, buf, sizeof(buf)) != NULL)
        ERR_add_error_data(1, buf);

 end:
    OSSL_CMP_MSG_free(rr);
    OSSL_CMP_MSG_free(rp);
    return ret;
}

STACK_OF(OSSL_CMP_ITAV) *OSSL_CMP_exec_GENM_ses(OSSL_CMP_CTX *ctx)
{
    OSSL_CMP_MSG *genm;
    OSSL_CMP_MSG *genp = NULL;
    STACK_OF(OSSL_CMP_ITAV) *itavs = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return NULL;
    }
    ctx->status = OSSL_CMP_PKISTATUS_request;

    if ((genm = ossl_cmp_genm_new(ctx)) == NULL)
        goto err;

    ctx->status = OSSL_CMP_PKISTATUS_trans;
    if (!send_receive_also_delayed(ctx, genm, &genp, OSSL_CMP_PKIBODY_GENP))
        goto err;
    ctx->status = OSSL_CMP_PKISTATUS_accepted;

    itavs = genp->body->value.genp;
    if (itavs == NULL)
        itavs = sk_OSSL_CMP_ITAV_new_null();
    /* received stack of itavs not to be freed with the genp */
    genp->body->value.genp = NULL;

 err:
    OSSL_CMP_MSG_free(genm);
    OSSL_CMP_MSG_free(genp);

    return itavs; /* NULL indicates error case */
}
