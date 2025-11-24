/*
 * Copyright 2008-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include <openssl/ess.h>
#include "internal/sizes.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/ess.h"
#include "crypto/x509.h" /* for ossl_x509_add_cert_new() */
#include "cms_local.h"

/* CMS SignedData Utilities */

static CMS_SignedData *cms_get0_signed(CMS_ContentInfo *cms)
{
    if (OBJ_obj2nid(cms->contentType) != NID_pkcs7_signed) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_NOT_SIGNED_DATA);
        return NULL;
    }
    return cms->d.signedData;
}

static CMS_SignedData *cms_signed_data_init(CMS_ContentInfo *cms)
{
    if (cms->d.other == NULL) {
        cms->d.signedData = M_ASN1_new_of(CMS_SignedData);
        if (!cms->d.signedData) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            return NULL;
        }
        cms->d.signedData->version = 1;
        cms->d.signedData->encapContentInfo->eContentType =
            OBJ_nid2obj(NID_pkcs7_data);
        cms->d.signedData->encapContentInfo->partial = 1;
        ASN1_OBJECT_free(cms->contentType);
        cms->contentType = OBJ_nid2obj(NID_pkcs7_signed);
        return cms->d.signedData;
    }
    return cms_get0_signed(cms);
}

/* Just initialise SignedData e.g. for certs only structure */
int CMS_SignedData_init(CMS_ContentInfo *cms)
{
    if (cms_signed_data_init(cms))
        return 1;
    else
        return 0;
}

/* Check structures and fixup version numbers (if necessary) */
static void cms_sd_set_version(CMS_SignedData *sd)
{
    int i;
    CMS_CertificateChoices *cch;
    CMS_RevocationInfoChoice *rch;
    CMS_SignerInfo *si;

    for (i = 0; i < sk_CMS_CertificateChoices_num(sd->certificates); i++) {
        cch = sk_CMS_CertificateChoices_value(sd->certificates, i);
        if (cch->type == CMS_CERTCHOICE_OTHER) {
            if (sd->version < 5)
                sd->version = 5;
        } else if (cch->type == CMS_CERTCHOICE_V2ACERT) {
            if (sd->version < 4)
                sd->version = 4;
        } else if (cch->type == CMS_CERTCHOICE_V1ACERT) {
            if (sd->version < 3)
                sd->version = 3;
        }
    }

    for (i = 0; i < sk_CMS_RevocationInfoChoice_num(sd->crls); i++) {
        rch = sk_CMS_RevocationInfoChoice_value(sd->crls, i);
        if (rch->type == CMS_REVCHOICE_OTHER) {
            if (sd->version < 5)
                sd->version = 5;
        }
    }

    if ((OBJ_obj2nid(sd->encapContentInfo->eContentType) != NID_pkcs7_data)
        && (sd->version < 3))
        sd->version = 3;

    for (i = 0; i < sk_CMS_SignerInfo_num(sd->signerInfos); i++) {
        si = sk_CMS_SignerInfo_value(sd->signerInfos, i);
        if (si->sid->type == CMS_SIGNERINFO_KEYIDENTIFIER) {
            if (si->version < 3)
                si->version = 3;
            if (sd->version < 3)
                sd->version = 3;
        } else if (si->version < 1) {
            si->version = 1;
        }
    }

    if (sd->version < 1)
        sd->version = 1;

}

/*
 * RFC 5652 Section 11.1 Content Type
 * The content-type attribute within signed-data MUST
 *   1) be present if there are signed attributes
 *   2) match the content type in the signed-data,
 *   3) be a signed attribute.
 *   4) not have more than one copy of the attribute.
 *
 * Note that since the CMS_SignerInfo_sign() always adds the "signing time"
 * attribute, the content type attribute MUST be added also.
 * Assumptions: This assumes that the attribute does not already exist.
 */
static int cms_set_si_contentType_attr(CMS_ContentInfo *cms, CMS_SignerInfo *si)
{
    ASN1_OBJECT *ctype = cms->d.signedData->encapContentInfo->eContentType;

    /* Add the contentType attribute */
    return CMS_signed_add1_attr_by_NID(si, NID_pkcs9_contentType,
                                       V_ASN1_OBJECT, ctype, -1) > 0;
}

/* Copy an existing messageDigest value */
static int cms_copy_messageDigest(CMS_ContentInfo *cms, CMS_SignerInfo *si)
{
    STACK_OF(CMS_SignerInfo) *sinfos;
    CMS_SignerInfo *sitmp;
    int i;

    sinfos = CMS_get0_SignerInfos(cms);
    for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
        ASN1_OCTET_STRING *messageDigest;

        sitmp = sk_CMS_SignerInfo_value(sinfos, i);
        if (sitmp == si)
            continue;
        if (CMS_signed_get_attr_count(sitmp) < 0)
            continue;
        if (OBJ_cmp(si->digestAlgorithm->algorithm,
                    sitmp->digestAlgorithm->algorithm))
            continue;
        messageDigest = CMS_signed_get0_data_by_OBJ(sitmp,
                                                    OBJ_nid2obj
                                                    (NID_pkcs9_messageDigest),
                                                    -3, V_ASN1_OCTET_STRING);
        if (!messageDigest) {
            ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_READING_MESSAGEDIGEST_ATTRIBUTE);
            return 0;
        }

        if (CMS_signed_add1_attr_by_NID(si, NID_pkcs9_messageDigest,
                                        V_ASN1_OCTET_STRING,
                                        messageDigest, -1))
            return 1;
        else
            return 0;
    }
    ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_DIGEST);
    return 0;
}

int ossl_cms_set1_SignerIdentifier(CMS_SignerIdentifier *sid, X509 *cert,
                                   int type, const CMS_CTX *ctx)
{
    switch (type) {
    case CMS_SIGNERINFO_ISSUER_SERIAL:
        if (!ossl_cms_set1_ias(&sid->d.issuerAndSerialNumber, cert))
            return 0;
        break;

    case CMS_SIGNERINFO_KEYIDENTIFIER:
        if (!ossl_cms_set1_keyid(&sid->d.subjectKeyIdentifier, cert))
            return 0;
        break;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNKNOWN_ID);
        return 0;
    }

    sid->type = type;

    return 1;
}

int ossl_cms_SignerIdentifier_get0_signer_id(CMS_SignerIdentifier *sid,
                                             ASN1_OCTET_STRING **keyid,
                                             X509_NAME **issuer,
                                             ASN1_INTEGER **sno)
{
    if (sid->type == CMS_SIGNERINFO_ISSUER_SERIAL) {
        if (issuer)
            *issuer = sid->d.issuerAndSerialNumber->issuer;
        if (sno)
            *sno = sid->d.issuerAndSerialNumber->serialNumber;
    } else if (sid->type == CMS_SIGNERINFO_KEYIDENTIFIER) {
        if (keyid)
            *keyid = sid->d.subjectKeyIdentifier;
    } else {
        return 0;
    }
    return 1;
}

int ossl_cms_SignerIdentifier_cert_cmp(CMS_SignerIdentifier *sid, X509 *cert)
{
    if (sid->type == CMS_SIGNERINFO_ISSUER_SERIAL)
        return ossl_cms_ias_cert_cmp(sid->d.issuerAndSerialNumber, cert);
    else if (sid->type == CMS_SIGNERINFO_KEYIDENTIFIER)
        return ossl_cms_keyid_cert_cmp(sid->d.subjectKeyIdentifier, cert);
    else
        return -1;
}

static int cms_signature_nomd(EVP_PKEY *pkey)
{
    char def_md[80];

    return EVP_PKEY_get_default_digest_name(pkey, def_md, sizeof(def_md)) == 2
        && strcmp(def_md, "UNDEF") == 0 ? 1 : 0;
}

/* Method to map any, incl. provider-implemented PKEY types to OIDs */
/* (EC)DSA and all provider-delivered signatures implementation is the same */
static int cms_generic_sign(CMS_SignerInfo *si, int verify)
{
    if (!ossl_assert(verify == 0 || verify == 1))
        return -1;

    if (!verify) {
        EVP_PKEY *pkey = si->pkey;
        int snid, hnid, pknid = EVP_PKEY_get_id(pkey);
        X509_ALGOR *alg1, *alg2;

        CMS_SignerInfo_get0_algs(si, NULL, NULL, &alg1, &alg2);
        if (alg1 == NULL || alg1->algorithm == NULL)
            return -1;
        hnid = OBJ_obj2nid(alg1->algorithm);
        if (hnid == NID_undef)
            return -1;
        if (pknid <= 0) { /* check whether a provider registered a NID */
            const char *typename = EVP_PKEY_get0_type_name(pkey);

            if (typename != NULL)
                pknid = OBJ_txt2nid(typename);
        }
        if (pknid > 0 && cms_signature_nomd(pkey))
            snid = pknid;
        else if (!OBJ_find_sigid_by_algs(&snid, hnid, pknid))
            return -1;
        return X509_ALGOR_set0(alg2, OBJ_nid2obj(snid), V_ASN1_UNDEF, NULL);
    }
    return 1;
}

static int cms_sd_asn1_ctrl(CMS_SignerInfo *si, int cmd)
{
    EVP_PKEY *pkey = si->pkey;
    int i;

    if (EVP_PKEY_is_a(pkey, "DSA") || EVP_PKEY_is_a(pkey, "EC"))
        return cms_generic_sign(si, cmd) > 0;
    else if (EVP_PKEY_is_a(pkey, "RSA") || EVP_PKEY_is_a(pkey, "RSA-PSS"))
        return ossl_cms_rsa_sign(si, cmd) > 0;

    /* Now give engines, providers, etc a chance to handle this */
    if (pkey->ameth == NULL || pkey->ameth->pkey_ctrl == NULL)
        return cms_generic_sign(si, cmd) > 0;
    i = pkey->ameth->pkey_ctrl(pkey, ASN1_PKEY_CTRL_CMS_SIGN, cmd, si);
    if (i == -2) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
        return 0;
    }
    if (i <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CTRL_FAILURE);
        return 0;
    }
    return 1;
}

/* Add SigningCertificate signed attribute to the signer info. */
static int ossl_cms_add1_signing_cert(CMS_SignerInfo *si,
                                      const ESS_SIGNING_CERT *sc)
{
    ASN1_STRING *seq = NULL;
    unsigned char *p, *pp = NULL;
    int ret, len = i2d_ESS_SIGNING_CERT(sc, NULL);

    if (len <= 0 || (pp = OPENSSL_malloc(len)) == NULL)
        return 0;

    p = pp;
    i2d_ESS_SIGNING_CERT(sc, &p);
    if (!(seq = ASN1_STRING_new()) || !ASN1_STRING_set(seq, pp, len)) {
        ASN1_STRING_free(seq);
        OPENSSL_free(pp);
        return 0;
    }
    OPENSSL_free(pp);
    ret = CMS_signed_add1_attr_by_NID(si, NID_id_smime_aa_signingCertificate,
                                      V_ASN1_SEQUENCE, seq, -1);
    ASN1_STRING_free(seq);
    return ret;
}

/* Add SigningCertificateV2 signed attribute to the signer info. */
static int ossl_cms_add1_signing_cert_v2(CMS_SignerInfo *si,
                                         const ESS_SIGNING_CERT_V2 *sc)
{
    ASN1_STRING *seq = NULL;
    unsigned char *p, *pp = NULL;
    int ret, len = i2d_ESS_SIGNING_CERT_V2(sc, NULL);

    if (len <= 0 || (pp = OPENSSL_malloc(len)) == NULL)
        return 0;

    p = pp;
    i2d_ESS_SIGNING_CERT_V2(sc, &p);
    if (!(seq = ASN1_STRING_new()) || !ASN1_STRING_set(seq, pp, len)) {
        ASN1_STRING_free(seq);
        OPENSSL_free(pp);
        return 0;
    }
    OPENSSL_free(pp);
    ret = CMS_signed_add1_attr_by_NID(si, NID_id_smime_aa_signingCertificateV2,
                                      V_ASN1_SEQUENCE, seq, -1);
    ASN1_STRING_free(seq);
    return ret;
}

CMS_SignerInfo *CMS_add1_signer(CMS_ContentInfo *cms,
                                X509 *signer, EVP_PKEY *pk, const EVP_MD *md,
                                unsigned int flags)
{
    CMS_SignedData *sd;
    CMS_SignerInfo *si = NULL;
    X509_ALGOR *alg;
    int i, type;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    if (!X509_check_private_key(signer, pk)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_PRIVATE_KEY_DOES_NOT_MATCH_CERTIFICATE);
        return NULL;
    }
    sd = cms_signed_data_init(cms);
    if (!sd)
        goto err;
    si = M_ASN1_new_of(CMS_SignerInfo);
    if (!si) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    /* Call for side-effect of computing hash and caching extensions */
    X509_check_purpose(signer, -1, -1);

    if (!X509_up_ref(signer))
        goto err;
    if (!EVP_PKEY_up_ref(pk)) {
        X509_free(signer);
        goto err;
    }

    si->cms_ctx = ctx;
    si->pkey = pk;
    si->signer = signer;
    si->mctx = EVP_MD_CTX_new();
    si->pctx = NULL;
    si->omit_signing_time = 0;

    if (si->mctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    if (flags & CMS_USE_KEYID) {
        si->version = 3;
        if (sd->version < 3)
            sd->version = 3;
        type = CMS_SIGNERINFO_KEYIDENTIFIER;
    } else {
        type = CMS_SIGNERINFO_ISSUER_SERIAL;
        si->version = 1;
    }

    if (!ossl_cms_set1_SignerIdentifier(si->sid, signer, type, ctx))
        goto err;

    if (md == NULL) {
        int def_nid;

        if (EVP_PKEY_get_default_digest_nid(pk, &def_nid) <= 0) {
            ERR_raise_data(ERR_LIB_CMS, CMS_R_NO_DEFAULT_DIGEST,
                           "pkey nid=%d", EVP_PKEY_get_id(pk));
            goto err;
        }
        md = EVP_get_digestbynid(def_nid);
        if (md == NULL) {
            ERR_raise_data(ERR_LIB_CMS, CMS_R_NO_DEFAULT_DIGEST,
                           "default md nid=%d", def_nid);
            goto err;
        }
    }

    X509_ALGOR_set_md(si->digestAlgorithm, md);

    /* See if digest is present in digestAlgorithms */
    for (i = 0; i < sk_X509_ALGOR_num(sd->digestAlgorithms); i++) {
        const ASN1_OBJECT *aoid;
        char name[OSSL_MAX_NAME_SIZE];

        alg = sk_X509_ALGOR_value(sd->digestAlgorithms, i);
        X509_ALGOR_get0(&aoid, NULL, NULL, alg);
        OBJ_obj2txt(name, sizeof(name), aoid, 0);
        if (EVP_MD_is_a(md, name))
            break;
    }

    if (i == sk_X509_ALGOR_num(sd->digestAlgorithms)) {
        if ((alg = X509_ALGOR_new()) == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            goto err;
        }
        X509_ALGOR_set_md(alg, md);
        if (!sk_X509_ALGOR_push(sd->digestAlgorithms, alg)) {
            X509_ALGOR_free(alg);
            ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    if (!(flags & CMS_KEY_PARAM) && !cms_sd_asn1_ctrl(si, 0)) {
        ERR_raise_data(ERR_LIB_CMS, CMS_R_UNSUPPORTED_SIGNATURE_ALGORITHM,
                       "pkey nid=%d", EVP_PKEY_get_id(pk));
        goto err;
    }
    if (!(flags & CMS_NOATTR)) {
        /*
         * Initialize signed attributes structure so other attributes
         * such as signing time etc are added later even if we add none here.
         */
        if (!si->signedAttrs) {
            si->signedAttrs = sk_X509_ATTRIBUTE_new_null();
            if (!si->signedAttrs) {
                ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
                goto err;
            }
        }

        if (!(flags & CMS_NOSMIMECAP)) {
            STACK_OF(X509_ALGOR) *smcap = NULL;

            i = CMS_add_standard_smimecap(&smcap);
            if (i)
                i = CMS_add_smimecap(si, smcap);
            sk_X509_ALGOR_pop_free(smcap, X509_ALGOR_free);
            if (!i) {
                ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
                goto err;
            }
        }
        if ((flags & CMS_NO_SIGNING_TIME) != 0) {
            /*
             * The signing-time signed attribute (NID_pkcs9_signingTime)
             * would normally be added later, in CMS_SignerInfo_sign(),
             * unless we set this flag here
             */
            si->omit_signing_time = 1;
        }
        if (flags & CMS_CADES) {
            ESS_SIGNING_CERT *sc = NULL;
            ESS_SIGNING_CERT_V2 *sc2 = NULL;
            int add_sc;

            if (md == NULL || EVP_MD_is_a(md, SN_sha1)) {
                if ((sc = OSSL_ESS_signing_cert_new_init(signer,
                                                         NULL, 1)) == NULL)
                    goto err;
                add_sc = ossl_cms_add1_signing_cert(si, sc);
                ESS_SIGNING_CERT_free(sc);
            } else {
                if ((sc2 = OSSL_ESS_signing_cert_v2_new_init(md, signer,
                                                             NULL, 1)) == NULL)
                    goto err;
                add_sc = ossl_cms_add1_signing_cert_v2(si, sc2);
                ESS_SIGNING_CERT_V2_free(sc2);
            }
            if (!add_sc)
                goto err;
        }
        if (flags & CMS_REUSE_DIGEST) {
            if (!cms_copy_messageDigest(cms, si))
                goto err;
            if (!cms_set_si_contentType_attr(cms, si))
                goto err;
            if (!(flags & (CMS_PARTIAL | CMS_KEY_PARAM)) &&
                !CMS_SignerInfo_sign(si))
                goto err;
        }
    }

    if (!(flags & CMS_NOCERTS)) {
        /* NB ignore -1 return for duplicate cert */
        if (!CMS_add1_cert(cms, signer)) {
            ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
            goto err;
        }
    }

    if (flags & CMS_KEY_PARAM) {
        if (flags & CMS_NOATTR) {
            si->pctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(ctx),
                                                  si->pkey,
                                                  ossl_cms_ctx_get0_propq(ctx));
            if (si->pctx == NULL)
                goto err;
            if (EVP_PKEY_sign_init(si->pctx) <= 0)
                goto err;
            if (EVP_PKEY_CTX_set_signature_md(si->pctx, md) <= 0)
                goto err;
        } else if (EVP_DigestSignInit_ex(si->mctx, &si->pctx,
                                         EVP_MD_get0_name(md),
                                         ossl_cms_ctx_get0_libctx(ctx),
                                         ossl_cms_ctx_get0_propq(ctx),
                                         pk, NULL) <= 0) {
            si->pctx = NULL;
            goto err;
        }
        else {
            EVP_MD_CTX_set_flags(si->mctx, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX);
        }
    }

    if (sd->signerInfos == NULL)
        sd->signerInfos = sk_CMS_SignerInfo_new_null();
    if (sd->signerInfos == NULL || !sk_CMS_SignerInfo_push(sd->signerInfos, si)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
        goto err;
    }

    return si;

 err:
    M_ASN1_free_of(si, CMS_SignerInfo);
    return NULL;

}

void ossl_cms_SignerInfos_set_cmsctx(CMS_ContentInfo *cms)
{
    int i;
    CMS_SignerInfo *si;
    STACK_OF(CMS_SignerInfo) *sinfos;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    ERR_set_mark();
    sinfos = CMS_get0_SignerInfos(cms);
    ERR_pop_to_mark(); /* removes error in case sinfos == NULL */

    for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
        si = sk_CMS_SignerInfo_value(sinfos, i);
        if (si != NULL)
            si->cms_ctx = ctx;
    }
}

static int cms_add1_signingTime(CMS_SignerInfo *si, ASN1_TIME *t)
{
    ASN1_TIME *tt;
    int r = 0;

    if (t != NULL)
        tt = t;
    else
        tt = X509_gmtime_adj(NULL, 0);

    if (tt == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_X509_LIB);
        goto err;
    }

    if (CMS_signed_add1_attr_by_NID(si, NID_pkcs9_signingTime,
                                    tt->type, tt, -1) <= 0) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }

    r = 1;
 err:
    if (t == NULL)
        ASN1_TIME_free(tt);

    return r;

}

EVP_PKEY_CTX *CMS_SignerInfo_get0_pkey_ctx(CMS_SignerInfo *si)
{
    return si->pctx;
}

EVP_MD_CTX *CMS_SignerInfo_get0_md_ctx(CMS_SignerInfo *si)
{
    return si->mctx;
}

STACK_OF(CMS_SignerInfo) *CMS_get0_SignerInfos(CMS_ContentInfo *cms)
{
    CMS_SignedData *sd = cms_get0_signed(cms);

    return sd != NULL ? sd->signerInfos : NULL;
}

STACK_OF(X509) *CMS_get0_signers(CMS_ContentInfo *cms)
{
    STACK_OF(X509) *signers = NULL;
    STACK_OF(CMS_SignerInfo) *sinfos;
    CMS_SignerInfo *si;
    int i;

    sinfos = CMS_get0_SignerInfos(cms);
    for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
        si = sk_CMS_SignerInfo_value(sinfos, i);
        if (si->signer != NULL) {
            if (!ossl_x509_add_cert_new(&signers, si->signer,
                                        X509_ADD_FLAG_DEFAULT)) {
                sk_X509_free(signers);
                return NULL;
            }
        }
    }
    return signers;
}

void CMS_SignerInfo_set1_signer_cert(CMS_SignerInfo *si, X509 *signer)
{
    if (signer != NULL) {
        if (!X509_up_ref(signer))
            return;
        EVP_PKEY_free(si->pkey);
        si->pkey = X509_get_pubkey(signer);
    }
    X509_free(si->signer);
    si->signer = signer;
}

int CMS_SignerInfo_get0_signer_id(CMS_SignerInfo *si,
                                  ASN1_OCTET_STRING **keyid,
                                  X509_NAME **issuer, ASN1_INTEGER **sno)
{
    return ossl_cms_SignerIdentifier_get0_signer_id(si->sid, keyid, issuer, sno);
}

int CMS_SignerInfo_cert_cmp(CMS_SignerInfo *si, X509 *cert)
{
    return ossl_cms_SignerIdentifier_cert_cmp(si->sid, cert);
}

int CMS_set1_signers_certs(CMS_ContentInfo *cms, STACK_OF(X509) *scerts,
                           unsigned int flags)
{
    CMS_SignedData *sd;
    CMS_SignerInfo *si;
    CMS_CertificateChoices *cch;
    STACK_OF(CMS_CertificateChoices) *certs;
    X509 *x;
    int i, j;
    int ret = 0;

    sd = cms_get0_signed(cms);
    if (sd == NULL)
        return -1;
    certs = sd->certificates;
    for (i = 0; i < sk_CMS_SignerInfo_num(sd->signerInfos); i++) {
        si = sk_CMS_SignerInfo_value(sd->signerInfos, i);
        if (si->signer != NULL)
            continue;

        for (j = 0; j < sk_X509_num(scerts); j++) {
            x = sk_X509_value(scerts, j);
            if (CMS_SignerInfo_cert_cmp(si, x) == 0) {
                CMS_SignerInfo_set1_signer_cert(si, x);
                ret++;
                break;
            }
        }

        if (si->signer != NULL || (flags & CMS_NOINTERN))
            continue;

        for (j = 0; j < sk_CMS_CertificateChoices_num(certs); j++) {
            cch = sk_CMS_CertificateChoices_value(certs, j);
            if (cch->type != CMS_CERTCHOICE_CERT)
                continue;
            x = cch->d.certificate;
            if (CMS_SignerInfo_cert_cmp(si, x) == 0) {
                CMS_SignerInfo_set1_signer_cert(si, x);
                ret++;
                break;
            }
        }
    }
    return ret;
}

void CMS_SignerInfo_get0_algs(CMS_SignerInfo *si, EVP_PKEY **pk,
                              X509 **signer, X509_ALGOR **pdig,
                              X509_ALGOR **psig)
{
    if (pk != NULL)
        *pk = si->pkey;
    if (signer != NULL)
        *signer = si->signer;
    if (pdig != NULL)
        *pdig = si->digestAlgorithm;
    if (psig != NULL)
        *psig = si->signatureAlgorithm;
}

ASN1_OCTET_STRING *CMS_SignerInfo_get0_signature(CMS_SignerInfo *si)
{
    return si->signature;
}

static int cms_SignerInfo_content_sign(CMS_ContentInfo *cms,
                                       CMS_SignerInfo *si, BIO *chain,
                                       const unsigned char *md,
                                       unsigned int mdlen)
{
    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    int r = 0;
    EVP_PKEY_CTX *pctx = NULL;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    if (mctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        return 0;
    }

    if (si->pkey == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_PRIVATE_KEY);
        goto err;
    }

    if (!ossl_cms_DigestAlgorithm_find_ctx(mctx, chain, si->digestAlgorithm))
        goto err;
    /* Set SignerInfo algorithm details if we used custom parameter */
    if (si->pctx && !cms_sd_asn1_ctrl(si, 0))
        goto err;

    /*
     * If any signed attributes calculate and add messageDigest attribute
     */
    if (CMS_signed_get_attr_count(si) >= 0) {
        unsigned char computed_md[EVP_MAX_MD_SIZE];

        if (md == NULL) {
            if (!EVP_DigestFinal_ex(mctx, computed_md, &mdlen))
                goto err;
            md = computed_md;
        }
        if (!CMS_signed_add1_attr_by_NID(si, NID_pkcs9_messageDigest,
                                         V_ASN1_OCTET_STRING, md, mdlen))
            goto err;
        /* Copy content type across */
        if (!cms_set_si_contentType_attr(cms, si))
            goto err;

        if (!CMS_SignerInfo_sign(si))
            goto err;
    } else if (si->pctx) {
        unsigned char *sig;
        size_t siglen;
        unsigned char computed_md[EVP_MAX_MD_SIZE];

        pctx = si->pctx;
        si->pctx = NULL;
        if (md == NULL) {
            if (!EVP_DigestFinal_ex(mctx, computed_md, &mdlen))
                goto err;
            md = computed_md;
        }
        siglen = EVP_PKEY_get_size(si->pkey);
        if (siglen == 0 || (sig = OPENSSL_malloc(siglen)) == NULL)
            goto err;
        if (EVP_PKEY_sign(pctx, sig, &siglen, md, mdlen) <= 0) {
            OPENSSL_free(sig);
            goto err;
        }
        ASN1_STRING_set0(si->signature, sig, siglen);
    } else {
        unsigned char *sig;
        unsigned int siglen;

        if (md != NULL) {
            ERR_raise(ERR_LIB_CMS, CMS_R_OPERATION_UNSUPPORTED);
            goto err;
        }
        siglen = EVP_PKEY_get_size(si->pkey);
        if (siglen == 0 || (sig = OPENSSL_malloc(siglen)) == NULL)
            goto err;
        if (!EVP_SignFinal_ex(mctx, sig, &siglen, si->pkey,
                              ossl_cms_ctx_get0_libctx(ctx),
                              ossl_cms_ctx_get0_propq(ctx))) {
            ERR_raise(ERR_LIB_CMS, CMS_R_SIGNFINAL_ERROR);
            OPENSSL_free(sig);
            goto err;
        }
        ASN1_STRING_set0(si->signature, sig, siglen);
    }

    r = 1;

 err:
    EVP_MD_CTX_free(mctx);
    EVP_PKEY_CTX_free(pctx);
    return r;

}

int ossl_cms_SignedData_final(CMS_ContentInfo *cms, BIO *chain,
                              const unsigned char *precomp_md,
                              unsigned int precomp_mdlen)
{
    STACK_OF(CMS_SignerInfo) *sinfos;
    CMS_SignerInfo *si;
    int i;

    sinfos = CMS_get0_SignerInfos(cms);
    for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
        si = sk_CMS_SignerInfo_value(sinfos, i);
        if (!cms_SignerInfo_content_sign(cms, si, chain,
                                         precomp_md, precomp_mdlen))
            return 0;
    }
    cms->d.signedData->encapContentInfo->partial = 0;
    return 1;
}

int CMS_SignerInfo_sign(CMS_SignerInfo *si)
{
    EVP_MD_CTX *mctx = si->mctx;
    EVP_PKEY_CTX *pctx = NULL;
    unsigned char *abuf = NULL;
    int alen;
    size_t siglen;
    const CMS_CTX *ctx = si->cms_ctx;
    char md_name_buf[OSSL_MAX_NAME_SIZE], *md_name;

    if (OBJ_obj2txt(md_name_buf, sizeof(md_name_buf),
                    si->digestAlgorithm->algorithm, 0) <= 0)
        return 0;
    md_name = cms_signature_nomd(si->pkey) ? NULL : md_name_buf;

    if (!si->omit_signing_time
        && CMS_signed_get_attr_by_NID(si, NID_pkcs9_signingTime, -1) < 0) {
        if (!cms_add1_signingTime(si, NULL))
            goto err;
    }

    if (!ossl_cms_si_check_attributes(si))
        goto err;

    if (si->pctx) {
        pctx = si->pctx;
    } else {
        EVP_MD_CTX_reset(mctx);
        if (EVP_DigestSignInit_ex(mctx, &pctx, md_name,
                                  ossl_cms_ctx_get0_libctx(ctx),
                                  ossl_cms_ctx_get0_propq(ctx), si->pkey,
                                  NULL) <= 0)
            goto err;
        EVP_MD_CTX_set_flags(mctx, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX);
        si->pctx = pctx;
    }

    if (md_name == NULL) {
        if (ASN1_item_sign_ctx(ASN1_ITEM_rptr(CMS_Attributes_Sign), NULL,
                               NULL, si->signature, si->signedAttrs, mctx) <= 0)
            goto err;
        return 1;
    }

    alen = ASN1_item_i2d((ASN1_VALUE *)si->signedAttrs, &abuf,
                         ASN1_ITEM_rptr(CMS_Attributes_Sign));
    if (alen < 0 || abuf == NULL)
        goto err;
    if (EVP_DigestSignUpdate(mctx, abuf, alen) <= 0)
        goto err;
    if (EVP_DigestSignFinal(mctx, NULL, &siglen) <= 0)
        goto err;
    OPENSSL_free(abuf);
    abuf = OPENSSL_malloc(siglen);
    if (abuf == NULL)
        goto err;
    if (EVP_DigestSignFinal(mctx, abuf, &siglen) <= 0)
        goto err;

    EVP_MD_CTX_reset(mctx);

    ASN1_STRING_set0(si->signature, abuf, siglen);

    return 1;

 err:
    OPENSSL_free(abuf);
    EVP_MD_CTX_reset(mctx);
    return 0;
}

int CMS_SignerInfo_verify(CMS_SignerInfo *si)
{
    EVP_MD_CTX *mctx = NULL;
    unsigned char *abuf = NULL;
    int alen, r = -1;
    char name[OSSL_MAX_NAME_SIZE];
    const EVP_MD *md;
    EVP_MD *fetched_md = NULL;
    const CMS_CTX *ctx = si->cms_ctx;
    OSSL_LIB_CTX *libctx = ossl_cms_ctx_get0_libctx(ctx);
    const char *propq = ossl_cms_ctx_get0_propq(ctx);

    if (si->pkey == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_PUBLIC_KEY);
        return -1;
    }

    if (!ossl_cms_si_check_attributes(si))
        return -1;

    if (cms_signature_nomd(si->pkey)) {
        r = ASN1_item_verify_ex(ASN1_ITEM_rptr(CMS_Attributes_Sign),
                                si->signatureAlgorithm, si->signature,
                                si->signedAttrs, NULL, si->pkey,
                                libctx, propq);
        if (r <= 0)
            ERR_raise(ERR_LIB_CMS, CMS_R_VERIFICATION_FAILURE);
        return r;
    }

    OBJ_obj2txt(name, sizeof(name), si->digestAlgorithm->algorithm, 0);

    (void)ERR_set_mark();
    fetched_md = EVP_MD_fetch(libctx, name, propq);

    if (fetched_md != NULL)
        md = fetched_md;
    else
        md = EVP_get_digestbyobj(si->digestAlgorithm->algorithm);
    if (md == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_CMS, CMS_R_UNKNOWN_DIGEST_ALGORITHM);
        return -1;
    }
    (void)ERR_pop_to_mark();

    if (si->mctx == NULL && (si->mctx = EVP_MD_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }
    mctx = si->mctx;
    if (si->pctx != NULL) {
        EVP_PKEY_CTX_free(si->pctx);
        si->pctx = NULL;
    }
    if (EVP_DigestVerifyInit_ex(mctx, &si->pctx, EVP_MD_get0_name(md), libctx,
                                propq, si->pkey, NULL) <= 0) {
        si->pctx = NULL;
        goto err;
    }
    EVP_MD_CTX_set_flags(mctx, EVP_MD_CTX_FLAG_KEEP_PKEY_CTX);

    if (!cms_sd_asn1_ctrl(si, 1))
        goto err;

    alen = ASN1_item_i2d((ASN1_VALUE *)si->signedAttrs, &abuf,
                         ASN1_ITEM_rptr(CMS_Attributes_Verify));
    if (abuf == NULL || alen < 0)
        goto err;
    r = EVP_DigestVerifyUpdate(mctx, abuf, alen);
    OPENSSL_free(abuf);
    if (r <= 0) {
        r = -1;
        goto err;
    }
    r = EVP_DigestVerifyFinal(mctx,
                              si->signature->data, si->signature->length);
    if (r <= 0)
        ERR_raise(ERR_LIB_CMS, CMS_R_VERIFICATION_FAILURE);
 err:
    EVP_MD_free(fetched_md);
    EVP_MD_CTX_reset(mctx);
    return r;
}

/* Create a chain of digest BIOs from a CMS ContentInfo */
BIO *ossl_cms_SignedData_init_bio(CMS_ContentInfo *cms)
{
    int i;
    CMS_SignedData *sd;
    BIO *chain = NULL;

    sd = cms_get0_signed(cms);
    if (sd == NULL)
        return NULL;
    if (cms->d.signedData->encapContentInfo->partial)
        cms_sd_set_version(sd);
    for (i = 0; i < sk_X509_ALGOR_num(sd->digestAlgorithms); i++) {
        X509_ALGOR *digestAlgorithm;
        BIO *mdbio;

        digestAlgorithm = sk_X509_ALGOR_value(sd->digestAlgorithms, i);
        mdbio = ossl_cms_DigestAlgorithm_init_bio(digestAlgorithm,
                                                  ossl_cms_get0_cmsctx(cms));
        if (mdbio == NULL)
            goto err;
        if (chain != NULL)
            BIO_push(chain, mdbio);
        else
            chain = mdbio;
    }
    return chain;
 err:
    BIO_free_all(chain);
    return NULL;
}

int CMS_SignerInfo_verify_content(CMS_SignerInfo *si, BIO *chain)
{
    ASN1_OCTET_STRING *os = NULL;
    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    EVP_PKEY_CTX *pkctx = NULL;
    int r = -1;
    unsigned char mval[EVP_MAX_MD_SIZE];
    unsigned int mlen;

    if (mctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }
    /* If we have any signed attributes look for messageDigest value */
    if (CMS_signed_get_attr_count(si) >= 0) {
        os = CMS_signed_get0_data_by_OBJ(si,
                                         OBJ_nid2obj(NID_pkcs9_messageDigest),
                                         -3, V_ASN1_OCTET_STRING);
        if (os == NULL) {
            ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_READING_MESSAGEDIGEST_ATTRIBUTE);
            goto err;
        }
    }

    if (!ossl_cms_DigestAlgorithm_find_ctx(mctx, chain, si->digestAlgorithm))
        goto err;

    if (EVP_DigestFinal_ex(mctx, mval, &mlen) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNABLE_TO_FINALIZE_CONTEXT);
        goto err;
    }

    /* If messageDigest found compare it */
    if (os != NULL) {
        if (mlen != (unsigned int)os->length) {
            ERR_raise(ERR_LIB_CMS, CMS_R_MESSAGEDIGEST_ATTRIBUTE_WRONG_LENGTH);
            goto err;
        }

        if (memcmp(mval, os->data, mlen)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_VERIFICATION_FAILURE);
            r = 0;
        } else {
            r = 1;
        }
    } else {
        const EVP_MD *md = EVP_MD_CTX_get0_md(mctx);
        const CMS_CTX *ctx = si->cms_ctx;

        pkctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(ctx),
                                           si->pkey,
                                           ossl_cms_ctx_get0_propq(ctx));
        if (pkctx == NULL)
            goto err;
        if (EVP_PKEY_verify_init(pkctx) <= 0)
            goto err;
        if (EVP_PKEY_CTX_set_signature_md(pkctx, md) <= 0)
            goto err;
        si->pctx = pkctx;
        if (!cms_sd_asn1_ctrl(si, 1)) {
            si->pctx = NULL;
            goto err;
        }
        si->pctx = NULL;
        r = EVP_PKEY_verify(pkctx, si->signature->data,
                            si->signature->length, mval, mlen);
        if (r <= 0) {
            ERR_raise(ERR_LIB_CMS, CMS_R_VERIFICATION_FAILURE);
            r = 0;
        }
    }

 err:
    EVP_PKEY_CTX_free(pkctx);
    EVP_MD_CTX_free(mctx);
    return r;

}

BIO *CMS_SignedData_verify(CMS_SignedData *sd, BIO *detached_data,
                           STACK_OF(X509) *scerts, X509_STORE *store,
                           STACK_OF(X509) *extra, STACK_OF(X509_CRL) *crls,
                           unsigned int flags,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *ci;
    BIO *bio = NULL;
    int i, res = 0;

    if (sd == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((ci = CMS_ContentInfo_new_ex(libctx, propq)) == NULL)
        return NULL;
    if ((bio = BIO_new(BIO_s_mem())) == NULL)
        goto end;
    ci->contentType = OBJ_nid2obj(NID_pkcs7_signed);
    ci->d.signedData = sd;

    for (i = 0; i < sk_X509_num(extra); i++)
        if (!CMS_add1_cert(ci, sk_X509_value(extra, i)))
            goto end;
    for (i = 0; i < sk_X509_CRL_num(crls); i++)
        if (!CMS_add1_crl(ci, sk_X509_CRL_value(crls, i)))
            goto end;
    res = CMS_verify(ci, scerts, store, detached_data, bio, flags);

 end:
    if (ci != NULL)
        ci->d.signedData = NULL; /* do not indirectly free |sd| */
    CMS_ContentInfo_free(ci);
    if (!res) {
        BIO_free(bio);
        bio = NULL;
    }
    return bio;
}

int CMS_add_smimecap(CMS_SignerInfo *si, STACK_OF(X509_ALGOR) *algs)
{
    unsigned char *smder = NULL;
    int smderlen, r;

    smderlen = i2d_X509_ALGORS(algs, &smder);
    if (smderlen <= 0)
        return 0;
    r = CMS_signed_add1_attr_by_NID(si, NID_SMIMECapabilities,
                                    V_ASN1_SEQUENCE, smder, smderlen);
    OPENSSL_free(smder);
    return r;
}

int CMS_add_simple_smimecap(STACK_OF(X509_ALGOR) **algs,
                            int algnid, int keysize)
{
    X509_ALGOR *alg;
    ASN1_INTEGER *key = NULL;

    if (keysize > 0) {
        key = ASN1_INTEGER_new();
        if (key == NULL || !ASN1_INTEGER_set(key, keysize)) {
            ASN1_INTEGER_free(key);
            return 0;
        }
    }
    alg = ossl_X509_ALGOR_from_nid(algnid, key != NULL ? V_ASN1_INTEGER :
                                   V_ASN1_UNDEF, key);
    if (alg == NULL) {
        ASN1_INTEGER_free(key);
        return 0;
    }

    if (*algs == NULL)
        *algs = sk_X509_ALGOR_new_null();
    if (*algs == NULL || !sk_X509_ALGOR_push(*algs, alg)) {
        X509_ALGOR_free(alg);
        return 0;
    }
    return 1;
}

/* Check to see if a cipher exists and if so add S/MIME capabilities */
static int cms_add_cipher_smcap(STACK_OF(X509_ALGOR) **sk, int nid, int arg)
{
    if (EVP_get_cipherbynid(nid))
        return CMS_add_simple_smimecap(sk, nid, arg);
    return 1;
}

static int cms_add_digest_smcap(STACK_OF(X509_ALGOR) **sk, int nid, int arg)
{
    if (EVP_get_digestbynid(nid))
        return CMS_add_simple_smimecap(sk, nid, arg);
    return 1;
}

int CMS_add_standard_smimecap(STACK_OF(X509_ALGOR) **smcap)
{
    if (!cms_add_cipher_smcap(smcap, NID_aes_256_cbc, -1)
        || !cms_add_digest_smcap(smcap, NID_id_GostR3411_2012_256, -1)
        || !cms_add_digest_smcap(smcap, NID_id_GostR3411_2012_512, -1)
        || !cms_add_digest_smcap(smcap, NID_id_GostR3411_94, -1)
        || !cms_add_cipher_smcap(smcap, NID_id_Gost28147_89, -1)
        || !cms_add_cipher_smcap(smcap, NID_aes_192_cbc, -1)
        || !cms_add_cipher_smcap(smcap, NID_aes_128_cbc, -1)
        || !cms_add_cipher_smcap(smcap, NID_des_ede3_cbc, -1)
        || !cms_add_cipher_smcap(smcap, NID_rc2_cbc, 128)
        || !cms_add_cipher_smcap(smcap, NID_rc2_cbc, 64)
        || !cms_add_cipher_smcap(smcap, NID_des_cbc, -1)
        || !cms_add_cipher_smcap(smcap, NID_rc2_cbc, 40))
        return 0;
    return 1;
}
