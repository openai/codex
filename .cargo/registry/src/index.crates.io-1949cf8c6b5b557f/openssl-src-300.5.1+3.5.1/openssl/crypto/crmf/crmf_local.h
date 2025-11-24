/*-
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * CRMF implementation by Martin Peylo, Miikka Viljanen, and David von Oheimb.
 */

#ifndef OSSL_CRYPTO_CRMF_LOCAL_H
# define OSSL_CRYPTO_CRMF_LOCAL_H

# include <openssl/crmf.h>
# include <openssl/cms.h> /* for CMS_EnvelopedData and CMS_SignedData */
# include <openssl/err.h>
# include "internal/crmf.h" /* for ossl_crmf_attributetypeandvalue_st */

/* explicit #includes not strictly needed since implied by the above: */
# include <openssl/types.h>
# include <openssl/safestack.h>
# include <openssl/x509.h>
# include <openssl/x509v3.h>

/*-
 * EncryptedValue ::= SEQUENCE {
 * intendedAlg   [0] AlgorithmIdentifier  OPTIONAL,
 *                  -- the intended algorithm for which the value will be used
 * symmAlg       [1] AlgorithmIdentifier  OPTIONAL,
 *                  -- the symmetric algorithm used to encrypt the value
 * encSymmKey    [2] BIT STRING           OPTIONAL,
 *                  -- the (encrypted) symmetric key used to encrypt the value
 * keyAlg        [3] AlgorithmIdentifier  OPTIONAL,
 *                  -- algorithm used to encrypt the symmetric key
 * valueHint     [4] OCTET STRING         OPTIONAL,
 *                  -- a brief description or identifier of the encValue content
 *                  -- (may be meaningful only to the sending entity, and
 *                  --  used only if EncryptedValue might be re-examined
 *                  --  by the sending entity in the future)
 * encValue      BIT STRING
 *                  -- the encrypted value itself
 * }
 */
struct ossl_crmf_encryptedvalue_st {
    X509_ALGOR *intendedAlg;      /* 0 */
    X509_ALGOR *symmAlg;          /* 1 */
    ASN1_BIT_STRING *encSymmKey;  /* 2 */
    X509_ALGOR *keyAlg;           /* 3 */
    ASN1_OCTET_STRING *valueHint; /* 4 */
    ASN1_BIT_STRING *encValue;
} /* OSSL_CRMF_ENCRYPTEDVALUE */;

/*
 *    EncryptedKey ::= CHOICE {
 *       encryptedValue        EncryptedValue, -- Deprecated
 *       envelopedData     [0] EnvelopedData }
 *       -- The encrypted private key MUST be placed in the envelopedData
 *       -- encryptedContentInfo encryptedContent OCTET STRING.
 */
# define OSSL_CRMF_ENCRYPTEDKEY_ENVELOPEDDATA 1

struct ossl_crmf_encryptedkey_st {
    int type;
    union {
        OSSL_CRMF_ENCRYPTEDVALUE *encryptedValue; /* 0 */ /* Deprecated */
# ifndef OPENSSL_NO_CMS
        CMS_EnvelopedData *envelopedData; /* 1 */
# endif
    } value;
} /* OSSL_CRMF_ENCRYPTEDKEY */;

/*-
 *  Attributes ::= SET OF Attribute
 *  => X509_ATTRIBUTE
 *
 *  PrivateKeyInfo ::= SEQUENCE {
 *     version                       INTEGER,
 *     privateKeyAlgorithm           AlgorithmIdentifier,
 *     privateKey                    OCTET STRING,
 *     attributes                    [0] IMPLICIT Attributes OPTIONAL
 *  }
 */
typedef struct ossl_crmf_privatekeyinfo_st {
    ASN1_INTEGER *version;
    X509_ALGOR *privateKeyAlgorithm;
    ASN1_OCTET_STRING *privateKey;
    STACK_OF(X509_ATTRIBUTE) *attributes; /* [ 0 ] */
} OSSL_CRMF_PRIVATEKEYINFO;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_PRIVATEKEYINFO)

/*-
 * section 4.2.1 Private Key Info Content Type
 * id-ct-encKeyWithID OBJECT IDENTIFIER ::= {id-ct 21}
 *
 * EncKeyWithID ::= SEQUENCE {
 * privateKey     PrivateKeyInfo,
 * identifier     CHOICE {
 *                      string         UTF8String,
 *                      generalName    GeneralName
 *                } OPTIONAL
 * }
 */
typedef struct ossl_crmf_enckeywithid_identifier_st {
    int type;
    union {
        ASN1_UTF8STRING *string;
        GENERAL_NAME *generalName;
    } value;
} OSSL_CRMF_ENCKEYWITHID_IDENTIFIER;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_ENCKEYWITHID_IDENTIFIER)

typedef struct ossl_crmf_enckeywithid_st {
    OSSL_CRMF_PRIVATEKEYINFO *privateKey;
    /* [0] */
    OSSL_CRMF_ENCKEYWITHID_IDENTIFIER *identifier;
} OSSL_CRMF_ENCKEYWITHID;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_ENCKEYWITHID)

/*-
 * CertId ::= SEQUENCE {
 *      issuer           GeneralName,
 *      serialNumber     INTEGER
 * }
 */
struct ossl_crmf_certid_st {
    GENERAL_NAME *issuer;
    ASN1_INTEGER *serialNumber;
} /* OSSL_CRMF_CERTID */;

/*-
 * SinglePubInfo ::= SEQUENCE {
 *  pubMethod        INTEGER {
 *      dontCare        (0),
 *      x500            (1),
 *      web             (2),
 *      ldap            (3) },
 *  pubLocation  GeneralName OPTIONAL
 * }
 */
struct ossl_crmf_singlepubinfo_st {
    ASN1_INTEGER *pubMethod;
    GENERAL_NAME *pubLocation;
} /* OSSL_CRMF_SINGLEPUBINFO */;
DEFINE_STACK_OF(OSSL_CRMF_SINGLEPUBINFO)
typedef STACK_OF(OSSL_CRMF_SINGLEPUBINFO) OSSL_CRMF_PUBINFOS;

/*-
 * PKIPublicationInfo ::= SEQUENCE {
 *      action     INTEGER {
 *                   dontPublish (0),
 *                   pleasePublish (1) },
 *      pubInfos   SEQUENCE SIZE (1..MAX) OF SinglePubInfo OPTIONAL
 *      -- pubInfos MUST NOT be present if action is "dontPublish"
 *      -- (if action is "pleasePublish" and pubInfos is omitted,
 *      -- "dontCare" is assumed)
 * }
 */
struct ossl_crmf_pkipublicationinfo_st {
    ASN1_INTEGER *action;
    OSSL_CRMF_PUBINFOS *pubInfos;
} /* OSSL_CRMF_PKIPUBLICATIONINFO */;
DECLARE_ASN1_DUP_FUNCTION(OSSL_CRMF_PKIPUBLICATIONINFO)

/*-
 * PKMACValue ::= SEQUENCE {
 * algId  AlgorithmIdentifier,
 * -- algorithm value shall be PasswordBasedMac {1 2 840 113533 7 66 13}
 * -- parameter value is PBMParameter
 * value  BIT STRING
 * }
 */
typedef struct ossl_crmf_pkmacvalue_st {
    X509_ALGOR *algId;
    ASN1_BIT_STRING *value;
} OSSL_CRMF_PKMACVALUE;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_PKMACVALUE)

/*-
 * SubsequentMessage ::= INTEGER {
 * encrCert (0),
 * -- requests that resulting certificate be encrypted for the
 * -- end entity (following which, POP will be proven in a
 * -- confirmation message)
 * challengeResp (1)
 * -- requests that CA engage in challenge-response exchange with
 * -- end entity in order to prove private key possession
 * }
 *
 * POPOPrivKey ::= CHOICE {
 * thisMessage       [0] BIT STRING,                 -- Deprecated
 * -- possession is proven in this message (which contains the private
 * -- key itself (encrypted for the CA))
 * subsequentMessage [1] SubsequentMessage,
 * -- possession will be proven in a subsequent message
 * dhMAC             [2] BIT STRING,                 -- Deprecated
 * agreeMAC          [3] PKMACValue,
 * encryptedKey      [4] EnvelopedData
 * }
 */

typedef struct ossl_crmf_popoprivkey_st {
    int type;
    union {
        ASN1_BIT_STRING *thisMessage; /* 0 */ /* Deprecated */
        ASN1_INTEGER *subsequentMessage; /* 1 */
        ASN1_BIT_STRING *dhMAC; /* 2 */ /* Deprecated */
        OSSL_CRMF_PKMACVALUE *agreeMAC; /* 3 */
        ASN1_NULL *encryptedKey; /* 4 */
        /* When supported, ASN1_NULL needs to be replaced by CMS_ENVELOPEDDATA */
    } value;
} OSSL_CRMF_POPOPRIVKEY;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_POPOPRIVKEY)

/*-
 * PBMParameter ::= SEQUENCE {
 *    salt                    OCTET STRING,
 *    owf                     AlgorithmIdentifier,
 *    -- AlgId for a One-Way Function (SHA-1 recommended)
 *    iterationCount          INTEGER,
 *    -- number of times the OWF is applied
 *    mac                     AlgorithmIdentifier
 *    -- the MAC AlgId (e.g., DES-MAC, Triple-DES-MAC [PKCS11],
 *    -- or HMAC [HMAC, RFC2202])
 * }
 */
struct ossl_crmf_pbmparameter_st {
    ASN1_OCTET_STRING *salt;
    X509_ALGOR *owf;
    ASN1_INTEGER *iterationCount;
    X509_ALGOR *mac;
} /* OSSL_CRMF_PBMPARAMETER */;
# define OSSL_CRMF_PBM_MAX_ITERATION_COUNT 100000 /* if too large allows DoS */

/*-
 * POPOSigningKeyInput ::= SEQUENCE {
 * authInfo       CHOICE {
 *     sender                 [0] GeneralName,
 *   -- used only if an authenticated identity has been
 *   -- established for the sender (e.g., a DN from a
 *   -- previously-issued and currently-valid certificate)
 *     publicKeyMAC           PKMACValue },
 *   -- used if no authenticated GeneralName currently exists for
 *   -- the sender; publicKeyMAC contains a password-based MAC
 *   -- on the DER-encoded value of publicKey
 * publicKey      SubjectPublicKeyInfo  -- from CertTemplate
 * }
 */
typedef struct ossl_crmf_poposigningkeyinput_authinfo_st {
    int type;
    union {
        /* 0 */ GENERAL_NAME *sender;
        /* 1 */ OSSL_CRMF_PKMACVALUE *publicKeyMAC;
    } value;
} OSSL_CRMF_POPOSIGNINGKEYINPUT_AUTHINFO;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_POPOSIGNINGKEYINPUT_AUTHINFO)

typedef struct ossl_crmf_poposigningkeyinput_st {
    OSSL_CRMF_POPOSIGNINGKEYINPUT_AUTHINFO *authInfo;
    X509_PUBKEY *publicKey;
} OSSL_CRMF_POPOSIGNINGKEYINPUT;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_POPOSIGNINGKEYINPUT)

/*-
 * POPOSigningKey ::= SEQUENCE {
 *  poposkInput           [0] POPOSigningKeyInput OPTIONAL,
 *  algorithmIdentifier   AlgorithmIdentifier,
 *  signature             BIT STRING
 * }
 */
struct ossl_crmf_poposigningkey_st {
    OSSL_CRMF_POPOSIGNINGKEYINPUT *poposkInput;
    X509_ALGOR *algorithmIdentifier;
    ASN1_BIT_STRING *signature;
} /* OSSL_CRMF_POPOSIGNINGKEY */;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_POPOSIGNINGKEY)

/*-
 * ProofOfPossession ::= CHOICE {
 *  raVerified        [0] NULL,
 *  -- used if the RA has already verified that the requester is in
 *  -- possession of the private key
 *  signature         [1] POPOSigningKey,
 *  keyEncipherment   [2] POPOPrivKey,
 *  keyAgreement      [3] POPOPrivKey
 * }
 */
typedef struct ossl_crmf_popo_st {
    int type;
    union {
        ASN1_NULL *raVerified; /* 0 */
        OSSL_CRMF_POPOSIGNINGKEY *signature; /* 1 */
        OSSL_CRMF_POPOPRIVKEY *keyEncipherment; /* 2 */
        OSSL_CRMF_POPOPRIVKEY *keyAgreement; /* 3 */
    } value;
} OSSL_CRMF_POPO;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_POPO)

/*-
 * OptionalValidity ::= SEQUENCE {
 *  notBefore      [0] Time OPTIONAL,
 *  notAfter       [1] Time OPTIONAL  -- at least one MUST be present
 * }
 */
struct ossl_crmf_optionalvalidity_st {
    /* 0 */ ASN1_TIME *notBefore;
    /* 1 */ ASN1_TIME *notAfter;
} /* OSSL_CRMF_OPTIONALVALIDITY */;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_OPTIONALVALIDITY)

/*-
 * CertTemplate ::= SEQUENCE {
 * version          [0] Version                   OPTIONAL,
 * serialNumber     [1] INTEGER                   OPTIONAL,
 * signingAlg       [2] AlgorithmIdentifier       OPTIONAL,
 * issuer           [3] Name                      OPTIONAL,
 * validity         [4] OptionalValidity          OPTIONAL,
 * subject          [5] Name                      OPTIONAL,
 * publicKey        [6] SubjectPublicKeyInfo      OPTIONAL,
 * issuerUID        [7] UniqueIdentifier          OPTIONAL,
 * subjectUID       [8] UniqueIdentifier          OPTIONAL,
 * extensions       [9] Extensions                OPTIONAL
 * }
 */
struct ossl_crmf_certtemplate_st {
    ASN1_INTEGER *version;
    ASN1_INTEGER *serialNumber; /* serialNumber MUST be omitted */
    /* This field is assigned by the CA during certificate creation */
    X509_ALGOR *signingAlg; /* signingAlg MUST be omitted */
    /* This field is assigned by the CA during certificate creation */
    const X509_NAME *issuer;
    OSSL_CRMF_OPTIONALVALIDITY *validity;
    const X509_NAME *subject;
    X509_PUBKEY *publicKey;
    ASN1_BIT_STRING *issuerUID; /* deprecated in version 2 */
    /* According to rfc 3280: UniqueIdentifier ::= BIT STRING */
    ASN1_BIT_STRING *subjectUID; /* deprecated in version 2 */
    /* Could be X509_EXTENSION*S*, but that's only cosmetic */
    STACK_OF(X509_EXTENSION) *extensions;
} /* OSSL_CRMF_CERTTEMPLATE */;

/*-
 * CertRequest ::= SEQUENCE {
 *  certReqId        INTEGER,          -- ID for matching request and reply
 *  certTemplate     CertTemplate,     -- Selected fields of cert to be issued
 *  controls         Controls OPTIONAL -- Attributes affecting issuance
 * }
 */
struct ossl_crmf_certrequest_st {
    ASN1_INTEGER *certReqId;
    OSSL_CRMF_CERTTEMPLATE *certTemplate;
    STACK_OF(OSSL_CRMF_ATTRIBUTETYPEANDVALUE /* Controls expanded */) *controls;
} /* OSSL_CRMF_CERTREQUEST */;
DECLARE_ASN1_FUNCTIONS(OSSL_CRMF_CERTREQUEST)
DECLARE_ASN1_DUP_FUNCTION(OSSL_CRMF_CERTREQUEST)

/*-
 * CertReqMessages ::= SEQUENCE SIZE (1..MAX) OF CertReqMsg
 * CertReqMsg ::= SEQUENCE {
 *  certReq        CertRequest,
 *  popo           ProofOfPossession  OPTIONAL,
 * -- content depends upon key type
 *  regInfo   SEQUENCE SIZE(1..MAX) OF AttributeTypeAndValue OPTIONAL
 * }
 */
struct ossl_crmf_msg_st {
    OSSL_CRMF_CERTREQUEST *certReq;
    /* 0 */
    OSSL_CRMF_POPO *popo;
    /* 1 */
    STACK_OF(OSSL_CRMF_ATTRIBUTETYPEANDVALUE) *regInfo;
} /* OSSL_CRMF_MSG */;
#endif
