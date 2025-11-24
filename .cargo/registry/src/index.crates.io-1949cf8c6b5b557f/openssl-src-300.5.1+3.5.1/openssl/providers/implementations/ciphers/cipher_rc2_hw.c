/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RC2 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_rc2.h"

static int cipher_hw_rc2_initkey(PROV_CIPHER_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_RC2_CTX *rctx =  (PROV_RC2_CTX *)ctx;
    RC2_KEY *ks = &(rctx->ks.ks);

    RC2_set_key(ks, (int)ctx->keylen, key, (int)rctx->key_bits);
    return 1;
}

# define PROV_CIPHER_HW_rc2_mode(mode, UCMODE)                                 \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, rc2, PROV_RC2_CTX, RC2_KEY,                 \
                             RC2_##mode)                                       \
static const PROV_CIPHER_HW rc2_##mode = {                                     \
    cipher_hw_rc2_initkey,                                                     \
    cipher_hw_rc2_##mode##_cipher                                              \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc2_##mode(size_t keybits)           \
{                                                                              \
    return &rc2_##mode;                                                        \
}

PROV_CIPHER_HW_rc2_mode(cbc, CBC)
PROV_CIPHER_HW_rc2_mode(ecb, ECB)
PROV_CIPHER_HW_rc2_mode(ofb64, OFB)
PROV_CIPHER_HW_rc2_mode(cfb64, CFB)
