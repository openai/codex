/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <openssl/bio.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/pkcs7.h>
#include <openssl/x509.h>
#include <openssl/err.h>

int PKCS7_add_attrib_smimecap(PKCS7_SIGNER_INFO *si,
                              STACK_OF(X509_ALGOR) *cap)
{
    ASN1_STRING *seq;

    if ((seq = ASN1_STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
        return 0;
    }
    seq->length = ASN1_item_i2d((ASN1_VALUE *)cap, &seq->data,
                                ASN1_ITEM_rptr(X509_ALGORS));
    if (seq->length <= 0 || seq->data == NULL) {
        ASN1_STRING_free(seq);
        return 1;
    }
    if (!PKCS7_add_signed_attribute(si, NID_SMIMECapabilities,
                                    V_ASN1_SEQUENCE, seq)) {
        ASN1_STRING_free(seq);
        return 0;
    }
    return 1;
}

STACK_OF(X509_ALGOR) *PKCS7_get_smimecap(PKCS7_SIGNER_INFO *si)
{
    ASN1_TYPE *cap;
    const unsigned char *p;

    cap = PKCS7_get_signed_attribute(si, NID_SMIMECapabilities);
    if (cap == NULL || (cap->type != V_ASN1_SEQUENCE))
        return NULL;
    p = cap->value.sequence->data;
    return (STACK_OF(X509_ALGOR) *)
        ASN1_item_d2i(NULL, &p, cap->value.sequence->length,
                      ASN1_ITEM_rptr(X509_ALGORS));
}

/* Basic smime-capabilities OID and optional integer arg */
int PKCS7_simple_smimecap(STACK_OF(X509_ALGOR) *sk, int nid, int arg)
{
    ASN1_INTEGER *nbit = NULL;
    X509_ALGOR *alg;

    if ((alg = X509_ALGOR_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
        return 0;
    }
    ASN1_OBJECT_free(alg->algorithm);
    alg->algorithm = OBJ_nid2obj(nid);
    if (arg > 0) {
        if ((alg->parameter = ASN1_TYPE_new()) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            goto err;
        }
        if ((nbit = ASN1_INTEGER_new()) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            goto err;
        }
        if (!ASN1_INTEGER_set(nbit, arg)) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            goto err;
        }
        alg->parameter->value.integer = nbit;
        alg->parameter->type = V_ASN1_INTEGER;
        nbit = NULL;
    }
    if (!sk_X509_ALGOR_push(sk, alg)) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_CRYPTO_LIB);
        goto err;
    }
    return 1;
err:
    ASN1_INTEGER_free(nbit);
    X509_ALGOR_free(alg);
    return 0;
}

int PKCS7_add_attrib_content_type(PKCS7_SIGNER_INFO *si, ASN1_OBJECT *coid)
{
    if (PKCS7_get_signed_attribute(si, NID_pkcs9_contentType))
        return 0;
    if (!coid)
        coid = OBJ_nid2obj(NID_pkcs7_data);
    return PKCS7_add_signed_attribute(si, NID_pkcs9_contentType,
                                      V_ASN1_OBJECT, coid);
}

int PKCS7_add0_attrib_signing_time(PKCS7_SIGNER_INFO *si, ASN1_TIME *t)
{
    ASN1_TIME *tmp = NULL;

    if (t == NULL && (tmp = t = X509_gmtime_adj(NULL, 0)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_X509_LIB);
        return 0;
    }
    if (!PKCS7_add_signed_attribute(si, NID_pkcs9_signingTime,
                                    V_ASN1_UTCTIME, t)) {
        ASN1_TIME_free(tmp);
        return 0;
    }
    return 1;
}

int PKCS7_add1_attrib_digest(PKCS7_SIGNER_INFO *si,
                             const unsigned char *md, int mdlen)
{
    ASN1_OCTET_STRING *os;
    os = ASN1_OCTET_STRING_new();
    if (os == NULL)
        return 0;
    if (!ASN1_STRING_set(os, md, mdlen)
        || !PKCS7_add_signed_attribute(si, NID_pkcs9_messageDigest,
                                       V_ASN1_OCTET_STRING, os)) {
        ASN1_OCTET_STRING_free(os);
        return 0;
    }
    return 1;
}
