/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_CRYPTO_CRMF_H
# define OSSL_CRYPTO_CRMF_H
# pragma once

# include <openssl/crmf.h>

struct ossl_crmf_attributetypeandvalue_st {
    ASN1_OBJECT *type;
    union {
        /* NID_id_regCtrl_regToken */
        ASN1_UTF8STRING *regToken;

        /* NID_id_regCtrl_authenticator */
        ASN1_UTF8STRING *authenticator;

        /* NID_id_regCtrl_pkiPublicationInfo */
        OSSL_CRMF_PKIPUBLICATIONINFO *pkiPublicationInfo;

        /* NID_id_regCtrl_oldCertID */
        OSSL_CRMF_CERTID *oldCertID;

        /* NID_id_regCtrl_protocolEncrKey */
        X509_PUBKEY *protocolEncrKey;

        /* NID_id_regCtrl_algId */
        X509_ALGOR *algId;

        /* NID_id_regCtrl_rsaKeyLen */
        ASN1_INTEGER *rsaKeyLen;

        /* NID_id_regInfo_utf8Pairs */
        ASN1_UTF8STRING *utf8Pairs;

        /* NID_id_regInfo_certReq */
        OSSL_CRMF_CERTREQUEST *certReq;

        ASN1_TYPE *other;
    } value;
} /* OSSL_CRMF_ATTRIBUTETYPEANDVALUE */;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_ATTRIBUTETYPEANDVALUE)
DECLARE_ASN1_DUP_FUNCTION(OSSL_CRMF_ATTRIBUTETYPEANDVALUE)

#endif /* OSSL_CRYPTO_CRMF_H */
