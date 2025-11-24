/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/seed.h>
#include "prov/ciphercommon.h"

typedef struct prov_seed_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        SEED_KEY_SCHEDULE ks;
    } ks;
} PROV_SEED_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_seed_cbc(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_seed_ecb(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_seed_ofb128(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_seed_cfb128(size_t keybits);
