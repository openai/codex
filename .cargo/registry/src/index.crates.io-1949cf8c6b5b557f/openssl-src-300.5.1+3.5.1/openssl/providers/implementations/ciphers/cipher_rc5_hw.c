/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RC5 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_rc5.h"

static int cipher_hw_rc5_initkey(PROV_CIPHER_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_RC5_CTX *rctx = (PROV_RC5_CTX *)ctx;

    return RC5_32_set_key(&rctx->ks.ks, keylen, key, rctx->rounds);
}

# define PROV_CIPHER_HW_rc5_mode(mode, UCMODE)                                 \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, rc5, PROV_RC5_CTX, RC5_32_KEY,              \
                             RC5_32_##mode)                                    \
static const PROV_CIPHER_HW rc5_##mode = {                                     \
    cipher_hw_rc5_initkey,                                                     \
    cipher_hw_rc5_##mode##_cipher                                              \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc5_##mode(size_t keybits)           \
{                                                                              \
    return &rc5_##mode;                                                        \
}

PROV_CIPHER_HW_rc5_mode(cbc, CBC)
PROV_CIPHER_HW_rc5_mode(ecb, ECB)
PROV_CIPHER_HW_rc5_mode(ofb64, OFB)
PROV_CIPHER_HW_rc5_mode(cfb64, CFB)
