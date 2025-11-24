/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * CAST low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "cipher_cast.h"

static int cipher_hw_cast5_initkey(PROV_CIPHER_CTX *ctx,
                                   const unsigned char *key, size_t keylen)
{
    PROV_CAST_CTX *bctx =  (PROV_CAST_CTX *)ctx;

    CAST_set_key(&(bctx->ks.ks), keylen, key);
    return 1;
}

# define PROV_CIPHER_HW_cast_mode(mode, UCMODE)                                \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, cast5, PROV_CAST_CTX, CAST_KEY,             \
                             CAST_##mode)                                      \
static const PROV_CIPHER_HW cast5_##mode = {                                   \
    cipher_hw_cast5_initkey,                                                   \
    cipher_hw_cast5_##mode##_cipher                                            \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_cast5_##mode(size_t keybits)         \
{                                                                              \
    return &cast5_##mode;                                                      \
}

PROV_CIPHER_HW_cast_mode(cbc, CBC)
PROV_CIPHER_HW_cast_mode(ecb, ECB)
PROV_CIPHER_HW_cast_mode(ofb64, OFB)
PROV_CIPHER_HW_cast_mode(cfb64, CFB)
