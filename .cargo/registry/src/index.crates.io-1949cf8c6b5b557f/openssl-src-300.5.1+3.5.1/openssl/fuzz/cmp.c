/*
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Test CMP DER parsing.
 */

#include <openssl/bio.h>
#include <openssl/cmp.h>
#include "../crypto/cmp/cmp_local.h"
#include <openssl/err.h>
#include "fuzzer.h"

int FuzzerInitialize(int *argc, char ***argv)
{
    FuzzerSetRand();
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    return 1;
}

static int num_responses;

static OSSL_CMP_MSG *transfer_cb(OSSL_CMP_CTX *ctx, const OSSL_CMP_MSG *req)
{
    if (num_responses++ > 2)
        return NULL; /* prevent loops due to repeated pollRep */
    return OSSL_CMP_MSG_dup((OSSL_CMP_MSG *)
                            OSSL_CMP_CTX_get_transfer_cb_arg(ctx));
}

static int print_noop(const char *func, const char *file, int line,
                      OSSL_CMP_severity level, const char *msg)
{
    return 1;
}

static int allow_unprotected(const OSSL_CMP_CTX *ctx, const OSSL_CMP_MSG *rep,
                             int invalid_protection, int expected_type)
{
    return 1;
}

static void cmp_client_process_response(OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg)
{
    X509_NAME *name = X509_NAME_new();
    ASN1_INTEGER *serial = ASN1_INTEGER_new();

    ctx->unprotectedSend = 1; /* satisfy ossl_cmp_msg_protect() */
    ctx->disableConfirm = 1; /* check just one response message */
    ctx->popoMethod = OSSL_CRMF_POPO_NONE; /* satisfy ossl_cmp_certReq_new() */
    ctx->oldCert = X509_new(); /* satisfy crm_new() and ossl_cmp_rr_new() */
    if (!OSSL_CMP_CTX_set1_secretValue(ctx, (unsigned char *)"",
                                       0) /* prevent too unspecific error */
            || ctx->oldCert == NULL
            || name == NULL || !X509_set_issuer_name(ctx->oldCert, name)
            || serial == NULL || !X509_set_serialNumber(ctx->oldCert, serial))
        goto err;

    (void)OSSL_CMP_CTX_set_transfer_cb(ctx, transfer_cb);
    (void)OSSL_CMP_CTX_set_transfer_cb_arg(ctx, msg);
    (void)OSSL_CMP_CTX_set_log_cb(ctx, print_noop);
    num_responses = 0;
    switch (msg->body != NULL ? msg->body->type : -1) {
    case OSSL_CMP_PKIBODY_IP:
        (void)OSSL_CMP_exec_IR_ses(ctx);
        break;
    case OSSL_CMP_PKIBODY_CP:
        (void)OSSL_CMP_exec_CR_ses(ctx);
        (void)OSSL_CMP_exec_P10CR_ses(ctx);
        break;
    case OSSL_CMP_PKIBODY_KUP:
        (void)OSSL_CMP_exec_KUR_ses(ctx);
        break;
    case OSSL_CMP_PKIBODY_POLLREP:
        ctx->status = OSSL_CMP_PKISTATUS_waiting;
        (void)OSSL_CMP_try_certreq(ctx, OSSL_CMP_PKIBODY_CR, NULL, NULL);
        break;
    case OSSL_CMP_PKIBODY_RP:
        (void)OSSL_CMP_exec_RR_ses(ctx);
        break;
    case OSSL_CMP_PKIBODY_GENP:
        sk_OSSL_CMP_ITAV_pop_free(OSSL_CMP_exec_GENM_ses(ctx),
                                  OSSL_CMP_ITAV_free);
        break;
    default:
        (void)ossl_cmp_msg_check_update(ctx, msg, allow_unprotected, 0);
        break;
    }
 err:
    X509_NAME_free(name);
    ASN1_INTEGER_free(serial);
}

static OSSL_CMP_PKISI *process_cert_request(OSSL_CMP_SRV_CTX *srv_ctx,
                                            const OSSL_CMP_MSG *cert_req,
                                            int certReqId,
                                            const OSSL_CRMF_MSG *crm,
                                            const X509_REQ *p10cr,
                                            X509 **certOut,
                                            STACK_OF(X509) **chainOut,
                                            STACK_OF(X509) **caPubs)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
    return NULL;
}

static OSSL_CMP_PKISI *process_rr(OSSL_CMP_SRV_CTX *srv_ctx,
                                  const OSSL_CMP_MSG *rr,
                                  const X509_NAME *issuer,
                                  const ASN1_INTEGER *serial)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
    return NULL;
}

static int process_genm(OSSL_CMP_SRV_CTX *srv_ctx,
                        const OSSL_CMP_MSG *genm,
                        const STACK_OF(OSSL_CMP_ITAV) *in,
                        STACK_OF(OSSL_CMP_ITAV) **out)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
    return 0;
}

static void process_error(OSSL_CMP_SRV_CTX *srv_ctx, const OSSL_CMP_MSG *error,
                          const OSSL_CMP_PKISI *statusInfo,
                          const ASN1_INTEGER *errorCode,
                          const OSSL_CMP_PKIFREETEXT *errorDetails)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
}

static int process_certConf(OSSL_CMP_SRV_CTX *srv_ctx,
                            const OSSL_CMP_MSG *certConf, int certReqId,
                            const ASN1_OCTET_STRING *certHash,
                            const OSSL_CMP_PKISI *si)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
    return 0;
}

static int process_pollReq(OSSL_CMP_SRV_CTX *srv_ctx,
                           const OSSL_CMP_MSG *pollReq, int certReqId,
                           OSSL_CMP_MSG **certReq, int64_t *check_after)
{
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROCESSING_MESSAGE);
    return 0;
}

static int clean_transaction(ossl_unused OSSL_CMP_SRV_CTX *srv_ctx,
                             ossl_unused const ASN1_OCTET_STRING *id)
{
    return 1;
}

static int delayed_delivery(ossl_unused OSSL_CMP_SRV_CTX *srv_ctx,
                            ossl_unused const OSSL_CMP_MSG *req)
{
    return 0;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    OSSL_CMP_MSG *msg;
    BIO *in;

    if (len == 0)
        return 0;

    in = BIO_new(BIO_s_mem());
    OPENSSL_assert((size_t)BIO_write(in, buf, len) == len);
    msg = d2i_OSSL_CMP_MSG_bio(in, NULL);
    if (msg != NULL) {
        BIO *out = BIO_new(BIO_s_null());
        OSSL_CMP_SRV_CTX *srv_ctx = OSSL_CMP_SRV_CTX_new(NULL, NULL);
        OSSL_CMP_CTX *client_ctx = OSSL_CMP_CTX_new(NULL, NULL);

        i2d_OSSL_CMP_MSG_bio(out, msg);
        ASN1_item_print(out, (ASN1_VALUE *)msg, 4,
                        ASN1_ITEM_rptr(OSSL_CMP_MSG), NULL);
        BIO_free(out);

        if (client_ctx != NULL)
            cmp_client_process_response(client_ctx, msg);
        if (srv_ctx != NULL
            && OSSL_CMP_CTX_set_log_cb(OSSL_CMP_SRV_CTX_get0_cmp_ctx(srv_ctx),
                                       print_noop)
            && OSSL_CMP_SRV_CTX_init(srv_ctx, NULL, process_cert_request,
                                     process_rr, process_genm, process_error,
                                     process_certConf, process_pollReq)
            && OSSL_CMP_SRV_CTX_init_trans(srv_ctx, delayed_delivery,
                                           clean_transaction))
            OSSL_CMP_MSG_free(OSSL_CMP_SRV_process_request(srv_ctx, msg));

        OSSL_CMP_CTX_free(client_ctx);
        OSSL_CMP_SRV_CTX_free(srv_ctx);
        OSSL_CMP_MSG_free(msg);
    }

    BIO_free(in);
    ERR_clear_error();

    return 0;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
