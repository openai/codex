/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rc5.h>
#include "prov/ciphercommon.h"

typedef struct prov_rc5_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        RC5_32_KEY ks;         /* key schedule */
    } ks;
    unsigned int rounds;       /* number of rounds */
} PROV_RC5_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc5_cbc(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc5_ecb(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc5_ofb64(size_t keybits);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc5_cfb64(size_t keybits);
