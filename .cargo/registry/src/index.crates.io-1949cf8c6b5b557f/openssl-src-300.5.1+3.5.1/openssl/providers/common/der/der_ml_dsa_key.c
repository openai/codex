/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/packet.h"
#include "prov/der_ml_dsa.h"

int ossl_DER_w_algorithmIdentifier_ML_DSA(WPACKET *pkt, int tag, ML_DSA_KEY *key)
{
    const uint8_t *alg;
    size_t len;
    const char *name = ossl_ml_dsa_key_get_name(key);

    if (OPENSSL_strcasecmp(name, "ML-DSA-44") == 0) {
        alg = ossl_der_oid_id_ml_dsa_44;
        len = sizeof(ossl_der_oid_id_ml_dsa_44);
    } else if (OPENSSL_strcasecmp(name, "ML-DSA-65") == 0) {
        alg = ossl_der_oid_id_ml_dsa_65;
        len = sizeof(ossl_der_oid_id_ml_dsa_65);
    } else if (OPENSSL_strcasecmp(name, "ML-DSA-87") == 0) {
        alg = ossl_der_oid_id_ml_dsa_87;
        len = sizeof(ossl_der_oid_id_ml_dsa_87);
    } else {
        return 0;
    }
    return ossl_DER_w_begin_sequence(pkt, tag)
        /* No parameters */
        && ossl_DER_w_precompiled(pkt, -1, alg, len)
        && ossl_DER_w_end_sequence(pkt, tag);
}
