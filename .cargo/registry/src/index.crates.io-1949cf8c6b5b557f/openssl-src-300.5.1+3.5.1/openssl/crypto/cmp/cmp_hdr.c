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

/* CMP functions for PKIHeader handling */

#include "cmp_local.h"

#include <openssl/rand.h>

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/asn1t.h>
#include <openssl/cmp.h>
#include <openssl/err.h>

int ossl_cmp_hdr_set_pvno(OSSL_CMP_PKIHEADER *hdr, int pvno)
{
    if (!ossl_assert(hdr != NULL))
        return 0;
    return ASN1_INTEGER_set(hdr->pvno, pvno);
}

int ossl_cmp_hdr_get_pvno(const OSSL_CMP_PKIHEADER *hdr)
{
    int64_t pvno;

    if (!ossl_assert(hdr != NULL))
        return -1;
    if (!ASN1_INTEGER_get_int64(&pvno, hdr->pvno) || pvno < 0 || pvno > INT_MAX)
        return -1;
    return (int)pvno;
}

int ossl_cmp_hdr_get_protection_nid(const OSSL_CMP_PKIHEADER *hdr)
{
    if (!ossl_assert(hdr != NULL)
            || hdr->protectionAlg == NULL)
        return NID_undef;
    return OBJ_obj2nid(hdr->protectionAlg->algorithm);
}

ASN1_OCTET_STRING *OSSL_CMP_HDR_get0_transactionID(const
                                                   OSSL_CMP_PKIHEADER *hdr)
{
    if (hdr == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return hdr->transactionID;
}

ASN1_OCTET_STRING *ossl_cmp_hdr_get0_senderNonce(const OSSL_CMP_PKIHEADER *hdr)
{
    if (!ossl_assert(hdr != NULL))
        return NULL;
    return hdr->senderNonce;
}

ASN1_OCTET_STRING *OSSL_CMP_HDR_get0_recipNonce(const OSSL_CMP_PKIHEADER *hdr)
{
    if (hdr == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return hdr->recipNonce;
}

STACK_OF(OSSL_CMP_ITAV)
    *OSSL_CMP_HDR_get0_geninfo_ITAVs(const OSSL_CMP_PKIHEADER *hdr)
{
    if (hdr == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }
    return hdr->generalInfo;
}

/* a NULL-DN as an empty sequence of RDNs */
int ossl_cmp_general_name_is_NULL_DN(GENERAL_NAME *name)
{
    return name == NULL
        || (name->type == GEN_DIRNAME && IS_NULL_DN(name->d.directoryName));
}

/*
 * Set the sender name in PKIHeader.
 * when nm is NULL, sender is set to an empty string
 * returns 1 on success, 0 on error
 */
int ossl_cmp_hdr_set1_sender(OSSL_CMP_PKIHEADER *hdr, const X509_NAME *nm)
{
    if (!ossl_assert(hdr != NULL))
        return 0;
    return GENERAL_NAME_set1_X509_NAME(&hdr->sender, nm);
}

int ossl_cmp_hdr_set1_recipient(OSSL_CMP_PKIHEADER *hdr, const X509_NAME *nm)
{
    if (!ossl_assert(hdr != NULL))
        return 0;
    return GENERAL_NAME_set1_X509_NAME(&hdr->recipient, nm);
}

int ossl_cmp_hdr_update_messageTime(OSSL_CMP_PKIHEADER *hdr)
{
    if (!ossl_assert(hdr != NULL))
        return 0;
    if (hdr->messageTime == NULL
            && (hdr->messageTime = ASN1_GENERALIZEDTIME_new()) == NULL)
        return 0;
    return ASN1_GENERALIZEDTIME_set(hdr->messageTime, time(NULL)) != NULL;
}

/* assign to *tgt a random byte array of given length */
static int set_random(ASN1_OCTET_STRING **tgt, OSSL_CMP_CTX *ctx, size_t len)
{
    unsigned char *bytes = OPENSSL_malloc(len);
    int res = 0;

    if (bytes == NULL || RAND_bytes_ex(ctx->libctx, bytes, len, 0) <= 0)
        ERR_raise(ERR_LIB_CMP, CMP_R_FAILURE_OBTAINING_RANDOM);
    else
        res = ossl_cmp_asn1_octet_string_set1_bytes(tgt, bytes, len);
    OPENSSL_free(bytes);
    return res;
}

int ossl_cmp_hdr_set1_senderKID(OSSL_CMP_PKIHEADER *hdr,
                                const ASN1_OCTET_STRING *senderKID)
{
    if (!ossl_assert(hdr != NULL))
        return 0;
    return ossl_cmp_asn1_octet_string_set1(&hdr->senderKID, senderKID);
}

/* push the given text string to the given PKIFREETEXT ft */
int ossl_cmp_hdr_push0_freeText(OSSL_CMP_PKIHEADER *hdr, ASN1_UTF8STRING *text)
{
    if (!ossl_assert(hdr != NULL && text != NULL))
        return 0;

    if (hdr->freeText == NULL
            && (hdr->freeText = sk_ASN1_UTF8STRING_new_null()) == NULL)
        return 0;

    return sk_ASN1_UTF8STRING_push(hdr->freeText, text);
}

int ossl_cmp_hdr_push1_freeText(OSSL_CMP_PKIHEADER *hdr, ASN1_UTF8STRING *text)
{
    if (!ossl_assert(hdr != NULL && text != NULL))
        return 0;

    if (hdr->freeText == NULL
            && (hdr->freeText = sk_ASN1_UTF8STRING_new_null()) == NULL)
        return 0;

    return
        ossl_cmp_sk_ASN1_UTF8STRING_push_str(hdr->freeText, (char *)text->data,
                                             text->length);
}

int ossl_cmp_hdr_generalInfo_push0_item(OSSL_CMP_PKIHEADER *hdr,
                                        OSSL_CMP_ITAV *itav)
{
    if (!ossl_assert(hdr != NULL && itav != NULL))
        return 0;
    return OSSL_CMP_ITAV_push0_stack_item(&hdr->generalInfo, itav);
}

int ossl_cmp_hdr_generalInfo_push1_items(OSSL_CMP_PKIHEADER *hdr,
                                         const STACK_OF(OSSL_CMP_ITAV) *itavs)
{
    int i;
    OSSL_CMP_ITAV *itav;

    if (!ossl_assert(hdr != NULL))
        return 0;

    for (i = 0; i < sk_OSSL_CMP_ITAV_num(itavs); i++) {
        itav = OSSL_CMP_ITAV_dup(sk_OSSL_CMP_ITAV_value(itavs, i));
        if (itav == NULL)
            return 0;

        if (!ossl_cmp_hdr_generalInfo_push0_item(hdr, itav)) {
            OSSL_CMP_ITAV_free(itav);
            return 0;
        }
    }
    return 1;
}

int ossl_cmp_hdr_set_implicitConfirm(OSSL_CMP_PKIHEADER *hdr)
{
    OSSL_CMP_ITAV *itav;
    ASN1_TYPE *asn1null;

    if (!ossl_assert(hdr != NULL))
        return 0;
    asn1null = (ASN1_TYPE *)ASN1_NULL_new();
    if (asn1null == NULL)
        return 0;
    if ((itav = OSSL_CMP_ITAV_create(OBJ_nid2obj(NID_id_it_implicitConfirm),
                                     asn1null)) == NULL)
        goto err;
    if (!ossl_cmp_hdr_generalInfo_push0_item(hdr, itav))
        goto err;
    return 1;

 err:
    ASN1_TYPE_free(asn1null);
    OSSL_CMP_ITAV_free(itav);
    return 0;
}

/* return 1 if implicitConfirm in the generalInfo field of the header is set */
int ossl_cmp_hdr_has_implicitConfirm(const OSSL_CMP_PKIHEADER *hdr)
{
    int itavCount;
    int i;
    OSSL_CMP_ITAV *itav;

    if (!ossl_assert(hdr != NULL))
        return 0;

    itavCount = sk_OSSL_CMP_ITAV_num(hdr->generalInfo);
    for (i = 0; i < itavCount; i++) {
        itav = sk_OSSL_CMP_ITAV_value(hdr->generalInfo, i);
        if (itav != NULL
                && OBJ_obj2nid(itav->infoType) == NID_id_it_implicitConfirm)
            return 1;
    }

    return 0;
}

/*
 * set ctx->transactionID in CMP header
 * if ctx->transactionID is NULL, a random one is created with 128 bit
 * according to section 5.1.1:
 *
 * It is RECOMMENDED that the clients fill the transactionID field with
 * 128 bits of (pseudo-) random data for the start of a transaction to
 * reduce the probability of having the transactionID in use at the server.
 */
int ossl_cmp_hdr_set_transactionID(OSSL_CMP_CTX *ctx, OSSL_CMP_PKIHEADER *hdr)
{
    if (ctx->transactionID == NULL) {
        char *tid;

        if (!set_random(&ctx->transactionID, ctx,
                        OSSL_CMP_TRANSACTIONID_LENGTH))
            return 0;
        tid = i2s_ASN1_OCTET_STRING(NULL, ctx->transactionID);
        if (tid != NULL)
            ossl_cmp_log1(DEBUG, ctx,
                          "Starting new transaction with ID=%s", tid);
        OPENSSL_free(tid);
    }

    return ossl_cmp_asn1_octet_string_set1(&hdr->transactionID,
                                           ctx->transactionID);
}

/* fill in all fields of the hdr according to the info given in ctx */
int ossl_cmp_hdr_init(OSSL_CMP_CTX *ctx, OSSL_CMP_PKIHEADER *hdr)
{
    const X509_NAME *sender;
    const X509_NAME *rcp = NULL;

    if (!ossl_assert(ctx != NULL && hdr != NULL))
        return 0;

    /* set the CMP version */
    if (!ossl_cmp_hdr_set_pvno(hdr, OSSL_CMP_PVNO))
        return 0;

    /*
     * If no protection cert nor oldCert nor CSR nor subject is given,
     * sender name is not known to the client and thus set to NULL-DN
     */
    sender = ctx->cert != NULL ? X509_get_subject_name(ctx->cert) :
        ctx->oldCert != NULL ? X509_get_subject_name(ctx->oldCert) :
        ctx->p10CSR != NULL ? X509_REQ_get_subject_name(ctx->p10CSR) :
        ctx->subjectName;
    if (!ossl_cmp_hdr_set1_sender(hdr, sender))
        return 0;

    /* determine recipient entry in PKIHeader */
    if (ctx->recipient != NULL)
        rcp = ctx->recipient;
    else if (ctx->srvCert != NULL)
        rcp = X509_get_subject_name(ctx->srvCert);
    else if (ctx->issuer != NULL)
        rcp = ctx->issuer;
    else if (ctx->oldCert != NULL)
        rcp = X509_get_issuer_name(ctx->oldCert);
    else if (ctx->cert != NULL)
        rcp = X509_get_issuer_name(ctx->cert);
    if (!ossl_cmp_hdr_set1_recipient(hdr, rcp))
        return 0;

    /* set current time as message time */
    if (!ossl_cmp_hdr_update_messageTime(hdr))
        return 0;

    if (ctx->recipNonce != NULL
            && !ossl_cmp_asn1_octet_string_set1(&hdr->recipNonce,
                                                ctx->recipNonce))
        return 0;

    if (!ossl_cmp_hdr_set_transactionID(ctx, hdr))
        return 0;

    /*-
     * set random senderNonce
     * according to section 5.1.1:
     *
     * senderNonce                  present
     *         -- 128 (pseudo-)random bits
     * The senderNonce and recipNonce fields protect the PKIMessage against
     * replay attacks. The senderNonce will typically be 128 bits of
     * (pseudo-) random data generated by the sender, whereas the recipNonce
     * is copied from the senderNonce of the previous message in the
     * transaction.
     */
    if (!set_random(&hdr->senderNonce, ctx, OSSL_CMP_SENDERNONCE_LENGTH))
        return 0;

    /* store senderNonce - for cmp with recipNonce in next outgoing msg */
    if (!OSSL_CMP_CTX_set1_senderNonce(ctx, hdr->senderNonce))
        return 0;

    /*-
     * freeText                [7] PKIFreeText OPTIONAL,
     * -- this may be used to indicate context-specific instructions
     * -- (this field is intended for human consumption)
     */
    if (ctx->freeText != NULL
            && !ossl_cmp_hdr_push1_freeText(hdr, ctx->freeText))
        return 0;

    return 1;
}
