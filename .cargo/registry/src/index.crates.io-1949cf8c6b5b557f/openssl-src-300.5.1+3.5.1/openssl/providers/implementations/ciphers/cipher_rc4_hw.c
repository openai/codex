/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RC4 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_rc4.h"

static int cipher_hw_rc4_initkey(PROV_CIPHER_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_RC4_CTX *rctx =  (PROV_RC4_CTX *)ctx;

    RC4_set_key(&rctx->ks.ks, keylen, key);
    return 1;
}

static int cipher_hw_rc4_cipher(PROV_CIPHER_CTX *ctx, unsigned char *out,
                                const unsigned char *in, size_t len)
{
    PROV_RC4_CTX *rctx =  (PROV_RC4_CTX *)ctx;

    RC4(&rctx->ks.ks, len, in, out);
    return 1;
}

static const PROV_CIPHER_HW rc4_hw = {
    cipher_hw_rc4_initkey,
    cipher_hw_rc4_cipher
};
const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc4(size_t keybits)
{
    return &rc4_hw;
}

