/*
 * Copyright 1999-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/x509v3.h>
#include "crypto/x509.h"
#include "ext_dat.h"

static ASN1_OCTET_STRING *s2i_skey_id(X509V3_EXT_METHOD *method,
                                      X509V3_CTX *ctx, char *str);
const X509V3_EXT_METHOD ossl_v3_skey_id = {
    NID_subject_key_identifier, 0, ASN1_ITEM_ref(ASN1_OCTET_STRING),
    0, 0, 0, 0,
    (X509V3_EXT_I2S)i2s_ASN1_OCTET_STRING,
    (X509V3_EXT_S2I)s2i_skey_id,
    0, 0, 0, 0,
    NULL
};

char *i2s_ASN1_OCTET_STRING(X509V3_EXT_METHOD *method,
                            const ASN1_OCTET_STRING *oct)
{
    return OPENSSL_buf2hexstr(oct->data, oct->length);
}

ASN1_OCTET_STRING *s2i_ASN1_OCTET_STRING(X509V3_EXT_METHOD *method,
                                         X509V3_CTX *ctx, const char *str)
{
    ASN1_OCTET_STRING *oct;
    long length;

    if ((oct = ASN1_OCTET_STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        return NULL;
    }

    if ((oct->data = OPENSSL_hexstr2buf(str, &length)) == NULL) {
        ASN1_OCTET_STRING_free(oct);
        return NULL;
    }

    oct->length = length;

    return oct;

}

ASN1_OCTET_STRING *ossl_x509_pubkey_hash(X509_PUBKEY *pubkey)
{
    ASN1_OCTET_STRING *oct;
    const unsigned char *pk;
    int pklen;
    unsigned char pkey_dig[EVP_MAX_MD_SIZE];
    unsigned int diglen;
    const char *propq;
    OSSL_LIB_CTX *libctx;
    EVP_MD *md;

    if (pubkey == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_NO_PUBLIC_KEY);
        return NULL;
    }
    if (!ossl_x509_PUBKEY_get0_libctx(&libctx, &propq, pubkey))
        return NULL;
    if ((md = EVP_MD_fetch(libctx, SN_sha1, propq)) == NULL)
        return NULL;
    if ((oct = ASN1_OCTET_STRING_new()) == NULL) {
        EVP_MD_free(md);
        return NULL;
    }

    X509_PUBKEY_get0_param(NULL, &pk, &pklen, NULL, pubkey);
    if (EVP_Digest(pk, pklen, pkey_dig, &diglen, md, NULL)
            && ASN1_OCTET_STRING_set(oct, pkey_dig, diglen)) {
        EVP_MD_free(md);
        return oct;
    }

    EVP_MD_free(md);
    ASN1_OCTET_STRING_free(oct);
    return NULL;
}

static ASN1_OCTET_STRING *s2i_skey_id(X509V3_EXT_METHOD *method,
                                      X509V3_CTX *ctx, char *str)
{
    if (strcmp(str, "none") == 0)
        return ASN1_OCTET_STRING_new(); /* dummy */

    if (strcmp(str, "hash") != 0)
        return s2i_ASN1_OCTET_STRING(method, ctx /* not used */, str);

    if (ctx != NULL && (ctx->flags & X509V3_CTX_TEST) != 0)
        return ASN1_OCTET_STRING_new();
    if (ctx == NULL
        || (ctx->subject_cert == NULL && ctx->subject_req == NULL)) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_NO_SUBJECT_DETAILS);
        return NULL;
    }

    return ossl_x509_pubkey_hash(ctx->subject_cert != NULL ?
                                 ctx->subject_cert->cert_info.key :
                                 ctx->subject_req->req_info.pubkey);
}
