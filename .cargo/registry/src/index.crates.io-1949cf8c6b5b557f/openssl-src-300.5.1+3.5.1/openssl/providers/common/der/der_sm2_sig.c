/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/obj_mac.h>
#include "internal/packet.h"
#include "prov/der_sm2.h"

/* Aliases so we can have a uniform MD_CASE */
#define ossl_der_oid_id_sm2_with_sm3   ossl_der_oid_sm2_with_SM3

#define MD_CASE(name)                                                   \
    case NID_##name:                                                    \
        precompiled = ossl_der_oid_id_sm2_with_##name;                  \
        precompiled_sz = sizeof(ossl_der_oid_id_sm2_with_##name);       \
        break;

int ossl_DER_w_algorithmIdentifier_SM2_with_MD(WPACKET *pkt, int cont,
                                               EC_KEY *ec, int mdnid)
{
    const unsigned char *precompiled = NULL;
    size_t precompiled_sz = 0;

    switch (mdnid) {
        MD_CASE(sm3);
    default:
        return 0;
    }

    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, precompiled, precompiled_sz)
        && ossl_DER_w_end_sequence(pkt, cont);
}
