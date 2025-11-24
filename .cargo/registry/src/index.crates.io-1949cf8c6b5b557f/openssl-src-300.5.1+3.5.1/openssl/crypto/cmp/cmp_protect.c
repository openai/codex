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
#include "crypto/asn1.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/asn1t.h>
#include <openssl/cmp.h>
#include <openssl/crmf.h>
#include <openssl/err.h>
#include <openssl/x509.h>

/*
 * This function is also used by the internal verify_PBMAC() in cmp_vfy.c.
 *
 * Calculate protection for |msg| according to |msg->header->protectionAlg|
 * using the credentials, library context, and property criteria in the ctx.
 * Unless |msg->header->protectionAlg| is PasswordBasedMAC,
 * its value is completed according to |ctx->pkey| and |ctx->digest|,
 * where the latter irrelevant in the case of Edwards curves.
 *
 * returns ASN1_BIT_STRING representing the protection on success, else NULL
 */
ASN1_BIT_STRING *ossl_cmp_calc_protection(const OSSL_CMP_CTX *ctx,
                                          const OSSL_CMP_MSG *msg)
{
    ASN1_BIT_STRING *prot = NULL;
    OSSL_CMP_PROTECTEDPART prot_part;
    const ASN1_OBJECT *algorOID = NULL;
    const void *ppval = NULL;
    int pptype = 0;

    if (!ossl_assert(ctx != NULL && msg != NULL))
        return NULL;

    /* construct data to be signed */
    prot_part.header = msg->header;
    prot_part.body = msg->body;

    if (msg->header->protectionAlg == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNKNOWN_ALGORITHM_ID);
        return NULL;
    }
    X509_ALGOR_get0(&algorOID, &pptype, &ppval, msg->header->protectionAlg);

    if (OBJ_obj2nid(algorOID) == NID_id_PasswordBasedMAC) {
        int len;
        size_t prot_part_der_len;
        unsigned char *prot_part_der = NULL;
        size_t sig_len;
        unsigned char *protection = NULL;
        OSSL_CRMF_PBMPARAMETER *pbm = NULL;
        ASN1_STRING *pbm_str = NULL;
        const unsigned char *pbm_str_uc = NULL;

        if (ctx->secretValue == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_PBM_SECRET);
            return NULL;
        }
        if (ppval == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CALCULATING_PROTECTION);
            return NULL;
        }

        len = i2d_OSSL_CMP_PROTECTEDPART(&prot_part, &prot_part_der);
        if (len < 0 || prot_part_der == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_CALCULATING_PROTECTION);
            goto end;
        }
        prot_part_der_len = (size_t)len;

        pbm_str = (ASN1_STRING *)ppval;
        pbm_str_uc = pbm_str->data;
        pbm = d2i_OSSL_CRMF_PBMPARAMETER(NULL, &pbm_str_uc, pbm_str->length);
        if (pbm == NULL) {
            ERR_raise(ERR_LIB_CMP, CMP_R_WRONG_ALGORITHM_OID);
            goto end;
        }

        if (!OSSL_CRMF_pbm_new(ctx->libctx, ctx->propq,
                               pbm, prot_part_der, prot_part_der_len,
                               ctx->secretValue->data, ctx->secretValue->length,
                               &protection, &sig_len))
            goto end;

        if ((prot = ASN1_BIT_STRING_new()) == NULL)
            goto end;
        /* OpenSSL by default encodes all bit strings as ASN.1 NamedBitList */
        ossl_asn1_string_set_bits_left(prot, 0);
        if (!ASN1_BIT_STRING_set(prot, protection, sig_len)) {
            ASN1_BIT_STRING_free(prot);
            prot = NULL;
        }
    end:
        OSSL_CRMF_PBMPARAMETER_free(pbm);
        OPENSSL_free(protection);
        OPENSSL_free(prot_part_der);
        return prot;
    } else {
        const EVP_MD *md = ctx->digest;
        char name[80] = "";

        if (ctx->pkey == NULL) {
            ERR_raise(ERR_LIB_CMP,
                      CMP_R_MISSING_KEY_INPUT_FOR_CREATING_PROTECTION);
            return NULL;
        }
        if (EVP_PKEY_get_default_digest_name(ctx->pkey, name, sizeof(name)) > 0
            && strcmp(name, "UNDEF") == 0) /* at least for Ed25519, Ed448 */
            md = NULL;

        if ((prot = ASN1_BIT_STRING_new()) == NULL)
            return NULL;
        if (ASN1_item_sign_ex(ASN1_ITEM_rptr(OSSL_CMP_PROTECTEDPART),
                              msg->header->protectionAlg, /* sets X509_ALGOR */
                              NULL, prot, &prot_part, NULL, ctx->pkey, md,
                              ctx->libctx, ctx->propq))
            return prot;
        ASN1_BIT_STRING_free(prot);
        return NULL;
    }
}

void ossl_cmp_set_own_chain(OSSL_CMP_CTX *ctx)
{
    if (!ossl_assert(ctx != NULL))
        return;
    /* if not yet done try to build chain using available untrusted certs */
    if (ctx->chain == NULL) {
        ossl_cmp_debug(ctx, "trying to build chain for own CMP signer cert");
        ctx->chain = X509_build_chain(ctx->cert, ctx->untrusted, NULL, 0,
                                      ctx->libctx, ctx->propq);
        if (ctx->chain != NULL) {
            ossl_cmp_debug(ctx, "success building chain for own CMP signer cert");
        } else {
            /* dump errors to avoid confusion when printing further ones */
            OSSL_CMP_CTX_print_errors(ctx);
            ossl_cmp_warn(ctx, "could not build chain for own CMP signer cert");
        }
    }
}

/* ctx is not const just because ctx->chain may get adapted */
int ossl_cmp_msg_add_extraCerts(OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg)
{
    if (!ossl_assert(ctx != NULL && msg != NULL))
        return 0;

    /* Add first ctx->cert and its chain if using signature-based protection */
    if (!ctx->unprotectedSend && ctx->secretValue == NULL
            && ctx->cert != NULL && ctx->pkey != NULL) {
        int prepend = X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP
            | X509_ADD_FLAG_PREPEND | X509_ADD_FLAG_NO_SS;

        ossl_cmp_set_own_chain(ctx);
        if (ctx->chain != NULL) {
            if (!ossl_x509_add_certs_new(&msg->extraCerts, ctx->chain, prepend))
                return 0;
        } else {
            /* make sure that at least our own signer cert is included first */
            if (!ossl_x509_add_cert_new(&msg->extraCerts, ctx->cert, prepend))
                return 0;
            ossl_cmp_debug(ctx, "fallback: adding just own CMP signer cert");
        }
    }

    /* add any additional certificates from ctx->extraCertsOut */
    if (!ossl_x509_add_certs_new(&msg->extraCerts, ctx->extraCertsOut,
                                 X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP))
        return 0;

    /* in case extraCerts are empty list avoid empty ASN.1 sequence */
    if (sk_X509_num(msg->extraCerts) == 0) {
        sk_X509_free(msg->extraCerts);
        msg->extraCerts = NULL;
    }
    return 1;
}

/*
 * Create an X509_ALGOR structure for PasswordBasedMAC protection based on
 * the pbm settings in the context
 */
static X509_ALGOR *pbmac_algor(const OSSL_CMP_CTX *ctx)
{
    OSSL_CRMF_PBMPARAMETER *pbm = NULL;
    unsigned char *pbm_der = NULL;
    int pbm_der_len;
    ASN1_STRING *pbm_str = NULL;
    X509_ALGOR *alg = NULL;

    if (!ossl_assert(ctx != NULL))
        return NULL;

    pbm = OSSL_CRMF_pbmp_new(ctx->libctx, ctx->pbm_slen,
                             EVP_MD_get_type(ctx->pbm_owf), ctx->pbm_itercnt,
                             ctx->pbm_mac);
    pbm_str = ASN1_STRING_new();
    if (pbm == NULL || pbm_str == NULL)
        goto err;
    if ((pbm_der_len = i2d_OSSL_CRMF_PBMPARAMETER(pbm, &pbm_der)) < 0)
        goto err;
    if (!ASN1_STRING_set(pbm_str, pbm_der, pbm_der_len))
        goto err;
    alg = ossl_X509_ALGOR_from_nid(NID_id_PasswordBasedMAC,
                                   V_ASN1_SEQUENCE, pbm_str);
 err:
    if (alg == NULL)
        ASN1_STRING_free(pbm_str);
    OPENSSL_free(pbm_der);
    OSSL_CRMF_PBMPARAMETER_free(pbm);
    return alg;
}

static int set_senderKID(const OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg,
                         const ASN1_OCTET_STRING *id)
{
    if (id == NULL)
        id = ctx->referenceValue; /* standard for PBM, fallback for sig-based */
    return id == NULL || ossl_cmp_hdr_set1_senderKID(msg->header, id);
}

/* ctx is not const just because ctx->chain may get adapted */
int ossl_cmp_msg_protect(OSSL_CMP_CTX *ctx, OSSL_CMP_MSG *msg)
{
    if (!ossl_assert(ctx != NULL && msg != NULL))
        return 0;

    /*
     * For the case of re-protection remove pre-existing protection.
     * Does not remove any pre-existing extraCerts.
     */
    X509_ALGOR_free(msg->header->protectionAlg);
    msg->header->protectionAlg = NULL;
    ASN1_BIT_STRING_free(msg->protection);
    msg->protection = NULL;

    if (ctx->unprotectedSend) {
        if (!set_senderKID(ctx, msg, NULL))
            goto err;
    } else if (ctx->secretValue != NULL) {
        /* use PasswordBasedMac according to 5.1.3.1 if secretValue is given */
        if ((msg->header->protectionAlg = pbmac_algor(ctx)) == NULL)
            goto err;
        if (!set_senderKID(ctx, msg, NULL))
            goto err;

        /*
         * will add any additional certificates from ctx->extraCertsOut
         * while not needed to validate the protection certificate,
         * the option to do this might be handy for certain use cases
         */
    } else if (ctx->cert != NULL && ctx->pkey != NULL) {
        /* use MSG_SIG_ALG according to 5.1.3.3 if client cert and key given */

        /* make sure that key and certificate match */
        if (!X509_check_private_key(ctx->cert, ctx->pkey)) {
            ERR_raise(ERR_LIB_CMP, CMP_R_CERT_AND_KEY_DO_NOT_MATCH);
            goto err;
        }

        if ((msg->header->protectionAlg = X509_ALGOR_new()) == NULL)
            goto err;
        /* set senderKID to keyIdentifier of the cert according to 5.1.1 */
        if (!set_senderKID(ctx, msg, X509_get0_subject_key_id(ctx->cert)))
            goto err;

        /*
         * will add ctx->cert followed, if possible, by its chain built
         * from ctx->untrusted, and then ctx->extraCertsOut
         */
    } else {
        ERR_raise(ERR_LIB_CMP,
                  CMP_R_MISSING_KEY_INPUT_FOR_CREATING_PROTECTION);
        goto err;
    }
    if (!ctx->unprotectedSend
        /* protect according to msg->header->protectionAlg partly set above */
            && ((msg->protection = ossl_cmp_calc_protection(ctx, msg)) == NULL))
        goto err;

    /*
     * For signature-based protection add ctx->cert followed by its chain.
     * Finally add any additional certificates from ctx->extraCertsOut;
     * even if not needed to validate the protection
     * the option to do this might be handy for certain use cases.
     */
    if (!ossl_cmp_msg_add_extraCerts(ctx, msg))
        goto err;

    /*
     * As required by RFC 4210 section 5.1.1., if the sender name is not known
     * to the client it set to NULL-DN. In this case for identification at least
     * the senderKID must be set, where we took the referenceValue as fallback.
     */
    if (!(ossl_cmp_general_name_is_NULL_DN(msg->header->sender)
          && msg->header->senderKID == NULL))
        return 1;
    ERR_raise(ERR_LIB_CMP, CMP_R_MISSING_SENDER_IDENTIFICATION);

 err:
    ERR_raise(ERR_LIB_CMP, CMP_R_ERROR_PROTECTING_MESSAGE);
    return 0;
}
