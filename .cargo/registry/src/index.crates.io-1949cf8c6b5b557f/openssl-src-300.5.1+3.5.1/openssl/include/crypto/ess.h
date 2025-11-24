/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ESS_H
# define OSSL_CRYPTO_ESS_H
# pragma once

/*-
 * IssuerSerial ::= SEQUENCE {
 *        issuer                  GeneralNames,
 *        serialNumber            CertificateSerialNumber
 * }
 */

struct ESS_issuer_serial {
    STACK_OF(GENERAL_NAME) *issuer;
    ASN1_INTEGER *serial;
};

/*-
 * ESSCertID ::=  SEQUENCE {
 *        certHash                Hash,
 *        issuerSerial            IssuerSerial OPTIONAL
 * }
 */

struct ESS_cert_id {
    ASN1_OCTET_STRING *hash;    /* Always SHA-1 digest. */
    ESS_ISSUER_SERIAL *issuer_serial;
};

/*-
 * SigningCertificate ::=  SEQUENCE {
 *        certs                   SEQUENCE OF ESSCertID,
 *        policies                SEQUENCE OF PolicyInformation OPTIONAL
 * }
 */

struct ESS_signing_cert {
    STACK_OF(ESS_CERT_ID) *cert_ids;
    STACK_OF(POLICYINFO) *policy_info;
};

/*-
 * ESSCertIDv2 ::=  SEQUENCE {
 *        hashAlgorithm           AlgorithmIdentifier DEFAULT id-sha256,
 *        certHash                Hash,
 *        issuerSerial            IssuerSerial OPTIONAL
 * }
 */

struct ESS_cert_id_v2_st {
    X509_ALGOR *hash_alg;       /* Default: SHA-256 */
    ASN1_OCTET_STRING *hash;
    ESS_ISSUER_SERIAL *issuer_serial;
};

/*-
 * SigningCertificateV2 ::= SEQUENCE {
 *        certs                   SEQUENCE OF ESSCertIDv2,
 *        policies                SEQUENCE OF PolicyInformation OPTIONAL
 * }
 */

struct ESS_signing_cert_v2_st {
    STACK_OF(ESS_CERT_ID_V2) *cert_ids;
    STACK_OF(POLICYINFO) *policy_info;
};

#endif /* OSSL_CRYPTO_ESS_H */
