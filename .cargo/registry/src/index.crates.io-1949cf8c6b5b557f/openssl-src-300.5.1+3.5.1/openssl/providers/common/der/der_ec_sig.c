/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/obj_mac.h>
#include "internal/packet.h"
#include "prov/der_ec.h"

/* Aliases so we can have a uniform MD_CASE */
#define ossl_der_oid_id_ecdsa_with_sha1   ossl_der_oid_ecdsa_with_SHA1
#define ossl_der_oid_id_ecdsa_with_sha224 ossl_der_oid_ecdsa_with_SHA224
#define ossl_der_oid_id_ecdsa_with_sha256 ossl_der_oid_ecdsa_with_SHA256
#define ossl_der_oid_id_ecdsa_with_sha384 ossl_der_oid_ecdsa_with_SHA384
#define ossl_der_oid_id_ecdsa_with_sha512 ossl_der_oid_ecdsa_with_SHA512

#define MD_CASE(name)                                                   \
    case NID_##name:                                                    \
        precompiled = ossl_der_oid_id_ecdsa_with_##name;                \
        precompiled_sz = sizeof(ossl_der_oid_id_ecdsa_with_##name);     \
        break;

int ossl_DER_w_algorithmIdentifier_ECDSA_with_MD(WPACKET *pkt, int cont,
                                                 EC_KEY *ec, int mdnid)
{
    const unsigned char *precompiled = NULL;
    size_t precompiled_sz = 0;

    switch (mdnid) {
        MD_CASE(sha1);
        MD_CASE(sha224);
        MD_CASE(sha256);
        MD_CASE(sha384);
        MD_CASE(sha512);
        MD_CASE(sha3_224);
        MD_CASE(sha3_256);
        MD_CASE(sha3_384);
        MD_CASE(sha3_512);
    default:
        return 0;
    }

    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, precompiled, precompiled_sz)
        && ossl_DER_w_end_sequence(pkt, cont);
}
