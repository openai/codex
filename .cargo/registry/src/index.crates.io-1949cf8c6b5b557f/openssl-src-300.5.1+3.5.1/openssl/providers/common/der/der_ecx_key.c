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
#include "prov/der_ecx.h"

int ossl_DER_w_algorithmIdentifier_X25519(WPACKET *pkt, int cont, ECX_KEY *ec)
{
    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, ossl_der_oid_id_X25519,
                                  sizeof(ossl_der_oid_id_X25519))
        && ossl_DER_w_end_sequence(pkt, cont);
}

int ossl_DER_w_algorithmIdentifier_X448(WPACKET *pkt, int cont, ECX_KEY *ec)
{
    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, ossl_der_oid_id_X448,
                                  sizeof(ossl_der_oid_id_X448))
        && ossl_DER_w_end_sequence(pkt, cont);
}

int ossl_DER_w_algorithmIdentifier_ED25519(WPACKET *pkt, int cont, ECX_KEY *ec)
{
    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, ossl_der_oid_id_Ed25519,
                                  sizeof(ossl_der_oid_id_Ed25519))
        && ossl_DER_w_end_sequence(pkt, cont);
}

int ossl_DER_w_algorithmIdentifier_ED448(WPACKET *pkt, int cont, ECX_KEY *ec)
{
    return ossl_DER_w_begin_sequence(pkt, cont)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, ossl_der_oid_id_Ed448,
                                  sizeof(ossl_der_oid_id_Ed448))
        && ossl_DER_w_end_sequence(pkt, cont);
}
