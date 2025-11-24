/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rc4.h>
#include "prov/ciphercommon.h"

typedef struct prov_rc4_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        RC4_KEY ks;
    } ks;
} PROV_RC4_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc4(size_t keybits);
