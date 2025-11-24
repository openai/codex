/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_types.h"
#include <openssl/rand.h>
#include <openssl/err.h>

int ossl_quic_gen_rand_conn_id(OSSL_LIB_CTX *libctx, size_t len,
                               QUIC_CONN_ID *cid)
{
    if (len > QUIC_MAX_CONN_ID_LEN)
        return 0;

    cid->id_len = (unsigned char)len;

    if (RAND_bytes_ex(libctx, cid->id, len, len * 8) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_RAND_LIB);
        cid->id_len = 0;
        return 0;
    }

    return 1;
}
