/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This table MUST be kept in ascending order of the NID each method
 * represents (corresponding to the pkey_id field) as OBJ_bsearch
 * is used to search it.
 */
static const EVP_PKEY_ASN1_METHOD *standard_methods[] = {
    &ossl_rsa_asn1_meths[0],
    &ossl_rsa_asn1_meths[1],
#ifndef OPENSSL_NO_DH
    &ossl_dh_asn1_meth,
#endif
#ifndef OPENSSL_NO_DSA
    &ossl_dsa_asn1_meths[0],
    &ossl_dsa_asn1_meths[1],
    &ossl_dsa_asn1_meths[2],
    &ossl_dsa_asn1_meths[3],
#endif
#ifndef OPENSSL_NO_EC
    &ossl_eckey_asn1_meth,
#endif
    &ossl_rsa_pss_asn1_meth,
#ifndef OPENSSL_NO_DH
    &ossl_dhx_asn1_meth,
#endif
#ifndef OPENSSL_NO_ECX
    &ossl_ecx25519_asn1_meth,
    &ossl_ecx448_asn1_meth,
    &ossl_ed25519_asn1_meth,
    &ossl_ed448_asn1_meth,
#endif
#ifndef OPENSSL_NO_SM2
    &ossl_sm2_asn1_meth,
#endif
};
