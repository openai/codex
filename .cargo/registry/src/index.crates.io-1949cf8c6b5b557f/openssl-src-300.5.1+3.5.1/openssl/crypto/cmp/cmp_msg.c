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

/* CMP functions for PKIMessage construction */

#include "cmp_local.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/asn1t.h>
#include <openssl/cmp.h>
#include <openssl/crmf.h>
#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/bio.h>
#include <internal/cms.h>

OSSL_CMP_MSG *OSSL_CMP_MSG_new(OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_CMP_MSG *msg = NULL;

    msg = (OSSL_CMP_MSG *)ASN1_item_new_ex(ASN1_ITEM_rptr(OSSL_CMP_MSG),
                                           libctx, propq);
    if (!ossl_cmp_msg_set0_libctx(msg, libctx, propq)) {
        OSSL_CMP_MSG_free(msg);
        msg = NULL;
    }
    return msg;
}

void OSSL_CMP_MSG_free(OSSL_CMP_MSG *msg)
{
    ASN1_item_free((ASN1_VALUE *)msg, ASN1_ITEM_rptr(OSSL_CMP_MSG));
}

/*
 * This should only be used if the X509 object was embedded inside another
 * asn1 object and it needs a libctx to operate.
 * Use OSSL_CMP_MSG_new() instead if possible.
 */
int ossl_cmp_msg_set0_libctx(OSSL_CMP_MSG *msg, OSSL_LIB_CTX *libctx,
                             const char *propq)
{
    if (msg != NULL) {
        msg->libctx = libctx;
        OPENSSL_free(msg->propq);
        msg->propq = NULL;
        if (propq != NULL) {
            msg->propq = OPENSSL_strdup(propq);
            if (msg->propq == NULL)
                return 0;
        }
    }
    return 1;
}

OSSL_CMP_PKIHEADER *OSSL_CMP_MSG_get0_header(const OSSL_CMP_MSG *msg)
{
    if (msg == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return msg->header;
}

const char *ossl_cmp_bodytype_to_string(int type)
{
    static const char *type_names[] = {
        "IR", "IP", "CR", "CP", "P10CR",
        "POPDECC", "POPDECR", "KUR", "KUP",
        "KRR", "KRP", "RR", "RP", "CCR", "CCP",
        "CKUANN", "CANN", "RANN", "CRLANN", "PKICONF", "NESTED",
        "GENM", "GENP", "ERROR", "CERTCONF", "POLLREQ", "POLLREP",
    };

    if (type < 0 || type > OSSL_CMP_PKIBODY_TYPE_MAX)
        return "illegal body type";
    return type_names[type];
}

int ossl_cmp_msg_set_bodytype(OSSL_CMP_MSG *msg, int type)
{
    if (!ossl_assert(msg != NULL && msg->body != NULL))
        return 0;

    msg->body->type = type;
    return 1;
}

int OSSL_CMP_MSG_get_bodytype(const OSSL_CMP_MSG *msg)
{
    if (!ossl_assert(msg != NULL && msg->body != NULL))
        return -1;

    return msg->body->type;
}

X509_PUBKEY *OSSL_CMP_MSG_get0_certreq_publickey(const OSSL_CMP_MSG *msg)
{
    const OSSL_CRMF_MSGS *reqs;
    const OSSL_CRMF_MSG *crm;
    const OSSL_CRMF_CERTTEMPLATE *tmpl;
    X509_PUBKEY *pubkey;

    switch (OSSL_CMP_MSG_get_bodytype(msg)) {
    case OSSL_CMP_PKIBODY_IR:
    case OSSL_CMP_PKIBODY_CR:
    case OSSL_CMP_PKIBODY_KUR:
        reqs = msg->body->value.ir; /* value.ir is same for cr and kur */
        if ((crm = sk_OSSL_CRMF_MSG_value(reqs, 0)) == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_CERTREQMSG_NOT_FOUND);
            return NULL;
        }
        if ((tmpl = OSSL_CRMF_MSG_get0_tmpl(crm)) == NULL
            || (pubkey = OSSL_CRMF_CERTTEMPLATE_get0_publicKey(tmpl)) == NULL) {
            ERR_raise(ERR_LIB_CMP, CRMF_R_POPO_MISSING_PUBLIC_KEY);
            return NULL;
        }
        return pubkey;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        return NULL;
    }
}

/* Add an extension to the referenced extension stack, which may be NULL */
static int add1_extension(X509_EXTENSIONS **pexts, int nid, int crit, void *ex)
{
    X509_EXTENSION *ext;
    int res;

    if (!ossl_assert(pexts != NULL)) /* pointer to var must not be NULL */
        return 0;

    if ((ext = X509V3_EXT_i2d(nid, crit, ex)) == NULL)
        return 0;

    res = X509v3_add_ext(pexts, ext, 0) != NULL;
    X509_EXTENSION_free(ext);
    return res;
}

/* Add a CRL revocation reason code to extension stack, which may be NULL */
static int add_crl_reason_extension(X509_EXTENSIONS **pexts, int reason_code)
{
    ASN1_ENUMERATED *val = ASN1_ENUMERATED_new();
    int res = 0;

    if (val != NULL && ASN1_ENUMERATED_set(val, reason_code))
        res = add1_extension(pexts, NID_crl_reason, 0 /* non-critical */, val);
    ASN1_ENUMERATED_free(val);
    return res;
}

OSSL_CMP_MSG *ossl_cmp_msg_create(OSSL_CMP_CTX *ctx, int bodytype)
{
    OSSL_CMP_MSG *msg = NULL;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if ((msg = OSSL_CMP_MSG_new(ctx->libctx, ctx->propq)) == NULL)
        return NULL;
    if (!ossl_cmp_hdr_init(ctx, msg->header)
            || !ossl_cmp_msg_set_bodytype(msg, bodytype))
        goto err;
    if (ctx->geninfo_ITAVs != NULL
            && !ossl_cmp_hdr_generalInfo_push1_items(msg->header,
                                                     ctx->geninfo_ITAVs))
        goto err;

    switch (bodytype) {
    case OSSL_CMP_PKIBODY_IR:
    case OSSL_CMP_PKIBODY_CR:
    case OSSL_CMP_PKIBODY_KUR:
        if ((msg->body->value.ir = OSSL_CRMF_MSGS_new()) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_P10CR:
        if (ctx->p10CSR == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_P10CSR);
            goto err;
        }
        if ((msg->body->value.p10cr = X509_REQ_dup(ctx->p10CSR)) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_IP:
    case OSSL_CMP_PKIBODY_CP:
    case OSSL_CMP_PKIBODY_KUP:
        if ((msg->body->value.ip = OSSL_CMP_CERTREPMESSAGE_new()) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_RR:
        if ((msg->body->value.rr = sk_OSSL_CMP_REVDETAILS_new_null()) == NULL)
            goto err;
        return msg;
    case OSSL_CMP_PKIBODY_RP:
        if ((msg->body->value.rp = OSSL_CMP_REVREPCONTENT_new()) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_CERTCONF:
        if ((msg->body->value.certConf =
             sk_OSSL_CMP_CERTSTATUS_new_null()) == NULL)
            goto err;
        return msg;
    case OSSL_CMP_PKIBODY_PKICONF:
        if ((msg->body->value.pkiconf = ASN1_TYPE_new()) == NULL)
            goto err;
        ASN1_TYPE_set(msg->body->value.pkiconf, V_ASN1_NULL, NULL);
        return msg;

    case OSSL_CMP_PKIBODY_POLLREQ:
        if ((msg->body->value.pollReq = sk_OSSL_CMP_POLLREQ_new_null()) == NULL)
            goto err;
        return msg;
    case OSSL_CMP_PKIBODY_POLLREP:
        if ((msg->body->value.pollRep = sk_OSSL_CMP_POLLREP_new_null()) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_GENM:
    case OSSL_CMP_PKIBODY_GENP:
        if ((msg->body->value.genm = sk_OSSL_CMP_ITAV_new_null()) == NULL)
            goto err;
        return msg;

    case OSSL_CMP_PKIBODY_ERROR:
        if ((msg->body->value.error = OSSL_CMP_ERRORMSGCONTENT_new()) == NULL)
            goto err;
        return msg;

    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_PKIBODY);
        goto err;
    }

 err:
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

#define HAS_SAN(ctx) \
    (sk_GENERAL_NAME_num((ctx)->subjectAltNames) > 0 \
         || OSSL_CMP_CTX_reqExtensions_have_SAN(ctx) == 1)

static const X509_NAME *determine_subj(OSSL_CMP_CTX *ctx, int for_KUR,
                                       const X509_NAME *ref_subj)
{
    if (ctx->subjectName != NULL)
        return IS_NULL_DN(ctx->subjectName) ? NULL : ctx->subjectName;
    if (ctx->p10CSR != NULL) /* first default is from any given CSR */
        return X509_REQ_get_subject_name(ctx->p10CSR);
    if (for_KUR || !HAS_SAN(ctx))
        /*
         * For KUR, copy subject from any reference cert as fallback.
         * For IR or CR, do the same only if there is no subjectAltName.
         */
        return ref_subj;
    return NULL;
}

OSSL_CRMF_MSG *OSSL_CMP_CTX_setup_CRM(OSSL_CMP_CTX *ctx, int for_KUR, int rid)
{
    OSSL_CRMF_MSG *crm = NULL;
    int central_keygen = OSSL_CMP_CTX_get_option(ctx, OSSL_CMP_OPT_POPO_METHOD)
        == OSSL_CRMF_POPO_NONE;
    X509 *refcert = ctx->oldCert != NULL ? ctx->oldCert : ctx->cert;
    /* refcert defaults to current client cert */
    EVP_PKEY *rkey = ossl_cmp_ctx_get0_newPubkey(ctx);
    STACK_OF(GENERAL_NAME) *default_sans = NULL;
    const X509_NAME *ref_subj =
        refcert != NULL ? X509_get_subject_name(refcert) : NULL;
    const X509_NAME *subject = determine_subj(ctx, for_KUR, ref_subj);
    const X509_NAME *issuer = ctx->issuer != NULL || refcert == NULL
        ? (IS_NULL_DN(ctx->issuer) ? NULL : ctx->issuer)
        : X509_get_issuer_name(refcert);
    int crit = ctx->setSubjectAltNameCritical || subject == NULL;
    /* RFC5280: subjectAltName MUST be critical if subject is null */
    OSSL_CRMF_CERTTEMPLATE *tmpl;
    X509_EXTENSIONS *exts = NULL;

    if (rkey == NULL && !central_keygen) {
#ifndef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
        ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_PUBLIC_KEY);
        return NULL;
#endif
    }
    if (for_KUR && refcert == NULL && ctx->p10CSR == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_REFERENCE_CERT);
        return NULL;
    }
    if ((crm = OSSL_CRMF_MSG_new()) == NULL)
        return NULL;
    tmpl = OSSL_CRMF_MSG_get0_tmpl(crm);
    if (!OSSL_CRMF_MSG_set_certReqId(crm, rid)
            /*
             * fill certTemplate, corresponding to CertificationRequestInfo
             * of PKCS#10. The rkey param cannot be NULL so far -
             * it could be NULL if centralized key creation was supported
             */
            || !OSSL_CRMF_CERTTEMPLATE_fill(OSSL_CRMF_MSG_get0_tmpl(crm), rkey,
                                            subject, issuer, NULL /* serial */))
        goto err;
    if (rkey != NULL && central_keygen)
        X509_PUBKEY_set0_public_key(OSSL_CRMF_CERTTEMPLATE_get0_publicKey(tmpl),
                                    NULL, 0);

    if (ctx->days != 0) {
        time_t now = time(NULL);
        ASN1_TIME *notBefore = ASN1_TIME_adj(NULL, now, 0, 0);
        ASN1_TIME *notAfter = ASN1_TIME_adj(NULL, now, ctx->days, 0);

        if (notBefore == NULL
                || notAfter == NULL
                || !OSSL_CRMF_MSG_set0_validity(crm, notBefore, notAfter)) {
            ASN1_TIME_free(notBefore);
            ASN1_TIME_free(notAfter);
            goto err;
        }
    }

    /* extensions */
    if (ctx->p10CSR != NULL
            && (exts = X509_REQ_get_extensions(ctx->p10CSR)) == NULL)
        goto err;
    if (!ctx->SubjectAltName_nodefault && !HAS_SAN(ctx) && refcert != NULL
        && (default_sans = X509V3_get_d2i(X509_get0_extensions(refcert),
                                          NID_subject_alt_name, NULL, NULL))
        != NULL
            && !add1_extension(&exts, NID_subject_alt_name, crit, default_sans))
        goto err;
    if (sk_X509_EXTENSION_num(ctx->reqExtensions) > 0 /* augment/override existing ones */
            && X509v3_add_extensions(&exts, ctx->reqExtensions) == NULL)
        goto err;
    if (sk_GENERAL_NAME_num(ctx->subjectAltNames) > 0
            && !add1_extension(&exts, NID_subject_alt_name,
                               crit, ctx->subjectAltNames))
        goto err;
    if (ctx->policies != NULL
            && !add1_extension(&exts, NID_certificate_policies,
                               ctx->setPoliciesCritical, ctx->policies))
        goto err;
    if (!OSSL_CRMF_MSG_set0_extensions(crm, exts))
        goto err;
    exts = NULL;
    /* end fill certTemplate, now set any controls */

    /* for KUR, set OldCertId according to D.6 */
    if (for_KUR && refcert != NULL) {
        OSSL_CRMF_CERTID *cid =
            OSSL_CRMF_CERTID_gen(X509_get_issuer_name(refcert),
                                 X509_get0_serialNumber(refcert));
        int ret;

        if (cid == NULL)
            goto err;
        ret = OSSL_CRMF_MSG_set1_regCtrl_oldCertID(crm, cid);
        OSSL_CRMF_CERTID_free(cid);
        if (ret == 0)
            goto err;
    }

    goto end;

 err:
    OSSL_CRMF_MSG_free(crm);
    crm = NULL;

 end:
    sk_X509_EXTENSION_pop_free(exts, X509_EXTENSION_free);
    sk_GENERAL_NAME_pop_free(default_sans, GENERAL_NAME_free);
    return crm;
}

OSSL_CMP_MSG *ossl_cmp_certreq_new(OSSL_CMP_CTX *ctx, int type,
                                   const OSSL_CRMF_MSG *crm)
{
    OSSL_CMP_MSG *msg;
    OSSL_CRMF_MSG *local_crm = NULL;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if (type != OSSL_CMP_PKIBODY_IR && type != OSSL_CMP_PKIBODY_CR
            && type != OSSL_CMP_PKIBODY_KUR && type != OSSL_CMP_PKIBODY_P10CR) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return NULL;
    }
    if (type == OSSL_CMP_PKIBODY_P10CR && crm != NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return NULL;
    }

    if ((msg = ossl_cmp_msg_create(ctx, type)) == NULL)
        goto err;

    /* header */
    if (ctx->implicitConfirm && !ossl_cmp_hdr_set_implicitConfirm(msg->header))
        goto err;

    /* body */
    /* For P10CR the content has already been set in OSSL_CMP_MSG_create */
    if (type != OSSL_CMP_PKIBODY_P10CR) {
        EVP_PKEY *privkey = OSSL_CMP_CTX_get0_newPkey(ctx, 1);

        /* privkey is ctx->newPkey (if private, else NULL) or ctx->pkey */
        if (ctx->popoMethod >= OSSL_CRMF_POPO_SIGNATURE && privkey == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_PRIVATE_KEY_FOR_POPO);
            goto err;
        }
        if (crm == NULL) {
            local_crm = OSSL_CMP_CTX_setup_CRM(ctx,
                                               type == OSSL_CMP_PKIBODY_KUR,
                                               OSSL_CMP_CERTREQID);
            if (local_crm == NULL
                || !OSSL_CRMF_MSG_create_popo(ctx->popoMethod, local_crm,
                                              privkey, ctx->digest,
                                              ctx->libctx, ctx->propq))
                goto err;
        } else {
            if ((local_crm = OSSL_CRMF_MSG_dup(crm)) == NULL)
                goto err;
        }

        /* value.ir is same for cr and kur */
        if (!sk_OSSL_CRMF_MSG_push(msg->body->value.ir, local_crm))
            goto err;
        local_crm = NULL;
    }

    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_CERTREQ);
    OSSL_CRMF_MSG_free(local_crm);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

#ifndef OPENSSL_NO_CMS
static OSSL_CRMF_ENCRYPTEDKEY *enc_privkey(OSSL_CMP_CTX *ctx, const EVP_PKEY *pkey)
{
    OSSL_CRMF_ENCRYPTEDKEY *ek = NULL;
    CMS_EnvelopedData *envData = NULL;
    BIO *privbio = NULL;
    EVP_CIPHER *cipher = NULL;
    X509 *recip = ctx->validatedSrvCert; /* this is the client cert */
    STACK_OF(X509) *encryption_recips = sk_X509_new_reserve(NULL, 1);

    if (encryption_recips == NULL
        || !X509_add_cert(encryption_recips, recip, X509_ADD_FLAG_UP_REF))
        goto err;

    privbio = BIO_new(BIO_s_mem());
    if (privbio == NULL || i2d_PrivateKey_bio(privbio, pkey) <= 0)
        goto err;
    ossl_cmp_set_own_chain(ctx);
    cipher = EVP_CIPHER_fetch(ctx->libctx, SN_aes_256_cbc, ctx->propq);
    envData = ossl_cms_sign_encrypt(privbio, ctx->cert, ctx->chain, ctx->pkey, CMS_BINARY,
                                    encryption_recips, cipher, CMS_BINARY,
                                    ctx->libctx, ctx->propq);
    EVP_CIPHER_free(cipher);
    if (envData == NULL)
        goto err;
    ek = OSSL_CRMF_ENCRYPTEDKEY_init_envdata(envData);

 err:
    sk_X509_pop_free(encryption_recips, X509_free);
    BIO_free(privbio);
    if (ek == NULL)
        M_ASN1_free_of(envData, CMS_EnvelopedData);

    return ek;
}
#endif

OSSL_CMP_MSG *ossl_cmp_certrep_new(OSSL_CMP_CTX *ctx, int bodytype,
                                   int certReqId, const OSSL_CMP_PKISI *si,
                                   X509 *cert, const EVP_PKEY *pkey,
                                   const X509 *encryption_recip,
                                   STACK_OF(X509) *chain, STACK_OF(X509) *caPubs,
                                   int unprotectedErrors)
{
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_CERTREPMESSAGE *repMsg = NULL;
    OSSL_CMP_CERTRESPONSE *resp = NULL;
    int status = OSSL_CMP_PKISTATUS_unspecified;

    if (!ossl_assert(ctx != NULL && si != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, bodytype)) == NULL)
        goto err;
    repMsg = msg->body->value.ip; /* value.ip is same for cp and kup */

    /* header */
    if (ctx->implicitConfirm && !ossl_cmp_hdr_set_implicitConfirm(msg->header))
        goto err;

    /* body */
    if ((resp = OSSL_CMP_CERTRESPONSE_new()) == NULL)
        goto err;
    OSSL_CMP_PKISI_free(resp->status);
    if ((resp->status = OSSL_CMP_PKISI_dup(si)) == NULL
            || !ASN1_INTEGER_set(resp->certReqId, certReqId))
        goto err;

    status = ossl_cmp_pkisi_get_status(resp->status);
    if (status != OSSL_CMP_PKISTATUS_rejection
            && status != OSSL_CMP_PKISTATUS_waiting && cert != NULL) {
        if (encryption_recip != NULL) {
            ERR_raise(ERR_LIB_CMP, ERR_R_UNSUPPORTED);
            goto err;
        }

        if ((resp->certifiedKeyPair = OSSL_CMP_CERTIFIEDKEYPAIR_new())
            == NULL)
            goto err;
        resp->certifiedKeyPair->certOrEncCert->type =
            OSSL_CMP_CERTORENCCERT_CERTIFICATE;
        if (!X509_up_ref(cert))
            goto err;
        resp->certifiedKeyPair->certOrEncCert->value.certificate = cert;

        if (pkey != NULL) {
#ifndef OPENSSL_NO_CMS
            if ((resp->certifiedKeyPair->privateKey = enc_privkey(ctx, pkey)) == NULL)
                goto err;
#else
            ERR_raise(ERR_LIB_CMP, ERR_R_UNSUPPORTED);
            goto err;
#endif
        }
    }

    if (!sk_OSSL_CMP_CERTRESPONSE_push(repMsg->response, resp))
        goto err;
    resp = NULL;

    if (bodytype == OSSL_CMP_PKIBODY_IP && caPubs != NULL
            && (repMsg->caPubs = X509_chain_up_ref(caPubs)) == NULL)
        goto err;
    if (sk_X509_num(chain) > 0
        && !ossl_x509_add_certs_new(&msg->extraCerts, chain,
                                    X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP))
        goto err;

    if (!unprotectedErrors
            || ossl_cmp_pkisi_get_status(si) != OSSL_CMP_PKISTATUS_rejection)
        if (!ossl_cmp_msg_protect(ctx, msg))
            goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_CERTREP);
    OSSL_CMP_CERTRESPONSE_free(resp);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_rr_new(OSSL_CMP_CTX *ctx)
{
    OSSL_CMP_MSG *msg = NULL;
    const X509_NAME *issuer = NULL;
    const X509_NAME *subject = NULL;
    const ASN1_INTEGER *serialNumber = NULL;
    EVP_PKEY *pubkey = NULL;
    OSSL_CMP_REVDETAILS *rd;
    int ret;

    if (!ossl_assert(ctx != NULL
                     && (ctx->oldCert != NULL || ctx->p10CSR != NULL
                         || (ctx->serialNumber != NULL && ctx->issuer != NULL))))
        return NULL;

    if ((rd = OSSL_CMP_REVDETAILS_new()) == NULL)
        goto err;

    if (ctx->serialNumber != NULL && ctx->issuer != NULL) {
        issuer = ctx->issuer;
        serialNumber = ctx->serialNumber;
    } else if (ctx->oldCert != NULL) {
        issuer = X509_get_issuer_name(ctx->oldCert);
        serialNumber = X509_get0_serialNumber(ctx->oldCert);
    } else if (ctx->p10CSR != NULL) {
        pubkey = X509_REQ_get0_pubkey(ctx->p10CSR);
        subject = X509_REQ_get_subject_name(ctx->p10CSR);
    } else {
        goto err;
    }

    /* Fill the template from the contents of the certificate to be revoked */
    ret = OSSL_CRMF_CERTTEMPLATE_fill(rd->certDetails, pubkey, subject,
                                      issuer, serialNumber);
    if (!ret)
        goto err;

    /* revocation reason code is optional */
    if (ctx->revocationReason != CRL_REASON_NONE
            && !add_crl_reason_extension(&rd->crlEntryDetails,
                                         ctx->revocationReason))
        goto err;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_RR)) == NULL)
        goto err;

    if (!sk_OSSL_CMP_REVDETAILS_push(msg->body->value.rr, rd))
        goto err;
    rd = NULL;
    /* Revocation Passphrase according to section 5.3.19.9 could be set here */

    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_RR);
    OSSL_CMP_MSG_free(msg);
    OSSL_CMP_REVDETAILS_free(rd);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_rp_new(OSSL_CMP_CTX *ctx, const OSSL_CMP_PKISI *si,
                              const OSSL_CRMF_CERTID *cid, int unprotectedErrors)
{
    OSSL_CMP_REVREPCONTENT *rep = NULL;
    OSSL_CMP_PKISI *si1 = NULL;
    OSSL_CRMF_CERTID *cid_copy = NULL;
    OSSL_CMP_MSG *msg = NULL;

    if (!ossl_assert(ctx != NULL && si != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_RP)) == NULL)
        goto err;
    rep = msg->body->value.rp;

    if ((si1 = OSSL_CMP_PKISI_dup(si)) == NULL
            || !sk_OSSL_CMP_PKISI_push(rep->status, si1))
        goto err;

    si1 = NULL; /* ownership transferred to rep->status */

    if ((rep->revCerts = sk_OSSL_CRMF_CERTID_new_null()) == NULL)
        goto err;
    if (cid != NULL) {
        if ((cid_copy = OSSL_CRMF_CERTID_dup(cid)) == NULL
                || !sk_OSSL_CRMF_CERTID_push(rep->revCerts, cid_copy))
            goto err;

        cid_copy = NULL; /* ownership transferred to rep->revCerts */
    }

    if (!unprotectedErrors
            || ossl_cmp_pkisi_get_status(si) != OSSL_CMP_PKISTATUS_rejection)
        if (!ossl_cmp_msg_protect(ctx, msg))
            goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_RP);
    OSSL_CMP_PKISI_free(si1);
    OSSL_CRMF_CERTID_free(cid_copy);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_pkiconf_new(OSSL_CMP_CTX *ctx)
{
    OSSL_CMP_MSG *msg;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_PKICONF)) == NULL)
        goto err;
    if (ossl_cmp_msg_protect(ctx, msg))
        return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_PKICONF);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

int ossl_cmp_msg_gen_push0_ITAV(OSSL_CMP_MSG *msg, OSSL_CMP_ITAV *itav)
{
    int bodytype;

    if (!ossl_assert(msg != NULL && itav != NULL))
        return 0;

    bodytype = OSSL_CMP_MSG_get_bodytype(msg);
    if (bodytype != OSSL_CMP_PKIBODY_GENM
            && bodytype != OSSL_CMP_PKIBODY_GENP) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ARGS);
        return 0;
    }

    /* value.genp has the same structure, so this works for genp as well */
    return OSSL_CMP_ITAV_push0_stack_item(&msg->body->value.genm, itav);
}

int ossl_cmp_msg_gen_push1_ITAVs(OSSL_CMP_MSG *msg,
                                 const STACK_OF(OSSL_CMP_ITAV) *itavs)
{
    int i;
    OSSL_CMP_ITAV *itav = NULL;

    if (!ossl_assert(msg != NULL))
        return 0;

    for (i = 0; i < sk_OSSL_CMP_ITAV_num(itavs); i++) {
        itav = OSSL_CMP_ITAV_dup(sk_OSSL_CMP_ITAV_value(itavs, i));
        if (itav == NULL
                || !ossl_cmp_msg_gen_push0_ITAV(msg, itav)) {
            OSSL_CMP_ITAV_free(itav);
            return 0;
        }
    }
    return 1;
}

/*
 * Creates a new General Message/Response with a copy of the given itav stack
 * returns a pointer to the PKIMessage on success, NULL on error
 */
static OSSL_CMP_MSG *gen_new(OSSL_CMP_CTX *ctx,
                             const STACK_OF(OSSL_CMP_ITAV) *itavs,
                             int body_type, int err_code)
{
    OSSL_CMP_MSG *msg = NULL;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, body_type)) == NULL)
        return NULL;

    if (itavs != NULL && !ossl_cmp_msg_gen_push1_ITAVs(msg, itavs))
        goto err;

    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, err_code);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_genm_new(OSSL_CMP_CTX *ctx)
{
    return gen_new(ctx, ctx->genm_ITAVs,
                   OSSL_CMP_PKIBODY_GENM, CMP_R_ERROR_CREATING_GENM);
}

OSSL_CMP_MSG *ossl_cmp_genp_new(OSSL_CMP_CTX *ctx,
                                const STACK_OF(OSSL_CMP_ITAV) *itavs)
{
    return gen_new(ctx, itavs,
                   OSSL_CMP_PKIBODY_GENP, CMP_R_ERROR_CREATING_GENP);
}

OSSL_CMP_MSG *ossl_cmp_error_new(OSSL_CMP_CTX *ctx, const OSSL_CMP_PKISI *si,
                                 int64_t errorCode, const char *details,
                                 int unprotected)
{
    OSSL_CMP_MSG *msg = NULL;
    const char *lib = NULL, *reason = NULL;
    OSSL_CMP_PKIFREETEXT *ft;

    if (!ossl_assert(ctx != NULL && si != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_ERROR)) == NULL)
        goto err;

    OSSL_CMP_PKISI_free(msg->body->value.error->pKIStatusInfo);
    if ((msg->body->value.error->pKIStatusInfo = OSSL_CMP_PKISI_dup(si))
        == NULL)
        goto err;
    if ((msg->body->value.error->errorCode = ASN1_INTEGER_new()) == NULL)
        goto err;
    if (!ASN1_INTEGER_set_int64(msg->body->value.error->errorCode, errorCode))
        goto err;
    if (errorCode > 0
            && (uint64_t)errorCode < ((uint64_t)ERR_SYSTEM_FLAG << 1)) {
        lib = ERR_lib_error_string((unsigned long)errorCode);
        reason = ERR_reason_error_string((unsigned long)errorCode);
    }
    if (lib != NULL || reason != NULL || details != NULL) {
        if ((ft = sk_ASN1_UTF8STRING_new_null()) == NULL)
            goto err;
        msg->body->value.error->errorDetails = ft;
        if (lib != NULL && *lib != '\0'
                && !ossl_cmp_sk_ASN1_UTF8STRING_push_str(ft, lib, -1))
            goto err;
        if (reason != NULL && *reason != '\0'
                && !ossl_cmp_sk_ASN1_UTF8STRING_push_str(ft, reason, -1))
            goto err;
        if (details != NULL
                && !ossl_cmp_sk_ASN1_UTF8STRING_push_str(ft, details, -1))
            goto err;
    }

    if (!unprotected && !ossl_cmp_msg_protect(ctx, msg))
        goto err;
    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_ERROR);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

/*
 * Set the certHash field of a OSSL_CMP_CERTSTATUS structure.
 * This is used in the certConf message, for example,
 * to confirm that the certificate was received successfully.
 */
int ossl_cmp_certstatus_set0_certHash(OSSL_CMP_CERTSTATUS *certStatus,
                                      ASN1_OCTET_STRING *hash)
{
    if (!ossl_assert(certStatus != NULL))
        return 0;
    ASN1_OCTET_STRING_free(certStatus->certHash);
    certStatus->certHash = hash;
    return 1;
}

OSSL_CMP_MSG *ossl_cmp_certConf_new(OSSL_CMP_CTX *ctx, int certReqId,
                                    int fail_info, const char *text)
{
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_CERTSTATUS *certStatus = NULL;
    EVP_MD *md;
    int is_fallback;
    ASN1_OCTET_STRING *certHash = NULL;
    OSSL_CMP_PKISI *sinfo;

    if (!ossl_assert(ctx != NULL && ctx->newCert != NULL
                     && (certReqId == OSSL_CMP_CERTREQID
                         || certReqId == OSSL_CMP_CERTREQID_NONE)))
        return NULL;

    if ((unsigned)fail_info > OSSL_CMP_PKIFAILUREINFO_MAX_BIT_PATTERN) {
        ERR_raise(ERR_LIB_CMP, CMP_R_FAIL_INFO_OUT_OF_RANGE);
        return NULL;
    }

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_CERTCONF)) == NULL)
        goto err;

    if ((certStatus = OSSL_CMP_CERTSTATUS_new()) == NULL)
        goto err;
    /* consume certStatus into msg right away so it gets deallocated with msg */
    if (sk_OSSL_CMP_CERTSTATUS_push(msg->body->value.certConf, certStatus) < 1) {
        OSSL_CMP_CERTSTATUS_free(certStatus);
        goto err;
    }

    /* set the ID of the certReq */
    if (!ASN1_INTEGER_set(certStatus->certReqId, certReqId))
        goto err;

    certStatus->hashAlg = NULL;
    /*
     * The hash of the certificate, using the same hash algorithm
     * as is used to create and verify the certificate signature.
     * If not available, a fallback hash algorithm is used.
     */
    if ((certHash = X509_digest_sig(ctx->newCert, &md, &is_fallback)) == NULL)
        goto err;
    if (is_fallback) {
        if (!ossl_cmp_hdr_set_pvno(msg->header, OSSL_CMP_PVNO_3))
            goto err;
        if ((certStatus->hashAlg = X509_ALGOR_new()) == NULL)
            goto err;
        X509_ALGOR_set_md(certStatus->hashAlg, md);
    }
    EVP_MD_free(md);

    if (!ossl_cmp_certstatus_set0_certHash(certStatus, certHash))
        goto err;
    certHash = NULL;
    /*
     * For any particular CertStatus, omission of the statusInfo field
     * indicates ACCEPTANCE of the specified certificate.  Alternatively,
     * explicit status details (with respect to acceptance or rejection) MAY
     * be provided in the statusInfo field, perhaps for auditing purposes at
     * the CA/RA.
     */
    sinfo = fail_info != 0 ?
        OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_rejection, fail_info, text) :
        OSSL_CMP_STATUSINFO_new(OSSL_CMP_PKISTATUS_accepted, 0, text);
    if (sinfo == NULL)
        goto err;
    certStatus->statusInfo = sinfo;

    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_CERTCONF);
    OSSL_CMP_MSG_free(msg);
    ASN1_OCTET_STRING_free(certHash);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_pollReq_new(OSSL_CMP_CTX *ctx, int crid)
{
    OSSL_CMP_MSG *msg = NULL;
    OSSL_CMP_POLLREQ *preq = NULL;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_POLLREQ)) == NULL)
        goto err;

    if ((preq = OSSL_CMP_POLLREQ_new()) == NULL
            || !ASN1_INTEGER_set(preq->certReqId, crid)
            || !sk_OSSL_CMP_POLLREQ_push(msg->body->value.pollReq, preq))
        goto err;

    preq = NULL;
    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;

    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_POLLREQ);
    OSSL_CMP_POLLREQ_free(preq);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

OSSL_CMP_MSG *ossl_cmp_pollRep_new(OSSL_CMP_CTX *ctx, int crid,
                                   int64_t poll_after)
{
    OSSL_CMP_MSG *msg;
    OSSL_CMP_POLLREP *prep;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    if ((msg = ossl_cmp_msg_create(ctx, OSSL_CMP_PKIBODY_POLLREP)) == NULL)
        goto err;
    if ((prep = OSSL_CMP_POLLREP_new()) == NULL)
        goto err;
    if (!sk_OSSL_CMP_POLLREP_push(msg->body->value.pollRep, prep))
        goto err;
    if (!ASN1_INTEGER_set(prep->certReqId, crid))
        goto err;
    if (!ASN1_INTEGER_set_int64(prep->checkAfter, poll_after))
        goto err;

    if (!ossl_cmp_msg_protect(ctx, msg))
        goto err;
    return msg;

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CREATING_POLLREP);
    OSSL_CMP_MSG_free(msg);
    return NULL;
}

/*-
 * returns the status field of the RevRepContent with the given
 * request/sequence id inside a revocation response.
 * RevRepContent has the revocation statuses in same order as they were sent in
 * RevReqContent.
 * returns NULL on error
 */
OSSL_CMP_PKISI *
ossl_cmp_revrepcontent_get_pkisi(OSSL_CMP_REVREPCONTENT *rrep, int rsid)
{
    OSSL_CMP_PKISI *status;

    if (!ossl_assert(rrep != NULL))
        return NULL;

    if ((status = sk_OSSL_CMP_PKISI_value(rrep->status, rsid)) != NULL)
        return status;

    ERR_raise(ERR_LIB_CMP, CMP_R_PKISTATUSINFO_NOT_FOUND);
    return NULL;
}

/*
 * returns the CertId field in the revCerts part of the RevRepContent
 * with the given request/sequence id inside a revocation response.
 * RevRepContent has the CertIds in same order as they were sent in
 * RevReqContent.
 * returns NULL on error
 */
OSSL_CRMF_CERTID *
ossl_cmp_revrepcontent_get_CertId(OSSL_CMP_REVREPCONTENT *rrep, int rsid)
{
    OSSL_CRMF_CERTID *cid = NULL;

    if (!ossl_assert(rrep != NULL))
        return NULL;

    if ((cid = sk_OSSL_CRMF_CERTID_value(rrep->revCerts, rsid)) != NULL)
        return cid;

    ERR_raise(ERR_LIB_CMP, CMP_R_CERTID_NOT_FOUND);
    return NULL;
}

static int suitable_rid(const ASN1_INTEGER *certReqId, int rid)
{
    int trid;

    if (rid == OSSL_CMP_CERTREQID_NONE)
        return 1;

    trid = ossl_cmp_asn1_get_int(certReqId);
    if (trid <= OSSL_CMP_CERTREQID_INVALID) {
        ERR_raise(ERR_LIB_CMP, CMP_R_BAD_REQUEST_ID);
        return 0;
    }
    return rid == trid;
}

/*
 * returns a pointer to the PollResponse with the given CertReqId
 * (or the first one in case -1) inside a PollRepContent
 * returns NULL on error or if no suitable PollResponse available
 */
OSSL_CMP_POLLREP *
ossl_cmp_pollrepcontent_get0_pollrep(const OSSL_CMP_POLLREPCONTENT *prc,
                                     int rid)
{
    OSSL_CMP_POLLREP *pollRep = NULL;
    int i;

    if (!ossl_assert(prc != NULL))
        return NULL;

    for (i = 0; i < sk_OSSL_CMP_POLLREP_num(prc); i++) {
        pollRep = sk_OSSL_CMP_POLLREP_value(prc, i);
        if (suitable_rid(pollRep->certReqId, rid))
            return pollRep;
    }

    ERR_raise_data(ERR_LIB_CMP, CMP_R_CERTRESPONSE_NOT_FOUND,
                   "expected certReqId = %d", rid);
    return NULL;
}

/*
 * returns a pointer to the CertResponse with the given CertReqId
 * (or the first one in case -1) inside a CertRepMessage
 * returns NULL on error or if no suitable CertResponse available
 */
OSSL_CMP_CERTRESPONSE *
ossl_cmp_certrepmessage_get0_certresponse(const OSSL_CMP_CERTREPMESSAGE *crm,
                                          int rid)
{
    OSSL_CMP_CERTRESPONSE *crep = NULL;
    int i;

    if (!ossl_assert(crm != NULL && crm->response != NULL))
        return NULL;

    for (i = 0; i < sk_OSSL_CMP_CERTRESPONSE_num(crm->response); i++) {
        crep = sk_OSSL_CMP_CERTRESPONSE_value(crm->response, i);
        if (suitable_rid(crep->certReqId, rid))
            return crep;
    }

    ERR_raise_data(ERR_LIB_CMP, CMP_R_CERTRESPONSE_NOT_FOUND,
                   "expected certReqId = %d", rid);
    return NULL;
}

/*-
 * Retrieve newly enrolled certificate and key from the given certResponse crep.
 * Stores any centrally generated key in ctx->newPkey.
 * In case of indirect POPO uses ctx->newPkey to decrypt the new certificate.
 * Returns a pointer to a copy of the found certificate, or NULL if not found.
 */
X509 *ossl_cmp_certresponse_get1_cert(const OSSL_CMP_CTX *ctx, const OSSL_CMP_CERTRESPONSE *crep)
{
    OSSL_CMP_CERTORENCCERT *coec;
    X509 *crt = NULL;
    OSSL_CRMF_ENCRYPTEDKEY *encr_key;
    EVP_PKEY *pkey = NULL;
    int central_keygen = OSSL_CMP_CTX_get_option(ctx, OSSL_CMP_OPT_POPO_METHOD)
        == OSSL_CRMF_POPO_NONE;

    if (crep->certifiedKeyPair == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_CERTIFICATE_NOT_FOUND);
        return NULL;
    }
    encr_key = crep->certifiedKeyPair->privateKey;
    if (encr_key == NULL && central_keygen) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_CENTRAL_GEN_KEY);
        return NULL;
    }
    if (encr_key != NULL) {
        if (!central_keygen) {
            ERR_raise(ERR_LIB_CMP, CMP_R_UNEXPECTED_CENTRAL_GEN_KEY);
            return NULL;
        }
        /* found encrypted private key, try to extract */
        pkey = OSSL_CRMF_ENCRYPTEDKEY_get1_pkey(encr_key, ctx->trusted,
                                                ctx->untrusted,
                                                ctx->pkey, ctx->cert,
                                                ctx->secretValue,
                                                ctx->libctx, ctx->propq);
        if (pkey == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_FAILED_EXTRACTING_CENTRAL_GEN_KEY);
            return NULL;
        }
        OSSL_CMP_CTX_set0_newPkey((OSSL_CMP_CTX *)ctx, 1, pkey);
    }

    if (!ossl_assert(crep != NULL && ctx != NULL))
        return NULL;

    if ((coec = crep->certifiedKeyPair->certOrEncCert) != NULL) {
        switch (coec->type) {
        case OSSL_CMP_CERTORENCCERT_CERTIFICATE:
            crt = X509_dup(coec->value.certificate);
            break;
        case OSSL_CMP_CERTORENCCERT_ENCRYPTEDCERT:
            /* cert encrypted for indirect PoP; RFC 4210, 5.2.8.2 */
            pkey = OSSL_CMP_CTX_get0_newPkey(ctx, 1);
            /* pkey is ctx->newPkey (if private, else NULL) or ctx->pkey */
            if (pkey == NULL) {
                ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_PRIVATE_KEY);
                return NULL;
            }
            crt = OSSL_CRMF_ENCRYPTEDKEY_get1_encCert(coec->value.encryptedCert,
                                                      ctx->libctx, ctx->propq, pkey, 0);
            break;
        default:
            ERR_raise(ERR_LIB_CMP, CMP_R_UNKNOWN_CERT_TYPE);
            return NULL;
        }
    }
    if (crt == NULL)
        ERR_raise(ERR_LIB_CMP, CMP_R_CERTIFICATE_NOT_FOUND);
    else
        (void)ossl_x509_set0_libctx(crt, ctx->libctx, ctx->propq);
    return crt;
}

int OSSL_CMP_MSG_update_transactionID(OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg)
{
    if (ctx == NULL || msg == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (!ossl_cmp_hdr_set_transactionID(ctx, msg->header))
        return 0;
    return msg->header->protectionAlg == NULL
            || ossl_cmp_msg_protect(ctx, msg);
}

int OSSL_CMP_MSG_update_recipNonce(OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg)
{
    if (ctx == NULL || msg == NULL || msg->header == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (ctx->recipNonce == NULL) /* nothing to do for 1st msg in transaction */
        return 1;
    if (!ossl_cmp_asn1_octet_string_set1(&msg->header->recipNonce,
                                         ctx->recipNonce))
        return 0;
    return msg->header->protectionAlg == NULL || ossl_cmp_msg_protect(ctx, msg);
}

OSSL_CMP_MSG *OSSL_CMP_MSG_read(const char *file, OSSL_LIB_CTX *libctx,
                                const char *propq)
{
    OSSL_CMP_MSG *msg;
    BIO *bio = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }

    msg = OSSL_CMP_MSG_new(libctx, propq);
    if (msg == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_CMP_LIB);
        return NULL;
    }

    if ((bio = BIO_new_file(file, "rb")) == NULL
            || d2i_OSSL_CMP_MSG_bio(bio, &msg) == NULL) {
        OSSL_CMP_MSG_free(msg);
        msg = NULL;
    }
    BIO_free(bio);
    return msg;
}

int OSSL_CMP_MSG_write(const char *file, const OSSL_CMP_MSG *msg)
{
    BIO *bio;
    int res;

    if (file == NULL || msg == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return -1;
    }

    bio = BIO_new_file(file, "wb");
    if (bio == NULL)
        return -2;
    res = i2d_OSSL_CMP_MSG_bio(bio, msg);
    BIO_free(bio);
    return res;
}

OSSL_CMP_MSG *d2i_OSSL_CMP_MSG(OSSL_CMP_MSG **msg, const unsigned char **in,
                               long len)
{
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (msg != NULL && *msg != NULL) {
        libctx  = (*msg)->libctx;
        propq = (*msg)->propq;
    }

    return (OSSL_CMP_MSG *)ASN1_item_d2i_ex((ASN1_VALUE **)msg, in, len,
                                            ASN1_ITEM_rptr(OSSL_CMP_MSG),
                                            libctx, propq);
}

int i2d_OSSL_CMP_MSG(const OSSL_CMP_MSG *msg, unsigned char **out)
{
    return ASN1_item_i2d((const ASN1_VALUE *)msg, out,
                         ASN1_ITEM_rptr(OSSL_CMP_MSG));
}

OSSL_CMP_MSG *d2i_OSSL_CMP_MSG_bio(BIO *bio, OSSL_CMP_MSG **msg)
{
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (msg != NULL && *msg != NULL) {
        libctx  = (*msg)->libctx;
        propq = (*msg)->propq;
    }

    return ASN1_item_d2i_bio_ex(ASN1_ITEM_rptr(OSSL_CMP_MSG), bio, msg, libctx,
                                propq);
}

int i2d_OSSL_CMP_MSG_bio(BIO *bio, const OSSL_CMP_MSG *msg)
{
    return ASN1_i2d_bio_of(OSSL_CMP_MSG, i2d_OSSL_CMP_MSG, bio, msg);
}

int ossl_cmp_is_error_with_waiting(const OSSL_CMP_MSG *msg)
{
    if (!ossl_assert(msg != NULL))
        return 0;

    return (OSSL_CMP_MSG_get_bodytype(msg) == OSSL_CMP_PKIBODY_ERROR
            && ossl_cmp_pkisi_get_status(msg->body->value.error->pKIStatusInfo)
            == OSSL_CMP_PKISTATUS_waiting);
}
