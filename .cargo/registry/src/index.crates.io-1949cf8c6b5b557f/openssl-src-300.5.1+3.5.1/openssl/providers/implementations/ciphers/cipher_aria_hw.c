/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/proverr.h>
#include "cipher_aria.h"

static int cipher_hw_aria_initkey(PROV_CIPHER_CTX *dat,
                                  const unsigned char *key, size_t keylen)
{
    int ret, mode = dat->mode;
    PROV_ARIA_CTX *adat = (PROV_ARIA_CTX *)dat;
    ARIA_KEY *ks = &adat->ks.ks;

    if (dat->enc || (mode != EVP_CIPH_ECB_MODE && mode != EVP_CIPH_CBC_MODE))
        ret = ossl_aria_set_encrypt_key(key, keylen * 8, ks);
    else
        ret = ossl_aria_set_decrypt_key(key, keylen * 8, ks);
    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }
    dat->ks = ks;
    dat->block = (block128_f)ossl_aria_encrypt;
    return 1;
}

IMPLEMENT_CIPHER_HW_COPYCTX(cipher_hw_aria_copyctx, PROV_ARIA_CTX)

# define PROV_CIPHER_HW_aria_mode(mode)                                        \
static const PROV_CIPHER_HW aria_##mode = {                                    \
    cipher_hw_aria_initkey,                                                    \
    ossl_cipher_hw_chunked_##mode,                                             \
    cipher_hw_aria_copyctx                                                     \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_aria_##mode(size_t keybits)          \
{                                                                              \
    return &aria_##mode;                                                       \
}

PROV_CIPHER_HW_aria_mode(cbc)
PROV_CIPHER_HW_aria_mode(ecb)
PROV_CIPHER_HW_aria_mode(ofb128)
PROV_CIPHER_HW_aria_mode(cfb128)
PROV_CIPHER_HW_aria_mode(cfb1)
PROV_CIPHER_HW_aria_mode(cfb8)
PROV_CIPHER_HW_aria_mode(ctr)
