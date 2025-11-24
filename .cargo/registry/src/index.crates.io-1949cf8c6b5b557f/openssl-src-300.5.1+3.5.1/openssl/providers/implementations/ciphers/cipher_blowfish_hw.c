/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * BF low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_blowfish.h"

static int cipher_hw_blowfish_initkey(PROV_CIPHER_CTX *ctx,
                                      const unsigned char *key, size_t keylen)
{
    PROV_BLOWFISH_CTX *bctx =  (PROV_BLOWFISH_CTX *)ctx;

    BF_set_key(&bctx->ks.ks, keylen, key);
    return 1;
}

# define PROV_CIPHER_HW_blowfish_mode(mode, UCMODE)                            \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, blowfish, PROV_BLOWFISH_CTX, BF_KEY,        \
                             BF_##mode)                                        \
static const PROV_CIPHER_HW bf_##mode = {                                      \
    cipher_hw_blowfish_initkey,                                                \
    cipher_hw_blowfish_##mode##_cipher                                         \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_blowfish_##mode(size_t keybits)      \
{                                                                              \
    return &bf_##mode;                                                         \
}

PROV_CIPHER_HW_blowfish_mode(cbc, CBC)
PROV_CIPHER_HW_blowfish_mode(ecb, ECB)
PROV_CIPHER_HW_blowfish_mode(ofb64, OFB)
PROV_CIPHER_HW_blowfish_mode(cfb64, CFB)
