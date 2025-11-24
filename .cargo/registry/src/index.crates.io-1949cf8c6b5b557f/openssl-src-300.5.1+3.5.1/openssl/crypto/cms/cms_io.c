/*
 * Copyright 2008-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/cms.h>
#include "cms_local.h"

/* unfortunately cannot constify BIO_new_NDEF() due to this and PKCS7_stream() */
int CMS_stream(unsigned char ***boundary, CMS_ContentInfo *cms)
{
    ASN1_OCTET_STRING **pos;

    pos = CMS_get0_content(cms);
    if (pos == NULL)
        return 0;
    if (*pos == NULL)
        *pos = ASN1_OCTET_STRING_new();
    if (*pos != NULL) {
        (*pos)->flags |= ASN1_STRING_FLAG_NDEF;
        (*pos)->flags &= ~ASN1_STRING_FLAG_CONT;
        *boundary = &(*pos)->data;
        return 1;
    }
    ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
    return 0;
}

CMS_ContentInfo *d2i_CMS_bio(BIO *bp, CMS_ContentInfo **cms)
{
    CMS_ContentInfo *ci;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms == NULL ? NULL : *cms);

    ci = ASN1_item_d2i_bio_ex(ASN1_ITEM_rptr(CMS_ContentInfo), bp, cms,
                              ossl_cms_ctx_get0_libctx(ctx),
                              ossl_cms_ctx_get0_propq(ctx));
    if (ci != NULL) {
        ERR_set_mark();
        ossl_cms_resolve_libctx(ci);
        ERR_pop_to_mark();
    }
    return ci;
}

int i2d_CMS_bio(BIO *bp, CMS_ContentInfo *cms)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(CMS_ContentInfo), bp, cms);
}

IMPLEMENT_PEM_rw(CMS, CMS_ContentInfo, PEM_STRING_CMS, CMS_ContentInfo)

BIO *BIO_new_CMS(BIO *out, CMS_ContentInfo *cms)
{
    return BIO_new_NDEF(out, (ASN1_VALUE *)cms,
                        ASN1_ITEM_rptr(CMS_ContentInfo));
}

/* CMS wrappers round generalised stream and MIME routines */

int i2d_CMS_bio_stream(BIO *out, CMS_ContentInfo *cms, BIO *in, int flags)
{
    return i2d_ASN1_bio_stream(out, (ASN1_VALUE *)cms, in, flags,
                               ASN1_ITEM_rptr(CMS_ContentInfo));
}

int PEM_write_bio_CMS_stream(BIO *out, CMS_ContentInfo *cms, BIO *in,
                             int flags)
{
    return PEM_write_bio_ASN1_stream(out, (ASN1_VALUE *)cms, in, flags,
                                     "CMS", ASN1_ITEM_rptr(CMS_ContentInfo));
}

int SMIME_write_CMS(BIO *bio, CMS_ContentInfo *cms, BIO *data, int flags)
{
    STACK_OF(X509_ALGOR) *mdalgs;
    int ctype_nid = OBJ_obj2nid(cms->contentType);
    int econt_nid = OBJ_obj2nid(CMS_get0_eContentType(cms));
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    if (ctype_nid == NID_pkcs7_signed)
        mdalgs = cms->d.signedData->digestAlgorithms;
    else
        mdalgs = NULL;

    return SMIME_write_ASN1_ex(bio, (ASN1_VALUE *)cms, data, flags, ctype_nid,
                               econt_nid, mdalgs,
                               ASN1_ITEM_rptr(CMS_ContentInfo),
                               ossl_cms_ctx_get0_libctx(ctx),
                               ossl_cms_ctx_get0_propq(ctx));
}

CMS_ContentInfo *SMIME_read_CMS_ex(BIO *bio, int flags, BIO **bcont,
                                   CMS_ContentInfo **cms)
{
    CMS_ContentInfo *ci;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms == NULL ? NULL : *cms);

    ci = (CMS_ContentInfo *)SMIME_read_ASN1_ex(bio, flags, bcont,
                                               ASN1_ITEM_rptr(CMS_ContentInfo),
                                               (ASN1_VALUE **)cms,
                                               ossl_cms_ctx_get0_libctx(ctx),
                                               ossl_cms_ctx_get0_propq(ctx));
    if (ci != NULL) {
        ERR_set_mark();
        ossl_cms_resolve_libctx(ci);
        ERR_pop_to_mark();
    }
    return ci;
}

CMS_ContentInfo *SMIME_read_CMS(BIO *bio, BIO **bcont)
{
    return SMIME_read_CMS_ex(bio, 0, bcont, NULL);
}
