/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"
#include "crypto/sm4.h"
#include "crypto/sm4_platform.h"

typedef struct prov_cast_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        SM4_KEY ks;
    } ks;
} PROV_SM4_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_cbc(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_ecb(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_ctr(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_ofb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_cfb128(size_t keybits);
