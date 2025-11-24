/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Fuzz ASN.1 parsing for various data structures. Specify which on the
 * command line:
 *
 * asn1 <data structure>
 */

/* We need to use some deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <string.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/dh.h>
#include <openssl/dsa.h>
#include <openssl/ec.h>
#include <openssl/ocsp.h>
#include <openssl/pkcs12.h>
#include <openssl/rsa.h>
#include <openssl/ts.h>
#include <openssl/x509v3.h>
#include <openssl/cms.h>
#include <openssl/ess.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/ssl.h>
#include <openssl/x509_acert.h>
#include "internal/nelem.h"
#include "fuzzer.h"

static ASN1_ITEM_EXP *item_type[] = {
    ASN1_ITEM_ref(ACCESS_DESCRIPTION),
#ifndef OPENSSL_NO_RFC3779
    ASN1_ITEM_ref(ASIdentifierChoice),
    ASN1_ITEM_ref(ASIdentifiers),
    ASN1_ITEM_ref(ASIdOrRange),
#endif
    ASN1_ITEM_ref(ASN1_ANY),
    ASN1_ITEM_ref(ASN1_BIT_STRING),
    ASN1_ITEM_ref(ASN1_BMPSTRING),
    ASN1_ITEM_ref(ASN1_BOOLEAN),
    ASN1_ITEM_ref(ASN1_ENUMERATED),
    ASN1_ITEM_ref(ASN1_FBOOLEAN),
    ASN1_ITEM_ref(ASN1_GENERALIZEDTIME),
    ASN1_ITEM_ref(ASN1_GENERALSTRING),
    ASN1_ITEM_ref(ASN1_IA5STRING),
    ASN1_ITEM_ref(ASN1_INTEGER),
    ASN1_ITEM_ref(ASN1_NULL),
    ASN1_ITEM_ref(ASN1_OBJECT),
    ASN1_ITEM_ref(ASN1_OCTET_STRING),
    ASN1_ITEM_ref(ASN1_OCTET_STRING_NDEF),
    ASN1_ITEM_ref(ASN1_PRINTABLE),
    ASN1_ITEM_ref(ASN1_PRINTABLESTRING),
    ASN1_ITEM_ref(ASN1_SEQUENCE),
    ASN1_ITEM_ref(ASN1_SEQUENCE_ANY),
    ASN1_ITEM_ref(ASN1_SET_ANY),
    ASN1_ITEM_ref(ASN1_T61STRING),
    ASN1_ITEM_ref(ASN1_TBOOLEAN),
    ASN1_ITEM_ref(ASN1_TIME),
    ASN1_ITEM_ref(ASN1_UNIVERSALSTRING),
    ASN1_ITEM_ref(ASN1_UTCTIME),
    ASN1_ITEM_ref(ASN1_UTF8STRING),
    ASN1_ITEM_ref(ASN1_VISIBLESTRING),
#ifndef OPENSSL_NO_RFC3779
    ASN1_ITEM_ref(ASRange),
#endif
    ASN1_ITEM_ref(AUTHORITY_INFO_ACCESS),
    ASN1_ITEM_ref(AUTHORITY_KEYID),
    ASN1_ITEM_ref(BASIC_CONSTRAINTS),
    ASN1_ITEM_ref(BIGNUM),
    ASN1_ITEM_ref(CBIGNUM),
    ASN1_ITEM_ref(CERTIFICATEPOLICIES),
#ifndef OPENSSL_NO_CMS
    ASN1_ITEM_ref(CMS_ContentInfo),
    ASN1_ITEM_ref(CMS_ReceiptRequest),
    ASN1_ITEM_ref(CRL_DIST_POINTS),
#endif
#ifndef OPENSSL_NO_DH
    ASN1_ITEM_ref(DHparams),
#endif
    ASN1_ITEM_ref(DIRECTORYSTRING),
    ASN1_ITEM_ref(DISPLAYTEXT),
    ASN1_ITEM_ref(DIST_POINT),
    ASN1_ITEM_ref(DIST_POINT_NAME),
#if !defined(OPENSSL_NO_EC) && !defined(OPENSSL_NO_DEPRECATED_3_0)
    ASN1_ITEM_ref(ECPARAMETERS),
    ASN1_ITEM_ref(ECPKPARAMETERS),
#endif
    ASN1_ITEM_ref(EDIPARTYNAME),
    ASN1_ITEM_ref(EXTENDED_KEY_USAGE),
    ASN1_ITEM_ref(GENERAL_NAME),
    ASN1_ITEM_ref(GENERAL_NAMES),
    ASN1_ITEM_ref(GENERAL_SUBTREE),
#ifndef OPENSSL_NO_RFC3779
    ASN1_ITEM_ref(IPAddressChoice),
    ASN1_ITEM_ref(IPAddressFamily),
    ASN1_ITEM_ref(IPAddressOrRange),
    ASN1_ITEM_ref(IPAddressRange),
#endif
    ASN1_ITEM_ref(ISSUING_DIST_POINT),
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ASN1_ITEM_ref(LONG),
#endif
    ASN1_ITEM_ref(NAME_CONSTRAINTS),
    ASN1_ITEM_ref(NETSCAPE_CERT_SEQUENCE),
    ASN1_ITEM_ref(NETSCAPE_SPKAC),
    ASN1_ITEM_ref(NETSCAPE_SPKI),
    ASN1_ITEM_ref(NOTICEREF),
#ifndef OPENSSL_NO_OCSP
    ASN1_ITEM_ref(OCSP_BASICRESP),
    ASN1_ITEM_ref(OCSP_CERTID),
    ASN1_ITEM_ref(OCSP_CERTSTATUS),
    ASN1_ITEM_ref(OCSP_CRLID),
    ASN1_ITEM_ref(OCSP_ONEREQ),
    ASN1_ITEM_ref(OCSP_REQINFO),
    ASN1_ITEM_ref(OCSP_REQUEST),
    ASN1_ITEM_ref(OCSP_RESPBYTES),
    ASN1_ITEM_ref(OCSP_RESPDATA),
    ASN1_ITEM_ref(OCSP_RESPID),
    ASN1_ITEM_ref(OCSP_RESPONSE),
    ASN1_ITEM_ref(OCSP_REVOKEDINFO),
    ASN1_ITEM_ref(OCSP_SERVICELOC),
    ASN1_ITEM_ref(OCSP_SIGNATURE),
    ASN1_ITEM_ref(OCSP_SINGLERESP),
#endif
    ASN1_ITEM_ref(OTHERNAME),
    ASN1_ITEM_ref(PBE2PARAM),
    ASN1_ITEM_ref(PBEPARAM),
    ASN1_ITEM_ref(PBKDF2PARAM),
    ASN1_ITEM_ref(PKCS12),
    ASN1_ITEM_ref(PKCS12_AUTHSAFES),
    ASN1_ITEM_ref(PKCS12_BAGS),
    ASN1_ITEM_ref(PKCS12_MAC_DATA),
    ASN1_ITEM_ref(PKCS12_SAFEBAG),
    ASN1_ITEM_ref(PKCS12_SAFEBAGS),
    ASN1_ITEM_ref(PKCS7),
    ASN1_ITEM_ref(PKCS7_ATTR_SIGN),
    ASN1_ITEM_ref(PKCS7_ATTR_VERIFY),
    ASN1_ITEM_ref(PKCS7_DIGEST),
    ASN1_ITEM_ref(PKCS7_ENC_CONTENT),
    ASN1_ITEM_ref(PKCS7_ENCRYPT),
    ASN1_ITEM_ref(PKCS7_ENVELOPE),
    ASN1_ITEM_ref(PKCS7_ISSUER_AND_SERIAL),
    ASN1_ITEM_ref(PKCS7_RECIP_INFO),
    ASN1_ITEM_ref(PKCS7_SIGNED),
    ASN1_ITEM_ref(PKCS7_SIGN_ENVELOPE),
    ASN1_ITEM_ref(PKCS7_SIGNER_INFO),
    ASN1_ITEM_ref(PKCS8_PRIV_KEY_INFO),
    ASN1_ITEM_ref(PKEY_USAGE_PERIOD),
    ASN1_ITEM_ref(POLICY_CONSTRAINTS),
    ASN1_ITEM_ref(POLICYINFO),
    ASN1_ITEM_ref(POLICY_MAPPING),
    ASN1_ITEM_ref(POLICY_MAPPINGS),
    ASN1_ITEM_ref(POLICYQUALINFO),
    ASN1_ITEM_ref(PROXY_CERT_INFO_EXTENSION),
    ASN1_ITEM_ref(PROXY_POLICY),
    ASN1_ITEM_ref(RSA_OAEP_PARAMS),
    ASN1_ITEM_ref(RSA_PSS_PARAMS),
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ASN1_ITEM_ref(RSAPrivateKey),
    ASN1_ITEM_ref(RSAPublicKey),
#endif
    ASN1_ITEM_ref(SXNET),
    ASN1_ITEM_ref(SXNETID),
    ASN1_ITEM_ref(OSSL_TARGETING_INFORMATION),
    ASN1_ITEM_ref(USERNOTICE),
    ASN1_ITEM_ref(X509),
    ASN1_ITEM_ref(X509_ALGOR),
    ASN1_ITEM_ref(X509_ALGORS),
    ASN1_ITEM_ref(X509_ATTRIBUTE),
    ASN1_ITEM_ref(X509_CERT_AUX),
    ASN1_ITEM_ref(X509_CINF),
    ASN1_ITEM_ref(X509_CRL),
    ASN1_ITEM_ref(X509_CRL_INFO),
    ASN1_ITEM_ref(X509_EXTENSION),
    ASN1_ITEM_ref(X509_EXTENSIONS),
    ASN1_ITEM_ref(X509_NAME),
    ASN1_ITEM_ref(X509_NAME_ENTRY),
    ASN1_ITEM_ref(X509_PUBKEY),
    ASN1_ITEM_ref(X509_REQ),
    ASN1_ITEM_ref(X509_REQ_INFO),
    ASN1_ITEM_ref(X509_REVOKED),
    ASN1_ITEM_ref(X509_SIG),
    ASN1_ITEM_ref(X509_VAL),
#ifndef OPENSSL_NO_DEPRECATED_3_0
    ASN1_ITEM_ref(ZLONG),
#endif
    ASN1_ITEM_ref(INT32),
    ASN1_ITEM_ref(ZINT32),
    ASN1_ITEM_ref(UINT32),
    ASN1_ITEM_ref(ZUINT32),
    ASN1_ITEM_ref(INT64),
    ASN1_ITEM_ref(ZINT64),
    ASN1_ITEM_ref(UINT64),
    ASN1_ITEM_ref(ZUINT64),
    NULL
};

static ASN1_PCTX *pctx;

#define DO_TEST(TYPE, D2I, I2D, PRINT) { \
    const unsigned char *p = buf; \
    unsigned char *der = NULL; \
    TYPE *type = D2I(NULL, &p, len); \
    \
    if (type != NULL) { \
        int len2; \
        BIO *bio = BIO_new(BIO_s_null()); \
        \
        if (bio != NULL) { \
            PRINT(bio, type); \
            BIO_free(bio); \
        } \
        len2 = I2D(type, &der); \
        if (len2 != 0) {} \
        OPENSSL_free(der); \
        TYPE ## _free(type); \
    } \
}

#define DO_TEST_PRINT_OFFSET(TYPE, D2I, I2D, PRINT) { \
    const unsigned char *p = buf; \
    unsigned char *der = NULL; \
    TYPE *type = D2I(NULL, &p, len); \
    \
    if (type != NULL) { \
        BIO *bio = BIO_new(BIO_s_null()); \
        \
        if (bio != NULL) { \
            PRINT(bio, type, 0); \
            BIO_free(bio); \
        } \
        I2D(type, &der); \
        OPENSSL_free(der); \
        TYPE ## _free(type); \
    } \
}

#define DO_TEST_PRINT_PCTX(TYPE, D2I, I2D, PRINT) { \
    const unsigned char *p = buf; \
    unsigned char *der = NULL; \
    TYPE *type = D2I(NULL, &p, len); \
    \
    if (type != NULL) { \
        BIO *bio = BIO_new(BIO_s_null()); \
        \
        if (bio != NULL) { \
            PRINT(bio, type, 0, pctx); \
            BIO_free(bio); \
        } \
        I2D(type, &der); \
        OPENSSL_free(der); \
        TYPE ## _free(type); \
    } \
}


#define DO_TEST_NO_PRINT(TYPE, D2I, I2D) { \
    const unsigned char *p = buf; \
    unsigned char *der = NULL; \
    TYPE *type = D2I(NULL, &p, len); \
    \
    if (type != NULL) { \
        BIO *bio = BIO_new(BIO_s_null()); \
        \
        BIO_free(bio); \
        I2D(type, &der); \
        OPENSSL_free(der); \
        TYPE ## _free(type); \
    } \
}


int FuzzerInitialize(int *argc, char ***argv)
{
    FuzzerSetRand();
    pctx = ASN1_PCTX_new();
    ASN1_PCTX_set_flags(pctx, ASN1_PCTX_FLAGS_SHOW_ABSENT |
        ASN1_PCTX_FLAGS_SHOW_SEQUENCE | ASN1_PCTX_FLAGS_SHOW_SSOF |
        ASN1_PCTX_FLAGS_SHOW_TYPE | ASN1_PCTX_FLAGS_SHOW_FIELD_STRUCT_NAME);
    ASN1_PCTX_set_str_flags(pctx, ASN1_STRFLGS_UTF8_CONVERT |
        ASN1_STRFLGS_SHOW_TYPE | ASN1_STRFLGS_DUMP_ALL);

    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);

    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    int n;


    for (n = 0; item_type[n] != NULL; ++n) {
        const uint8_t *b = buf;
        unsigned char *der = NULL;
        const ASN1_ITEM *i = ASN1_ITEM_ptr(item_type[n]);
        ASN1_VALUE *o = ASN1_item_d2i(NULL, &b, len, i);

        if (o != NULL) {
            /*
             * Don't print excessively long output to prevent spurious fuzzer
             * timeouts.
             */
            if (b - buf < 10000) {
                BIO *bio = BIO_new(BIO_s_null());
                if (bio != NULL) {
                    ASN1_item_print(bio, o, 4, i, pctx);
                    BIO_free(bio);
                }
            }
            if (ASN1_item_i2d(o, &der, i) > 0) {
                OPENSSL_free(der);
            }
            ASN1_item_free(o, i);
        }
    }

#ifndef OPENSSL_NO_TS
    DO_TEST(TS_REQ, d2i_TS_REQ, i2d_TS_REQ, TS_REQ_print_bio);
    DO_TEST(TS_MSG_IMPRINT, d2i_TS_MSG_IMPRINT, i2d_TS_MSG_IMPRINT, TS_MSG_IMPRINT_print_bio);
    DO_TEST(TS_RESP, d2i_TS_RESP, i2d_TS_RESP, TS_RESP_print_bio);
    DO_TEST(TS_STATUS_INFO, d2i_TS_STATUS_INFO, i2d_TS_STATUS_INFO, TS_STATUS_INFO_print_bio);
    DO_TEST(TS_TST_INFO, d2i_TS_TST_INFO, i2d_TS_TST_INFO, TS_TST_INFO_print_bio);
    DO_TEST_NO_PRINT(TS_ACCURACY, d2i_TS_ACCURACY, i2d_TS_ACCURACY);
#endif
    DO_TEST_NO_PRINT(ESS_ISSUER_SERIAL, d2i_ESS_ISSUER_SERIAL, i2d_ESS_ISSUER_SERIAL);
    DO_TEST_NO_PRINT(ESS_CERT_ID, d2i_ESS_CERT_ID, i2d_ESS_CERT_ID);
    DO_TEST_NO_PRINT(ESS_SIGNING_CERT, d2i_ESS_SIGNING_CERT, i2d_ESS_SIGNING_CERT);
    DO_TEST_NO_PRINT(ESS_CERT_ID_V2, d2i_ESS_CERT_ID_V2, i2d_ESS_CERT_ID_V2);
    DO_TEST_NO_PRINT(ESS_SIGNING_CERT_V2, d2i_ESS_SIGNING_CERT_V2, i2d_ESS_SIGNING_CERT_V2);
#if !defined(OPENSSL_NO_DH) && !defined(OPENSSL_NO_DEPRECATED_3_0)
    DO_TEST_NO_PRINT(DH, d2i_DHparams, i2d_DHparams);
    DO_TEST_NO_PRINT(DH, d2i_DHxparams, i2d_DHxparams);
#endif
#ifndef OPENSSL_NO_DSA
    DO_TEST_NO_PRINT(DSA_SIG, d2i_DSA_SIG, i2d_DSA_SIG);
# ifndef OPENSSL_NO_DEPRECATED_3_0
    DO_TEST_NO_PRINT(DSA, d2i_DSAPrivateKey, i2d_DSAPrivateKey);
    DO_TEST_NO_PRINT(DSA, d2i_DSAPublicKey, i2d_DSAPublicKey);
    DO_TEST_NO_PRINT(DSA, d2i_DSAparams, i2d_DSAparams);
# endif
#endif
#ifndef OPENSSL_NO_DEPRECATED_3_0
    DO_TEST_NO_PRINT(RSA, d2i_RSAPublicKey, i2d_RSAPublicKey);
#endif
#ifndef OPENSSL_NO_EC
# ifndef OPENSSL_NO_DEPRECATED_3_0
    DO_TEST_PRINT_OFFSET(EC_GROUP, d2i_ECPKParameters, i2d_ECPKParameters, ECPKParameters_print);
    DO_TEST_PRINT_OFFSET(EC_KEY, d2i_ECPrivateKey, i2d_ECPrivateKey, EC_KEY_print);
    DO_TEST(EC_KEY, d2i_ECParameters, i2d_ECParameters, ECParameters_print);
    DO_TEST_NO_PRINT(ECDSA_SIG, d2i_ECDSA_SIG, i2d_ECDSA_SIG);
# endif
#endif
    DO_TEST_PRINT_PCTX(EVP_PKEY, d2i_AutoPrivateKey, i2d_PrivateKey, EVP_PKEY_print_private);
    DO_TEST(SSL_SESSION, d2i_SSL_SESSION, i2d_SSL_SESSION, SSL_SESSION_print);

    ERR_clear_error();

    return 0;
}

void FuzzerCleanup(void)
{
    ASN1_PCTX_free(pctx);
    FuzzerClearRand();
}
