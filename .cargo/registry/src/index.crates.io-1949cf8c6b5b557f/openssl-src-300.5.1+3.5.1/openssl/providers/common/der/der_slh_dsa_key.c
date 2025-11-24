/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/obj_mac.h>
#include <openssl/objects.h>
#include "internal/packet.h"
#include "prov/der_slh_dsa.h"

#define CASE_OID(nid, name)                  \
    case nid:                                \
        alg = ossl_der_oid_##name;           \
        len = sizeof(ossl_der_oid_##name);   \
        break

int ossl_DER_w_algorithmIdentifier_SLH_DSA(WPACKET *pkt, int tag, SLH_DSA_KEY *key)
{
    const uint8_t *alg;
    size_t len;
    int nid = ossl_slh_dsa_key_get_type(key);

    switch (nid) {
        CASE_OID(NID_SLH_DSA_SHA2_128s, id_slh_dsa_sha2_128s);
        CASE_OID(NID_SLH_DSA_SHA2_128f, id_slh_dsa_sha2_128f);
        CASE_OID(NID_SLH_DSA_SHA2_192s, id_slh_dsa_sha2_192s);
        CASE_OID(NID_SLH_DSA_SHA2_192f, id_slh_dsa_sha2_192f);
        CASE_OID(NID_SLH_DSA_SHA2_256s, id_slh_dsa_sha2_256s);
        CASE_OID(NID_SLH_DSA_SHA2_256f, id_slh_dsa_sha2_256f);
        CASE_OID(NID_SLH_DSA_SHAKE_128s, id_slh_dsa_shake_128s);
        CASE_OID(NID_SLH_DSA_SHAKE_128f, id_slh_dsa_shake_128f);
        CASE_OID(NID_SLH_DSA_SHAKE_192s, id_slh_dsa_shake_192s);
        CASE_OID(NID_SLH_DSA_SHAKE_192f, id_slh_dsa_shake_192f);
        CASE_OID(NID_SLH_DSA_SHAKE_256s, id_slh_dsa_shake_256s);
        CASE_OID(NID_SLH_DSA_SHAKE_256f, id_slh_dsa_shake_256f);
        default:
            return 0;
    }
    return ossl_DER_w_begin_sequence(pkt, tag)
        /* No parameters */
        && ossl_DER_w_precompiled(pkt, -1, alg, len)
        && ossl_DER_w_end_sequence(pkt, tag);
}
