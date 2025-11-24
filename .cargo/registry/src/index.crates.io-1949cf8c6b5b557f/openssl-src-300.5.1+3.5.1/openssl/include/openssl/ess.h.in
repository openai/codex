/*
 * {- join("\n * ", @autowarntext) -}
 *
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

{-
use OpenSSL::stackhash qw(generate_stack_macros);
-}

#ifndef OPENSSL_ESS_H
# define OPENSSL_ESS_H
# pragma once

# include <openssl/opensslconf.h>

# include <openssl/safestack.h>
# include <openssl/x509.h>
# include <openssl/esserr.h>

# ifdef  __cplusplus
extern "C" {
# endif


typedef struct ESS_issuer_serial ESS_ISSUER_SERIAL;
typedef struct ESS_cert_id ESS_CERT_ID;
typedef struct ESS_signing_cert ESS_SIGNING_CERT;

{-
    generate_stack_macros("ESS_CERT_ID");
-}


typedef struct ESS_signing_cert_v2_st ESS_SIGNING_CERT_V2;
typedef struct ESS_cert_id_v2_st ESS_CERT_ID_V2;

{-
    generate_stack_macros("ESS_CERT_ID_V2");
-}

DECLARE_ASN1_ALLOC_FUNCTIONS(ESS_ISSUER_SERIAL)
DECLARE_ASN1_ENCODE_FUNCTIONS_only(ESS_ISSUER_SERIAL, ESS_ISSUER_SERIAL)
DECLARE_ASN1_DUP_FUNCTION(ESS_ISSUER_SERIAL)

DECLARE_ASN1_ALLOC_FUNCTIONS(ESS_CERT_ID)
DECLARE_ASN1_ENCODE_FUNCTIONS_only(ESS_CERT_ID, ESS_CERT_ID)
DECLARE_ASN1_DUP_FUNCTION(ESS_CERT_ID)

DECLARE_ASN1_FUNCTIONS(ESS_SIGNING_CERT)
DECLARE_ASN1_DUP_FUNCTION(ESS_SIGNING_CERT)

DECLARE_ASN1_ALLOC_FUNCTIONS(ESS_CERT_ID_V2)
DECLARE_ASN1_ENCODE_FUNCTIONS_only(ESS_CERT_ID_V2, ESS_CERT_ID_V2)
DECLARE_ASN1_DUP_FUNCTION(ESS_CERT_ID_V2)

DECLARE_ASN1_FUNCTIONS(ESS_SIGNING_CERT_V2)
DECLARE_ASN1_DUP_FUNCTION(ESS_SIGNING_CERT_V2)

ESS_SIGNING_CERT *OSSL_ESS_signing_cert_new_init(const X509 *signcert,
                                                 const STACK_OF(X509) *certs,
                                                 int set_issuer_serial);
ESS_SIGNING_CERT_V2 *OSSL_ESS_signing_cert_v2_new_init(const EVP_MD *hash_alg,
                                                       const X509 *signcert,
                                                       const
                                                       STACK_OF(X509) *certs,
                                                       int set_issuer_serial);
int OSSL_ESS_check_signing_certs(const ESS_SIGNING_CERT *ss,
                                 const ESS_SIGNING_CERT_V2 *ssv2,
                                 const STACK_OF(X509) *chain,
                                 int require_signing_cert);

# ifdef  __cplusplus
}
# endif
#endif
