/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_X509_ACERT_H
# define OSSL_CRYPTO_X509_ACERT_H
# pragma once

# include <openssl/x509_acert.h>

#define OSSL_ODI_TYPE_PUBLIC_KEY      0
#define OSSL_ODI_TYPE_PUBLIC_KEY_CERT 1
#define OSSL_ODI_TYPE_OTHER           2

struct ossl_object_digest_info_st {
    ASN1_ENUMERATED digestedObjectType;
    ASN1_OBJECT *otherObjectTypeID;
    X509_ALGOR digestAlgorithm;
    ASN1_BIT_STRING objectDigest;
};

struct ossl_issuer_serial_st {
    STACK_OF(GENERAL_NAME) *issuer;
    ASN1_INTEGER serial;
    ASN1_BIT_STRING *issuerUID;
};

struct X509_acert_issuer_v2form_st {
    STACK_OF(GENERAL_NAME) *issuerName;
    OSSL_ISSUER_SERIAL *baseCertificateId;
    OSSL_OBJECT_DIGEST_INFO *objectDigestInfo;
};

typedef struct X509_acert_issuer_st {
    int type;
    union {
        STACK_OF(GENERAL_NAME) *v1Form;
        X509_ACERT_ISSUER_V2FORM *v2Form;
    } u;
} X509_ACERT_ISSUER;

typedef struct X509_holder_st {
    OSSL_ISSUER_SERIAL *baseCertificateID;
    STACK_OF(GENERAL_NAME) *entityName;
    OSSL_OBJECT_DIGEST_INFO *objectDigestInfo;
} X509_HOLDER;

struct X509_acert_info_st {
    ASN1_INTEGER version;      /* default of v2 */
    X509_HOLDER holder;
    X509_ACERT_ISSUER issuer;
    X509_ALGOR signature;
    ASN1_INTEGER serialNumber;
    X509_VAL validityPeriod;
    STACK_OF(X509_ATTRIBUTE) *attributes;
    ASN1_BIT_STRING *issuerUID;
    X509_EXTENSIONS *extensions;
};

struct X509_acert_st {
    X509_ACERT_INFO *acinfo;
    X509_ALGOR sig_alg;
    ASN1_BIT_STRING signature;
};
#endif
