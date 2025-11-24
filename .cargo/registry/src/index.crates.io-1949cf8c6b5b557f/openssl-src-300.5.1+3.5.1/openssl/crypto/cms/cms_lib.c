/*
 * Copyright 2008-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/bio.h>
#include <openssl/asn1.h>
#include <openssl/cms.h>
#include <openssl/core_names.h>
#include "internal/sizes.h"
#include "internal/cryptlib.h"
#include "crypto/x509.h"
#include "cms_local.h"
#include "internal/cms.h"

static STACK_OF(CMS_CertificateChoices)
**cms_get0_certificate_choices(CMS_ContentInfo *cms);

IMPLEMENT_ASN1_ALLOC_FUNCTIONS(CMS_ContentInfo)
IMPLEMENT_ASN1_PRINT_FUNCTION(CMS_ContentInfo)

CMS_ContentInfo *d2i_CMS_ContentInfo(CMS_ContentInfo **a,
                                     const unsigned char **in, long len)
{
    CMS_ContentInfo *ci;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(a == NULL ? NULL : *a);

    ci = (CMS_ContentInfo *)ASN1_item_d2i_ex((ASN1_VALUE **)a, in, len,
                                          (CMS_ContentInfo_it()),
                                          ossl_cms_ctx_get0_libctx(ctx),
                                          ossl_cms_ctx_get0_propq(ctx));
    if (ci != NULL) {
        ERR_set_mark();
        ossl_cms_resolve_libctx(ci);
        ERR_pop_to_mark();
    }
    return ci;
}

int i2d_CMS_ContentInfo(const CMS_ContentInfo *a, unsigned char **out)
{
    return ASN1_item_i2d((const ASN1_VALUE *)a, out, (CMS_ContentInfo_it()));
}

CMS_ContentInfo *CMS_ContentInfo_new_ex(OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *ci;

    ci = (CMS_ContentInfo *)ASN1_item_new_ex(ASN1_ITEM_rptr(CMS_ContentInfo),
                                             libctx, propq);
    if (ci != NULL) {
        ci->ctx.libctx = libctx;
        ci->ctx.propq = NULL;
        if (propq != NULL) {
            ci->ctx.propq = OPENSSL_strdup(propq);
            if (ci->ctx.propq == NULL) {
                CMS_ContentInfo_free(ci);
                ci = NULL;
            }
        }
    }
    return ci;
}

const CMS_CTX *ossl_cms_get0_cmsctx(const CMS_ContentInfo *cms)
{
    return cms != NULL ? &cms->ctx : NULL;
}

OSSL_LIB_CTX *ossl_cms_ctx_get0_libctx(const CMS_CTX *ctx)
{
    return ctx != NULL ? ctx->libctx : NULL;
}

const char *ossl_cms_ctx_get0_propq(const CMS_CTX *ctx)
{
    return ctx != NULL ? ctx->propq : NULL;
}

void ossl_cms_resolve_libctx(CMS_ContentInfo *ci)
{
    int i;
    CMS_CertificateChoices *cch;
    STACK_OF(CMS_CertificateChoices) **pcerts;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(ci);
    OSSL_LIB_CTX *libctx = ossl_cms_ctx_get0_libctx(ctx);
    const char *propq = ossl_cms_ctx_get0_propq(ctx);

    ossl_cms_SignerInfos_set_cmsctx(ci);
    ossl_cms_RecipientInfos_set_cmsctx(ci);

    pcerts = cms_get0_certificate_choices(ci);
    if (pcerts != NULL) {
        for (i = 0; i < sk_CMS_CertificateChoices_num(*pcerts); i++) {
            cch = sk_CMS_CertificateChoices_value(*pcerts, i);
            if (cch->type == CMS_CERTCHOICE_CERT)
                ossl_x509_set0_libctx(cch->d.certificate, libctx, propq);
        }
    }
}

const ASN1_OBJECT *CMS_get0_type(const CMS_ContentInfo *cms)
{
    return cms->contentType;
}

CMS_ContentInfo *ossl_cms_Data_create(OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *cms = CMS_ContentInfo_new_ex(libctx, propq);

    if (cms != NULL) {
        cms->contentType = OBJ_nid2obj(NID_pkcs7_data);
        /* Never detached */
        CMS_set_detached(cms, 0);
    }
    return cms;
}

BIO *ossl_cms_content_bio(CMS_ContentInfo *cms)
{
    ASN1_OCTET_STRING **pos = CMS_get0_content(cms);

    if (pos == NULL)
        return NULL;
    /* If content detached data goes nowhere: create NULL BIO */
    if (*pos == NULL)
        return BIO_new(BIO_s_null());
    /*
     * If content not detached and created return memory BIO
     */
    if (*pos == NULL || ((*pos)->flags == ASN1_STRING_FLAG_CONT))
        return BIO_new(BIO_s_mem());
    /* Else content was read in: return read only BIO for it */
    return BIO_new_mem_buf((*pos)->data, (*pos)->length);
}

BIO *CMS_dataInit(CMS_ContentInfo *cms, BIO *icont)
{
    BIO *cmsbio, *cont;
    if (icont)
        cont = icont;
    else
        cont = ossl_cms_content_bio(cms);
    if (!cont) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CONTENT);
        return NULL;
    }
    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_data:
        return cont;

    case NID_pkcs7_signed:
        cmsbio = ossl_cms_SignedData_init_bio(cms);
        break;

    case NID_pkcs7_digest:
        cmsbio = ossl_cms_DigestedData_init_bio(cms);
        break;
#ifndef OPENSSL_NO_ZLIB
    case NID_id_smime_ct_compressedData:
        cmsbio = ossl_cms_CompressedData_init_bio(cms);
        break;
#endif

    case NID_pkcs7_encrypted:
        cmsbio = ossl_cms_EncryptedData_init_bio(cms);
        break;

    case NID_pkcs7_enveloped:
        cmsbio = ossl_cms_EnvelopedData_init_bio(cms);
        break;

    case NID_id_smime_ct_authEnvelopedData:
        cmsbio = ossl_cms_AuthEnvelopedData_init_bio(cms);
        break;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_TYPE);
        goto err;
    }

    if (cmsbio)
        return BIO_push(cmsbio, cont);
err:
    if (!icont)
        BIO_free(cont);
    return NULL;

}

/* unfortunately cannot constify SMIME_write_ASN1() due to this function */
int CMS_dataFinal(CMS_ContentInfo *cms, BIO *cmsbio)
{
    return ossl_cms_DataFinal(cms, cmsbio, NULL, 0);
}

int ossl_cms_DataFinal(CMS_ContentInfo *cms, BIO *cmsbio,
                       const unsigned char *precomp_md,
                       unsigned int precomp_mdlen)
{
    ASN1_OCTET_STRING **pos = CMS_get0_content(cms);

    if (pos == NULL)
        return 0;
    /* If embedded content find memory BIO and set content */
    if (*pos && ((*pos)->flags & ASN1_STRING_FLAG_CONT)) {
        BIO *mbio;
        unsigned char *cont;
        long contlen;
        mbio = BIO_find_type(cmsbio, BIO_TYPE_MEM);
        if (!mbio) {
            ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_NOT_FOUND);
            return 0;
        }
        contlen = BIO_get_mem_data(mbio, &cont);
        /* Set bio as read only so its content can't be clobbered */
        BIO_set_flags(mbio, BIO_FLAGS_MEM_RDONLY);
        BIO_set_mem_eof_return(mbio, 0);
        ASN1_STRING_set0(*pos, cont, contlen);
        (*pos)->flags &= ~ASN1_STRING_FLAG_CONT;
    }

    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_data:
    case NID_pkcs7_encrypted:
    case NID_id_smime_ct_compressedData:
        /* Nothing to do */
        return 1;

    case NID_pkcs7_enveloped:
        return ossl_cms_EnvelopedData_final(cms, cmsbio);

    case NID_id_smime_ct_authEnvelopedData:
        return ossl_cms_AuthEnvelopedData_final(cms, cmsbio);

    case NID_pkcs7_signed:
        return ossl_cms_SignedData_final(cms, cmsbio, precomp_md, precomp_mdlen);

    case NID_pkcs7_digest:
        return ossl_cms_DigestedData_do_final(cms, cmsbio, 0);

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_TYPE);
        return 0;
    }
}

/*
 * Return an OCTET STRING pointer to content. This allows it to be accessed
 * or set later.
 */

ASN1_OCTET_STRING **CMS_get0_content(CMS_ContentInfo *cms)
{
    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_data:
        return &cms->d.data;

    case NID_pkcs7_signed:
        return &cms->d.signedData->encapContentInfo->eContent;

    case NID_pkcs7_enveloped:
        return &cms->d.envelopedData->encryptedContentInfo->encryptedContent;

    case NID_pkcs7_digest:
        return &cms->d.digestedData->encapContentInfo->eContent;

    case NID_pkcs7_encrypted:
        return &cms->d.encryptedData->encryptedContentInfo->encryptedContent;

    case NID_id_smime_ct_authEnvelopedData:
        return &cms->d.authEnvelopedData->authEncryptedContentInfo
                                        ->encryptedContent;

    case NID_id_smime_ct_authData:
        return &cms->d.authenticatedData->encapContentInfo->eContent;

    case NID_id_smime_ct_compressedData:
        return &cms->d.compressedData->encapContentInfo->eContent;

    default:
        if (cms->d.other->type == V_ASN1_OCTET_STRING)
            return &cms->d.other->value.octet_string;
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_CONTENT_TYPE);
        return NULL;

    }
}

/*
 * Return an ASN1_OBJECT pointer to content type. This allows it to be
 * accessed or set later.
 */

static ASN1_OBJECT **cms_get0_econtent_type(CMS_ContentInfo *cms)
{
    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_signed:
        return &cms->d.signedData->encapContentInfo->eContentType;

    case NID_pkcs7_enveloped:
        return &cms->d.envelopedData->encryptedContentInfo->contentType;

    case NID_pkcs7_digest:
        return &cms->d.digestedData->encapContentInfo->eContentType;

    case NID_pkcs7_encrypted:
        return &cms->d.encryptedData->encryptedContentInfo->contentType;

    case NID_id_smime_ct_authEnvelopedData:
        return &cms->d.authEnvelopedData->authEncryptedContentInfo
                                        ->contentType;
    case NID_id_smime_ct_authData:
        return &cms->d.authenticatedData->encapContentInfo->eContentType;

    case NID_id_smime_ct_compressedData:
        return &cms->d.compressedData->encapContentInfo->eContentType;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_CONTENT_TYPE);
        return NULL;

    }
}

const ASN1_OBJECT *CMS_get0_eContentType(CMS_ContentInfo *cms)
{
    ASN1_OBJECT **petype;
    petype = cms_get0_econtent_type(cms);
    if (petype)
        return *petype;
    return NULL;
}

int CMS_set1_eContentType(CMS_ContentInfo *cms, const ASN1_OBJECT *oid)
{
    ASN1_OBJECT **petype, *etype;

    petype = cms_get0_econtent_type(cms);
    if (petype == NULL)
        return 0;
    if (oid == NULL)
        return 1;
    etype = OBJ_dup(oid);
    if (etype == NULL)
        return 0;
    ASN1_OBJECT_free(*petype);
    *petype = etype;
    return 1;
}

int CMS_is_detached(CMS_ContentInfo *cms)
{
    ASN1_OCTET_STRING **pos;

    pos = CMS_get0_content(cms);
    if (pos == NULL)
        return -1;
    if (*pos != NULL)
        return 0;
    return 1;
}

int CMS_set_detached(CMS_ContentInfo *cms, int detached)
{
    ASN1_OCTET_STRING **pos;

    pos = CMS_get0_content(cms);
    if (pos == NULL)
        return 0;
    if (detached) {
        ASN1_OCTET_STRING_free(*pos);
        *pos = NULL;
        return 1;
    }
    if (*pos == NULL)
        *pos = ASN1_OCTET_STRING_new();
    if (*pos != NULL) {
        /*
         * NB: special flag to show content is created and not read in.
         */
        (*pos)->flags |= ASN1_STRING_FLAG_CONT;
        return 1;
    }
    ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
    return 0;
}

/* Create a digest BIO from an X509_ALGOR structure */

BIO *ossl_cms_DigestAlgorithm_init_bio(X509_ALGOR *digestAlgorithm,
                                       const CMS_CTX *ctx)
{
    BIO *mdbio = NULL;
    const ASN1_OBJECT *digestoid;
    const EVP_MD *digest = NULL;
    EVP_MD *fetched_digest = NULL;
    char alg[OSSL_MAX_NAME_SIZE];
    size_t xof_len = 0;

    X509_ALGOR_get0(&digestoid, NULL, NULL, digestAlgorithm);
    OBJ_obj2txt(alg, sizeof(alg), digestoid, 0);

    (void)ERR_set_mark();
    fetched_digest = EVP_MD_fetch(ossl_cms_ctx_get0_libctx(ctx), alg,
                                  ossl_cms_ctx_get0_propq(ctx));

    if (fetched_digest != NULL)
        digest = fetched_digest;
    else
        digest = EVP_get_digestbyobj(digestoid);
    if (digest == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_CMS, CMS_R_UNKNOWN_DIGEST_ALGORITHM);
        goto err;
    }
    (void)ERR_pop_to_mark();

    mdbio = BIO_new(BIO_f_md());
    if (mdbio == NULL || BIO_set_md(mdbio, digest) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_MD_BIO_INIT_ERROR);
        goto err;
    }
    if (EVP_MD_xof(digest)) {
        if (EVP_MD_is_a(digest, SN_shake128))
            xof_len = 32;
        else if (EVP_MD_is_a(digest, SN_shake256))
            xof_len = 64;
        if (xof_len > 0) {
            EVP_MD_CTX *mdctx;
            OSSL_PARAM params[2];

            if (BIO_get_md_ctx(mdbio, &mdctx) <= 0 || mdctx == NULL)
                goto err;
            params[0] = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_XOFLEN,
                                                    &xof_len);
            params[1] = OSSL_PARAM_construct_end();
            if (!EVP_MD_CTX_set_params(mdctx, params))
                goto err;
        }
    }
    EVP_MD_free(fetched_digest);
    return mdbio;
 err:
    EVP_MD_free(fetched_digest);
    BIO_free(mdbio);
    return NULL;
}

/* Locate a message digest content from a BIO chain based on SignerInfo */

int ossl_cms_DigestAlgorithm_find_ctx(EVP_MD_CTX *mctx, BIO *chain,
                                      X509_ALGOR *mdalg)
{
    int nid;
    const ASN1_OBJECT *mdoid;
    X509_ALGOR_get0(&mdoid, NULL, NULL, mdalg);
    nid = OBJ_obj2nid(mdoid);
    /* Look for digest type to match signature */
    for (;;) {
        EVP_MD_CTX *mtmp;
        chain = BIO_find_type(chain, BIO_TYPE_MD);
        if (chain == NULL) {
            ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_DIGEST);
            return 0;
        }
        BIO_get_md_ctx(chain, &mtmp);
        if (EVP_MD_CTX_get_type(mtmp) == nid
            /*
             * Workaround for broken implementations that use signature
             * algorithm OID instead of digest.
             */
            || EVP_MD_get_pkey_type(EVP_MD_CTX_get0_md(mtmp)) == nid)
            return EVP_MD_CTX_copy_ex(mctx, mtmp);
        chain = BIO_next(chain);
    }
}

static STACK_OF(CMS_CertificateChoices)
**cms_get0_certificate_choices(CMS_ContentInfo *cms)
{
    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_signed:
        return &cms->d.signedData->certificates;

    case NID_pkcs7_enveloped:
        if (cms->d.envelopedData->originatorInfo == NULL)
            return NULL;
        return &cms->d.envelopedData->originatorInfo->certificates;

    case NID_id_smime_ct_authEnvelopedData:
        if (cms->d.authEnvelopedData->originatorInfo == NULL)
            return NULL;
        return &cms->d.authEnvelopedData->originatorInfo->certificates;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_CONTENT_TYPE);
        return NULL;

    }
}

CMS_CertificateChoices *CMS_add0_CertificateChoices(CMS_ContentInfo *cms)
{
    STACK_OF(CMS_CertificateChoices) **pcerts;
    CMS_CertificateChoices *cch;

    pcerts = cms_get0_certificate_choices(cms);
    if (pcerts == NULL)
        return NULL;
    if (*pcerts == NULL)
        *pcerts = sk_CMS_CertificateChoices_new_null();
    if (*pcerts == NULL)
        return NULL;
    cch = M_ASN1_new_of(CMS_CertificateChoices);
    if (!cch)
        return NULL;
    if (!sk_CMS_CertificateChoices_push(*pcerts, cch)) {
        M_ASN1_free_of(cch, CMS_CertificateChoices);
        return NULL;
    }
    return cch;
}

int CMS_add0_cert(CMS_ContentInfo *cms, X509 *cert)
{
    CMS_CertificateChoices *cch;
    STACK_OF(CMS_CertificateChoices) **pcerts;
    int i;

    pcerts = cms_get0_certificate_choices(cms);
    if (pcerts == NULL)
        return 0;
    for (i = 0; i < sk_CMS_CertificateChoices_num(*pcerts); i++) {
        cch = sk_CMS_CertificateChoices_value(*pcerts, i);
        if (cch->type == CMS_CERTCHOICE_CERT) {
            if (X509_cmp(cch->d.certificate, cert) == 0) {
                X509_free(cert);
                return 1; /* cert already present */
            }
        }
    }
    cch = CMS_add0_CertificateChoices(cms);
    if (!cch)
        return 0;
    cch->type = CMS_CERTCHOICE_CERT;
    cch->d.certificate = cert;
    return 1;
}

int CMS_add1_cert(CMS_ContentInfo *cms, X509 *cert)
{
    if (!X509_up_ref(cert))
        return 0;
    if (CMS_add0_cert(cms, cert))
        return 1;
    X509_free(cert);
    return 0;
}

static STACK_OF(CMS_RevocationInfoChoice)
**cms_get0_revocation_choices(CMS_ContentInfo *cms)
{
    switch (OBJ_obj2nid(cms->contentType)) {

    case NID_pkcs7_signed:
        return &cms->d.signedData->crls;

    case NID_pkcs7_enveloped:
        if (cms->d.envelopedData->originatorInfo == NULL)
            return NULL;
        return &cms->d.envelopedData->originatorInfo->crls;

    case NID_id_smime_ct_authEnvelopedData:
        if (cms->d.authEnvelopedData->originatorInfo == NULL)
            return NULL;
        return &cms->d.authEnvelopedData->originatorInfo->crls;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_CONTENT_TYPE);
        return NULL;

    }
}

CMS_RevocationInfoChoice *CMS_add0_RevocationInfoChoice(CMS_ContentInfo *cms)
{
    STACK_OF(CMS_RevocationInfoChoice) **pcrls;
    CMS_RevocationInfoChoice *rch;

    pcrls = cms_get0_revocation_choices(cms);
    if (pcrls == NULL)
        return NULL;
    if (*pcrls == NULL)
        *pcrls = sk_CMS_RevocationInfoChoice_new_null();
    if (*pcrls == NULL)
        return NULL;
    rch = M_ASN1_new_of(CMS_RevocationInfoChoice);
    if (rch == NULL)
        return NULL;
    if (!sk_CMS_RevocationInfoChoice_push(*pcrls, rch)) {
        M_ASN1_free_of(rch, CMS_RevocationInfoChoice);
        return NULL;
    }
    return rch;
}

int CMS_add0_crl(CMS_ContentInfo *cms, X509_CRL *crl)
{
    CMS_RevocationInfoChoice *rch = CMS_add0_RevocationInfoChoice(cms);

    if (rch == NULL)
        return 0;
    rch->type = CMS_REVCHOICE_CRL;
    rch->d.crl = crl;
    return 1;
}

int CMS_add1_crl(CMS_ContentInfo *cms, X509_CRL *crl)
{
    if (!X509_CRL_up_ref(crl))
        return 0;
    if (CMS_add0_crl(cms, crl))
        return 1;
    X509_CRL_free(crl);
    return 0;
}

STACK_OF(X509) *CMS_get1_certs(CMS_ContentInfo *cms)
{
    STACK_OF(X509) *certs = NULL;

    if (!ossl_cms_get1_certs_ex(cms, &certs))
        return NULL;
    if (sk_X509_num(certs) == 0) {
        sk_X509_free(certs);
        return NULL;
    }
    return certs;
}

int ossl_cms_get1_certs_ex(CMS_ContentInfo *cms, STACK_OF(X509) **certs)
{
    CMS_CertificateChoices *cch;
    STACK_OF(CMS_CertificateChoices) **pcerts;
    int i, n;

    if (certs == NULL)
        return 0;
    *certs = NULL;
    pcerts = cms_get0_certificate_choices(cms);
    if (pcerts == NULL)
        return 0;

    /* make sure to return NULL *certs only on error */
    n = sk_CMS_CertificateChoices_num(*pcerts);
    if ((*certs = sk_X509_new_reserve(NULL, n)) == NULL)
        return 0;

    for (i = 0; i < n; i++) {
        cch = sk_CMS_CertificateChoices_value(*pcerts, i);
        if (cch->type == 0) {
            if (!X509_add_cert(*certs, cch->d.certificate,
                               X509_ADD_FLAG_UP_REF)) {
                OSSL_STACK_OF_X509_free(*certs);
                *certs = NULL;
                return 0;
            }
        }
    }
    return 1;
}

STACK_OF(X509_CRL) *CMS_get1_crls(CMS_ContentInfo *cms)
{
    STACK_OF(X509_CRL) *crls = NULL;

    if (!ossl_cms_get1_crls_ex(cms, &crls))
        return NULL;
    if (sk_X509_CRL_num(crls) == 0) {
        sk_X509_CRL_free(crls);
        return NULL;
    }
    return crls;
}

int ossl_cms_get1_crls_ex(CMS_ContentInfo *cms, STACK_OF(X509_CRL) **crls)
{
    STACK_OF(CMS_RevocationInfoChoice) **pcrls;
    CMS_RevocationInfoChoice *rch;
    int i, n;

    if (crls == NULL)
        return 0;
    *crls = NULL;
    pcrls = cms_get0_revocation_choices(cms);
    if (pcrls == NULL)
        return 0;

    /* make sure to return NULL *crls only on error */
    n = sk_CMS_RevocationInfoChoice_num(*pcrls);
    if ((*crls = sk_X509_CRL_new_reserve(NULL, n)) == NULL)
        return 0;

    for (i = 0; i < n; i++) {
        rch = sk_CMS_RevocationInfoChoice_value(*pcrls, i);
        if (rch->type == 0) {
            if (!X509_CRL_up_ref(rch->d.crl)
                || !ossl_assert(sk_X509_CRL_push(*crls, rch->d.crl))) {
                /* push cannot fail on reserved stack */
                sk_X509_CRL_pop_free(*crls, X509_CRL_free);
                *crls = NULL;
                return 0;
            }
        }
    }
    return 1;
}

int ossl_cms_ias_cert_cmp(CMS_IssuerAndSerialNumber *ias, X509 *cert)
{
    int ret;
    ret = X509_NAME_cmp(ias->issuer, X509_get_issuer_name(cert));
    if (ret)
        return ret;
    return ASN1_INTEGER_cmp(ias->serialNumber, X509_get0_serialNumber(cert));
}

int ossl_cms_keyid_cert_cmp(ASN1_OCTET_STRING *keyid, X509 *cert)
{
    const ASN1_OCTET_STRING *cert_keyid = X509_get0_subject_key_id(cert);

    if (cert_keyid == NULL)
        return -1;
    return ASN1_OCTET_STRING_cmp(keyid, cert_keyid);
}

int ossl_cms_set1_ias(CMS_IssuerAndSerialNumber **pias, X509 *cert)
{
    CMS_IssuerAndSerialNumber *ias;
    ias = M_ASN1_new_of(CMS_IssuerAndSerialNumber);
    if (!ias) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    if (!X509_NAME_set(&ias->issuer, X509_get_issuer_name(cert))) {
        ERR_raise(ERR_LIB_CMS, ERR_R_X509_LIB);
        goto err;
    }
    if (!ASN1_STRING_copy(ias->serialNumber, X509_get0_serialNumber(cert))) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    M_ASN1_free_of(*pias, CMS_IssuerAndSerialNumber);
    *pias = ias;
    return 1;
 err:
    M_ASN1_free_of(ias, CMS_IssuerAndSerialNumber);
    return 0;
}

int ossl_cms_set1_keyid(ASN1_OCTET_STRING **pkeyid, X509 *cert)
{
    ASN1_OCTET_STRING *keyid = NULL;
    const ASN1_OCTET_STRING *cert_keyid;
    cert_keyid = X509_get0_subject_key_id(cert);
    if (cert_keyid == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CERTIFICATE_HAS_NO_KEYID);
        return 0;
    }
    keyid = ASN1_STRING_dup(cert_keyid);
    if (!keyid) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        return 0;
    }
    ASN1_OCTET_STRING_free(*pkeyid);
    *pkeyid = keyid;
    return 1;
}

CMS_EnvelopedData *ossl_cms_sign_encrypt(BIO *data, X509 *sign_cert, STACK_OF(X509) *certs,
                                         EVP_PKEY *sign_key, unsigned int sign_flags,
                                         STACK_OF(X509) *enc_recip, const EVP_CIPHER *cipher,
                                         unsigned int enc_flags, OSSL_LIB_CTX *libctx,
                                         const char *propq)
{
    CMS_EnvelopedData *evd = NULL;
    BIO *privbio = NULL, *signbio = NULL;
    CMS_ContentInfo *signcms = NULL, *evpcms = NULL;

    if (data == NULL || sign_key == NULL || sign_cert == NULL || enc_recip == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    signcms = CMS_sign_ex(sign_cert, sign_key, certs, data, sign_flags, libctx, propq);
    if (signcms == NULL)
        goto err;

    signbio = BIO_new(BIO_s_mem());
    if (signbio == NULL
        || ASN1_item_i2d_bio(ASN1_ITEM_rptr(CMS_SignedData), signbio, signcms->d.signedData) <= 0)
        goto err;

    evpcms = CMS_encrypt_ex(enc_recip, signbio, cipher, enc_flags, libctx, propq);
    if (evpcms == NULL)
        goto err;
    evd = CMS_EnvelopedData_dup(evpcms->d.envelopedData);

 err:
    BIO_free(privbio);
    BIO_free(signbio);
    CMS_ContentInfo_free(signcms);
    CMS_ContentInfo_free(evpcms);

    return evd;
}
