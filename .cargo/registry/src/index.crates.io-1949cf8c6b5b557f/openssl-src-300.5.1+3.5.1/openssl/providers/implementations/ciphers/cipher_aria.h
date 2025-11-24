/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/aria.h"
#include "prov/ciphercommon.h"

typedef struct prov_aria_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        ARIA_KEY ks;
    } ks;
} PROV_ARIA_CTX;


#define ossl_prov_cipher_hw_aria_ofb ossl_prov_cipher_hw_aria_ofb128
#define ossl_prov_cipher_hw_aria_cfb ossl_prov_cipher_hw_aria_cfb128
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_ecb(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_cbc(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_ofb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_cfb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_cfb1(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_cfb8(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_ctr(size_t keybits);
