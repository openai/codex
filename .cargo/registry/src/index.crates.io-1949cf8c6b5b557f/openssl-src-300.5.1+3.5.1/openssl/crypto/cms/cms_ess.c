/*
 * Copyright 2008-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/rand.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include <openssl/ess.h>
#include "crypto/ess.h"
#include "crypto/x509.h"
#include "cms_local.h"

IMPLEMENT_ASN1_FUNCTIONS(CMS_ReceiptRequest)

/* ESS services */

int CMS_get1_ReceiptRequest(CMS_SignerInfo *si, CMS_ReceiptRequest **prr)
{
    ASN1_STRING *str;
    CMS_ReceiptRequest *rr;
    ASN1_OBJECT *obj = OBJ_nid2obj(NID_id_smime_aa_receiptRequest);

    if (prr != NULL)
        *prr = NULL;
    str = CMS_signed_get0_data_by_OBJ(si, obj, -3, V_ASN1_SEQUENCE);
    if (str == NULL)
        return 0;

    rr = ASN1_item_unpack(str, ASN1_ITEM_rptr(CMS_ReceiptRequest));
    if (rr == NULL)
        return -1;
    if (prr != NULL)
        *prr = rr;
    else
        CMS_ReceiptRequest_free(rr);
    return 1;
}

/*
 * Returns 0 if attribute is not found, 1 if found,
 * or -1 on attribute parsing failure.
 */
static int ossl_cms_signerinfo_get_signing_cert(const CMS_SignerInfo *si,
                                                ESS_SIGNING_CERT **psc)
{
    ASN1_STRING *str;
    ESS_SIGNING_CERT *sc;
    ASN1_OBJECT *obj = OBJ_nid2obj(NID_id_smime_aa_signingCertificate);

    if (psc != NULL)
        *psc = NULL;
    str = CMS_signed_get0_data_by_OBJ(si, obj, -3, V_ASN1_SEQUENCE);
    if (str == NULL)
        return 0;

    sc = ASN1_item_unpack(str, ASN1_ITEM_rptr(ESS_SIGNING_CERT));
    if (sc == NULL)
        return -1;
    if (psc != NULL)
        *psc = sc;
    else
        ESS_SIGNING_CERT_free(sc);
    return 1;
}

/*
 * Returns 0 if attribute is not found, 1 if found,
 * or -1 on attribute parsing failure.
 */
static int ossl_cms_signerinfo_get_signing_cert_v2(const CMS_SignerInfo *si,
                                                   ESS_SIGNING_CERT_V2 **psc)
{
    ASN1_STRING *str;
    ESS_SIGNING_CERT_V2 *sc;
    ASN1_OBJECT *obj = OBJ_nid2obj(NID_id_smime_aa_signingCertificateV2);

    if (psc != NULL)
        *psc = NULL;
    str = CMS_signed_get0_data_by_OBJ(si, obj, -3, V_ASN1_SEQUENCE);
    if (str == NULL)
        return 0;

    sc = ASN1_item_unpack(str, ASN1_ITEM_rptr(ESS_SIGNING_CERT_V2));
    if (sc == NULL)
        return -1;
    if (psc != NULL)
        *psc = sc;
    else
        ESS_SIGNING_CERT_V2_free(sc);
    return 1;
}

int ossl_cms_check_signing_certs(const CMS_SignerInfo *si,
                                 const STACK_OF(X509) *chain)
{
    ESS_SIGNING_CERT *ss = NULL;
    ESS_SIGNING_CERT_V2 *ssv2 = NULL;
    int ret = ossl_cms_signerinfo_get_signing_cert(si, &ss) >= 0
        && ossl_cms_signerinfo_get_signing_cert_v2(si, &ssv2) >= 0
        && OSSL_ESS_check_signing_certs(ss, ssv2, chain, 1) > 0;

    ESS_SIGNING_CERT_free(ss);
    ESS_SIGNING_CERT_V2_free(ssv2);
    return ret;
}

CMS_ReceiptRequest *CMS_ReceiptRequest_create0_ex(
    unsigned char *id, int idlen, int allorfirst,
    STACK_OF(GENERAL_NAMES) *receiptList, STACK_OF(GENERAL_NAMES) *receiptsTo,
    OSSL_LIB_CTX *libctx)
{
    CMS_ReceiptRequest *rr;

    rr = CMS_ReceiptRequest_new();
    if (rr == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }
    if (id)
        ASN1_STRING_set0(rr->signedContentIdentifier, id, idlen);
    else {
        if (!ASN1_STRING_set(rr->signedContentIdentifier, NULL, 32)) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            goto err;
        }
        if (RAND_bytes_ex(libctx, rr->signedContentIdentifier->data, 32,
                          0) <= 0)
            goto err;
    }

    sk_GENERAL_NAMES_pop_free(rr->receiptsTo, GENERAL_NAMES_free);
    rr->receiptsTo = receiptsTo;

    if (receiptList != NULL) {
        rr->receiptsFrom->type = 1;
        rr->receiptsFrom->d.receiptList = receiptList;
    } else {
        rr->receiptsFrom->type = 0;
        rr->receiptsFrom->d.allOrFirstTier = allorfirst;
    }

    return rr;

 err:
    CMS_ReceiptRequest_free(rr);
    return NULL;

}

CMS_ReceiptRequest *CMS_ReceiptRequest_create0(
    unsigned char *id, int idlen, int allorfirst,
    STACK_OF(GENERAL_NAMES) *receiptList, STACK_OF(GENERAL_NAMES) *receiptsTo)
{
    return CMS_ReceiptRequest_create0_ex(id, idlen, allorfirst, receiptList,
                                         receiptsTo, NULL);
}

int CMS_add1_ReceiptRequest(CMS_SignerInfo *si, CMS_ReceiptRequest *rr)
{
    unsigned char *rrder = NULL;
    int rrderlen, r = 0;

    rrderlen = i2d_CMS_ReceiptRequest(rr, &rrder);
    if (rrderlen < 0) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }

    if (!CMS_signed_add1_attr_by_NID(si, NID_id_smime_aa_receiptRequest,
                                     V_ASN1_SEQUENCE, rrder, rrderlen)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }

    r = 1;

 err:
    OPENSSL_free(rrder);

    return r;

}

void CMS_ReceiptRequest_get0_values(CMS_ReceiptRequest *rr,
                                    ASN1_STRING **pcid,
                                    int *pallorfirst,
                                    STACK_OF(GENERAL_NAMES) **plist,
                                    STACK_OF(GENERAL_NAMES) **prto)
{
    if (pcid != NULL)
        *pcid = rr->signedContentIdentifier;
    if (rr->receiptsFrom->type == 0) {
        if (pallorfirst != NULL)
            *pallorfirst = (int)rr->receiptsFrom->d.allOrFirstTier;
        if (plist != NULL)
            *plist = NULL;
    } else {
        if (pallorfirst != NULL)
            *pallorfirst = -1;
        if (plist != NULL)
            *plist = rr->receiptsFrom->d.receiptList;
    }
    if (prto != NULL)
        *prto = rr->receiptsTo;
}

/* Digest a SignerInfo structure for msgSigDigest attribute processing */

static int cms_msgSigDigest(CMS_SignerInfo *si,
                            unsigned char *dig, unsigned int *diglen)
{
    const EVP_MD *md = EVP_get_digestbyobj(si->digestAlgorithm->algorithm);

    if (md == NULL)
        return 0;
    if (!ossl_asn1_item_digest_ex(ASN1_ITEM_rptr(CMS_Attributes_Verify), md,
                                  si->signedAttrs, dig, diglen,
                                  ossl_cms_ctx_get0_libctx(si->cms_ctx),
                                  ossl_cms_ctx_get0_propq(si->cms_ctx)))
        return 0;
    return 1;
}

/* Add a msgSigDigest attribute to a SignerInfo */

int ossl_cms_msgSigDigest_add1(CMS_SignerInfo *dest, CMS_SignerInfo *src)
{
    unsigned char dig[EVP_MAX_MD_SIZE];
    unsigned int diglen;

    if (!cms_msgSigDigest(src, dig, &diglen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_MSGSIGDIGEST_ERROR);
        return 0;
    }
    if (!CMS_signed_add1_attr_by_NID(dest, NID_id_smime_aa_msgSigDigest,
                                     V_ASN1_OCTET_STRING, dig, diglen)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        return 0;
    }
    return 1;
}

/* Verify signed receipt after it has already passed normal CMS verify */

int ossl_cms_Receipt_verify(CMS_ContentInfo *cms, CMS_ContentInfo *req_cms)
{
    int r = 0, i;
    CMS_ReceiptRequest *rr = NULL;
    CMS_Receipt *rct = NULL;
    STACK_OF(CMS_SignerInfo) *sis, *osis;
    CMS_SignerInfo *si, *osi = NULL;
    ASN1_OCTET_STRING *msig, **pcont;
    ASN1_OBJECT *octype;
    unsigned char dig[EVP_MAX_MD_SIZE];
    unsigned int diglen;

    /* Get SignerInfos, also checks SignedData content type */
    osis = CMS_get0_SignerInfos(req_cms);
    sis = CMS_get0_SignerInfos(cms);
    if (!osis || !sis)
        goto err;

    if (sk_CMS_SignerInfo_num(sis) != 1) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NEED_ONE_SIGNER);
        goto err;
    }

    /* Check receipt content type */
    if (OBJ_obj2nid(CMS_get0_eContentType(cms)) != NID_id_smime_ct_receipt) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_A_SIGNED_RECEIPT);
        goto err;
    }

    /* Extract and decode receipt content */
    pcont = CMS_get0_content(cms);
    if (pcont == NULL || *pcont == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CONTENT);
        goto err;
    }

    rct = ASN1_item_unpack(*pcont, ASN1_ITEM_rptr(CMS_Receipt));

    if (!rct) {
        ERR_raise(ERR_LIB_CMS, CMS_R_RECEIPT_DECODE_ERROR);
        goto err;
    }

    /* Locate original request */

    for (i = 0; i < sk_CMS_SignerInfo_num(osis); i++) {
        osi = sk_CMS_SignerInfo_value(osis, i);
        if (!ASN1_STRING_cmp(osi->signature, rct->originatorSignatureValue))
            break;
    }

    if (i == sk_CMS_SignerInfo_num(osis)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_SIGNATURE);
        goto err;
    }

    si = sk_CMS_SignerInfo_value(sis, 0);

    /* Get msgSigDigest value and compare */

    msig = CMS_signed_get0_data_by_OBJ(si,
                                       OBJ_nid2obj
                                       (NID_id_smime_aa_msgSigDigest), -3,
                                       V_ASN1_OCTET_STRING);

    if (!msig) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_MSGSIGDIGEST);
        goto err;
    }

    if (!cms_msgSigDigest(osi, dig, &diglen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_MSGSIGDIGEST_ERROR);
        goto err;
    }

    if (diglen != (unsigned int)msig->length) {
        ERR_raise(ERR_LIB_CMS, CMS_R_MSGSIGDIGEST_WRONG_LENGTH);
        goto err;
    }

    if (memcmp(dig, msig->data, diglen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_MSGSIGDIGEST_VERIFICATION_FAILURE);
        goto err;
    }

    /* Compare content types */

    octype = CMS_signed_get0_data_by_OBJ(osi,
                                         OBJ_nid2obj(NID_pkcs9_contentType),
                                         -3, V_ASN1_OBJECT);
    if (!octype) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CONTENT_TYPE);
        goto err;
    }

    /* Compare details in receipt request */

    if (OBJ_cmp(octype, rct->contentType)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_MISMATCH);
        goto err;
    }

    /* Get original receipt request details */

    if (CMS_get1_ReceiptRequest(osi, &rr) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_RECEIPT_REQUEST);
        goto err;
    }

    if (ASN1_STRING_cmp(rr->signedContentIdentifier,
                        rct->signedContentIdentifier)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENTIDENTIFIER_MISMATCH);
        goto err;
    }

    r = 1;

 err:
    CMS_ReceiptRequest_free(rr);
    M_ASN1_free_of(rct, CMS_Receipt);
    return r;

}

/*
 * Encode a Receipt into an OCTET STRING read for including into content of a
 * SignedData ContentInfo.
 */

ASN1_OCTET_STRING *ossl_cms_encode_Receipt(CMS_SignerInfo *si)
{
    CMS_Receipt rct;
    CMS_ReceiptRequest *rr = NULL;
    ASN1_OBJECT *ctype;
    ASN1_OCTET_STRING *os = NULL;

    /* Get original receipt request */

    /* Get original receipt request details */

    if (CMS_get1_ReceiptRequest(si, &rr) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_RECEIPT_REQUEST);
        goto err;
    }

    /* Get original content type */

    ctype = CMS_signed_get0_data_by_OBJ(si,
                                        OBJ_nid2obj(NID_pkcs9_contentType),
                                        -3, V_ASN1_OBJECT);
    if (!ctype) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CONTENT_TYPE);
        goto err;
    }

    rct.version = 1;
    rct.contentType = ctype;
    rct.signedContentIdentifier = rr->signedContentIdentifier;
    rct.originatorSignatureValue = si->signature;

    os = ASN1_item_pack(&rct, ASN1_ITEM_rptr(CMS_Receipt), NULL);

 err:
    CMS_ReceiptRequest_free(rr);
    return os;
}
