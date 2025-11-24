/*
 * Copyright 2008-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_CMS_LOCAL_H
# define OSSL_CRYPTO_CMS_LOCAL_H

# include <openssl/x509.h>

/*
 * Cryptographic message syntax (CMS) structures: taken from RFC3852
 */

/* Forward references */

typedef struct CMS_IssuerAndSerialNumber_st CMS_IssuerAndSerialNumber;
typedef struct CMS_EncapsulatedContentInfo_st CMS_EncapsulatedContentInfo;
typedef struct CMS_SignerIdentifier_st CMS_SignerIdentifier;
typedef struct CMS_OtherRevocationInfoFormat_st CMS_OtherRevocationInfoFormat;
typedef struct CMS_OriginatorInfo_st CMS_OriginatorInfo;
typedef struct CMS_EncryptedContentInfo_st CMS_EncryptedContentInfo;
typedef struct CMS_DigestedData_st CMS_DigestedData;
typedef struct CMS_EncryptedData_st CMS_EncryptedData;
typedef struct CMS_AuthenticatedData_st CMS_AuthenticatedData;
typedef struct CMS_AuthEnvelopedData_st CMS_AuthEnvelopedData;
typedef struct CMS_CompressedData_st CMS_CompressedData;
typedef struct CMS_OtherCertificateFormat_st CMS_OtherCertificateFormat;
typedef struct CMS_KeyTransRecipientInfo_st CMS_KeyTransRecipientInfo;
typedef struct CMS_OriginatorPublicKey_st CMS_OriginatorPublicKey;
typedef struct CMS_OriginatorIdentifierOrKey_st CMS_OriginatorIdentifierOrKey;
typedef struct CMS_KeyAgreeRecipientInfo_st CMS_KeyAgreeRecipientInfo;
typedef struct CMS_RecipientKeyIdentifier_st CMS_RecipientKeyIdentifier;
typedef struct CMS_KeyAgreeRecipientIdentifier_st
    CMS_KeyAgreeRecipientIdentifier;
typedef struct CMS_KEKIdentifier_st CMS_KEKIdentifier;
typedef struct CMS_KEKRecipientInfo_st CMS_KEKRecipientInfo;
typedef struct CMS_PasswordRecipientInfo_st CMS_PasswordRecipientInfo;
typedef struct CMS_OtherRecipientInfo_st CMS_OtherRecipientInfo;
typedef struct CMS_ReceiptsFrom_st CMS_ReceiptsFrom;
typedef struct CMS_CTX_st CMS_CTX;

struct CMS_CTX_st {
    OSSL_LIB_CTX *libctx;
    char *propq;
};

struct CMS_ContentInfo_st {
    ASN1_OBJECT *contentType;
    union {
        ASN1_OCTET_STRING *data;
        CMS_SignedData *signedData;
        CMS_EnvelopedData *envelopedData;
        CMS_DigestedData *digestedData;
        CMS_EncryptedData *encryptedData;
        CMS_AuthEnvelopedData *authEnvelopedData;
        CMS_AuthenticatedData *authenticatedData;
        CMS_CompressedData *compressedData;
        ASN1_TYPE *other;
        /* Other types ... */
        void *otherData;
    } d;
    CMS_CTX ctx;
};

DEFINE_STACK_OF(CMS_CertificateChoices)

struct CMS_SignedData_st {
    int32_t version;
    STACK_OF(X509_ALGOR) *digestAlgorithms;
    CMS_EncapsulatedContentInfo *encapContentInfo;
    STACK_OF(CMS_CertificateChoices) *certificates;
    STACK_OF(CMS_RevocationInfoChoice) *crls;
    STACK_OF(CMS_SignerInfo) *signerInfos;
};

struct CMS_EncapsulatedContentInfo_st {
    ASN1_OBJECT *eContentType;
    ASN1_OCTET_STRING *eContent;
    /* Set to 1 if incomplete structure only part set up */
    int partial;
};

struct CMS_SignerInfo_st {
    int32_t version;
    CMS_SignerIdentifier *sid;
    X509_ALGOR *digestAlgorithm;
    STACK_OF(X509_ATTRIBUTE) *signedAttrs;
    X509_ALGOR *signatureAlgorithm;
    ASN1_OCTET_STRING *signature;
    STACK_OF(X509_ATTRIBUTE) *unsignedAttrs;
    /* Signing certificate and key */
    X509 *signer;
    EVP_PKEY *pkey;
    /* Digest and public key context for alternative parameters */
    EVP_MD_CTX *mctx;
    EVP_PKEY_CTX *pctx;
    const CMS_CTX *cms_ctx;
    /* Set to 1 if signing time attribute is to be omitted */
    int omit_signing_time;
};

struct CMS_SignerIdentifier_st {
    int type;
    union {
        CMS_IssuerAndSerialNumber *issuerAndSerialNumber;
        ASN1_OCTET_STRING *subjectKeyIdentifier;
    } d;
};

struct CMS_EnvelopedData_st {
    int32_t version;
    CMS_OriginatorInfo *originatorInfo;
    STACK_OF(CMS_RecipientInfo) *recipientInfos;
    CMS_EncryptedContentInfo *encryptedContentInfo;
    STACK_OF(X509_ATTRIBUTE) *unprotectedAttrs;
};

struct CMS_OriginatorInfo_st {
    STACK_OF(CMS_CertificateChoices) *certificates;
    STACK_OF(CMS_RevocationInfoChoice) *crls;
};

struct CMS_EncryptedContentInfo_st {
    ASN1_OBJECT *contentType;
    X509_ALGOR *contentEncryptionAlgorithm;
    ASN1_OCTET_STRING *encryptedContent;
    /* Content encryption algorithm, key and tag */
    const EVP_CIPHER *cipher;
    unsigned char *key;
    size_t keylen;
    unsigned char *tag;
    size_t taglen;
    /* Set to 1 if we are debugging decrypt and don't fake keys for MMA */
    int debug;
    /* Set to 1 if we have no cert and need extra safety measures for MMA */
    int havenocert;
};

struct CMS_RecipientInfo_st {
    int type;
    union {
        CMS_KeyTransRecipientInfo *ktri;
        CMS_KeyAgreeRecipientInfo *kari;
        CMS_KEKRecipientInfo *kekri;
        CMS_PasswordRecipientInfo *pwri;
        CMS_OtherRecipientInfo *ori;
    } d;
};

typedef CMS_SignerIdentifier CMS_RecipientIdentifier;

struct CMS_KeyTransRecipientInfo_st {
    int32_t version;
    CMS_RecipientIdentifier *rid;
    X509_ALGOR *keyEncryptionAlgorithm;
    ASN1_OCTET_STRING *encryptedKey;
    /* Recipient Key and cert */
    X509 *recip;
    EVP_PKEY *pkey;
    /* Public key context for this operation */
    EVP_PKEY_CTX *pctx;
    const CMS_CTX *cms_ctx;
};

struct CMS_KeyAgreeRecipientInfo_st {
    int32_t version;
    CMS_OriginatorIdentifierOrKey *originator;
    ASN1_OCTET_STRING *ukm;
    X509_ALGOR *keyEncryptionAlgorithm;
    STACK_OF(CMS_RecipientEncryptedKey) *recipientEncryptedKeys;
    /* Public key context associated with current operation */
    EVP_PKEY_CTX *pctx;
    /* Cipher context for CEK wrapping */
    EVP_CIPHER_CTX *ctx;
    const CMS_CTX *cms_ctx;
};

struct CMS_OriginatorIdentifierOrKey_st {
    int type;
    union {
        CMS_IssuerAndSerialNumber *issuerAndSerialNumber;
        ASN1_OCTET_STRING *subjectKeyIdentifier;
        CMS_OriginatorPublicKey *originatorKey;
    } d;
};

struct CMS_OriginatorPublicKey_st {
    X509_ALGOR *algorithm;
    ASN1_BIT_STRING *publicKey;
};

struct CMS_RecipientEncryptedKey_st {
    CMS_KeyAgreeRecipientIdentifier *rid;
    ASN1_OCTET_STRING *encryptedKey;
    /* Public key associated with this recipient */
    EVP_PKEY *pkey;
};

struct CMS_KeyAgreeRecipientIdentifier_st {
    int type;
    union {
        CMS_IssuerAndSerialNumber *issuerAndSerialNumber;
        CMS_RecipientKeyIdentifier *rKeyId;
    } d;
};

struct CMS_RecipientKeyIdentifier_st {
    ASN1_OCTET_STRING *subjectKeyIdentifier;
    ASN1_GENERALIZEDTIME *date;
    CMS_OtherKeyAttribute *other;
};

struct CMS_KEKRecipientInfo_st {
    int32_t version;
    CMS_KEKIdentifier *kekid;
    X509_ALGOR *keyEncryptionAlgorithm;
    ASN1_OCTET_STRING *encryptedKey;
    /* Extra info: symmetric key to use */
    unsigned char *key;
    size_t keylen;
    const CMS_CTX *cms_ctx;
};

struct CMS_KEKIdentifier_st {
    ASN1_OCTET_STRING *keyIdentifier;
    ASN1_GENERALIZEDTIME *date;
    CMS_OtherKeyAttribute *other;
};

struct CMS_PasswordRecipientInfo_st {
    int32_t version;
    X509_ALGOR *keyDerivationAlgorithm;
    X509_ALGOR *keyEncryptionAlgorithm;
    ASN1_OCTET_STRING *encryptedKey;
    /* Extra info: password to use */
    unsigned char *pass;
    size_t passlen;
    const CMS_CTX *cms_ctx;
};

struct CMS_OtherRecipientInfo_st {
    ASN1_OBJECT *oriType;
    ASN1_TYPE *oriValue;
};

struct CMS_DigestedData_st {
    int32_t version;
    X509_ALGOR *digestAlgorithm;
    CMS_EncapsulatedContentInfo *encapContentInfo;
    ASN1_OCTET_STRING *digest;
};

struct CMS_EncryptedData_st {
    int32_t version;
    CMS_EncryptedContentInfo *encryptedContentInfo;
    STACK_OF(X509_ATTRIBUTE) *unprotectedAttrs;
};

struct CMS_AuthenticatedData_st {
    int32_t version;
    CMS_OriginatorInfo *originatorInfo;
    STACK_OF(CMS_RecipientInfo) *recipientInfos;
    X509_ALGOR *macAlgorithm;
    X509_ALGOR *digestAlgorithm;
    CMS_EncapsulatedContentInfo *encapContentInfo;
    STACK_OF(X509_ATTRIBUTE) *authAttrs;
    ASN1_OCTET_STRING *mac;
    STACK_OF(X509_ATTRIBUTE) *unauthAttrs;
};

struct CMS_AuthEnvelopedData_st {
    int32_t version;
    CMS_OriginatorInfo *originatorInfo;
    STACK_OF(CMS_RecipientInfo) *recipientInfos;
    CMS_EncryptedContentInfo *authEncryptedContentInfo;
    STACK_OF(X509_ATTRIBUTE) *authAttrs;
    ASN1_OCTET_STRING *mac;
    STACK_OF(X509_ATTRIBUTE) *unauthAttrs;
};

struct CMS_CompressedData_st {
    int32_t version;
    X509_ALGOR *compressionAlgorithm;
    STACK_OF(CMS_RecipientInfo) *recipientInfos;
    CMS_EncapsulatedContentInfo *encapContentInfo;
};

struct CMS_RevocationInfoChoice_st {
    int type;
    union {
        X509_CRL *crl;
        CMS_OtherRevocationInfoFormat *other;
    } d;
};

# define CMS_REVCHOICE_CRL               0
# define CMS_REVCHOICE_OTHER             1

struct CMS_OtherRevocationInfoFormat_st {
    ASN1_OBJECT *otherRevInfoFormat;
    ASN1_TYPE *otherRevInfo;
};

struct CMS_CertificateChoices {
    int type;
    union {
        X509 *certificate;
        ASN1_STRING *extendedCertificate; /* Obsolete */
        ASN1_STRING *v1AttrCert; /* Left encoded for now */
        ASN1_STRING *v2AttrCert; /* Left encoded for now */
        CMS_OtherCertificateFormat *other;
    } d;
};

# define CMS_CERTCHOICE_CERT             0
# define CMS_CERTCHOICE_EXCERT           1
# define CMS_CERTCHOICE_V1ACERT          2
# define CMS_CERTCHOICE_V2ACERT          3
# define CMS_CERTCHOICE_OTHER            4

struct CMS_OtherCertificateFormat_st {
    ASN1_OBJECT *otherCertFormat;
    ASN1_TYPE *otherCert;
};

/*
 * This is also defined in pkcs7.h but we duplicate it to allow the CMS code
 * to be independent of PKCS#7
 */

struct CMS_IssuerAndSerialNumber_st {
    X509_NAME *issuer;
    ASN1_INTEGER *serialNumber;
};

struct CMS_OtherKeyAttribute_st {
    ASN1_OBJECT *keyAttrId;
    ASN1_TYPE *keyAttr;
};

/* ESS structures */

struct CMS_ReceiptRequest_st {
    ASN1_OCTET_STRING *signedContentIdentifier;
    CMS_ReceiptsFrom *receiptsFrom;
    STACK_OF(GENERAL_NAMES) *receiptsTo;
};

struct CMS_ReceiptsFrom_st {
    int type;
    union {
        int32_t allOrFirstTier;
        STACK_OF(GENERAL_NAMES) *receiptList;
    } d;
};

struct CMS_Receipt_st {
    int32_t version;
    ASN1_OBJECT *contentType;
    ASN1_OCTET_STRING *signedContentIdentifier;
    ASN1_OCTET_STRING *originatorSignatureValue;
};

DECLARE_ASN1_FUNCTIONS(CMS_ContentInfo)
DECLARE_ASN1_ITEM(CMS_SignerInfo)
DECLARE_ASN1_ITEM(CMS_EncryptedContentInfo)
DECLARE_ASN1_ITEM(CMS_IssuerAndSerialNumber)
DECLARE_ASN1_ITEM(CMS_Attributes_Sign)
DECLARE_ASN1_ITEM(CMS_Attributes_Verify)
DECLARE_ASN1_ITEM(CMS_RecipientInfo)
DECLARE_ASN1_ITEM(CMS_PasswordRecipientInfo)
DECLARE_ASN1_ALLOC_FUNCTIONS(CMS_IssuerAndSerialNumber)

# define CMS_SIGNERINFO_ISSUER_SERIAL    0
# define CMS_SIGNERINFO_KEYIDENTIFIER    1

# define CMS_RECIPINFO_ISSUER_SERIAL     0
# define CMS_RECIPINFO_KEYIDENTIFIER     1

# define CMS_REK_ISSUER_SERIAL           0
# define CMS_REK_KEYIDENTIFIER           1

# define CMS_OIK_ISSUER_SERIAL           0
# define CMS_OIK_KEYIDENTIFIER           1
# define CMS_OIK_PUBKEY                  2

BIO *ossl_cms_content_bio(CMS_ContentInfo *cms);
const CMS_CTX *ossl_cms_get0_cmsctx(const CMS_ContentInfo *cms);
OSSL_LIB_CTX *ossl_cms_ctx_get0_libctx(const CMS_CTX *ctx);
const char *ossl_cms_ctx_get0_propq(const CMS_CTX *ctx);
void ossl_cms_resolve_libctx(CMS_ContentInfo *ci);

CMS_ContentInfo *ossl_cms_Data_create(OSSL_LIB_CTX *ctx, const char *propq);
int ossl_cms_DataFinal(CMS_ContentInfo *cms, BIO *cmsbio,
                       const unsigned char *precomp_md,
                       unsigned int precomp_mdlen);

CMS_ContentInfo *ossl_cms_DigestedData_create(const EVP_MD *md,
                                              OSSL_LIB_CTX *libctx,
                                              const char *propq);
BIO *ossl_cms_DigestedData_init_bio(const CMS_ContentInfo *cms);
int ossl_cms_DigestedData_do_final(const CMS_ContentInfo *cms,
                                   BIO *chain, int verify);

BIO *ossl_cms_SignedData_init_bio(CMS_ContentInfo *cms);
int ossl_cms_SignedData_final(CMS_ContentInfo *cms, BIO *chain,
                              const unsigned char *precomp_md,
                              unsigned int precomp_mdlen);
int ossl_cms_set1_SignerIdentifier(CMS_SignerIdentifier *sid, X509 *cert,
                                   int type, const CMS_CTX *ctx);
int ossl_cms_SignerIdentifier_get0_signer_id(CMS_SignerIdentifier *sid,
                                             ASN1_OCTET_STRING **keyid,
                                             X509_NAME **issuer,
                                             ASN1_INTEGER **sno);
int ossl_cms_SignerIdentifier_cert_cmp(CMS_SignerIdentifier *sid, X509 *cert);

CMS_ContentInfo *ossl_cms_CompressedData_create(int comp_nid,
                                                OSSL_LIB_CTX *libctx,
                                                const char *propq);
BIO *ossl_cms_CompressedData_init_bio(const CMS_ContentInfo *cms);

BIO *ossl_cms_DigestAlgorithm_init_bio(X509_ALGOR *digestAlgorithm,
                                       const CMS_CTX *ctx);
int ossl_cms_DigestAlgorithm_find_ctx(EVP_MD_CTX *mctx, BIO *chain,
                                      X509_ALGOR *mdalg);

int ossl_cms_ias_cert_cmp(CMS_IssuerAndSerialNumber *ias, X509 *cert);
int ossl_cms_keyid_cert_cmp(ASN1_OCTET_STRING *keyid, X509 *cert);
int ossl_cms_set1_ias(CMS_IssuerAndSerialNumber **pias, X509 *cert);
int ossl_cms_set1_keyid(ASN1_OCTET_STRING **pkeyid, X509 *cert);

BIO *ossl_cms_EncryptedContent_init_bio(CMS_EncryptedContentInfo *ec,
                                        const CMS_CTX *ctx);
BIO *ossl_cms_EncryptedData_init_bio(const CMS_ContentInfo *cms);
int ossl_cms_EncryptedContent_init(CMS_EncryptedContentInfo *ec,
                                   const EVP_CIPHER *cipher,
                                   const unsigned char *key, size_t keylen,
                                   const CMS_CTX *ctx);

int ossl_cms_Receipt_verify(CMS_ContentInfo *cms, CMS_ContentInfo *req_cms);
int ossl_cms_msgSigDigest_add1(CMS_SignerInfo *dest, CMS_SignerInfo *src);
ASN1_OCTET_STRING *ossl_cms_encode_Receipt(CMS_SignerInfo *si);

BIO *ossl_cms_EnvelopedData_init_bio(CMS_ContentInfo *cms);
int ossl_cms_EnvelopedData_final(CMS_ContentInfo *cms, BIO *chain);
BIO *ossl_cms_AuthEnvelopedData_init_bio(CMS_ContentInfo *cms);
int ossl_cms_AuthEnvelopedData_final(CMS_ContentInfo *cms, BIO *cmsbio);
CMS_EnvelopedData *ossl_cms_get0_enveloped(CMS_ContentInfo *cms);
CMS_AuthEnvelopedData *ossl_cms_get0_auth_enveloped(CMS_ContentInfo *cms);
CMS_EncryptedContentInfo *ossl_cms_get0_env_enc_content(const CMS_ContentInfo *cms);

/* RecipientInfo routines */
int ossl_cms_env_asn1_ctrl(CMS_RecipientInfo *ri, int cmd);
int ossl_cms_pkey_get_ri_type(EVP_PKEY *pk);
int ossl_cms_pkey_is_ri_type_supported(EVP_PKEY *pk, int ri_type);

void ossl_cms_RecipientInfos_set_cmsctx(CMS_ContentInfo *cms);

/* KARI routines */
int ossl_cms_RecipientInfo_kari_init(CMS_RecipientInfo *ri, X509 *recip,
                                     EVP_PKEY *recipPubKey, X509 *originator,
                                     EVP_PKEY *originatorPrivKey,
                                     unsigned int flags,
                                     const CMS_CTX *ctx);
int ossl_cms_RecipientInfo_kari_encrypt(const CMS_ContentInfo *cms,
                                        CMS_RecipientInfo *ri);

/* PWRI routines */
int ossl_cms_RecipientInfo_pwri_crypt(const CMS_ContentInfo *cms,
                                      CMS_RecipientInfo *ri, int en_de);
/* SignerInfo routines */
int ossl_cms_si_check_attributes(const CMS_SignerInfo *si);
void ossl_cms_SignerInfos_set_cmsctx(CMS_ContentInfo *cms);


/* ESS routines */
int ossl_cms_check_signing_certs(const CMS_SignerInfo *si,
                                 const STACK_OF(X509) *chain);

int ossl_cms_dh_envelope(CMS_RecipientInfo *ri, int decrypt);
int ossl_cms_ecdh_envelope(CMS_RecipientInfo *ri, int decrypt);
int ossl_cms_rsa_envelope(CMS_RecipientInfo *ri, int decrypt);
int ossl_cms_rsa_sign(CMS_SignerInfo *si, int verify);

int ossl_cms_get1_certs_ex(CMS_ContentInfo *cms, STACK_OF(X509) **certs);
int ossl_cms_get1_crls_ex(CMS_ContentInfo *cms, STACK_OF(X509_CRL) **crls);

DECLARE_ASN1_ITEM(CMS_CertificateChoices)
DECLARE_ASN1_ITEM(CMS_DigestedData)
DECLARE_ASN1_ITEM(CMS_EncryptedData)
DECLARE_ASN1_ITEM(CMS_EnvelopedData)
DECLARE_ASN1_ITEM(CMS_AuthEnvelopedData)
DECLARE_ASN1_ITEM(CMS_KEKRecipientInfo)
DECLARE_ASN1_ITEM(CMS_KeyAgreeRecipientInfo)
DECLARE_ASN1_ITEM(CMS_KeyTransRecipientInfo)
DECLARE_ASN1_ITEM(CMS_OriginatorPublicKey)
DECLARE_ASN1_ITEM(CMS_OtherKeyAttribute)
DECLARE_ASN1_ITEM(CMS_Receipt)
DECLARE_ASN1_ITEM(CMS_ReceiptRequest)
DECLARE_ASN1_ITEM(CMS_RecipientEncryptedKey)
DECLARE_ASN1_ITEM(CMS_RecipientKeyIdentifier)
DECLARE_ASN1_ITEM(CMS_RevocationInfoChoice)
DECLARE_ASN1_ITEM(CMS_SignedData)
DECLARE_ASN1_ITEM(CMS_CompressedData)

#endif
