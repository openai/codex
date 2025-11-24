/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
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

#include <openssl/obj_mac.h>
#include "internal/packet.h"
#include "prov/der_dsa.h"

int ossl_DER_w_algorithmIdentifier_DSA(WPACKET *pkt, int tag, DSA *dsa)
{
    return ossl_DER_w_begin_sequence(pkt, tag)
        /* No parameters (yet?) */
        && ossl_DER_w_precompiled(pkt, -1, ossl_der_oid_id_dsa,
                                  sizeof(ossl_der_oid_id_dsa))
        && ossl_DER_w_end_sequence(pkt, tag);
}
